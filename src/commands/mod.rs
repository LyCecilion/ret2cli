pub mod auth;
pub mod challenge;
pub mod game;
pub mod interactive;
pub mod submission;
pub mod team;

use std::io::{self, IsTerminal};

use dialoguer::{Confirm, Input, Password};

use crate::{
    cli::{CompletionArgs, ProfileAddArgs, ProfileRemoveArgs},
    config::{ClientConfig, Profile},
    error::{CliError, CliResult},
    output,
};

pub fn completion(args: CompletionArgs) {
    let mut cmd = <crate::Cli as clap::CommandFactory>::command();
    let name = cmd.get_name().to_owned();
    clap_complete::generate(args.shell, &mut cmd, name, &mut io::stdout());
}

pub fn require_or_input(value: Option<String>, prompt: &str, json: bool) -> CliResult<String> {
    if let Some(value) = value {
        return Ok(value);
    }
    if json || !io::stdin().is_terminal() {
        return Err(CliError::Config(format!("missing required value: {prompt}")));
    }
    Input::new().with_prompt(prompt).interact_text().map_err(|e| CliError::Io(io::Error::other(e)))
}

pub fn require_or_password(value: Option<String>, prompt: &str, json: bool) -> CliResult<String> {
    if let Some(value) = value {
        return Ok(value);
    }
    if json || !io::stdin().is_terminal() {
        return Err(CliError::Config(format!("missing required value: {prompt}")));
    }
    Password::new().with_prompt(prompt).interact().map_err(|e| CliError::Io(io::Error::other(e)))
}

pub fn confirm(prompt: &str, yes: bool, json: bool) -> CliResult<bool> {
    if yes {
        return Ok(true);
    }
    if json || !io::stdin().is_terminal() {
        return Err(CliError::Config("confirmation required; pass --yes".to_owned()));
    }
    Confirm::new()
        .with_prompt(prompt)
        .default(false)
        .interact()
        .map_err(|e| CliError::Io(io::Error::other(e)))
}

pub fn profile_list(config: &ClientConfig, json: bool) {
    let mut names: Vec<_> = config.profiles.keys().collect();
    names.sort();
    if json {
        let rows: Vec<_> = names
            .into_iter()
            .map(|name| {
                serde_json::json!({
                    "name": name,
                    "active": name == &config.active_profile,
                    "url": config.profiles[name].url,
                    "game": config.profiles[name].game,
                    "logged_in": config.profiles[name].token.is_some(),
                })
            })
            .collect();
        output::print_json(&rows);
    } else {
        for name in names {
            let marker = if name == &config.active_profile { "*" } else { " " };
            println!("{marker} {name:<16} {}", config.profiles[name].url);
        }
    }
}

pub fn profile_show(config: &ClientConfig, name: Option<&str>, json: bool) -> CliResult<()> {
    let name = name.unwrap_or(&config.active_profile);
    let profile = config
        .profiles
        .get(name)
        .ok_or_else(|| CliError::Config(format!("profile '{name}' not found")))?;
    if json {
        output::print_json(&serde_json::json!({
            "name": name, "active": name == config.active_profile,
            "url": profile.url, "game": profile.game,
            "logged_in": profile.token.is_some(),
        }));
    } else {
        output::print_key_value(&[
            ("Name", name),
            ("URL", &profile.url),
            ("Game", profile.game.as_deref().unwrap_or("—")),
            ("Status", if profile.token.is_some() { "Token stored" } else { "No token" }),
        ]);
    }
    Ok(())
}

pub fn profile_add(config: &mut ClientConfig, args: ProfileAddArgs, json: bool) -> CliResult<()> {
    if config.profiles.contains_key(&args.name) {
        return Err(CliError::Config(format!("profile '{}' already exists", args.name)));
    }
    reqwest::Url::parse(&args.url).map_err(|e| CliError::Config(format!("invalid URL: {e}")))?;
    config
        .profiles
        .insert(args.name.clone(), Profile { url: args.url.clone(), token: None, game: None });
    if args.use_now {
        config.active_profile = args.name.clone();
    }
    config.save()?;
    if json {
        output::print_json(&serde_json::json!({ "name": args.name, "url": args.url }));
    } else {
        output::success(&format!("Added profile '{}'", args.name));
    }
    Ok(())
}

pub fn profile_use(config: &mut ClientConfig, name: &str, json: bool) -> CliResult<()> {
    if !config.profiles.contains_key(name) {
        return Err(CliError::Config(format!("profile '{name}' not found")));
    }
    config.active_profile = name.to_owned();
    config.save()?;
    if json {
        output::print_json(&serde_json::json!({ "active_profile": name }));
    } else {
        output::success(&format!("Using profile '{name}'"));
    }
    Ok(())
}

pub fn profile_remove(
    config: &mut ClientConfig,
    args: ProfileRemoveArgs,
    json: bool,
) -> CliResult<()> {
    if args.name == "default" || args.name == config.active_profile {
        return Err(CliError::Config("cannot remove the default or active profile".to_owned()));
    }
    if !config.profiles.contains_key(&args.name) {
        return Err(CliError::Config(format!("profile '{}' not found", args.name)));
    }
    if !confirm(&format!("Remove profile '{}'?", args.name), args.yes, json)? {
        output::info("Aborted");
        return Ok(());
    }
    config.profiles.remove(&args.name);
    config.save()?;
    if json {
        output::print_json(&serde_json::json!({ "removed": args.name }));
    } else {
        output::success(&format!("Removed profile '{}'", args.name));
    }
    Ok(())
}
