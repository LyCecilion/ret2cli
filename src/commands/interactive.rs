use dialoguer::{Confirm, Input, MultiSelect, Password, Select};

use crate::{
    cli::{
        ChallengeArgs, DownloadArgs, GameContextArgs, LoginArgs, ProfileAddArgs, RegisterArgs,
        SubmitArgs, TeamCreateArgs, TeamJoinArgs, TeamLeaveArgs, TeamShowArgs, UnlockHintArgs,
    },
    client::Client,
    commands::{self, challenge, game, submission, team},
    config::ClientConfig,
    error::{CliError, CliResult},
};

pub async fn run(config: &mut ClientConfig, requested_profile: Option<&str>) -> CliResult<()> {
    if let Some(name) = requested_profile {
        if !config.profiles.contains_key(name) {
            return Err(CliError::Config(format!("profile '{name}' not found")));
        }
        config.active_profile = name.to_owned();
        config.save()?;
    }
    ensure_url(config)?;
    if config.active_profile_resolved(None)?.token.is_none() {
        match select(
            "This profile is not logged in",
            &["Login", "Register", "Continue without login"],
        )? {
            0 => {
                let account = input("Account")?;
                let password =
                    Password::new().with_prompt("Password").interact().map_err(dialoguer_error)?;
                with_client(config, |c, cfg| {
                    Box::pin(commands::auth::login(
                        c,
                        cfg,
                        LoginArgs { account: Some(account), password: Some(password) },
                        false,
                        None,
                    ))
                })
                .await?;
            }
            1 => {
                let account = input("Account")?;
                let nickname = input("Nickname")?;
                let email = input("Email")?;
                let password =
                    Password::new().with_prompt("Password").interact().map_err(dialoguer_error)?;
                with_client(config, |c, cfg| {
                    Box::pin(commands::auth::register(
                        c,
                        cfg,
                        RegisterArgs {
                            account: Some(account),
                            nickname: Some(nickname),
                            email: Some(email),
                            password: Some(password),
                        },
                        false,
                        None,
                    ))
                })
                .await?;
            }
            _ => {}
        }
    }
    loop {
        let profile = config.active_profile_resolved(None)?;
        println!(
            "\nret2cli  profile={}  game={}",
            config.active_profile,
            profile.game.as_deref().unwrap_or("none")
        );
        let choice = select(
            "Main menu",
            &[
                "Account",
                "Profiles",
                "Select game",
                "Challenges",
                "Scoreboard",
                "Teams",
                "Submissions",
                "Exit",
            ],
        )?;
        let result = match choice {
            0 => account_menu(config).await,
            1 => profile_menu(config),
            2 => select_game(config).await,
            3 => challenge_menu(config).await,
            4 => {
                with_client(config, |client, config| {
                    Box::pin(game::scoreboard(
                        client,
                        config,
                        GameContextArgs::default(),
                        false,
                        None,
                    ))
                })
                .await
            }
            5 => team_menu(config).await,
            6 => {
                with_client(config, |client, config| {
                    Box::pin(submission::submissions(
                        client,
                        config,
                        GameContextArgs::default(),
                        false,
                        None,
                    ))
                })
                .await
            }
            _ => return Ok(()),
        };
        if let Err(error) = result {
            eprintln!("✗ {error}");
        }
    }
}

fn ensure_url(config: &mut ClientConfig) -> CliResult<()> {
    if !config.active_profile_resolved(None)?.url.is_empty() {
        return Ok(());
    }
    let url: String =
        Input::new().with_prompt("Ret2Shell URL").interact_text().map_err(dialoguer_error)?;
    reqwest::Url::parse(&url).map_err(|e| CliError::Config(format!("invalid URL: {e}")))?;
    config.active_profile_mut(None)?.url = url;
    config.save()
}

async fn account_menu(config: &mut ClientConfig) -> CliResult<()> {
    loop {
        match select("Account", &["Status", "Show account", "Login", "Register", "Logout", "Back"])?
        {
            0 => {
                with_client(config, |c, cfg| Box::pin(commands::auth::status(c, cfg, false, None)))
                    .await?
            }
            1 => {
                with_client(config, |c, cfg| Box::pin(commands::auth::show(c, cfg, false, None)))
                    .await?
            }
            2 => {
                let account: String =
                    Input::new().with_prompt("Account").interact_text().map_err(dialoguer_error)?;
                let password =
                    Password::new().with_prompt("Password").interact().map_err(dialoguer_error)?;
                with_client(config, |c, cfg| {
                    Box::pin(commands::auth::login(
                        c,
                        cfg,
                        LoginArgs { account: Some(account), password: Some(password) },
                        false,
                        None,
                    ))
                })
                .await?;
            }
            3 => {
                let account = input("Account")?;
                let nickname = input("Nickname")?;
                let email = input("Email")?;
                let password =
                    Password::new().with_prompt("Password").interact().map_err(dialoguer_error)?;
                with_client(config, |c, cfg| {
                    Box::pin(commands::auth::register(
                        c,
                        cfg,
                        RegisterArgs {
                            account: Some(account),
                            nickname: Some(nickname),
                            email: Some(email),
                            password: Some(password),
                        },
                        false,
                        None,
                    ))
                })
                .await?;
            }
            4 => {
                with_client(config, |c, cfg| Box::pin(commands::auth::logout(c, cfg, false, None)))
                    .await?
            }
            _ => return Ok(()),
        }
    }
}

fn profile_menu(config: &mut ClientConfig) -> CliResult<()> {
    loop {
        match select("Profiles", &["List", "Add", "Switch", "Remove", "Back"])? {
            0 => commands::profile_list(config, false),
            1 => {
                let name = input("Profile name")?;
                let url = input("Ret2Shell URL")?;
                commands::profile_add(config, ProfileAddArgs { name, url, use_now: false }, false)?;
            }
            2 => {
                let mut names: Vec<_> = config.profiles.keys().cloned().collect();
                names.sort();
                let idx = Select::new()
                    .with_prompt("Use profile")
                    .items(&names)
                    .interact()
                    .map_err(dialoguer_error)?;
                commands::profile_use(config, &names[idx], false)?;
                ensure_url(config)?;
            }
            3 => {
                let names: Vec<_> = config
                    .profiles
                    .keys()
                    .filter(|n| n.as_str() != "default" && *n != &config.active_profile)
                    .cloned()
                    .collect();
                if names.is_empty() {
                    println!("No removable profiles");
                    continue;
                }
                let idx = Select::new()
                    .with_prompt("Remove profile")
                    .items(&names)
                    .interact()
                    .map_err(dialoguer_error)?;
                commands::profile_remove(
                    config,
                    crate::cli::ProfileRemoveArgs { name: names[idx].clone(), yes: false },
                    false,
                )?;
            }
            _ => return Ok(()),
        }
    }
}

async fn select_game(config: &mut ClientConfig) -> CliResult<()> {
    #[derive(serde::Deserialize)]
    struct Item {
        id: i64,
        name: String,
    }
    let items: Vec<Item> = with_client_value(config, |client, config| {
        Box::pin(async move {
            let (items, _): (Vec<Item>, u64) =
                client.get("game", &[("page_size", "100")], config, None).await?;
            Ok(items)
        })
    })
    .await?;
    if items.is_empty() {
        return Err(CliError::Config("no games available".to_owned()));
    }
    let labels: Vec<_> = items.iter().map(|g| format!("{}  {}", g.id, g.name)).collect();
    let idx = Select::new()
        .with_prompt("Select game")
        .items(&labels)
        .interact()
        .map_err(dialoguer_error)?;
    with_client(config, |c, cfg| {
        Box::pin(game::use_game(c, cfg, items[idx].id.to_string(), None, false))
    })
    .await
}

async fn challenge_menu(config: &mut ClientConfig) -> CliResult<()> {
    let game_id = current_game_id(config)?;
    loop {
        let items = with_client_value(config, move |c, cfg| {
            Box::pin(challenge::fetch_challenges(c, cfg, None, game_id))
        })
        .await?;
        if items.is_empty() {
            return Err(CliError::Config("no challenges available".to_owned()));
        }
        let mut labels: Vec<_> =
            items.iter().map(|c| format!("{}  {}  [{}]", c.id, c.name, c.score)).collect();
        labels.push("Back".to_owned());
        let idx = Select::new()
            .with_prompt("Challenge")
            .items(&labels)
            .interact()
            .map_err(dialoguer_error)?;
        if idx == items.len() {
            return Ok(());
        }
        let item = &items[idx];
        challenge_actions(config, item.id.to_string()).await?;
    }
}

async fn challenge_actions(config: &mut ClientConfig, challenge_id: String) -> CliResult<()> {
    loop {
        let choice = select(
            "Challenge actions",
            &[
                "Show",
                "Submit flag",
                "Hints",
                "Unlock hint",
                "Start instance",
                "Stop instance",
                "Files",
                "Download",
                "Back",
            ],
        )?;
        let args = || ChallengeArgs { challenge: challenge_id.clone(), game: None };
        match choice {
            0 => {
                with_client(config, |c, cfg| Box::pin(challenge::view(c, cfg, args(), false, None)))
                    .await?
            }
            1 => {
                let flag = input("Flag")?;
                with_client(config, |c, cfg| {
                    Box::pin(challenge::solve(
                        c,
                        cfg,
                        SubmitArgs {
                            challenge: challenge_id.clone(),
                            flag: Some(flag),
                            game: None,
                        },
                        false,
                        None,
                    ))
                })
                .await?;
            }
            2 => {
                with_client(config, |c, cfg| {
                    Box::pin(challenge::hints(c, cfg, args(), false, None))
                })
                .await?
            }
            3 => {
                let id: i64 =
                    Input::new().with_prompt("Hint ID").interact_text().map_err(dialoguer_error)?;
                with_client(config, |c, cfg| {
                    Box::pin(challenge::unlock_hint(
                        c,
                        cfg,
                        UnlockHintArgs {
                            challenge: challenge_id.clone(),
                            id: Some(id),
                            game: None,
                        },
                        false,
                        None,
                    ))
                })
                .await?;
            }
            4 => {
                with_client(config, |c, cfg| {
                    Box::pin(challenge::start(c, cfg, args(), false, None))
                })
                .await?
            }
            5 => {
                with_client(config, |c, cfg| Box::pin(challenge::stop(c, cfg, args(), false, None)))
                    .await?
            }
            6 => {
                with_client(config, |c, cfg| {
                    Box::pin(challenge::files(c, cfg, args(), false, None))
                })
                .await?
            }
            7 => interactive_download(config, challenge_id.clone()).await?,
            _ => return Ok(()),
        }
    }
}

async fn interactive_download(config: &mut ClientConfig, challenge_id: String) -> CliResult<()> {
    let game_id = current_game_id(config)?;
    let parsed_challenge_id =
        challenge_id.parse().map_err(|_| CliError::Config("invalid challenge ID".to_owned()))?;
    let files = with_client_value(config, move |c, cfg| {
        Box::pin(challenge::fetch_files(c, cfg, None, game_id, parsed_challenge_id))
    })
    .await?;
    if files.is_empty() {
        return Err(CliError::Config("no attachments available".to_owned()));
    }
    let labels: Vec<_> = files.iter().map(|f| format!("{} / {}", f.folder, f.file)).collect();
    let chosen = MultiSelect::new()
        .with_prompt("Download attachments")
        .items(&labels)
        .interact()
        .map_err(dialoguer_error)?;
    for idx in chosen {
        let file = files[idx].file.clone();
        with_client(config, |c, cfg| {
            Box::pin(challenge::download(
                c,
                cfg,
                DownloadArgs {
                    challenge: challenge_id.clone(),
                    file: Some(file),
                    output: None,
                    game: None,
                },
                false,
                None,
            ))
        })
        .await?;
    }
    Ok(())
}

async fn team_menu(config: &mut ClientConfig) -> CliResult<()> {
    loop {
        match select(
            "Teams",
            &["My team", "List teams", "Show team", "Create", "Join", "Leave", "Back"],
        )? {
            0 => {
                with_client(config, |c, cfg| {
                    Box::pin(team::my(c, cfg, GameContextArgs::default(), false, None))
                })
                .await?
            }
            1 => {
                with_client(config, |c, cfg| {
                    Box::pin(team::teams(c, cfg, GameContextArgs::default(), false, None))
                })
                .await?
            }
            2 => {
                let name = input("Team name or ID")?;
                with_client(config, |c, cfg| {
                    Box::pin(team::team(
                        c,
                        cfg,
                        TeamShowArgs { team: name, game: None },
                        false,
                        None,
                    ))
                })
                .await?;
            }
            3 => {
                let name = input("Team name")?;
                let tag: String = Input::new()
                    .with_prompt("Tag (optional)")
                    .allow_empty(true)
                    .interact_text()
                    .map_err(dialoguer_error)?;
                with_client(config, |c, cfg| {
                    Box::pin(team::team_create(
                        c,
                        cfg,
                        TeamCreateArgs {
                            name: Some(name),
                            tag: (!tag.is_empty()).then_some(tag),
                            game: None,
                        },
                        false,
                        None,
                    ))
                })
                .await?;
            }
            4 => {
                let token = input("Invitation token")?;
                with_client(config, |c, cfg| {
                    Box::pin(team::team_join(
                        c,
                        cfg,
                        TeamJoinArgs { token: Some(token), game: None },
                        false,
                        None,
                    ))
                })
                .await?;
            }
            5 => {
                if Confirm::new()
                    .with_prompt("Leave your current team?")
                    .default(false)
                    .interact()
                    .map_err(dialoguer_error)?
                {
                    with_client(config, |c, cfg| {
                        Box::pin(team::team_leave(
                            c,
                            cfg,
                            TeamLeaveArgs { game: None, yes: true },
                            false,
                            None,
                        ))
                    })
                    .await?;
                }
            }
            _ => return Ok(()),
        }
    }
}

fn current_game_id(config: &ClientConfig) -> CliResult<i64> {
    config
        .active_profile_resolved(None)?
        .game
        .as_deref()
        .ok_or_else(|| CliError::Config("select a game first".to_owned()))?
        .parse()
        .map_err(|_| {
            CliError::Config("interactive mode requires a selected numeric game ID".to_owned())
        })
}
fn select(prompt: &str, items: &[&str]) -> CliResult<usize> {
    Select::new().with_prompt(prompt).items(items).interact().map_err(dialoguer_error)
}
fn input(prompt: &str) -> CliResult<String> {
    Input::new().with_prompt(prompt).interact_text().map_err(dialoguer_error)
}
fn dialoguer_error(error: dialoguer::Error) -> CliError {
    CliError::Io(std::io::Error::other(error))
}
fn make_client(config: &ClientConfig) -> CliResult<Client> {
    let p = config.active_profile_resolved(None)?;
    Client::new(p.url.clone(), p.token.clone())
}

async fn with_client<F>(config: &mut ClientConfig, f: F) -> CliResult<()>
where
    F: for<'a> FnOnce(
        &'a mut Client,
        &'a mut ClientConfig,
    )
        -> std::pin::Pin<Box<dyn std::future::Future<Output = CliResult<()>> + 'a>>,
{
    let mut client = make_client(config)?;
    f(&mut client, config).await
}
async fn with_client_value<T, F>(config: &mut ClientConfig, f: F) -> CliResult<T>
where
    F: for<'a> FnOnce(
        &'a mut Client,
        &'a mut ClientConfig,
    )
        -> std::pin::Pin<Box<dyn std::future::Future<Output = CliResult<T>> + 'a>>,
{
    let mut client = make_client(config)?;
    f(&mut client, config).await
}
