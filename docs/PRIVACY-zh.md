# 隐私政策

## 概述

Ret2CLI 是 Ret2Shell CTF 平台的终端客户端。本文档说明该工具存储、传输以及**不**收集哪些数据。

## 本地存储的数据

Ret2CLI 仅在您的设备上存储一个配置文件：

| 平台 | 路径 |
| ------ | ------ |
| Linux | `~/.config/ret2cli/config.toml` |
| macOS | `~/Library/Application Support/ret2cli/config.toml` |
| Windows | `%APPDATA%\ret2cli\config.toml` |

配置文件可能包含：

- **API 令牌** — 您的 Ret2Shell 账户 Bearer Token，以明文形式存储在文件中。
- **电子邮箱** — 可选，与令牌一起存储用于账户标识。
- **服务器地址** — 您所连接的 Ret2Shell 实例的 URL。
- **界面偏好** — 分页模式、分页器程序、编辑器程序。
- **已选比赛** — 当前选中的比赛 ID 和名称。

## 传输至服务器的数据

当您执行命令时，Ret2CLI 会向配置的 Ret2Shell 服务器发送以下内容：

- 向配置服务器 URL 的 `/api/*` 端点发送 **HTTP 请求**。
- 在 `Authorization` 请求头中携带 **Bearer Token**（仅在已配置令牌时）。
- **命令数据** — 题目 ID、提交内容、战队操作等，取决于您执行的具体命令。

所有通信均使用 HTTPS（通过 `rustls` 实现 TLS）。不会向任何第三方服务发送数据。

## 不收集的数据

- **无遥测** — Ret2CLI 不会向任何服务器发送使用统计、崩溃报告或分析数据。
- **无追踪** — 不使用 Cookie、设备指纹或唯一标识符。
- **REPL 历史不写入磁盘** — 交互模式的历史记录仅保存在内存中，永不写入文件，防止意外泄露在 REPL 中输入的 flag 或令牌。

## 存储数据的安全措施

- 配置文件采用原子写入（先写临时文件再重命名），并使用文件 advisory lock 防止并发写入导致的数据损坏。
- `--url` 命令行覆盖**不会**携带当前 profile 的令牌，防止在切换服务器实例时意外泄露凭据。
- 编译器级别禁止 `unsafe` 代码（`#![forbid(unsafe_code)]`）。

## 您的控制权

- **查看数据**：打开上方列出的配置文件路径。
- **删除数据**：删除配置文件。不存在其他持久化数据。
- **撤销令牌**：使用 `ret2cli auth logout` 命令，或直接在 Ret2Shell 网页端撤销。

## 联系方式

如有隐私相关问题，请在 [GitHub Repo](https://github.com/ret2shell/ret2cli) 提交 Issue。
