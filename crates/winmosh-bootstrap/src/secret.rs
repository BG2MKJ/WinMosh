use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct SessionKey {
    bytes: Vec<u8>,
}

impl SessionKey {
    pub fn from_server_text(value: &str) -> Self {
        Self {
            bytes: value.as_bytes().to_vec(),
        }
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn as_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.bytes)
    }
}

impl Drop for SessionKey {
    fn drop(&mut self) {
        self.bytes.fill(0);
    }
}

impl fmt::Debug for SessionKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SessionKey")
            .field("len", &self.bytes.len())
            .field("redacted", &true)
            .finish()
    }
}
