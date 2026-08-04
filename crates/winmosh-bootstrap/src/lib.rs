#![forbid(unsafe_code)]

pub mod command;
pub mod error;
pub mod parser;
pub mod secret;
pub mod ssh;

pub use command::{build_remote_command, build_server_command, BootstrapRequest};
pub use error::BootstrapError;
pub use parser::{parse_mosh_server_output, parse_mosh_server_output_bytes, BootstrapResult};
pub use secret::SessionKey;
pub use ssh::start_bootstrap;
