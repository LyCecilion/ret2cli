# 安全策略

## 报告漏洞

如果您发现 Ret2CLI 的安全漏洞，请负责任地报告：

1. **请勿**在 GitHub 上公开提交 Issue。
2. 请通过邮件联系维护者，或使用 [GitHub 私密漏洞报告](https://github.com/ret2shell/ret2cli/security/advisories/new)。
3. 请包含漏洞描述、复现步骤以及建议的修复方案。
4. 我们将尽快确认收到，并提供修复时间线。

## 安全架构

### 禁止 Unsafe 代码

Ret2CLI 在 crate 级别禁止 `unsafe` 代码：

```rust
#![forbid(unsafe_code)]
```

该保证由编译器强制执行，并通过 CI 中的 `cargo clippy --all-targets -- -D warnings` 验证。

### TLS / 传输安全

所有网络通信仅使用 HTTPS，基于 `reqwest` 的 `rustls-tls` 特性（无系统 OpenSSL 依赖）。提供：

- 现代 TLS（1.2/1.3），支持强密码套件。
- 基于 Mozilla 根证书库的证书验证。
- 不依赖宿主系统的 OpenSSL，减少供应链攻击面。

### 身份认证

- 令牌以 `Bearer` Token 形式在 `Authorization` 请求头中传输。
- 服务器可通过 `Set-Token` 响应头刷新令牌；客户端会相应更新本地配置。
- `--url` 命令行覆盖有意**不**携带当前 profile 的令牌，防止指向不同服务器实例时泄露凭据。

### 本地存储安全

- 配置文件存储在平台标准的用户配置目录中（路径详见 [PRIVACY-zh.md](./PRIVACY-zh.md)）。
- 写入为原子操作：内容先写入带 PID 标记的临时文件，再重命名覆盖目标文件，防止部分写入导致的数据损坏。
- 使用 advisory 文件锁（`config.toml.lock`）序列化并发写入。
- **注意**：令牌以明文形式存储在配置文件中。请相应保护文件权限（工具不会在操作系统默认权限之外额外设置权限）。

### REPL 安全性

- 交互模式（REPL）历史记录**永不**持久化到磁盘，防止意外存储 flag、令牌或敏感命令输出。
- 交互提示（`confirm`、密码输入）在 `--json` 模式和非 TTY 环境下被抑制，防止在自动化管道中阻塞 stdin。

## 供应链安全

| 措施 | 工具 |
| ------ | ------ |
| 依赖许可证审计 | `cargo deny check licenses` |
| 已知漏洞检查 | `cargo audit`（advisory 数据库） |
| 严格 Lint | `cargo clippy --all-targets -- -D warnings` |
| 发布产物签名 | `cargo-dist` + GitHub Actions 签名证明 |
| 语义化版本 & 变更日志 | `release-plz` 自动化版本管理 |

## 受支持版本

安全修复应用于 `main` 分支上的最新版本。旧版本不会收到回溯修复，除非漏洞严重且被明确要求回溯。

## 功能边界

Ret2CLI 永远不会实现可能破坏 CTF 比赛公平性的功能，包括但不限于：

- 自动化 AI 解题。
- Flag 暴力破解或枚举。
- 绕过速率限制的批量提交。
