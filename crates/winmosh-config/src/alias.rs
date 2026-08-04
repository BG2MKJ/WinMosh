use crate::model::{AppConfig, ConfigError, HostConfig, PortSpec, PredictionMode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasAdd {
    pub name: String,
    pub ssh_target: String,
    pub udp_host: Option<String>,
    pub udp_port: Option<PortSpec>,
    pub mosh_server: Option<String>,
    pub terminal: Option<String>,
    pub prediction: Option<PredictionMode>,
}

pub fn is_valid_alias_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

pub fn is_valid_ssh_target(target: &str) -> bool {
    let trimmed = target.trim();
    !trimmed.is_empty() && !trimmed.starts_with('-')
}

pub fn sanitize_server_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.len() >= 2 && trimmed.as_bytes()[1] == b':' {
        let colon = trimmed.find(':').unwrap_or(0);
        let relative = &trimmed[colon + 1..];
        if relative.starts_with('/') {
            return relative.to_owned();
        }
        let without_drive = relative.strip_prefix('/').unwrap_or(relative);
        return format!("/{}", without_drive.replace('\\', "/"));
    }
    trimmed.to_owned()
}

pub fn add_alias(config: &mut AppConfig, request: AliasAdd) -> Result<(), ConfigError> {
    validate_alias_name(&request.name)?;
    if config.hosts.contains_key(&request.name) {
        return Err(ConfigError::AliasExists { name: request.name });
    }
    if request.ssh_target.trim().is_empty() {
        return Err(ConfigError::Parse {
            path: None,
            line: None,
            message: "ssh target cannot be empty".to_owned(),
        });
    }
    if !is_valid_ssh_target(&request.ssh_target) {
        return Err(ConfigError::Parse {
            path: None,
            line: None,
            message: "ssh target must not start with '-'".to_owned(),
        });
    }

    config.hosts.insert(
        request.name,
        HostConfig {
            ssh_target: request.ssh_target,
            udp_host: request.udp_host,
            udp_port: request.udp_port,
            mosh_server: request.mosh_server.map(|s| sanitize_server_path(&s)),
            terminal: request.terminal,
            prediction: request.prediction,
        },
    );
    Ok(())
}

pub fn list_aliases(config: &AppConfig) -> Vec<(&str, &HostConfig)> {
    config
        .hosts
        .iter()
        .map(|(name, host)| (name.as_str(), host))
        .collect()
}

pub fn show_alias<'a>(config: &'a AppConfig, name: &str) -> Result<&'a HostConfig, ConfigError> {
    config
        .hosts
        .get(name)
        .ok_or_else(|| ConfigError::AliasNotFound {
            name: name.to_owned(),
        })
}

pub fn remove_alias(config: &mut AppConfig, name: &str) -> Result<HostConfig, ConfigError> {
    validate_alias_name(name)?;
    config
        .hosts
        .remove(name)
        .ok_or_else(|| ConfigError::AliasNotFound {
            name: name.to_owned(),
        })
}

pub fn rename_alias(config: &mut AppConfig, old: &str, new: &str) -> Result<(), ConfigError> {
    validate_alias_name(old)?;
    validate_alias_name(new)?;
    if config.hosts.contains_key(new) {
        return Err(ConfigError::AliasExists {
            name: new.to_owned(),
        });
    }
    let host = remove_alias(config, old)?;
    config.hosts.insert(new.to_owned(), host);
    Ok(())
}

fn validate_alias_name(name: &str) -> Result<(), ConfigError> {
    if is_valid_alias_name(name) {
        Ok(())
    } else {
        Err(ConfigError::InvalidAliasName {
            name: name.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::model::AppConfig;

    use super::{add_alias, is_valid_alias_name, is_valid_ssh_target, remove_alias, rename_alias, AliasAdd};

    #[test]
    fn validates_alias_names() {
        assert!(is_valid_alias_name("my.server-01_test"));
        assert!(!is_valid_alias_name(""));
        assert!(!is_valid_alias_name("bad/name"));
        assert!(!is_valid_alias_name("中文"));
    }

    #[test]
    fn supports_alias_crud() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = AppConfig::default();
        add_alias(
            &mut config,
            AliasAdd {
                name: "myserver".to_owned(),
                ssh_target: "root@example.com".to_owned(),
                udp_host: None,
                udp_port: None,
                mosh_server: None,
                terminal: None,
                prediction: None,
            },
        )?;
        assert!(config.hosts.contains_key("myserver"));
        rename_alias(&mut config, "myserver", "prod")?;
        assert!(config.hosts.contains_key("prod"));
        remove_alias(&mut config, "prod")?;
        assert!(config.hosts.is_empty());
        Ok(())
    }

    #[test]
    fn rejects_dash_prefixed_ssh_targets() {
        assert!(!is_valid_ssh_target("-oProxyCommand=evil"));
        assert!(!is_valid_ssh_target("  -v"));
        assert!(is_valid_ssh_target("user@host"));
        assert!(is_valid_ssh_target("192.168.1.1"));
    }
}
