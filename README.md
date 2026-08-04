WinMosh 是使用 Rust 编写的原生 Windows Mosh 客户端，直接兼容 Linux 上现有的官方 mosh-server，运行时不依赖 WSL、Cygwin 或虚拟机。

这不是“重新发明一个远程终端协议”，而是对官方 Mosh 客户端进行协议兼容型 Rust 移植。

WinMosh

A native Mosh-compatible client for Windows, written in Rust.使用 Rust 编写、直接运行在 Windows 终端中的 Mosh 原生客户端。

[!IMPORTANT]WinMosh 已完成第一轮协议验证和网络韧性测试，与 mosh-server 1.4.0 成功互通。当前为 `v0.1.0` 预览版本，功能基本可用，欢迎测试反馈。

## 安装

在 **PowerShell** 中运行以下命令（需要系统已安装 SSH 客户端）：

```powershell
irm https://raw.githubusercontent.com/BG2MKJ/WinMosh/main/install.ps1 | iex
```

这会自动从 GitHub Release 下载最新 `winmosh.exe`，安装到 `%LOCALAPPDATA%\WinMosh\` 并加入用户 PATH。安装完成后重新打开终端即可使用 `winmosh` 命令。

如需手动安装：从 [Releases](https://github.com/BG2MKJ/WinMosh/releases) 页面下载 `winmosh.exe`，放到任意在 PATH 中的目录。

## 使用

```powershell
# 直接连接
winmosh user@host

# 保存别名
winmosh alias add myserver user@192.168.1.100

# 使用别名连接
winmosh myserver

# 诊断
winmosh doctor
```

1. 项目愿景

Mosh 能在网络中断、设备休眠、IP 地址变化、高延迟和丢包环境下保持远程终端会话，但官方目前没有提供原生 Windows 可执行客户端。Windows 用户通常需要依赖 WSL、Cygwin 或其他兼容环境，安装和终端集成都不够自然。

WinMosh 的目标是：

winmosh myserver

用户无需启动 WSL，也无需安装完整 Unix 兼容层，就能在 Windows Terminal、PowerShell 或命令提示符中连接现有 Linux/Unix 服务器上的官方 mosh-server。

WinMosh 不是重新设计一种“类似 Mosh”的协议，而是：

使用 Rust 实现一个与官方 mosh-server 线级兼容、行为兼容的原生 Windows Mosh 客户端。

核心目标：

原生 Windows 可执行文件；

运行时不依赖 WSL、Cygwin、MSYS2、Docker 或虚拟机；

直接兼容服务器上现有的官方 mosh-server；

使用系统 ssh.exe 完成登录认证和远端服务启动；

支持连接别名，做到 winmosh myserver 一条命令连接；

支持断网恢复、休眠唤醒、网络切换和客户端 IP 漫游；

在高延迟、丢包和乱序环境下维持可用的交互终端；

最终支持 Mosh 的预测性本地回显；

以可测试、可审计的方式逐层移植官方协议和终端行为。

1.1 当前版本状态

- **版本**: v0.1.0
- **协议**: 与 mosh-server 1.4.0 互通，状态同步、加密传输验证通过
- **测试**: 85 个单元测试覆盖协议、终端、配置、安全模块
- **网络**: SSH bootstrap 正常，UDP 加密通道正常，模拟 70% 丢包率下状态最终收敛

2. 第一版产品形态

第一版 WinMosh 是一个纯命令行程序：

Windows Terminal / PowerShell / cmd
                │
                └── winmosh.exe

它不创建独立 GUI 窗口，也不内置终端标签页。WinMosh 直接在当前终端中：

读取用户键盘输入；

输出 ANSI/VT 控制序列；

监听终端尺寸变化；

显示网络状态；

在退出时恢复原始控制台模式。

第一版不以 ConPTY 为核心依赖。ConPTY 更适合“终端宿主启动另一个控制台程序”的场景，而 WinMosh 本身就是当前终端里的前台交互程序。未来开发独立 GUI 客户端时，再增加 ConPTY 后端。

2.1 第一版用户体验

直接连接：

winmosh user@example.com

使用 SSH 配置中的别名：

winmosh production

使用 WinMosh 自己的别名：

winmosh myserver

第一次创建别名：

winmosh alias add myserver root@192.168.1.20
winmosh myserver

查看和管理别名：

winmosh alias list
winmosh alias show myserver
winmosh alias remove myserver
winmosh config path

诊断连接环境：

winmosh doctor
winmosh doctor myserver

检查或下载最新版本：

winmosh update --check
winmosh update --download

临时覆盖配置：

winmosh myserver --udp-port 60020
winmosh myserver --udp-host gateway.example.com
winmosh myserver --server /usr/local/bin/mosh-server

3. 别名与配置设计

别名不是附属功能，而是 WinMosh 第一版的核心体验。

WinMosh 同时支持两种别名来源：

WinMosh 自己的主机配置；

现有 OpenSSH 的 Host 别名。

因此用户既可以直接复用：

# %USERPROFILE%\.ssh\config

Host production
    HostName 203.0.113.10
    User deploy
    Port 2222
    IdentityFile ~/.ssh/production_ed25519

然后运行：

winmosh production

也可以在 WinMosh 配置中定义更适合 Mosh 的参数，例如 UDP 地址、UDP 端口范围或远端 mosh-server 路径。

3.1 配置文件位置

默认配置文件：

%APPDATA%\WinMosh\config.toml

用户可以通过以下命令查看实际路径：

winmosh config path

也可以临时指定：

winmosh --config D:\configs\winmosh.toml myserver

配置文件只保存主机信息和非敏感选项，不保存：

SSH 密码；

私钥内容；

一次性验证码；

MOSH_KEY；

任何临时会话密钥。

3.2 配置示例

version = 1

[defaults]
mosh_server = "mosh-server"
udp_port = "60000:61000"
terminal = "xterm-256color"
prediction = "off"

[hosts.myserver]
ssh_target = "root@192.168.1.20"

[hosts.production]
# 可以指向 ~/.ssh/config 中的 Host 别名
ssh_target = "production-ssh"

# SSH 所连接的地址和 UDP 可达地址不同时使用。
# 例如 SSH 通过跳板机启动 mosh-server，但服务器另有可直达 UDP 地址。
udp_host = "203.0.113.10"
udp_port = "60020:60030"

[hosts.lab]
ssh_target = "student@lab.example.edu"
mosh_server = "/usr/local/bin/mosh-server"
terminal = "xterm-256color"

3.3 别名写入命令

最简单的别名：

winmosh alias add myserver root@192.168.1.20

等价于写入：

[hosts.myserver]
ssh_target = "root@192.168.1.20"

带 Mosh 专用覆盖项：

winmosh alias add production production-ssh `
    --udp-host 203.0.113.10 `
    --udp-port 60020:60030 `
    --server /usr/local/bin/mosh-server

别名名称第一版只允许：

A-Z a-z 0-9 . _ -

配置写入必须采用：

解析已有配置；

在内存中修改；

写入同目录临时文件；

fsync 或等价刷新；

原子替换原文件。

禁止直接截断原文件后覆盖，避免程序中途退出造成配置损坏。

3.4 目标解析规则

执行：

winmosh myserver

时按照以下流程解析：

CLI 输入
   │
   ├── 1. 查找 WinMosh [hosts.myserver]
   │       └── 找到：取得 ssh_target 和 Mosh 专用配置
   │
   └── 2. 未找到 WinMosh 别名
           └── 将 myserver 原样视为 OpenSSH target

随后 WinMosh 调用：

ssh.exe -G <ssh_target>

获取 OpenSSH 计算后的有效配置，例如：

hostname；

user；

port；

地址族；

配置文件合并结果。

WinMosh 不自行重新实现完整的 OpenSSH 配置解析器。Include、Match、Host 通配符、IdentityFile、ProxyJump 和认证逻辑仍由系统 ssh.exe 负责。

最终配置优先级为：

命令行参数
    >
WinMosh 主机配置
    >
WinMosh defaults
    >
OpenSSH 有效配置
    >
WinMosh 内置默认值

3.5 UDP 地址解析

SSH 只负责启动远端 mosh-server，真正的会话通过 UDP 直连服务器。因此“SSH 能连接”不代表“UDP 一定可达”。

UDP 目标地址按以下优先级选择：

--udp-host
    >
[hosts.<alias>].udp_host
    >
ssh -G 返回的 hostname
    >
用户输入中的主机名

若主机名解析出多个 IPv4/IPv6 地址，WinMosh 应保留候选列表，并尝试从能通过认证数据报收到有效响应的地址建立会话，而不是把第一个 DNS 结果永久写死。

通过 ProxyJump 或 ProxyCommand 启动远端程序并不意味着 Mosh 的 UDP 数据也会经过 SSH 跳板。第一版只有在目标服务器存在客户端可直接访问的 UDP 地址时才支持这种场景，可通过 udp_host 显式指定。

4. WinMosh 如何工作

整体连接流程：

winmosh myserver
        │
        ▼
解析 CLI 与 WinMosh 配置
        │
        ▼
调用 ssh.exe -G 取得有效 SSH 配置
        │
        ▼
通过 ssh.exe 登录并启动官方 mosh-server
        │
        ▼
从 SSH 输出中解析 UDP 端口和临时会话密钥
        │
        ▼
SSH bootstrap 通道结束
        │
        ▼
WinMosh 使用加密 UDP 与 mosh-server 通信
        │
        ▼
SSP 同步用户输入状态与远端终端屏幕状态
        │
        ▼
在当前 Windows 终端中渲染会话

Mosh 和普通 SSH 的根本区别不是“TCP 改成 UDP”。

普通 SSH 主要传输连续字节流：

远端输出字节流 → 本地终端模拟器

Mosh 在服务器和客户端都维护终端状态，并通过 State Synchronization Protocol（SSP）同步“当前状态”：

客户端 → 服务端：用户输入历史状态
服务端 → 客户端：终端屏幕状态

因此即使中间数据包丢失、重复、乱序或连接暂时中断，双方仍可以继续向最新状态收敛，而不要求每一个旧数据包都按顺序到达。

5. 范围与非目标

5.1 v0.1.0 必须实现

原生 winmosh.exe；

Windows Terminal 中直接交互；

winmosh user@host；

winmosh myserver；

WinMosh TOML 别名；

复用 %USERPROFILE%\.ssh\config；

调用系统 ssh.exe 完成认证；

支持公钥、密码以及 SSH 自身可以完成的交互认证；

启动现有官方 mosh-server；

与官方服务端兼容的加密 UDP 数据报；

SSP 基础状态同步；

远程 Shell 输入和显示；

UTF-8；

基本颜色、光标、清屏和备用屏幕；

终端窗口尺寸变化；

Ctrl-C、Ctrl-D、方向键和常见功能键；

短时断网后自动恢复；

客户端 IP 地址变化后恢复通信；

清晰的错误诊断；

Windows x86_64 MSVC 发布包。

5.2 v0.1.0 不要求实现

Rust 版 mosh-server；

独立 GUI；

标签页或多会话管理界面；

SSH 端口转发；

X11 转发；

文件传输；

内置 SSH 协议栈；

将 Mosh UDP 流量通过 SSH ProxyJump 转发；

完整滚动历史同步；

全部预测性本地回显；

Windows 以外平台的正式支持。

5.3 v0.2.0 计划

预测性本地回显；

未确认预测字符样式；

预测失败回滚；

更完整的 Unicode 宽字符和组合字符支持；

Windows ARM64；

安装器、WinGet 和 Scoop 分发；

Windows Terminal profile 自动安装；

更完善的弱网状态提示；

连接统计与可选诊断日志。

6. 架构原则

6.1 兼容优先，不重新设计协议

官方 Mosh C++ 实现、官方 Protobuf 定义、官方测试和 Mosh 论文是协议行为基准。

任何以下内容都不得凭经验猜测：

数据报头格式；

序号编码；

nonce 构造；

AES-128 OCB 参数；

tag 长度；

时间戳格式；

Protobuf 字段；

SSP ACK 行为；

diff 语义；-终端状态语义；

用户输入同步语义；

本地预测行为。

每移植一个模块，都必须在 docs/protocol-mapping.md 中记录：

官方实现

WinMosh 实现

状态

测试证据

C++ 文件、类或函数

Rust crate、module、type

TODO / PARTIAL / VERIFIED

测试名称或互操作记录

6.2 小步互操作，而不是一次性重写

开发顺序必须是：

工程与别名
    →
SSH bootstrap
    →
加密数据报
    →
通用 SSP
    →
用户输入状态
    →
终端状态
    →
Windows 渲染
    →
断网与漫游
    →
预测性本地回显

没有通过官方服务端互操作测试之前，禁止声明“兼容 Mosh”。

6.3 单一状态所有者

协议状态、终端状态和加密状态由单个 SessionActor 独占。

不要让多个异步任务共同持有：

Arc<Mutex<ProtocolState>>
Arc<Mutex<TerminalState>>

其他任务只产生事件，由 SessionActor 顺序处理。

6.4 Windows 代码隔离

Win32 API 和 unsafe 代码只能出现在 winmosh-platform 中。

每处 unsafe 必须写明：

指针或句柄为什么有效；

生命周期由谁保证；

缓冲区边界如何保证；

调用失败后如何恢复；

多线程访问是否安全。

6.5 秘密最小暴露

临时 Mosh 会话密钥：

不写入环境变量，除非互操作研究证明无法避免；

不写入磁盘；

不出现在命令行；

不进入日志；

不实现 Debug；

不实现 Display；

不实现 Serialize；

默认不实现 Clone；

Drop 时主动清零；

测试错误消息中也不能出现原始密钥。

7. Workspace 结构

winmosh/
├── Cargo.toml
├── Cargo.lock
├── LICENSE
├── README.md
├── rustfmt.toml
├── deny.toml
├── .github/
│   └── workflows/
│       ├── ci.yml
│       └── release.yml
├── crates/
│   ├── winmosh/
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── cli.rs
│   │   │   ├── session.rs
│   │   │   ├── doctor.rs
│   │   │   └── error.rs
│   │   └── Cargo.toml
│   ├── winmosh-config/
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── model.rs
│   │   │   ├── load.rs
│   │   │   ├── resolve.rs
│   │   │   ├── alias.rs
│   │   │   └── ssh_config.rs
│   │   └── Cargo.toml
│   ├── winmosh-bootstrap/
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── ssh.rs
│   │   │   ├── command.rs
│   │   │   ├── parser.rs
│   │   │   ├── secret.rs
│   │   │   └── error.rs
│   │   └── Cargo.toml
│   ├── winmosh-protocol/
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── crypto.rs
│   │   │   ├── datagram.rs
│   │   │   ├── timing.rs
│   │   │   ├── transport.rs
│   │   │   ├── sequence.rs
│   │   │   └── proto.rs
│   │   ├── proto/
│   │   └── Cargo.toml
│   ├── winmosh-terminal/
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── cell.rs
│   │   │   ├── framebuffer.rs
│   │   │   ├── cursor.rs
│   │   │   ├── rendition.rs
│   │   │   ├── complete_terminal.rs
│   │   │   ├── user_input.rs
│   │   │   ├── diff.rs
│   │   │   └── renderer.rs
│   │   └── Cargo.toml
│   └── winmosh-platform/
│       ├── src/
│       │   ├── lib.rs
│       │   └── windows/
│       │       ├── console.rs
│       │       ├── input.rs
│       │       ├── output.rs
│       │       ├── resize.rs
│       │       └── signals.rs
│       └── Cargo.toml
├── tests/
│   ├── fixtures/
│   │   ├── bootstrap/
│   │   ├── crypto/
│   │   ├── datagram/
│   │   ├── statesync/
│   │   └── terminal/
│   ├── interoperability/
│   └── network-chaos/
└── docs/
    ├── architecture.md
    ├── bootstrap.md
    ├── configuration.md
    ├── protocol-mapping.md
    ├── compatibility.md
    ├── security.md
    ├── testing.md
    └── roadmap.md

8. 模块职责

8.1 winmosh

二进制入口和会话编排：

CLI；

子命令；

错误展示；

目标解析流程；

连接生命周期；

SessionActor；

诊断命令；

退出和清理。

它可以依赖其他所有 crate，但其他 crate 不得反向依赖它。

8.2 winmosh-config

负责：

配置文件定位；

TOML Schema；

配置版本；

默认值；

别名 CRUD；

原子写入；

CLI、WinMosh 配置与 OpenSSH 配置合并；

调用 ssh.exe -G 获取有效 SSH 配置；

生成 UDP 候选地址；

对用户展示不含秘密的最终配置。

建议的核心类型：

pub struct AppConfig {
    pub version: u32,
    pub defaults: Defaults,
    pub hosts: BTreeMap<String, HostConfig>,
}

pub struct HostConfig {
    pub ssh_target: String,
    pub udp_host: Option<String>,
    pub udp_port: Option<PortSpec>,
    pub mosh_server: Option<PathBuf>,
    pub terminal: Option<String>,
    pub prediction: Option<PredictionMode>,
}

pub struct ResolvedTarget {
    pub display_name: String,
    pub ssh_target: String,
    pub effective_ssh: EffectiveSshConfig,
    pub udp_candidates: Vec<SocketCandidate>,
    pub mosh_server: String,
    pub udp_port: PortSpec,
    pub terminal: String,
}

ssh.exe -G 失败时要区分：

ssh.exe 不存在；

target 无法解析；

配置文件错误；

OpenSSH 不支持所需参数；

子进程返回异常输出。

8.3 winmosh-bootstrap

只负责通过 SSH 建立 Mosh 会话参数。

输入：

pub struct BootstrapRequest {
    pub target: ResolvedTarget,
    pub columns: u16,
    pub rows: u16,
    pub locale: String,
}

输出：

pub struct BootstrapResult {
    pub udp_port: u16,
    pub session_key: SessionKey,
    pub remote_pid: Option<u32>,
    pub server_version: Option<String>,
}

职责：

查找 ssh.exe；

构造参数列表；

继承 stdin，使密码和 MFA 能由 ssh.exe 正常交互；

继承或转发 stderr，使用户看到 SSH 提示；

管道读取 stdout；

启动官方 mosh-server；

解析连接行；

限制最大输出；

设置 bootstrap 超时；

保证错误信息不含密钥；

正确等待或终止 SSH 子进程。

远端命令必须以官方 mosh 启动脚本为行为参考。不得根据记忆猜测 mosh-server 参数。

输出解析器必须覆盖：

正常连接行；

前后存在 MOTD 或 banner；

CRLF 和 LF；

非 UTF-8 噪声；

缺少字段；

非法端口；

非法密钥；

多条连接行；

超长输出；

SSH 中途退出。

8.4 winmosh-protocol

项目最核心且安全敏感的部分。

内部逻辑分层：

Crypto
  ↓
Datagram
  ↓
SSP Transport
  ↓
Terminal/UserInput Object

Crypto

负责与官方 Mosh 完全一致的：

AES-128 OCB；

nonce；

tag；

密钥长度；

认证失败；

常量时间比较；

密钥清零。

可以评估 RustCrypto 的 aes、ocb3 和 zeroize，但只有通过官方生成的 golden vector 后才能采用。不得因为算法名称相同就认定线级兼容。

Datagram

负责：

UDP 数据报封装；

发送序号；

接收序号；

重复包处理；

乱序包处理；

时间戳；

timestamp reply；

SRTT；

RTTVAR；

RTO；

心跳；

对端地址漫游；

数据报大小上限；

网络输入验证。

服务端只有在收到认证成功且序号更新的数据报后，才可以把新的来源地址视为客户端的新 UDP 地址。该行为必须与官方实现一致。

SSP Transport

负责在不可靠数据报上同步抽象状态：

sender state；

receiver state；

acknowledged state；

source state number；

target state number；

instruction；

diff；

ack；

retransmission；

delayed acknowledgement；

当前最新状态收敛。

SSP 必须先用简单测试对象验证，再接入终端：

struct TestState(String);

先证明以下情况都能最终收敛：

丢包；

乱序；

重复包；

延迟；

ACK 丢失；

双向同时更新；

短时完全断网；

恢复后的旧包到达。

8.5 winmosh-terminal

WinMosh 不能把 Mosh 当成普通“UDP 字节流终端”。

它需要维护和同步：

Cell；

Framebuffer；

Cursor；

Rendition；

CompleteTerminal；

UserInput；

终端 diff；

用户输入 diff；

预测状态；

权威服务端状态；

本地显示状态。

第一版最低支持：

UTF-8；

ASCII；

CJK 宽字符；

组合字符的基本行为；

前景色和背景色；

256 色；

粗体；

下划线；

反色；

光标位置；

光标显示和隐藏；

清屏；

区域清除；

行插入和删除；

备用屏幕；

resize；

常见 ECMA-48/VT 控制行为。

渲染器不应每次重画整个屏幕，而应：

比较上次已渲染状态和目标状态；

生成最小或足够小的 VT 更新；

批量写入 stdout；

一次刷新；

更新 rendered state。

8.6 winmosh-platform

隔离所有 Windows 行为：

获取标准输入输出句柄；

判断是否运行在交互终端；

保存控制台模式；

启用 VT 输出；

验证 VT 输入路径；

读取 Unicode 输入；

转换方向键和功能键；

处理 Ctrl 组合键；

获取行列数；

监听 resize；

处理 Ctrl-C 和关闭事件；

退出时恢复控制台；

终端能力检测。

核心 RAII 类型：

pub struct ConsoleGuard {
    // original handles and modes
}

要求：

构造过程中任一步失败，都恢复已经修改的状态；

Drop 始终尝试恢复；

恢复失败只能记录脱敏错误，不能 panic；

不允许在正常路径使用 process::exit 绕过析构；

panic hook 和控制台关闭事件也要触发最佳努力清理。

输入后端定义为可替换接口：

pub trait InputBackend {
    fn read_event(&mut self) -> Result<InputEvent, PlatformError>;
}

第一阶段应先做技术验证，确认 Windows Terminal 下最小可靠输入方案，再决定使用 VT input 还是 ReadConsoleInputW。不要在未验证时把某一种路径写死到上层协议。

9. 并发模型

采用单个 SessionActor 作为状态唯一所有者：

                    ┌─────────────────────┐
键盘输入任务 ───────→                     │
UDP 接收任务 ───────→    SessionActor     ├────→ UDP 发送
Resize 任务 ─────────→                     ├────→ 终端渲染
定时器任务 ──────────→                     │
关闭信号 ────────────→                     │
                    └─────────────────────┘

事件模型：

pub enum SessionEvent {
    UserInput(Vec<u8>),
    DatagramReceived {
        bytes: bytes::Bytes,
        source: std::net::SocketAddr,
    },
    Resize {
        columns: u16,
        rows: u16,
    },
    Tick(std::time::Instant),
    Interrupt,
    Shutdown,
}

SessionActor 独占：

会话密钥；

加密上下文；

发送序号；

接收窗口；

RTT 状态；

SSP sender；

SSP receiver；

用户输入状态；

服务端权威终端状态；

本地预测状态；

已渲染状态；

当前服务器 UDP 地址；

最后发送和接收时间；

连接状态提示。

UDP 接收任务只负责：

从 socket 读数据；

检查绝对大小上限；

把原始数据和来源地址投递给 actor。

它不得自行修改漫游地址、序号或 SSP 状态。

10. CLI 设计

winmosh [OPTIONS] <TARGET>
winmosh alias <COMMAND>
winmosh config <COMMAND>
winmosh doctor [TARGET]
winmosh version

10.1 连接命令

winmosh myserver
winmosh user@example.com

计划参数：

--config <PATH>
--ssh <PATH>
--server <REMOTE_PATH>
--udp-host <HOST>
--udp-port <PORT|START:END>
--family <auto|ipv4|ipv6>
--terminal <TERM>
--prediction <off|adaptive|always|never>
--connect-timeout <DURATION>
--log-level <error|warn|info|debug|trace>
--log-file <PATH>
--bootstrap-only
--no-color

--bootstrap-only 仅用于开发诊断。执行器必须确认官方服务端在客户端不建立 UDP 会话时如何退出，避免无意留下大量孤儿进程。

10.2 Alias 子命令

winmosh alias add <NAME> <SSH_TARGET>
winmosh alias list
winmosh alias show <NAME>
winmosh alias remove <NAME>
winmosh alias rename <OLD> <NEW>

输出必须适合人阅读，同时预留：

--json

供未来脚本使用。

10.3 Config 子命令

winmosh config path
winmosh config show
winmosh config edit
winmosh config validate

config edit 可以调用用户的 $EDITOR / %EDITOR%，未配置编辑器时只打印路径，不自行捆绑编辑器。

10.4 Doctor 子命令

winmosh doctor

本地检查：

Windows 版本和架构；

当前是否为交互终端；

VT 输出能力；

输入后端能力；

ssh.exe 是否存在；

ssh.exe -V；

配置文件是否可读；

配置 Schema 是否有效；

日志目录是否可写。

winmosh doctor myserver

额外检查：

目标解析；

ssh.exe -G；

SSH 登录和认证；

远端 mosh-server 是否存在；

服务端版本；

UDP 端口配置；

UDP 地址候选；

bootstrap 能否成功；

诊断结果中禁止显示会话密钥。

11. 错误与用户提示

错误必须告诉用户：

哪个阶段失败；

可以检查什么；

WinMosh 是否已经清理资源；

是否适合重试。

示例：

error: SSH bootstrap failed

Target: myserver
SSH target: production-ssh
Reason: the remote command could not find "mosh-server"

Install mosh on the server or configure:
  winmosh alias add myserver production-ssh --server /custom/path/mosh-server

error: no authenticated UDP response was received

SSH bootstrap succeeded and mosh-server selected UDP port 60024,
but WinMosh could not establish the UDP session.

Check:
  1. UDP 60024 is allowed by the server firewall.
  2. The configured UDP host is reachable from this computer.
  3. NAT or a jump host is not hiding the real UDP destination.

状态信息不得污染远程应用的主屏幕。可以采用：

短暂标题栏更新；

状态行；

与官方 Mosh 相似的连接状态覆盖；

stderr 提示。

具体策略需要通过 vim、less、top 和 tmux 实测。

12. 安全要求

WinMosh 是安全敏感的网络客户端。

强制要求：

#![forbid(unsafe_code)] 用于除 winmosh-platform 外的 crate；

网络长度字段转换使用 checked conversion；

Protobuf 解码设置大小上限；

数据报设置绝对最大长度；

端口严格限制在 1..=65535；

alias 不允许控制字符；

配置路径规范化；

远端命令参数必须验证和安全引用；

不接受任意未审计 shell 片段作为配置；

不记录密钥；

不记录密码；

默认日志不记录用户输入内容；

日志中的 IP、用户名和路径提供隐私说明；

依赖使用 cargo-deny 和安全审计；

release 构建启用可用的编译加固；

所有加密互操作均使用官方测试向量验证；

认证失败的数据报不得推动任何状态；

旧序号、重复包和恶意乱序包不能造成无限内存增长；

所有缓存必须有上限；

配置更新使用原子替换；

错误路径必须恢复控制台模式。

许可证暂定：

GPL-3.0-or-later

因为项目将以官方 GPL Mosh 实现为兼容和移植基准。正式引入或翻译代码时，保留来源、版权和许可证记录。

13. 推荐 Rust 依赖

依赖版本由实现时选择当前稳定且兼容的版本，并提交 Cargo.lock。

候选依赖：

用途

候选

CLI

clap

TOML

serde, toml

配置目录

directories

异步运行时

tokio

字节缓冲

bytes

错误类型

thiserror

二进制顶层错误

anyhow

日志

tracing, tracing-subscriber

Protobuf

prost, prost-build

Windows API

windows

密钥清零

zeroize

加密候选

aes, ocb3

Unicode

unicode-width, unicode-segmentation

性质测试

proptest

临时文件

tempfile

依赖检查

cargo-deny

规则：

库 crate 不使用 anyhow 作为公共错误类型；

公共错误使用具体枚举；

不因方便而引入大型 SSH 库；

v0.1 直接使用系统 ssh.exe；

加密库只有通过互操作测试后才能进入正式实现。

14. 测试策略

14.1 单元测试

覆盖：

TOML 解析；

配置版本；

alias 校验；

配置优先级；

原子写入；

ssh -G 输出解析；

bootstrap 输出解析；

SessionKey 脱敏；

端口范围；

地址候选；

序号边界；

数据报长度；

RTT 计算；

diff 生成与应用；

framebuffer 操作；

VT 渲染；

Windows 模式状态机。

14.2 Golden Test

从官方 Mosh 生成固定样例：

Protobuf 编码；

AES-128 OCB 数据报；

nonce；

tag；

Datagram；

SSP Instruction；

ACK；

UserInput diff；

CompleteTerminal diff；

framebuffer 状态。

Rust 对相同输入必须产生兼容结果。

14.3 Differential Test

同一组输入分别交给：

官方 C++ 模块
WinMosh Rust 模块

比较：

最终状态；

diff；

序号；

ACK；

framebuffer；

cursor；

rendition；

预测确认和回滚。

14.4 Interoperability Test

测试环境启动官方 mosh-server，WinMosh 连接并执行：

printf 'hello\n'
pwd
uname -a

再测试：

bash；

vim；

nano；

less；

top；

htop；

tmux；

中文输出；

中文输入；

resize；

Ctrl-C；

Ctrl-D；

备用屏幕进入和退出。

产品运行时不依赖 Docker 或 WSL，但开发和 CI 可以使用 Linux runner、容器或远程测试机启动官方服务端。

14.5 Network Chaos Test

模拟：

1%、5%、10%、30% 丢包；

随机乱序；

数据包重复；

50 ms、200 ms、500 ms RTT；

10 秒、30 秒、5 分钟完全断网；

Wi-Fi 切换热点；

本地 IP 地址变化；

设备休眠和唤醒；

服务端地址变化；

旧数据包在恢复后到达。

验收不是“所有包都收到”，而是：

会话不崩溃；

不出现无限内存增长；

不出现 CPU 空转；

用户输入最终到达；

屏幕最终收敛；

状态提示正确；

网络恢复后无需重新登录。

14.6 Windows 测试矩阵

第一版主目标：

Windows 11 x86_64
Windows Terminal
PowerShell 7
x86_64-pc-windows-msvc

补充测试：

Windows PowerShell
cmd.exe
传统 conhost
高 DPI
不同代码页
中文输入法
英文输入法
UTF-8 与非 UTF-8 服务端 locale

15. 开发里程碑

M0：工程骨架与别名体验

交付：

Rust workspace；

六个 crate；

GPL License；

README；

CI；

CLI；

配置模型；

alias CRUD；

config path/show/validate；

ssh.exe 查找；

ssh -G 解析；

doctor 本地部分；

Windows ConsoleGuard 骨架。

验收：

cargo build --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings

winmosh --help
winmosh --version
winmosh alias add myserver root@example.com
winmosh alias show myserver
winmosh alias list
winmosh config validate
winmosh doctor

M1：SSH Bootstrap

交付：

远端 mosh-server 启动；

连接信息解析；

SessionKey；

密钥清零；

交互认证；

bootstrap 超时；

明确错误；

真实 Linux 服务器验证。

验收：

winmosh myserver --bootstrap-only

输出只能包含：

target: myserver
ssh bootstrap: ok
remote mosh-server: found
udp port: 60024
bootstrap succeeded

禁止输出 key。

M2：加密 Datagram 互操作

交付：

官方 Proto；

AES-128 OCB；

nonce；

Datagram encode/decode；

认证失败处理；

序号；

timestamp；

官方 golden vectors。

验收：

WinMosh 数据报可被官方服务端接受；

WinMosh 可验证并解密官方服务端数据报；

任意字节被修改后认证失败；

key 不出现在日志和 panic 中。

M3：通用 SSP

交付：

generic state object；

instruction；

diff；

ACK；

retransmission；

delayed ACK；

RTT/RTO；

heartbeat；

property tests；

chaos tests。

验收：

简单字符串状态在丢包、乱序和重复环境下最终收敛；

断网后恢复；

缓存有界；

无死锁。

M4：终端状态与基础 Shell

交付：

UserInput；

CompleteTerminal；

framebuffer；

cursor；

rendition；

VT renderer；

Windows input；

resize。

验收：

winmosh myserver

能够：

pwd
ls
echo hello

M5：完整交互终端

交付：

备用屏幕；

颜色；

Unicode；

常用控制键；

vim/less/top/tmux；

状态提示；

控制台清理。

M6：断网与漫游

交付：

IP 切换；

休眠唤醒；

心跳；

无响应状态；

UDP 地址迁移；

network chaos 自动验收。

M7：预测性本地回显

交付：

预测模型；

未确认样式；

确认；

失败回滚；

高延迟体验测试。

16. v0.1.0 完成定义

只有同时满足以下条件，才能发布 v0.1.0：

在干净的 Windows 11 x86_64 环境中运行；

不安装 WSL、Cygwin 或 MSYS2；

winmosh myserver 可以解析 WinMosh 别名；

可以复用 %USERPROFILE%\.ssh\config；

可以使用系统 SSH 完成公钥和密码登录；

可以启动未经修改的官方 mosh-server；

可以进入远程交互 Shell；

vim、less、top 和 tmux 基础可用；

终端 resize 正常；

Ctrl-C 正常；

UTF-8 英文和中文基础可用；

断网 30 秒后恢复；

Wi-Fi 切换热点后会话恢复；

数据报认证和状态同步通过官方互操作测试；

无明文密钥日志；

异常退出后终端模式可以恢复；

所有 CI 检查通过；

发布文档明确列出未支持能力。

17. 执行器 AI 工作协议

阅读本 README 的代码执行器必须遵守以下规则。

17.1 不得擅自改变产品方向

禁止：

自行实现一个不兼容的新协议；

把 TCP 重连包装成 Mosh；

第一阶段实现 Rust 服务端；

用 WSL 内部调用官方 mosh 冒充原生实现；

把 GUI 放在基础协议之前；

跳过官方互操作；

把 SSH 密码存入配置；

把 session key 打印出来；

用大量 TODO 声称里程碑完成。

17.2 每次工作流程

每个子任务遵循：

阅读官方对应实现
    →
更新 protocol-mapping
    →
写最小设计
    →
写测试
    →
实现
    →
运行测试
    →
记录证据
    →
小提交

每次修改后运行：

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

涉及 Windows API时，还要在真实 Windows runner 上运行对应测试。

17.3 完成报告格式

每轮执行结束必须报告：

本轮目标；

修改文件；

架构决策；

实际执行的命令；

测试结果；

真实互操作结果；

未完成事项；

已知风险；

下一步；

commit hash。

没有运行过的测试不能写成“通过”。没有连接过官方 mosh-server 不能写成“兼容”。

18. 执行器第一轮任务

第一轮只实现 M0，不得提前伪造 Mosh 协议。

18.1 仓库初始化

创建 workspace；

创建六个 crate；

设置 Rust edition；

提交 Cargo.lock；

添加 GPL-3.0-or-later；

添加 rustfmt.toml；

添加 cargo-deny 配置；

添加 Windows CI。

18.2 CLI

实现：

winmosh --help
winmosh --version
winmosh alias add
winmosh alias list
winmosh alias show
winmosh alias remove
winmosh config path
winmosh config show
winmosh config validate
winmosh doctor

连接命令会完成 SSH bootstrap，并进入加密 UDP 会话：

protocol status: interactive encrypted session implemented locally; interoperability unverified

真实 Linux 服务端互操作仍需单独验证。

18.3 配置与别名

实现：

默认配置路径；

TOML Schema；

version 字段；

defaults；

hosts；

alias 名称校验；

add/list/show/remove；

原子写入；

损坏配置保护；

并发修改检测；

单元测试；

--config 覆盖。

18.4 OpenSSH 解析

实现：

--ssh <PATH>；

PATH 查找；

Windows OpenSSH 常见路径；

调用 ssh.exe -G target；

解析有效 hostname/user/port/addressfamily；

不解析私钥内容；

超时；

输出大小限制；

测试 fixture；

ResolvedTarget。

WinMosh alias 不存在时，应把用户参数原样传给 OpenSSH：

winmosh production

可以直接使用 .ssh/config 中的 Host production。

18.5 Windows ConsoleGuard 技术验证

实现最小 ConsoleGuard：

保存输入输出模式；

启用 VT 输出；

Drop 恢复；

构造失败回滚；

对可纯逻辑测试的部分写单元测试；

写一个手工测试命令；

不在 M0 提前实现完整输入映射。

18.6 文档

创建：

docs/architecture.md
docs/configuration.md
docs/bootstrap.md
docs/protocol-mapping.md
docs/security.md
docs/testing.md
docs/roadmap.md

protocol-mapping.md 记录官方源码模块与 Rust 移植状态，已实现部分标记为 IMPLEMENTED LOCALLY，
仍需真实服务端验证的部分明确标记为 interoperability pending。

18.7 M0 验收

必须实际演示：

winmosh alias add myserver root@example.com
winmosh alias show myserver
winmosh myserver

第三条命令应能展示解析后的非敏感信息：

alias: myserver
ssh target: root@example.com
effective host: example.com
effective user: root
effective ssh port: 22
mosh server: mosh-server
udp port preference: 60000:61000
protocol status: interactive encrypted session implemented locally; interoperability unverified

M0-M4 的本地实现已完成，包含 SSH bootstrap、加密 UDP、fragment/SSP、VT 终端和交互会话；
仍需连接真实 Linux 服务端验证完整兼容性。

19. 构建与开发

预期开发环境：

Windows 11
Rust stable
MSVC Build Tools
Git
Windows OpenSSH Client
Windows Terminal

构建：

cargo build --workspace

测试：

cargo test --workspace

运行：

cargo run -p winmosh -- --help

静态检查：

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check

发布构建：

cargo build --release -p winmosh

20. 服务端要求

WinMosh 第一版不安装或修改服务端。

远端需要：

可通过 SSH 登录；

安装官方 mosh-server；

使用 UTF-8 locale；

允许客户端访问 Mosh UDP 端口；

默认通常使用高位 UDP 端口范围；

防火墙和云安全组允许对应 UDP 流量。

示例服务端检查：

command -v mosh-server
mosh-server --version
locale

具体启动参数和连接输出格式必须以实际官方版本和源码为准。

21. 项目定位

WinMosh 追求的不是“Rust 写过一个网络 Demo”，而是：

一个可以真正日常使用的 Windows 系统工具；

一个与成熟 C++ 网络协议实现互操作的工程；

一个涵盖网络协议、状态同步、终端模拟、Windows API、密码学使用和可靠性测试的完整项目；

一个可以提交给真实开源社区审查的实现。

项目优先级：

正确性
  >
协议兼容
  >
安全
  >
可恢复性
  >
可测试性
  >
性能
  >
功能数量

22. 官方参考

执行器开始协议工作前必须阅读：

Mosh 官方网站：https://mosh.org/

Mosh 官方仓库：https://github.com/mobile-shell/mosh

Mosh 论文：https://mosh.org/mosh-paper.pdf

Windows OpenSSH 概览：https://learn.microsoft.com/windows-server/administration/openssh/openssh-overview

Windows OpenSSH 配置：https://learn.microsoft.com/windows-server/administration/openssh/openssh-server-configuration

Windows Terminal SSH 指南：https://learn.microsoft.com/windows/terminal/tutorials/ssh

协议实现时还应逐项阅读官方仓库中的：

scripts/
src/crypto/
src/network/
src/protobufs/
src/statesync/
src/terminal/
src/frontend/
