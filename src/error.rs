use reqwest::StatusCode;
use thiserror::Error;

pub type CliResult<T> = Result<T, CliError>;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("{0}")]
    Config(String),
    #[error("{0}")]
    Authentication(String),
    #[error("{0}")]
    Request(#[from] reqwest::Error),
    #[error("failed to serialize response: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("file io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("server returned {status}: {message}")]
    Api { status: StatusCode, message: String },
}

impl CliError {
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Authentication(_) => 2,
            Self::Request(_) => 5,
            Self::Api { status, .. } => match status {
                s if *s == StatusCode::UNAUTHORIZED => 2,
                s if *s == StatusCode::FORBIDDEN => 3,
                s if *s == StatusCode::NOT_FOUND => 4,
                s if s.is_server_error() => 5,
                _ => 1,
            },
            _ => 1,
        }
    }
}
