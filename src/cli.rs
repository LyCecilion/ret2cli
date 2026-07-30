use clap::{Args, Parser, Subcommand, ValueEnum};

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

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Account authentication and identity
    Account {
        #[command(subcommand)]
        command: AccountCommand,
    },
    /// Manage local `Ret2Shell` connection profiles
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
    },
    /// Browse and select games
    Game {
        #[command(subcommand)]
        command: GameCommand,
    },
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
    /// Open the guided interactive interface
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
    /// Verify the active account session with the server
    Status,
    /// Show the active account's server-side profile
    Show,
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
    List(GameListArgs),
    Show { game: Option<String> },
    Use { game: String },
    Scoreboard(GameContextArgs),
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
    Mine(GameContextArgs),
    Create(TeamCreateArgs),
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
    pub team: String,
    #[arg(long)]
    pub game: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct TeamCreateArgs {
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long)]
    pub game: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct TeamJoinArgs {
    pub token: Option<String>,
    #[arg(long)]
    pub game: Option<String>,
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
}
