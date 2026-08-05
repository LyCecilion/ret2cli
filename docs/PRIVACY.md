# Privacy Policy

## Overview

Ret2CLI is a terminal client for the Ret2Shell CTF platform. This document describes what data the tool stores, transmits, and never collects.

## Data Stored Locally

Ret2CLI stores a single configuration file on your machine:

| Platform | Path |
| ---------- | ------ |
| Linux | `~/.config/ret2cli/config.toml` |
| macOS | `~/Library/Application Support/ret2cli/config.toml` |
| Windows | `%APPDATA%\ret2cli\config.toml` |

The configuration file may contain:

- **API tokens** — Bearer tokens for your Ret2Shell account(s), stored in plaintext within the file.
- **Email addresses** — Optionally stored alongside tokens for account identification.
- **Server URLs** — The base URL(s) of the Ret2Shell instance(s) you connect to.
- **UI preferences** — Pager mode, pager program, and editor program.
- **Selected game** — The ID and name of your currently selected game.

## Data Transmitted to Servers

When you run a command, Ret2CLI sends the following to the configured Ret2Shell server:

- **HTTP requests** to the `/api/*` endpoints of the configured server URL.
- **Bearer token** in the `Authorization` header (only if a token is configured).
- **Command payloads** — challenge IDs, submission content, team actions, etc., as required by the specific command you invoke.

All communication uses HTTPS (TLS via `rustls`). No data is sent to any third-party service.

## What Is NOT Collected

- **No telemetry** — Ret2CLI does not send usage statistics, crash reports, or analytics to any server.
- **No tracking** — No cookies, no fingerprinting, no unique identifiers.
- **No REPL history on disk** — Interactive mode history is kept in memory only and is never written to a file. This prevents accidental leakage of flags or tokens typed in the REPL.

## Security Measures for Stored Data

- The configuration file is written atomically (write-to-temp then rename) with an advisory file lock to prevent concurrent corruption.
- The `--url` CLI override does **not** carry the profile's token, preventing accidental credential leakage when switching between server instances.
- `unsafe` code is forbidden at the compiler level (`#![forbid(unsafe_code)]`).

## Your Control

- **View your data**: Open the config file at the path listed above.
- **Delete your data**: Remove the config file. No other persistent data exists.
- **Revoke tokens**: Use the `ret2cli auth logout` command or revoke directly on the Ret2Shell web interface.

## Contact

For privacy concerns, open an issue on the [GitHub repository](https://github.com/ret2shell/ret2cli).
