use chrono::{DateTime, Utc};
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
    pub description: Option<String>,
    pub introduction: Option<String>,
    pub r#type: Option<String>,
    pub start_at: Option<String>,
    pub end_at: Option<String>,
    pub is_active: Option<bool>,
    pub team_count: Option<i64>,
    pub team_limit: Option<i64>,
    pub status: Option<i64>,
}

impl GameInfo {
    fn status_str(&self) -> &str {
        let now = Utc::now();
        let start = self
            .start_at
            .as_ref()
            .and_then(|s| s.parse::<DateTime<Utc>>().ok());
        let end = self
            .end_at
            .as_ref()
            .and_then(|s| s.parse::<DateTime<Utc>>().ok());

        match (start, end) {
            (Some(s), Some(_e)) if now < s => "upcoming",
            (Some(_), Some(e)) if now > e => "ended",
            (Some(_), Some(_)) => "active",
            (None, Some(e)) if now > e => "ended",
            _ => "active",
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
    #[tabled(rename = "Teams")]
    teams: String,
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
    let mut query: Vec<(&str, &str)> = vec![
        ("page", &page_str),
        ("page_size", &page_size_str),
    ];
    let type_str;
    if let Some(ref t) = args.r#type {
        type_str = t.clone();
        query.push(("type", &type_str));
    }

    #[derive(Deserialize)]
    struct GamePage {
        data: Option<Vec<GameInfo>>,
        #[serde(default)]
        records: Option<Vec<GameInfo>>,
        #[serde(default)]
        items: Option<Vec<GameInfo>>,
    }

    let response: GamePage = client.get("game", &query, config, profile_name).await?;
    let games = response
        .data
        .or(response.records)
        .or(response.items)
        .unwrap_or_default();

    if json {
        output::print_json(&games);
    } else {
        let rows: Vec<GameRow> = games
            .into_iter()
            .map(|g| {
                let status = g.status_str().to_owned();
                let game_type = g.r#type.unwrap_or_else(|| "Game".to_owned());
                let start = g.start_at.unwrap_or_else(|| "—".to_owned());
                let end = g.end_at.unwrap_or_else(|| "—".to_owned());
                let teams = g
                    .team_count
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "—".to_owned());
                GameRow {
                    id: g.id,
                    name: g.name,
                    game_type,
                    start,
                    end,
                    teams,
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
        let teams_str = game
            .team_count
            .map(|c| c.to_string())
            .unwrap_or_else(|| "—".to_owned());
        let status_str = game.status_str();
        let pairs: Vec<(&str, &str)> = vec![
            ("ID", &id_str),
            ("Name", game.name.as_str()),
            ("Type", game.r#type.as_deref().unwrap_or("—")),
            ("Start", game.start_at.as_deref().unwrap_or("—")),
            ("End", game.end_at.as_deref().unwrap_or("—")),
            ("Teams", &teams_str),
            ("Status", status_str),
        ];
        output::print_key_value(&pairs);

        if let Some(ref desc) = game.description {
            println!();
            println!("Description:");
            output::print_markdown(desc);
        }
        if let Some(ref intro) = game.introduction {
            println!();
            println!("Introduction:");
            output::print_markdown(intro);
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

    #[derive(Deserialize)]
    struct TeamPage {
        data: Option<Vec<TeamEntry>>,
        #[serde(default)]
        records: Option<Vec<TeamEntry>>,
    }

    let response: TeamPage = client.get(&path, query, config, profile_name).await?;
    let teams = response.data.or(response.records).unwrap_or_default();

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
