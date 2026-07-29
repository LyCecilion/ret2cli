use serde::{Deserialize, Serialize};
use tabled::Tabled;

use crate::{
    cli::SubmissionsArgs,
    client::Client,
    config::ClientConfig,
    error::CliResult,
    output,
};

#[derive(Debug, Deserialize, Serialize)]
struct SubmissionInfo {
    id: Option<i64>,
    challenge: Option<serde_json::Value>,
    result: Option<String>,
    score: Option<i64>,
    created_at: Option<String>,
}

#[derive(Tabled)]
struct SubmissionRow {
    #[tabled(rename = "Time")]
    time: String,
    #[tabled(rename = "Challenge")]
    challenge: String,
    #[tabled(rename = "Result")]
    result: String,
    #[tabled(rename = "Score")]
    score: String,
}

pub async fn submissions(
    client: &mut Client,
    config: &mut ClientConfig,
    args: SubmissionsArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let game_id = args.game;

    let subs: Vec<SubmissionInfo> = if let Some(game_id) = game_id {
        let path = format!("game/{game_id}/solve");

        let (subs, _total): (Vec<SubmissionInfo>, i64) =
            client.get(&path, &[], config, profile_name).await?;
        subs
    } else {
        // Try recent games
        #[derive(Deserialize)]
        struct GameItem {
            id: i64,
        }

        let (games, _total): (Vec<GameItem>, i64) =
            client.get("game", &[("page_size", "10")], config, profile_name).await?;


        let mut all_subs = Vec::new();
        for g in games {
            let path = format!("game/{}/solve", g.id);

            if let Ok((data, _total)) = client
                .get::<(Vec<SubmissionInfo>, i64)>(&path, &[], config, profile_name)
                .await
            {
                all_subs.extend(data);
            }
        }
        all_subs
    };

    if json {
        output::print_json(&subs);
    } else {
        let rows: Vec<SubmissionRow> = subs
            .into_iter()
            .map(|s| {
                let challenge_name = s
                    .challenge
                    .as_ref()
                    .and_then(|c| c.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("—");
                SubmissionRow {
                    time: s.created_at.unwrap_or_else(|| "—".to_owned()),
                    challenge: challenge_name.to_owned(),
                    result: s.result.unwrap_or_else(|| "—".to_owned()),
                    score: s
                        .score
                        .map(|sc| sc.to_string())
                        .unwrap_or_else(|| "—".to_owned()),
                }
            })
            .collect();
        output::print_table(&rows);
    }

    Ok(())
}
