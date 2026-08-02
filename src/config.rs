use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    cli::PagerMode,
    error::{CliError, CliResult},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientConfig {
    #[serde(default = "default_profile_name")]
    pub active_profile: String,
    #[serde(default)]
    pub profiles: HashMap<String, ConnectionProfile>,
    #[serde(default)]
    pub ui: UiConfig,
}

/// Persistent UI preferences. Every field is optional so an empty `[ui]`
/// section (or none at all) keeps the built-in defaults.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct UiConfig {
    /// Paging behavior; overridden by the `--pager` flag.
    pub pager_mode: Option<PagerMode>,
    /// Pager program (shell-words split); overridden by `$PAGER`.
    pub pager: Option<String>,
    /// Editor program (shell-words split); overridden by `$VISUAL`/`$EDITOR`.
    pub editor: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ConnectionProfile {
    pub url: String,
    #[serde(default)]
    pub active_account: Option<String>,
    #[serde(default)]
    pub accounts: HashMap<String, AccountSession>,
    #[serde(default)]
    pub game: Option<SelectedGame>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SelectedGame {
    pub id: i64,
    pub name: String,
}

impl std::fmt::Display for SelectedGame {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} ({})", self.id, self.name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSession {
    pub token: String,
    #[serde(default)]
    pub email: Option<String>,
}

impl ConnectionProfile {
    #[must_use]
    pub fn new(url: String) -> Self {
        Self { url, ..Self::default() }
    }

    #[must_use]
    pub fn active_token(&self) -> Option<&str> {
        self.active_account
            .as_ref()
            .and_then(|name| self.accounts.get(name))
            .map(|session| session.token.as_str())
    }

    pub fn store_account(&mut self, account: String, token: String, email: Option<String>) {
        self.accounts.insert(account.clone(), AccountSession { token, email });
        self.active_account = Some(account);
    }

    pub fn clear_active_account(&mut self) -> Option<String> {
        let account = self.active_account.take()?;
        self.accounts.remove(&account);
        Some(account)
    }
}

fn default_profile_name() -> String {
    "default".to_owned()
}

impl Default for ClientConfig {
    fn default() -> Self {
        let mut profiles = HashMap::new();
        profiles.insert("default".to_owned(), ConnectionProfile::default());
        Self { active_profile: default_profile_name(), profiles, ui: UiConfig::default() }
    }
}

impl ClientConfig {
    /// Load the configuration from disk, returning the default if none exists.
    ///
    /// # Errors
    ///
    /// Returns `CliError::Config` if the config file cannot be read or parsed.
    pub fn load() -> CliResult<Self> {
        let path = config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)
            .map_err(|e| CliError::Config(format!("failed to read config: {e}")))?;
        toml::from_str(&content)
            .map_err(|e| CliError::Config(format!("failed to parse config: {e}")))
    }

    /// Write the current configuration to disk atomically.
    ///
    /// A per-file advisory lock serializes concurrent writers (e.g. two
    /// `ret2cli` processes saving at once), and the temporary file carries the
    /// process id so concurrent writers never share a scratch file.
    ///
    /// # Errors
    ///
    /// Returns `CliError::Config` if the config directory cannot be created,
    /// the lock cannot be acquired, or if serialization, writing, or renaming
    /// the temp file fails.
    pub fn save(&self) -> CliResult<()> {
        self.save_to(&config_path()?)
    }

    fn save_to(&self, path: &Path) -> CliResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CliError::Config(format!("failed to create config dir: {e}")))?;
        }
        let lock_path = path.with_extension("toml.lock");
        let lock_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&lock_path)
            .map_err(|e| CliError::Config(format!("failed to open config lock: {e}")))?;
        lock_file.lock().map_err(|e| CliError::Config(format!("failed to lock config: {e}")))?;

        let content = toml::to_string_pretty(self)
            .map_err(|e| CliError::Config(format!("failed to serialize config: {e}")))?;
        let temp = path.with_extension(format!("toml.tmp.{}", std::process::id()));
        std::fs::write(&temp, content)
            .map_err(|e| CliError::Config(format!("failed to write config: {e}")))?;
        std::fs::rename(&temp, path)
            .map_err(|e| CliError::Config(format!("failed to replace config: {e}")))?;
        Ok(())
    }

    /// Resolve the active profile name from CLI arg, env var, or config default.
    ///
    /// # Errors
    ///
    /// Returns `CliError::Config` if the resolved profile name does not exist in `self.profiles`.
    pub fn active_profile_name<'a>(&'a self, cli_profile: Option<&'a str>) -> CliResult<String> {
        let name = cli_profile
            .map(str::to_owned)
            .or_else(|| env::var("R2S_PROFILE").ok())
            .unwrap_or_else(|| self.active_profile.clone());
        if self.profiles.contains_key(&name) {
            Ok(name)
        } else {
            Err(CliError::Config(format!("profile '{name}' not found")))
        }
    }

    /// Resolve the active profile and return a shared reference to it.
    ///
    /// # Errors
    ///
    /// Returns `CliError::Config` if the profile name cannot be resolved or the profile is not found.
    pub fn active_profile_resolved(
        &self,
        cli_profile: Option<&str>,
    ) -> CliResult<&ConnectionProfile> {
        let name = self.active_profile_name(cli_profile)?;
        self.profiles.get(&name).ok_or_else(|| CliError::Config("profile not found".to_owned()))
    }

    /// Resolve the active profile and return a mutable reference to it.
    ///
    /// # Errors
    ///
    /// Returns `CliError::Config` if the profile name cannot be resolved or the profile is not found.
    pub fn active_profile_mut(
        &mut self,
        cli_profile: Option<&str>,
    ) -> CliResult<&mut ConnectionProfile> {
        let name = self.active_profile_name(cli_profile)?;
        self.profiles.get_mut(&name).ok_or_else(|| CliError::Config("profile not found".to_owned()))
    }
}

fn config_path() -> CliResult<PathBuf> {
    let dir = dirs::config_dir()
        .ok_or_else(|| CliError::Config("cannot determine config directory".to_owned()))?;
    Ok(dir.join("ret2cli").join("config.toml"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn unknown_profile_is_an_error() {
        let config = ClientConfig::default();
        assert!(config.active_profile_resolved(Some("typo")).is_err());
    }

    #[test]
    fn accounts_are_scoped_to_their_connection_profile() {
        let mut config = ClientConfig::default();
        let profile = config.active_profile_mut(None).unwrap();
        profile.store_account("alice".to_owned(), "alice-token".to_owned(), None);
        profile.store_account("bob".to_owned(), "bob-token".to_owned(), None);
        profile.game = Some(SelectedGame { id: 11, name: "Example CTF".to_owned() });

        assert_eq!(profile.active_account.as_deref(), Some("bob"));
        assert_eq!(profile.active_token(), Some("bob-token"));
        profile.active_account = Some("alice".to_owned());
        assert_eq!(profile.active_token(), Some("alice-token"));

        let serialized = toml::to_string(&config).unwrap();
        let round_trip: ClientConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(round_trip.profiles["default"].active_token(), Some("alice-token"));
        assert_eq!(round_trip.profiles["default"].accounts.len(), 2);
        assert_eq!(
            round_trip.profiles["default"].game,
            Some(SelectedGame { id: 11, name: "Example CTF".to_owned() })
        );
    }

    #[test]
    fn unreleased_string_game_format_is_rejected() {
        let old = r#"
active_profile = "default"

[profiles.default]
url = "https://example.invalid"
game = "11"
"#;
        assert!(toml::from_str::<ClientConfig>(old).is_err());
    }

    #[test]
    fn ui_section_round_trips_and_old_configs_stay_valid() {
        let config: ClientConfig = toml::from_str(
            r#"
active_profile = "default"

[ui]
pager_mode = "always"
pager = "less -R -N"
editor = "hx"
"#,
        )
        .unwrap();
        assert_eq!(config.ui.pager_mode, Some(PagerMode::Always));
        assert_eq!(config.ui.pager.as_deref(), Some("less -R -N"));
        assert_eq!(config.ui.editor.as_deref(), Some("hx"));
        let serialized = toml::to_string(&config).unwrap();
        let round_trip: ClientConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(round_trip.ui, config.ui);

        // Configs written before the [ui] section existed must keep parsing.
        let old: ClientConfig = toml::from_str(
            r#"
active_profile = "default"

[profiles.default]
url = "https://example.invalid"
"#,
        )
        .unwrap();
        assert_eq!(old.ui, UiConfig::default());
    }

    #[test]
    fn concurrent_saves_never_corrupt_the_config() {
        let dir = std::env::temp_dir().join(format!(
            "ret2cli-config-lock-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        let mut handles = Vec::new();
        for index in 0..8 {
            let path = path.clone();
            handles.push(std::thread::spawn(move || {
                for round in 0..25 {
                    let mut config = ClientConfig::default();
                    config.profiles.insert(
                        format!("p{index}"),
                        ConnectionProfile::new(format!("https://{index}.example/{round}")),
                    );
                    config.save_to(&path).unwrap();
                }
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: ClientConfig = toml::from_str(&content).unwrap();
        // The final file is one complete snapshot, never interleaved writes:
        // the built-in default profile plus exactly one writer's profile.
        assert_eq!(parsed.profiles.len(), 2);
        assert!(parsed.profiles.contains_key("default"));
        assert_eq!(parsed.profiles.keys().filter(|name| name.as_str() != "default").count(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
