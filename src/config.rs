use std::{collections::HashMap, env, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{CliError, CliResult};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ClientConfig {
    #[serde(default)]
    pub default: Profile,
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub url: String,
    pub token: Option<String>,
}

impl ClientConfig {
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

    pub fn save(&self) -> CliResult<()> {
        let path = config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CliError::Config(format!("failed to create config dir: {e}")))?;
        }
        let content = toml::to_string_pretty(self)
            .map_err(|e| CliError::Config(format!("failed to serialize config: {e}")))?;
        std::fs::write(&path, content)
            .map_err(|e| CliError::Config(format!("failed to write config: {e}")))?;
        Ok(())
    }

    /// Resolve profile: cli arg > R2S_PROFILE env > default.
    pub fn active_profile_resolved(&self, cli_profile: Option<&str>) -> &Profile {
        if let Some(name) = cli_profile {
            if let Some(profile) = self.profiles.get(name) {
                return profile;
            }
        }
        if let Ok(env_name) = env::var("R2S_PROFILE") {
            if let Some(profile) = self.profiles.get(&env_name) {
                return profile;
            }
        }
        &self.default
    }

    /// Get mutable access to the active profile for token updates.
    pub fn active_profile_mut(&mut self, cli_profile: Option<&str>) -> &mut Profile {
        if let Some(name) = cli_profile {
            if self.profiles.contains_key(name) {
                return self.profiles.get_mut(name).unwrap();
            }
        }
        if let Ok(env_name) = env::var("R2S_PROFILE") {
            if self.profiles.contains_key(&env_name) {
                return self.profiles.get_mut(&env_name).unwrap();
            }
        }
        &mut self.default
    }
}

fn config_path() -> CliResult<PathBuf> {
    let dir = dirs::config_dir()
        .ok_or_else(|| CliError::Config("cannot determine config directory".to_owned()))?;
    Ok(dir.join("ret2cli").join("config.toml"))
}
