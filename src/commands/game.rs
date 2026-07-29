use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};
use tabled::Tabled;

use crate::{
    cli::{GameListArgs, GameViewArgs, ScoreboardArgs},
    client::Client,
    config::ClientConfig,
    error::CliResult,
    output, resolve_game_id,
};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct GameInfo {
    pub id: i64,
    pub name: String,
    pub brief: Option<String>,
    pub start_at: i64,
    pub end_at: i64,
    pub host_type: Option<i64>,
    pub team_size: Option<i64>,
    pub hidden: Option<bool>,
    pub offline: Option<bool>,
    pub frozen: Option<bool>,
}

impl GameInfo {
    fn status_str(&self) -> &str {
        let now = Utc::now().timestamp();
        if self.start_at > now {
            "upcoming"
        } else if self.end_at < now {
            "ended"
        } else if self.frozen.unwrap_or(false) {
            "frozen"
        } else {
            "active"
        }
    }

    fn format_ts(ts: i64) -> String {
        if ts == 0 {
            return "—".to_owned();
        }
        Utc.timestamp_opt(ts, 0)
            .single()
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| ts.to_string())
    }

    fn host_type_str(&self) -> &str {
        match self.host_type.unwrap_or(0) {
            0 => "Individual",
            1 => "Team",
            _ => "Unknown",
        }
    }
}

#[derive(Tabled)]
struct GameRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Type")]
    game_type: String,
    #[tabled(rename = "Start")]
    start: String,
    #[tabled(rename = "End")]
    end: String,
    #[tabled(rename = "Status")]
    status: String,
}

pub async fn games(
    client: &mut Client,
    config: &mut ClientConfig,
    args: GameListArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let page_str = args.page.to_string();
    let page_size_str = args.page_size.to_string();
    let query: Vec<(&str, &str)> = vec![
        ("page", &page_str),
        ("page_size", &page_size_str),
    ];

    // Response: [data_array, total_count]
    let (games, _total): (Vec<GameInfo>, i64) =
        client.get("game", &query, config, profile_name).await?;

    if json {
        output::print_json(&games);
    } else {
        let rows: Vec<GameRow> = games
            .into_iter()
            .map(|g| {
                let status = g.status_str().to_owned();
                let game_type = g.host_type_str().to_owned();
                let start = GameInfo::format_ts(g.start_at);
                let end = GameInfo::format_ts(g.end_at);
                GameRow {
                    id: g.id,
                    name: g.name,
                    game_type,
                    start,
                    end,
                    status,
                }
            })
            .collect();
        output::print_table(&rows);
    }

    Ok(())
}

pub async fn game(
    client: &mut Client,
    config: &mut ClientConfig,
    args: GameViewArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let game_id = if let Some(ref g) = args.game {
        resolve_game_id(client, config, profile_name, Some(g)).await?
    } else {
        return Err(crate::CliError::Config(
            "specify a game name or ID".to_owned(),
        ));
    };

    let Some(game_id) = game_id else {
        return Err(crate::CliError::Config("game not specified".to_owned()));
    };

    let path = format!("game/{game_id}");
    let game: GameInfo = client.get(&path, &[], config, profile_name).await?;

    if json {
        output::print_json(&game);
    } else {
        let id_str = game.id.to_string();
        let start_str = GameInfo::format_ts(game.start_at);
        let end_str = GameInfo::format_ts(game.end_at);
        let status_str = game.status_str();

        let pairs: Vec<(&str, &str)> = vec![
            ("ID", &id_str),
            ("Name", game.name.as_str()),
            ("Type", game.host_type_str()),
            ("Start", &start_str),
            ("End", &end_str),
            ("Status", status_str),
        ];
        output::print_key_value(&pairs);

        if let Some(ref brief) = game.brief {
            println!();
            println!("{}", brief);
        }
    }

    Ok(())
}

#[derive(Deserialize, Serialize)]
struct TeamEntry {
    name: Option<String>,
    score: Option<i64>,
    rank: Option<i64>,
    solve_count: Option<i64>,
}

pub async fn scoreboard(
    client: &mut Client,
    config: &mut ClientConfig,
    args: ScoreboardArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let game_id = resolve_game_id(client, config, profile_name, args.game.as_deref()).await?;
    let Some(game_id) = game_id else {
        return Err(crate::CliError::Config("specify --game".to_owned()));
    };

    let path = format!("game/{game_id}/team");
    let query = &[("order_by", "score"), ("asc", "false")];

    // Response: [data_array, total_count]
    let (teams, _total): (Vec<TeamEntry>, i64) =
        client.get(&path, query, config, profile_name).await?;

    #[derive(Tabled)]
    struct ScoreRow {
        #[tabled(rename = "Rank")]
        rank: String,
        #[tabled(rename = "Team")]
        name: String,
        #[tabled(rename = "Score")]
        score: String,
        #[tabled(rename = "Solves")]
        solves: String,
    }

    if json {
        output::print_json(&teams);
    } else {
        let rows: Vec<ScoreRow> = teams
            .into_iter()
            .enumerate()
            .map(|(i, t)| ScoreRow {
                rank: t
                    .rank
                    .map(|r| r.to_string())
                    .unwrap_or_else(|| (i + 1).to_string()),
                name: t.name.unwrap_or_default(),
                score: t
                    .score
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "0".to_owned()),
                solves: t
                    .solve_count
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "—".to_owned()),
            })
            .collect();
        output::print_table(&rows);
    }

    Ok(())
}
