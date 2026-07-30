use serde::{Deserialize, Serialize};

use crate::{
    cli::{LoginArgs, RegisterArgs},
    client::Client,
    commands::{require_or_input, require_or_password},
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
    *client = Client::new(client.base_url.clone(), None)?;
    config.active_profile_mut(profile_name)?.url = client.base_url.clone();

    let captcha: CaptchaResponse =
        client.get("account/captcha/cli", &[], config, profile_name).await?;
    let request = LoginRequest {
        account,
        password,
        captcha_id: captcha.id,
        captcha_answer: solve_pow(&captcha.challenge),
    };
    let token = client
        .post_no_body("account/login", &request, config, profile_name)
        .await?
        .ok_or_else(|| CliError::Api {
            status: reqwest::StatusCode::OK,
            message: "server did not return Set-Token".to_owned(),
        })?;
    config.save()?;
    let profile = client.get_value("account/profile", &[], config, profile_name).await?;
    let nickname = profile.get("nickname").and_then(|v| v.as_str()).unwrap_or("unknown");
    if json {
        output::print_json(&serde_json::json!({ "nickname": nickname, "logged_in": true }));
    } else {
        output::success(&format!("Logged in as {nickname}"));
    }
    // Keep token owned by config/client; never print it.
    drop(token);
    Ok(())
}

pub async fn logout(
    client: &mut Client,
    config: &mut ClientConfig,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    client.post_value("account/logout", &serde_json::json!({}), config, profile_name).await?;
    config.active_profile_mut(profile_name)?.token = None;
    config.save()?;
    if json {
        output::print_json(&serde_json::json!({ "logged_in": false }));
    } else {
        output::success("Logged out");
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
    if json {
        output::print_json(&serde_json::json!({
            "logged_in": true, "url": client.base_url, "nickname": nickname,
        }));
    } else {
        output::print_key_value(&[
            ("URL", &client.base_url),
            ("Status", "Logged in"),
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
    *client = Client::new(client.base_url.clone(), None)?;
    config.active_profile_mut(profile_name)?.url = client.base_url.clone();
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
    let registered = value
        .get("registered_at")
        .and_then(|v| v.as_i64())
        .map(|v| {
            chrono::DateTime::from_timestamp(v, 0)
                .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
                .unwrap_or_else(|| v.to_string())
        })
        .unwrap_or_else(|| "—".to_owned());
    let institute = value
        .get("institute_id")
        .and_then(|v| v.as_i64())
        .map(|v| v.to_string())
        .unwrap_or_else(|| "—".to_owned());
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
