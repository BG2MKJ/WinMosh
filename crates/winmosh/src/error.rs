use std::fmt;

#[derive(Debug)]
pub enum Error {
    Cli(String),
    Config(winmosh_config::ConfigError),
    Bootstrap(winmosh_bootstrap::BootstrapError),
    Protocol(String),
    Update(String),
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cli(message) => formatter.write_str(message),
            Self::Config(error) => error.fmt(formatter),
            Self::Bootstrap(error) => error.fmt(formatter),
            Self::Protocol(message) => formatter.write_str(message),
            Self::Update(message) => formatter.write_str(message),
            Self::Io(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config(error) => Some(error),
            Self::Bootstrap(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::Cli(_) | Self::Protocol(_) | Self::Update(_) => None,
        }
    }
}

impl From<winmosh_config::ConfigError> for Error {
    fn from(error: winmosh_config::ConfigError) -> Self {
        Self::Config(error)
    }
}

impl From<winmosh_bootstrap::BootstrapError> for Error {
    fn from(error: winmosh_bootstrap::BootstrapError) -> Self {
        Self::Bootstrap(error)
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
