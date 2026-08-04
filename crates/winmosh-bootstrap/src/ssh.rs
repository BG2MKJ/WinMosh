use std::io::{self, Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use winmosh_config::ssh_config::find_ssh;
use winmosh_config::SSH_OUTPUT_LIMIT;

use crate::command::{build_remote_command, BootstrapRequest};
use crate::error::BootstrapError;
use crate::parser::{parse_mosh_server_output_bytes, BootstrapResult};

const POLL_INTERVAL: Duration = Duration::from_millis(10);

pub fn start_bootstrap(request: &BootstrapRequest) -> Result<BootstrapResult, BootstrapError> {
    let ssh_path = find_ssh(request.ssh_path.as_deref())?;
    let remote_command = build_remote_command(request);
    let mut child = Command::new(&ssh_path)
        .arg("-p")
        .arg(request.target.effective_ssh.port.to_string())
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("--")
        .arg(&request.target.ssh_target)
        .arg(remote_command)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| BootstrapError::SshStart {
            path: ssh_path.clone(),
            source,
        })?;

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| BootstrapError::OutputReader("ssh stdout was not captured".to_owned()))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| BootstrapError::OutputReader("ssh stderr was not captured".to_owned()))?;

    let stdout_handle = thread::spawn(move || read_limited(&mut stdout, SSH_OUTPUT_LIMIT));
    let stderr_handle =
        thread::spawn(move || read_and_forward_stderr(&mut stderr, SSH_OUTPUT_LIMIT));

    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() >= request.timeout => {
                let _ = child.kill();
                let _ = child.wait();
                join_output_threads(stdout_handle, stderr_handle)?;
                return Err(BootstrapError::SshTimeout {
                    timeout: request.timeout,
                });
            }
            Ok(None) => thread::sleep(POLL_INTERVAL),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                join_output_threads(stdout_handle, stderr_handle)?;
                return Err(BootstrapError::SshWait(error));
            }
        }
    };

    let stdout = stdout_handle
        .join()
        .map_err(|_| BootstrapError::OutputReader("stdout reader panicked".to_owned()))?
        .map_err(|error| BootstrapError::OutputReader(error.to_string()))?;
    let stderr = stderr_handle
        .join()
        .map_err(|_| BootstrapError::OutputReader("stderr reader panicked".to_owned()))?
        .map_err(|error| BootstrapError::OutputReader(error.to_string()))?;

    if stdout.truncated || stderr.truncated {
        return Err(BootstrapError::OutputTooLarge {
            limit: SSH_OUTPUT_LIMIT,
        });
    }

    if !status.success() {
        return Err(BootstrapError::SshExit {
            code: status.code(),
            diagnostic: diagnostic_text(&stderr.bytes, &stdout.bytes),
        });
    }

    parse_mosh_server_output_bytes(&stdout.bytes)
}

struct ReadResult {
    bytes: Vec<u8>,
    truncated: bool,
}

fn read_limited(reader: &mut impl Read, limit: usize) -> io::Result<ReadResult> {
    let mut bytes = Vec::new();
    let mut truncated = false;
    let mut buffer = [0_u8; 4096];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        if bytes.len() < limit {
            let remaining = limit - bytes.len();
            let to_copy = remaining.min(read);
            bytes.extend_from_slice(&buffer[..to_copy]);
            if to_copy < read {
                truncated = true;
            }
        } else {
            truncated = true;
        }
    }
    Ok(ReadResult { bytes, truncated })
}

fn read_and_forward_stderr(reader: &mut impl Read, limit: usize) -> io::Result<ReadResult> {
    let mut bytes = Vec::new();
    let mut truncated = false;
    let mut buffer = [0_u8; 4096];
    let mut output = io::stderr().lock();
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        output.write_all(&buffer[..read])?;
        output.flush()?;
        if bytes.len() < limit {
            let remaining = limit - bytes.len();
            let to_copy = remaining.min(read);
            bytes.extend_from_slice(&buffer[..to_copy]);
            if to_copy < read {
                truncated = true;
            }
        } else {
            truncated = true;
        }
    }
    Ok(ReadResult { bytes, truncated })
}

fn join_output_threads(
    stdout_handle: thread::JoinHandle<io::Result<ReadResult>>,
    stderr_handle: thread::JoinHandle<io::Result<ReadResult>>,
) -> Result<(), BootstrapError> {
    let stdout_result = stdout_handle
        .join()
        .map_err(|_| BootstrapError::OutputReader("stdout reader panicked".to_owned()))?;
    let stderr_result = stderr_handle
        .join()
        .map_err(|_| BootstrapError::OutputReader("stderr reader panicked".to_owned()))?;
    stdout_result.map_err(|error| BootstrapError::OutputReader(error.to_string()))?;
    stderr_result.map_err(|error| BootstrapError::OutputReader(error.to_string()))?;
    Ok(())
}

fn diagnostic_text(stderr: &[u8], stdout: &[u8]) -> String {
    first_non_empty_line(stderr)
        .or_else(|| first_non_empty_line(stdout))
        .map(|line| sanitize_diagnostic(&line))
        .unwrap_or_else(|| "no diagnostic output".to_owned())
}

fn first_non_empty_line(bytes: &[u8]) -> Option<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToOwned::to_owned)
}

fn sanitize_diagnostic(line: &str) -> String {
    if line.starts_with("MOSH CONNECT ") {
        "mosh-server returned a connection line while ssh.exe failed".to_owned()
    } else {
        line.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::diagnostic_text;

    #[test]
    fn diagnostics_never_echo_session_key() {
        let diagnostic =
            diagnostic_text(b"MOSH CONNECT 60024 AAECAwQFBgcICQoLDA0ODw\n", b"fallback");
        assert!(!diagnostic.contains("AAECAwQFBgcICQoLDA0ODw"));
        assert!(diagnostic.contains("connection line"));
    }
}
