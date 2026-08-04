# Protocol Mapping

All protocol work must map to the official Mosh implementation rather than inventing a new protocol.

## Upstream Sources

- `scripts/`: RESEARCHED — bootstrap argument order and `MOSH CONNECT` grammar reviewed.
- `src/crypto/`: IMPLEMENTED LOCALLY — AES-OCB session keys, nonce construction, authentication,
  and usage limits.
- `src/network/`: IMPLEMENTED LOCALLY — packet framing, UDP transport, timeout handling, and roaming.
- `src/protobufs/`: IMPLEMENTED LOCALLY — proto2-compatible instruction encoding and decoding.
- `src/statesync/`: IMPLEMENTED LOCALLY — bounded history, ACKs, retransmission, fragments, and convergence tests.
- `src/terminal/`: IMPLEMENTED LOCALLY — VT parser, framebuffer, state application, and rendering.
- `src/frontend/`: IMPLEMENTED LOCALLY — bootstrap-to-UDP session wiring and keyboard event mapping.

## Rust Targets

- `winmosh-bootstrap`: IMPLEMENTED — SSH bootstrap execution, bounded output parsing, and secret-safe diagnostics.
- `winmosh-protocol::crypto`: IMPLEMENTED LOCALLY — AES-OCB datagram cryptography.
- `winmosh-protocol::datagram`: IMPLEMENTED LOCALLY — authenticated packet encoding and decoding.
- `winmosh-protocol::transport`: IMPLEMENTED LOCALLY — UDP transport, timeouts, and roaming.
- `winmosh-protocol::sequence`: IMPLEMENTED LOCALLY — packet direction, sequence, and replay handling.
- `winmosh-terminal`: IMPLEMENTED LOCALLY — terminal model, state diffing, input, and rendering.

The local implementation is covered by unit tests and can run an encrypted interactive session.
Interoperability with an unmodified Linux `mosh-server`, upstream SSP timing edge cases, and
predictive local echo still require real-server validation.
