use std::io::{self, Write};

use serde::{Deserialize, Serialize};

use crate::{
    cli::{LoginArgs, RegisterArgs},
    client::Client,
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
    // Determine URL
    let url = if let Some(ref u) = args.url {
        u.clone()
    } else if !client.base_url.is_empty() {
        client.base_url.clone()
    } else {
        return Err(CliError::Config(
            "no URL configured. Use --url or set a profile with 'ret2cli use <name>'".to_owned(),
        ));
    };

    // Re-create client with the resolved URL if different
    if client.base_url != url {
        *client = Client::new(url.clone(), None)?;
        let profile = config.active_profile_mut(profile_name);
        profile.url = url.clone();
    }

    // Read account and password
    let account = if let Some(a) = args.account {
        a
    } else {
        prompt_input("Account: ")?
    };

    let password = if let Some(p) = args.password {
        p
    } else {
        rpassword::prompt_password("Password: ")?
    };

    // 1. Get captcha
    let captcha: CaptchaResponse = client
        .get("account/captcha/cli", &[], config, profile_name)
        .await?;

    let difficulty = captcha
        .challenge
        .split('#')
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(4);

    if difficulty > 6 {
        output::info(&format!(
            "Captcha difficulty is high ({difficulty}), this may take a while..."
        ));
    }

    // 2. Solve PoW — answer is challenge+nonce (full SHA256 input)
    let answer = solve_pow(&captcha.challenge);

    // 3. Login — server returns empty body, token is in Set-Token header
    let login_req = LoginRequest {
        account,
        password,
        captcha_id: captcha.id,
        captcha_answer: answer,
    };

    let token = client
        .post_no_body("account/login", &login_req, config, profile_name)
        .await?;

    if let Some(ref token) = token {
        let profile_result = client
            .get_value("account/profile", &[], config, profile_name)
            .await
            .ok();
        let nickname = profile_result
            .as_ref()
            .and_then(|v| v.get("nickname"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        if json {
            output::print_json(&serde_json::json!({
                "token": token,
                "nickname": nickname,
            }));
        } else {
            output::success(&format!("Logged in as {nickname}"));
        }
    } else {
        return Err(CliError::Api {
            status: reqwest::StatusCode::OK,
            message: "no token in response".to_owned(),
        });
    }

    Ok(())
}

pub async fn logout(
    client: &mut Client,
    config: &mut ClientConfig,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let _ = client
        .post_value("account/logout", &serde_json::json!({}), config, profile_name)
        .await;

    let profile = config.active_profile_mut(profile_name);
    profile.token = None;
    config.save()?;
    output::success("Logged out");
    Ok(())
}

pub async fn status(
    client: &mut Client,
    config: &ClientConfig,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let profile = config.active_profile_resolved(profile_name);

    if json {
        let mut temp_config = ClientConfig::default();
        let profile_info = if client.token.is_some() {
            client
                .get_value("account/profile", &[], &mut temp_config, None)
                .await
                .ok()
        } else {
            None
        };

        output::print_json(&serde_json::json!({
            "url": profile.url,
            "logged_in": client.token.is_some(),
            "nickname": profile_info.as_ref().and_then(|v| v.get("nickname")).and_then(|v| v.as_str()),
        }));
    } else {
        let url_display = if profile.url.is_empty() {
            "(not configured)"
        } else {
            profile.url.as_str()
        };
        let pairs: Vec<(&str, &str)> = vec![
            ("URL", url_display),
            (
                "Status",
                if client.token.is_some() {
                    "Logged in"
                } else {
                    "Not logged in"
                },
            ),
        ];
        output::print_key_value(&pairs);

        if client.token.is_some() {
            let mut temp_config = ClientConfig::default();
            match client
                .get_value("account/profile", &[], &mut temp_config, None)
                .await
            {
                Ok(v) => {
                    if let Some(nickname) = v.get("nickname").and_then(|v| v.as_str()) {
                        output::print_key_value(&[("Nickname", nickname)]);
                    }
                }
                Err(_) => {
                    output::error("Token may be invalid");
                }
            }
        }
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
    // Determine URL
    let url = if let Some(ref u) = args.url {
        u.clone()
    } else if !client.base_url.is_empty() {
        client.base_url.clone()
    } else {
        return Err(CliError::Config(
            "no URL configured. Use --url or set a profile".to_owned(),
        ));
    };

    if client.base_url != url {
        *client = Client::new(url.clone(), None)?;
        let profile = config.active_profile_mut(profile_name);
        profile.url = url.clone();
    }
    let account = prompt_input("Account: ")?;
    let nickname = prompt_input("Nickname: ")?;
    let email = prompt_input("Email: ")?;
    let password = rpassword::prompt_password("Password: ")?;

    // Get captcha
    let captcha: CaptchaResponse = client
        .get("account/captcha/cli", &[], config, profile_name)
        .await?;
    let answer = solve_pow(&captcha.challenge);

    let result = client
        .post_value(
            "account/register",
            &serde_json::json!({
                "account": account,
                "nickname": nickname,
                "email": email,
                "password": password,
                "captcha_id": captcha.id,
                "captcha_answer": answer,
            }),
            config,
            profile_name,
        )
        .await?;

    if json {
        output::print_json(&result);
    } else {
        output::success("Registration successful. You can now log in.");
    }
    Ok(())
}

/// Show auth status without network access (offline/config-only).
pub fn status_local(
    config: &ClientConfig,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let profile = config.active_profile_resolved(profile_name);

    if json {
        output::print_json(&serde_json::json!({
            "url": profile.url,
            "logged_in": profile.token.is_some(),
        }));
    } else {
        let url_display = if profile.url.is_empty() {
            "(not configured)"
        } else {
            profile.url.as_str()
        };
        let pairs: Vec<(&str, &str)> = vec![
            ("URL", url_display),
            ("Status", if profile.token.is_some() { "Logged in" } else { "Not logged in" }),
        ];
        output::print_key_value(&pairs);
    }
    Ok(())
}

/// Solve a Ret2Shell PoW challenge. Format: `difficulty#seed`
/// Returns the full SHA256 candidate (seed+nonce), not just the nonce.
fn solve_pow(challenge: &str) -> String {
    use ring::digest::{digest, SHA256};

    let parts: Vec<&str> = challenge.splitn(2, '#').collect();
    let difficulty = parts[0].parse::<u32>().unwrap_or(4);
    let seed = parts.get(1).copied().unwrap_or("");

    let target_prefix = "0".repeat(difficulty as usize);

    for i in 0u64.. {
        let candidate = format!("{seed}{i}");
        let hash = digest(&SHA256, candidate.as_bytes());
        let hex_hash = hex::encode(hash);
        if hex_hash.starts_with(&target_prefix) {
            return candidate;
        }
        if i == u64::MAX {
            break;
        }
    }

    "0".to_owned()
}

fn prompt_input(prompt: &str) -> CliResult<String> {
    let mut stdout = io::stdout();
    write!(stdout, "{prompt}")?;
    stdout.flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_owned())
}
