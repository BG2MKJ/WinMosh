use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::cli::UpdateMode;
use crate::error::{Error, Result};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const RELEASE_API_URL: &str = "https://api.github.com/repos/BG2MKJ/WinMosh/releases/latest";
const ASSET_CANDIDATES: [&str; 2] = ["winmosh-windows-x86_64.zip", "winmosh.exe"];

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReleaseInfo {
    version: Version,
    tag: String,
    asset_name: String,
    asset_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Version(Vec<u64>);

pub fn run(mode: UpdateMode) -> Result<()> {
    let current = Version::parse(CURRENT_VERSION)?;
    let release = latest_release()?;
    println!("current version: {CURRENT_VERSION}");
    println!("latest version: {}", release.tag);

    if release.version <= current {
        println!("winmosh is up to date");
        return Ok(());
    }

    println!("update available: {}", release.tag);
    match mode {
        UpdateMode::Check => {
            println!("run `winmosh update --download` to download the release artifact");
        }
        UpdateMode::Download => {
            let path = download_update(&release)?;
            println!("downloaded: {}", path.display());
            println!("replace winmosh.exe manually after verifying the archive contents");
        }
    }
    Ok(())
}

fn latest_release() -> Result<ReleaseInfo> {
    let output = match run_powershell(&[
        "-NoProfile",
        "-Command",
        &format!(
            "$ProgressPreference='SilentlyContinue'; \
             Invoke-RestMethod -Headers @{{'User-Agent'='WinMosh/{CURRENT_VERSION}'}} \
             -Uri '{}' | ConvertTo-Json -Depth 8",
            RELEASE_API_URL
        ),
    ]) {
        Ok(output) => output,
        Err(Error::Update(message)) if message.contains("Not Found") || message.contains("404") => {
            return Err(Error::Update(format!(
                "no published GitHub Release was found at {RELEASE_API_URL}"
            )));
        }
        Err(error) => return Err(error),
    };
    parse_release_json(&output)
}

fn download_update(release: &ReleaseInfo) -> Result<PathBuf> {
    let destination_dir = std::env::current_dir()?
        .join("target")
        .join("winmosh-update");
    fs::create_dir_all(&destination_dir)?;
    let final_path = destination_dir.join(&release.asset_name);
    let partial_path = final_path.with_extension("download");
    let command = format!(
        "$ProgressPreference='SilentlyContinue'; \
         Invoke-WebRequest -Headers @{{'User-Agent'='WinMosh/{CURRENT_VERSION}'}} \
         -Uri '{}' -OutFile '{}'",
        escape_powershell_single_quoted(&release.asset_url),
        escape_powershell_single_quoted(&partial_path.to_string_lossy())
    );
    run_powershell(&["-NoProfile", "-Command", &command])?;
    if fs::metadata(&partial_path)?.len() == 0 {
        return Err(Error::Update(
            "downloaded update artifact is empty".to_owned(),
        ));
    }
    fs::rename(&partial_path, &final_path)?;
    Ok(final_path)
}

fn run_powershell(args: &[&str]) -> Result<String> {
    let output = Command::new("powershell.exe").args(args).output()?;
    if !output.status.success() {
        let diagnostic = String::from_utf8_lossy(&output.stderr)
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("powershell command failed")
            .to_owned();
        return Err(Error::Update(diagnostic));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse_release_json(json: &str) -> Result<ReleaseInfo> {
    let tag = extract_string_field(json, "tag_name")
        .ok_or_else(|| Error::Update("release response did not contain tag_name".to_owned()))?;
    let version = Version::parse(tag.trim_start_matches('v'))?;
    let assets = extract_assets(json);
    let (asset_name, asset_url) = ASSET_CANDIDATES
        .iter()
        .find_map(|candidate| assets.iter().find(|(name, _)| name == candidate).cloned())
        .ok_or_else(|| {
            Error::Update(format!(
                "release does not contain any supported asset: {}",
                ASSET_CANDIDATES.join(", ")
            ))
        })?;
    Ok(ReleaseInfo {
        version,
        tag,
        asset_name,
        asset_url,
    })
}

fn extract_assets(json: &str) -> Vec<(String, String)> {
    let mut assets = Vec::new();
    let mut remaining = json;
    while let Some(position) = remaining.find("\"browser_download_url\"") {
        let before = &remaining[..position];
        let after = &remaining[position..];
        let Some(asset_url) = extract_string_field(after, "browser_download_url") else {
            break;
        };
        let Some(name_position) = before.rfind("\"name\"") else {
            remaining = &after[1..];
            continue;
        };
        let Some(asset_name) = extract_string_field(&before[name_position..], "name") else {
            remaining = &after[1..];
            continue;
        };
        assets.push((asset_name, asset_url));
        remaining = &after[1..];
    }
    assets
}

fn extract_string_field(json: &str, field: &str) -> Option<String> {
    let key = format!("\"{field}\"");
    let index = json.find(&key)?;
    let after_key = &json[index + key.len()..];
    let colon = after_key.find(':')?;
    let after_colon = after_key[colon + 1..].trim_start();
    let mut chars = after_colon.chars();
    if chars.next()? != '"' {
        return None;
    }
    let mut output = String::new();
    let mut escaped = false;
    for character in chars {
        if escaped {
            match character {
                '"' => output.push('"'),
                '\\' => output.push('\\'),
                '/' => output.push('/'),
                'b' => output.push('\u{0008}'),
                'f' => output.push('\u{000c}'),
                'n' => output.push('\n'),
                'r' => output.push('\r'),
                't' => output.push('\t'),
                other => output.push(other),
            }
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            return Some(output);
        } else {
            output.push(character);
        }
    }
    None
}

fn escape_powershell_single_quoted(value: &str) -> String {
    value.replace('\'', "''")
}

impl Version {
    fn parse(input: &str) -> Result<Self> {
        let trimmed = input.trim().trim_start_matches('v');
        let core = trimmed
            .split_once('-')
            .map(|(core, _)| core)
            .unwrap_or(trimmed);
        let mut parts = core
            .split('.')
            .map(|part| {
                part.parse::<u64>().map_err(|_| {
                    Error::Update(format!("invalid semantic version component: {part}"))
                })
            })
            .collect::<Result<Vec<_>>>()?;
        if parts.is_empty() {
            return Err(Error::Update("empty semantic version".to_owned()));
        }
        while parts.len() > 1 && parts.last() == Some(&0) {
            parts.pop();
        }
        Ok(Self(parts))
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let length = self.0.len().max(other.0.len());
        for index in 0..length {
            let left = self.0.get(index).copied().unwrap_or(0);
            let right = other.0.get(index).copied().unwrap_or(0);
            match left.cmp(&right) {
                std::cmp::Ordering::Equal => {}
                ordering => return ordering,
            }
        }
        std::cmp::Ordering::Equal
    }
}

#[cfg(test)]
mod tests {
    use super::{extract_assets, parse_release_json, Version};

    #[test]
    fn semantic_versions_are_compared_numerically() -> Result<(), Box<dyn std::error::Error>> {
        assert!(Version::parse("0.10.0")? > Version::parse("0.2.9")?);
        assert_eq!(Version::parse("v1.2")?, Version::parse("1.2.0")?);
        Ok(())
    }

    #[test]
    fn release_json_selects_windows_asset() -> Result<(), Box<dyn std::error::Error>> {
        let json = r#"{
            "tag_name": "v0.2.0",
            "assets": [
                {"name":"other.zip","browser_download_url":"https://example.test/other.zip"},
                {"name":"winmosh-windows-x86_64.zip","browser_download_url":"https://example.test/winmosh.zip"}
            ]
        }"#;
        let release = parse_release_json(json)?;
        assert_eq!(release.tag, "v0.2.0");
        assert_eq!(release.asset_url, "https://example.test/winmosh.zip");
        Ok(())
    }

    #[test]
    fn asset_parser_pairs_nearest_name_with_url() {
        let assets = extract_assets(
            r#"{"name":"first","browser_download_url":"https://example.test/first"}"#,
        );
        assert_eq!(
            assets,
            vec![("first".to_owned(), "https://example.test/first".to_owned())]
        );
    }
}
