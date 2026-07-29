use serde::{Deserialize, Serialize};
use tabled::Tabled;

use crate::{
    cli::{
        MyTeamArgs, TeamCreateArgs, TeamJoinArgs, TeamLeaveArgs, TeamListArgs,
        TeamViewArgs,
    },
    client::Client,
    config::ClientConfig,
    error::CliResult,
    output, resolve_game_id,
};

#[derive(Debug, Deserialize, Serialize)]
struct TeamInfo {
    id: i64,
    name: Option<String>,
    score: Option<i64>,
    rank: Option<i64>,
    members: Option<Vec<MemberInfo>>,
    solve_count: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize)]
struct MemberInfo {
    nickname: Option<String>,
    account: Option<String>,
}

#[derive(Tabled)]
struct TeamRow {
    #[tabled(rename = "Rank")]
    rank: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Score")]
    score: String,
    #[tabled(rename = "Members")]
    members: String,
}

pub async fn teams(
    client: &mut Client,
    config: &mut ClientConfig,
    args: TeamListArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let game_id = resolve_game_id(client, config, profile_name, args.game.as_deref()).await?;
    let Some(game_id) = game_id else {
        return Err(crate::CliError::Config("specify --game".to_owned()));
    };

    let path = format!("game/{game_id}/team");


    let (teams, _total): (Vec<TeamInfo>, i64) =
        client.get(&path, &[], config, profile_name).await?;

    if json {
        output::print_json(&teams);
    } else {
        let rows: Vec<TeamRow> = teams
            .into_iter()
            .map(|t| {
                let members = t
                    .members
                    .as_ref()
                    .map(|m| {
                        m.iter()
                            .filter_map(|m| m.nickname.as_deref())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                TeamRow {
                    rank: t
                        .rank
                        .map(|r| r.to_string())
                        .unwrap_or_else(|| "—".to_owned()),
                    name: t.name.unwrap_or_default(),
                    score: t
                        .score
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "0".to_owned()),
                    members,
                }
            })
            .collect();
        output::print_table(&rows);
    }

    Ok(())
}

pub async fn team(
    client: &mut Client,
    config: &mut ClientConfig,
    args: TeamViewArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let game_id = resolve_game_id(client, config, profile_name, args.game.as_deref()).await?;
    let Some(game_id) = game_id else {
        return Err(crate::CliError::Config("specify --game".to_owned()));
    };

    let team_id = if let Some(ref t) = args.team {
        if let Ok(id) = t.parse::<i64>() {
            id
        } else {
            let list_path = format!("game/{game_id}/team");


            let (teams, _total): (Vec<TeamInfo>, i64) =
                client.get(&list_path, &[], config, profile_name).await?;

            teams
                .iter()
                .find(|tm| {
                    tm.name
                        .as_deref()
                        .map(|n| n.eq_ignore_ascii_case(t))
                        .unwrap_or(false)
                })
                .map(|tm| tm.id)
                .ok_or_else(|| crate::CliError::Config(format!("team '{t}' not found")))?
        }
    } else {
        return Err(crate::CliError::Config(
            "specify a team name or ID".to_owned(),
        ));
    };

    let path = format!("game/{game_id}/team/{team_id}");
    let team: TeamInfo = client.get(&path, &[], config, profile_name).await?;

    if json {
        output::print_json(&team);
    } else {
        let score_str = team
            .score
            .map(|s| s.to_string())
            .unwrap_or_else(|| "—".to_owned());
        let rank_str = team
            .rank
            .map(|r| r.to_string())
            .unwrap_or_else(|| "—".to_owned());
        let solves_str = team
            .solve_count
            .map(|s| s.to_string())
            .unwrap_or_else(|| "—".to_owned());
        let pairs: Vec<(&str, &str)> = vec![
            ("Name", team.name.as_deref().unwrap_or("—")),
            ("Score", &score_str),
            ("Rank", &rank_str),
            ("Solves", &solves_str),
        ];
        output::print_key_value(&pairs);

        if let Some(ref members) = team.members {
            if !members.is_empty() {
                println!();
                println!("Members:");
                for m in members {
                    let label = m.nickname.as_deref().unwrap_or("?");
                    let account = m.account.as_deref().unwrap_or("—");
                    println!("  • {label} ({account})");
                }
            }
        }
    }

    Ok(())
}

pub async fn my(
    client: &mut Client,
    config: &mut ClientConfig,
    args: MyTeamArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let game_id = resolve_game_id(client, config, profile_name, args.game.as_deref()).await?;
    let Some(game_id) = game_id else {
        return Err(crate::CliError::Config("specify --game".to_owned()));
    };

    let path = format!("game/{game_id}/team/self");
    let team: TeamInfo = client.get(&path, &[], config, profile_name).await?;

    if json {
        output::print_json(&team);
    } else {
        let score_str = team
            .score
            .map(|s| s.to_string())
            .unwrap_or_else(|| "—".to_owned());
        let rank_str = team
            .rank
            .map(|r| r.to_string())
            .unwrap_or_else(|| "—".to_owned());
        let pairs: Vec<(&str, &str)> = vec![
            ("Name", team.name.as_deref().unwrap_or("—")),
            ("Score", &score_str),
            ("Rank", &rank_str),
        ];
        output::print_key_value(&pairs);

        if let Some(ref members) = team.members {
            if !members.is_empty() {
                println!();
                println!("Members:");
                for m in members {
                    let label = m.nickname.as_deref().unwrap_or("?");
                    println!("  • {label}");
                }
            }
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
    let game_id = resolve_game_id(client, config, profile_name, args.game.as_deref()).await?;
    let Some(game_id) = game_id else {
        return Err(crate::CliError::Config("specify --game".to_owned()));
    };

    let name = rpassword::prompt_password("Team name: ")?;
    let description = rpassword::prompt_password("Description (optional): ").ok();
    let team_token =
        rpassword::prompt_password("Team token (for others to join, optional): ").ok();

    let mut body = serde_json::json!({
        "name": name,
    });
    if let Some(ref desc) = description {
        if !desc.is_empty() {
            body["description"] = serde_json::json!(desc);
        }
    }
    if let Some(ref token) = team_token {
        if !token.is_empty() {
            body["token"] = serde_json::json!(token);
        }
    }

    let path = format!("game/{game_id}/team");
    let result = client
        .post_value(&path, &body, config, profile_name)
        .await?;

    if json {
        output::print_json(&result);
    } else {
        output::success("Team created");
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
    let game_id = resolve_game_id(client, config, profile_name, args.game.as_deref()).await?;
    let Some(game_id) = game_id else {
        return Err(crate::CliError::Config("specify --game".to_owned()));
    };

    let path = format!("game/{game_id}/team/join");
    let result = client
        .post_value(
            &path,
            &serde_json::json!({ "token": args.token }),
            config,
            profile_name,
        )
        .await?;

    if json {
        output::print_json(&result);
    } else {
        output::success("Joined team");
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
    let game_id = resolve_game_id(client, config, profile_name, args.game.as_deref()).await?;
    let Some(game_id) = game_id else {
        return Err(crate::CliError::Config("specify --game".to_owned()));
    };

    let path = format!("game/{game_id}/team/self");

    if !json {
        let confirm =
            rpassword::prompt_password("Are you sure you want to leave your team? (y/N): ")?;
        if !confirm.eq_ignore_ascii_case("y") && !confirm.eq_ignore_ascii_case("yes") {
            output::info("Aborted.");
            return Ok(());
        }
    }

    client.delete(&path, config, profile_name).await?;

    if json {
        output::print_json(&serde_json::json!({ "status": "left" }));
    } else {
        output::success("Left team");
    }

    Ok(())
}
