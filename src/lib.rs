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

/// Execute the CLI given parsed arguments.
///
/// # Errors
///
/// Returns `CliError` on configuration load failure, command dispatch failures,
/// or network errors from API commands.
pub async fn run(cli: Cli) -> CliResult<()> {
    let mut config = ClientConfig::load()?;
    let interactive = matches!(cli.command, None | Some(Commands::Interactive));
    if interactive {
        if cli.json {
            return Err(CliError::Config("interactive mode does not support --json".to_owned()));
        }
        if !std::io::stdin().is_terminal() {
            return Err(CliError::Config("interactive mode requires a TTY".to_owned()));
        }
        return commands::interactive::run(&mut config, cli.profile.as_deref(), cli.url, cli.token)
            .await;
    }

    run_in_session(cli, &mut config).await
}

pub(crate) async fn run_in_session(cli: Cli, config: &mut ClientConfig) -> CliResult<()> {
    let capture = output::Capture::start(cli.pager, cli.json);
    let result = run_in_session_inner(cli, config).await;
    let output_result = capture.finish();
    result.and(output_result)
}

async fn run_in_session_inner(cli: Cli, config: &mut ClientConfig) -> CliResult<()> {
    let Cli { json, profile, url, token, pager: _, command } = cli;
    let profile_name = profile.as_deref();

    match command {
        None => Err(CliError::Config("no command specified; use help".to_owned())),
        Some(Commands::Interactive) => {
            Err(CliError::Config("already in interactive mode".to_owned()))
        }
        Some(Commands::Completion(args)) => {
            commands::completion(args);
            Ok(())
        }
        Some(Commands::Profile { command }) => match command {
            ProfileCommand::List => {
                commands::profile_list(config, json);
                Ok(())
            }
            ProfileCommand::Show { name } => commands::profile_show(config, name.as_deref(), json),
            ProfileCommand::Add(args) => commands::profile_add(config, &args, json),
            ProfileCommand::Use { name } => commands::profile_use(config, &name, json),
            ProfileCommand::Remove(args) => commands::profile_remove(config, &args, json),
        },
        Some(Commands::Account { command: AccountCommand::List }) => {
            commands::auth::list(config, profile_name, json)
        }
        Some(Commands::Account { command: AccountCommand::Use { account } }) => {
            commands::auth::use_account(config, profile_name, &account, json)
        }
        Some(Commands::Account { command: AccountCommand::Remove(args) }) => {
            commands::auth::remove(config, profile_name, &args, json)
        }
        command => {
            let cmd =
                command.ok_or_else(|| CliError::Config("unexpected command state".to_owned()))?;
            dispatch_network(cmd, url, token, config, profile_name, json).await
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
    let (profile_url, profile_token) = {
        let profile = config.active_profile_resolved(profile_name)?;
        (profile.url.clone(), profile.active_token().map(str::to_owned))
    };
    let explicit_url = url_override.or_else(|| env::var("R2S_URL").ok());
    let explicit_token = token_override.or_else(|| env::var("R2S_TOKEN").ok());
    let (base_url, token, persist_token) =
        resolve_connection(&profile_url, profile_token, explicit_url, explicit_token);
    let mut client = Client::new(&base_url, token)?;
    client.set_token_persistence(persist_token);
    match command {
        Commands::Account { command } => match command {
            AccountCommand::List | AccountCommand::Use { .. } | AccountCommand::Remove(_) => {
                unreachable!()
            }
            AccountCommand::Login(args) => {
                commands::auth::login(&mut client, config, args, json, profile_name).await
            }
            AccountCommand::Logout => {
                commands::auth::logout(&mut client, config, json, profile_name).await
            }
            AccountCommand::Register(args) => {
                commands::auth::register(&mut client, config, args, json, profile_name).await
            }
            AccountCommand::Ping => {
                commands::auth::ping(&mut client, config, json, profile_name).await
            }
            AccountCommand::Show => {
                commands::auth::show(&mut client, config, json, profile_name).await
            }
            AccountCommand::Edit(args) => {
                commands::auth::edit(&mut client, config, args, json, profile_name).await
            }
            AccountCommand::Code(args) => {
                commands::auth::code(&mut client, config, args, json, profile_name).await
            }
        },
        Commands::Game { command } => match command {
            GameCommand::List(args) => {
                commands::game::games(&mut client, config, args, json, profile_name).await
            }
            GameCommand::Show { game } => {
                commands::game::game(&mut client, config, game, json, profile_name).await
            }
            GameCommand::Select { game } => {
                commands::game::select_game(&mut client, config, game, profile_name, json).await
            }
            GameCommand::Scoreboard(args) => {
                commands::game::scoreboard(&mut client, config, args, json, profile_name).await
            }
            GameCommand::Challenge { command } => {
                dispatch_challenge(&mut client, config, command, json, profile_name).await
            }
            GameCommand::Team { command } => match command {
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
            GameCommand::Submission { command: SubmissionCommand::List(args) } => {
                commands::submission::submissions(&mut client, config, args, json, profile_name)
                    .await
            }
        },
        Commands::Profile { .. } | Commands::Interactive | Commands::Completion(_) => {
            unreachable!()
        }
    }
}

async fn dispatch_challenge(
    client: &mut Client,
    config: &mut ClientConfig,
    command: ChallengeCommand,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    match command {
        ChallengeCommand::List(args) => {
            commands::challenge::challenges(client, config, args, json, profile_name).await
        }
        ChallengeCommand::Show(args) => {
            commands::challenge::view(client, config, args, json, profile_name).await
        }
        ChallengeCommand::Submit(args) => {
            commands::challenge::solve(client, config, args, json, profile_name).await
        }
        ChallengeCommand::Hints(args) => {
            commands::challenge::hints(client, config, args, json, profile_name).await
        }
        ChallengeCommand::UnlockHint(args) => {
            commands::challenge::unlock_hint(client, config, args, json, profile_name).await
        }
        ChallengeCommand::Start(args) => {
            commands::challenge::start(client, config, args, json, profile_name).await
        }
        ChallengeCommand::Stop(args) => {
            commands::challenge::stop(client, config, args, json, profile_name).await
        }
        ChallengeCommand::Files(args) => {
            commands::challenge::files(client, config, args, json, profile_name).await
        }
        ChallengeCommand::Download(args) => {
            commands::challenge::download(client, config, args, json, profile_name).await
        }
    }
}

fn resolve_connection(
    profile_url: &str,
    profile_token: Option<String>,
    explicit_url: Option<String>,
    explicit_token: Option<String>,
) -> (String, Option<String>, bool) {
    let base_url = explicit_url.unwrap_or_else(|| profile_url.to_owned());
    let same_endpoint = base_url.trim_end_matches('/') == profile_url.trim_end_matches('/');
    let persist_token = explicit_token.is_none() && same_endpoint;
    let token = explicit_token.or_else(|| same_endpoint.then_some(profile_token).flatten());
    (base_url, token, persist_token)
}

async fn resolve_game_id(
    client: &mut Client,
    config: &mut ClientConfig,
    profile_name: Option<&str>,
    game: Option<&str>,
) -> CliResult<Option<i64>> {
    #[derive(serde::Deserialize)]
    struct Item {
        id: i64,
        name: String,
    }
    let value = match game {
        Some(value) => value.to_owned(),
        None => match config.active_profile_resolved(profile_name)?.game.as_ref() {
            Some(value) => return Ok(Some(value.id)),
            None => return Ok(None),
        },
    };
    if let Ok(id) = value.parse() {
        return Ok(Some(id));
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
    #[derive(serde::Deserialize)]
    struct Item {
        id: i64,
        name: String,
    }
    if let Ok(id) = challenge.parse() {
        return Ok(id);
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
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::{
        Cli,
        cli::{AccountCommand, ChallengeCommand, Commands, GameCommand},
    };
    use clap::{CommandFactory, Parser};
    #[test]
    fn parses_new_command_tree() {
        let cli =
            Cli::try_parse_from(["ret2cli", "game", "challenge", "submit", "pwn", "--flag", "x"])
                .unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Game {
                command: GameCommand::Challenge { command: ChallengeCommand::Submit(_) }
            })
        ));
    }
    #[test]
    fn accepts_no_command_for_interactive_mode() {
        assert!(Cli::try_parse_from(["ret2cli"]).unwrap().command.is_none());
    }

    #[test]
    fn parses_account_switch_and_local_removal() {
        let use_account = Cli::try_parse_from(["ret2cli", "account", "use", "alt"]).unwrap();
        assert!(matches!(
            use_account.command,
            Some(Commands::Account { command: AccountCommand::Use { account } }) if account == "alt"
        ));

        let remove = Cli::try_parse_from(["ret2cli", "account", "remove", "alt", "--yes"]).unwrap();
        assert!(matches!(
            remove.command,
            Some(Commands::Account { command: AccountCommand::Remove(args) })
                if args.account == "alt" && args.yes
        ));
    }

    #[test]
    fn every_user_workflow_has_a_one_line_command() {
        let commands = [
            "profile list",
            "profile show default",
            "profile add school --url https://ctf.example/ --use-now",
            "profile use default",
            "profile remove school --yes",
            "account list",
            "account login --account alice --password secret",
            "account logout",
            "account register --account alice --nickname Alice --email alice@example.com --password secret",
            "account ping",
            "account show",
            "account edit --description hello --yes",
            "account edit --description-file intro.md --avatar avatar.png --yes",
            "account edit --remove-avatar --yes",
            "account code --yes",
            "account use alice",
            "account remove alice --yes",
            "game list --type training",
            "game show 11",
            "game select 11",
            "game scoreboard",
            "game challenge list",
            "game challenge show pwn",
            "game challenge submit pwn --flag flag{test}",
            "game challenge hints pwn",
            "game challenge unlock-hint pwn --id 3",
            "game challenge start pwn",
            "game challenge stop pwn",
            "game challenge files pwn",
            "game challenge download pwn --file attachment.zip --output task.zip",
            "game team list",
            "game team show example",
            "game team mine",
            "game team create --name example --tag TEST",
            "game team join invitation-token",
            "game team leave --yes",
            "game submission list",
        ];

        for command in commands {
            let args = std::iter::once("ret2cli").chain(command.split_ascii_whitespace());
            assert!(Cli::try_parse_from(args).is_ok(), "one-line command did not parse: {command}");
        }
    }

    #[test]
    fn old_root_level_game_resource_paths_are_removed() {
        for command in ["challenge list", "team list", "submission list"] {
            let args = std::iter::once("ret2cli").chain(command.split_ascii_whitespace());
            assert!(Cli::try_parse_from(args).is_err(), "obsolete command parsed: {command}");
        }
    }

    #[test]
    fn account_status_alias_is_removed() {
        assert!(Cli::try_parse_from(["ret2cli", "account", "status"]).is_err());
    }

    #[test]
    fn game_use_alias_is_removed() {
        assert!(Cli::try_parse_from(["ret2cli", "game", "use", "11"]).is_err());
    }

    #[test]
    fn root_help_uses_semantic_group_order() {
        let help = Cli::command().render_long_help().to_string();
        let positions: Vec<_> = ["profile", "account", "game", "interactive", "completion"]
            .into_iter()
            .map(|name| help.find(&format!("  {name}")).unwrap())
            .collect();
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(!help.contains("  challenge "));
        assert!(!help.contains("  team "));
        assert!(!help.contains("  submission "));
    }

    #[test]
    fn url_override_does_not_reuse_or_persist_profile_token() {
        let (url, token, persist) = super::resolve_connection(
            "https://one.example/",
            Some("one-token".to_owned()),
            Some("https://two.example/".to_owned()),
            None,
        );
        assert_eq!(url, "https://two.example/");
        assert_eq!(token, None);
        assert!(!persist);
    }

    #[test]
    fn same_profile_endpoint_uses_its_active_account_token() {
        let (_, token, persist) = super::resolve_connection(
            "https://one.example/",
            Some("one-token".to_owned()),
            Some("https://one.example".to_owned()),
            None,
        );
        assert_eq!(token.as_deref(), Some("one-token"));
        assert!(persist);
    }
}
