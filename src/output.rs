use colored::*;
use serde::Serialize;
use tabled::Tabled;

pub fn print_json(value: &impl Serialize) {
    println!("{}", serde_json::to_string(value).unwrap_or_default());
}

pub fn print_table<T: Tabled>(rows: &[T]) {
    if rows.is_empty() {
        println!("No results.");
        return;
    }
    let table = tabled::Table::new(rows)
        .with(tabled::settings::Style::psql())
        .to_string();
    println!("{table}");
}

pub fn print_markdown(content: &str) {
    if content.is_empty() {
        return;
    }
    let skin = termimad::MadSkin::default();
    println!("{}", skin.inline(content));
}

pub fn print_key_value(pairs: &[(&str, &str)]) {
    if pairs.is_empty() {
        return;
    }
    let max_key = pairs.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    for (key, value) in pairs {
        let label = format!("{}{}", key, ":");
        println!("  {:<width$} {}", label.bold(), value, width = max_key + 1);
    }
}

pub fn success(msg: &str) {
    println!("{} {}", "✓".green().bold(), msg);
}

pub fn info(msg: &str) {
    println!("{} {}", "ℹ".blue().bold(), msg);
}
