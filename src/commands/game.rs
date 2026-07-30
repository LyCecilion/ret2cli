use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};
use tabled::Tabled;

use crate::{
    cli::{GameContextArgs, GameListArgs},
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
            0 => "Training",
            1 => "Game",
            _ => "Unknown",
        }
    }
}

#[derive(Tabled)]
pub struct GameRow {
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
    let mut query: Vec<(&str, &str)> = vec![("page", &page_str), ("page_size", &page_size_str)];
    if let Some(kind) = args.r#type {
        query.push(("host_type", kind.api_value()));
    }

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
                GameRow { id: g.id, name: g.name, game_type, start, end, status }
            })
            .collect();
        output::print_table(&rows);
    }

    Ok(())
}

pub async fn game(
    client: &mut Client,
    config: &mut ClientConfig,
    game_arg: Option<String>,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let game_id = if let Some(ref g) = game_arg {
        resolve_game_id(client, config, profile_name, Some(g)).await?
    } else {
        resolve_game_id(client, config, profile_name, None).await?
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
    id: i64,
    name: Option<String>,
    score: Option<i64>,
}

pub async fn scoreboard(
    client: &mut Client,
    config: &mut ClientConfig,
    args: GameContextArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let game_id = resolve_game_id(client, config, profile_name, args.game.as_deref()).await?;
    let Some(game_id) = game_id else {
        return Err(crate::CliError::Config("specify --game".to_owned()));
    };

    let path = format!("game/{game_id}/team");
    let query = &[("order", "score"), ("asc", "false")];

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
        #[tabled(rename = "ID")]
        id: i64,
    }

    if json {
        output::print_json(&teams);
    } else {
        let rows: Vec<ScoreRow> = teams
            .into_iter()
            .enumerate()
            .map(|(i, t)| ScoreRow {
                rank: (i + 1).to_string(),
                id: t.id,
                name: t.name.unwrap_or_default(),
                score: t.score.map(|s| s.to_string()).unwrap_or_else(|| "0".to_owned()),
            })
            .collect();
        output::print_table(&rows);
    }

    Ok(())
}

pub async fn use_game(
    client: &mut Client,
    config: &mut ClientConfig,
    game: String,
    profile_name: Option<&str>,
    json: bool,
) -> CliResult<()> {
    let id = resolve_game_id(client, config, profile_name, Some(&game))
        .await?
        .ok_or_else(|| crate::CliError::Config("game not found".to_owned()))?;
    config.active_profile_mut(profile_name)?.game = Some(id.to_string());
    config.save()?;
    if json {
        output::print_json(&serde_json::json!({ "game": id }));
    } else {
        output::success(&format!("Selected game '{game}' ({id})"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_ret2shell_game_timestamps_and_host_type() {
        let game: GameInfo = serde_json::from_str(
            r#"{
          "id":1,"name":"Training","brief":"demo","start_at":1,"end_at":2,
          "host_type":0,"team_size":0,"hidden":false,"offline":false,"frozen":false
        }"#,
        )
        .unwrap();
        assert_eq!(game.host_type_str(), "Training");
        assert_eq!(game.start_at, 1);
    }
}
