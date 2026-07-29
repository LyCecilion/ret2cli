pub mod auth;
pub mod challenge;
pub mod game;
pub mod profile;
pub mod submission;
pub mod team;

use std::io;

use crate::{
    cli::{CompletionArgs, UseArgs, UseGameArgs},
    config::ClientConfig,
    error::CliResult,
    output,
};

pub fn completion(args: CompletionArgs) {
    let mut cmd = <crate::Cli as clap::CommandFactory>::command();
    let name = cmd.get_name().to_owned();
    clap_complete::generate(args.shell, &mut cmd, name, &mut io::stdout());
}

pub fn use_profile(config: &mut ClientConfig, args: UseArgs) -> CliResult<()> {
    if !config.profiles.contains_key(&args.profile) {
        return Err(crate::CliError::Config(format!(
            "profile '{}' not found in config",
            args.profile
        )));
    }

    // Swap default with the named profile
    let profile = config.profiles.remove(&args.profile).unwrap();
    let old_default = std::mem::replace(&mut config.default, profile);
    config.profiles.insert(args.profile.clone(), old_default);
    config.save()?;

    output::success(&format!("Switched to profile '{}'", args.profile));
    Ok(())
}
pub fn use_game(args: UseGameArgs, config: &mut ClientConfig, json: bool) -> CliResult<()> {
    config.default_game = Some(args.game.clone());
    config.save()?;
    if json {
        output::print_json(&serde_json::json!({ "game": args.game }));
    } else {
        output::success(&format!("Default game set to '{}'", args.game));
    }
    Ok(())
}
