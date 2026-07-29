use std::process::exit;

use clap::Parser;
use ret2cli::{run, Cli};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let json = cli.json;
    if let Err(err) = run(cli).await {
        if json {
            eprintln!(
                "{}",
                serde_json::json!({
                    "error": err.to_string(),
                })
            );
        } else {
            eprintln!("✗ {err}");
        }
        exit(err.exit_code());
    }
}
