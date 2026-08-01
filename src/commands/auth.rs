use serde::{Deserialize, Serialize};

use crate::{
    cli::{AccountRemoveArgs, LoginArgs, RegisterArgs},
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
    let token = client.token.clone().ok_or_else(|| {
        CliError::Config("login completed without an authentication token".to_owned())
    })?;
    config.active_profile_mut(profile_name)?.store_account(canonical_account.clone(), token);
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

pub async fn status(
    client: &mut Client,
    config: &mut ClientConfig,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    if client.token.is_none() {
        if json {
            output::print_json(&serde_json::json!({ "logged_in": false, "url": client.base_url }));
        } else {
            output::print_key_value(&[("URL", &client.base_url), ("Status", "Not logged in")]);
        }
        return Ok(());
    }
    let profile = client.get_value("account/profile", &[], config, profile_name).await?;
    let nickname = profile.get("nickname").and_then(|v| v.as_str()).unwrap_or("—");
    let account = profile.get("account").and_then(|v| v.as_str()).unwrap_or("—");
    if json {
        output::print_json(&serde_json::json!({
            "logged_in": true, "url": client.base_url, "account": account, "nickname": nickname,
        }));
    } else {
        output::print_key_value(&[
            ("URL", &client.base_url),
            ("Status", "Logged in"),
            ("Account", account),
            ("Nickname", nickname),
        ]);
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
    let value = client.get_value("account/profile", &[], config, profile_name).await?;
    if json {
        output::print_json(&value);
        return Ok(());
    }
    let registered = value.get("registered_at").and_then(serde_json::Value::as_i64).map_or_else(
        || "—".to_owned(),
        |v| {
            chrono::DateTime::from_timestamp(v, 0)
                .map_or_else(|| v.to_string(), |d| d.format("%Y-%m-%d %H:%M UTC").to_string())
        },
    );
    let institute = value
        .get("institute_id")
        .and_then(serde_json::Value::as_i64)
        .map_or_else(|| "—".to_owned(), |v| v.to_string());
    output::print_key_value(&[
        ("Account", value.get("account").and_then(|v| v.as_str()).unwrap_or("—")),
        ("Nickname", value.get("nickname").and_then(|v| v.as_str()).unwrap_or("—")),
        ("Email", value.get("email").and_then(|v| v.as_str()).unwrap_or("—")),
        ("Institute ID", &institute),
        ("Registered", &registered),
    ]);
    Ok(())
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
mod tests {
    #[test]
    fn pow_answer_keeps_seed_and_meets_difficulty() {
        let answer = super::solve_pow("2#seed");
        let digest = ring::digest::digest(&ring::digest::SHA256, answer.as_bytes());
        assert!(answer.starts_with("seed"));
        assert!(hex::encode(digest).starts_with("00"));
    }
}
