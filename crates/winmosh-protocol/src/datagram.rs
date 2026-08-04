use std::fmt;

use crate::crypto::{CryptoError, CryptoSession, MAX_PLAINTEXT_LENGTH};
use crate::sequence::{Direction, SequenceError, SequenceNumber};

pub const TIMESTAMP_HEADER_LENGTH: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Datagram {
    pub direction: Direction,
    pub sequence: SequenceNumber,
    pub timestamp: u16,
    pub timestamp_reply: u16,
    pub payload: Vec<u8>,
}

#[derive(Debug)]
pub enum DatagramError {
    Crypto(CryptoError),
    InvalidPlaintext(String),
    DirectionMismatch {
        expected: Direction,
        actual: Direction,
    },
    Sequence(SequenceError),
}

impl fmt::Display for DatagramError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Crypto(error) => error.fmt(formatter),
            Self::InvalidPlaintext(message) => {
                write!(formatter, "invalid Mosh datagram: {message}")
            }
            Self::DirectionMismatch { expected, actual } => {
                write!(
                    formatter,
                    "unexpected datagram direction: expected {expected:?}, got {actual:?}"
                )
            }
            Self::Sequence(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for DatagramError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Crypto(error) => Some(error),
            Self::Sequence(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CryptoError> for DatagramError {
    fn from(error: CryptoError) -> Self {
        Self::Crypto(error)
    }
}

impl From<SequenceError> for DatagramError {
    fn from(error: SequenceError) -> Self {
        Self::Sequence(error)
    }
}

impl Datagram {
    pub fn new(
        direction: Direction,
        sequence: SequenceNumber,
        timestamp: u16,
        timestamp_reply: u16,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            direction,
            sequence,
            timestamp,
            timestamp_reply,
            payload,
        }
    }

    fn encode_plaintext(&self) -> Result<Vec<u8>, DatagramError> {
        let plaintext_length = TIMESTAMP_HEADER_LENGTH + self.payload.len();
        if plaintext_length > MAX_PLAINTEXT_LENGTH {
            return Err(DatagramError::InvalidPlaintext(
                "payload exceeds the Mosh datagram limit".to_owned(),
            ));
        }

        let mut plaintext = Vec::with_capacity(plaintext_length);
        plaintext.extend_from_slice(&self.timestamp.to_be_bytes());
        plaintext.extend_from_slice(&self.timestamp_reply.to_be_bytes());
        plaintext.extend_from_slice(&self.payload);
        Ok(plaintext)
    }

    fn decode_plaintext(
        direction: Direction,
        sequence: SequenceNumber,
        plaintext: &[u8],
    ) -> Result<Self, DatagramError> {
        if plaintext.len() < TIMESTAMP_HEADER_LENGTH {
            return Err(DatagramError::InvalidPlaintext(
                "timestamp header is truncated".to_owned(),
            ));
        }
        let timestamp = u16::from_be_bytes([plaintext[0], plaintext[1]]);
        let timestamp_reply = u16::from_be_bytes([plaintext[2], plaintext[3]]);
        Ok(Self {
            direction,
            sequence,
            timestamp,
            timestamp_reply,
            payload: plaintext[TIMESTAMP_HEADER_LENGTH..].to_vec(),
        })
    }
}

pub fn encode_datagram(
    crypto: &mut CryptoSession,
    datagram: &Datagram,
) -> Result<Vec<u8>, DatagramError> {
    let plaintext = datagram.encode_plaintext()?;
    crypto
        .encrypt(datagram.sequence.to_wire(datagram.direction), &plaintext)
        .map_err(Into::into)
}

pub fn decode_datagram(
    crypto: &CryptoSession,
    packet: &[u8],
    expected_direction: Direction,
) -> Result<Datagram, DatagramError> {
    let (wire_sequence, plaintext) = crypto.decrypt(packet)?;
    let (direction, sequence) = SequenceNumber::from_wire(wire_sequence);
    if direction != expected_direction {
        return Err(DatagramError::DirectionMismatch {
            expected: expected_direction,
            actual: direction,
        });
    }
    Datagram::decode_plaintext(direction, sequence, &plaintext)
}

pub struct DatagramCodec {
    crypto: CryptoSession,
    send_direction: Direction,
    receive_direction: Direction,
    next_send: SequenceNumber,
    expected_receive: SequenceNumber,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiveDisposition {
    Fresh,
    Replayed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceivedDatagram {
    pub datagram: Datagram,
    pub disposition: ReceiveDisposition,
}

impl DatagramCodec {
    pub fn new(crypto: CryptoSession, send_direction: Direction) -> Self {
        let receive_direction = match send_direction {
            Direction::ToServer => Direction::ToClient,
            Direction::ToClient => Direction::ToServer,
        };
        Self {
            crypto,
            send_direction,
            receive_direction,
            next_send: SequenceNumber::ZERO,
            expected_receive: SequenceNumber::ZERO,
        }
    }

    pub fn encode(
        &mut self,
        timestamp: u16,
        timestamp_reply: u16,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, DatagramError> {
        let datagram = Datagram::new(
            self.send_direction,
            self.next_send,
            timestamp,
            timestamp_reply,
            payload,
        );
        self.next_send = self.next_send.next()?;
        encode_datagram(&mut self.crypto, &datagram)
    }

    pub fn decode(&mut self, packet: &[u8]) -> Result<ReceivedDatagram, DatagramError> {
        let datagram = decode_datagram(&self.crypto, packet, self.receive_direction)?;
        let disposition = if datagram.sequence < self.expected_receive {
            ReceiveDisposition::Replayed
        } else {
            self.expected_receive = datagram.sequence.next()?;
            ReceiveDisposition::Fresh
        };
        Ok(ReceivedDatagram {
            datagram,
            disposition,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_datagram, encode_datagram, Datagram, DatagramCodec, ReceiveDisposition};
    use crate::crypto::{CryptoSession, SessionKey};
    use crate::sequence::{Direction, SequenceNumber};

    fn crypto_pair() -> (CryptoSession, CryptoSession) {
        let key = SessionKey::from_bytes([7_u8; 16]);
        (CryptoSession::new(key.clone()), CryptoSession::new(key))
    }

    #[test]
    fn encodes_mosh_timestamp_header() -> Result<(), Box<dyn std::error::Error>> {
        let (mut sender, receiver) = crypto_pair();
        let datagram = Datagram::new(
            Direction::ToServer,
            SequenceNumber::new(3)?,
            100,
            99,
            b"payload".to_vec(),
        );
        let packet = encode_datagram(&mut sender, &datagram)?;
        let decoded = decode_datagram(&receiver, &packet, Direction::ToServer)?;
        assert_eq!(decoded, datagram);
        Ok(())
    }

    #[test]
    fn codec_tracks_replayed_sequence_numbers() -> Result<(), Box<dyn std::error::Error>> {
        let (sender_crypto, receiver_crypto) = crypto_pair();
        let mut sender = DatagramCodec::new(sender_crypto, Direction::ToServer);
        let mut receiver = DatagramCodec::new(receiver_crypto, Direction::ToClient);
        let first = sender.encode(1, 2, b"first".to_vec())?;
        let second = sender.encode(3, 4, b"second".to_vec())?;
        assert_eq!(
            receiver.decode(&first)?.disposition,
            ReceiveDisposition::Fresh
        );
        assert_eq!(
            receiver.decode(&second)?.disposition,
            ReceiveDisposition::Fresh
        );
        assert_eq!(
            receiver.decode(&first)?.disposition,
            ReceiveDisposition::Replayed
        );
        Ok(())
    }
}
