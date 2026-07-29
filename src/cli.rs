use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "CLI client for Ret2Shell CTF platform"
)]
pub struct Cli {
    /// Output as JSON instead of formatted text
    #[arg(long, global = true)]
    pub json: bool,

    /// Profile name to use from config
    #[arg(long, global = true)]
    pub profile: Option<String>,

    /// Override the configured API base URL
    #[arg(long, global = true)]
    pub url: Option<String>,

    /// Override the configured token
    #[arg(long, global = true)]
    pub token: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Authenticate with a Ret2Shell instance
    #[command(name = "login")]
    Login(LoginArgs),

    /// Log out of the current profile
    #[command(name = "logout")]
    Logout,

    /// Show authentication status
    #[command(name = "status")]
    Status,

    /// Register a new account
    #[command(name = "register")]
    Register(RegisterArgs),

    /// List available games
    #[command(name = "games")]
    Games(GameListArgs),

    /// View game details
    #[command(name = "game")]
    Game(GameViewArgs),

    /// Show scoreboard for a game
    #[command(name = "scoreboard")]
    Scoreboard(ScoreboardArgs),

    /// List challenges in a game
    #[command(name = "challenges")]
    Challenges(ChallengeListArgs),

    /// View challenge details
    #[command(name = "view")]
    View(ChallengeViewArgs),

    /// Submit a flag
    #[command(name = "solve")]
    Solve(SolveArgs),

    /// List available hints for a challenge
    #[command(name = "hints")]
    Hints(HintsArgs),

    /// Unlock a hint
    #[command(name = "hint")]
    Hint(HintArgs),

    /// Start a challenge instance
    #[command(name = "start")]
    Start(StartArgs),

    /// Stop a challenge instance
    #[command(name = "stop")]
    Stop(StopArgs),

    /// Download challenge attachments
    #[command(name = "download")]
    Download(DownloadArgs),

    /// List teams in a game
    #[command(name = "teams")]
    Teams(TeamListArgs),

    /// View team details
    #[command(name = "team")]
    Team(TeamViewArgs),

    /// View my team
    #[command(name = "my")]
    My(MyTeamArgs),

    /// Create a team
    #[command(name = "team-create")]
    TeamCreate(TeamCreateArgs),

    /// Join a team by token
    #[command(name = "team-join")]
    TeamJoin(TeamJoinArgs),

    /// Leave your current team
    #[command(name = "team-leave")]
    TeamLeave(TeamLeaveArgs),

    /// View your profile
    #[command(name = "profile")]
    Profile,

    /// View your submission history
    #[command(name = "submissions")]
    Submissions(SubmissionsArgs),

    /// Generate shell completions
    #[command(name = "completion")]
    Completion(CompletionArgs),

    /// Switch default profile
    #[command(name = "use")]
    Use(UseArgs),

    /// Set default game for current session
    #[command(name = "use-game")]
    UseGame(UseGameArgs),
}

// --- Auth ---

#[derive(clap::Args, Debug, Clone)]
pub struct LoginArgs {
    /// URL of the Ret2Shell instance
    #[arg(long)]
    pub url: Option<String>,

    /// Account/username
    #[arg(long)]
    pub account: Option<String>,

    /// Password (insecure, prefer interactive prompt)
    #[arg(long)]
    pub password: Option<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct RegisterArgs {
    /// URL of the Ret2Shell instance
    #[arg(long)]
    pub url: Option<String>,
}

// --- Game ---

#[derive(clap::Args, Debug, Clone)]
pub struct GameListArgs {
    /// Page number
    #[arg(long, default_value = "1")]
    pub page: u32,

    /// Items per page
    #[arg(long, default_value = "20")]
    pub page_size: u32,

    /// Filter by game type
    #[arg(long)]
    pub r#type: Option<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct GameViewArgs {
    /// Game name or ID
    pub game: Option<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ScoreboardArgs {
    /// Game name or ID
    #[arg(long)]
    pub game: Option<String>,
}

// --- Challenge ---

#[derive(clap::Args, Debug, Clone)]
pub struct ChallengeListArgs {
    /// Game name or ID
    #[arg(long)]
    pub game: Option<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ChallengeViewArgs {
    /// Challenge name or ID
    pub challenge: String,

    /// Game name or ID
    #[arg(long)]
    pub game: Option<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct SolveArgs {
    /// Challenge name or ID
    pub challenge: String,

    /// Flag to submit (prompts if omitted)
    #[arg(long)]
    pub flag: Option<String>,

    /// Game name or ID
    #[arg(long)]
    pub game: Option<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct HintsArgs {
    /// Challenge name or ID
    pub challenge: String,

    /// Game name or ID
    #[arg(long)]
    pub game: Option<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct HintArgs {
    /// Challenge name or ID
    pub challenge: String,

    /// Specific hint ID to unlock
    #[arg(long)]
    pub id: Option<i64>,

    /// Game name or ID
    #[arg(long)]
    pub game: Option<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct StartArgs {
    /// Challenge name or ID
    pub challenge: String,

    /// Game name or ID
    #[arg(long)]
    pub game: Option<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct StopArgs {
    /// Challenge name or ID
    pub challenge: String,

    /// Game name or ID
    #[arg(long)]
    pub game: Option<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct DownloadArgs {
    /// Challenge name or ID
    pub challenge: String,

    /// Output file path (default: current directory)
    #[arg(long)]
    pub output: Option<String>,

    /// Game name or ID
    #[arg(long)]
    pub game: Option<String>,
}

// --- Team ---

#[derive(clap::Args, Debug, Clone)]
pub struct TeamListArgs {
    /// Game name or ID
    #[arg(long)]
    pub game: Option<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct TeamViewArgs {
    /// Team name or ID
    pub team: Option<String>,

    /// Game name or ID
    #[arg(long)]
    pub game: Option<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct MyTeamArgs {
    /// Game name or ID
    #[arg(long)]
    pub game: Option<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct TeamCreateArgs {
    /// Game name or ID
    #[arg(long)]
    pub game: Option<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct TeamJoinArgs {
    /// Game name or ID
    #[arg(long)]
    pub game: Option<String>,

    /// Team invitation token
    pub token: String,
}

#[derive(clap::Args, Debug, Clone)]
pub struct TeamLeaveArgs {
    /// Game name or ID
    #[arg(long)]
    pub game: Option<String>,
}

// --- Profile & Submissions ---

#[derive(clap::Args, Debug, Clone)]
pub struct SubmissionsArgs {
    /// Game ID
    #[arg(long)]
    pub game: Option<i64>,
}

// --- Shell completions ---

#[derive(clap::Args, Debug, Clone)]
pub struct CompletionArgs {
    /// Shell to generate completions for
    #[arg(value_enum)]
    pub shell: clap_complete::Shell,
}

// --- Use profile ---

#[derive(clap::Args, Debug, Clone)]
pub struct UseArgs {
    /// Profile name to switch to
    pub profile: String,
}

// --- Use game ---

#[derive(clap::Args, Debug, Clone)]
pub struct UseGameArgs {
    /// Game name or ID
    pub game: String,
}
