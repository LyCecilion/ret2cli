pub mod auth;
pub mod challenge;
pub mod game;
pub mod interactive;
pub mod submission;
pub mod team;

use std::io::{self, IsTerminal, Write};

use dialoguer::{Confirm, Input, Password};
use tabled::Tabled;

use crate::{
    cli::{CompletionArgs, ProfileAddArgs, ProfileRemoveArgs},
    config::{ClientConfig, ConnectionProfile},
    error::{CliError, CliResult},
    output,
};

#[allow(clippy::needless_pass_by_value)]
pub fn completion(args: CompletionArgs, json: bool) -> CliResult<()> {
    if json {
        return Err(CliError::Config("completion does not support --json".to_owned()));
    }
    let mut cmd = <crate::Cli as clap::CommandFactory>::command();
    let name = cmd.get_name().to_owned();
    let mut script = Vec::new();
    clap_complete::generate(args.shell, &mut cmd, name, &mut script);

    if let Some(path) = args.output.as_deref() {
        let mut options = std::fs::OpenOptions::new();
        options.write(true);
        if args.force {
            options.create(true).truncate(true);
        } else {
            options.create_new(true);
        }
        let mut file = options.open(path).map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                CliError::Config(format!(
                    "completion output '{}' already exists; pass --force to overwrite it",
                    path.display()
                ))
            } else {
                CliError::Io(error)
            }
        })?;
        file.write_all(&script)?;
        file.flush()?;
        output::success(&format!("Wrote completion script to {}", path.display()));
        return Ok(());
    }

    if io::stdout().is_terminal() {
        output::info(&format!(
            "Generated {} lines ({} bytes) of {} completion output.",
            line_count(&script),
            script.len(),
            args.shell
        ));
        output::flush_before_prompt()?;
        if !confirm("Print the completion script?", args.yes, false)? {
            output::info("Aborted");
            return Ok(());
        }
    }
    output::write_direct(&script)
}

fn line_count(content: &[u8]) -> usize {
    String::from_utf8_lossy(content).lines().count()
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
    output::flush_before_prompt()?;
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
                    "active_account": config.profiles[name].active_account,
                    "saved_accounts": config.profiles[name].accounts.len(),
                    "logged_in": config.profiles[name].active_token().is_some(),
                })
            })
            .collect();
        output::print_json(&rows);
    } else {
        #[derive(Tabled)]
        struct Row {
            #[tabled(rename = "Active")]
            active: &'static str,
            #[tabled(rename = "Name")]
            name: String,
            #[tabled(rename = "URL")]
            url: String,
            #[tabled(rename = "Account")]
            account: String,
            #[tabled(rename = "Game")]
            game: String,
        }
        let rows: Vec<_> = names
            .into_iter()
            .map(|name| {
                let profile = &config.profiles[name];
                Row {
                    active: if name == &config.active_profile { "*" } else { "" },
                    name: name.clone(),
                    url: profile.url.clone(),
                    account: profile
                        .active_account
                        .clone()
                        .unwrap_or_else(|| "anonymous".to_owned()),
                    game: profile.game.as_ref().map_or_else(|| "—".to_owned(), ToString::to_string),
                }
            })
            .collect();
        output::print_table(&rows);
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
            "active_account": profile.active_account,
            "saved_accounts": profile.accounts.len(),
            "logged_in": profile.active_token().is_some(),
        }));
    } else {
        output::print_key_value(&[
            ("Name", name),
            ("URL", &profile.url),
            ("Account", profile.active_account.as_deref().unwrap_or("—")),
            ("Saved accounts", &profile.accounts.len().to_string()),
            ("Game", &profile.game.as_ref().map_or_else(|| "—".to_owned(), ToString::to_string)),
            ("Status", if profile.active_token().is_some() { "Token stored" } else { "No token" }),
        ]);
    }
    Ok(())
}

pub fn profile_add(config: &mut ClientConfig, args: &ProfileAddArgs, json: bool) -> CliResult<()> {
    if config.profiles.contains_key(&args.name) {
        return Err(CliError::Config(format!("profile '{}' already exists", args.name)));
    }
    reqwest::Url::parse(&args.url).map_err(|e| CliError::Config(format!("invalid URL: {e}")))?;
    config.profiles.insert(args.name.clone(), ConnectionProfile::new(args.url.clone()));
    if args.use_now {
        config.active_profile.clone_from(&args.name);
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
    config.active_profile = name.to_string();
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
    args: &ProfileRemoveArgs,
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn completion_file_creation_is_non_destructive_by_default() {
        let path = std::env::temp_dir().join(format!(
            "ret2cli-completion-{}-{}.bash",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = std::fs::remove_file(&path);
        let args = |force| CompletionArgs {
            shell: clap_complete::Shell::Bash,
            output: Some(path.clone()),
            force,
            yes: false,
        };
        completion(args(false), false).unwrap();
        let generated = std::fs::read_to_string(&path).unwrap();
        assert!(generated.contains("_ret2cli"));
        assert!(completion(args(false), false).unwrap_err().to_string().contains("--force"));
        completion(args(true), false).unwrap();
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn completion_rejects_json_before_writing() {
        let error = completion(
            CompletionArgs {
                shell: clap_complete::Shell::Bash,
                output: None,
                force: false,
                yes: true,
            },
            true,
        )
        .unwrap_err();
        assert!(error.to_string().contains("does not support --json"));
    }

    #[test]
    fn line_count_handles_trailing_newlines() {
        assert_eq!(line_count(b""), 0);
        assert_eq!(line_count(b"one"), 1);
        assert_eq!(line_count(b"one\ntwo\n"), 2);
    }
}
