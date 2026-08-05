use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use winmosh_platform::filesystem;

use crate::model::{
    AppConfig, ConfigError, Defaults, HostConfig, PortSpec, PredictionMode, CONFIG_VERSION,
};

#[derive(Debug, Clone)]
pub struct ConfigDocument {
    pub path: PathBuf,
    pub config: AppConfig,
    pub original_bytes: Option<Vec<u8>>,
}

impl ConfigDocument {
    pub fn new(path: PathBuf, config: AppConfig, original_bytes: Option<Vec<u8>>) -> Self {
        Self {
            path,
            config,
            original_bytes,
        }
    }
}

pub fn default_config_path() -> PathBuf {
    filesystem::roaming_app_data_dir("WinMosh")
        .unwrap_or_else(|| PathBuf::from("."))
        .join("config.toml")
}

pub const MAX_CONFIG_SIZE: u64 = 1024 * 1024;

pub fn load_config(path: &Path) -> Result<ConfigDocument, ConfigError> {
    match fs::read(path) {
        Ok(bytes) => {
            if bytes.len() as u64 > MAX_CONFIG_SIZE {
                return Err(ConfigError::Parse {
                    path: Some(path.to_path_buf()),
                    line: None,
                    message: format!(
                        "config file exceeds {:.1} MiB limit",
                        MAX_CONFIG_SIZE as f64 / (1024.0 * 1024.0)
                    ),
                });
            }
            let text = std::str::from_utf8(&bytes).map_err(|error| ConfigError::Parse {
                path: Some(path.to_path_buf()),
                line: None,
                message: format!("configuration is not valid UTF-8: {error}"),
            })?;
            let config = parse_config(text, Some(path))?;
            Ok(ConfigDocument::new(path.to_path_buf(), config, Some(bytes)))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(ConfigDocument::new(
            path.to_path_buf(),
            AppConfig::default(),
            None,
        )),
        Err(error) => Err(ConfigError::io(path, error)),
    }
}

pub fn save_config(document: &ConfigDocument) -> Result<(), ConfigError> {
    if let Some(original_bytes) = &document.original_bytes {
        match fs::read(&document.path) {
            Ok(current_bytes) if &current_bytes == original_bytes => {}
            Ok(_) => {
                return Err(ConfigError::ConcurrentModification {
                    path: document.path.clone(),
                })
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(ConfigError::ConcurrentModification {
                    path: document.path.clone(),
                })
            }
            Err(error) => return Err(ConfigError::io(document.path.as_path(), error)),
        }
    } else if document.path.exists() {
        return Err(ConfigError::ConcurrentModification {
            path: document.path.clone(),
        });
    }

    filesystem::ensure_parent(&document.path)
        .map_err(|error| ConfigError::io(document.path.as_path(), error))?;
    let parent = document
        .path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let temp_path = parent.join(format!(
        ".{}.{}.tmp",
        document
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("config.toml"),
        unique_suffix()
    ));

    let serialized = serialize_config(&document.config);
    let write_result = (|| -> Result<(), ConfigError> {
        let mut file = File::create(&temp_path)
            .map_err(|error| ConfigError::io(temp_path.as_path(), error))?;
        file.write_all(serialized.as_bytes())
            .map_err(|error| ConfigError::io(temp_path.as_path(), error))?;
        file.sync_all()
            .map_err(|error| ConfigError::io(temp_path.as_path(), error))?;
        drop(file);
        filesystem::replace_file_atomic(&temp_path, &document.path)
            .map_err(|error| ConfigError::io(document.path.as_path(), error))?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    write_result
}

pub fn parse_config(input: &str, path: Option<&Path>) -> Result<AppConfig, ConfigError> {
    let mut config = AppConfig {
        version: 0,
        defaults: Defaults::default(),
        hosts: BTreeMap::new(),
    };
    let mut section = Section::Root;

    for (index, raw_line) in input.lines().enumerate() {
        let line_number = index + 1;
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') {
            if !line.ends_with(']') {
                return parse_error(path, line_number, "section header is missing closing ']' ");
            }
            let section_name = &line[1..line.len() - 1];
            section = if section_name == "defaults" {
                Section::Defaults
            } else if let Some(name) = section_name.strip_prefix("hosts.") {
                let name = name.trim();
                if !crate::alias::is_valid_alias_name(name) {
                    return Err(ConfigError::InvalidAliasName {
                        name: name.to_owned(),
                    });
                }
                config.hosts.entry(name.to_owned()).or_default();
                Section::Host(name.to_owned())
            } else {
                return parse_error(
                    path,
                    line_number,
                    format!("unknown section: {section_name}"),
                );
            };
            continue;
        }

        let (key, value) = line.split_once('=').ok_or_else(|| ConfigError::Parse {
            path: path.map(Path::to_path_buf),
            line: Some(line_number),
            message: "expected key = value".to_owned(),
        })?;
        let key = key.trim();
        let value = parse_string_value(value.trim(), path, line_number)?;

        match &section {
            Section::Root => match key {
                "version" => {
                    config.version = value.parse::<u32>().map_err(|_| ConfigError::Parse {
                        path: path.map(Path::to_path_buf),
                        line: Some(line_number),
                        message: "version must be an integer".to_owned(),
                    })?;
                }
                _ => return parse_error(path, line_number, format!("unknown root key: {key}")),
            },
            Section::Defaults => {
                apply_default(&mut config.defaults, key, &value, path, line_number)?
            }
            Section::Host(name) => {
                let host = config
                    .hosts
                    .get_mut(name)
                    .ok_or_else(|| ConfigError::Parse {
                        path: path.map(Path::to_path_buf),
                        line: Some(line_number),
                        message: format!("internal parser error for host: {name}"),
                    })?;
                apply_host(host, key, &value, path, line_number)?;
            }
        }
    }

    if input.trim().is_empty() {
        return Ok(AppConfig::default());
    }

    if config.version == 0 {
        return parse_error(path, 1, "missing required root key: version");
    }
    if config.version != CONFIG_VERSION {
        return Err(ConfigError::UnsupportedVersion {
            version: config.version,
        });
    }

    for (name, host) in &config.hosts {
        if host.ssh_target.trim().is_empty() {
            return Err(ConfigError::Parse {
                path: path.map(Path::to_path_buf),
                line: None,
                message: format!("hosts.{name}.ssh_target is required"),
            });
        }
    }

    Ok(config)
}

pub fn serialize_config(config: &AppConfig) -> String {
    let mut output = String::new();
    output.push_str(&format!("version = {}\n", config.version));

    output.push_str("\n[defaults]\n");
    if let Some(value) = &config.defaults.mosh_server {
        output.push_str(&format!("mosh_server = \"{}\"\n", escape_toml(value)));
    }
    if let Some(value) = config.defaults.udp_port {
        output.push_str(&format!("udp_port = \"{}\"\n", value));
    }
    if let Some(value) = &config.defaults.terminal {
        output.push_str(&format!("terminal = \"{}\"\n", escape_toml(value)));
    }
    if let Some(value) = config.defaults.prediction {
        output.push_str(&format!("prediction = \"{}\"\n", value));
    }

    for (name, host) in &config.hosts {
        output.push_str(&format!("\n[hosts.{name}]\n"));
        output.push_str(&format!(
            "ssh_target = \"{}\"\n",
            escape_toml(&host.ssh_target)
        ));
        if let Some(value) = &host.udp_host {
            output.push_str(&format!("udp_host = \"{}\"\n", escape_toml(value)));
        }
        if let Some(value) = host.udp_port {
            output.push_str(&format!("udp_port = \"{}\"\n", value));
        }
        if let Some(value) = &host.mosh_server {
            output.push_str(&format!("mosh_server = \"{}\"\n", escape_toml(value)));
        }
        if let Some(value) = &host.terminal {
            output.push_str(&format!("terminal = \"{}\"\n", escape_toml(value)));
        }
        if let Some(value) = host.prediction {
            output.push_str(&format!("prediction = \"{}\"\n", value));
        }
    }

    output
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Section {
    Root,
    Defaults,
    Host(String),
}

fn apply_default(
    defaults: &mut Defaults,
    key: &str,
    value: &str,
    path: Option<&Path>,
    line: usize,
) -> Result<(), ConfigError> {
    match key {
        "mosh_server" => defaults.mosh_server = Some(crate::alias::sanitize_server_path(value)),
        "udp_port" => defaults.udp_port = Some(PortSpec::parse(value)?),
        "terminal" => defaults.terminal = Some(value.to_owned()),
        "prediction" => defaults.prediction = Some(PredictionMode::parse(value)?),
        _ => return parse_error(path, line, format!("unknown defaults key: {key}")),
    }
    Ok(())
}

fn apply_host(
    host: &mut HostConfig,
    key: &str,
    value: &str,
    path: Option<&Path>,
    line: usize,
) -> Result<(), ConfigError> {
    match key {
        "ssh_target" => {
            if !crate::alias::is_valid_ssh_target(value) {
                return parse_error(path, line, "ssh_target must not be empty or start with '-'");
            }
            host.ssh_target = value.to_owned()
        }
        "udp_host" => host.udp_host = Some(value.to_owned()),
        "udp_port" => host.udp_port = Some(PortSpec::parse(value)?),
        "mosh_server" => host.mosh_server = Some(crate::alias::sanitize_server_path(value)),
        "terminal" => host.terminal = Some(value.to_owned()),
        "prediction" => host.prediction = Some(PredictionMode::parse(value)?),
        _ => return parse_error(path, line, format!("unknown host key: {key}")),
    }
    Ok(())
}

fn parse_string_value(
    value: &str,
    path: Option<&Path>,
    line: usize,
) -> Result<String, ConfigError> {
    if value.starts_with('"') {
        if !value.ends_with('"') || value.len() < 2 {
            return parse_error(path, line, "unterminated string value");
        }
        unescape_toml(&value[1..value.len() - 1], path, line)
    } else {
        Ok(value.to_owned())
    }
}

fn strip_comment(line: &str) -> &str {
    let mut escaped = false;
    let mut quoted = false;
    for (index, character) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            '#' if !quoted => return &line[..index],
            _ => {}
        }
    }
    line
}

fn unescape_toml(input: &str, path: Option<&Path>, line: usize) -> Result<String, ConfigError> {
    let mut output = String::with_capacity(input.len());
    let mut characters = input.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            output.push(character);
            continue;
        }
        let escaped = characters.next().ok_or_else(|| ConfigError::Parse {
            path: path.map(Path::to_path_buf),
            line: Some(line),
            message: "unterminated escape sequence".to_owned(),
        })?;
        match escaped {
            '"' => output.push('"'),
            '\\' => output.push('\\'),
            'n' => output.push('\n'),
            'r' => output.push('\r'),
            't' => output.push('\t'),
            other => {
                return parse_error(
                    path,
                    line,
                    format!("unsupported escape sequence: \\{other}"),
                )
            }
        }
    }
    Ok(output)
}

fn escape_toml(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for character in input.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            other => output.push(other),
        }
    }
    output
}

fn parse_error<T>(
    path: Option<&Path>,
    line: usize,
    message: impl Into<String>,
) -> Result<T, ConfigError> {
    Err(ConfigError::Parse {
        path: path.map(Path::to_path_buf),
        line: Some(line),
        message: message.into(),
    })
}

fn unique_suffix() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{}.{}", std::process::id(), now)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::alias::{add_alias, AliasAdd};
    use crate::model::PortSpec;

    use super::{load_config, parse_config, save_config, serialize_config, ConfigDocument};

    #[test]
    fn parses_readme_style_config() -> Result<(), Box<dyn std::error::Error>> {
        let config = parse_config(
            r#"
version = 1

[defaults]
mosh_server = "mosh-server"
udp_port = "60000:61000"
terminal = "xterm-256color"
prediction = "off"

[hosts.myserver]
ssh_target = "root@192.168.1.20"
"#,
            None,
        )?;

        assert_eq!(config.version, 1);
        assert_eq!(config.hosts["myserver"].ssh_target, "root@192.168.1.20");
        assert_eq!(
            config.defaults.udp_port,
            Some(PortSpec::Range {
                start: 60000,
                end: 61000,
            })
        );
        Ok(())
    }

    #[test]
    fn serializes_hosts_in_stable_order() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = parse_config("version = 1\n", None)?;
        add_alias(
            &mut config,
            AliasAdd {
                name: "b".to_owned(),
                ssh_target: "root@b".to_owned(),
                udp_host: None,
                udp_port: None,
                mosh_server: None,
                terminal: None,
                prediction: None,
            },
        )?;
        add_alias(
            &mut config,
            AliasAdd {
                name: "a".to_owned(),
                ssh_target: "root@a".to_owned(),
                udp_host: None,
                udp_port: None,
                mosh_server: None,
                terminal: None,
                prediction: None,
            },
        )?;

        let serialized = serialize_config(&config);
        let host_a = serialized.find("[hosts.a]");
        let host_b = serialized.find("[hosts.b]");
        match (host_a, host_b) {
            (Some(host_a), Some(host_b)) => assert!(host_a < host_b),
            _ => return Err("serialized aliases were missing".into()),
        }
        Ok(())
    }

    #[test]
    fn detects_concurrent_writes() -> Result<(), Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!(
            "winmosh-config-test-{}-{}.toml",
            std::process::id(),
            "concurrent"
        ));
        let _ = fs::remove_file(&path);
        fs::write(&path, "version = 1\n")?;
        let mut document = load_config(&path)?;
        document.config.hosts.insert(
            "myserver".to_owned(),
            crate::model::HostConfig {
                ssh_target: "root@example.com".to_owned(),
                ..crate::model::HostConfig::default()
            },
        );
        fs::write(&path, "version = 1\n# changed\n")?;
        assert!(save_config(&document).is_err());
        let _ = fs::remove_file(&path);
        Ok(())
    }

    #[test]
    fn writes_new_config_atomically() -> Result<(), Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!(
            "winmosh-config-test-{}-{}.toml",
            std::process::id(),
            "atomic"
        ));
        let _ = fs::remove_file(&path);
        let mut document =
            ConfigDocument::new(path.clone(), crate::model::AppConfig::default(), None);
        document.config.hosts.insert(
            "myserver".to_owned(),
            crate::model::HostConfig {
                ssh_target: "root@example.com".to_owned(),
                ..crate::model::HostConfig::default()
            },
        );
        save_config(&document)?;
        let loaded = load_config(&path)?;
        assert_eq!(
            loaded.config.hosts["myserver"].ssh_target,
            "root@example.com"
        );
        let _ = fs::remove_file(&path);
        Ok(())
    }
}
