use std::{
    env,
    io::{self, IsTerminal, Read},
    path::Path,
    time::Instant,
};

use dialoguer::Editor;
use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};

use crate::{
    cli::{AccountCodeArgs, AccountEditArgs, AccountRemoveArgs, LoginArgs, RegisterArgs},
    client::Client,
    commands::{confirm, require_or_input, require_or_password},
    config::ClientConfig,
    error::{CliError, CliResult},
    output,
};

#[derive(Debug, Serialize)]
struct LoginRequest {
    account: String,
    password: String,
    captcha_id: String,
    captcha_answer: String,
}

#[derive(Debug, Deserialize)]
struct CaptchaResponse {
    id: String,
    challenge: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct AccountProfile {
    id: i64,
    registered_at: i64,
    account: String,
    nickname: String,
    email: Option<String>,
    description: Option<String>,
    avatar: Option<String>,
    institute_id: Option<i64>,
    permissions: serde_json::Value,
    hidden: bool,
    banned: bool,
}

#[derive(Debug, Deserialize)]
struct MediaUpload {
    hash: String,
}

#[derive(Debug, Deserialize)]
struct TemporaryCode {
    code: u64,
    generate_at: i64,
}

pub async fn login(
    client: &mut Client,
    config: &mut ClientConfig,
    args: LoginArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let account = require_or_input(args.account, "Account", json)?;
    let password = require_or_password(args.password, "Password", json)?;

    // A valid old token makes Ret2Shell reject /login as an already-authenticated request.
    *client = Client::new(&client.base_url, None)?;
    client.set_token_persistence(false);
    bind_profile_url(config, profile_name, &client.base_url)?;

    let captcha: CaptchaResponse =
        client.get("account/captcha/cli", &[], config, profile_name).await?;
    let request = LoginRequest {
        account: account.clone(),
        password,
        captcha_id: captcha.id,
        captcha_answer: solve_pow(&captcha.challenge),
    };
    client.post_no_body("account/login", &request, config, profile_name).await?.ok_or_else(
        || CliError::Api {
            status: reqwest::StatusCode::OK,
            message: "server did not return Set-Token".to_owned(),
        },
    )?;
    let profile = client.get_value("account/profile", &[], config, profile_name).await?;
    let canonical_account =
        profile.get("account").and_then(|v| v.as_str()).unwrap_or(&account).to_owned();
    let nickname = profile.get("nickname").and_then(|v| v.as_str()).unwrap_or("unknown");
    let email = profile.get("email").and_then(|v| v.as_str()).map(str::to_owned);
    let token = client.token.clone().ok_or_else(|| {
        CliError::Config("login completed without an authentication token".to_owned())
    })?;
    config.active_profile_mut(profile_name)?.store_account(canonical_account.clone(), token, email);
    config.save()?;
    client.set_token_persistence(true);
    if json {
        output::print_json(&serde_json::json!({
            "account": canonical_account, "nickname": nickname, "logged_in": true,
        }));
    } else {
        output::success(&format!("Logged in as {nickname} ({canonical_account})"));
    }
    Ok(())
}

pub async fn logout(
    client: &mut Client,
    config: &mut ClientConfig,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let account =
        if client.persists_token() {
            Some(
                config.active_profile_resolved(profile_name)?.active_account.clone().ok_or_else(
                    || CliError::Config("no active account in this profile".to_owned()),
                )?,
            )
        } else {
            None
        };
    client.post_value("account/logout", &serde_json::json!({}), config, profile_name).await?;
    if account.is_some() {
        config.active_profile_mut(profile_name)?.clear_active_account();
        config.save()?;
    }
    if json {
        output::print_json(&serde_json::json!({
            "account": account, "logged_in": false, "local_session_removed": account.is_some(),
        }));
    } else if let Some(account) = account {
        output::success(&format!("Logged out and removed account '{account}'"));
    } else {
        output::success("Logged out explicit token; saved accounts were not changed");
    }
    Ok(())
}

pub async fn ping(
    client: &mut Client,
    config: &mut ClientConfig,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    if client.token.is_none() {
        return Err(CliError::Authentication("no active session to ping".to_owned()));
    }
    let started = Instant::now();
    let _: AccountProfile = client.get("account/profile", &[], config, profile_name).await?;
    let latency_ms = started.elapsed().as_millis();
    if json {
        output::print_json(&serde_json::json!({ "alive": true, "latency_ms": latency_ms }));
    } else {
        output::print_key_value(&[("Status", "Alive"), ("Latency", &format!("{latency_ms} ms"))]);
    }
    Ok(())
}

pub async fn register(
    client: &mut Client,
    config: &mut ClientConfig,
    args: RegisterArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let account = require_or_input(args.account, "Account", json)?;
    let nickname = require_or_input(args.nickname, "Nickname", json)?;
    let email = require_or_input(args.email, "Email", json)?;
    let password = require_or_password(args.password, "Password", json)?;
    *client = Client::new(&client.base_url, None)?;
    client.set_token_persistence(false);
    bind_profile_url(config, profile_name, &client.base_url)?;
    let captcha: CaptchaResponse =
        client.get("account/captcha/cli", &[], config, profile_name).await?;
    let result = client
        .post_value(
            "account/register",
            &serde_json::json!({
                "account": account, "nickname": nickname, "email": email, "password": password,
                "captcha_id": captcha.id, "captcha_answer": solve_pow(&captcha.challenge),
            }),
            config,
            profile_name,
        )
        .await?;
    config.save()?;
    if json {
        output::print_json(&result);
    } else {
        output::success("Registration successful; you can now log in");
    }
    Ok(())
}

pub fn list(config: &ClientConfig, profile_name: Option<&str>, json: bool) -> CliResult<()> {
    let profile = config.active_profile_resolved(profile_name)?;
    let mut accounts: Vec<_> = profile.accounts.keys().collect();
    accounts.sort();
    if json {
        let rows: Vec<_> = accounts
            .into_iter()
            .map(|account| {
                serde_json::json!({
                    "account": account,
                    "active": profile.active_account.as_ref() == Some(account),
                })
            })
            .collect();
        output::print_json(&rows);
    } else if accounts.is_empty() {
        output::line("No saved accounts.");
    } else {
        for account in accounts {
            let marker = if profile.active_account.as_ref() == Some(account) { "*" } else { " " };
            output::line(&format!("{marker} {account}"));
        }
    }
    Ok(())
}

pub fn use_account(
    config: &mut ClientConfig,
    profile_name: Option<&str>,
    account: &str,
    json: bool,
) -> CliResult<()> {
    let profile = config.active_profile_mut(profile_name)?;
    if !profile.accounts.contains_key(account) {
        return Err(CliError::Config(format!("account '{account}' is not saved in this profile")));
    }
    profile.active_account = Some(account.to_owned());
    config.save()?;
    if json {
        output::print_json(&serde_json::json!({ "active_account": account }));
    } else {
        output::success(&format!("Using account '{account}'"));
    }
    Ok(())
}

pub fn remove(
    config: &mut ClientConfig,
    profile_name: Option<&str>,
    args: &AccountRemoveArgs,
    json: bool,
) -> CliResult<()> {
    let profile = config.active_profile_resolved(profile_name)?;
    if !profile.accounts.contains_key(&args.account) {
        return Err(CliError::Config(format!(
            "account '{}' is not saved in this profile",
            args.account
        )));
    }
    if !confirm(&format!("Remove saved account '{}' locally?", args.account), args.yes, json)? {
        output::info("Aborted");
        return Ok(());
    }
    let profile = config.active_profile_mut(profile_name)?;
    profile.accounts.remove(&args.account);
    if profile.active_account.as_deref() == Some(&args.account) {
        profile.active_account = None;
    }
    config.save()?;
    if json {
        output::print_json(&serde_json::json!({ "removed": args.account }));
    } else {
        output::success(&format!("Removed saved account '{}'", args.account));
    }
    Ok(())
}

fn configured_editor<'a>(
    visual: Option<&'a str>,
    editor: Option<&'a str>,
    ui_editor: Option<&'a str>,
) -> Option<&'a str> {
    // Env overrides win over the config file; the editor itself falls back
    // to dialoguer's default (vi) when everything is unset.
    if visual.is_none() && editor.is_none() { ui_editor } else { None }
}

fn bind_profile_url(
    config: &mut ClientConfig,
    profile_name: Option<&str>,
    base_url: &str,
) -> CliResult<()> {
    let profile = config.active_profile_mut(profile_name)?;
    if profile.url.is_empty() {
        base_url.clone_into(&mut profile.url);
        config.save()?;
        return Ok(());
    }
    if profile.url.trim_end_matches('/') != base_url.trim_end_matches('/') {
        return Err(CliError::Config(format!(
            "URL '{base_url}' does not belong to the selected profile ({}); add or select another profile",
            profile.url
        )));
    }
    Ok(())
}

pub async fn show(
    client: &mut Client,
    config: &mut ClientConfig,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let profile: AccountProfile = client.get("account/profile", &[], config, profile_name).await?;
    if cache_active_account_email(config, profile_name, profile.email.as_deref())? {
        config.save()?;
    }
    if json {
        output::print_json(&profile);
        return Ok(());
    }
    let registered = chrono::DateTime::from_timestamp(profile.registered_at, 0).map_or_else(
        || profile.registered_at.to_string(),
        |date| date.format("%Y-%m-%d %H:%M UTC").to_string(),
    );
    let institute = profile.institute_id.map_or_else(|| "—".to_owned(), |value| value.to_string());
    output::print_key_value(&[
        ("Account", &profile.account),
        ("Nickname", &profile.nickname),
        ("Email", profile.email.as_deref().unwrap_or("—")),
        ("Avatar", profile.avatar.as_deref().unwrap_or("—")),
        ("Institute ID", &institute),
        ("Registered", &registered),
    ]);
    if let Some(description) = profile.description.as_deref().filter(|value| !value.is_empty()) {
        output::blank();
        output::print_markdown(description);
    }
    Ok(())
}

fn cache_active_account_email(
    config: &mut ClientConfig,
    profile_name: Option<&str>,
    email: Option<&str>,
) -> CliResult<bool> {
    let Some(account) = config.active_profile_resolved(profile_name)?.active_account.clone() else {
        return Ok(false);
    };
    let changed = {
        let profile = config.active_profile_mut(profile_name)?;
        if let Some(session) = profile.accounts.get_mut(&account)
            && session.email.as_deref() != email
        {
            session.email = email.map(str::to_owned);
            true
        } else {
            false
        }
    };
    Ok(changed)
}

pub async fn edit(
    client: &mut Client,
    config: &mut ClientConfig,
    args: AccountEditArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let explicit = args.has_explicit_change();
    if !explicit && (json || !io::stdin().is_terminal()) {
        return Err(CliError::Config(
            "account edit requires explicit data in JSON or non-TTY mode".to_owned(),
        ));
    }
    let mut profile: AccountProfile =
        client.get("account/profile", &[], config, profile_name).await?;
    let old_description = profile.description.clone();
    let old_avatar = profile.avatar.clone();

    let description = if let Some(value) = args.description.clone() {
        Some(value)
    } else if let Some(path) = args.description_file.as_deref() {
        Some(read_description(path)?)
    } else if !explicit {
        let mut editor = Editor::new();
        editor.extension(".md");
        if let Some(configured) = configured_editor(
            env::var_os("VISUAL").and_then(|value| value.into_string().ok()).as_deref(),
            env::var_os("EDITOR").and_then(|value| value.into_string().ok()).as_deref(),
            config.ui.editor.as_deref(),
        ) {
            editor.executable(configured);
        }
        let edited = editor
            .edit(profile.description.as_deref().unwrap_or(""))
            .map_err(|error| CliError::Io(io::Error::other(error)))?;
        let Some(edited) = edited else {
            output::info("Aborted");
            return Ok(());
        };
        Some(edited)
    } else {
        None
    };
    apply_profile_edits(&mut profile, description, args.remove_avatar);

    let description_changed = profile.description != old_description;
    let avatar_requested = args.avatar.is_some() || profile.avatar != old_avatar;
    if !description_changed && !avatar_requested {
        return Err(CliError::Config("no profile changes were provided".to_owned()));
    }

    if description_changed {
        output::info("Personal introduction preview:");
        if let Some(description) = profile.description.as_deref() {
            output::print_markdown(description);
        } else {
            output::line("(empty)");
        }
    }
    if let Some(path) = args.avatar.as_deref() {
        validate_avatar(path)?;
        output::info(&format!("Avatar: upload {}", path.display()));
    } else if args.remove_avatar {
        output::info("Avatar: remove current avatar");
    }
    if !confirm("Submit these profile changes?", args.yes, json)? {
        output::info("Aborted");
        return Ok(());
    }

    if let Some(path) = args.avatar.as_deref() {
        profile.avatar = Some(upload_avatar(client, config, profile_name, path).await?);
    }
    client.patch_no_body("account/profile", &profile, config, profile_name).await?;
    if json {
        output::print_json(&profile);
    } else {
        output::success("Profile updated");
    }
    Ok(())
}

pub async fn code(
    client: &mut Client,
    config: &mut ClientConfig,
    args: AccountCodeArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    if !confirm(
        "Generate a temporary identity code? Anyone with it can access your identity for 5 minutes.",
        args.yes,
        json,
    )? {
        output::info("Aborted");
        return Ok(());
    }
    let response: TemporaryCode = client.post("account/code", &(), config, profile_name).await?;
    let code = format_temporary_code(response.code);
    let expires_at = response.generate_at + 300;
    if json {
        output::print_json(&serde_json::json!({
            "code": code,
            "generate_at": response.generate_at,
            "expires_at": expires_at,
        }));
    } else {
        let expiry = chrono::DateTime::from_timestamp(expires_at, 0).map_or_else(
            || expires_at.to_string(),
            |date| date.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        );
        output::print_key_value(&[("Code", &code), ("Expires", &expiry)]);
    }
    Ok(())
}

fn apply_profile_edits(
    profile: &mut AccountProfile,
    description: Option<String>,
    remove_avatar: bool,
) {
    if let Some(description) = description {
        profile.description = (!description.is_empty()).then_some(description);
    }
    if remove_avatar {
        profile.avatar = None;
    }
}

fn format_temporary_code(code: u64) -> String {
    format!("{code:06X}")
}

fn read_description(path: &str) -> CliResult<String> {
    if path == "-" {
        let mut content = String::new();
        io::stdin().read_to_string(&mut content)?;
        Ok(content)
    } else {
        std::fs::read_to_string(path).map_err(CliError::Io)
    }
}

fn validate_avatar(path: &Path) -> CliResult<()> {
    const MAX_AVATAR_SIZE: u64 = 10 * 1024 * 1024;
    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(CliError::Config(format!("avatar is not a file: {}", path.display())));
    }
    if metadata.len() > MAX_AVATAR_SIZE {
        return Err(CliError::Config("avatar exceeds the server's 10 MiB limit".to_owned()));
    }
    Ok(())
}

async fn upload_avatar(
    client: &mut Client,
    config: &mut ClientConfig,
    profile_name: Option<&str>,
    path: &Path,
) -> CliResult<String> {
    let bytes = tokio::fs::read(path).await?;
    let file_name =
        path.file_name().and_then(|value| value.to_str()).unwrap_or("avatar").to_owned();
    let form = Form::new().part("file", Part::bytes(bytes).file_name(file_name));
    let upload: MediaUpload = client.post_multipart("media", form, config, profile_name).await?;
    Ok(upload.hash)
}

fn solve_pow(challenge: &str) -> String {
    use ring::digest::{SHA256, digest};
    let mut parts = challenge.splitn(2, '#');
    let difficulty = parts.next().and_then(|s| s.parse::<usize>().ok()).unwrap_or(4);
    let seed = parts.next().unwrap_or("");
    let prefix = "0".repeat(difficulty);
    for nonce in 0u64.. {
        let candidate = format!("{seed}{nonce:x}");
        if hex::encode(digest(&SHA256, candidate.as_bytes())).starts_with(&prefix) {
            return candidate;
        }
    }
    unreachable!()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn pow_answer_keeps_seed_and_meets_difficulty() {
        let answer = super::solve_pow("2#seed");
        let digest = ring::digest::digest(&ring::digest::SHA256, answer.as_bytes());
        assert!(answer.starts_with("seed"));
        assert!(hex::encode(digest).starts_with("00"));
    }

    #[test]
    fn configured_editor_prefers_env_over_config() {
        assert_eq!(configured_editor(Some("hx"), None, Some("vi")), None);
        assert_eq!(configured_editor(None, Some("nvim"), Some("vi")), None);
        assert_eq!(configured_editor(None, None, Some("hx")), Some("hx"));
        assert_eq!(configured_editor(None, None, None), None);
    }

    #[test]
    fn show_caches_active_account_email_locally() {
        let mut config = ClientConfig::default();
        let profile = config.active_profile_mut(None).unwrap();
        profile.store_account("alice".to_owned(), "token".to_owned(), None);
        profile.active_account = None;
        // Without an active account (e.g. a --token override) the local config is untouched.
        assert!(!cache_active_account_email(&mut config, None, Some("a@example.com")).unwrap());

        let profile = config.active_profile_mut(None).unwrap();
        profile.active_account = Some("alice".to_owned());
        // A changed email updates the cache and reports the change.
        assert!(cache_active_account_email(&mut config, None, Some("a@example.com")).unwrap());
        assert_eq!(
            config.active_profile_resolved(None).unwrap().accounts["alice"].email.as_deref(),
            Some("a@example.com")
        );
        // An unchanged email does not touch the config.
        assert!(!cache_active_account_email(&mut config, None, Some("a@example.com")).unwrap());
        // A missing server email clears the cached value.
        assert!(cache_active_account_email(&mut config, None, None).unwrap());
        assert_eq!(config.active_profile_resolved(None).unwrap().accounts["alice"].email, None);
    }

    #[test]
    fn profile_edits_preserve_protected_fields() {
        let mut profile: AccountProfile = serde_json::from_str(
            r#"{
                "id":7,"registered_at":1,"account":"alice","nickname":"Alice",
                "email":"alice@example.com","description":"old","avatar":"old-hash",
                "institute_id":3,"permissions":[0,1],"hidden":false,"banned":false
            }"#,
        )
        .unwrap();
        let protected = (
            profile.id,
            profile.registered_at,
            profile.account.clone(),
            profile.nickname.clone(),
            profile.email.clone(),
            profile.institute_id,
            profile.permissions.clone(),
            profile.hidden,
            profile.banned,
        );
        apply_profile_edits(&mut profile, Some("# New introduction".to_owned()), true);
        assert_eq!(profile.description.as_deref(), Some("# New introduction"));
        assert_eq!(profile.avatar, None);
        assert_eq!(
            protected,
            (
                profile.id,
                profile.registered_at,
                profile.account,
                profile.nickname,
                profile.email,
                profile.institute_id,
                profile.permissions,
                profile.hidden,
                profile.banned,
            )
        );
    }

    #[test]
    fn temporary_codes_are_uppercase_six_digit_hex() {
        assert_eq!(format_temporary_code(0), "000000");
        assert_eq!(format_temporary_code(0xAB_CDEF), "ABCDEF");
    }

    #[tokio::test]
    async fn ping_requires_a_session_without_contacting_the_server() {
        let mut client = Client::new("https://example.invalid", None).unwrap();
        let mut config = ClientConfig::default();
        let error = ping(&mut client, &mut config, true, None).await.unwrap_err();
        assert_eq!(error.exit_code(), 2);
    }

    #[tokio::test]
    async fn non_tty_edit_requires_explicit_changes() {
        let mut client = Client::new("https://example.invalid", Some("token".to_owned())).unwrap();
        let mut config = ClientConfig::default();
        let error = edit(&mut client, &mut config, AccountEditArgs::default(), true, None)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("explicit data"));
    }

    #[tokio::test]
    async fn ping_validates_the_session_with_profile_get() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let request = read_request(&mut socket).await;
            assert!(request.starts_with("GET /api/account/profile "));
            assert!(request.contains("authorization: Bearer token"));
            respond_json(
                &mut socket,
                r#"{"id":7,"registered_at":1,"account":"alice","nickname":"Alice","email":"alice@example.com","description":null,"avatar":null,"institute_id":null,"permissions":[0,1],"hidden":false,"banned":false}"#,
            )
            .await;
        });
        let mut config = ClientConfig::default();
        config.active_profile_mut(None).unwrap().url = format!("http://{address}");
        let mut client =
            Client::new(&format!("http://{address}"), Some("token".to_owned())).unwrap();
        ping(&mut client, &mut config, true, None).await.unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn temporary_code_uses_the_generation_endpoint() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let request = read_request(&mut socket).await;
            assert!(request.starts_with("POST /api/account/code "));
            respond_json(&mut socket, r#"{"code":11259375,"generate_at":1000}"#).await;
        });
        let mut config = ClientConfig::default();
        config.active_profile_mut(None).unwrap().url = format!("http://{address}");
        let mut client =
            Client::new(&format!("http://{address}"), Some("token".to_owned())).unwrap();
        code(&mut client, &mut config, AccountCodeArgs { yes: true }, true, None).await.unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn avatar_edit_uses_multipart_and_preserves_protected_profile_fields() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let profile = r#"{"id":7,"registered_at":1,"account":"alice","nickname":"Alice","email":"alice@example.com","description":"old","avatar":"old-hash","institute_id":3,"permissions":[0,1],"hidden":false,"banned":false}"#;

            let (mut socket, _) = listener.accept().await.unwrap();
            let request = read_request(&mut socket).await;
            assert!(request.starts_with("GET /api/account/profile "));
            respond_json(&mut socket, profile).await;

            let (mut socket, _) = listener.accept().await.unwrap();
            let request = read_request(&mut socket).await;
            assert!(request.starts_with("POST /api/media "));
            assert!(request.contains("multipart/form-data"));
            assert!(request.contains("name=\"file\""));
            assert!(request.contains("avatar-bytes"));
            respond_json(&mut socket, r#"{"id":1,"hash":"new-hash","uploader_id":7}"#).await;

            let (mut socket, _) = listener.accept().await.unwrap();
            let request = read_request(&mut socket).await;
            assert!(request.starts_with("PATCH /api/account/profile "));
            let body = request.split_once("\r\n\r\n").unwrap().1;
            let patched: serde_json::Value = serde_json::from_str(body).unwrap();
            assert_eq!(patched["account"], "alice");
            assert_eq!(patched["nickname"], "Alice");
            assert_eq!(patched["email"], "alice@example.com");
            assert_eq!(patched["institute_id"], 3);
            assert_eq!(patched["permissions"], serde_json::json!([0, 1]));
            assert_eq!(patched["description"], "# Updated");
            assert_eq!(patched["avatar"], "new-hash");
            respond_empty(&mut socket).await;
        });

        let avatar = std::env::temp_dir().join(format!(
            "ret2cli-avatar-{}-{}",
            std::process::id(),
            address.port()
        ));
        std::fs::write(&avatar, b"avatar-bytes").unwrap();
        let mut config = ClientConfig::default();
        config.active_profile_mut(None).unwrap().url = format!("http://{address}");
        let mut client =
            Client::new(&format!("http://{address}"), Some("token".to_owned())).unwrap();
        edit(
            &mut client,
            &mut config,
            AccountEditArgs {
                description: Some("# Updated".to_owned()),
                avatar: Some(avatar.clone()),
                yes: true,
                ..AccountEditArgs::default()
            },
            true,
            None,
        )
        .await
        .unwrap();
        server.await.unwrap();
        std::fs::remove_file(avatar).unwrap();
    }

    async fn read_request(socket: &mut tokio::net::TcpStream) -> String {
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 4096];
        let mut expected = None;
        loop {
            let read = socket.read(&mut buffer).await.unwrap();
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..read]);
            if expected.is_none()
                && let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n")
            {
                let headers = String::from_utf8_lossy(&bytes[..header_end]);
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .and_then(|value| value.parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                expected = Some(header_end + 4 + length);
            }
            if expected.is_some_and(|length| bytes.len() >= length) {
                break;
            }
        }
        String::from_utf8(bytes).unwrap()
    }

    async fn respond_json(socket: &mut tokio::net::TcpStream, body: &str) {
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(response.as_bytes()).await.unwrap();
    }

    async fn respond_empty(socket: &mut tokio::net::TcpStream) {
        socket
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\nconnection: close\r\n\r\n")
            .await
            .unwrap();
    }
}
