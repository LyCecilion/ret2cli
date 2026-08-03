# 🤝 参与贡献

感谢你愿意为 Ret2CLI 添砖加瓦。在向 Ret2CLI 贡献代码前，请阅读下面的贡献规范。

## 🌊 分支规范

Ret2CLI 使用 `main` 和 `develop` 两条长期分支。日常开发进入 `develop`；经过发布准备的 `release/*` 合入 `main`，发布 tag 只指向 `main` 上的稳定提交。

```text
main                    ← 稳定发布；vX.Y.Z tag 指向这里
├── hotfix/*            ← 已发布版本的紧急修复
└── develop             ← 默认分支与日常集成分支
    ├── feat/*          ← 新功能
    ├── fix/*           ← 缺陷修复
    ├── docs/*          ← 文档变更
    ├── refactor/*      ← 重构
    ├── chore/*         ← 工程维护
    └── release/*       ← 发布候选，从 develop 拉出、合入 main
```

`release-plz-*` 分支由发布自动化创建和维护。release-plz 固定向 GitHub 默认分支发起版本 PR，因此仓库默认分支保持为 `develop`；新增 `main` 后不要将默认分支切换过去。发布分支上的额外修订在发布后同步回 `develop`，避免两条长期分支漂移。

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
git checkout -b feat/<name>
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
git push origin feat/<name>
```

### 5. 发布流程

Ret2CLI 遵循 SemVer。`release-plz` 读取 Conventional Commits、维护版本 PR 并发布 crates.io（`release-plz.toml` 中 `changelog_update = false`，它不会改写 CHANGELOG.md）；CHANGELOG 由维护者以项目风格手工维护。`cargo-dist` 随后创建同版本的 tag、GitHub Release 和三端二进制附件。

首次配置仓库时，维护者需要：

1. 保持 GitHub 默认分支为 `develop`，因为 release-plz 固定向默认分支创建版本 PR；
2. 在 Actions 设置中启用 **Allow GitHub Actions to create and approve pull requests**；
3. 创建具备 crates.io `publish-new` 与 `publish-update` 权限的 API token，并保存为 Actions secret `CARGO_REGISTRY_TOKEN`。

日常发布流程如下：

1. 变更合入 `develop` 后，`.github/workflows/release-plz.yml` 创建或更新面向 `develop` 的版本 PR。该 PR 只 bump `Cargo.toml` / `Cargo.lock`，不触碰 CHANGELOG。
2. 首次提升 minor 或 major 时，必须先把新发布线注册到 `release.rs`（映射 + 测试）并更新 `dist-workspace.toml` 的展示名，否则 `build.rs` 会拒绝未知发布线；该改动需要先于版本 PR 合入 `develop`。
3. 维护者从版本 PR 的 head 提交切出 `release/vX.Y.Z`（版本 PR 暂不合入 `develop`），在发布分支上补写该版本的 CHANGELOG 章节（中文、手工条目，标题格式 `## [X.Y.Z] - YYYY-MM-DD - CODENAME`）并修复版本敏感的测试（如硬编码 codename 的断言），完成最终验证（`cargo test`、`cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo deny check licenses`；`dist plan` / `dist generate --check` 由 CI 的 `plan` 任务验证）后向 `main` 发起 PR。
4. 发布 PR 合入 `main` 后，release-plz 将 `X.Y.Z` 发布到 crates.io，计算对应的 `vX.Y.Z`，但不自行创建 tag 或 GitHub Release；随后通过 `workflow_dispatch` 调用 cargo-dist 工作流。
5. cargo-dist 从 `main` 为 Windows x86_64、Linux x86_64/AArch64、macOS Intel/Apple Silicon 构建压缩包与校验文件，随后创建 tag 和 GitHub Release。
6. 确认 tag 已创建后，**最后**把版本 PR 合入 `develop`，随后把 `main` 合入 `develop`，同步发布分支上的修订并保持两条长期分支的历史对齐。整个发布窗口内 `develop` 应保持冻结——任何推送都会让 release-plz rebase 版本 PR。发布完成后，release-plz 会在 `develop` 出现新提交时自动为**下一个版本**开启 PR（即使只有 docs 变更也会提议补丁版本，如 1.1.1）：该 PR 是下一次发布的起点，保持开启、不要关闭；准备发布时完成发布前准备后合入即可。

正常流程中不要手工推送版本 tag。若 crates.io 已发布但 cargo-dist 调度意外失败，维护者可从 `main` 手动 dispatch `Release` 工作流并填写同版本 tag；不得重复 bump 版本。若发布中途取消，把版本 PR 照常合入 `develop` 继续开发；在 tag 创建前 release-plz 会持续更新该版本 PR，属正常现象。已发布版本的紧急修复从 `main` 切出 `hotfix/*`，合入 `main` 发布后再同步到 `develop`。Winget、Scoop、Homebrew 等包管理器发布暂不属于本流程。

正式 CI 构建会在程序报告的 SemVer 后附加 `+build.<run_number>.<run_attempt>.g<short_sha>`；Cargo.toml 和 tag 仍只保存规范版本号，不提交构建元数据。

## 📋 Pull Request 之前

1. `cargo fmt --all --check`，确保格式化通过。
2. `cargo clippy --all-targets -- -D warnings`，确保无警告。
3. `cargo test`，确保全部测试通过。
4. `cargo deny check licenses`，确保依赖许可通过审查。
5. 新行为有对应的测试覆盖；纯内部重构不得引入测试缺口。
6. 基于 PR 的目标分支 rebase（日常变更为 `develop`，发布或 hotfix 为 `main`），解决冲突后再请求审阅。

## 🌟 注意事项

- 一个 Pull Request 应当仅包含一个逻辑变更，并确保其小而可审阅。
- PR 标题使用 [Conventional Commits](https://www.conventionalcommits.org/zh-hans) 格式，风格可参考现有提交历史（`type: :emoji: subject`）。
- 日常开发 PR 以 `develop` 为目标，发布和 hotfix PR 以 `main` 为目标。
- 修改公开行为（CLI 参数、输出格式、配置文件格式）时，同步更新 [USAGE.md](./USAGE.md)。
- 涉及服务端 API 的行为，请先对照 [Ret2Shell 源码](https://github.com/ret2shell/ret2shell) 确认语义，不要臆测。

## 💬 获取帮助

可以发送 Issues 或 Discussions，或在 Project Hazelita 社群中询问维护者。
