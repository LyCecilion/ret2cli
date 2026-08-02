# AGENTS.md

This file provides background and conventions for AI assistants working in this repository. All changes should follow this file and the conventions of the existing codebase.

## Project Overview

Ret2CLI is a Rust terminal client for the [Ret2Shell](https://github.com/ret2shell/ret2shell) CTF platform.

It offers scriptable one-line subcommands as well as an interpreter-style interactive REPL. Both entry points share the same clap command tree and execution logic.

## Toolchain and Environment

- Rust edition 2024, `rust-version = "1.89"`
- Key dependencies: clap 4 (derive), reqwest 0.12 (rustls), tokio, serde, tabled, termimad, rustyline, dialoguer, indicatif, ring

## Common Commands

```bash
cargo build --release        # build
cargo test                   # all unit tests
cargo clippy --all-targets -- -D warnings   # strict lint (no new warnings allowed)
cargo fmt --all --check      # formatting
cargo deny check licenses    # dependency licenses
cargo run -q -- <args>       # run (in development environment)
```

## Architecture

```text
src/
├── main.rs          tokio entry; prints errors by exit code (JSON mode emits {"error": ...})
├── lib.rs           dispatch: run → interactive or run_in_session → dispatch_network;
│                    resolve_game_id / resolve_challenge_id (numeric ID, exact name, unique prefix)
├── cli.rs           clap command tree + global flags (--json / --profile / --url / --token / --pager)
├── client.rs        reqwest wrapper: /api/{path}, Bearer token, Set-Token refresh, streaming download, download_bytes
├── config.rs        ~/.config/ret2cli/config.toml; atomic writes + file lock; [ui] section
├── error.rs         CliError → exit codes: 1 config/serialization, 2 unauthenticated, 3 forbidden, 4 not found, 5 network/server
├── output.rs        output capture + pager ($PAGER > [ui].pager > less -R > more), tabled tables, Markdown
└── commands/        auth / game / challenge / team / submission / interactive / local profile management
```

## Code Conventions

- **Commits & PRs**: follow [CONTRIBUTING.md](./CONTRIBUTING.md) — Conventional Commits for both commit messages and PR titles, one logical change per PR, target branch per Git Flow (feature → `develop`)
- **Errors**: use `CliError`; interactive prompts (`confirm` / `require_or_input` / `require_or_password`) never appear in JSON or non-TTY mode — missing arguments fail with a non-zero exit and require an explicit `--yes`
- **Precedence**: CLI flag > environment variable > config file > built-in default (e.g. pager mode, editor selection)
- **Output**: human-readable output goes through `output::` (buffered + paged); with `--json` stdout emits exactly one JSON value; download progress must not mix into stdout
- **Security constraints**: `unsafe_code = forbid`; `--url` overrides must not carry the profile's token (prevents credential leaks across instances); REPL history must never be written to disk (flag/token leaks)
- **Testing**: new behavior must have unit test coverage; prefer extracting pure functions for testability (e.g. `resolve_team_candidates`, `build_pager_candidates`); network paths are verified with tokio mocks or a local mock server

## Before touching API behavior

Verify behavior against the Ret2Shell documentation or source before changing API-dependent code; do not guess.

## Scope Boundaries

Never implement features that could break competition fairness (automated AI solving, flag brute-forcing, etc.)
