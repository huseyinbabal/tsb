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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_empty() {
        let cfg = TsbConfig::default();
        assert!(cfg.apps.is_empty());
        assert_eq!(cfg.active_app_url, None);
    }

    #[test]
    fn add_app_to_empty_config() {
        let mut cfg = TsbConfig::default();
        cfg.add_app("my-app".into(), "http://localhost:8080".into());
        assert_eq!(cfg.apps.len(), 1);
        assert_eq!(cfg.apps[0].name, "my-app");
        assert_eq!(cfg.apps[0].url, "http://localhost:8080");
    }

    #[test]
    fn add_two_distinct_apps() {
        let mut cfg = TsbConfig::default();
        cfg.add_app("app1".into(), "http://localhost:8080".into());
        cfg.add_app("app2".into(), "http://localhost:9090".into());
        assert_eq!(cfg.apps.len(), 2);
    }

    #[test]
    fn add_app_same_url_updates_name() {
        let mut cfg = TsbConfig::default();
        cfg.add_app("old-name".into(), "http://localhost:8080".into());
        cfg.add_app("new-name".into(), "http://localhost:8080".into());
        assert_eq!(cfg.apps.len(), 1);
        assert_eq!(cfg.apps[0].name, "new-name");
    }

    #[test]
    fn remove_existing_app() {
        let mut cfg = TsbConfig::default();
        cfg.add_app("app1".into(), "http://localhost:8080".into());
        cfg.add_app("app2".into(), "http://localhost:9090".into());
        cfg.remove_app("http://localhost:8080");
        assert_eq!(cfg.apps.len(), 1);
        assert_eq!(cfg.apps[0].url, "http://localhost:9090");
    }

    #[test]
    fn remove_nonexistent_url_is_noop() {
        let mut cfg = TsbConfig::default();
        cfg.add_app("app1".into(), "http://localhost:8080".into());
        cfg.remove_app("http://localhost:9999");
        assert_eq!(cfg.apps.len(), 1);
    }

    #[test]
    fn remove_active_app_clears_active_url() {
        let mut cfg = TsbConfig::default();
        cfg.add_app("app1".into(), "http://localhost:8080".into());
        cfg.active_app_url = Some("http://localhost:8080".into());
        cfg.remove_app("http://localhost:8080");
        assert_eq!(cfg.active_app_url, None);
    }

    #[test]
    fn remove_non_active_app_preserves_active_url() {
        let mut cfg = TsbConfig::default();
        cfg.add_app("app1".into(), "http://localhost:8080".into());
        cfg.add_app("app2".into(), "http://localhost:9090".into());
        cfg.active_app_url = Some("http://localhost:8080".into());
        cfg.remove_app("http://localhost:9090");
        assert_eq!(cfg.active_app_url, Some("http://localhost:8080".into()));
    }

    #[test]
    fn config_serde_roundtrip() {
        let mut cfg = TsbConfig::default();
        cfg.add_app("my-app".into(), "http://localhost:8080".into());
        cfg.active_app_url = Some("http://localhost:8080".into());

        let yaml = serde_yaml::to_string(&cfg).unwrap();
        let parsed: TsbConfig = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(parsed.apps.len(), 1);
        assert_eq!(parsed.apps[0].name, "my-app");
        assert_eq!(parsed.active_app_url, Some("http://localhost:8080".into()));
    }

    #[test]
    fn config_yaml_deserializes_from_string() {
        let yaml = r#"
apps:
  - name: app1
    url: http://localhost:8080
  - name: app2
    url: http://localhost:9090
active_app_url: http://localhost:8080
"#;
        let cfg: TsbConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.apps.len(), 2);
        assert_eq!(cfg.apps[0].name, "app1");
        assert_eq!(cfg.apps[1].url, "http://localhost:9090");
        assert_eq!(cfg.active_app_url, Some("http://localhost:8080".into()));
    }

    #[test]
    fn config_yaml_deserializes_empty() {
        let yaml = "apps: []\n";
        let cfg: TsbConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(cfg.apps.is_empty());
        assert_eq!(cfg.active_app_url, None);
    }

    #[test]
    fn config_yaml_missing_fields_uses_defaults() {
        let yaml = "{}";
        let cfg: TsbConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(cfg.apps.is_empty());
        assert_eq!(cfg.active_app_url, None);
    }

    #[test]
    fn saved_app_serde_roundtrip() {
        let app = SavedApp {
            name: "test".into(),
            url: "http://localhost:8080".into(),
        };
        let json = serde_json::to_string(&app).unwrap();
        let parsed: SavedApp = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "test");
        assert_eq!(parsed.url, "http://localhost:8080");
    }

    #[test]
    fn config_load_returns_default_if_no_file() {
        // This calls TsbConfig::load() which returns default if file doesn't exist.
        // The file at ~/.config/tsb/config.yaml may or may not exist, but either way
        // load() should not panic.
        let result = TsbConfig::load();
        assert!(result.is_ok());
    }

    #[test]
    fn config_save_and_load_roundtrip() {
        // Save, then load, verify data persists
        let original = TsbConfig::load().unwrap();

        let mut cfg = TsbConfig::default();
        cfg.add_app("roundtrip-test".into(), "http://roundtrip-test:1234".into());
        cfg.active_app_url = Some("http://roundtrip-test:1234".into());
        cfg.save().unwrap();

        let loaded = TsbConfig::load().unwrap();
        assert!(loaded.apps.iter().any(|a| a.name == "roundtrip-test"));

        // Restore original config
        original.save().unwrap();
    }

    #[test]
    fn config_dir_exists() {
        let dir = TsbConfig::config_dir();
        assert!(dir.is_ok());
        let path = dir.unwrap();
        assert!(path.to_str().unwrap().contains("tsb"));
    }

    #[test]
    fn add_app_then_remove_all() {
        let mut cfg = TsbConfig::default();
        cfg.add_app("a".into(), "http://a".into());
        cfg.add_app("b".into(), "http://b".into());
        cfg.remove_app("http://a");
        cfg.remove_app("http://b");
        assert!(cfg.apps.is_empty());
    }
}
