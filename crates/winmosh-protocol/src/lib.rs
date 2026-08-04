#![forbid(unsafe_code)]

pub mod crypto;
pub mod datagram;
pub mod fragment;
pub mod proto;
pub mod sequence;
pub mod statesync;
pub mod timing;
pub mod transport;

pub fn protocol_status() -> &'static str {
    "interactive encrypted session implemented locally; interoperability unverified"
}
