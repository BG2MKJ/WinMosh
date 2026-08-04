use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use winmosh_platform::process;

use crate::model::{AddressFamily, ConfigError, EffectiveSshConfig, SSH_OUTPUT_LIMIT};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshProbe {
    pub path: PathBuf,
    pub version: Option<String>,
}

pub fn find_ssh(explicit_path: Option<&Path>) -> Result<PathBuf, ConfigError> {
    if let Some(path) = explicit_path {
        if path.is_file() {
            return Ok(path.to_path_buf());
        }
        return Err(ConfigError::SshConfig(format!(
            "ssh executable does not exist: {}",
            path.display()
        )));
    }

    for candidate in common_windows_ssh_paths() {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    if let Some(path) = process::find_in_path("ssh") {
        return Ok(path);
    }

    Err(ConfigError::SshConfig(
        "ssh.exe was not found in PATH or common Windows OpenSSH locations".to_owned(),
    ))
}

pub fn probe_ssh(explicit_path: Option<&Path>, timeout: Duration) -> Result<SshProbe, ConfigError> {
    let path = find_ssh(explicit_path)?;
    let output = run_command_limited(&path, &["-V"], timeout, 32 * 1024)?;
    let version = first_non_empty_line(&output.stderr)
        .or_else(|| first_non_empty_line(&output.stdout))
        .map(ToOwned::to_owned);
    Ok(SshProbe { path, version })
}

pub fn read_effective_ssh_config(
    explicit_path: Option<&Path>,
    target: &str,
    timeout: Duration,
) -> Result<EffectiveSshConfig, ConfigError> {
    let path = find_ssh(explicit_path)?;
    let output = run_command_limited(&path, &["-G", "--", target], timeout, SSH_OUTPUT_LIMIT)?;
    if output.status != Some(0) {
        let message = first_non_empty_line(&output.stderr)
            .or_else(|| first_non_empty_line(&output.stdout))
            .unwrap_or("ssh -G failed without diagnostic output");
        return Err(ConfigError::SshConfig(format!(
            "ssh -G {target} failed: {message}"
        )));
    }
    parse_effective_ssh_config(&output.stdout, target)
}

pub fn parse_effective_ssh_config(
    input: &str,
    original_target: &str,
) -> Result<EffectiveSshConfig, ConfigError> {
    let mut hostname: Option<String> = None;
    let mut user: Option<String> = None;
    let mut port: Option<u16> = None;
    let mut family = AddressFamily::Auto;

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = split_ssh_config_line(trimmed) else {
            continue;
        };
        match key.to_ascii_lowercase().as_str() {
            "hostname" => hostname = Some(value.to_owned()),
            "user" => user = Some(value.to_owned()),
            "port" => {
                port = Some(value.parse::<u16>().map_err(|_| {
                    ConfigError::SshConfig(format!("ssh -G returned invalid port: {value}"))
                })?)
            }
            "addressfamily" => family = AddressFamily::parse(value)?,
            _ => {}
        }
    }

    let fallback = split_user_host(original_target);
    Ok(EffectiveSshConfig {
        hostname: hostname.unwrap_or_else(|| fallback.host.to_owned()),
        user: user.unwrap_or_else(|| fallback.user.unwrap_or_else(default_user)),
        port: port.unwrap_or(22),
        address_family: family,
    })
}

pub fn common_windows_ssh_paths() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(system_root) = std::env::var_os("SystemRoot") {
        candidates.push(
            PathBuf::from(system_root)
                .join("System32")
                .join("OpenSSH")
                .join("ssh.exe"),
        );
    }
    candidates.push(PathBuf::from(r"C:\Windows\System32\OpenSSH\ssh.exe"));
    candidates.push(PathBuf::from(r"C:\Program Files\OpenSSH\ssh.exe"));
    candidates
}

#[derive(Debug)]
struct CommandOutput {
    status: Option<i32>,
    stdout: String,
    stderr: String,
}

fn run_command_limited(
    path: &Path,
    args: &[&str],
    timeout: Duration,
    output_limit: usize,
) -> Result<CommandOutput, ConfigError> {
    let mut child = Command::new(path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| ConfigError::SshConfig(format!("failed to start ssh.exe: {error}")))?;

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| ConfigError::SshConfig("failed to capture ssh.exe stdout".to_owned()))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| ConfigError::SshConfig("failed to capture ssh.exe stderr".to_owned()))?;

    let stdout_handle = thread::spawn(move || read_limited(&mut stdout, output_limit));
    let stderr_handle = thread::spawn(move || read_limited(&mut stderr, output_limit));
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status.code().unwrap_or(-1)),
            Ok(None) if start.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(error) => {
                return Err(ConfigError::SshConfig(format!(
                    "ssh.exe wait failed: {error}"
                )))
            }
        }
    };

    if status.is_none() {
        return Err(ConfigError::SshConfig(format!(
            "ssh.exe timed out after {} seconds",
            timeout.as_secs()
        )));
    }

    let stdout = stdout_handle
        .join()
        .map_err(|_| ConfigError::SshConfig("ssh stdout reader panicked".to_owned()))?
        .map_err(|error| ConfigError::SshConfig(format!("failed reading ssh stdout: {error}")))?;
    let stderr = stderr_handle
        .join()
        .map_err(|_| ConfigError::SshConfig("ssh stderr reader panicked".to_owned()))?
        .map_err(|error| ConfigError::SshConfig(format!("failed reading ssh stderr: {error}")))?;

    Ok(CommandOutput {
        status,
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
    })
}

fn read_limited(reader: &mut impl Read, output_limit: usize) -> std::io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let remaining = output_limit.saturating_sub(output.len());
        if remaining == 0 {
            break;
        }
        let to_copy = remaining.min(read);
        output.extend_from_slice(&buffer[..to_copy]);
    }
    Ok(output)
}

fn split_ssh_config_line(line: &str) -> Option<(&str, &str)> {
    for (index, character) in line.char_indices() {
        if character.is_whitespace() {
            let key = &line[..index];
            let value = line[index..].trim();
            if !key.is_empty() && !value.is_empty() {
                return Some((key, value));
            }
            break;
        }
    }
    line.split_once('=').and_then(|(key, value)| {
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            None
        } else {
            Some((key, value))
        }
    })
}

struct TargetParts<'a> {
    user: Option<String>,
    host: &'a str,
}

fn split_user_host(target: &str) -> TargetParts<'_> {
    if let Some((user, host)) = target.rsplit_once('@') {
        if !user.is_empty() && !host.is_empty() {
            return TargetParts {
                user: Some(user.to_owned()),
                host,
            };
        }
    }
    TargetParts {
        user: None,
        host: target,
    }
}

fn default_user() -> String {
    std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "unknown".to_owned())
}

fn first_non_empty_line(input: &str) -> Option<&str> {
    input.lines().map(str::trim).find(|line| !line.is_empty())
}

#[cfg(test)]
mod tests {
    use crate::model::AddressFamily;

    use super::{parse_effective_ssh_config, split_ssh_config_line};

    #[test]
    fn parses_ssh_g_output() -> Result<(), Box<dyn std::error::Error>> {
        let config = parse_effective_ssh_config(
            include_str!("../../../tests/fixtures/ssh-config/basic.txt"),
            "ignored",
        )?;
        assert_eq!(config.hostname, "203.0.113.10");
        assert_eq!(config.user, "deploy");
        assert_eq!(config.port, 2222);
        assert_eq!(config.address_family, AddressFamily::Ipv4);
        Ok(())
    }

    #[test]
    fn falls_back_to_user_host_target() -> Result<(), Box<dyn std::error::Error>> {
        let config = parse_effective_ssh_config("", "root@example.com")?;
        assert_eq!(config.hostname, "example.com");
        assert_eq!(config.user, "root");
        assert_eq!(config.port, 22);
        Ok(())
    }

    #[test]
    fn splits_space_and_equals_lines() {
        assert_eq!(
            split_ssh_config_line("hostname example.com"),
            Some(("hostname", "example.com"))
        );
        assert_eq!(split_ssh_config_line("port=22"), Some(("port", "22")));
    }
}
