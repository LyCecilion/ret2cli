# ret2cli 使用指南

`ret2cli` 是 Ret2Shell CTF 平台的终端客户端，既支持可脚本化的子命令，也提供方向键操作的交互界面。

## 构建

```bash
cd ~/Workspace/active/ret2cli
nix develop
cargo build --release
```

开发时可以用 `cargo run -- <参数>` 代替已安装的 `ret2cli`。

## 交互界面

在终端中直接运行以下任意一条命令：

```bash
ret2cli
ret2cli interactive
```

首次启动会询问 Ret2Shell 地址。登录后可从菜单完成 profile 与账号切换、比赛选择、查看题目、提交 flag、Hint、实例、附件、队伍和提交记录等操作。

裸跑只在 TTY 中进入交互界面；管道或脚本中未指定子命令会直接失败，不会等待输入。

## 快速开始

```bash
# 设置默认服务器并登录
ret2cli account login --url https://ctf.xidian.edu.cn/ --account lycecilion

# 选择比赛；选择结果保存在当前 profile 中
ret2cli game list
ret2cli game use 11

# 浏览和解题
ret2cli challenge list
ret2cli challenge show phptrick
ret2cli challenge submit phptrick --flag 'flag{...}'
```

登录时省略 `--password` 会安全地隐藏输入。自动化环境可显式传入 `--password`，但应避免把密码留在 shell 历史中。

## 命令结构

### 账户

```bash
ret2cli account login --url https://ctf.example/ --account alice
ret2cli account login --account alice-alt
ret2cli account list
ret2cli account use alice
ret2cli account status
ret2cli account show
ret2cli account logout
ret2cli account remove alice-alt --yes

ret2cli account register --url https://ctf.example/ \
  --account alice --nickname Alice --email alice@example.com
```

同一个 connection profile 可以保存多个账号会话；每次登录都会保存并切换到该账号，`account use` 切换时不需要重新输入密码。`account logout` 会通知服务器并删除当前账号的本地会话，其他已保存账号不受影响；token 已失效、无法正常登出时可用 `account remove` 仅清理本地会话。

`account status` 会实际请求服务器验证 token。无效或过期的 token 不会被报告为“已登录”。

### 本地 profile

每个 profile 表示一个命名的 Ret2Shell 连接上下文，独立保存服务器 URL、多个账号会话和当前比赛：

```bash
ret2cli profile list
ret2cli profile add school --url https://ctf.school.example/ --use-now
ret2cli profile show school
ret2cli profile use default
ret2cli profile remove school
```

也可用全局参数只覆盖一次调用：

```bash
ret2cli --profile school game list
ret2cli --url https://temporary.example/ --token "$TOKEN" game list
```

未知 profile 会立即报错，不会静默退回 default。只覆盖 URL 时不会携带当前 profile 的 token，避免把一个 Ret2Shell 实例的凭据发送给另一个实例；临时认证必须同时显式提供 `--token`。

### 比赛

```bash
ret2cli game list
ret2cli game list --type training
ret2cli game list --page 2 --page-size 10
ret2cli game show 11
ret2cli game use 'MoeCTF 2026'
ret2cli game scoreboard
```

比赛可以用数字 ID、完整名称或唯一的名称前缀指定。

### 题目

```bash
ret2cli challenge list
ret2cli challenge show phptrick
ret2cli challenge submit phptrick --flag 'flag{...}'
ret2cli challenge hints phptrick
ret2cli challenge unlock-hint phptrick --id 3
ret2cli challenge start phptrick
ret2cli challenge stop phptrick
```

提交 flag 后，客户端会等待 Ret2Shell 的异步 checker 返回最终结果，而不是把刚创建的 pending submission 当成判题结果。

所有题目命令都可用 `--game <比赛>` 临时覆盖当前比赛：

```bash
ret2cli challenge list --game 37
```

### 附件

```bash
# 查看后端实际提供的附件
ret2cli challenge files phptrick

# 未指定 --file：全部下载到以题目名命名的目录
ret2cli challenge download phptrick

# 单独下载，并可指定目标文件
ret2cli challenge download phptrick --file attachment.zip
ret2cli challenge download phptrick --file attachment.zip --output ./task.zip
```

交互界面支持从附件列表中多选。客户端会分别下载 static/mapped 文件，不会把附件列表 JSON 冒充 ZIP 文件保存。

### 队伍

```bash
ret2cli team list
ret2cli team show 'Team Name'
ret2cli team mine
ret2cli team create --name 'Team Name' --tag XDSEC
ret2cli team join '<invitation-token>'
ret2cli team leave
```

脚本或 JSON 模式下退出队伍必须显式确认：

```bash
ret2cli --json team leave --yes
```

### 提交记录

```bash
ret2cli submission list
ret2cli submission list --game 11
```

## JSON 与自动化

全局 `--json` 让 stdout 只输出一个 JSON 值：

```bash
ret2cli --json game list | jq '.[].name'
ret2cli --json account show | jq .nickname
```

JSON 模式和非 TTY 环境绝不弹出输入提示。缺少必要参数时命令直接以非零状态退出；下载进度也不会混入 stdout。

常用退出码：

- `0`：成功；
- `1`：参数、配置、序列化、判题失败或服务端普通错误；
- `2`：未认证；
- `3`：无权限；
- `4`：资源不存在；
- `5`：网络或服务端错误。

## Shell 补全

```bash
source <(ret2cli completion bash)
source <(ret2cli completion zsh)
ret2cli completion fish | source
```

## 配置与旧版本迁移

配置位于 `~/.config/ret2cli/config.toml`：

```toml
active_profile = "default"

[profiles.default]
url = "https://ctf.xidian.edu.cn/"
active_account = "lycecilion"
game = "11"

[profiles.default.accounts.lycecilion]
token = "<redacted>"

[profiles.default.accounts.lycecilion-alt]
token = "<redacted>"
```

首次读取旧配置时，客户端会自动迁移 `[default]`、`[profiles.*]`、`default_game` 和 profile 下的单个 `token`。迁移前会保留一份 `config.toml.bak`；若已有旧备份则追加编号，已有 URL、token 和比赛选择不会丢失。旧 token 暂存为当前 profile 的 `legacy` 账号会话，下一次成功执行 `account status` 或 `account show` 后会自动改为服务端返回的真实账号名。
