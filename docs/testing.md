# Testing

Expected checks from the README:

```text
cargo build --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check
```

## Local Coverage

- Alias validation and CRUD.
- TOML parsing and serialization.
- Atomic-write and concurrent-modification behavior.
- `ssh.exe -G` fixture parsing.
- Official-shaped bootstrap command construction.
- Bootstrap output parsing without key disclosure through `Debug` or process errors.
- CRLF, banner, non-UTF-8 noise, malformed fields, and duplicate connect-line handling.
- Secret-safe SSH diagnostic formatting.
- CLI parser behavior.
- Update command mode parsing, semantic-version comparison, and release-asset selection.
- Ignored live-server test for SSH bootstrap, encrypted UDP state exchange, and remote state decoding.
- AES-OCB golden vector and tamper rejection.
- Datagram direction, sequence, replay, and UDP transport behavior.
- Fragment compression, reordering, duplicate handling, and size limits.
- State-sync convergence, ACKs, retransmission, and missing-base rejection.
- VT parsing, cursor movement, rendering, terminal state replay, and key mapping.
- Minimal `ConsoleGuard` logic.

## Interoperability

No real `mosh-server` interoperability test has been run in this workspace yet. The local
interactive session path and `--bootstrap-only` path are implemented, but both must be exercised
against an unmodified Linux `mosh-server` before compatibility is claimed.

The update command uses the latest GitHub Release API. A live update check/download test requires
the repository to have a published release with a supported Windows asset; unit tests cover the
parser and asset-selection logic without depending on the network.

The live mosh-server test is `crates/winmosh/tests/live_remote.rs`. It is ignored by default and
requires passwordless SSH access to a Linux host with `mosh-server` installed. It sends the first
encrypted client state and waits for a decodable remote state response.
