use crate::error::BootstrapError;
use crate::secret::SessionKey;

#[derive(Debug)]
pub struct BootstrapResult {
    pub udp_port: u16,
    pub session_key: SessionKey,
    pub remote_pid: Option<u32>,
    pub server_version: Option<String>,
}

pub fn parse_mosh_server_output(output: &str) -> Result<BootstrapResult, BootstrapError> {
    parse_mosh_server_output_bytes(output.as_bytes())
}

pub fn parse_mosh_server_output_bytes(output: &[u8]) -> Result<BootstrapResult, BootstrapError> {
    let output = String::from_utf8_lossy(output);
    let mut result: Option<BootstrapResult> = None;
    let mut server_version = None;

    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(version) = parse_server_version(trimmed) {
            server_version = Some(version);
        }
        if let Some(rest) = trimmed.strip_prefix("MOSH CONNECT ") {
            let mut parts = rest.split_whitespace();
            let port = parts
                .next()
                .ok_or_else(|| BootstrapError::InvalidServerOutput("missing UDP port".to_owned()))?
                .parse::<u16>()
                .map_err(|_| BootstrapError::InvalidServerOutput("invalid UDP port".to_owned()))?;
            let key = parts.next().ok_or_else(|| {
                BootstrapError::InvalidServerOutput("missing session key".to_owned())
            })?;
            if parts.next().is_some() {
                return Err(BootstrapError::InvalidServerOutput(
                    "MOSH CONNECT line has extra fields".to_owned(),
                ));
            }
            if !is_valid_session_key(key) {
                return Err(BootstrapError::InvalidServerOutput(
                    "session key has invalid format".to_owned(),
                ));
            }
            if result.is_some() {
                return Err(BootstrapError::InvalidServerOutput(
                    "multiple MOSH CONNECT lines were found".to_owned(),
                ));
            }
            result = Some(BootstrapResult {
                udp_port: port,
                session_key: SessionKey::from_server_text(key),
                remote_pid: None,
                server_version: server_version.clone(),
            });
        }
    }

    result
        .map(|mut result| {
            result.server_version = server_version;
            result
        })
        .ok_or_else(|| {
            BootstrapError::InvalidServerOutput("MOSH CONNECT line was not found".to_owned())
        })
}

fn is_valid_session_key(value: &str) -> bool {
    if value.len() != 22 {
        return false;
    }
    let mut accumulator = 0_u32;
    let mut bits = 0_u8;
    let mut decoded_length = 0_usize;
    for byte in value.bytes() {
        let Some(value) = base64_value(byte) else {
            return false;
        };
        accumulator = (accumulator << 6) | u32::from(value);
        bits += 6;
        while bits >= 8 {
            bits -= 8;
            accumulator &= if bits == 0 { 0 } else { (1_u32 << bits) - 1 };
            decoded_length += 1;
        }
    }
    decoded_length == 16 && bits == 4 && accumulator == 0
}

fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn parse_server_version(line: &str) -> Option<String> {
    let marker = "mosh-server (mosh) version ";
    line.strip_prefix(marker)
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::parse_mosh_server_output;

    #[test]
    fn parses_connect_line_without_exposing_key() -> Result<(), Box<dyn std::error::Error>> {
        let result =
            parse_mosh_server_output("noise\nMOSH CONNECT 60024 AAECAwQFBgcICQoLDA0ODw\n")?;
        assert_eq!(result.udp_port, 60024);
        assert_eq!(result.session_key.len(), 22);
        let debug = format!("{:?}", result.session_key);
        assert!(!debug.contains("AAECAwQFBgcICQoLDA0ODw"));
        Ok(())
    }

    #[test]
    fn accepts_banner_and_extracts_version() -> Result<(), Box<dyn std::error::Error>> {
        let result = parse_mosh_server_output(
            "welcome\r\nmosh-server (mosh) version 1.4.0\r\nMOSH CONNECT 60024 AAECAwQFBgcICQoLDA0ODw\r\n",
        )?;
        assert_eq!(result.server_version.as_deref(), Some("1.4.0"));
        Ok(())
    }

    #[test]
    fn rejects_invalid_key() {
        let error =
            parse_mosh_server_output("MOSH CONNECT 60024 invalid").expect_err("invalid key");
        assert!(error.to_string().contains("invalid format"));
    }

    #[test]
    fn rejects_duplicate_connect_lines() {
        let output =
            "MOSH CONNECT 60024 AAECAwQFBgcICQoLDA0ODw\nMOSH CONNECT 60025 AAECAwQFBgcICQoLDA0ODw";
        let error = parse_mosh_server_output(output).expect_err("duplicate connect lines");
        assert!(error.to_string().contains("multiple"));
    }

    #[test]
    fn accepts_non_utf8_noise_before_connect_line() -> Result<(), Box<dyn std::error::Error>> {
        let mut output = vec![0xff, 0xfe, b'\n'];
        output.extend_from_slice(b"MOSH CONNECT 60024 AAECAwQFBgcICQoLDA0ODw\r\n");
        let result = super::parse_mosh_server_output_bytes(&output)?;
        assert_eq!(result.udp_port, 60024);
        Ok(())
    }
}
