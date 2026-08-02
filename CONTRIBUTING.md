# 🤝 参与贡献

感谢你愿意为 Ret2CLI 添砖加瓦。在向 Ret2CLI 贡献代码前，请阅读下面的贡献规范。

## 🌊 分支规范

Ret2CLI 使用 [Git Flow](https://nvie.com/posts/a-successful-git-branching-model/)，包含两条长期分支，加上用于功能和发布的短期分支。

```text
main                    ← 生产就绪。带版本 tag。
  └── develop           ← 集成分支。所有功能在此汇合。
        ├── feature/*   ← 每个功能一个分支。从 develop 拉出。
        └── release/*   ← 发布候选。从 develop 拉出。
  └── hotfix/*          ← 紧急修复。从 main 拉出。
```

| 分支             | 从哪拉    | 合到哪             | 用途                                            |
| ---------------- | --------- | ------------------ | ----------------------------------------------- |
| `main`           | —         | —                  | 仅稳定发布。从 `release/*` 或 `hotfix/*` 合入。 |
| `develop`        | `main`    | —                  | 集成分支。所有功能先合到这里。                  |
| `feature/<name>` | `develop` | `develop`          | 一个功能或一个修复一个分支。                    |
| `release/vX.Y.Z` | `develop` | `main` + `develop` | 发布前冻结：更新版本号、完善 CHANGELOG.md。     |
| `hotfix/<name>`  | `main`    | `main` + `develop` | 线上紧急修复。                                  |

## 🚀 提交流程

### 1. Fork 并 Clone

```bash
git clone git@github.com:<username>/ret2cli.git
cd ret2cli
git remote add upstream git@github.com:LyCecilion/ret2cli.git
```

### 2. 创建功能分支

始终从 `develop` 拉分支：

```bash
git checkout develop
git pull upstream develop
git checkout -b feature/<name>
```

### 3. 开发并提交

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo deny check licenses

git add -A
git commit -m "<conventional message>"

git pull --rebase upstream develop
```

### 4. 发起 Pull Request

推送后向 `upstream/develop` 发起 PR：

```bash
git push origin feature/<name>
```

### 5. 发布流程

由维护者从 `develop` 切出发布分支：

```bash
git checkout develop
git pull upstream develop
git checkout -b release/vX.Y.Z

# 更新版本号、完善 CHANGELOG.md
git add -A
git commit -m "chore: release vX.Y.Z"

git push upstream release/vX.Y.Z
```

合入 `main` 后，发布分支合回 `develop` 保持同步，并打 tag：

```bash
git checkout main
git pull upstream main
git tag -a vX.Y.Z -m "Ret2CLI vX.Y.Z"
git push upstream --tags

git checkout develop
git pull upstream develop
git merge release/vX.Y.Z
git push upstream develop

git branch -d release/vX.Y.Z
git push upstream --delete release/vX.Y.Z
```

### 6. 紧急修复

`main` 上的紧急 bug 直接从 `main` 拉 `hotfix/<name>`，修复合入 `main` 后同样合回 `develop`。

## 📋 Pull Request 之前

1. `cargo fmt --all --check`，确保格式化通过。
2. `cargo clippy --all-targets -- -D warnings`，确保无警告。
3. `cargo test`，确保全部测试通过。
4. `cargo deny check licenses`，确保依赖许可通过审查。
5. 新行为有对应的测试覆盖；纯内部重构不得引入测试缺口。
6. 基于目标分支 rebase（功能 rebase 到 `develop`，修复 rebase 到 `main`）。

## 🌟 注意事项

- 一个 Pull Request 应当仅包含一个逻辑变更，并确保其小而可审阅。
- PR 标题使用 [Conventional Commits](https://www.conventionalcommits.org/zh-hans) 格式，风格可参考现有提交历史（`type: :emoji: subject`）。
- 将 feature 合入 `develop`、release 合入 `main`、hotfix 合入 `main`。
- 修改公开行为（CLI 参数、输出格式、配置文件格式）时，同步更新 [USAGE.md](./USAGE.md)。
- 涉及服务端 API 的行为，请先对照 [Ret2Shell 源码](https://github.com/ret2shell/ret2shell) 确认语义，不要臆测。

## 💬 获取帮助

可以发送 Issues 或 Discussions，或在 Project Hazelita 社群中询问维护者。
