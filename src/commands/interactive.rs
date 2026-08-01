use std::{borrow::Cow, env, io};

use clap::{CommandFactory, Parser};
use rustyline::{
    Editor, Helper, completion::Completer, error::ReadlineError, highlight::Highlighter,
    hint::Hinter, history::DefaultHistory, validate::Validator,
};

use crate::{
    Cli,
    config::ClientConfig,
    error::{CliError, CliResult},
};

#[derive(Debug)]
struct ReplPrompt {
    plain: String,
    colored: String,
}

#[derive(Debug, Default)]
struct PromptHighlighter {
    colored_prompt: String,
}

impl Completer for PromptHighlighter {
    type Candidate = String;
}

impl Hinter for PromptHighlighter {
    type Hint = String;
}

impl Highlighter for PromptHighlighter {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        _prompt: &'p str,
        _default: bool,
    ) -> Cow<'b, str> {
        Cow::Borrowed(&self.colored_prompt)
    }
}

impl Validator for PromptHighlighter {}
impl Helper for PromptHighlighter {}

#[derive(Debug)]
enum ReplAction {
    Empty,
    Exit,
    Context,
    Help(Vec<String>),
    Command(Cli),
}

#[derive(Debug)]
enum ReplParseError {
    Arguments(String),
    Clap(clap::Error),
}

/// Run the command-oriented interactive shell.
///
/// Commands use exactly the same grammar as one-line `ret2cli` invocations,
/// without requiring the executable name at the beginning of each line.
pub async fn run(
    config: &mut ClientConfig,
    requested_profile: Option<&str>,
    default_url: Option<String>,
    default_token: Option<String>,
) -> CliResult<()> {
    if let Some(name) = requested_profile {
        if !config.profiles.contains_key(name) {
            return Err(CliError::Config(format!("profile '{name}' not found")));
        }
        name.clone_into(&mut config.active_profile);
        config.save()?;
    }

    print_banner(config)?;
    let mut editor = Editor::<PromptHighlighter, DefaultHistory>::new().map_err(readline_error)?;

    loop {
        let prompt = build_prompt(config)?;
        editor.set_helper(Some(PromptHighlighter { colored_prompt: prompt.colored }));
        match editor.readline(&prompt.plain) {
            Ok(line) => {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    let _ = editor.add_history_entry(trimmed);
                }

                let action = match parse_line(trimmed) {
                    Ok(action) => action,
                    Err(ReplParseError::Arguments(message)) => {
                        eprintln!("✗ {message}");
                        continue;
                    }
                    Err(ReplParseError::Clap(error)) => {
                        error.print().map_err(CliError::Io)?;
                        continue;
                    }
                };

                match action {
                    ReplAction::Empty => {}
                    ReplAction::Exit => break,
                    ReplAction::Context => print_context(config)?,
                    ReplAction::Help(path) => print_help(&path)?,
                    ReplAction::Command(mut cli) => {
                        if cli.url.is_none() {
                            cli.url.clone_from(&default_url);
                        }
                        if cli.token.is_none() {
                            cli.token.clone_from(&default_token);
                        }
                        if let Err(error) = crate::run_in_session(cli, config).await {
                            eprintln!("✗ {error}");
                        }
                    }
                }
            }
            Err(ReadlineError::Interrupted) => println!("KeyboardInterrupt"),
            Err(ReadlineError::Eof) => {
                println!();
                break;
            }
            Err(error) => return Err(readline_error(error)),
        }
    }

    Ok(())
}

fn parse_line(line: &str) -> Result<ReplAction, ReplParseError> {
    if line.is_empty() {
        return Ok(ReplAction::Empty);
    }

    let mut words = shell_words::split(line)
        .map_err(|error| ReplParseError::Arguments(format!("cannot parse command: {error}")))?;
    if words.first().is_some_and(|word| word == "ret2cli") {
        words.remove(0);
    }
    if words.is_empty() {
        return Ok(ReplAction::Empty);
    }

    match words.as_slice() {
        [command] if matches!(command.as_str(), "exit" | "exit()" | "quit" | "quit()") => {
            Ok(ReplAction::Exit)
        }
        [command] if command == "context" => Ok(ReplAction::Context),
        [command] if matches!(command.as_str(), "help" | "help()") => {
            Ok(ReplAction::Help(Vec::new()))
        }
        [command, path @ ..] if command == "help" => Ok(ReplAction::Help(path.to_vec())),
        _ => {
            let args = std::iter::once("ret2cli".to_owned()).chain(words);
            let cli = Cli::try_parse_from(args).map_err(ReplParseError::Clap)?;
            Ok(ReplAction::Command(cli))
        }
    }
}

fn print_banner(config: &ClientConfig) -> CliResult<()> {
    println!("Ret2CLI {} interactive shell", env!("CARGO_PKG_VERSION"));
    println!(
        "Type \"help\" for commands, \"context\" for the active context, or \"exit\" to leave."
    );
    if config.active_profile_resolved(None)?.url.is_empty() {
        println!(
            "No server URL is configured. Start with `profile add ... --use-now` or `account login --url ...`."
        );
    }
    Ok(())
}

fn build_prompt(config: &ClientConfig) -> CliResult<ReplPrompt> {
    let profile_name = config.active_profile_name(None)?;
    let profile = config.active_profile_resolved(None)?;
    let account = sanitize_prompt_segment(profile.active_account.as_deref().unwrap_or("anonymous"));
    let profile_name = sanitize_prompt_segment(&profile_name);
    let game = profile.game.as_ref().map_or_else(|| "none".to_owned(), |game| game.id.to_string());
    let plain = format!("{account}@{profile_name}:{game} $ ");
    let colored = if colors_enabled() {
        format!(
            "\x1b[1;32m{account}\x1b[0m\x1b[90m@\x1b[0m\x1b[1;36m{profile_name}\x1b[0m\x1b[90m:\x1b[0m\x1b[1;35m{game}\x1b[0m \x1b[1;90m$\x1b[0m "
        )
    } else {
        plain.clone()
    };
    Ok(ReplPrompt { plain, colored })
}

fn colors_enabled() -> bool {
    env::var_os("NO_COLOR").is_none()
        && match env::var("TERM") {
            Ok(term) => term != "dumb",
            Err(_) => true,
        }
}

fn sanitize_prompt_segment(value: &str) -> String {
    value.chars().map(|character| if character.is_control() { '?' } else { character }).collect()
}

fn print_context(config: &ClientConfig) -> CliResult<()> {
    let profile_name = config.active_profile_name(None)?;
    let profile = config.active_profile_resolved(None)?;
    println!(
        "profile={}  account={}  game={}",
        profile_name,
        profile.active_account.as_deref().unwrap_or("anonymous"),
        profile.game.as_ref().map_or_else(|| "none".to_owned(), ToString::to_string)
    );
    Ok(())
}

fn print_help(path: &[String]) -> CliResult<()> {
    if path.is_empty() {
        let mut command = Cli::command();
        command.print_long_help().map_err(CliError::Io)?;
        println!(
            "\n\nInteractive built-ins:\n  help [COMMAND...]  Show command help\n  context             Show the active profile, account, and game\n  exit, quit          Leave the interactive shell"
        );
        return Ok(());
    }

    let args = std::iter::once("ret2cli".to_owned())
        .chain(path.iter().cloned())
        .chain(std::iter::once("--help".to_owned()));
    match Cli::try_parse_from(args) {
        Ok(_) => Ok(()),
        Err(error) => error.print().map_err(CliError::Io),
    }
}

fn readline_error(error: ReadlineError) -> CliError {
    CliError::Io(io::Error::other(error))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{
        Commands,
        cli::{ChallengeCommand, GameCommand, TeamCommand},
    };

    #[test]
    fn parses_the_same_command_tree_without_an_executable_name() {
        let action =
            parse_line("game challenge submit 'shell basics' --flag 'flag{hello world}'").unwrap();
        assert!(matches!(
            action,
            ReplAction::Command(Cli {
                command: Some(Commands::Game {
                    command: GameCommand::Challenge { command: ChallengeCommand::Submit(_) }
                }),
                ..
            })
        ));
    }

    #[test]
    fn accepts_pasted_one_line_commands() {
        let action = parse_line("ret2cli game team show 'The A Team'").unwrap();
        assert!(matches!(
            action,
            ReplAction::Command(Cli {
                command: Some(Commands::Game {
                    command: GameCommand::Team { command: TeamCommand::Show(_) }
                }),
                ..
            })
        ));
    }

    #[test]
    fn recognizes_interpreter_built_ins() {
        assert!(matches!(parse_line("").unwrap(), ReplAction::Empty));
        assert!(
            matches!(parse_line("help game challenge submit").unwrap(), ReplAction::Help(path) if path == ["game", "challenge", "submit"])
        );
        assert!(matches!(parse_line("context").unwrap(), ReplAction::Context));
        assert!(matches!(parse_line("exit()").unwrap(), ReplAction::Exit));
    }

    #[test]
    fn reports_unclosed_quotes_without_exiting() {
        assert!(matches!(parse_line("team show 'unfinished"), Err(ReplParseError::Arguments(_))));
    }

    #[test]
    fn prompt_tracks_the_current_account_profile_and_game() {
        let mut config = ClientConfig::default();
        assert_eq!(build_prompt(&config).unwrap().plain, "anonymous@default:none $ ");

        let profile = config.active_profile_mut(None).unwrap();
        profile.active_account = Some("stellalyRin".to_owned());
        profile.game = Some(crate::config::SelectedGame { id: 31, name: "MoeCTF 2026".to_owned() });
        assert_eq!(build_prompt(&config).unwrap().plain, "stellalyRin@default:31 $ ");

        config.profiles.insert(
            "school".to_owned(),
            crate::config::ConnectionProfile::new("https://ctf.example".to_owned()),
        );
        config.active_profile = "school".to_owned();
        assert_eq!(build_prompt(&config).unwrap().plain, "anonymous@school:none $ ");
    }

    #[test]
    fn colored_prompt_keeps_every_context_segment_visible() {
        let mut config = ClientConfig::default();
        let profile = config.active_profile_mut(None).unwrap();
        profile.active_account = Some("alice".to_owned());
        profile.game = Some(crate::config::SelectedGame { id: 7, name: "Game".to_owned() });
        let prompt = build_prompt(&config).unwrap();
        for segment in ["alice", "@", "default", ":", "7", "$"] {
            assert!(prompt.colored.contains(segment));
        }
    }

    #[test]
    fn prompt_segments_cannot_inject_terminal_controls() {
        assert_eq!(sanitize_prompt_segment("safe\n\u{1b}[31m"), "safe??[31m");
    }
}
