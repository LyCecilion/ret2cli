# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in Ret2CLI, please report it responsibly:

1. **Do not** open a public GitHub issue.
2. Email the maintainer or use [GitHub's private vulnerability reporting](https://github.com/LyCecilion/ret2cli/security/advisories/new).
3. Include a description of the vulnerability, steps to reproduce, and any suggested mitigations.
4. We will acknowledge receipt ASAP and provide a timeline for a fix.

## Security Architecture

### No Unsafe Code

Ret2CLI forbids `unsafe` code at the crate level:

```rust
#![forbid(unsafe_code)]
```

This guarantee is enforced by the compiler and verified in CI via `cargo clippy --all-targets -- -D warnings`.

### TLS / Transport Security

All network communication uses HTTPS exclusively, powered by `reqwest` with the `rustls-tls` feature (no system OpenSSL dependency). This provides:

- Modern TLS (1.2/1.3) with strong cipher suites.
- Certificate verification against the Mozilla root store.
- No dependency on the host system's OpenSSL, reducing supply-chain surface.

### Authentication

- Tokens are transmitted as `Bearer` tokens in the `Authorization` header.
- The server may issue token refreshes via the `Set-Token` response header; the client updates the local config accordingly.
- The `--url` CLI override intentionally does **not** carry the profile's stored token, preventing credential leakage when pointing at a different server instance.

### Local Storage Security

- Configuration is stored in the platform-standard user config directory (see [PRIVACY.md](./PRIVACY.md) for paths).
- Writes are atomic: content is written to a PID-tagged temp file, then renamed over the target, preventing partial-write corruption.
- An advisory file lock (`config.toml.lock`) serializes concurrent writers.
- **Note**: Tokens are stored in plaintext in the config file. Protect file permissions accordingly (the tool does not escalate permissions beyond the OS default for the config directory).

### REPL Safety

- Interactive mode (REPL) history is **never** persisted to disk, preventing accidental storage of flags, tokens, or sensitive command output.
- Interactive prompts (`confirm`, password input) are suppressed in `--json` mode and non-TTY environments, preventing blocking on stdin in automated pipelines.

## Supply Chain

| Measure | Tool |
| --------- | ------ |
| Dependency license audit | `cargo deny check licenses` |
| Known vulnerability check | `cargo audit` (advisory database) |
| Lint strictness | `cargo clippy --all-targets -- -D warnings` |
| Release artifact signing | `cargo-dist` with GitHub Actions attestations |
| SemVer & changelog | `release-plz` automated versioning |

## Supported Versions

Security fixes are applied to the latest release on the `main` branch. Older releases do not receive backports unless the vulnerability is critical and a backport is explicitly requested.

## Scope Boundaries

Ret2CLI will never implement features that could compromise CTF competition fairness, including but not limited to:

- Automated AI-powered flag solving.
- Flag brute-forcing or enumeration.
- Batch submission bypassing rate limits.
