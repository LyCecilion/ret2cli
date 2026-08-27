# ret2cli 使用指南

`ret2cli` 是 Ret2Shell CTF 平台的终端客户端，既支持可脚本化的 one-line 子命令，也提供类似 Python 解释器的交互式命令提示符。两种方式使用同一套命令语法和执行逻辑。

## 构建

```bash
cd ~/Workspace/active/ret2cli
nix develop
cargo build --release
```

开发时可以用 `cargo run -- <参数>` 代替已安装的 `ret2cli`。

`ret2cli --version` 显示规范版本与发布线 codename，例如 `1.1.0 (Bloom in Two)`。正式 GitHub Actions 构建还会在版本后附加 `+build.<run_number>.<run_attempt>.g<short_sha>` 形式的构建元数据；本地构建只显示 Cargo.toml 中的规范版本。

## 交互式命令提示符

在终端中直接运行以下任意一条命令：

```bash
ret2cli
ret2cli interactive
```

启动时先显示 `figlet` 大字体渲染的 `Ret2CLI` 横幅，在支持颜色的终端上带 `lolcat` 式彩虹渐变（纯 Rust 实现，跨平台，不依赖系统的 figlet/lolcat；`NO_COLOR`、`TERM=dumb` 或 stdout 非终端时显示无颜色版本）。随后是 `账号@profile:比赛ID $` 形式的彩色动态提示符。未登录时账号为 `anonymous`，未选择比赛时比赛为 `none`。直接输入 one-line 命令时省略开头的 `ret2cli` 即可，也可以原样粘贴包含 `ret2cli` 的命令：

```text
 ____      _   ____   ____ _     ___ 
|  _ \ ___| |_|___ \ / ___| |   |_ _|
| |_) / _ \ __| __) | |   | |    | | 
|  _ <  __/ |_ / __/| |___| |___ | | 
|_| \_\___|\__|_____|\____|_____|___|
Ret2CLI 1.0.0 (LORELEI) interactive shell
Type "help" for commands, "context" for the active context, or "exit" to leave.
limityrochen@default:22 $ game list
limityrochen@default:22 $ game challenge show 'Pyjail 6'
limityrochen@default:22 $ game challenge submit 'Pyjail 6' --flag 'examplectf{...}'
limityrochen@default:22 $ ret2cli game submission list
limityrochen@default:22 $ exit
```

提示符分别对账号、profile 和比赛分段着色（设置 `NO_COLOR` 或使用 `TERM=dumb` 时自动关闭），并在每条命令完成后根据当前内存配置重新生成；因此 `profile use`、`account use/login/logout` 和 `game select` 会立即反映到下一行。比赛名称可能很长或包含空格，所以提示符只显示 ID。这里展示的是本地选中的上下文，不代表服务器已经验证 session，在线状态仍应使用 `account ping` 检查。

命令参数支持 shell 风格的单引号、双引号和反斜杠转义，但不会执行管道、重定向或其他 shell 语法。交互模式提供当前进程内的方向键历史与行编辑，不会把包含 flag、token 或密码的历史落盘。

交互模式下按 `Tab` 可补全命令与参数：

- 子命令、长选项（`--json`、`--game` 等）以及枚举值（如 `--pager auto`）静态补全，来源于与 one-line 完全相同的 clap 命令树；
- profile 与账号名补全自本地配置；比赛、题目、队伍同时提供 **数字 ID** 和 **名称** 两种候选（显示为 `ID 名称`），输入数字前缀则只收敛 ID 候选，输入名称前缀则自动为含空格的名称加引号（例如 `show pyja<Tab>` 插入 `"Pyjail 6"`）；
- 平台列表在会话内缓存并每 120 秒后台刷新，`account login/logout/use`、`profile add/use`、`game select` 等改变上下文的命令执行后会立即重新拉取；拉取失败时静默降级为仅静态补全。

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
ret2cli account login --url https://ctf.example/ --account limityrochen

# 选择比赛；选择结果保存在当前 profile 中
ret2cli game list
ret2cli game select 22

# 浏览和解题
ret2cli game challenge list
ret2cli game challenge show 'Pyjail 6'
ret2cli game challenge submit 'Pyjail 6' --flag 'examplectf{...}'
```

登录时省略 `--password` 会安全地隐藏输入。自动化环境可显式传入 `--password`，但应避免把密码留在 shell 历史中。

## 命令结构

### 账户

```bash
ret2cli account login --url https://ctf.example/ --account limityrochen
ret2cli account login --account limityrochen-alt
ret2cli account list
ret2cli account use limityrochen
ret2cli account ping
ret2cli account show
ret2cli account edit
ret2cli account edit --description '# About me' --yes
ret2cli account edit --description-file ./intro.md --avatar ./avatar.png --yes
ret2cli account edit --description-file - --remove-avatar --yes
ret2cli account code
ret2cli account logout
ret2cli account remove limityrochen-alt --yes

ret2cli account register --url https://ctf.example/ \
  --account limityrochen --nickname LimityroChen --email limityrochen@example.com
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

`profile list`、`profile show` 和交互式 `context` 会同时显示比赛 ID 与名称，例如 `22 (ExampleCTF 2025)`。邮箱来自最近一次使用当前持久化 session 执行 `account login` 或 `account show` 的本地缓存；通过 `--token`/`R2S_TOKEN` 临时认证时不会改写它。本项目尚未发布旧配置格式，因此不迁移原先的 `game = "22"`；若本地已有该格式，请删除该行后重新执行 `game select`。新格式为：

```toml
[profiles.default.game]
id = 22
name = "ExampleCTF 2025"
```

### 比赛

```bash
ret2cli game list
ret2cli game list --type training
ret2cli game list --page 2 --page-size 10
ret2cli game show 22
ret2cli game show 22 --intro
ret2cli game show 22 --rules
ret2cli game show 22 --cover
ret2cli game select 'ExampleCTF 2025'
ret2cli game scoreboard
```

比赛可以用数字 ID、完整名称或唯一的名称前缀指定。`game select` 会保存后端返回的规范 ID 与名称，不保留旧的 `game use` 别名。`game show` 会显示队伍人数上限（`team_size` 是上限而非必须人数，例如 `Team size: ≤4` 表示 1~4 人；`0` 显示 `unlimited`）和封面 hash。`--intro` 渲染比赛详细介绍文档（readme），`--rules` 渲染参赛规则，二者输出 Markdown；`--cover` 在 Kitty 中通过 `kitten icat` 的 stream 模式显示，在 iTerm2 中使用 OSC 1337。两者都把图片插入当前文本流，图片随文字滚动，后续提示符显示在图片下方；不会使用固定坐标放置。其他终端或缺少 `kitten` 时会提示图片的 `media?hash=` 地址。scoreboard 的 `Group` 列表示 Ret2Shell institute；未分组显示 `—`，JSON 同时保留 `institute_id` 并增加 `institute_name`。若 institute 映射请求失败，scoreboard 会明确失败，不会输出可能误导的空组名。

### 题目

```bash
ret2cli game challenge list
ret2cli game challenge show 'Pyjail 6'
ret2cli game challenge submit 'Pyjail 6' --flag 'examplectf{...}'
ret2cli game challenge hints 'Pyjail 6'
ret2cli game challenge unlock-hint 'Pyjail 6' --id 3
ret2cli game challenge instance start 'Pyjail 6'
ret2cli game challenge instance stop 'Pyjail 6'
ret2cli game challenge instance status 'Pyjail 6'
ret2cli game challenge instance renew 'Pyjail 6'
```

`instance start` / `stop` 在执行前先查询实例状态：实例已启动时 `start` 直接报告 already started 并返回成功（不触发 Ret2Shell 的 60 秒冷却）；实例未启动时 `stop` 报告 not running 而不谎报 stopped。`instance status` 显示实例的 pod 状态、剩余时间与续期次数（剩余时间 = 创建时间 + (续期次数 + 1) 小时）。`instance renew` 为运行中的实例续期 1 小时；未启动时提示 not running，超过续期上限时后端返回错误。

提交 flag 后，客户端会等待 Ret2Shell 的异步 checker 返回最终结果，而不是把刚创建的 pending submission 当成判题结果。

所有题目命令都可用 `--game <比赛>` 临时覆盖当前比赛：

```bash
ret2cli game challenge list --game 37
```

### 附件

```bash
# 查看后端实际提供的附件
ret2cli game challenge files 'Pyjail 6'

# 未指定 --file：全部下载到以题目名命名的目录
ret2cli game challenge download 'Pyjail 6'

# 单独下载，并可指定目标文件
ret2cli game challenge download 'Pyjail 6' --file src.zip
ret2cli game challenge download 'Pyjail 6' --file src.zip --output ./task.zip
```

客户端会分别下载 static/mapped 文件，不会把附件列表 JSON 冒充 ZIP 文件保存。未指定 `--file` 会下载全部附件；若要挑选多个附件，可分别执行多条 `game challenge download ... --file ...`。

### 队伍

```bash
ret2cli game team list
ret2cli game team show Team Name
ret2cli game team show mine
ret2cli game team create --name 'Team Name' --tag Hazelita --yes
ret2cli game team update --name 'New Name'
ret2cli game team join '<invitation-token>'
ret2cli game team leave
```

`game team create` 和 `game team join` 在交互模式下会先展示该比赛的参赛规则（`doc/rules`，若存在）并要求确认已阅读；`--yes` 跳过展示与确认（脚本或 JSON 模式必须显式传入）。`game team update` 会请求服务器改名（`PATCH /game/{id}/team/self`），并保留队伍现有的 tag 与 institute。`team_size` 是队伍人数上限而非必须人数：多人赛确认后直接修改；单人赛（`team_size = 1`）时服务器会强制队伍名跟随账号昵称，因此客户端会先提示该改名将被忽略，确认后仍发送请求。

`game team show` 会把一个或多个位置参数拼成队伍名，因此包含空格的名称无需引号也能查询；仍支持数字 ID、大小写不敏感的完整名称和唯一前缀。前缀不唯一时错误信息会列出候选队伍。`mine` 是 `show` 下保留的自身队伍目标；确实名为 `mine` 的队伍仍可通过数字 ID 访问。旧的 `game team mine` 路径不再保留。

脚本或 JSON 模式下退出队伍必须显式确认：

```bash
ret2cli --json game team leave --yes
```

### 提交记录

```bash
ret2cli game submission list
ret2cli game submission list --game 22
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

配置文件默认位于各平台的用户配置目录：

| 平台 | 路径 |
| --- | --- |
| Linux | `~/.config/ret2cli/config.toml`（`$XDG_CONFIG_HOME` 优先） |
| macOS | `~/Library/Application Support/ret2cli/config.toml` |
| Windows | `%APPDATA%\ret2cli\config.toml` |

完整配置示例：

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

[profiles.default.accounts.limityrochen-alt]
token = "<REDACTED>"
```

### UI 偏好

`[ui]` 段可选，所有字段缺省时保持内置默认。优先级为：命令行参数 > 环境变量 > 配置文件 > 内置默认。

```toml
[ui]
pager_mode = "always"   # auto | always | never，低于 --pager 参数
pager = "less -R -N"    # 分页程序，低于 $PAGER
editor = "hx"           # 编辑器，低于 $VISUAL/$EDITOR
```
