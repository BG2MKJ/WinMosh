# WinMosh Configuration

The default configuration path is:

```text
%APPDATA%\WinMosh\config.toml
```

Use `winmosh --config <PATH>` to override the path for a single command.

## Schema

```toml
version = 1

[defaults]
mosh_server = "mosh-server"
udp_port = "60000:61000"
terminal = "xterm-256color"
prediction = "off"

[hosts.myserver]
ssh_target = "root@example.com"
udp_host = "example.com"
udp_port = "60020:60030"
mosh_server = "/usr/local/bin/mosh-server"
terminal = "xterm-256color"
prediction = "off"
```

## Alias Commands

```text
winmosh alias add myserver root@example.com
winmosh alias list
winmosh alias show myserver
winmosh alias remove myserver
winmosh alias rename old-name new-name
```

Alias names are limited to ASCII letters, digits, `.`, `_`, and `-`.

## Safety

Configuration writes use a same-directory temporary file and atomic replacement. Existing config bytes are checked before writing so concurrent edits are detected instead of overwritten.

Secrets are not stored in the configuration file. Do not add SSH passwords, private-key contents, one-time codes, `MOSH_KEY`, or temporary session keys.
