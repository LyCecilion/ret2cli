<!-- markdownlint-disable MD024 -->

# 📰 CHANGELOG

Ret2CLI 的所有重要变更将记录在此文件中。格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)。

本项目遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

---

## [Unreleased]

### Added

- 交互模式启动横幅：`figlet` 大字体渲染 `Ret2CLI` 并带 `lolcat` 式彩虹渐变（纯 Rust 实现，跨平台，不依赖系统 figlet/lolcat 或外部字体；设置 `NO_COLOR` 或 `TERM=dumb`、stdout 非终端时显示无颜色版本）
- 在 Windows 构建中加入了资源头，包含 icon、名称、公司名称、注释、版权信息等

### Removed

- 移除弃用的 `game challenge start` / `stop` / `status` / `renew` 扁平子命令，请使用 `game challenge instance <action>`

## [1.1.0] - 2026-08-03 - Bloom in Two

### Added

- `game challenge instance` 子命令组：`instance start` / `stop` / `status` / `renew`
- `game challenge status`：查询实例状态（pod 状态、剩余时间、续期次数）
- `game challenge renew`：为运行中的实例续期 1 小时

### Deprecated

- `game challenge start` / `stop` / `status` / `renew` 弃用，迁移到 `game challenge instance <action>`；旧用法将在下一个 major 版本移除

### Fixed

- `game challenge start` 在实例已启动时直接报告 already started 并成功返回，不再撞上 Ret2Shell 的 60 秒重建冷却（412）
- `game challenge stop` 在实例未启动时报告 not running，不再谎报 Instance stopped

## [1.0.0] - 2026-08-02 - LORELEI

### Added

- 交互式命令提示符（REPL）：与 one-line 子命令共用同一套命令语法，支持方向键历史、行编辑、彩色上下文提示符和内置 `help` / `context` 命令；历史不落盘
- 多 Profile 与多账号管理：每个 profile 独立保存服务器 URL、多个账号会话和当前比赛；`--profile` / `--url` / `--token` 全局覆盖，URL 覆盖时不会携带当前 profile 的 token
- 账号命令：登录（含 PoW 验证码）、注册、登出、`ping`、`show`、`edit`（Markdown 简介 + 头像）、临时身份验证码、会话切换与本地移除
- 比赛命令：列表、详情、选择、排行榜（含 institute 分组）
- 题目命令：列表、详情、提交（等待异步 checker 最终结果）、提示、解锁提示、实例启停、附件列表与下载（区分 static / mapped）
- 队伍命令：列表、详情（排名、成员、解题数）、创建、改名、加入、退出；名称支持数字 ID / 完整名称 / 唯一前缀
- 提交记录查询
- Shell 补全生成（bash / zsh / fish），支持安全导出到文件
- 全局 `--json` 模式：stdout 只输出一个 JSON 值，JSON 与非 TTY 环境绝不弹出交互提示
- 分页输出：`--pager auto|always|never`，按 `$PAGER`、`[ui].pager`、`less -R`、`more` 顺序尝试分页程序
- 配置文件 `[ui]` 段：`pager_mode`、`pager`、`editor`，优先级为命令行参数 > 环境变量 > 配置文件 > 内置默认
- `game show --intro` / `--rules`：渲染比赛详细介绍与参赛规则文档（Markdown）
- `game show --cover`：通过 `kitten icat` 在 kitty 终端内联显示比赛宣传图
- `game team create` / `join` 在创建或加入队伍前展示参赛规则并要求确认（`--yes` 跳过）
- 语义化退出码：1 参数/配置/判题失败，2 未认证，3 无权限，4 资源不存在，5 网络或服务端错误
- `deny.toml` 依赖许可审查配置
- SemVer 发布自动化：release-plz 维护版本 PR 并发布 crates.io，cargo-dist 为 Windows、Linux 与 macOS 创建对应的 GitHub Release 附件
- v1.0.x 发布线 codename `LORELEI`，正式 CI 二进制附带 GitHub run、attempt 与 commit 构成的 SemVer build metadata

### Fixed

- REPL 会话继承启动时的 `--pager` 设置，且单条命令的显式 `--pager` 优先级更高
- 账号邮箱缓存仅在持久会话且服务器账号与本地账号一致时写入，`--token` / `R2S_TOKEN` 临时认证不再改写本地元数据
- `context` 按终端显示宽度对齐，支持中文等宽字符
- `game team update` 保留队伍现有 tag 与 institute，不再因改名请求而清空
- 并发配置写入通过文件锁与 PID 隔离的临时文件串行化，避免互相覆盖
- `team show` 的排名/成员/解题数据请求失败时明确报错，不再静默显示空数据
- `interactive` 在 REPL 内输入时给出友好提示而非报错
