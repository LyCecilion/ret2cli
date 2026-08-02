use std::collections::HashMap;

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
            .map_or_else(|| ts.to_string(), |dt| dt.format("%Y-%m-%d").to_string())
    }

    fn host_type_str(&self) -> &str {
        match self.host_type.unwrap_or(0) {
            0 => "Training",
            1 => "Game",
            _ => "Unknown",
        }
    }

    /// Team size is a cap, not a quota: 1..=N members are valid, 0 means unlimited.
    fn team_size_str(&self) -> String {
        match self.team_size {
            None => "—".to_owned(),
            Some(0) => "unlimited".to_owned(),
            Some(value) => format!("\u{2264}{value}"),
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
        let team_size_str = game.team_size_str();

        let pairs: Vec<(&str, &str)> = vec![
            ("ID", &id_str),
            ("Name", game.name.as_str()),
            ("Type", game.host_type_str()),
            ("Team size", &team_size_str),
            ("Start", &start_str),
            ("End", &end_str),
            ("Status", status_str),
        ];
        output::print_key_value(&pairs);

        if let Some(ref brief) = game.brief {
            output::blank();
            output::line(brief);
        }
    }

    Ok(())
}

#[derive(Deserialize, Serialize)]
struct TeamEntry {
    id: i64,
    name: Option<String>,
    score: Option<i64>,
    institute_id: Option<i64>,
}

#[derive(Deserialize)]
struct InstituteEntry {
    id: i64,
    name: String,
}

#[derive(Serialize)]
struct ScoreboardEntry {
    id: i64,
    name: Option<String>,
    score: Option<i64>,
    institute_id: Option<i64>,
    institute_name: Option<String>,
}

pub async fn scoreboard(
    client: &mut Client,
    config: &mut ClientConfig,
    args: GameContextArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    #[derive(Tabled)]
    struct ScoreRow {
        #[tabled(rename = "Rank")]
        rank: String,
        #[tabled(rename = "Team")]
        name: String,
        #[tabled(rename = "Group")]
        group: String,
        #[tabled(rename = "Score")]
        score: String,
        #[tabled(rename = "ID")]
        id: i64,
    }
    let game_id = resolve_game_id(client, config, profile_name, args.game.as_deref()).await?;
    let Some(game_id) = game_id else {
        return Err(crate::CliError::Config("specify --game".to_owned()));
    };

    let path = format!("game/{game_id}/team");
    let query = &[("order", "score"), ("asc", "false")];

    // Response: [data_array, total_count]
    let (teams, _total): (Vec<TeamEntry>, i64) =
        client.get(&path, query, config, profile_name).await?;
    let institutes: Vec<InstituteEntry> =
        client.get("account/institute", &[], config, profile_name).await?;
    let institute_names: HashMap<_, _> =
        institutes.into_iter().map(|item| (item.id, item.name)).collect();
    let teams = enrich_scoreboard(teams, &institute_names);

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
                group: t.institute_name.unwrap_or_else(|| "—".to_owned()),
                score: t.score.map_or_else(|| "0".to_owned(), |s| s.to_string()),
            })
            .collect();
        output::print_table(&rows);
    }

    Ok(())
}

pub async fn select_game(
    client: &mut Client,
    config: &mut ClientConfig,
    game: String,
    profile_name: Option<&str>,
    json: bool,
) -> CliResult<()> {
    let id = resolve_game_id(client, config, profile_name, Some(&game))
        .await?
        .ok_or_else(|| crate::CliError::Config("game not found".to_owned()))?;
    let path = format!("game/{id}");
    let selected: GameInfo = client.get(&path, &[], config, profile_name).await?;
    let selected = crate::config::SelectedGame { id: selected.id, name: selected.name };
    config.active_profile_mut(profile_name)?.game = Some(selected.clone());
    config.save()?;
    if json {
        output::print_json(&serde_json::json!({ "game": selected }));
    } else {
        output::success(&format!("Selected game {selected}"));
    }
    Ok(())
}

fn enrich_scoreboard(
    teams: Vec<TeamEntry>,
    institute_names: &HashMap<i64, String>,
) -> Vec<ScoreboardEntry> {
    teams
        .into_iter()
        .map(|team| ScoreboardEntry {
            id: team.id,
            name: team.name,
            score: team.score,
            institute_id: team.institute_id,
            institute_name: team.institute_id.and_then(|id| institute_names.get(&id).cloned()),
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
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

    #[test]
    fn team_size_is_a_cap_not_a_quota() {
        let game: GameInfo = serde_json::from_str(
            r#"{
          "id":1,"name":"Mini L-CTF 2026","brief":null,"start_at":1,"end_at":2,
          "host_type":1,"team_size":4,"hidden":false,"offline":false,"frozen":false
        }"#,
        )
        .unwrap();
        assert_eq!(game.team_size_str(), "\u{2264}4");

        let unlimited: GameInfo = serde_json::from_str(
            r#"{
          "id":1,"name":"Open","brief":null,"start_at":1,"end_at":2,
          "host_type":1,"team_size":0,"hidden":false,"offline":false,"frozen":false
        }"#,
        )
        .unwrap();
        assert_eq!(unlimited.team_size_str(), "unlimited");

        let unknown: GameInfo = serde_json::from_str(
            r#"{
          "id":1,"name":"Legacy","brief":null,"start_at":1,"end_at":2,
          "host_type":1,"hidden":false,"offline":false,"frozen":false
        }"#,
        )
        .unwrap();
        assert_eq!(unknown.team_size_str(), "—");
    }

    #[test]
    fn scoreboard_maps_institute_names_to_groups() {
        let teams = vec![
            TeamEntry {
                id: 1,
                name: Some("Grouped".to_owned()),
                score: Some(100),
                institute_id: Some(7),
            },
            TeamEntry {
                id: 2,
                name: Some("Independent".to_owned()),
                score: Some(50),
                institute_id: None,
            },
        ];
        let names = HashMap::from([(7, "XDSEC".to_owned())]);
        let mapped = enrich_scoreboard(teams, &names);
        assert_eq!(mapped[0].institute_id, Some(7));
        assert_eq!(mapped[0].institute_name.as_deref(), Some("XDSEC"));
        assert_eq!(mapped[1].institute_name, None);
    }
}
