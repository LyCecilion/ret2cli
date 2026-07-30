use std::{collections::HashMap, env, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{CliError, CliResult};

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientConfig {
    #[serde(default = "default_profile_name")]
    pub active_profile: String,
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,

    // Legacy v0.1 fields. They are consumed on load and never written again.
    #[serde(default, skip_serializing)]
    default: Option<Profile>,
    #[serde(default, skip_serializing)]
    default_game: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub url: String,
    pub token: Option<String>,
    #[serde(default)]
    pub game: Option<String>,
}

fn default_profile_name() -> String {
    "default".to_owned()
}

impl Default for ClientConfig {
    fn default() -> Self {
        let mut profiles = HashMap::new();
        profiles.insert("default".to_owned(), Profile::default());
        Self { active_profile: default_profile_name(), profiles, default: None, default_game: None }
    }
}

impl ClientConfig {
    pub fn load() -> CliResult<Self> {
        let path = config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)
            .map_err(|e| CliError::Config(format!("failed to read config: {e}")))?;
        let mut config: Self = toml::from_str(&content)
            .map_err(|e| CliError::Config(format!("failed to parse config: {e}")))?;
        let migrated = config.migrate_legacy();
        if migrated {
            let backup = path.with_extension("toml.bak");
            if !backup.exists() {
                std::fs::copy(&path, &backup)
                    .map_err(|e| CliError::Config(format!("failed to back up config: {e}")))?;
            }
            config.save()?;
        }
        Ok(config)
    }

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

    fn migrate_legacy(&mut self) -> bool {
        let Some(mut legacy_default) = self.default.take() else {
            if self.profiles.is_empty() {
                self.profiles.insert("default".to_owned(), Profile::default());
                return true;
            }
            return false;
        };
        if legacy_default.game.is_none() {
            legacy_default.game = self.default_game.take();
        }
        self.profiles.entry("default".to_owned()).or_insert(legacy_default);
        self.active_profile = default_profile_name();
        true
    }

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

    pub fn active_profile_resolved(&self, cli_profile: Option<&str>) -> CliResult<&Profile> {
        let name = self.active_profile_name(cli_profile)?;
        Ok(self.profiles.get(&name).expect("validated profile"))
    }

    pub fn active_profile_mut(&mut self, cli_profile: Option<&str>) -> CliResult<&mut Profile> {
        let name = self.active_profile_name(cli_profile)?;
        Ok(self.profiles.get_mut(&name).expect("validated profile"))
    }
}

fn config_path() -> CliResult<PathBuf> {
    let dir = dirs::config_dir()
        .ok_or_else(|| CliError::Config("cannot determine config directory".to_owned()))?;
    Ok(dir.join("ret2cli").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_legacy_default_and_game_without_losing_profiles() {
        let input = r#"
default_game = "11"

[default]
url = "https://default.example/"
token = "default-token"

[profiles.school]
url = "https://school.example/"
token = "school-token"
"#;
        let mut config: ClientConfig = toml::from_str(input).unwrap();
        assert!(config.migrate_legacy());
        assert_eq!(config.active_profile, "default");
        assert_eq!(config.profiles["default"].game.as_deref(), Some("11"));
        assert_eq!(config.profiles["default"].token.as_deref(), Some("default-token"));
        assert_eq!(config.profiles["school"].token.as_deref(), Some("school-token"));
        let serialized = toml::to_string(&config).unwrap();
        assert!(!serialized.contains("default_game"));
        assert!(!serialized.contains("[default]"));
    }

    #[test]
    fn unknown_profile_is_an_error() {
        let config = ClientConfig::default();
        assert!(config.active_profile_resolved(Some("typo")).is_err());
    }
}
