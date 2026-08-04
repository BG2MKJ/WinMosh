# SSH Bootstrap

The bootstrap crate is responsible for starting the official remote `mosh-server` through `ssh.exe` and parsing the resulting connection line.

## Current Status

Implemented:

- Non-secret bootstrap request type.
- Remote command construction compatible with the official `mosh-server new` shape.
- Parser for `MOSH CONNECT <PORT> <KEY>` server output.
- CRLF, banner, non-UTF-8 noise, duplicate-line, and malformed-output handling.
- `SessionKey` debug redaction and zero-on-drop storage.
- Interactive `ssh.exe` execution with inherited stdin and forwarded stderr.
- Bootstrap timeout, output limit, child cleanup, and non-secret diagnostics.
- `winmosh TARGET --bootstrap-only` acceptance output.

Not verified yet:

- Remote orphan-process cleanup behavior for `--bootstrap-only`.
- Interoperability against a real Linux host running the unmodified official `mosh-server`.

The UDP protocol session is wired locally through the encrypted transport, fragment assembler,
state synchronization, and VT terminal layers. It still requires validation against a real server
before compatibility is claimed.
