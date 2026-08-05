# Title

Show HN: WinMosh – Native Windows Mosh client, no WSL required

# Body

Mosh is an amazing protocol for remote terminal work — it survives
laptop suspend, Wi-Fi roaming, and high-latency links. But for 14 years,
Windows users had to install WSL or Cygwin to use it.

WinMosh is a from-scratch Rust implementation of the Mosh client protocol
that runs natively on Windows.

Key facts:
- One-command install (PowerShell):
  irm https://raw.githubusercontent.com/BG2MKJ/WinMosh/main/install.ps1 | iex
- Compatible with any existing Linux mosh-server (tested with v1.4.0)
- Native Windows Terminal — no WSL, Cygwin, or MSYS2 needed
- AES-OCB encryption verified against Mosh golden vectors
- 86 unit tests covering SSP state sync, fragmentation, encryption
- Survives 70% simulated packet loss without session loss
- GPL-3.0, ~8K lines of Rust

The protocol layer (SSP, encryption, datagram framing) follows the
official Mosh specification. The Windows platform layer and TOML config
system are original work.

Limitations: prediction/local echo is not yet implemented. Terminal
rendering now writes HostBytes directly — matching the original Mosh
approach.

Happy to answer questions. This has been a one-person project and I'd
love feedback from Windows users who've wanted a native Mosh client.

Repo: https://github.com/BG2MKJ/WinMosh
