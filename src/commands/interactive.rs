use std::io;

use clap::{CommandFactory, Parser};
use rustyline::{DefaultEditor, error::ReadlineError};

use crate::{
    Cli,
    config::ClientConfig,
    error::{CliError, CliResult},
};

const PROMPT: &str = ">>> ";

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
    let mut editor = DefaultEditor::new().map_err(readline_error)?;

    loop {
        match editor.readline(PROMPT) {
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
    print_context(config)?;
    if config.active_profile_resolved(None)?.url.is_empty() {
        println!(
            "No server URL is configured. Start with `profile add ... --use-now` or `account login --url ...`."
        );
    }
    Ok(())
}

fn print_context(config: &ClientConfig) -> CliResult<()> {
    let profile_name = config.active_profile_name(None)?;
    let profile = config.active_profile_resolved(None)?;
    println!(
        "profile={}  account={}  game={}",
        profile_name,
        profile.active_account.as_deref().unwrap_or("anonymous"),
        profile.game.as_deref().unwrap_or("none")
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
        cli::{ChallengeCommand, TeamCommand},
    };

    #[test]
    fn parses_the_same_command_tree_without_an_executable_name() {
        let action =
            parse_line("challenge submit 'shell basics' --flag 'flag{hello world}'").unwrap();
        assert!(matches!(
            action,
            ReplAction::Command(Cli {
                command: Some(Commands::Challenge { command: ChallengeCommand::Submit(_) }),
                ..
            })
        ));
    }

    #[test]
    fn accepts_pasted_one_line_commands() {
        let action = parse_line("ret2cli team show 'The A Team'").unwrap();
        assert!(matches!(
            action,
            ReplAction::Command(Cli {
                command: Some(Commands::Team { command: TeamCommand::Show(_) }),
                ..
            })
        ));
    }

    #[test]
    fn recognizes_interpreter_built_ins() {
        assert!(matches!(parse_line("").unwrap(), ReplAction::Empty));
        assert!(
            matches!(parse_line("help challenge submit").unwrap(), ReplAction::Help(path) if path == ["challenge", "submit"])
        );
        assert!(matches!(parse_line("context").unwrap(), ReplAction::Context));
        assert!(matches!(parse_line("exit()").unwrap(), ReplAction::Exit));
    }

    #[test]
    fn reports_unclosed_quotes_without_exiting() {
        assert!(matches!(parse_line("team show 'unfinished"), Err(ReplParseError::Arguments(_))));
    }
}
