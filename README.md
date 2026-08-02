<!-- markdownlint-disable MD033 MD036 MD041 -->

<div align="center">

![Example Banner](./assets/banner.png)

# 🚩 Ret2CLI 🖥

CLI client for [Ret2Shell](https://github.com/ret2shell/ret2shell) CTF platform.

[![Typing SVG](https://readme-typing-svg.demolab.com?font=Cascadia+Code&duration=2000&pause=800&center=true&vCenter=true&width=850&lines=A+legend+from+long+ago+now+will+never+leave+my+head.;The+air+is+cool+and+night+is+coming%2C+as+the+calm+Rhine+gently+flows.;Up+high+on+a+ledge+sitting+is+a+maiden+most+marvelously+fair%2C;Combing+her+hair+with+a+golden+comb%2C+singing+as+well.;It+was+a+marvelous+and+spellbinding+melody.;The+boatman%2C+seized+by+wild+yearning+guides+his+small+raft+downstream.;His+eyes+not+at+the+rocky+ledge%2C+but+rather+high+up+into+the+sky%2C;The+waves+devour+the+boat+along+with+the+boatman+in+the+end.;And+this+by+her+song's+sheer+power;fair+Lorelei+has+done.)](https://git.io/typing-svg)

</div>

> [!WARNING]
>
> **免责声明 / DISCLAIMER**
>
> 1. 该项目的代码 **完全** 使用 AI 生成，文档亦有 AI 辅助构建。参与的 Agent 包含 GPT-5.6 Sol 和 DeepSeek V4 Flash 0731。我们使用严格的 Harness 和单元测试保证 AI 生成的代码尽可能符合我们的预期，对代码和文档进行了 review 并人工测试了几乎所有功能，但并不保证代码的完善。
> 2. 该项目与 Ret2Shell 实例的通信逻辑依照其官方文档和 Ret2Shell 源码。Ret2CLI 仅对官方 Ret2Shell 实例的通信进行了适配；若目标 Ret2Shell 实例安装了插件或进行了第三方配置，其行为可能发生变化，我们不对此情形做出功能性的担保。
> 3. Ret2CLI **不会官方实现** 任何可能破坏比赛公平性的功能，如自动接入 AI 工具解题、爆破 Flag 等。
>
> 该项目为 Project Hazelita 社群共创项目。
>

## 📖 About

Ret2CLI 是适用于 [Ret2Shell](https://github.com/ret2shell/ret2shell) CTF 平台的 CLI 客户端，目标定位类似于 `gh` 之于 GitHub：让选手在终端内完成从登录、选赛、看题、提交 Flag 到管理队伍的全部流程，无须打开浏览器。

Ret2CLI 采用与 Ret2Shell 相同的 Rust 技术栈，行为语义、均对照 Ret2Shell 服务端源码实现。

## ✨ Features

Ret2CLI 具有部分在浏览器中无法获得的使用体验，包括但不限于：

- **多 Profile / 多账号管理：** 每个 profile 独立保存服务器 URL、多个账号会话与当前比赛，方便在不同 Ret2Shell 实例与赛事间切换；账号登录支持 PoW 验证码与 token 自动轮换。
- **终端内完成赛事：** 查看比赛与排行榜、浏览题目、提交 Flag、下载附件、管理队伍与查看提交记录，全程无需离开终端。
- **交互式 REPL 和 JSON 输出：** 采用类解释器的命令提示符，与 one-line 子命令共用同一套语法；全局 `--json` 模式让 stdout 只输出一个 JSON 值，做到人类和脚本友好。

## 👀 Preview

下面的 GIF 录制于 Ret2CLI 的开发阶段。正式版和开发版可能有部分差异。

![Demo](./assets/demo.gif)

<!-- Generate with

```bash
agg --text-font-family "Maple Mono NF CN" --font-size 20 --line-height 1.2 --speed 1.2 --idle-time-limit 1 demo.cast demo.gif
```

-->

## 🚀 Quick Start

## 📦 Manual Installation

你可以手动安装 Ret2CLI。从 [Releases](https://github.com/LyCecilion/ret2cli/releases) 中取得目标操作系统的二进制文件后，可以直接使用或加入 PATH 后调用。

你也可以自行从源码编译。在你的设备上 [配置 Rust 开发环境](https://doc.rust-lang.org/book/ch01-01-installation.html) 后，使用 `cargo build` 编译：

```bash
git clone https://github.com/LyCecilion/ret2cli.git
cd ret2cli
cargo build --release
```

如果使用 NixOS 或 Determinate Nix，可以直接使用 `flake.nix` 提供的 Rust 开发环境。

```bash
git clone https://github.com/LyCecilion/ret2cli.git
cd ret2cli
nix develop
cargo build --release
target/release/ret2cli
```

编译后可以取得编译后的二进制文件 `./target/release/ret2cli` 或 `.\target\release\ret2cli.exe`。

## 📝 Usage

完整使用指南参见 [USAGE](./USAGE.md)。

## ⚙️ Configuration

配置文件位于 `~/.config/ret2cli/config.toml`。首次运行或文件不存在时，客户端使用一个空的 `default` profile，登录或添加 profile 时自动建档。

```toml
active_profile = "default"

[profiles.default]
url = "https://ctf.example/"
active_account = "limityrochen"

[profiles.default.game]
id = 22
name = "ExampleCTF 2025"

[profiles.default.accounts.limityrochen]
token = "<REDACTED>"
email = "<REDACTED>"

[ui]
pager_mode = "always"   # auto | always | never。将会被调用时的 `--pager` 参数覆盖。
pager = "less -R -N"    # 分页程序。将会被 `$PAGER` 覆盖。
editor = "vim"          # 编辑器。将会被 `$VISUAL/$EDITOR` 覆盖。
```

| 配置项 | 说明 | 覆盖优先级 |
| --- | --- | --- |
| `profiles.<name>.url` | 实例 API 基础地址 | `--url` / `R2S_URL` |
| `profiles.<name>.active_account` | 当前账号 | `account use` |
| `profiles.<name>.accounts.<name>.token` | 账号会话 token | `--token` / `R2S_TOKEN` |
| `profiles.<name>.game` | 当前比赛信息 | `game select` / `--game` |
| `ui.pager_mode` | 分页行为 | `--pager` |
| `ui.pager` | 分页程序 | `$PAGER` |
| `ui.editor` | `account edit` 调用的编辑器 | `$VISUAL` / `$EDITOR` |

对于同一个配置项，命令行参数优先于环境变量，环境变量优先于配置文件。在三者均缺省时采用默认值。

## 📁 Project Structure

```text
ret2cli/
├── src/
│   ├── main.rs          tokio 入口与错误输出
│   ├── lib.rs           调度中枢与 ID/名称解析
│   ├── cli.rs           clap 命令树与全局参数
│   ├── client.rs        HTTP 客户端（Bearer token、Set-Token 轮换、下载）
│   ├── config.rs        配置加载与原子写入（文件锁）
│   ├── error.rs         错误类型与退出码
│   ├── output.rs        输出缓冲、pager、表格、Markdown 渲染
│   └── commands/        auth / game / challenge / team / submission / interactive
├── assets/              banner 与演示录屏
├── USAGE.md             完整使用指南
├── CHANGELOG.md         变更日志
├── CONTRIBUTING.md      贡献指南
├── AGENTS.md            AI 助手项目说明
├── flake.nix            Nix 开发环境
├── deny.toml            依赖许可审查
└── LICENSE              MIT
```

## 💻 Development

可参考 [上文](#-manual-installation) 安装 Rust 1.89+ 工具链。

| 命令 | 说明 |
| --- | --- |
| `cargo build --release` | 构建发布二进制 |
| `cargo test` | 运行全部单元测试 |
| `cargo clippy --all-targets -- -D warnings` | 严格静态检查 |
| `cargo fmt --all --check` | 格式检查 |
| `cargo deny check licenses` | 依赖许可审查 |
| `cargo run -q -- <args>` | 在开发环境中运行 `ret2cli` |

## 🧪 Testing

测试全部为单元测试（`cargo test`），分布在各个模块的 `#[cfg(test)]` 中，覆盖：

- clap 命令树解析（含全部 one-line 工作流的可解析性）
- 配置读写与旧格式兼容（`[ui]` 段、并发写入快照完整性）
- 名称解析与歧义处理（game / challenge / team）
- 纯逻辑函数（pager 候选优先级、编辑器选择、team_size 语义、邮箱缓存四态）
- 网络路径通过 tokio 本地 mock 服务器验证关键请求

运行：

```bash
cargo test
```

## 📰 Changelog

参见 [CHANGELOG](./CHANGELOG.md)。

## 🤝 Contributing

参见 [CONTRIBUTING](./CONTRIBUTING.md)。

## 🙏 Acknowledgments

## 📄 License

本项目使用 [MIT LICENSE](./LICENSE)。
