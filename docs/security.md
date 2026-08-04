# Security

WinMosh is a security-sensitive network client.

## M0 Rules

- Do not store SSH passwords, private-key contents, one-time codes, `MOSH_KEY`, or session keys.
- Do not print `MOSH_KEY` or parsed session-key contents.
- Keep unsafe code isolated to `winmosh-platform` for Windows API boundaries.
- Use `ssh.exe -G` only for effective non-secret configuration.
- Treat config parse failures as hard errors to protect damaged configuration files.

## Future Work

M1 and M2 must add tests proving bootstrap logs and panic/debug output never expose session keys.
