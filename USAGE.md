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

启动后会显示 `账号@profile:比赛ID $` 形式的彩色动态提示符。未登录时账号为 `anonymous`，未选择比赛时比赛为 `none`。直接输入 one-line 命令时省略开头的 `ret2cli` 即可，也可以原样粘贴包含 `ret2cli` 的命令：

```text
Ret2CLI 0.1.0 interactive shell
Type "help" for commands, "context" for the active context, or "exit" to leave.
alice@default:11 $ game list
alice@default:11 $ game challenge show phptrick
alice@default:11 $ game challenge submit phptrick --flag 'flag{...}'
alice@default:11 $ ret2cli game submission list
alice@default:11 $ exit
```

提示符分别对账号、profile 和比赛分段着色（设置 `NO_COLOR` 或使用 `TERM=dumb` 时自动关闭），并在每条命令完成后根据当前内存配置重新生成；因此 `profile use`、`account use/login/logout` 和 `game select` 会立即反映到下一行。比赛名称可能很长或包含空格，所以提示符只显示 ID。这里展示的是本地选中的上下文，不代表服务器已经验证 session，在线状态仍应使用 `account ping` 检查。

命令参数支持 shell 风格的单引号、双引号和反斜杠转义，但不会执行管道、重定向或其他 shell 语法。交互模式提供当前进程内的方向键历史与行编辑，不会把包含 flag、token 或密码的历史落盘。

交互内置命令：

- `help`：显示完整命令树；
- `help game challenge submit`：显示指定命令的帮助；
- `context`：分三行显示当前上下文——profile（含 URL）、账号（含本地缓存的邮箱）和比赛（完整 ID 与名称）；字段按终端显示宽度对齐，支持中文等宽字符；
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
ret2cli game select 11

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
ret2cli account ping
ret2cli account show
ret2cli account edit
ret2cli account edit --description '# About me' --yes
ret2cli account edit --description-file ./intro.md --avatar ./avatar.png --yes
ret2cli account edit --description-file - --remove-avatar --yes
ret2cli account code
ret2cli account logout
ret2cli account remove alice-alt --yes

ret2cli account register --url https://ctf.example/ \
  --account alice --nickname Alice --email alice@example.com
```

同一个 connection profile 可以保存多个账号会话；每次登录都会保存并切换到该账号，`account use` 切换时不需要重新输入密码。`account logout` 会通知服务器并删除当前账号的本地会话，其他已保存账号不受影响；token 已失效、无法正常登出时可用 `account remove` 仅清理本地会话。

`account ping` 只请求服务器验证当前 session 是否仍然存活，并显示往返延迟；它不会重复输出个人资料。缺少 session 或 token 无效时会以认证失败退出。

`account show` 会显示头像 hash，并在终端中渲染 Markdown 格式的 Personal introduction。`account edit` 不带参数时会通过 `$VISUAL` 或 `$EDITOR`（都未设置时用 `[ui].editor`，最后回退 vi）打开多行 Markdown 编辑器，随后在 CLI 中渲染预览并确认；也可用 `--description`、`--description-file <PATH|->`、`--avatar PATH` 或 `--remove-avatar` 完成 one-line 修改。头像上传限制为 10 MiB。JSON 或非 TTY 模式必须显式提供修改内容并传入 `--yes`。客户端提交完整后端 profile 时只改变 description/avatar，并保留昵称、邮箱、权限等字段；它不会提供邮箱、密码、第三方验证服务或删除账号的修改入口。

`account code` 会在确认敏感性后生成六位大写十六进制临时身份验证码，有效期为五分钟。JSON 或非 TTY 模式必须传入 `--yes`。

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

`profile list`、`profile show` 和交互式 `context` 会同时显示比赛 ID 与名称，例如 `11 (MoeCTF 2026)`。邮箱来自最近一次使用当前持久化 session 执行 `account login` 或 `account show` 的本地缓存；通过 `--token`/`R2S_TOKEN` 临时认证时不会改写它。本项目尚未发布旧配置格式，因此不迁移原先的 `game = "11"`；若本地已有该格式，请删除该行后重新执行 `game select`。新格式为：

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
ret2cli game select 'MoeCTF 2026'
ret2cli game scoreboard
```

比赛可以用数字 ID、完整名称或唯一的名称前缀指定。`game select` 会保存后端返回的规范 ID 与名称，不保留旧的 `game use` 别名。`game show` 会显示队伍人数上限（`team_size` 是上限而非必须人数，例如 `Team size: ≤4` 表示 1~4 人；`0` 显示 `unlimited`）。scoreboard 的 `Group` 列表示 Ret2Shell institute；未分组显示 `—`，JSON 同时保留 `institute_id` 并增加 `institute_name`。若 institute 映射请求失败，scoreboard 会明确失败，不会输出可能误导的空组名。

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
ret2cli game team show Team Name
ret2cli game team show mine
ret2cli game team create --name 'Team Name' --tag XDSEC
ret2cli game team update --name 'New Name'
ret2cli game team join '<invitation-token>'
ret2cli game team leave
```

`game team update` 会请求服务器改名（`PATCH /game/{id}/team/self`）。`team_size` 是队伍人数上限而非必须人数：多人赛确认后直接修改；单人赛（`team_size = 1`）时服务器会强制队伍名跟随账号昵称，因此客户端会先提示该改名将被忽略，确认后仍发送请求。

`game team show` 会把一个或多个位置参数拼成队伍名，因此包含空格的名称无需引号也能查询；仍支持数字 ID、大小写不敏感的完整名称和唯一前缀。前缀不唯一时错误信息会列出候选队伍。`mine` 是 `show` 下保留的自身队伍目标；确实名为 `mine` 的队伍仍可通过数字 ID 访问。旧的 `game team mine` 路径不再保留。

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

可能产生较长的人类可读输出时，可以使用全局 `--pager auto|always|never`，它优先于配置文件 `[ui].pager_mode`（默认 `auto`）。启动交互模式时指定的 `--pager` 会作为该 REPL 会话的默认值，REPL 内单条命令仍可再次覆盖。`auto` 只在 stdout 是终端且内容超过终端高度时分页；`always` 强制分页，`never` 始终直接输出。分页程序按 `$PAGER`、`[ui].pager`、`less -R`、系统 `more` 的顺序尝试（`$PAGER` 与 `[ui].pager` 均不通过 shell 启动），全部失败时安全回退到 stdout。JSON、管道、重定向和补全不会自动分页。

常用退出码：

- `0`：成功；
- `1`：参数、配置、序列化、判题失败或服务端普通错误；
- `2`：未认证；
- `3`：无权限；
- `4`：资源不存在；
- `5`：网络或服务端错误。

## Shell 补全

```bash
# 非 TTY stdout 会直接输出，保留 source <(...) 用法
source <(ret2cli completion bash)
source <(ret2cli completion zsh)
ret2cli completion fish | source

# 安全导出到文件；已有文件默认拒绝覆盖
ret2cli completion bash --output ~/.local/share/bash-completion/completions/ret2cli
ret2cli completion zsh --output ~/.zfunc/_ret2cli --force
```

不指定 `--output` 且 stdout 是终端时，客户端先显示脚本的行数和字节数，并询问是否展开；`--yes` 可跳过确认。管道或重定向时会直接输出完整脚本，不额外插入提示文本。completion 始终绕过 pager，且拒绝与 `--json` 同时使用。

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
email = "<redacted>"

[profiles.default.accounts.lycecilion-alt]
token = "<redacted>"
```

### UI 偏好

`[ui]` 段可选，所有字段缺省时保持内置默认。优先级为：命令行参数 > 环境变量 > 配置文件 > 内置默认。

```toml
[ui]
pager_mode = "always"  # auto | always | never，低于 --pager 参数
pager = "less -R -N"    # 分页程序，低于 $PAGER
editor = "hx"           # 编辑器，低于 $VISUAL/$EDITOR
```
