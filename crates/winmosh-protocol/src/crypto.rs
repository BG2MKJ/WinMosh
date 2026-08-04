use std::fmt;

use aes::Aes128;
use base64::engine::general_purpose::STANDARD_NO_PAD;
use base64::Engine;
use ocb3::aead::{generic_array::GenericArray, AeadInPlace, KeyInit};
use ocb3::{consts::U12, Ocb3};
use zeroize::{Zeroize, Zeroizing};

pub const KEY_LENGTH: usize = 16;
pub const PRINTABLE_KEY_LENGTH: usize = 22;
pub const NONCE_LENGTH: usize = 12;
pub const WIRE_NONCE_LENGTH: usize = 8;
pub const TAG_LENGTH: usize = 16;
pub const MAX_DATAGRAM_LENGTH: usize = 2048;
pub const MAX_PLAINTEXT_LENGTH: usize = MAX_DATAGRAM_LENGTH - WIRE_NONCE_LENGTH - TAG_LENGTH;

type Aes128Ocb = Ocb3<Aes128, U12>;

#[derive(Debug)]
pub enum CryptoError {
    InvalidKey(String),
    InvalidPacket(String),
    Encrypt,
    Decrypt,
    KeyUsageLimit,
}

impl fmt::Display for CryptoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKey(message) => write!(formatter, "invalid Mosh session key: {message}"),
            Self::InvalidPacket(message) => {
                write!(formatter, "invalid encrypted packet: {message}")
            }
            Self::Encrypt => formatter.write_str("AES-OCB encryption failed"),
            Self::Decrypt => formatter.write_str("AES-OCB authentication failed"),
            Self::KeyUsageLimit => {
                formatter.write_str("Mosh session encryption block limit exceeded")
            }
        }
    }
}

impl std::error::Error for CryptoError {}

pub struct SessionKey {
    bytes: Zeroizing<[u8; KEY_LENGTH]>,
}

impl SessionKey {
    pub fn from_printable(value: &str) -> Result<Self, CryptoError> {
        if value.len() != PRINTABLE_KEY_LENGTH
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"/+".contains(&byte))
        {
            return Err(CryptoError::InvalidKey(
                "expected 22 base64 characters".to_owned(),
            ));
        }

        let decoded = STANDARD_NO_PAD
            .decode(value.as_bytes())
            .map_err(|_| CryptoError::InvalidKey("invalid base64 encoding".to_owned()))?;
        if decoded.len() != KEY_LENGTH {
            return Err(CryptoError::InvalidKey(
                "base64 value did not decode to 16 bytes".to_owned(),
            ));
        }

        let mut bytes = [0_u8; KEY_LENGTH];
        bytes.copy_from_slice(&decoded);
        Ok(Self {
            bytes: Zeroizing::new(bytes),
        })
    }

    pub fn from_bytes(bytes: [u8; KEY_LENGTH]) -> Self {
        Self {
            bytes: Zeroizing::new(bytes),
        }
    }

    fn as_bytes(&self) -> &[u8; KEY_LENGTH] {
        &self.bytes
    }
}

impl Clone for SessionKey {
    fn clone(&self) -> Self {
        Self::from_bytes(*self.as_bytes())
    }
}

impl fmt::Debug for SessionKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SessionKey")
            .field("len", &KEY_LENGTH)
            .field("redacted", &true)
            .finish()
    }
}

pub struct CryptoSession {
    key: SessionKey,
    blocks_encrypted: u64,
}

impl CryptoSession {
    pub fn new(key: SessionKey) -> Self {
        Self {
            key,
            blocks_encrypted: 0,
        }
    }

    pub fn encrypt(&mut self, nonce_value: u64, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        if plaintext.len() > MAX_PLAINTEXT_LENGTH {
            return Err(CryptoError::InvalidPacket(
                "plaintext exceeds the Mosh datagram limit".to_owned(),
            ));
        }

        let blocks = plaintext.len().div_ceil(16) as u64;
        if self.blocks_encrypted > (1_u64 << 47).saturating_sub(blocks) {
            return Err(CryptoError::KeyUsageLimit);
        }
        self.blocks_encrypted += blocks;

        let nonce = nonce_bytes(nonce_value);
        let cipher = cipher_for(&self.key);
        let mut ciphertext = plaintext.to_vec();
        let tag = cipher
            .encrypt_in_place_detached(&nonce, &[], &mut ciphertext)
            .map_err(|_| CryptoError::Encrypt)?;

        let mut packet = Vec::with_capacity(WIRE_NONCE_LENGTH + ciphertext.len() + TAG_LENGTH);
        packet.extend_from_slice(&nonce_value.to_be_bytes());
        packet.append(&mut ciphertext);
        packet.extend_from_slice(tag.as_slice());
        Ok(packet)
    }

    pub fn decrypt(&self, packet: &[u8]) -> Result<(u64, Vec<u8>), CryptoError> {
        if packet.len() < WIRE_NONCE_LENGTH + TAG_LENGTH {
            return Err(CryptoError::InvalidPacket(
                "packet is shorter than nonce and authentication tag".to_owned(),
            ));
        }

        let mut nonce_value_bytes = [0_u8; WIRE_NONCE_LENGTH];
        nonce_value_bytes.copy_from_slice(&packet[..WIRE_NONCE_LENGTH]);
        let nonce_value = u64::from_be_bytes(nonce_value_bytes);
        let body = &packet[WIRE_NONCE_LENGTH..];
        let ciphertext_length = body.len() - TAG_LENGTH;
        let (ciphertext, tag) = body.split_at(ciphertext_length);
        let mut plaintext = ciphertext.to_vec();
        let nonce = nonce_bytes(nonce_value);
        let cipher = cipher_for(&self.key);
        let tag = GenericArray::from_slice(tag);
        cipher
            .decrypt_in_place_detached(&nonce, &[], &mut plaintext, tag)
            .map_err(|_| CryptoError::Decrypt)?;
        Ok((nonce_value, plaintext))
    }
}

fn cipher_for(key: &SessionKey) -> Aes128Ocb {
    let key = GenericArray::from_slice(key.as_bytes());
    Aes128Ocb::new(key)
}

fn nonce_bytes(value: u64) -> GenericArray<u8, U12> {
    let mut nonce = [0_u8; NONCE_LENGTH];
    nonce[4..].copy_from_slice(&value.to_be_bytes());
    GenericArray::clone_from_slice(&nonce)
}

impl Drop for CryptoSession {
    fn drop(&mut self) {
        self.blocks_encrypted.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::{CryptoError, CryptoSession, SessionKey};

    const KEY: &str = "AAECAwQFBgcICQoLDA0ODw";

    #[test]
    fn printable_key_decodes_to_aes128_key() -> Result<(), Box<dyn std::error::Error>> {
        let key = SessionKey::from_printable(KEY)?;
        assert_eq!(format!("{key:?}"), "SessionKey { len: 16, redacted: true }");
        Ok(())
    }

    #[test]
    fn encrypt_and_decrypt_round_trip() -> Result<(), Box<dyn std::error::Error>> {
        let key = SessionKey::from_printable(KEY)?;
        let mut sender = CryptoSession::new(key.clone());
        let receiver = CryptoSession::new(key);
        let packet = sender.encrypt(0x0102_0304_0506_0708, b"mosh payload")?;
        let (nonce, plaintext) = receiver.decrypt(&packet)?;
        assert_eq!(nonce, 0x0102_0304_0506_0708);
        assert_eq!(plaintext, b"mosh payload");
        Ok(())
    }

    #[test]
    fn tampering_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let key = SessionKey::from_printable(KEY)?;
        let mut sender = CryptoSession::new(key.clone());
        let receiver = CryptoSession::new(key);
        let mut packet = sender.encrypt(7, b"payload")?;
        let last_byte = packet.len() - 1;
        packet[last_byte] ^= 1;
        assert!(matches!(
            receiver.decrypt(&packet),
            Err(CryptoError::Decrypt)
        ));
        Ok(())
    }

    #[test]
    fn matches_ocb_golden_vector() -> Result<(), Box<dyn std::error::Error>> {
        let key = SessionKey::from_bytes([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
        let mut session = CryptoSession::new(key);
        let packet = session.encrypt(0x0102_0304_0506_0708, b"\x00\x64\x00\x63mosh payload")?;
        let expected = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x5f, 0xae, 0x9a, 0x89, 0xcc, 0x84,
            0xc4, 0x27, 0x10, 0x51, 0x35, 0xd5, 0xeb, 0x3c, 0xb0, 0x40, 0x0a, 0x81, 0xb8, 0x69,
            0x10, 0x63, 0x65, 0x8f, 0x8b, 0xdf, 0xf3, 0xdb, 0x54, 0xda, 0x4e, 0xef,
        ];
        assert_eq!(packet, expected);
        Ok(())
    }

    #[test]
    fn key_format_is_strict() {
        assert!(SessionKey::from_printable("short").is_err());
        assert!(SessionKey::from_printable("abcdefghijklmnopqrstu!").is_err());
    }
}
