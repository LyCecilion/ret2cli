use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
#[command(author, version, about = "CLI client for Ret2Shell CTF platform")]
pub struct Cli {
    /// Emit one JSON value on stdout
    #[arg(long, global = true)]
    pub json: bool,

    /// Use a named local connection profile
    #[arg(long, global = true)]
    pub profile: Option<String>,

    /// Override the API base URL for this invocation
    #[arg(long, global = true)]
    pub url: Option<String>,

    /// Override the bearer token for this invocation
    #[arg(long, global = true)]
    pub token: Option<String>,

    /// Control paging for human-readable command output
    #[arg(long, global = true, value_enum)]
    pub pager: Option<PagerMode>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Manage local `Ret2Shell` connection profiles
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
    },
    /// Account authentication and identity
    Account {
        #[command(subcommand)]
        command: AccountCommand,
    },
    /// Work with games and their challenges, teams, and submissions
    Game {
        #[command(subcommand)]
        command: GameCommand,
    },
    /// Open the interpreter-style interactive shell
    Interactive,
    /// Generate shell completions
    Completion(CompletionArgs),
}

#[derive(Subcommand, Debug)]
pub enum AccountCommand {
    /// List accounts saved in the selected connection profile
    List,
    /// Log in and save or replace an account session
    Login(LoginArgs),
    /// Log out and remove the active account session
    Logout,
    /// Register an account on the selected `Ret2Shell` instance
    Register(RegisterArgs),
    /// Check whether the active session is alive on the server
    Ping,
    /// Show the active account's server-side profile
    Show,
    /// Edit the profile description or avatar
    Edit(AccountEditArgs),
    /// Generate a sensitive temporary identity code
    Code(AccountCodeArgs),
    /// Switch the active account within the selected connection profile
    Use { account: String },
    /// Remove a saved account session without contacting the server
    Remove(AccountRemoveArgs),
}

#[derive(Subcommand, Debug)]
pub enum ProfileCommand {
    List,
    Show { name: Option<String> },
    Add(ProfileAddArgs),
    Use { name: String },
    Remove(ProfileRemoveArgs),
}

#[derive(Subcommand, Debug)]
pub enum GameCommand {
    /// List available games
    List(GameListArgs),
    /// Show one game and its introduction, rules, or cover image
    Show(GameShowArgs),
    /// Save the selected game in the current profile
    Select { game: String },
    /// Show the selected game's scoreboard
    Scoreboard(GameContextArgs),
    /// Work with challenges in the selected game
    Challenge {
        #[command(subcommand)]
        command: ChallengeCommand,
    },
    /// Manage teams in the selected game
    Team {
        #[command(subcommand)]
        command: TeamCommand,
    },
    /// View submission history
    Submission {
        #[command(subcommand)]
        command: SubmissionCommand,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PagerMode {
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Subcommand, Debug)]
pub enum ChallengeCommand {
    List(GameContextArgs),
    Show(ChallengeArgs),
    Submit(SubmitArgs),
    Hints(ChallengeArgs),
    UnlockHint(UnlockHintArgs),
    Start(ChallengeArgs),
    Stop(ChallengeArgs),
    Files(ChallengeArgs),
    Download(DownloadArgs),
}

#[derive(Subcommand, Debug)]
pub enum TeamCommand {
    List(GameContextArgs),
    Show(TeamShowArgs),
    Create(TeamCreateArgs),
    Update(TeamUpdateArgs),
    Join(TeamJoinArgs),
    Leave(TeamLeaveArgs),
}

#[derive(Subcommand, Debug)]
pub enum SubmissionCommand {
    List(GameContextArgs),
}

#[derive(Args, Debug, Clone)]
pub struct LoginArgs {
    #[arg(long)]
    pub account: Option<String>,
    /// Insecure on shared systems; omit to use a hidden TTY prompt
    #[arg(long)]
    pub password: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct RegisterArgs {
    #[arg(long)]
    pub account: Option<String>,
    #[arg(long)]
    pub nickname: Option<String>,
    #[arg(long)]
    pub email: Option<String>,
    #[arg(long)]
    pub password: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct AccountRemoveArgs {
    /// Saved account name
    pub account: String,
    /// Skip the confirmation prompt
    #[arg(long)]
    pub yes: bool,
}

#[derive(Args, Debug, Clone, Default)]
pub struct AccountEditArgs {
    /// Set the personal introduction directly
    #[arg(long, conflicts_with = "description_file")]
    pub description: Option<String>,
    /// Read the personal introduction from PATH, or stdin with '-'
    #[arg(long, value_name = "PATH", conflicts_with = "description")]
    pub description_file: Option<String>,
    /// Upload a new avatar (maximum 10 MiB)
    #[arg(long, value_name = "PATH", conflicts_with = "remove_avatar")]
    pub avatar: Option<std::path::PathBuf>,
    /// Remove the current avatar
    #[arg(long, conflicts_with = "avatar")]
    pub remove_avatar: bool,
    /// Submit without confirmation
    #[arg(long)]
    pub yes: bool,
}

impl AccountEditArgs {
    #[must_use]
    pub fn has_explicit_change(&self) -> bool {
        self.description.is_some()
            || self.description_file.is_some()
            || self.avatar.is_some()
            || self.remove_avatar
    }
}

#[derive(Args, Debug, Clone, Default)]
pub struct AccountCodeArgs {
    /// Confirm that the code grants temporary access to your identity
    #[arg(long)]
    pub yes: bool,
}

#[derive(Args, Debug, Clone)]
pub struct ProfileAddArgs {
    pub name: String,
    #[arg(long)]
    pub url: String,
    #[arg(long)]
    pub use_now: bool,
}

#[derive(Args, Debug, Clone)]
pub struct ProfileRemoveArgs {
    pub name: String,
    #[arg(long)]
    pub yes: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum GameType {
    Training,
    Game,
}

impl GameType {
    pub fn api_value(self) -> &'static str {
        match self {
            Self::Training => "0",
            Self::Game => "1",
        }
    }
}

#[derive(Args, Debug, Clone, Default)]
pub struct GameShowArgs {
    /// Game id, name, or unique prefix; defaults to the selected game
    pub game: Option<String>,
    /// Show the detailed introduction (readme document)
    #[arg(long, conflicts_with_all = ["rules", "cover"])]
    pub intro: bool,
    /// Show the participation rules document
    #[arg(long, conflicts_with_all = ["intro", "cover"])]
    pub rules: bool,
    /// Render the cover image inline (Kitty or iTerm2 terminals)
    #[arg(long, conflicts_with_all = ["intro", "rules"])]
    pub cover: bool,
}

#[derive(Args, Debug, Clone)]
pub struct GameListArgs {
    #[arg(long, default_value_t = 1)]
    pub page: u32,
    #[arg(long, default_value_t = 20)]
    pub page_size: u32,
    #[arg(long, value_enum)]
    pub r#type: Option<GameType>,
}

#[derive(Args, Debug, Clone, Default)]
pub struct GameContextArgs {
    /// Override the selected game by ID, exact name, or unique prefix
    #[arg(long)]
    pub game: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct ChallengeArgs {
    pub challenge: String,
    #[arg(long)]
    pub game: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct SubmitArgs {
    pub challenge: String,
    #[arg(long)]
    pub flag: Option<String>,
    #[arg(long)]
    pub game: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct UnlockHintArgs {
    pub challenge: String,
    #[arg(long)]
    pub id: Option<i64>,
    #[arg(long)]
    pub game: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct DownloadArgs {
    pub challenge: String,
    #[arg(long)]
    pub file: Option<String>,
    /// Output directory, or an output file when --file selects one attachment
    #[arg(long)]
    pub output: Option<String>,
    #[arg(long)]
    pub game: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct TeamShowArgs {
    /// Team name as one or more words, a numeric ID, or the reserved word 'mine'
    #[arg(required = true, num_args = 1..)]
    pub team: Vec<String>,
    #[arg(long)]
    pub game: Option<String>,
}

impl TeamShowArgs {
    #[must_use]
    pub fn team_name(&self) -> String {
        self.team.join(" ")
    }

    #[must_use]
    pub fn is_mine(&self) -> bool {
        self.team.len() == 1 && self.team[0].eq_ignore_ascii_case("mine")
    }
}

#[derive(Args, Debug, Clone)]
pub struct TeamCreateArgs {
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long)]
    pub game: Option<String>,
    /// Skip the rules confirmation prompt
    #[arg(long)]
    pub yes: bool,
}

#[derive(Args, Debug, Clone)]
pub struct TeamUpdateArgs {
    /// New team name
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub game: Option<String>,
    /// Skip the confirmation prompt
    #[arg(long)]
    pub yes: bool,
}

#[derive(Args, Debug, Clone)]
pub struct TeamJoinArgs {
    pub token: Option<String>,
    #[arg(long)]
    pub game: Option<String>,
    /// Skip the rules confirmation prompt
    #[arg(long)]
    pub yes: bool,
}

#[derive(Args, Debug, Clone)]
pub struct TeamLeaveArgs {
    #[arg(long)]
    pub game: Option<String>,
    #[arg(long)]
    pub yes: bool,
}

#[derive(Args, Debug, Clone)]
pub struct CompletionArgs {
    #[arg(value_enum)]
    #[allow(clippy::needless_pass_by_value)]
    pub shell: clap_complete::Shell,
    /// Write the generated script to a file
    #[arg(long, value_name = "PATH")]
    pub output: Option<std::path::PathBuf>,
    /// Overwrite an existing output file
    #[arg(long, requires = "output")]
    pub force: bool,
    /// Print to an interactive terminal without confirmation
    #[arg(long)]
    pub yes: bool,
}
