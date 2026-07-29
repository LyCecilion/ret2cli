use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use tabled::Tabled;

use crate::{
    cli::{
        ChallengeListArgs, ChallengeViewArgs, DownloadArgs, HintArgs, HintsArgs,
        SolveArgs, StartArgs, StopArgs,
    },
    client::Client,
    config::ClientConfig,
    error::{CliError, CliResult},
    output, resolve_challenge_id, resolve_game_id,
};

#[derive(Debug, Deserialize, Serialize)]
struct ChallengeInfo {
    id: i64,
    name: String,
    score: Option<i64>,
    tags: Option<Value>,
    solved: Option<bool>,
    is_enabled: Option<bool>,
    content: Option<String>,
    attachments: Option<Value>,
    instance: Option<Value>,
    hints: Option<Value>,
}

#[derive(Tabled)]
struct ChallengeRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Score")]
    score: String,
    #[tabled(rename = "Tags")]
    tags: String,
    #[tabled(rename = "Solved")]
    solved: String,
}

pub async fn challenges(
    client: &mut Client,
    config: &mut ClientConfig,
    args: ChallengeListArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let game_id = resolve_game_id(client, config, profile_name, args.game.as_deref()).await?;
    let Some(game_id) = game_id else {
        return Err(CliError::Config("specify --game".to_owned()));
    };

    let path = format!("game/{game_id}/challenge");

    #[derive(Deserialize)]
    struct ChallengePage {
        data: Option<Vec<ChallengeInfo>>,
        #[serde(default)]
        records: Option<Vec<ChallengeInfo>>,
    }

    let response: ChallengePage = client.get(&path, &[], config, profile_name).await?;
    let challenges = response.data.or(response.records).unwrap_or_default();

    if json {
        output::print_json(&challenges);
    } else {
        let rows: Vec<ChallengeRow> = challenges
            .into_iter()
            .map(|c| ChallengeRow {
                id: c.id,
                name: c.name,
                score: c
                    .score
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "—".to_owned()),
                tags: format_tags(&c.tags),
                solved: if c.solved.unwrap_or(false) {
                    "✓".to_owned()
                } else if c.is_enabled.unwrap_or(true) {
                    "—".to_owned()
                } else {
                    "locked".to_owned()
                },
            })
            .collect();
        output::print_table(&rows);
    }

    Ok(())
}

pub async fn view(
    client: &mut Client,
    config: &mut ClientConfig,
    args: ChallengeViewArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let game_id = resolve_game_id(client, config, profile_name, args.game.as_deref()).await?;
    let Some(game_id) = game_id else {
        return Err(CliError::Config("specify --game".to_owned()));
    };

    let challenge_id =
        resolve_challenge_id(client, config, profile_name, game_id, &args.challenge).await?;

    let path = format!("game/{game_id}/challenge/{challenge_id}");
    let challenge: ChallengeInfo = client.get(&path, &[], config, profile_name).await?;

    if json {
        output::print_json(&challenge);
    } else {
        let tags = format_tags(&challenge.tags);
        let score_str = challenge
            .score
            .map(|s| s.to_string())
            .unwrap_or_else(|| "—".to_owned());
        let pairs: Vec<(&str, &str)> = vec![
            ("Name", challenge.name.as_str()),
            ("Score", &score_str),
            ("Tags", &tags),
            (
                "Status",
                if challenge.solved.unwrap_or(false) {
                    "Solved"
                } else if challenge.is_enabled.unwrap_or(true) {
                    "Unsolved"
                } else {
                    "Locked"
                },
            ),
        ];
        output::print_key_value(&pairs);

        if let Some(ref content) = challenge.content {
            println!();
            output::print_markdown(content);
        }

        // Attachments
        if let Some(ref attachments) = challenge.attachments {
            if let Some(arr) = attachments.as_array() {
                println!();
                println!("Attachments:");
                for att in arr {
                    if let Some(name) = att.get("name").and_then(|v| v.as_str()) {
                        println!("  • {name}");
                    }
                }
            }
        }

        // Instance
        if let Some(ref instance) = challenge.instance {
            println!();
            println!("Instance: {:?}", instance);
        }
    }

    Ok(())
}

pub async fn solve(
    client: &mut Client,
    config: &mut ClientConfig,
    args: SolveArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let game_id = resolve_game_id(client, config, profile_name, args.game.as_deref()).await?;
    let Some(game_id) = game_id else {
        return Err(CliError::Config("specify --game".to_owned()));
    };

    let challenge_id =
        resolve_challenge_id(client, config, profile_name, game_id, &args.challenge).await?;

    let flag = if let Some(f) = args.flag {
        f
    } else {
        rpassword::prompt_password("Flag: ")?
    };

    let path = format!("game/{game_id}/challenge/{challenge_id}/submit");
    let result = client
        .post_value(
            &path,
            &serde_json::json!({ "content": flag }),
            config,
            profile_name,
        )
        .await?;

    if json {
        output::print_json(&result);
    } else {
        let correct = result
            .get("correct")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let score = result.get("score").and_then(|v| v.as_i64());
        let info = result.get("result").and_then(|v| v.as_str());

        if correct {
            if let Some(score) = score {
                output::success(&format!("Correct! (+{score} pts)"));
            } else {
                output::success("Already solved");
            }
        } else {
            output::error(&format!(
                "Incorrect{}",
                info.map(|s| format!(": {s}")).unwrap_or_default()
            ));
        }
    }

    Ok(())
}

#[derive(Deserialize, Serialize)]
struct HintInfo {
    id: i64,
    content: Option<String>,
    cost: Option<i64>,
    unlocked: Option<bool>,
}

pub async fn hints(
    client: &mut Client,
    config: &mut ClientConfig,
    args: HintsArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let game_id = resolve_game_id(client, config, profile_name, args.game.as_deref()).await?;
    let Some(game_id) = game_id else {
        return Err(CliError::Config("specify --game".to_owned()));
    };

    let challenge_id =
        resolve_challenge_id(client, config, profile_name, game_id, &args.challenge).await?;

    let path = format!("game/{game_id}/challenge/{challenge_id}/hint");

    #[derive(Deserialize)]
    struct HintPage {
        data: Option<Vec<HintInfo>>,
        #[serde(default)]
        records: Option<Vec<HintInfo>>,
    }

    let response: HintPage = client.get(&path, &[], config, profile_name).await?;
    let hints = response.data.or(response.records).unwrap_or_default();

    if json {
        output::print_json(&hints);
    } else if hints.is_empty() {
        output::info("No hints available.");
    } else {
        #[derive(Tabled)]
        struct HintRow {
            #[tabled(rename = "ID")]
            id: i64,
            #[tabled(rename = "Content")]
            content: String,
            #[tabled(rename = "Cost")]
            cost: String,
            #[tabled(rename = "Status")]
            status: String,
        }

        let rows: Vec<HintRow> = hints
            .into_iter()
            .map(|h| HintRow {
                id: h.id,
                content: h.content.unwrap_or_default(),
                cost: h
                    .cost
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "—".to_owned()),
                status: if h.unlocked.unwrap_or(false) {
                    "Unlocked".to_owned()
                } else {
                    "Locked".to_owned()
                },
            })
            .collect();
        output::print_table(&rows);
    }

    Ok(())
}

pub async fn hint(
    client: &mut Client,
    config: &mut ClientConfig,
    args: HintArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let game_id = resolve_game_id(client, config, profile_name, args.game.as_deref()).await?;
    let Some(game_id) = game_id else {
        return Err(CliError::Config("specify --game".to_owned()));
    };

    let challenge_id =
        resolve_challenge_id(client, config, profile_name, game_id, &args.challenge).await?;

    let path = format!("game/{game_id}/challenge/{challenge_id}/hint/unlock");
    let body = if let Some(hint_id) = args.id {
        serde_json::json!({ "id": hint_id })
    } else {
        // List available hints
        let list_path = format!("game/{game_id}/challenge/{challenge_id}/hint");

        #[derive(Deserialize)]
        struct HintPage {
            data: Option<Vec<HintInfo>>,
            #[serde(default)]
            records: Option<Vec<HintInfo>>,
        }

        let response: HintPage = client.get(&list_path, &[], config, profile_name).await?;
        let hints = response.data.or(response.records).unwrap_or_default();

        let locked: Vec<_> = hints
            .iter()
            .filter(|h| !h.unlocked.unwrap_or(false))
            .collect();

        if locked.is_empty() {
            output::info("All hints are already unlocked.");
            return Ok(());
        }

        println!("Available locked hints:");
        for h in &locked {
            println!(
                "  ID: {} | Cost: {} | {}",
                h.id,
                h.cost
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "?".to_owned()),
                h.content.as_deref().unwrap_or("(no preview)"),
            );
        }

        let hint_id_str = rpassword::prompt_password("Hint ID to unlock: ")?;
        let hint_id: i64 = hint_id_str
            .parse()
            .map_err(|_| CliError::Config("invalid hint ID".to_owned()))?;

        serde_json::json!({ "id": hint_id })
    };

    let result = client
        .post_value(&path, &body, config, profile_name)
        .await?;

    if json {
        output::print_json(&result);
    } else {
        let content = result
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("Hint unlocked");
        output::success(content);
    }

    Ok(())
}

pub async fn start(
    client: &mut Client,
    config: &mut ClientConfig,
    args: StartArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let game_id = resolve_game_id(client, config, profile_name, args.game.as_deref()).await?;
    let Some(game_id) = game_id else {
        return Err(CliError::Config("specify --game".to_owned()));
    };

    let challenge_id =
        resolve_challenge_id(client, config, profile_name, game_id, &args.challenge).await?;

    let path = format!("game/{game_id}/challenge/{challenge_id}/instance");
    let result = client
        .post_value(&path, &serde_json::json!({}), config, profile_name)
        .await?;

    if json {
        output::print_json(&result);
    } else {
        output::success("Instance started");

        if let Some(env) = result.get("env") {
            if let Some(obj) = env.as_object() {
                let pairs: Vec<(&str, &str)> = obj
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str().unwrap_or("")))
                    .collect();
                output::print_key_value(&pairs);
            }
        }
    }

    Ok(())
}

pub async fn stop(
    client: &mut Client,
    config: &mut ClientConfig,
    args: StopArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let game_id = resolve_game_id(client, config, profile_name, args.game.as_deref()).await?;
    let Some(game_id) = game_id else {
        return Err(CliError::Config("specify --game".to_owned()));
    };

    let challenge_id =
        resolve_challenge_id(client, config, profile_name, game_id, &args.challenge).await?;

    let path = format!("game/{game_id}/challenge/{challenge_id}/instance");
    let result = client
        .delete_value(&path, config, profile_name)
        .await?;

    if json {
        output::print_json(&result);
    } else {
        output::success("Instance stopped");
    }

    Ok(())
}

pub async fn download(
    client: &mut Client,
    config: &mut ClientConfig,
    args: DownloadArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let game_id = resolve_game_id(client, config, profile_name, args.game.as_deref()).await?;
    let Some(game_id) = game_id else {
        return Err(CliError::Config("specify --game".to_owned()));
    };

    let challenge_id =
        resolve_challenge_id(client, config, profile_name, game_id, &args.challenge).await?;

    let output_path = match args.output {
        Some(ref p) => PathBuf::from(p),
        None => {
            let chal_path = format!("game/{game_id}/challenge/{challenge_id}");
            let challenge: ChallengeInfo =
                client.get(&chal_path, &[], config, profile_name).await?;
            PathBuf::from(format!("{}.zip", challenge.name))
        }
    };

    let path = format!("game/{game_id}/challenge/{challenge_id}/file");

    if json {
        output::info(&format!(
            "Downloading attachments to {}",
            output_path.display()
        ));
    }

    client
        .download(&path, &output_path, config, profile_name)
        .await?;

    if !json {
        output::success(&format!(
            "Downloaded attachments to {}",
            output_path.display()
        ));
    }

    Ok(())
}

fn format_tags(tags: &Option<Value>) -> String {
    match tags {
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        _ => "—".to_owned(),
    }
}
