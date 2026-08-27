//! Completion engine for the interactive REPL.
//!
//! Pure logic only: tokenizing the edited line, descending the clap command
//! tree, and producing candidates from a [`Snapshot`] of local names and
//! cached platform lists. Network access is not performed here; callers pass
//! whatever data they have and stale or missing data simply yields fewer
//! candidates.
//!
//! Candidate model mirrors Bash: an entity appears once as its numeric id and
//! once as its (quoted) name, so `challenge show 9<Tab>` fills ids while
//! `challenge show Pyjail<Tab>` fills `"Pyjail 6"`-style names.

use std::sync::Arc;

use clap::CommandFactory;
use rustyline::completion::Candidate;

use crate::Cli;

/// An `(id, name)` pair for games, challenges, or teams.
pub type Entity = (i64, String);

/// Platform entity lists used by dynamic completions.
#[derive(Debug, Default, Clone)]
pub struct Lists {
    pub games: Vec<Entity>,
    pub challenges: Vec<Entity>,
    pub teams: Vec<Entity>,
}

/// Everything the completion engine may reference for one Tab press.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub profiles: Vec<String>,
    pub accounts: Vec<String>,
    pub lists: Arc<Lists>,
}

/// A completion candidate with separated insert text and display text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// Text inserted into the line (quoting included when required).
    pub replacement: String,
    /// Text shown in the suggestion list.
    pub display: String,
}

impl Candidate for Suggestion {
    fn display(&self) -> &str {
        &self.display
    }

    fn replacement(&self) -> &str {
        &self.replacement
    }
}

/// Result of one completion request.
#[derive(Debug, Clone)]
pub struct CompletionOutcome {
    /// Byte offset in the line where replacements are inserted.
    pub word_start: usize,
    pub candidates: Vec<Suggestion>,
}

impl CompletionOutcome {
    fn empty(word_start: usize) -> Self {
        Self { word_start, candidates: Vec::new() }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Token {
    /// Byte offset of the first raw character (including any opening quote).
    start: usize,
    text: String,
    /// Quote style that opened this token, if any.
    quote: Option<char>,
}

/// Interactive built-ins accepted by `parse_line` but absent from the clap tree.
const BUILTINS: [&str; 4] = ["help", "context", "exit", "quit"];

fn is_separator(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\n' | '\r')
}

/// Split `line` into shell-like tokens, tolerating incomplete quoting.
///
/// A trailing token ending inside quotes is emitted as-is, so
/// `challenge show "Pyjail` still completes against `Pyjail`.
fn scan_tokens(line: &str) -> Vec<Token> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mode {
        Bare,
        Single,
        Double,
    }

    let mut tokens = Vec::new();
    // (mode, accumulated text); `start` tracks the token's byte offset.
    let mut current: Option<(Mode, String)> = None;
    let mut start = 0usize;
    let mut escaped = false;

    macro_rules! flush {
        ($mode:expr) => {{
            let (_, text) = current.take().expect("token is open");
            let quote = match $mode {
                Mode::Single | Mode::Double => Some(if $mode == Mode::Single { '\'' } else { '"' }),
                Mode::Bare => None,
            };
            tokens.push(Token { start, text, quote });
        }};
    }

    for (offset, character) in line.char_indices() {
        match current.as_mut() {
            None => {
                if is_separator(character) {
                    continue;
                }
                start = offset;
                escaped = false;
                match character {
                    '\'' => current = Some((Mode::Single, String::new())),
                    '"' => current = Some((Mode::Double, String::new())),
                    _ => current = Some((Mode::Bare, character.to_string())),
                }
            }
            Some((mode, text)) => {
                if escaped {
                    escaped = false;
                    text.push(character);
                    continue;
                }
                match *mode {
                    Mode::Bare => {
                        if is_separator(character) {
                            flush!(Mode::Bare);
                        } else if character == '\\' {
                            escaped = true;
                        } else {
                            text.push(character);
                        }
                    }
                    Mode::Single => {
                        if character == '\'' {
                            flush!(Mode::Single);
                        } else {
                            text.push(character);
                        }
                    }
                    Mode::Double => {
                        if character == '"' {
                            flush!(Mode::Double);
                        } else if character == '\\' {
                            escaped = true;
                        } else {
                            text.push(character);
                        }
                    }
                }
            }
        }
    }

    if let Some((mode, _)) = current {
        // Trailing token possibly unterminated; emit as a partial word.
        flush!(mode);
    }
    tokens
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Profiles,
    Accounts,
    Games,
    Challenges,
    Teams,
}

/// Map `(command path, arg id)` to its dynamic source. Mirrors the resolution
/// semantics of `resolve_game_id` / `resolve_challenge_id` / team resolution.
fn source_for(path: &[String], id: &str) -> Option<Source> {
    let path: Vec<&str> = path.iter().map(String::as_str).collect();
    match id {
        "game" => Some(Source::Games),
        "challenge" => Some(Source::Challenges),
        "team" => Some(Source::Teams),
        "account" if path == ["account", "use"] || path == ["account", "remove"] => {
            Some(Source::Accounts)
        }
        "name" if path == ["profile", "use"] || path == ["profile", "remove"] => {
            Some(Source::Profiles)
        }
        _ => None,
    }
}

struct Descent<'a> {
    cmd: &'a clap::Command,
    path: Vec<String>,
    /// Positional values consumed at this level.
    positionals_used: usize,
    /// A value-taking flag whose value has not been typed yet.
    pending_flag: Option<String>,
    /// Dynamic source bound to the pending flag value, when known.
    pending_source: Option<Source>,
    /// Unknown words were seen; completing would guess wrongly.
    unresolvable: bool,
}

fn takes_values(argument: &clap::Arg) -> bool {
    argument.get_num_args().is_some_and(|bounds| bounds.takes_values())
}

fn find_long<'a>(
    cmd: &'a clap::Command,
    root: &'a clap::Command,
    long: &str,
) -> Option<&'a clap::Arg> {
    cmd.get_arguments().find(|argument| argument.get_long() == Some(long)).or_else(|| {
        root.get_arguments()
            .find(|argument| argument.is_global_set() && argument.get_long() == Some(long))
    })
}

/// Upper bound on positional values at this level (`usize::MAX` = unbounded).
fn positional_cap(cmd: &clap::Command) -> usize {
    let positionals: Vec<_> = cmd.get_positionals().collect();
    let unbounded = positionals
        .iter()
        .any(|argument| argument.get_num_args().is_some_and(|bounds| bounds.max_values() > 1));
    if unbounded { usize::MAX } else { positionals.len() }
}

/// Walk the tree along fully typed words. Value flags swallow their next
/// word; unknown words stop completion instead of guessing.
fn descend<'a>(root: &'a clap::Command, tokens: &[Token]) -> Descent<'a> {
    let mut state = Descent {
        cmd: root,
        path: Vec::new(),
        positionals_used: 0,
        pending_flag: None,
        pending_source: None,
        unresolvable: false,
    };

    for token in tokens {
        // This word belonged to the previous flag.
        if state.pending_flag.take().is_some() {
            state.pending_source = None;
            continue;
        }

        if token.text.starts_with('-') && token.text.len() > 1 {
            if !token.text.starts_with("--") {
                // Short-flag clusters are unsupported; no suggestions.
                state.unresolvable = true;
                return state;
            }
            let Some(argument) = find_long(state.cmd, root, &token.text[2..]) else {
                state.unresolvable = true;
                return state;
            };
            if takes_values(argument) {
                state.pending_flag = Some(token.text.trim_start_matches('-').to_owned());
                state.pending_source =
                    (argument.get_id().as_str() == "game").then_some(Source::Games);
            }
            continue;
        }

        if let Some(sub) = state.cmd.find_subcommand(&token.text) {
            state.path.push(sub.get_name().to_owned());
            state.cmd = sub;
            state.positionals_used = 0;
            state.pending_flag = None;
            state.pending_source = None;
            continue;
        }

        state.positionals_used += 1;
        if state.positionals_used > positional_cap(state.cmd) {
            state.unresolvable = true;
            return state;
        }
    }
    state
}

fn static_flag_candidates(
    prefix: &str,
    root: &clap::Command,
    state: &Descent<'_>,
) -> Vec<Suggestion> {
    let stripped = prefix.trim_start_matches('-');
    let mut longs: Vec<&str> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for argument in state
        .cmd
        .get_arguments()
        .chain(root.get_arguments().filter(|argument| argument.is_global_set()))
    {
        if let Some(long) = argument.get_long()
            && seen.insert(long)
        {
            longs.push(long);
        }
    }
    longs.sort_unstable();
    longs
        .into_iter()
        .filter(|long| long.starts_with(stripped))
        .map(|long| Suggestion { replacement: format!("--{long}"), display: format!("--{long}") })
        .collect()
}

fn static_subcommand_candidates(prefix: &str, state: &Descent<'_>) -> Vec<Suggestion> {
    let mut out: Vec<Suggestion> = state
        .cmd
        .get_subcommands()
        .filter(|sub| sub.get_name().starts_with(prefix))
        .map(|sub| {
            let name = sub.get_name().to_owned();
            Suggestion { replacement: name.clone(), display: name }
        })
        .collect();
    if state.path.is_empty() {
        for builtin in BUILTINS {
            if builtin.starts_with(prefix) {
                out.push(Suggestion {
                    replacement: (*builtin).to_owned(),
                    display: (*builtin).to_owned(),
                });
            }
        }
    }
    out.sort_by(|left, right| left.display.cmp(&right.display));
    out.dedup_by(|left, right| left.display == right.display);
    out
}

fn enum_candidates(argument: &clap::Arg) -> Vec<Suggestion> {
    argument
        .get_value_parser()
        .possible_values()
        .map(|values| {
            values
                .into_iter()
                .map(|value| {
                    let name = value.get_name().to_owned();
                    let display = value
                        .get_help()
                        .map_or_else(|| name.clone(), |help| format!("{name}  ({help})"));
                    Suggestion { replacement: name, display }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn needs_quoting(text: &str) -> bool {
    text.is_empty()
        || text.chars().any(|character| {
            matches!(
                character,
                ' ' | '\t'
                    | '\n'
                    | '\r'
                    | '\''
                    | '"'
                    | '\\'
                    | '$'
                    | '&'
                    | ';'
                    | '|'
                    | '<'
                    | '>'
                    | '('
                    | ')'
            )
        })
}

fn quoted(text: &str, quote_style: Option<char>) -> String {
    if quote_style == Some('\'') {
        return format!("'{text}'");
    }
    if needs_quoting(text) { format!("\"{text}\"") } else { text.to_owned() }
}

fn name_candidates(names: &[String], prefix: &str, quote_style: Option<char>) -> Vec<Suggestion> {
    let lowered_prefix = prefix.to_lowercase();
    let mut matched: Vec<&String> =
        names.iter().filter(|name| name.to_lowercase().starts_with(&lowered_prefix)).collect();
    matched.sort_unstable_by_key(|name| name.to_lowercase());
    matched.dedup_by(|left, right| left.to_lowercase() == right.to_lowercase());
    matched
        .into_iter()
        .map(|name| {
            let replacement = quoted(name, quote_style);
            Suggestion { replacement, display: (*name).clone() }
        })
        .collect()
}

/// Dual-form candidates for entities: numeric id plus quoted display name,
/// both shown as `{id} {name}` rows.
fn entity_candidates(
    entities: &[Entity],
    prefix: &str,
    quote_style: Option<char>,
) -> Vec<Suggestion> {
    let lowered_prefix = prefix.to_lowercase();
    let mut out = Vec::new();
    for (id, name) in entities {
        let id_text = id.to_string();
        let id_matches = id_text.starts_with(prefix);
        let name_matches = name.to_lowercase().starts_with(&lowered_prefix);
        if !id_matches && !name_matches {
            continue;
        }
        let display = format!("{id} {name}");
        if id_matches {
            out.push(Suggestion { replacement: id_text, display: display.clone() });
        }
        if name_matches {
            let replacement = quoted(name, quote_style);
            out.push(Suggestion { replacement, display });
        }
    }
    out.sort_by(|left, right| left.display.cmp(&right.display));
    out
}

/// Complete `line` against the command tree and dynamic snapshot. Assumes the
/// caret sits at the end of `line`, which is how rustyline presents Tab.
#[must_use]
pub fn complete_line(line: &str, snapshot: &Snapshot) -> CompletionOutcome {
    let ends_with_separator = line.chars().next_back().is_some_and(is_separator);
    let mut tokens = scan_tokens(line);

    if tokens.first().is_some_and(|token| token.text == "ret2cli") {
        tokens.remove(0);
    }

    let fragment = if ends_with_separator {
        // Cursor opens a fresh word.
        Token { start: line.len(), text: String::new(), quote: None }
    } else {
        match tokens.pop() {
            Some(token) => token,
            // Empty or whitespace-free prefix line.
            None => Token { start: 0, text: String::new(), quote: None },
        }
    };

    let mut root = Cli::command();
    root.build();

    let descent = descend(&root, &tokens);
    if descent.unresolvable {
        return CompletionOutcome::empty(fragment.start);
    }

    // A previous flag awaits its value.
    if let Some(flag) = &descent.pending_flag {
        let candidates = match descent.pending_source {
            Some(Source::Games) => {
                entity_candidates(&snapshot.lists.games, &fragment.text, fragment.quote)
            }
            None => find_long(descent.cmd, &root, flag).map(enum_candidates).unwrap_or_default(),
            _ => Vec::new(),
        };
        // Enum values are plain words; filtered by their replacement text.
        let candidates = if descent.pending_source.is_none() {
            filter_by_replacement(candidates, &fragment.text)
        } else {
            // `entity_candidates` already matched on the unquoted fragment.
            candidates
        };
        return CompletionOutcome { word_start: fragment.start, candidates };
    }

    // Flag-name completion (`--pa<Tab>`).
    if fragment.text.starts_with('-') {
        if fragment.text.starts_with("--") || fragment.text.len() == 1 {
            let candidates = static_flag_candidates(&fragment.text, &root, &descent);
            return CompletionOutcome { word_start: fragment.start, candidates };
        }
        return CompletionOutcome::empty(fragment.start);
    }

    // Subcommand level: offer nested commands (plus REPL built-ins at root).
    if descent.cmd.has_subcommands() {
        let candidates = static_subcommand_candidates(&fragment.text, &descent);
        return CompletionOutcome { word_start: fragment.start, candidates };
    }

    // Leaf level: positional values.
    let positionals: Vec<_> = descent.cmd.get_positionals().collect();
    let Some(argument) = positionals.get(descent.positionals_used) else {
        return CompletionOutcome::empty(fragment.start);
    };
    let id = argument.get_id().as_str();
    let quote_style = fragment.quote;
    let prefix = fragment.text.as_str();

    let candidates = match source_for(&descent.path, id) {
        Some(source) => match source {
            Source::Profiles => name_candidates(&snapshot.profiles, prefix, quote_style),
            Source::Accounts => name_candidates(&snapshot.accounts, prefix, quote_style),
            other => {
                let entities = match other {
                    Source::Games => &snapshot.lists.games,
                    Source::Challenges => &snapshot.lists.challenges,
                    Source::Teams => &snapshot.lists.teams,
                    Source::Profiles | Source::Accounts => unreachable!("handled above"),
                };
                entity_candidates(entities, prefix, quote_style)
            }
        },
        None => filter_by_replacement(enum_candidates(argument), prefix),
    };

    CompletionOutcome { word_start: fragment.start, candidates }
}

/// Narrow candidates whose insert text must extend what is already typed.
fn filter_by_replacement(candidates: Vec<Suggestion>, prefix: &str) -> Vec<Suggestion> {
    if prefix.is_empty() {
        return candidates;
    }
    candidates.into_iter().filter(|candidate| candidate.replacement.starts_with(prefix)).collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn fixture() -> Snapshot {
        Snapshot {
            profiles: vec!["default".to_owned(), "work".to_owned()],
            accounts: vec!["alice".to_owned(), "bob".to_owned()],
            lists: Arc::new(Lists {
                games: vec![(22, "ExampleCTF 2025".to_owned())],
                challenges: vec![(900, "Pyjail 6".to_owned()), (901, "Web".to_owned())],
                teams: vec![(7, "The A Team".to_owned())],
            }),
        }
    }

    fn ids(line: &str) -> Vec<String> {
        complete_line(line, &fixture()).candidates.into_iter().map(|c| c.replacement).collect()
    }

    #[test]
    fn scanner_handles_quotes_and_flags() {
        let toks = scan_tokens("game challenge show 'Pyjail 6'");
        assert_eq!(toks.len(), 4);
        assert_eq!(toks[3].text, "Pyjail 6");

        // Unterminated quote keeps the partial text.
        let toks = scan_tokens("challenge show \"Pyjail");
        assert_eq!(toks.last().unwrap().text, "Pyjail");
        assert_eq!(toks.last().unwrap().quote, Some('"'));

        let toks = scan_tokens("help\t context ");
        assert_eq!(toks.len(), 2);
    }

    #[test]
    fn root_completes_subcommands_and_builtins() {
        let out = complete_line("", &fixture());
        for expected in ["profile", "account", "game", "interactive", "completion", "help"] {
            assert!(out.candidates.iter().any(|c| c.replacement == expected), "missing {expected}");
        }
    }

    #[test]
    fn prefixes_narrow_subcommands() {
        assert_eq!(ids("gam"), vec!["game"]);
        assert_eq!(ids("prof"), vec!["profile"]);
    }

    #[test]
    fn challenge_positional_offers_id_and_name_forms() {
        let out = complete_line("game challenge show ", &fixture());
        let replacements: Vec<_> = out.candidates.iter().map(|c| c.replacement.as_str()).collect();
        assert_eq!(replacements, ["900", "\"Pyjail 6\"", "901", "Web"].to_vec());
        // Insertion point is the fresh word after the trailing space.
        assert_eq!(out.word_start, "game challenge show ".len());
    }

    #[test]
    fn numeric_prefix_completes_ids_only() {
        let replacements = ids("game challenge show 9");
        assert_eq!(replacements, ["900", "901"]);
        let displays: Vec<String> = complete_line("game challenge show 9", &fixture())
            .candidates
            .iter()
            .map(|c| c.display.clone())
            .collect();
        assert_eq!(displays, ["900 Pyjail 6", "901 Web"]);
    }

    #[test]
    fn name_prefix_inserts_quoted_full_name() {
        let out = complete_line("game challenge show pyjail", &fixture());
        assert_eq!(out.candidates.len(), 1);
        assert_eq!(out.candidates[0].replacement, "\"Pyjail 6\"");
        assert_eq!(out.candidates[0].display, "900 Pyjail 6");
    }

    #[test]
    fn quoted_fragment_includes_quote_in_insert_point() {
        let line = "game challenge show \"Pyjail";
        let out = complete_line(line, &fixture());
        assert_eq!(out.word_start, "game challenge show ".len());
        assert_eq!(out.candidates[0].replacement, "\"Pyjail 6\"");

        let line = "game challenge show 'Pyjail";
        let out = complete_line(line, &fixture());
        assert_eq!(out.candidates[0].replacement, "'Pyjail 6'");
    }

    #[test]
    fn profile_and_account_values_come_from_the_snapshot() {
        assert_eq!(ids("profile use ").join(","), "default,work");
        assert_eq!(ids("account use a"), vec!["alice"]);
    }

    #[test]
    fn game_flag_value_completes_games() {
        let replacements = ids("game challenge list --game ex");
        assert_eq!(replacements, ["\"ExampleCTF 2025\""]);

        let numeric_only = ids("game challenge list --game 2");
        assert_eq!(numeric_only, vec!["22"]);
    }

    #[test]
    fn enum_flags_and_positionals_complete_possible_values() {
        let values = ids("game list --pager a");
        assert!(values.contains(&"always".to_owned()));
        assert!(values.contains(&"auto".to_owned()));
        assert!(!values.contains(&"never".to_owned()));

        let shells = ids("completion b");
        assert!(shells.contains(&"bash".to_owned()));
    }

    #[test]
    fn unknown_words_disable_completion() {
        assert!(
            complete_line("nope nope2 no<Tab-like-fragment>", &fixture()).candidates.is_empty()
        );
        assert!(complete_line("game bogus x", &fixture()).candidates.is_empty());
    }

    #[test]
    fn team_positional_matches_prefix_with_quoting() {
        let out = complete_line("game team show the", &fixture());
        assert_eq!(out.candidates[0].replacement, "\"The A Team\"");
    }
}
