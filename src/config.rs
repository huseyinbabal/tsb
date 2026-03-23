use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// A saved Spring Boot application entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedApp {
    pub name: String,
    pub url: String,
}

/// Persistent configuration for TSB.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TsbConfig {
    #[serde(default)]
    pub apps: Vec<SavedApp>,
    pub active_app_url: Option<String>,
}

impl TsbConfig {
    /// Returns the base directory for TSB config (~/.config/tsb)
    pub fn config_dir() -> Result<PathBuf> {
        let home = dirs::home_dir().context("could not determine home directory")?;
        Ok(home.join(".config").join("tsb"))
    }

    /// Returns the path to the config file (~/.config/tsb/config.yaml).
    fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.yaml"))
    }

    /// Loads config from disk. Returns default config if the file doesn't exist.
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read config at {}", path.display()))?;
        let config: TsbConfig = serde_yaml::from_str(&contents)
            .with_context(|| format!("failed to parse config at {}", path.display()))?;
        Ok(config)
    }

    /// Saves the current config to disk, creating the directory if needed.
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create config dir {}", parent.display()))?;
        }
        let yaml = serde_yaml::to_string(self).context("failed to serialize config")?;
        fs::write(&path, yaml)
            .with_context(|| format!("failed to write config to {}", path.display()))?;
        Ok(())
    }

    /// Adds a new app or updates an existing one (matched by url).
    pub fn add_app(&mut self, name: String, url: String) {
        if let Some(existing) = self.apps.iter_mut().find(|a| a.url == url) {
            existing.name = name;
        } else {
            self.apps.push(SavedApp { name, url });
        }
    }

    /// Removes an app by url. Also clears `active_app_url` if it matched.
    pub fn remove_app(&mut self, url: &str) {
        self.apps.retain(|a| a.url != url);
        if self.active_app_url.as_deref() == Some(url) {
            self.active_app_url = None;
        }
    }
}
