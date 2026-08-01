use std::{
    env,
    io::{self, IsTerminal, Write},
    process::{Command, Stdio},
    sync::Mutex,
};

use colored::Colorize;
use serde::Serialize;
use tabled::Tabled;

use crate::{
    cli::PagerMode,
    error::{CliError, CliResult},
};

static BUFFER: Mutex<Option<String>> = Mutex::new(None);

pub struct Capture {
    pager: PagerMode,
    json: bool,
    stdout_is_terminal: bool,
    terminal_height: usize,
    finished: bool,
}

impl Capture {
    #[must_use]
    pub fn start(pager: PagerMode, json: bool) -> Self {
        *buffer().lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(String::new());
        Self {
            pager,
            json,
            stdout_is_terminal: io::stdout().is_terminal(),
            terminal_height: detect_terminal_height(),
            finished: false,
        }
    }

    pub fn finish(mut self) -> CliResult<()> {
        self.finished = true;
        flush(self.pager, self.json, self.stdout_is_terminal, self.terminal_height)
    }
}

fn detect_terminal_height() -> usize {
    env::var("LINES")
        .ok()
        .and_then(|value| value.parse().ok())
        .or_else(|| {
            Command::new("tput")
                .arg("lines")
                .output()
                .ok()
                .filter(|output| output.status.success())
                .and_then(|output| String::from_utf8(output.stdout).ok())
                .and_then(|value| value.trim().parse().ok())
        })
        .filter(|height| *height > 0)
        .unwrap_or(24)
}

impl Drop for Capture {
    fn drop(&mut self) {
        if !self.finished {
            let _ = flush(self.pager, self.json, self.stdout_is_terminal, self.terminal_height);
        }
    }
}

fn buffer() -> &'static Mutex<Option<String>> {
    &BUFFER
}

fn flush(
    pager: PagerMode,
    json: bool,
    stdout_is_terminal: bool,
    terminal_height: usize,
) -> CliResult<()> {
    let content = buffer()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
        .unwrap_or_default();
    if content.is_empty() {
        return Ok(());
    }
    let line_count = content.lines().count();
    if should_page(pager, json, stdout_is_terminal, line_count, terminal_height) && page(&content) {
        return Ok(());
    }
    io::stdout().write_all(content.as_bytes()).map_err(CliError::Io)
}

fn should_page(
    pager: PagerMode,
    json: bool,
    stdout_is_terminal: bool,
    line_count: usize,
    terminal_height: usize,
) -> bool {
    if json {
        return false;
    }
    match pager {
        PagerMode::Never => false,
        PagerMode::Always => true,
        PagerMode::Auto => stdout_is_terminal && line_count > terminal_height,
    }
}

fn pager_candidates() -> Vec<Vec<String>> {
    let mut candidates = Vec::new();
    if let Some(value) = env::var_os("PAGER").and_then(|value| value.into_string().ok())
        && let Ok(words) = shell_words::split(&value)
        && !words.is_empty()
    {
        candidates.push(words);
    }
    candidates.push(vec!["less".to_owned(), "-R".to_owned()]);
    candidates.push(vec!["more".to_owned()]);
    candidates
}

fn page(content: &str) -> bool {
    for candidate in pager_candidates() {
        let Some((program, args)) = candidate.split_first() else {
            continue;
        };
        let Ok(mut child) = Command::new(program).args(args).stdin(Stdio::piped()).spawn() else {
            continue;
        };
        let wrote =
            child.stdin.take().is_some_and(|mut stdin| stdin.write_all(content.as_bytes()).is_ok());
        if wrote && child.wait().is_ok_and(|status| status.success()) {
            return true;
        }
    }
    false
}

fn emit(value: &str) {
    let mut state = buffer().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(content) = state.as_mut() {
        content.push_str(value);
    } else {
        let _ = io::stdout().write_all(value.as_bytes());
    }
}

pub fn flush_before_prompt() -> CliResult<()> {
    let content = {
        let mut state = buffer().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.as_mut().map(std::mem::take).unwrap_or_default()
    };
    io::stdout().write_all(content.as_bytes()).map_err(CliError::Io)?;
    io::stdout().flush().map_err(CliError::Io)
}

pub fn line(value: &str) {
    emit(value);
    emit("\n");
}

pub fn blank() {
    emit("\n");
}

pub fn print_json(value: &impl Serialize) {
    line(&serde_json::to_string(value).unwrap_or_default());
}

pub fn print_table<T: Tabled>(rows: &[T]) {
    if rows.is_empty() {
        line("No results.");
        return;
    }
    let table = tabled::Table::new(rows).with(tabled::settings::Style::psql()).to_string();
    line(&table);
}

pub fn print_markdown(content: &str) {
    if content.is_empty() {
        return;
    }
    let skin = termimad::MadSkin::default();
    line(&skin.inline(content).to_string());
}

pub fn print_key_value(pairs: &[(&str, &str)]) {
    if pairs.is_empty() {
        return;
    }
    let max_key = pairs.iter().map(|(key, _)| key.len()).max().unwrap_or(0);
    for (key, value) in pairs {
        let label = format!("{key}:");
        line(&format!("  {:<width$} {value}", label.bold(), width = max_key + 1));
    }
}

pub fn success(message: &str) {
    line(&format!("{} {message}", "✓".green().bold()));
}

pub fn info(message: &str) {
    line(&format!("{} {message}", "ℹ".blue().bold()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_paging_only_applies_to_long_terminal_output() {
        assert!(should_page(PagerMode::Auto, false, true, 25, 24));
        assert!(!should_page(PagerMode::Auto, false, true, 24, 24));
        assert!(!should_page(PagerMode::Auto, false, false, 100, 24));
        assert!(!should_page(PagerMode::Auto, true, true, 100, 24));
    }

    #[test]
    fn explicit_pager_modes_override_terminal_length() {
        assert!(should_page(PagerMode::Always, false, false, 1, 24));
        assert!(!should_page(PagerMode::Never, false, true, 100, 24));
        assert!(!should_page(PagerMode::Always, true, true, 100, 24));
    }
}
