mod cli;
pub mod client;
mod commands;
pub mod config;
mod error;
mod output;

use std::env;

pub use cli::{Cli, Commands};
pub use error::{CliError, CliResult};

use crate::{
    client::Client,
    config::ClientConfig,
};

pub async fn run(cli: Cli) -> CliResult<()> {
    let mut config = ClientConfig::load()?;
    let json = cli.json;
    let profile_name = cli.profile.as_deref();

    // Commands that don't need a network client
    match &cli.command {
        Commands::Completion(args) => {
            commands::completion(args.clone());
            return Ok(());
        }
        Commands::Use(args) => {
            return commands::use_profile(&mut config, args.clone());
        }
        Commands::UseGame(args) => {
            return commands::use_game(args.clone(), &mut config, json);
        }
        _ => {}
    }
    // Resolve base URL: --url > env > profile > config default
    let base_url = cli
        .url
        .or_else(|| env::var("R2S_URL").ok())
        .or_else(|| {
            let profile = config.active_profile_resolved(profile_name);
            if profile.url.is_empty() {
                None
            } else {
                Some(profile.url.clone())
            }
        })
        .unwrap_or_default();

    // Status can work offline
    if matches!(&cli.command, Commands::Status) && base_url.is_empty() {
        return commands::auth::status_local(&config, json, profile_name);
    }

    // Resolve token: --token > env > profile
    let token = cli
        .token
        .or_else(|| env::var("R2S_TOKEN").ok())
        .or_else(|| {
            let profile = config.active_profile_resolved(profile_name);
            profile.token.clone()
        });

    let mut client = Client::new(base_url, token)?;

    match cli.command {
        Commands::Login(args) => commands::auth::login(&mut client, &mut config, args, json, profile_name).await,
        Commands::Logout => commands::auth::logout(&mut client, &mut config, profile_name).await,
        Commands::Register(args) => commands::auth::register(&mut client, &mut config, args, json, profile_name).await,
        Commands::Games(args) => commands::game::games(&mut client, &mut config, args, json, profile_name).await,
        Commands::Game(args) => commands::game::game(&mut client, &mut config, args, json, profile_name).await,
        Commands::Scoreboard(args) => commands::game::scoreboard(&mut client, &mut config, args, json, profile_name).await,
        Commands::Challenges(args) => commands::challenge::challenges(&mut client, &mut config, args, json, profile_name).await,
        Commands::View(args) => commands::challenge::view(&mut client, &mut config, args, json, profile_name).await,
        Commands::Solve(args) => commands::challenge::solve(&mut client, &mut config, args, json, profile_name).await,
        Commands::Hints(args) => commands::challenge::hints(&mut client, &mut config, args, json, profile_name).await,
        Commands::Hint(args) => commands::challenge::hint(&mut client, &mut config, args, json, profile_name).await,
        Commands::Start(args) => commands::challenge::start(&mut client, &mut config, args, json, profile_name).await,
        Commands::Stop(args) => commands::challenge::stop(&mut client, &mut config, args, json, profile_name).await,
        Commands::Download(args) => commands::challenge::download(&mut client, &mut config, args, json, profile_name).await,
        Commands::Teams(args) => commands::team::teams(&mut client, &mut config, args, json, profile_name).await,
        Commands::Team(args) => commands::team::team(&mut client, &mut config, args, json, profile_name).await,
        Commands::My(args) => commands::team::my(&mut client, &mut config, args, json, profile_name).await,
        Commands::TeamCreate(args) => commands::team::team_create(&mut client, &mut config, args, json, profile_name).await,
        Commands::TeamJoin(args) => commands::team::team_join(&mut client, &mut config, args, json, profile_name).await,
        Commands::TeamLeave(args) => commands::team::team_leave(&mut client, &mut config, args, json, profile_name).await,
        Commands::Profile => commands::profile::profile(&mut client, &mut config, json, profile_name).await,
        Commands::Submissions(args) => commands::submission::submissions(&mut client, &mut config, args, json, profile_name).await,
        Commands::Status => commands::auth::status(&mut client, &config, json, profile_name).await,
        // Already handled above
        Commands::Completion(_) | Commands::Use(_) | Commands::UseGame(_) => unreachable!(),
    }
}
/// Helper to resolve game ID from name or ID.
/// Accepts a name string or numeric ID. If name, queries the game list to resolve.
async fn resolve_game_id(
    client: &mut Client,
    config: &mut ClientConfig,
    profile_name: Option<&str>,
    game: Option<&str>,
) -> CliResult<Option<i64>> {
    let game = match game {
        Some(g) => g.to_owned(),
        None => match &config.default_game {
            Some(d) => d.clone(),
            None => return Ok(None),
        },
    };
    let game_ref = game.as_str();
    // Try parsing as numeric ID first
    if let Ok(id) = game_ref.parse::<i64>() {
        return Ok(Some(id));
    }
    // Response: [data_array, total_count]
    #[derive(serde::Deserialize)]
    struct GameItem {
        id: i64,
        name: String,
    }

    let (games, _total): (Vec<GameItem>, i64) = client
        .get("game", &[("page_size", "100")], config, profile_name)
        .await?;
    for g in &games {
        if g.name.eq_ignore_ascii_case(game_ref) {
            return Ok(Some(g.id));
        }
    }
    // Try prefix match
    let prefix_matches: Vec<_> = games.iter().filter(|g| g.name.to_lowercase().starts_with(&game_ref.to_lowercase())).collect();
    if prefix_matches.len() == 1 {
        return Ok(Some(prefix_matches[0].id));
    }
    Err(CliError::Config(format!(
        "game '{}' not found. Use numeric ID or exact name.",
        game_ref
    )))
}

/// Helper to resolve challenge ID from name or ID.
async fn resolve_challenge_id(
    client: &mut Client,
    config: &mut ClientConfig,
    profile_name: Option<&str>,
    game_id: i64,
    challenge: &str,
) -> CliResult<i64> {
    // Try parsing as numeric ID
    if let Ok(id) = challenge.parse::<i64>() {
        return Ok(id);
    }

    #[derive(serde::Deserialize)]
    struct ChallengeItem {
        id: i64,
        name: String,
    }

    let path = format!("game/{game_id}/challenge");
    let (challenges, _total): (Vec<ChallengeItem>, i64) = client
        .get(&path, &[("page_size", "500")], config, profile_name)
        .await?;

    // Exact match
    for c in &challenges {
        if c.name.eq_ignore_ascii_case(challenge) {
            return Ok(c.id);
        }
    }
    // Prefix match
    let prefix_matches: Vec<_> = challenges
        .iter()
        .filter(|c| c.name.to_lowercase().starts_with(&challenge.to_lowercase()))
        .collect();
    if prefix_matches.len() == 1 {
        return Ok(prefix_matches[0].id);
    }
    // Fuzzy match
    if prefix_matches.len() > 1 {
        return Err(CliError::Config(format!(
            "multiple challenges match '{challenge}': {}",
            prefix_matches
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    Err(CliError::Config(format!(
        "challenge '{challenge}' not found"
    )))
}
