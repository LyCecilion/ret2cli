mod cli;
pub mod client;
mod commands;
pub mod config;
mod error;
mod output;

use std::{env, io::IsTerminal};

pub use cli::{Cli, Commands};
pub use error::{CliError, CliResult};

use crate::{
    cli::{
        AccountCommand, ChallengeCommand, GameCommand, ProfileCommand, SubmissionCommand,
        TeamCommand,
    },
    client::Client,
    config::ClientConfig,
};

pub async fn run(cli: Cli) -> CliResult<()> {
    let mut config = ClientConfig::load()?;
    let json = cli.json;
    let profile_override = cli.profile.clone();
    let profile_name = profile_override.as_deref();

    match cli.command {
        None => {
            if !std::io::stdin().is_terminal() || json {
                return Err(CliError::Config("no command specified; use --help".to_owned()));
            }
            return commands::interactive::run(&mut config, profile_name).await;
        }
        Some(Commands::Interactive) => {
            return commands::interactive::run(&mut config, profile_name).await;
        }
        Some(Commands::Completion(args)) => {
            commands::completion(args);
            Ok(())
        }
        Some(Commands::Profile { command }) => match command {
            ProfileCommand::List => {
                commands::profile_list(&config, json);
                Ok(())
            }
            ProfileCommand::Show { name } => commands::profile_show(&config, name.as_deref(), json),
            ProfileCommand::Add(args) => commands::profile_add(&mut config, args, json),
            ProfileCommand::Use { name } => commands::profile_use(&mut config, &name, json),
            ProfileCommand::Remove(args) => commands::profile_remove(&mut config, args, json),
        },
        command => {
            dispatch_network(
                command.expect("matched Some"),
                cli.url,
                cli.token,
                &mut config,
                profile_name,
                json,
            )
            .await
        }
    }
}

async fn dispatch_network(
    command: Commands,
    url_override: Option<String>,
    token_override: Option<String>,
    config: &mut ClientConfig,
    profile_name: Option<&str>,
    json: bool,
) -> CliResult<()> {
    let profile = config.active_profile_resolved(profile_name)?;
    let base_url =
        url_override.or_else(|| env::var("R2S_URL").ok()).unwrap_or_else(|| profile.url.clone());
    let token =
        token_override.or_else(|| env::var("R2S_TOKEN").ok()).or_else(|| profile.token.clone());
    let mut client = Client::new(base_url, token)?;
    match command {
        Commands::Account { command } => match command {
            AccountCommand::Login(args) => {
                commands::auth::login(&mut client, config, args, json, profile_name).await
            }
            AccountCommand::Logout => {
                commands::auth::logout(&mut client, config, json, profile_name).await
            }
            AccountCommand::Register(args) => {
                commands::auth::register(&mut client, config, args, json, profile_name).await
            }
            AccountCommand::Status => {
                commands::auth::status(&mut client, config, json, profile_name).await
            }
            AccountCommand::Show => {
                commands::auth::show(&mut client, config, json, profile_name).await
            }
        },
        Commands::Game { command } => match command {
            GameCommand::List(args) => {
                commands::game::games(&mut client, config, args, json, profile_name).await
            }
            GameCommand::Show { game } => {
                commands::game::game(&mut client, config, game, json, profile_name).await
            }
            GameCommand::Use { game } => {
                commands::game::use_game(&mut client, config, game, profile_name, json).await
            }
            GameCommand::Scoreboard(args) => {
                commands::game::scoreboard(&mut client, config, args, json, profile_name).await
            }
        },
        Commands::Challenge { command } => match command {
            ChallengeCommand::List(args) => {
                commands::challenge::challenges(&mut client, config, args, json, profile_name).await
            }
            ChallengeCommand::Show(args) => {
                commands::challenge::view(&mut client, config, args, json, profile_name).await
            }
            ChallengeCommand::Submit(args) => {
                commands::challenge::solve(&mut client, config, args, json, profile_name).await
            }
            ChallengeCommand::Hints(args) => {
                commands::challenge::hints(&mut client, config, args, json, profile_name).await
            }
            ChallengeCommand::UnlockHint(args) => {
                commands::challenge::unlock_hint(&mut client, config, args, json, profile_name)
                    .await
            }
            ChallengeCommand::Start(args) => {
                commands::challenge::start(&mut client, config, args, json, profile_name).await
            }
            ChallengeCommand::Stop(args) => {
                commands::challenge::stop(&mut client, config, args, json, profile_name).await
            }
            ChallengeCommand::Files(args) => {
                commands::challenge::files(&mut client, config, args, json, profile_name).await
            }
            ChallengeCommand::Download(args) => {
                commands::challenge::download(&mut client, config, args, json, profile_name).await
            }
        },
        Commands::Team { command } => match command {
            TeamCommand::List(args) => {
                commands::team::teams(&mut client, config, args, json, profile_name).await
            }
            TeamCommand::Show(args) => {
                commands::team::team(&mut client, config, args, json, profile_name).await
            }
            TeamCommand::Mine(args) => {
                commands::team::my(&mut client, config, args, json, profile_name).await
            }
            TeamCommand::Create(args) => {
                commands::team::team_create(&mut client, config, args, json, profile_name).await
            }
            TeamCommand::Join(args) => {
                commands::team::team_join(&mut client, config, args, json, profile_name).await
            }
            TeamCommand::Leave(args) => {
                commands::team::team_leave(&mut client, config, args, json, profile_name).await
            }
        },
        Commands::Submission { command: SubmissionCommand::List(args) } => {
            commands::submission::submissions(&mut client, config, args, json, profile_name).await
        }
        Commands::Profile { .. } | Commands::Interactive | Commands::Completion(_) => {
            unreachable!()
        }
    }
}

async fn resolve_game_id(
    client: &mut Client,
    config: &mut ClientConfig,
    profile_name: Option<&str>,
    game: Option<&str>,
) -> CliResult<Option<i64>> {
    let value = match game {
        Some(value) => value.to_owned(),
        None => match config.active_profile_resolved(profile_name)?.game.clone() {
            Some(value) => value,
            None => return Ok(None),
        },
    };
    if let Ok(id) = value.parse() {
        return Ok(Some(id));
    }
    #[derive(serde::Deserialize)]
    struct Item {
        id: i64,
        name: String,
    }
    let (items, _): (Vec<Item>, u64) =
        client.get("game", &[("page_size", "100")], config, profile_name).await?;
    let lowered = value.to_lowercase();
    let matches: Vec<_> = items
        .into_iter()
        .filter(|g| {
            g.name.eq_ignore_ascii_case(&value) || g.name.to_lowercase().starts_with(&lowered)
        })
        .collect();
    if matches.len() == 1 {
        Ok(Some(matches[0].id))
    } else {
        Err(CliError::Config(format!("game '{value}' is missing or ambiguous")))
    }
}

async fn resolve_challenge_id(
    client: &mut Client,
    config: &mut ClientConfig,
    profile_name: Option<&str>,
    game_id: i64,
    challenge: &str,
) -> CliResult<i64> {
    if let Ok(id) = challenge.parse() {
        return Ok(id);
    }
    #[derive(serde::Deserialize)]
    struct Item {
        id: i64,
        name: String,
    }
    let path = format!("game/{game_id}/challenge");
    let (items, _): (Vec<Item>, u64) = client.get(&path, &[], config, profile_name).await?;
    let lowered = challenge.to_lowercase();
    let matches: Vec<_> = items
        .into_iter()
        .filter(|c| {
            c.name.eq_ignore_ascii_case(challenge) || c.name.to_lowercase().starts_with(&lowered)
        })
        .collect();
    if matches.len() == 1 {
        Ok(matches[0].id)
    } else {
        Err(CliError::Config(format!("challenge '{challenge}' is missing or ambiguous")))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Cli,
        cli::{ChallengeCommand, Commands},
    };
    use clap::Parser;
    #[test]
    fn parses_new_command_tree() {
        let cli =
            Cli::try_parse_from(["ret2cli", "challenge", "submit", "pwn", "--flag", "x"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Challenge { command: ChallengeCommand::Submit(_) })
        ));
    }
    #[test]
    fn accepts_no_command_for_interactive_mode() {
        assert!(Cli::try_parse_from(["ret2cli"]).unwrap().command.is_none());
    }
}
