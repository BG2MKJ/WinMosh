# WinMosh

原生 Windows Mosh 客户端，使用 Rust 编写。直接兼容 Linux 上的官方 mosh-server，无需 WSL、Cygwin 或虚拟机。

与 mosh-server 1.4.0 协议互通。

---

## 安装

**PowerShell 一键安装**（需系统已安装 SSH 客户端）：

```powershell
irm https://raw.githubusercontent.com/BG2MKJ/WinMosh/main/install.ps1 | iex
```

这会自动下载最新 `winmosh.exe`，安装到 `%LOCALAPPDATA%\WinMosh\` 并加入 PATH。重新打开终端后即可使用。

> 手动安装：从 [Releases](https://github.com/BG2MKJ/WinMosh/releases) 下载 `winmosh.exe`，放到任意 PATH 目录。

## 使用

```powershell
# 直接连接
winmosh user@host

# 保存别名
winmosh alias add myserver user@192.168.1.100

# 用别名连接
winmosh myserver

# 覆盖选项
winmosh myserver --udp-port 60020 --terminal xterm-256color

# 诊断环境
winmosh doctor
winmosh doctor myserver
```

## 配置

配置文件位于 `%APPDATA%\WinMosh\config.toml`，可通过 `winmosh config path` 查看。

```toml
version = 1

[defaults]
mosh_server = "mosh-server"
udp_port = "60000:61000"
terminal = "xterm-256color"

[hosts.myserver]
ssh_target = "root@192.168.1.20"

[hosts.production]
ssh_target = "production-ssh"
udp_host = "203.0.113.10"
udp_port = "60020:60030"
```

WinMosh 同时支持 OpenSSH 的 `~/.ssh/config` Host 别名，配置优先级为：

> 命令行 > 主机配置 > defaults > OpenSSH 配置 > 内置默认值

## 架构

```
winmosh.exe
├── CLI / Session
├── Config (TOML, alias, SSH config 解析)
├── Bootstrap (SSH 启动远端 mosh-server)
├── Protocol (AES-OCB 加密, SSP 状态同步, 分片传输)
├── Terminal (VT/ANSI 解析, 帧缓冲, 渲染)
└── Platform (Windows 控制台 I/O)
```

## 构建

```powershell
# 需要 Rust 1.80+
cargo build --release -p winmosh

# 运行测试
cargo test --workspace
```

> MSVC toolchain 如遇 linker 错误，使用 GNU toolchain: `rustup default stable-x86_64-pc-windows-gnu`

## License

GPL-3.0-or-later
