use std::{collections::HashMap, env, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{CliError, CliResult};

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientConfig {
    #[serde(default = "default_profile_name")]
    pub active_profile: String,
    #[serde(default)]
    pub profiles: HashMap<String, ConnectionProfile>,
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
        self.accounts
            .insert(account.clone(), AccountSession { token, email });
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
        Self { active_profile: default_profile_name(), profiles }
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
    /// # Errors
    ///
    /// Returns `CliError::Config` if the config directory cannot be created,
    /// or if serialization, writing, or renaming the temp file fails.
    pub fn save(&self) -> CliResult<()> {
        let path = config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CliError::Config(format!("failed to create config dir: {e}")))?;
        }
        let content = toml::to_string_pretty(self)
            .map_err(|e| CliError::Config(format!("failed to serialize config: {e}")))?;
        let temp = path.with_extension("toml.tmp");
        std::fs::write(&temp, content)
            .map_err(|e| CliError::Config(format!("failed to write config: {e}")))?;
        std::fs::rename(&temp, &path)
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
}
