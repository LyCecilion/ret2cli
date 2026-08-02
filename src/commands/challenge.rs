use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use tabled::Tabled;

use crate::{
    cli::{ChallengeArgs, DownloadArgs, GameContextArgs, SubmitArgs, UnlockHintArgs},
    client::Client,
    commands::require_or_input,
    config::ClientConfig,
    error::{CliError, CliResult},
    output, resolve_challenge_id, resolve_game_id,
};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChallengeInfo {
    pub id: i64,
    pub name: String,
    pub content: Option<String>,
    pub score: i32,
    pub tag: Vec<TagEntry>,
    pub score_rule: ScoreRule,
    pub bucket: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TagEntry {
    pub name: String,
    pub primary: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ScoreRule {
    pub initial: i32,
    pub minimum: i32,
    pub decay: i32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SubmissionInfo {
    pub id: i64,
    pub created_at: i64,
    pub challenge_id: i64,
    pub challenge_name: Option<String>,
    pub solved: Option<bool>,
    pub result: Option<String>,
    pub score: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HintInfo {
    pub id: i64,
    pub content: String,
    pub cost: i32,
}

impl HintInfo {
    pub fn locked(&self) -> bool {
        self.cost > 0 && self.content.is_empty()
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FileInfo {
    pub folder: String,
    pub file: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct InstanceInfo {
    pub state: String,
    pub renew_count: i32,
    /// Unix seconds when the instance was created; it expires `renew_count + 1` hours later.
    pub created_at: i64,
    pub challenge_id: i64,
}

pub async fn fetch_challenges(
    client: &mut Client,
    config: &mut ClientConfig,
    profile_name: Option<&str>,
    game_id: i64,
) -> CliResult<Vec<ChallengeInfo>> {
    let path = format!("game/{game_id}/challenge");
    let (items, _): (Vec<ChallengeInfo>, u64) =
        client.get(&path, &[], config, profile_name).await?;
    Ok(items)
}

pub async fn challenges(
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
        #[tabled(rename = "Score")]
        score: i32,
        #[tabled(rename = "Tags")]
        tags: String,
        #[tabled(rename = "Solved")]
        solved: String,
    }
    let game_id = required_game(client, config, profile_name, args.game.as_deref()).await?;
    let items = fetch_challenges(client, config, profile_name, game_id).await?;
    let solved = fetch_solved_ids(client, config, profile_name, game_id).await?;
    if json {
        output::print_json(&items);
        return Ok(());
    }
    let rows: Vec<_> = items
        .into_iter()
        .map(|c| Row {
            id: c.id,
            name: c.name,
            score: c.score,
            tags: tags(&c.tag),
            solved: if solved.contains(&c.id) { "✓" } else { "—" }.to_owned(),
        })
        .collect();
    output::print_table(&rows);
    Ok(())
}

pub async fn view(
    client: &mut Client,
    config: &mut ClientConfig,
    args: ChallengeArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let game_id = required_game(client, config, profile_name, args.game.as_deref()).await?;
    let id = resolve_challenge_id(client, config, profile_name, game_id, &args.challenge).await?;
    let path = format!("game/{game_id}/challenge/{id}");
    let item: ChallengeInfo = client.get(&path, &[], config, profile_name).await?;
    if json {
        output::print_json(&item);
        return Ok(());
    }
    let score = item.score.to_string();
    let tag_text = tags(&item.tag);
    let solved = fetch_solved_ids(client, config, profile_name, game_id).await?.contains(&id);
    output::print_key_value(&[
        ("Name", &item.name),
        ("Score", &score),
        ("Tags", &tag_text),
        ("Status", if solved { "Solved" } else { "Unsolved" }),
    ]);
    if let Some(content) = &item.content {
        output::blank();
        output::print_markdown(content);
    }
    let files = fetch_files(client, config, profile_name, game_id, id).await?;
    if !files.is_empty() {
        output::blank();
        output::info(&format!("{} attachment(s) available", files.len()));
    }
    Ok(())
}

pub async fn solve(
    client: &mut Client,
    config: &mut ClientConfig,
    args: SubmitArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let game_id = required_game(client, config, profile_name, args.game.as_deref()).await?;
    let challenge_id =
        resolve_challenge_id(client, config, profile_name, game_id, &args.challenge).await?;
    let flag = require_or_input(args.flag, "Flag", json)?;
    let path = format!("game/{game_id}/challenge/{challenge_id}/submit");
    let mut submission: SubmissionInfo =
        client.post(&path, &serde_json::json!({ "content": flag }), config, profile_name).await?;
    for attempt in 0..10 {
        if submission.solved.is_some() {
            break;
        }
        if attempt > 0 || submission.solved.is_none() {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        let id = submission.id.to_string();
        submission = client.get(&path, &[("id", &id)], config, profile_name).await?;
    }
    match submission.solved {
        Some(true) => {
            if json {
                output::print_json(&submission);
            } else {
                output::success(&format!(
                    "Correct: {}",
                    submission.result.as_deref().unwrap_or("accepted")
                ));
            }
            Ok(())
        }
        Some(false) => Err(CliError::Config(format!(
            "incorrect flag: {}",
            submission.result.as_deref().unwrap_or("rejected")
        ))),
        None => Err(CliError::Config(format!("submission {} is still pending", submission.id))),
    }
}

pub async fn fetch_hints(
    client: &mut Client,
    config: &mut ClientConfig,
    profile_name: Option<&str>,
    game_id: i64,
    challenge_id: i64,
) -> CliResult<Vec<HintInfo>> {
    let path = format!("game/{game_id}/challenge/{challenge_id}/hint");
    client.get(&path, &[], config, profile_name).await
}

pub async fn hints(
    client: &mut Client,
    config: &mut ClientConfig,
    args: ChallengeArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    #[derive(Tabled)]
    struct Row {
        #[tabled(rename = "ID")]
        id: i64,
        #[tabled(rename = "Cost")]
        cost: i32,
        #[tabled(rename = "Status")]
        status: String,
        #[tabled(rename = "Content")]
        content: String,
    }
    let (game_id, challenge_id) =
        resolve_context(client, config, profile_name, args.game.as_deref(), &args.challenge)
            .await?;
    let items = fetch_hints(client, config, profile_name, game_id, challenge_id).await?;
    if json {
        output::print_json(&items);
        return Ok(());
    }
    let rows: Vec<_> = items
        .into_iter()
        .map(|h| Row {
            id: h.id,
            cost: h.cost,
            status: if h.locked() { "Locked" } else { "Available" }.to_owned(),
            content: if h.locked() { "—".to_owned() } else { h.content },
        })
        .collect();
    output::print_table(&rows);
    Ok(())
}

pub async fn unlock_hint(
    client: &mut Client,
    config: &mut ClientConfig,
    args: UnlockHintArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let (game_id, challenge_id) =
        resolve_context(client, config, profile_name, args.game.as_deref(), &args.challenge)
            .await?;
    let id = if let Some(id) = args.id {
        id
    } else {
        let locked: Vec<_> = fetch_hints(client, config, profile_name, game_id, challenge_id)
            .await?
            .into_iter()
            .filter(HintInfo::locked)
            .collect();
        if locked.len() == 1 && !json {
            locked[0].id
        } else {
            return Err(CliError::Config("specify --id for the hint to unlock".to_owned()));
        }
    };
    let path = format!("game/{game_id}/challenge/{challenge_id}/hint/unlock");
    let _: serde_json::Value =
        client.post(&path, &serde_json::json!({"id": id}), config, profile_name).await?;
    let unlocked = fetch_hints(client, config, profile_name, game_id, challenge_id)
        .await?
        .into_iter()
        .find(|h| h.id == id)
        .ok_or_else(|| CliError::Config("unlocked hint was not returned".to_owned()))?;
    if json {
        output::print_json(&unlocked);
    } else {
        output::success(&format!("Hint unlocked: {}", unlocked.content));
    }
    Ok(())
}

pub async fn start(
    client: &mut Client,
    config: &mut ClientConfig,
    args: ChallengeArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    instance_action(client, config, args, json, profile_name, true).await
}
pub async fn stop(
    client: &mut Client,
    config: &mut ClientConfig,
    args: ChallengeArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    instance_action(client, config, args, json, profile_name, false).await
}

/// Outcome of a start/stop action, derived from the pre-check state.
#[derive(Clone, Copy)]
enum InstanceAction {
    Started,
    AlreadyStarted(Option<i64>),
    Stopped,
    NotRunning,
}

async fn instance_action(
    client: &mut Client,
    config: &mut ClientConfig,
    args: ChallengeArgs,
    json: bool,
    profile_name: Option<&str>,
    start: bool,
) -> CliResult<()> {
    let (game_id, challenge_id) =
        resolve_context(client, config, profile_name, args.game.as_deref(), &args.challenge)
            .await?;
    let path = format!("game/{game_id}/challenge/{challenge_id}/instance");
    let action = if start {
        let instances = fetch_instances(client, config, profile_name, game_id).await?;
        match find_instance(&instances, challenge_id) {
            Some(instance) => InstanceAction::AlreadyStarted(Some(remaining_seconds(instance))),
            None => {
                match client.post_value(&path, &serde_json::json!({}), config, profile_name).await {
                    Ok(_) => InstanceAction::Started,
                    // A concurrent start may have won the race; the pod now exists.
                    Err(err) if is_already_started(&err) => InstanceAction::AlreadyStarted(None),
                    Err(err) => return Err(err),
                }
            }
        }
    } else {
        let instances = fetch_instances(client, config, profile_name, game_id).await?;
        if find_instance(&instances, challenge_id).is_some() {
            client.delete(&path, config, profile_name).await?;
            InstanceAction::Stopped
        } else {
            InstanceAction::NotRunning
        }
    };
    let state = match action {
        InstanceAction::Started => "started",
        InstanceAction::AlreadyStarted(_) => "already_started",
        InstanceAction::Stopped => "stopped",
        InstanceAction::NotRunning => "not_running",
    };
    if json {
        output::print_json(&serde_json::json!({ "instance": state, "challenge_id": challenge_id }));
        return Ok(());
    }
    match action {
        InstanceAction::Started => output::success("Instance started"),
        InstanceAction::AlreadyStarted(Some(remaining)) => output::info(&format!(
            "Instance already started ({} remaining)",
            format_duration(remaining)
        )),
        InstanceAction::AlreadyStarted(None) => output::info("Instance already started"),
        InstanceAction::Stopped => output::success("Instance stopped"),
        InstanceAction::NotRunning => output::info("Instance is not running"),
    }
    Ok(())
}

pub async fn renew(
    client: &mut Client,
    config: &mut ClientConfig,
    args: ChallengeArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let (game_id, challenge_id) =
        resolve_context(client, config, profile_name, args.game.as_deref(), &args.challenge)
            .await?;
    let instances = fetch_instances(client, config, profile_name, game_id).await?;
    let Some(instance) = find_instance(&instances, challenge_id) else {
        if json {
            output::print_json(
                &serde_json::json!({ "instance": "not_running", "challenge_id": challenge_id }),
            );
        } else {
            output::info("Instance is not running");
        }
        return Ok(());
    };
    let path = format!("game/{game_id}/challenge/{challenge_id}/instance");
    client.patch_no_body(&path, &serde_json::json!({}), config, profile_name).await?;
    let remaining = remaining_seconds(instance) + 3600;
    if json {
        output::print_json(&serde_json::json!({
            "instance": "renewed",
            "challenge_id": challenge_id,
            "renew_count": instance.renew_count + 1,
            "remaining_seconds": remaining,
        }));
    } else {
        output::success(&format!("Instance renewed — {} remaining", format_duration(remaining)));
    }
    Ok(())
}

pub async fn status(
    client: &mut Client,
    config: &mut ClientConfig,
    args: ChallengeArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let (game_id, challenge_id) =
        resolve_context(client, config, profile_name, args.game.as_deref(), &args.challenge)
            .await?;
    let instances = fetch_instances(client, config, profile_name, game_id).await?;
    match find_instance(&instances, challenge_id) {
        Some(instance) => {
            let remaining = remaining_seconds(instance);
            if json {
                output::print_json(&serde_json::json!({
                    "challenge_id": challenge_id,
                    "instance": {
                        "state": instance.state,
                        "renew_count": instance.renew_count,
                        "remaining_seconds": remaining,
                    },
                }));
            } else {
                output::info(&format!(
                    "Instance is {} — {} remaining (renewed {}×)",
                    instance.state,
                    format_duration(remaining),
                    instance.renew_count,
                ));
            }
        }
        None => {
            if json {
                output::print_json(&serde_json::json!({
                    "challenge_id": challenge_id,
                    "instance": null,
                }));
            } else {
                output::info("Instance is not running");
            }
        }
    }
    Ok(())
}

/// List every instance of the current user and their team in the game.
pub async fn fetch_instances(
    client: &mut Client,
    config: &mut ClientConfig,
    profile_name: Option<&str>,
    game_id: i64,
) -> CliResult<Vec<InstanceInfo>> {
    let path = format!("game/{game_id}/instance");
    client.get(&path, &[], config, profile_name).await
}

fn find_instance(instances: &[InstanceInfo], challenge_id: i64) -> Option<&InstanceInfo> {
    instances.iter().find(|instance| instance.challenge_id == challenge_id)
}

/// Seconds until `created_at + (renew_count + 1) hours`, clamped at zero.
fn remaining_seconds(instance: &InstanceInfo) -> i64 {
    let expires_at = instance.created_at + i64::from(instance.renew_count + 1) * 3600;
    (expires_at - chrono::Utc::now().timestamp()).max(0)
}

fn format_duration(secs: i64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else if minutes > 0 {
        format!("{minutes}m")
    } else {
        format!("{secs}s")
    }
}

fn is_already_started(err: &CliError) -> bool {
    matches!(
        err,
        CliError::Api { status, message }
            if *status == reqwest::StatusCode::CONFLICT
                && message == "this challenge instance is already launched"
    )
}

pub async fn fetch_files(
    client: &mut Client,
    config: &mut ClientConfig,
    profile_name: Option<&str>,
    game_id: i64,
    challenge_id: i64,
) -> CliResult<Vec<FileInfo>> {
    let path = format!("game/{game_id}/challenge/{challenge_id}/file");
    client.get(&path, &[], config, profile_name).await
}

pub async fn files(
    client: &mut Client,
    config: &mut ClientConfig,
    args: ChallengeArgs,
    json: bool,
    profile_name: Option<&str>,
) -> CliResult<()> {
    let (game_id, challenge_id) =
        resolve_context(client, config, profile_name, args.game.as_deref(), &args.challenge)
            .await?;
    let items = fetch_files(client, config, profile_name, game_id, challenge_id).await?;
    if json {
        output::print_json(&items);
    } else {
        for file in items {
            output::line(&format!("{:<8} {}", file.folder, file.file));
        }
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
    let (game_id, challenge_id) =
        resolve_context(client, config, profile_name, args.game.as_deref(), &args.challenge)
            .await?;
    let all = fetch_files(client, config, profile_name, game_id, challenge_id).await?;
    let selected: Vec<_> = if let Some(name) = &args.file {
        let matches: Vec<_> = all.into_iter().filter(|f| &f.file == name).collect();
        if matches.len() != 1 {
            return Err(CliError::Config(format!("attachment '{name}' is missing or ambiguous")));
        }
        matches
    } else {
        all
    };
    if selected.is_empty() {
        return Err(CliError::Config("challenge has no downloadable attachments".to_owned()));
    }
    let single_output = args.file.is_some() && args.output.is_some();
    let base = args
        .output
        .as_ref()
        .map_or_else(|| PathBuf::from(safe_name(&args.challenge)), PathBuf::from);
    if !single_output {
        std::fs::create_dir_all(&base)?;
    }
    let path = format!("game/{game_id}/challenge/{challenge_id}/file");
    let mut downloaded = Vec::new();
    for item in selected {
        let filename = Path::new(&item.file)
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| CliError::Config("invalid attachment filename".to_owned()))?;
        let target = if single_output { base.clone() } else { base.join(filename) };
        client
            .download_query(
                &path,
                &[("folder", &item.folder), ("file", &item.file)],
                &target,
                config,
                profile_name,
                !json,
            )
            .await?;
        downloaded.push(target.display().to_string());
    }
    if json {
        output::print_json(&serde_json::json!({ "downloaded": downloaded }));
    } else {
        output::success(&format!("Downloaded {} attachment(s)", downloaded.len()));
    }
    Ok(())
}

async fn required_game(
    client: &mut Client,
    config: &mut ClientConfig,
    profile_name: Option<&str>,
    game: Option<&str>,
) -> CliResult<i64> {
    resolve_game_id(client, config, profile_name, game).await?.ok_or_else(|| {
        CliError::Config(
            "no game selected; run 'ret2cli game select <game>' or pass --game".to_owned(),
        )
    })
}
async fn resolve_context(
    client: &mut Client,
    config: &mut ClientConfig,
    profile_name: Option<&str>,
    game: Option<&str>,
    challenge: &str,
) -> CliResult<(i64, i64)> {
    let game_id = required_game(client, config, profile_name, game).await?;
    let challenge_id =
        resolve_challenge_id(client, config, profile_name, game_id, challenge).await?;
    Ok((game_id, challenge_id))
}
async fn fetch_solved_ids(
    client: &mut Client,
    config: &mut ClientConfig,
    profile_name: Option<&str>,
    game_id: i64,
) -> CliResult<HashSet<i64>> {
    let path = format!("game/{game_id}/solve");
    let solves: Vec<SubmissionInfo> = client.get(&path, &[], config, profile_name).await?;
    Ok(solves.into_iter().filter(|s| s.solved == Some(true)).map(|s| s.challenge_id).collect())
}
fn tags(tags: &[TagEntry]) -> String {
    let value =
        tags.iter().filter(|t| t.primary).map(|t| t.name.as_str()).collect::<Vec<_>>().join(", ");
    if value.is_empty() { "—".to_owned() } else { value }
}
fn safe_name(value: &str) -> String {
    let value: String = value
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') { c } else { '_' })
        .collect();
    if value.is_empty() { "attachments".to_owned() } else { value }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn parses_real_hint_shape() {
        let h: Vec<HintInfo> = serde_json::from_str(
            r#"[{"id":1,"created_at":0,"challenge_id":2,"content":"","cost":10}]"#,
        )
        .unwrap();
        assert!(h[0].locked());
    }
    #[test]
    fn parses_real_submission_shape() {
        let s: SubmissionInfo = serde_json::from_str(r#"{"id":1,"created_at":2,"user_id":3,"challenge_id":4,"team_id":null,"content":null,"solved":true,"result":"ok"}"#).unwrap();
        assert_eq!(s.solved, Some(true));
    }
    #[test]
    fn sanitizes_download_directory() {
        assert_eq!(safe_name("../a b"), ".._a_b");
    }
    #[test]
    fn finds_instance_by_challenge() {
        let instances = vec![
            InstanceInfo {
                state: "Running".to_owned(),
                renew_count: 1,
                created_at: 1_700_000_000,
                challenge_id: 933,
            },
            InstanceInfo {
                state: "Running".to_owned(),
                renew_count: 0,
                created_at: 1_700_000_000,
                challenge_id: 22,
            },
        ];
        assert_eq!(find_instance(&instances, 933).unwrap().renew_count, 1);
        assert!(find_instance(&instances, 42).is_none());
    }
    #[test]
    fn computes_remaining_time_from_renew_count() {
        let instance = InstanceInfo {
            state: "Running".to_owned(),
            renew_count: 1,
            created_at: chrono::Utc::now().timestamp() - 60,
            challenge_id: 933,
        };
        let remaining = remaining_seconds(&instance);
        assert!((6_900..=7_200).contains(&remaining), "remaining={remaining}");
    }
    #[test]
    fn remaining_time_is_clamped_at_zero() {
        let instance = InstanceInfo {
            state: "Running".to_owned(),
            renew_count: 0,
            created_at: chrono::Utc::now().timestamp() - 7200,
            challenge_id: 933,
        };
        assert_eq!(remaining_seconds(&instance), 0);
    }
    #[test]
    fn formats_duration() {
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(59), "59s");
        assert_eq!(format_duration(60), "1m");
        assert_eq!(format_duration(3720), "1h 02m");
        assert_eq!(format_duration(7200), "2h 00m");
    }
    #[test]
    fn recognizes_already_launched_conflict() {
        let conflict = CliError::Api {
            status: reqwest::StatusCode::CONFLICT,
            message: "this challenge instance is already launched".to_owned(),
        };
        assert!(is_already_started(&conflict));
        let other = CliError::Api {
            status: reqwest::StatusCode::PRECONDITION_FAILED,
            message: "please wait for rebuilding cargo crates".to_owned(),
        };
        assert!(!is_already_started(&other));
    }
}
