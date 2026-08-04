# WinMosh Architecture

WinMosh is a native Windows client that targets protocol compatibility with the official `mosh-server`.

## Workspace

- `crates/winmosh`: CLI entry point, command dispatch, diagnostics, and session orchestration.
- `crates/winmosh-config`: TOML configuration, alias CRUD, target resolution, and `ssh.exe -G` parsing.
- `crates/winmosh-bootstrap`: SSH bootstrap command construction and server-output parsing.
- `crates/winmosh-protocol`: AES-OCB datagrams, UDP roaming transport, protobuf-compatible
  instructions, fragments, and local SSP state synchronization.
- `crates/winmosh-terminal`: VT parser, framebuffer, terminal state object, input stream, and renderer.
- `crates/winmosh-platform`: Windows-specific filesystem, process, and console APIs.

## Current Milestone

M0 and M1 implement the local CLI, configuration, SSH bootstrap, and key-free diagnostics. The
interactive path now wires bootstrap output into the encrypted UDP, fragment, state-sync, and VT
terminal layers. Live compatibility with an unmodified Linux server remains unverified.

The diagnostic path resolves a target and prints non-sensitive effective values, including:

```text
protocol status: interactive encrypted session implemented locally; interoperability unverified
```

## Direction

Future milestones must follow the upstream Mosh protocol and should update `docs/protocol-mapping.md` before porting implementation details.
