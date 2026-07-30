use chrono::{DateTime, Utc};
use tabled::Tabled;

use crate::{
    cli::GameContextArgs,
    client::Client,
    commands::challenge::SubmissionInfo,
    config::ClientConfig,
    error::{CliError, CliResult},
    output, resolve_game_id,
};

pub async fn submissions(
    client: &mut Client,
    config: &mut ClientConfig,
    args: GameContextArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    #[derive(Tabled)]
    struct Row {
        #[tabled(rename = "Time")]
        time: String,
        #[tabled(rename = "Challenge")]
        challenge: String,
        #[tabled(rename = "Result")]
        result: String,
        #[tabled(rename = "Score")]
        score: String,
    }
    let game_id = resolve_game_id(client, config, profile_name, args.game.as_deref())
        .await?
        .ok_or_else(|| CliError::Config("no game selected".to_owned()))?;
    let path = format!("game/{game_id}/solve");
    let items: Vec<SubmissionInfo> = client.get(&path, &[], config, profile_name).await?;
    if json {
        output::print_json(&items);
        return Ok(());
    }
    let rows: Vec<_> = items
        .into_iter()
        .map(|s| Row {
            time: DateTime::<Utc>::from_timestamp(s.created_at, 0).map_or_else(
                || s.created_at.to_string(),
                |d| d.format("%Y-%m-%d %H:%M").to_string(),
            ),
            challenge: s.challenge_name.unwrap_or_else(|| s.challenge_id.to_string()),
            result: s
                .result
                .unwrap_or_else(|| if s.solved == Some(true) { "Solved" } else { "—" }.to_owned()),
            score: s.score.map_or_else(|| "—".to_owned(), |v| v.to_string()),
        })
        .collect();
    output::print_table(&rows);
    Ok(())
}
