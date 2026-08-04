# Compatibility

WinMosh is still pre-release, but the local client path is now substantially implemented.

Implemented compatibility-related behavior:

- Uses Windows-native executable layout.
- Reuses OpenSSH by invoking `ssh.exe -G <target>` for effective target configuration.
- Allows WinMosh aliases to point at either direct SSH targets or OpenSSH `Host` aliases.

Implemented locally but not yet verified against the upstream server:

- Launching remote `mosh-server` through SSH bootstrap.
- Encrypted UDP datagram/session wiring.
- Local state synchronization, retransmission, and compressed fragments.
- Interactive terminal session wiring with Windows console input/output.

Not yet verified or complete:

- Interoperability with an unmodified Linux `mosh-server`.
- Full upstream SSP timing, chaff, and roaming semantics.
- Predictive local echo and network migration behavior.
- Production update delivery until a tagged GitHub Release is published.
