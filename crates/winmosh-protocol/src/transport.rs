use std::fmt;
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

use crate::crypto::{CryptoSession, MAX_DATAGRAM_LENGTH};
use crate::datagram::{DatagramCodec, DatagramError, ReceivedDatagram};

#[derive(Debug)]
pub enum TransportError {
    Io(io::Error),
    Datagram(DatagramError),
    OversizeDatagram,
    MissingRemote,
}

impl fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Datagram(error) => error.fmt(formatter),
            Self::OversizeDatagram => formatter.write_str("received an oversized UDP datagram"),
            Self::MissingRemote => formatter.write_str("UDP remote address is not configured"),
        }
    }
}

impl std::error::Error for TransportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Datagram(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for TransportError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<DatagramError> for TransportError {
    fn from(error: DatagramError) -> Self {
        Self::Datagram(error)
    }
}

pub struct UdpTransport {
    socket: UdpSocket,
    remote: Option<SocketAddr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceivedPacket {
    pub payload: Vec<u8>,
    pub source: SocketAddr,
}

impl UdpTransport {
    pub fn bind(local: SocketAddr) -> Result<Self, TransportError> {
        Ok(Self {
            socket: UdpSocket::bind(local)?,
            remote: None,
        })
    }

    pub fn bind_for_remote(remote: SocketAddr) -> Result<Self, TransportError> {
        let local = if remote.is_ipv4() {
            SocketAddr::from(([0, 0, 0, 0], 0))
        } else {
            SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 0], 0))
        };
        let mut transport = Self::bind(local)?;
        transport.remote = Some(remote);
        Ok(transport)
    }

    pub fn local_addr(&self) -> Result<SocketAddr, TransportError> {
        Ok(self.socket.local_addr()?)
    }

    pub fn remote_addr(&self) -> Option<SocketAddr> {
        self.remote
    }

    pub fn set_remote(&mut self, remote: SocketAddr) {
        self.remote = Some(remote);
    }

    pub fn send(&self, payload: &[u8]) -> Result<usize, TransportError> {
        if payload.len() > MAX_DATAGRAM_LENGTH {
            return Err(TransportError::OversizeDatagram);
        }
        let remote = self.remote.ok_or(TransportError::MissingRemote)?;
        Ok(self.socket.send_to(payload, remote)?)
    }

    pub fn receive(&self, timeout: Duration) -> Result<Option<ReceivedPacket>, TransportError> {
        self.socket.set_read_timeout(Some(timeout))?;
        let mut buffer = vec![0_u8; MAX_DATAGRAM_LENGTH + 1];
        match self.socket.recv_from(&mut buffer) {
            Ok((length, source)) => {
                if length > MAX_DATAGRAM_LENGTH {
                    return Ok(None);
                }
                buffer.truncate(length);
                Ok(Some(ReceivedPacket {
                    payload: buffer,
                    source,
                }))
            }
            Err(error) if error.kind() == io::ErrorKind::TimedOut => Ok(None),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

pub struct EncryptedTransport {
    socket: UdpTransport,
    codec: DatagramCodec,
}

impl EncryptedTransport {
    pub fn new(socket: UdpTransport, codec: DatagramCodec) -> Self {
        Self { socket, codec }
    }

    pub fn send(
        &mut self,
        timestamp: u16,
        timestamp_reply: u16,
        payload: Vec<u8>,
    ) -> Result<usize, TransportError> {
        let packet = self.codec.encode(timestamp, timestamp_reply, payload)?;
        self.socket.send(&packet)
    }

    pub fn receive(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<(ReceivedDatagram, SocketAddr)>, TransportError> {
        let Some(packet) = self.socket.receive(timeout)? else {
            return Ok(None);
        };
        let decoded = self.codec.decode(&packet.payload)?;
        Ok(Some((decoded, packet.source)))
    }

    pub fn note_roaming_source(&mut self, source: SocketAddr) {
        self.socket.set_remote(source);
    }

    pub fn local_addr(&self) -> Result<SocketAddr, TransportError> {
        self.socket.local_addr()
    }

    pub fn remote_addr(&self) -> Option<SocketAddr> {
        self.socket.remote_addr()
    }
}

pub fn encrypted_transport(
    remote: SocketAddr,
    crypto: CryptoSession,
    send_direction: crate::sequence::Direction,
) -> Result<EncryptedTransport, TransportError> {
    let socket = UdpTransport::bind_for_remote(remote)?;
    let codec = DatagramCodec::new(crypto, send_direction);
    Ok(EncryptedTransport::new(socket, codec))
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::time::Duration;

    use super::UdpTransport;

    #[test]
    fn sends_and_receives_without_connecting_socket() -> Result<(), Box<dyn std::error::Error>> {
        let receiver = UdpTransport::bind(SocketAddr::from(([127, 0, 0, 1], 0)))?;
        let sender = UdpTransport::bind_for_remote(receiver.local_addr()?)?;
        sender.send(b"packet")?;
        let packet = receiver.receive(Duration::from_secs(1))?.ok_or("timeout")?;
        assert_eq!(packet.payload, b"packet");
        let loopback = "127.0.0.1".parse::<std::net::IpAddr>()?;
        assert_eq!(packet.source.ip(), loopback);
        assert_eq!(packet.source.port(), sender.local_addr()?.port());
        Ok(())
    }
}
