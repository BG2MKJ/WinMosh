# WinMosh：原生 Windows Mosh 客户端，无需 WSL，一行命令安装

## 什么是 Mosh？为什么需要 WinMosh？

Mosh（Mobile Shell）是 MIT 开发的远程终端协议，专为移动场景设计。它的核心能力是：

- **断网不中断**：笔记本电脑休眠、Wi-Fi 切换、IP 地址变化后，SSH 连接直接断开，Mosh 自动恢复
- **高延迟可用**：在 4G/5G、跨国链路上，SSH 的逐字回显几乎不可用，Mosh 通过本地回显保持流畅
- **基于 UDP**：不同于 SSH 的 TCP 长连接，Mosh 的无状态 UDP 天然抗丢包

但官方 Mosh 发布 14 年来，一直没有原生 Windows 客户端。Windows 用户要装 WSL、Cygwin 或 MSYS2 才能用。

WinMosh 填补了这个空白：用 Rust 完整实现了 Mosh 客户端协议，直接运行在 Windows Terminal / PowerShell / cmd 中，**不依赖任何 Unix 兼容层**。

## 安装

```powershell
irm https://raw.githubusercontent.com/BG2MKJ/WinMosh/main/install.ps1 | iex
```

一行命令，自动下载解压，加入 PATH。安装后同时提供 `winmosh` 和 `wm` 两个命令名。

## 使用

```powershell
# 直接连接
wm user@host

# 指定服务器端 mosh-server 路径（conda 安装的情况）
wm user@host --server /home/user/miniconda3/bin/mosh-server

# 保存别名
wm alias add myserver user@192.168.1.100 --server /path/to/mosh-server
wm myserver

# 更新
wm update --download

# 卸载
wm --uninstall
```

## 技术实现

项目使用 Rust workspace 分为六个 crate：

| 模块 | 职责 | 原创度 |
|------|------|--------|
| winmosh-protocol | SSP 状态同步、AES-OCB 加密、数据报分片 | 协议逻辑参照官方 C++，Rust 独立编写 |
| winmosh-terminal | VT/ANSI 解析、帧缓冲 | 自研，渲染直接透传 HostBytes |
| winmosh-bootstrap | SSH 连接、远端 mosh-server 启动 | 自研 |
| winmosh-config | TOML 配置、别名管理、SSH config 解析 | 完全原创 |
| winmosh-platform | Windows 控制台 I/O、raw mode | 完全原创 |
| winmosh | CLI、会话主循环、更新/卸载 | 自研 |

关键协议实现：
- **SSP（State Synchronization Protocol）**：状态编号、diff 生成/应用、ACK 机制、重传，完全跟随官方 Mosh 的 C++ 实现
- **AES-128 OCB 加密**：通过官方 golden vector 验证，与 mosh-server 线级兼容
- **Protobuf 编解码**：手写轻量解析器，不依赖 protoc

## 兼容性

| 服务器 | mosh-server 来源 | 结果 |
|--------|-----------------|------|
| Ubuntu 22.04, apt 安装 | 系统包管理器 | ✅ |
| Ubuntu 22.04, conda-forge | miniconda3 | ✅ |

两个环境均无需修改服务器端，直接连接正常使用。

## 测试覆盖

86 个单元测试：
- winmosh-protocol: 38（加密、分片、状态同步、传输层）
- winmosh-terminal: 16（终端解析、帧缓冲、渲染）
- winmosh-config: 13（配置加载、别名、SSH 解析）
- winmosh-bootstrap: 8（SSH 命令构建、输出解析）
- winmosh: 10（CLI、会话、按键映射）
- winmosh-platform: 1（控制台模式）

网络韧性：模拟 70% 丢包率下 200 次状态变更，最终收敛一致；断连 40 个状态后恢复；任意乱序到达正确重组。

## 已知限制

- 预测性本地回显尚未实现，高延迟下打字有滞后感
- 不支持 Mosh 的滚动历史同步
- 仅支持 Windows x86_64

## 项目地址

https://github.com/BG2MKJ/WinMosh
