# ret2cli 使用指南

`ret2cli` 是 Ret2Shell CTF 平台的终端客户端，既支持可脚本化的 one-line 子命令，也提供类似 Python 解释器的交互式命令提示符。两种方式使用同一套命令语法和执行逻辑。

## 构建

```bash
cd ~/Workspace/active/ret2cli
nix develop
cargo build --release
```

开发时可以用 `cargo run -- <参数>` 代替已安装的 `ret2cli`。

## 交互式命令提示符

在终端中直接运行以下任意一条命令：

```bash
ret2cli
ret2cli interactive
```

启动后会显示 `>>>` 提示符。直接输入 one-line 命令时省略开头的 `ret2cli` 即可，也可以原样粘贴包含 `ret2cli` 的命令：

```text
Ret2CLI 0.1.0 interactive shell
Type "help" for commands, "context" for the active context, or "exit" to leave.
profile=default  account=alice  game=11 (MoeCTF 2026)
>>> game list
>>> game challenge show phptrick
>>> game challenge submit phptrick --flag 'flag{...}'
>>> ret2cli game submission list
>>> exit
```

命令参数支持 shell 风格的单引号、双引号和反斜杠转义，但不会执行管道、重定向或其他 shell 语法。交互模式提供当前进程内的方向键历史与行编辑，不会把包含 flag、token 或密码的历史落盘。

交互内置命令：

- `help`：显示完整命令树；
- `help game challenge submit`：显示指定命令的帮助；
- `context`：显示当前 profile、账号和比赛；
- `exit`、`quit`、`exit()`、`quit()`：退出；
- `Ctrl-C`：取消当前输入并回到提示符；
- `Ctrl-D`：退出。

首次使用时，可在提示符内通过 `profile add <名称> --url <地址> --use-now` 建立 profile，或直接执行 `account login --url <地址> --account <账号>`，成功登录时 URL 会绑定到当前空 profile。

裸跑只在 TTY 中进入交互界面；管道或脚本中未指定子命令会直接失败，不会等待输入。

## 快速开始

```bash
# 设置默认服务器并登录
ret2cli account login --url https://ctf.xidian.edu.cn/ --account lycecilion

# 选择比赛；选择结果保存在当前 profile 中
ret2cli game list
ret2cli game use 11

# 浏览和解题
ret2cli game challenge list
ret2cli game challenge show phptrick
ret2cli game challenge submit phptrick --flag 'flag{...}'
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

`profile list`、`profile show` 和交互式 `context` 会同时显示比赛 ID 与名称，例如 `11 (MoeCTF 2026)`。本项目尚未发布旧配置格式，因此不迁移原先的 `game = "11"`；若本地已有该格式，请删除该行后重新执行 `game use`。新格式为：

```toml
[profiles.default.game]
id = 11
name = "MoeCTF 2026"
```

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
ret2cli game challenge list
ret2cli game challenge show phptrick
ret2cli game challenge submit phptrick --flag 'flag{...}'
ret2cli game challenge hints phptrick
ret2cli game challenge unlock-hint phptrick --id 3
ret2cli game challenge start phptrick
ret2cli game challenge stop phptrick
```

提交 flag 后，客户端会等待 Ret2Shell 的异步 checker 返回最终结果，而不是把刚创建的 pending submission 当成判题结果。

所有题目命令都可用 `--game <比赛>` 临时覆盖当前比赛：

```bash
ret2cli game challenge list --game 37
```

### 附件

```bash
# 查看后端实际提供的附件
ret2cli game challenge files phptrick

# 未指定 --file：全部下载到以题目名命名的目录
ret2cli game challenge download phptrick

# 单独下载，并可指定目标文件
ret2cli game challenge download phptrick --file attachment.zip
ret2cli game challenge download phptrick --file attachment.zip --output ./task.zip
```

客户端会分别下载 static/mapped 文件，不会把附件列表 JSON 冒充 ZIP 文件保存。未指定 `--file` 会下载全部附件；若要挑选多个附件，可分别执行多条 `game challenge download ... --file ...`。

### 队伍

```bash
ret2cli game team list
ret2cli game team show 'Team Name'
ret2cli game team mine
ret2cli game team create --name 'Team Name' --tag XDSEC
ret2cli game team join '<invitation-token>'
ret2cli game team leave
```

脚本或 JSON 模式下退出队伍必须显式确认：

```bash
ret2cli --json game team leave --yes
```

### 提交记录

```bash
ret2cli game submission list
ret2cli game submission list --game 11
```

## JSON 与自动化

全局 `--json` 让 stdout 只输出一个 JSON 值：

```bash
ret2cli --json game list | jq '.[].name'
ret2cli --json account show | jq .nickname
```

JSON 模式和非 TTY 环境绝不弹出输入提示。缺少必要参数时命令直接以非零状态退出；下载进度也不会混入 stdout。

可能产生较长的人类可读输出时，可以使用全局 `--pager auto|always|never`。默认 `auto` 只在 stdout 是终端且内容超过终端高度时分页；`always` 强制使用 `$PAGER`（不通过 shell 启动），`never` 始终直接输出。`$PAGER` 不可用时会依次尝试 `less -R`、系统 `more`，最终安全回退到 stdout。JSON、管道、重定向和补全不会自动分页。

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

## 配置

配置位于 `~/.config/ret2cli/config.toml`：

```toml
active_profile = "default"

[profiles.default]
url = "https://ctf.xidian.edu.cn/"
active_account = "lycecilion"

[profiles.default.game]
id = 11
name = "MoeCTF 2026"

[profiles.default.accounts.lycecilion]
token = "<redacted>"

[profiles.default.accounts.lycecilion-alt]
token = "<redacted>"
```
