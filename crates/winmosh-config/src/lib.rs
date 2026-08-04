#![forbid(unsafe_code)]

pub mod alias;
pub mod load;
pub mod model;
pub mod resolve;
pub mod ssh_config;

pub use alias::{add_alias, list_aliases, remove_alias, rename_alias, show_alias, AliasAdd};
pub use load::{default_config_path, load_config, save_config, ConfigDocument};
pub use model::{
    AddressFamily, AppConfig, ConfigError, Defaults, EffectiveSshConfig, HostConfig, PortSpec,
    PredictionMode, ResolveOverrides, ResolvedTarget, SocketCandidate, CONFIG_VERSION,
    DEFAULT_CONNECT_TIMEOUT, DEFAULT_MOSH_SERVER, DEFAULT_PREDICTION, DEFAULT_TERMINAL,
    DEFAULT_UDP_PORT, SSH_OUTPUT_LIMIT,
};
pub use resolve::resolve_target;
