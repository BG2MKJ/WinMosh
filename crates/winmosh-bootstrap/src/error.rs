use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use winmosh_config::ConfigError;

#[derive(Debug)]
pub enum BootstrapError {
    InvalidServerOutput(String),
    OutputTooLarge {
        limit: usize,
    },
    SshConfig(ConfigError),
    SshStart {
        path: PathBuf,
        source: std::io::Error,
    },
    SshWait(std::io::Error),
    SshTimeout {
        timeout: Duration,
    },
    SshExit {
        code: Option<i32>,
        diagnostic: String,
    },
    OutputReader(String),
}

impl fmt::Display for BootstrapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidServerOutput(message) => {
                write!(formatter, "invalid mosh-server output: {message}")
            }
            Self::OutputTooLarge { limit } => {
                write!(
                    formatter,
                    "mosh-server output exceeded the {limit}-byte limit"
                )
            }
            Self::SshConfig(error) => error.fmt(formatter),
            Self::SshStart { path, source } => {
                write!(formatter, "failed to start {}: {source}", path.display())
            }
            Self::SshWait(error) => write!(formatter, "failed waiting for ssh.exe: {error}"),
            Self::SshTimeout { timeout } => {
                write!(
                    formatter,
                    "ssh.exe timed out after {} seconds",
                    timeout.as_secs()
                )
            }
            Self::SshExit { code, diagnostic } => match code {
                Some(code) => write!(formatter, "ssh.exe exited with code {code}: {diagnostic}"),
                None => write!(formatter, "ssh.exe exited without a status: {diagnostic}"),
            },
            Self::OutputReader(message) => {
                write!(formatter, "failed reading ssh output: {message}")
            }
        }
    }
}

impl std::error::Error for BootstrapError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SshConfig(error) => Some(error),
            Self::SshStart { source, .. } => Some(source),
            Self::SshWait(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ConfigError> for BootstrapError {
    fn from(error: ConfigError) -> Self {
        Self::SshConfig(error)
    }
}
