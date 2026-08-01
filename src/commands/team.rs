use serde::{Deserialize, Serialize};
use tabled::Tabled;

use crate::{
    cli::{GameContextArgs, TeamCreateArgs, TeamJoinArgs, TeamLeaveArgs, TeamShowArgs},
    client::Client,
    commands::{confirm, require_or_input},
    config::ClientConfig,
    error::{CliError, CliResult},
    output, resolve_game_id,
};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TeamInfo {
    pub id: i64,
    pub name: String,
    pub game_id: i64,
    pub token: Option<String>,
    pub state: i32,
    pub institute_id: Option<i64>,
    pub score: i32,
    pub tag: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct MemberInfo {
    account: String,
    nickname: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct TeamSolve {
    challenge_id: i64,
    challenge_name: Option<String>,
    solved: Option<bool>,
}

pub async fn fetch_teams(
    client: &mut Client,
    config: &mut ClientConfig,
    profile_name: Option<&str>,
    game_id: i64,
) -> CliResult<Vec<TeamInfo>> {
    let path = format!("game/{game_id}/team");
    let (teams, _): (Vec<TeamInfo>, u64) = client.get(&path, &[], config, profile_name).await?;
    Ok(teams)
}

pub async fn teams(
    client: &mut Client,
    config: &mut ClientConfig,
    args: GameContextArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    #[derive(Tabled)]
    struct Row {
        #[tabled(rename = "ID")]
        id: i64,
        #[tabled(rename = "Name")]
        name: String,
        #[tabled(rename = "Tag")]
        tag: String,
        #[tabled(rename = "Score")]
        score: i32,
    }
    let game_id = required_game(client, config, profile_name, args.game.as_deref()).await?;
    let teams = fetch_teams(client, config, profile_name, game_id).await?;
    if json {
        output::print_json(&teams);
        return Ok(());
    }
    let rows: Vec<_> = teams
        .into_iter()
        .map(|t| Row {
            id: t.id,
            name: t.name,
            tag: t.tag.unwrap_or_else(|| "—".to_owned()),
            score: t.score,
        })
        .collect();
    output::print_table(&rows);
    Ok(())
}

pub async fn team(
    client: &mut Client,
    config: &mut ClientConfig,
    args: TeamShowArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let game_id = required_game(client, config, profile_name, args.game.as_deref()).await?;
    let team_id = resolve_team(client, config, profile_name, game_id, &args.team).await?;
    show_team(client, config, profile_name, game_id, team_id, json).await
}

pub async fn my(
    client: &mut Client,
    config: &mut ClientConfig,
    args: GameContextArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let game_id = required_game(client, config, profile_name, args.game.as_deref()).await?;
    let path = format!("game/{game_id}/team/self");
    let team: TeamInfo = client.get(&path, &[], config, profile_name).await?;
    show_team(client, config, profile_name, game_id, team.id, json).await
}

async fn show_team(
    client: &mut Client,
    config: &mut ClientConfig,
    profile_name: Option<&str>,
    game_id: i64,
    team_id: i64,
    json: bool,
) -> CliResult<()> {
    let base = format!("game/{game_id}/team/{team_id}");
    let team: TeamInfo = client.get(&base, &[], config, profile_name).await?;
    let rank: Option<u64> =
        client.get(&format!("{base}/rank"), &[], config, profile_name).await.ok();
    let members: Vec<MemberInfo> =
        client.get(&format!("{base}/member"), &[], config, profile_name).await.unwrap_or_default();
    let solves: Vec<TeamSolve> =
        client.get(&format!("{base}/solve"), &[], config, profile_name).await.unwrap_or_default();
    if json {
        output::print_json(
            &serde_json::json!({ "team": team, "rank": rank, "members": members, "solves": solves }),
        );
        return Ok(());
    }
    let score = team.score.to_string();
    let rank = rank.map_or_else(|| "—".to_owned(), |v| v.to_string());
    let solve_count = solves.iter().filter(|s| s.solved == Some(true)).count().to_string();
    output::print_key_value(&[
        ("Name", &team.name),
        ("Tag", team.tag.as_deref().unwrap_or("—")),
        ("Score", &score),
        ("Rank", &rank),
        ("Solves", &solve_count),
    ]);
    if !members.is_empty() {
        output::blank();
        output::line("Members:");
        for m in members {
            output::line(&format!("  • {} ({})", m.nickname, m.account));
        }
    }
    Ok(())
}

pub async fn team_create(
    client: &mut Client,
    config: &mut ClientConfig,
    args: TeamCreateArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let game_id = required_game(client, config, profile_name, args.game.as_deref()).await?;
    let name = require_or_input(args.name, "Team name", json)?;
    let path = format!("game/{game_id}/team");
    let result: TeamInfo = client
        .post(&path, &serde_json::json!({ "name": name, "tag": args.tag }), config, profile_name)
        .await?;
    if json {
        output::print_json(&result);
    } else {
        output::success(&format!("Created team '{}'", result.name));
    }
    Ok(())
}

pub async fn team_join(
    client: &mut Client,
    config: &mut ClientConfig,
    args: TeamJoinArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let game_id = required_game(client, config, profile_name, args.game.as_deref()).await?;
    let token = require_or_input(args.token, "Team invitation token", json)?;
    let path = format!("game/{game_id}/team");
    let result: TeamInfo =
        client.patch(&path, &serde_json::json!({ "token": token }), config, profile_name).await?;
    if json {
        output::print_json(&result);
    } else {
        output::success(&format!("Joined team '{}'", result.name));
    }
    Ok(())
}

pub async fn team_leave(
    client: &mut Client,
    config: &mut ClientConfig,
    args: TeamLeaveArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let game_id = required_game(client, config, profile_name, args.game.as_deref()).await?;
    if !confirm("Leave your current team?", args.yes, json)? {
        output::info("Aborted");
        return Ok(());
    }
    client.delete(&format!("game/{game_id}/team/self"), config, profile_name).await?;
    if json {
        output::print_json(&serde_json::json!({ "left": true }));
    } else {
        output::success("Left team");
    }
    Ok(())
}

async fn required_game(
    client: &mut Client,
    config: &mut ClientConfig,
    profile_name: Option<&str>,
    game: Option<&str>,
) -> CliResult<i64> {
    resolve_game_id(client, config, profile_name, game)
        .await?
        .ok_or_else(|| CliError::Config("no game selected".to_owned()))
}
async fn resolve_team(
    client: &mut Client,
    config: &mut ClientConfig,
    profile_name: Option<&str>,
    game_id: i64,
    value: &str,
) -> CliResult<i64> {
    if let Ok(id) = value.parse() {
        return Ok(id);
    }
    let matches: Vec<_> = fetch_teams(client, config, profile_name, game_id)
        .await?
        .into_iter()
        .filter(|t| {
            t.name.eq_ignore_ascii_case(value)
                || t.name.to_lowercase().starts_with(&value.to_lowercase())
        })
        .collect();
    if matches.len() == 1 {
        Ok(matches[0].id)
    } else {
        Err(CliError::Config(format!("team '{value}' is missing or ambiguous")))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    #[test]
    fn parses_ret2shell_team_model() {
        let team: TeamInfo = serde_json::from_str(
            r#"{
            "id":1,"name":"A","game_id":2,"token":null,"state":3,
            "institute_id":null,"score":100,"history":[],"last_active_at":0,"tag":"TAG"
        }"#,
        )
        .unwrap();
        assert_eq!(team.score, 100);
    }

    #[tokio::test]
    async fn join_uses_patch_on_team_collection() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut buf = [0u8; 2048];
            loop {
                let read = socket.read(&mut buf).await.unwrap();
                if read == 0 {
                    break;
                }
                bytes.extend_from_slice(&buf[..read]);
                if let Some(split) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..split + 4]);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|v| v.parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= split + 4 + length {
                        break;
                    }
                }
            }
            let request = String::from_utf8(bytes).unwrap();
            assert!(request.starts_with("PATCH /api/game/1/team HTTP/1.1"));
            assert!(request.contains(r#""token":"invite""#));
            let body = r#"{"id":9,"name":"Joined","game_id":1,"token":null,"state":3,"institute_id":null,"score":0,"tag":null}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        let mut client = Client::new(&format!("http://{addr}"), None).unwrap();
        let mut config = ClientConfig::default();
        team_join(
            &mut client,
            &mut config,
            TeamJoinArgs { token: Some("invite".to_owned()), game: Some("1".to_owned()) },
            true,
            None,
        )
        .await
        .unwrap();
        server.await.unwrap();
    }
}
