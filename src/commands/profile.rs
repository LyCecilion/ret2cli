use serde_json::Value;

use crate::{
    client::Client,
    config::ClientConfig,
    error::CliResult,
    output,
};

pub async fn profile(
    client: &mut Client,
    config: &mut ClientConfig,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let profile_data: Value = client
        .get_value("account/profile", &[], config, profile_name)
        .await?;

    if json {
        output::print_json(&profile_data);
    } else {
        let pairs: Vec<(&str, &str)> = vec![
            (
                "Account",
                profile_data
                    .get("account")
                    .and_then(|v| v.as_str())
                    .unwrap_or("—"),
            ),
            (
                "Nickname",
                profile_data
                    .get("nickname")
                    .and_then(|v| v.as_str())
                    .unwrap_or("—"),
            ),
            (
                "Email",
                profile_data
                    .get("email")
                    .and_then(|v| v.as_str())
                    .unwrap_or("—"),
            ),
            (
                "Institute",
                profile_data
                    .get("institute")
                    .and_then(|v| v.as_str())
                    .unwrap_or("—"),
            ),
            (
                "Registered",
                profile_data
                    .get("registered_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or("—"),
            ),
        ];
        output::print_key_value(&pairs);

        if let Some(perms) = profile_data.get("permissions") {
            println!();
            println!("Permissions:");
            output::print_json(&perms);
        }
    }

    Ok(())
}
