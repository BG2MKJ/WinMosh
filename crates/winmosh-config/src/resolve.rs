use crate::model::{
    AddressFamily, AppConfig, EffectiveSshConfig, PredictionMode, ResolveOverrides, ResolvedTarget,
    SocketCandidate, DEFAULT_CONNECT_TIMEOUT, DEFAULT_MOSH_SERVER, DEFAULT_PREDICTION,
    DEFAULT_TERMINAL, DEFAULT_UDP_PORT,
};
use crate::ssh_config;
use crate::{ConfigError, HostConfig, PortSpec};

pub fn resolve_target(
    config: &AppConfig,
    input: &str,
    overrides: &ResolveOverrides,
) -> Result<ResolvedTarget, ConfigError> {
    let trimmed_input = input.trim();
    if trimmed_input.is_empty() {
        return Err(ConfigError::Parse {
            path: None,
            line: None,
            message: "target cannot be empty".to_owned(),
        });
    }

    let alias = config.hosts.get(trimmed_input);
    let ssh_target = alias
        .map(|host| host.ssh_target.as_str())
        .unwrap_or(trimmed_input)
        .to_owned();
    let timeout = overrides.connect_timeout.unwrap_or(DEFAULT_CONNECT_TIMEOUT);
    let effective_ssh =
        ssh_config::read_effective_ssh_config(overrides.ssh_path.as_deref(), &ssh_target, timeout)?;
    Ok(resolve_target_with_effective_ssh(
        config,
        trimmed_input,
        alias,
        ssh_target,
        effective_ssh,
        overrides,
    ))
}

pub fn resolve_target_with_effective_ssh(
    config: &AppConfig,
    display_name: &str,
    host: Option<&HostConfig>,
    ssh_target: String,
    mut effective_ssh: EffectiveSshConfig,
    overrides: &ResolveOverrides,
) -> ResolvedTarget {
    if let Some(family) = overrides.family {
        effective_ssh.address_family = family;
    }

    let udp_port = pick_port(
        overrides.udp_port,
        host.and_then(|host| host.udp_port),
        config.defaults.udp_port,
    );
    let udp_host = overrides
        .udp_host
        .as_deref()
        .or_else(|| host.and_then(|host| host.udp_host.as_deref()))
        .unwrap_or(&effective_ssh.hostname)
        .to_owned();
    let mosh_server = overrides
        .mosh_server
        .clone()
        .or_else(|| host.and_then(|host| host.mosh_server.clone()))
        .or_else(|| config.defaults.mosh_server.clone())
        .unwrap_or_else(|| DEFAULT_MOSH_SERVER.to_owned());
    let terminal = overrides
        .terminal
        .clone()
        .or_else(|| host.and_then(|host| host.terminal.clone()))
        .or_else(|| config.defaults.terminal.clone())
        .unwrap_or_else(|| DEFAULT_TERMINAL.to_owned());
    let prediction = overrides
        .prediction
        .or_else(|| host.and_then(|host| host.prediction))
        .or(config.defaults.prediction)
        .unwrap_or(DEFAULT_PREDICTION);

    ResolvedTarget {
        display_name: display_name.to_owned(),
        alias_name: host.map(|_| display_name.to_owned()),
        ssh_target,
        effective_ssh,
        udp_candidates: vec![SocketCandidate {
            host: udp_host,
            port: udp_port,
        }],
        mosh_server,
        udp_port,
        terminal,
        prediction,
    }
}

fn pick_port(
    cli: Option<PortSpec>,
    host: Option<PortSpec>,
    defaults: Option<PortSpec>,
) -> PortSpec {
    cli.or(host).or(defaults).unwrap_or(DEFAULT_UDP_PORT)
}

pub fn display_address_family(family: AddressFamily) -> &'static str {
    family.as_str()
}

pub fn display_prediction(prediction: PredictionMode) -> &'static str {
    prediction.as_str()
}

#[cfg(test)]
mod tests {
    use crate::alias::{add_alias, AliasAdd};
    use crate::model::{AddressFamily, AppConfig, EffectiveSshConfig, PortSpec, ResolveOverrides};

    use super::resolve_target_with_effective_ssh;

    #[test]
    fn applies_precedence_for_udp_and_terminal() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = AppConfig::default();
        add_alias(
            &mut config,
            AliasAdd {
                name: "myserver".to_owned(),
                ssh_target: "root@example.com".to_owned(),
                udp_host: Some("alias-udp.example.com".to_owned()),
                udp_port: Some(PortSpec::Single(60020)),
                mosh_server: Some("/usr/local/bin/mosh-server".to_owned()),
                terminal: Some("vt100".to_owned()),
                prediction: None,
            },
        )?;
        let host = config.hosts.get("myserver");
        let resolved = resolve_target_with_effective_ssh(
            &config,
            "myserver",
            host,
            "root@example.com".to_owned(),
            EffectiveSshConfig {
                hostname: "example.com".to_owned(),
                user: "root".to_owned(),
                port: 22,
                address_family: AddressFamily::Auto,
            },
            &ResolveOverrides {
                udp_host: Some("cli-udp.example.com".to_owned()),
                terminal: Some("xterm".to_owned()),
                ..ResolveOverrides::default()
            },
        );
        assert_eq!(resolved.udp_candidates[0].host, "cli-udp.example.com");
        assert_eq!(resolved.udp_port, PortSpec::Single(60020));
        assert_eq!(resolved.terminal, "xterm");
        assert_eq!(resolved.mosh_server, "/usr/local/bin/mosh-server");
        Ok(())
    }
}
