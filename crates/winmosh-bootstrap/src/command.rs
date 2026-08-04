use std::path::PathBuf;
use std::time::Duration;

use winmosh_config::{PortSpec, ResolvedTarget};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapRequest {
    pub target: ResolvedTarget,
    pub columns: u16,
    pub rows: u16,
    pub locale: String,
    pub ssh_path: Option<PathBuf>,
    pub timeout: Duration,
}

pub fn build_server_command(request: &BootstrapRequest) -> Vec<String> {
    let mut command = vec![
        request.target.mosh_server.clone(),
        "new".to_owned(),
        "-s".to_owned(),
        "-c".to_owned(),
        terminal_color_count(&request.target.terminal).to_string(),
        "-l".to_owned(),
        format!("LANG={}", request.locale),
    ];
    match request.target.udp_port {
        PortSpec::Single(port) => {
            command.push("-p".to_owned());
            command.push(port.to_string());
        }
        PortSpec::Range { start, end } => {
            command.push("-p".to_owned());
            command.push(format!("{start}:{end}"));
        }
    }
    command
}

pub fn build_remote_command(request: &BootstrapRequest) -> String {
    build_server_command(request)
        .iter()
        .map(|argument| shell_quote(argument))
        .collect::<Vec<_>>()
        .join(" ")
}

fn terminal_color_count(terminal: &str) -> u16 {
    let terminal = terminal.to_ascii_lowercase();
    if terminal.contains("truecolor") || terminal.contains("24bit") {
        256
    } else if terminal.ends_with("-256color") || terminal.contains("-256") {
        256
    } else if terminal.ends_with("-88color") {
        88
    } else if terminal.ends_with("-16color") {
        16
    } else {
        8
    }
}

fn shell_quote(value: &str) -> String {
    if value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"_./:-=+".contains(&byte))
    {
        return value.to_owned();
    }

    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use winmosh_config::{
        AddressFamily, EffectiveSshConfig, PredictionMode, ResolvedTarget, SocketCandidate,
    };

    use super::{build_server_command, BootstrapRequest};

    #[test]
    fn builds_non_secret_server_command() {
        let target = ResolvedTarget {
            display_name: "myserver".to_owned(),
            alias_name: Some("myserver".to_owned()),
            ssh_target: "root@example.com".to_owned(),
            effective_ssh: EffectiveSshConfig {
                hostname: "example.com".to_owned(),
                user: "root".to_owned(),
                port: 22,
                address_family: AddressFamily::Auto,
            },
            udp_candidates: vec![SocketCandidate {
                host: "example.com".to_owned(),
                port: winmosh_config::PortSpec::Single(60024),
            }],
            mosh_server: "mosh-server".to_owned(),
            udp_port: winmosh_config::PortSpec::Single(60024),
            terminal: "xterm-256color".to_owned(),
            prediction: PredictionMode::Off,
        };

        let command = build_server_command(&BootstrapRequest {
            target,
            columns: 80,
            rows: 24,
            locale: "en_US.UTF-8".to_owned(),
            ssh_path: None,
            timeout: Duration::from_secs(10),
        });
        assert_eq!(command[0], "mosh-server");
        assert!(command.windows(2).any(|pair| pair == ["-p", "60024"]));
        assert!(command.windows(2).any(|pair| pair == ["-c", "256"]));
        assert!(command
            .windows(2)
            .any(|pair| pair == ["-l", "LANG=en_US.UTF-8"]));
    }

    #[test]
    fn quotes_remote_command_arguments() -> Result<(), Box<dyn std::error::Error>> {
        let mut request = sample_request();
        request.target.mosh_server = "/opt/my mosh-server".to_owned();
        let command = super::build_remote_command(&request);
        assert!(command.starts_with("'/opt/my mosh-server' new"));
        Ok(())
    }

    fn sample_request() -> BootstrapRequest {
        BootstrapRequest {
            target: ResolvedTarget {
                display_name: "myserver".to_owned(),
                alias_name: Some("myserver".to_owned()),
                ssh_target: "root@example.com".to_owned(),
                effective_ssh: EffectiveSshConfig {
                    hostname: "example.com".to_owned(),
                    user: "root".to_owned(),
                    port: 22,
                    address_family: AddressFamily::Auto,
                },
                udp_candidates: vec![SocketCandidate {
                    host: "example.com".to_owned(),
                    port: winmosh_config::PortSpec::Single(60024),
                }],
                mosh_server: "mosh-server".to_owned(),
                udp_port: winmosh_config::PortSpec::Single(60024),
                terminal: "xterm-256color".to_owned(),
                prediction: PredictionMode::Off,
            },
            columns: 80,
            rows: 24,
            locale: "en_US.UTF-8".to_owned(),
            ssh_path: None,
            timeout: Duration::from_secs(10),
        }
    }
}
