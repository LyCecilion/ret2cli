use std::{collections::HashMap, io, process::Stdio};

use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};
use tabled::Tabled;
use tokio::{io::AsyncWriteExt, process::Command};

use crate::{
    cli::{GameContextArgs, GameListArgs, GameShowArgs},
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
    pub introduction_id: Option<i64>,
    pub cover: Option<String>,
    pub logo: Option<String>,
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
    args: GameShowArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let game_id = resolve_game_id(client, config, profile_name, args.game.as_deref())
        .await?
        .ok_or_else(|| crate::CliError::Config("game not specified".to_owned()))?;
    let path = format!("game/{game_id}");
    let game: GameInfo = client.get(&path, &[], config, profile_name).await?;

    if args.intro || args.rules {
        let doc = if args.intro { "readme" } else { "rules" };
        return show_game_doc(client, config, profile_name, game_id, doc, json).await;
    }
    if args.cover {
        let hash = game
            .cover
            .as_deref()
            .ok_or_else(|| crate::CliError::Config("this game has no cover image".to_owned()))?;
        if json {
            output::print_json(&serde_json::json!({ "cover": hash }));
            return Ok(());
        }
        return show_game_cover(client, config, profile_name, hash).await;
    }

    if json {
        output::print_json(&game);
        return Ok(());
    }
    let id_str = game.id.to_string();
    let start_str = GameInfo::format_ts(game.start_at);
    let end_str = GameInfo::format_ts(game.end_at);
    let status_str = game.status_str();
    let team_size_str = game.team_size_str();
    let cover_str = game.cover.as_deref().unwrap_or("—");

    let pairs: Vec<(&str, &str)> = vec![
        ("ID", &id_str),
        ("Name", game.name.as_str()),
        ("Type", game.host_type_str()),
        ("Team size", &team_size_str),
        ("Cover", cover_str),
        ("Start", &start_str),
        ("End", &end_str),
        ("Status", status_str),
    ];
    output::print_key_value(&pairs);
    output::blank();
    output::line("Use --intro / --rules to read the game documents, --cover to render the image.");

    if let Some(ref brief) = game.brief {
        output::blank();
        output::line(brief);
    }

    Ok(())
}

async fn show_game_doc(
    client: &mut Client,
    config: &mut ClientConfig,
    profile_name: Option<&str>,
    game_id: i64,
    doc: &str,
    json: bool,
) -> CliResult<()> {
    let doc_path = format!("game/{game_id}/doc/{doc}");
    let value: serde_json::Value =
        client.get_value(&doc_path, &[], config, profile_name).await.map_err(|error| {
            if matches!(
                error,
                crate::CliError::Api { status, .. } if status == reqwest::StatusCode::NOT_FOUND
            ) {
                crate::CliError::Config(format!("this game has no {doc} document"))
            } else {
                error
            }
        })?;
    let content = value.as_str().unwrap_or_default().to_owned();
    if json {
        output::print_json(&serde_json::json!({ "doc": doc, "content": content }));
    } else if content.is_empty() {
        output::line("(empty document)");
    } else {
        output::print_markdown(&content);
    }
    Ok(())
}

async fn show_game_cover(
    client: &mut Client,
    config: &mut ClientConfig,
    profile_name: Option<&str>,
    hash: &str,
) -> CliResult<()> {
    let Some(protocol) = inline_image_protocol() else {
        return Err(crate::CliError::Config(format!(
            "cover display requires Kitty or iTerm2; the image is available at \
             /api/media?hash={hash}"
        )));
    };
    let (bytes, _content_type) =
        client.download_bytes("media", &[("hash", hash)], config, profile_name).await?;
    match protocol {
        InlineImageProtocol::Kitty => render_kitty_inline_image(&bytes, hash).await?,
        InlineImageProtocol::Iterm2 => {
            output::write_direct(&render_iterm2_inline_image(&bytes))?;
            output::write_direct(b"\n")?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InlineImageProtocol {
    Kitty,
    Iterm2,
}

fn inline_image_protocol() -> Option<InlineImageProtocol> {
    detect_inline_image_protocol(
        std::env::var_os("KITTY_WINDOW_ID").is_some(),
        std::env::var("TERM").ok().as_deref(),
        std::env::var("TERM_PROGRAM").ok().as_deref(),
        std::env::var("LC_TERMINAL").ok().as_deref(),
    )
}

fn detect_inline_image_protocol(
    kitty_window: bool,
    term: Option<&str>,
    term_program: Option<&str>,
    lc_terminal: Option<&str>,
) -> Option<InlineImageProtocol> {
    if kitty_window || term.is_some_and(|value| value.contains("kitty")) {
        Some(InlineImageProtocol::Kitty)
    } else if term_program == Some("iTerm.app") || lc_terminal == Some("iTerm2") {
        Some(InlineImageProtocol::Iterm2)
    } else {
        None
    }
}

const KITTY_ICAT_ARGS: &[&str] =
    &["--stdin=yes", "--transfer-mode=stream", "--fit=width", "--align=center"];

async fn render_kitty_inline_image(data: &[u8], hash: &str) -> CliResult<()> {
    output::flush_before_prompt()?;
    let mut child = spawn_kitty_icat().map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            crate::CliError::Config(format!(
                "Kitty cover display requires 'kitten icat'; the image is available at \
                 /api/media?hash={hash}"
            ))
        } else {
            crate::CliError::Io(error)
        }
    })?;
    let write_result = if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(data).await
    } else {
        Err(io::Error::other("kitten icat stdin was not piped"))
    };
    let status = child.wait().await.map_err(crate::CliError::Io)?;
    write_result.map_err(crate::CliError::Io)?;
    if !status.success() {
        return Err(crate::CliError::Config(format!(
            "kitten icat exited with {status}; the image is available at /api/media?hash={hash}"
        )));
    }
    Ok(())
}

fn spawn_kitty_icat() -> io::Result<tokio::process::Child> {
    let mut kitten = Command::new("kitten");
    kitten
        .arg("icat")
        .args(KITTY_ICAT_ARGS)
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    match kitten.spawn() {
        Ok(child) => Ok(child),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let mut kitty = Command::new("kitty");
            kitty
                .args(["+kitten", "icat"])
                .args(KITTY_ICAT_ARGS)
                .stdin(Stdio::piped())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()
        }
        Err(error) => Err(error),
    }
}

fn render_iterm2_inline_image(data: &[u8]) -> Vec<u8> {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(data);
    let mut output = Vec::with_capacity(encoded.len() + 64);
    output.extend_from_slice(b"\x1b]1337;File=inline=1;width=60%:");
    output.extend_from_slice(encoded.as_bytes());
    output.push(b'\x1b');
    output.push(b'\\');
    output
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
    fn parses_game_cover_and_introduction_id() {
        let game: GameInfo = serde_json::from_str(
            r#"{
          "id":1,"name":"MoeCTF 2025","brief":"demo","start_at":1,"end_at":2,
          "host_type":1,"team_size":1,"hidden":false,"offline":false,"frozen":false,
          "introduction_id":30,"cover":"ae91cce0","logo":"7c113534"
        }"#,
        )
        .unwrap();
        assert_eq!(game.introduction_id, Some(30));
        assert_eq!(game.cover.as_deref(), Some("ae91cce0"));
        assert_eq!(game.logo.as_deref(), Some("7c113534"));
    }

    #[test]
    fn inline_image_uses_iterm2_escape_with_base64_payload() {
        let rendered = render_iterm2_inline_image(b"hello");
        let text = String::from_utf8(rendered).unwrap();
        assert!(text.starts_with("\x1b]1337;File=inline=1;width=60%:"));
        assert!(text.ends_with("\x1b\\"));
        assert!(text.contains("aGVsbG8="));
    }

    #[test]
    fn detects_kitty_and_iterm2_image_protocols() {
        assert_eq!(
            detect_inline_image_protocol(true, Some("screen"), Some("iTerm.app"), None),
            Some(InlineImageProtocol::Kitty)
        );
        assert_eq!(
            detect_inline_image_protocol(false, Some("xterm-kitty"), None, None),
            Some(InlineImageProtocol::Kitty)
        );
        assert_eq!(
            detect_inline_image_protocol(false, Some("xterm-256color"), Some("iTerm.app"), None),
            Some(InlineImageProtocol::Iterm2)
        );
        assert_eq!(
            detect_inline_image_protocol(false, Some("screen"), None, Some("iTerm2")),
            Some(InlineImageProtocol::Iterm2)
        );
        assert_eq!(detect_inline_image_protocol(false, Some("xterm-256color"), None, None), None);
    }

    #[test]
    fn kitty_icat_is_inline_and_scrolls_with_text() {
        assert!(KITTY_ICAT_ARGS.contains(&"--stdin=yes"));
        assert!(KITTY_ICAT_ARGS.contains(&"--transfer-mode=stream"));
        assert!(!KITTY_ICAT_ARGS.iter().any(|arg| arg.starts_with("--place")));
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
