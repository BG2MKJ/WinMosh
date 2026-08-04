use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

pub const CONFIG_VERSION: u32 = 1;
pub const DEFAULT_MOSH_SERVER: &str = "mosh-server";
pub const DEFAULT_TERMINAL: &str = "xterm-256color";
pub const DEFAULT_UDP_PORT: PortSpec = PortSpec::Range {
    start: 60000,
    end: 61000,
};
pub const DEFAULT_PREDICTION: PredictionMode = PredictionMode::Off;
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub const SSH_OUTPUT_LIMIT: usize = 128 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub version: u32,
    pub defaults: Defaults,
    pub hosts: BTreeMap<String, HostConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            defaults: Defaults::default(),
            hosts: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Defaults {
    pub mosh_server: Option<String>,
    pub udp_port: Option<PortSpec>,
    pub terminal: Option<String>,
    pub prediction: Option<PredictionMode>,
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            mosh_server: Some(DEFAULT_MOSH_SERVER.to_owned()),
            udp_port: Some(DEFAULT_UDP_PORT),
            terminal: Some(DEFAULT_TERMINAL.to_owned()),
            prediction: Some(DEFAULT_PREDICTION),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostConfig {
    pub ssh_target: String,
    pub udp_host: Option<String>,
    pub udp_port: Option<PortSpec>,
    pub mosh_server: Option<String>,
    pub terminal: Option<String>,
    pub prediction: Option<PredictionMode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortSpec {
    Single(u16),
    Range { start: u16, end: u16 },
}

impl PortSpec {
    pub fn parse(input: &str) -> Result<Self, ConfigError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(ConfigError::invalid_port_spec(input));
        }

        if let Some((start, end)) = trimmed.split_once(':') {
            let start = parse_port(start, input)?;
            let end = parse_port(end, input)?;
            if start > end {
                return Err(ConfigError::invalid_port_spec(input));
            }
            return Ok(Self::Range { start, end });
        }

        Ok(Self::Single(parse_port(trimmed, input)?))
    }

    pub fn first_port(self) -> u16 {
        match self {
            Self::Single(port) => port,
            Self::Range { start, .. } => start,
        }
    }
}

impl fmt::Display for PortSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Single(port) => write!(formatter, "{port}"),
            Self::Range { start, end } => write!(formatter, "{start}:{end}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PredictionMode {
    Off,
    Adaptive,
    Always,
    Never,
}

impl PredictionMode {
    pub fn parse(input: &str) -> Result<Self, ConfigError> {
        match input.trim().to_ascii_lowercase().as_str() {
            "off" => Ok(Self::Off),
            "adaptive" => Ok(Self::Adaptive),
            "always" => Ok(Self::Always),
            "never" => Ok(Self::Never),
            _ => Err(ConfigError::Parse {
                path: None,
                line: None,
                message: format!("invalid prediction mode: {input}"),
            }),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Adaptive => "adaptive",
            Self::Always => "always",
            Self::Never => "never",
        }
    }
}

impl fmt::Display for PredictionMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressFamily {
    Auto,
    Ipv4,
    Ipv6,
}

impl AddressFamily {
    pub fn parse(input: &str) -> Result<Self, ConfigError> {
        match input.trim().to_ascii_lowercase().as_str() {
            "auto" | "any" => Ok(Self::Auto),
            "ipv4" | "inet" => Ok(Self::Ipv4),
            "ipv6" | "inet6" => Ok(Self::Ipv6),
            _ => Err(ConfigError::Parse {
                path: None,
                line: None,
                message: format!("invalid address family: {input}"),
            }),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Ipv4 => "ipv4",
            Self::Ipv6 => "ipv6",
        }
    }
}

impl fmt::Display for AddressFamily {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveSshConfig {
    pub hostname: String,
    pub user: String,
    pub port: u16,
    pub address_family: AddressFamily,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocketCandidate {
    pub host: String,
    pub port: PortSpec,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolveOverrides {
    pub ssh_path: Option<PathBuf>,
    pub udp_host: Option<String>,
    pub udp_port: Option<PortSpec>,
    pub mosh_server: Option<String>,
    pub terminal: Option<String>,
    pub prediction: Option<PredictionMode>,
    pub family: Option<AddressFamily>,
    pub connect_timeout: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTarget {
    pub display_name: String,
    pub alias_name: Option<String>,
    pub ssh_target: String,
    pub effective_ssh: EffectiveSshConfig,
    pub udp_candidates: Vec<SocketCandidate>,
    pub mosh_server: String,
    pub udp_port: PortSpec,
    pub terminal: String,
    pub prediction: PredictionMode,
}

#[derive(Debug)]
pub enum ConfigError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: Option<PathBuf>,
        line: Option<usize>,
        message: String,
    },
    UnsupportedVersion {
        version: u32,
    },
    InvalidAliasName {
        name: String,
    },
    AliasExists {
        name: String,
    },
    AliasNotFound {
        name: String,
    },
    ConcurrentModification {
        path: PathBuf,
    },
    InvalidPortSpec {
        value: String,
    },
    SshConfig(String),
}

impl ConfigError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub fn invalid_port_spec(value: impl Into<String>) -> Self {
        Self::InvalidPortSpec {
            value: value.into(),
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Parse {
                path,
                line,
                message,
            } => match (path, line) {
                (Some(path), Some(line)) => {
                    write!(formatter, "{}:{line}: {message}", path.display())
                }
                (Some(path), None) => write!(formatter, "{}: {message}", path.display()),
                (None, Some(line)) => write!(formatter, "line {line}: {message}"),
                (None, None) => formatter.write_str(message),
            },
            Self::UnsupportedVersion { version } => {
                write!(formatter, "unsupported config version: {version}")
            }
            Self::InvalidAliasName { name } => write!(
                formatter,
                "invalid alias name '{name}'; allowed characters are A-Z a-z 0-9 . _ -"
            ),
            Self::AliasExists { name } => write!(formatter, "alias already exists: {name}"),
            Self::AliasNotFound { name } => write!(formatter, "alias not found: {name}"),
            Self::ConcurrentModification { path } => write!(
                formatter,
                "configuration changed while editing: {}",
                path.display()
            ),
            Self::InvalidPortSpec { value } => write!(formatter, "invalid UDP port spec: {value}"),
            Self::SshConfig(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

fn parse_port(value: &str, original: &str) -> Result<u16, ConfigError> {
    let port = value
        .trim()
        .parse::<u16>()
        .map_err(|_| ConfigError::invalid_port_spec(original))?;
    if port == 0 {
        Err(ConfigError::invalid_port_spec(original))
    } else {
        Ok(port)
    }
}

#[cfg(test)]
mod tests {
    use super::{AddressFamily, PortSpec, PredictionMode};

    #[test]
    fn parses_port_ranges() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(PortSpec::parse("60000")?, PortSpec::Single(60000));
        assert_eq!(
            PortSpec::parse("60000:61000")?,
            PortSpec::Range {
                start: 60000,
                end: 61000,
            }
        );
        assert!(PortSpec::parse("61000:60000").is_err());
        Ok(())
    }

    #[test]
    fn parses_enum_values() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(PredictionMode::parse("adaptive")?, PredictionMode::Adaptive);
        assert_eq!(AddressFamily::parse("inet6")?, AddressFamily::Ipv6);
        Ok(())
    }
}
