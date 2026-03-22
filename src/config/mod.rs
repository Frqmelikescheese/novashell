pub mod schema;
pub mod watcher;

pub use schema::*;
pub use watcher::{ChangeEvent, ConfigWatcher};

use crate::error::{NovaError, Result};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Config loader responsible for reading and hot-reloading the YAML config.
pub struct ConfigLoader {
    /// Resolved path to the loaded config file
    pub path: PathBuf,
    /// The parsed config
    pub config: NovaConfig,
}

impl ConfigLoader {
    /// Load configuration from a specific path.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let config = Self::parse_file(&path)?;
        info!("Loaded config from {}", path.display());
        Ok(Self { path, config })
    }

    /// Load configuration using the default search order:
    /// 1. `~/.config/novashell/config.yaml`
    /// 2. `/etc/novashell/config.yaml`
    /// 3. Built-in defaults
    pub fn load_default() -> Result<Self> {
        let candidates = Self::default_config_paths();

        for path in &candidates {
            if path.exists() {
                debug!("Found config at {}", path.display());
                return Self::load(path);
            }
        }

        warn!("No config file found, using built-in defaults");
        let path = candidates
            .into_iter()
            .next()
            .unwrap_or_else(|| PathBuf::from("/tmp/novashell-default.yaml"));

        Ok(Self {
            path,
            config: NovaConfig::default(),
        })
    }

    /// Reload the config from its current path. Returns the new config on success.
    pub fn reload(&mut self) -> Result<&NovaConfig> {
        self.config = Self::parse_file(&self.path)?;
        info!("Config reloaded from {}", self.path.display());
        Ok(&self.config)
    }

    /// Returns a reference to the current config.
    pub fn config(&self) -> &NovaConfig {
        &self.config
    }

    /// Parse and deserialize a YAML config file at `path`.
    fn parse_file(path: &Path) -> Result<NovaConfig> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            NovaError::Config(format!("Cannot read {}: {e}", path.display()))
        })?;

        let config: NovaConfig = serde_yaml::from_str(&content).map_err(|e| {
            NovaError::Config(format!("YAML parse error in {}: {e}", path.display()))
        })?;

        Ok(config)
    }

    /// Returns the ordered list of paths to search for the default config.
    fn default_config_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // User config dir
        if let Some(cfg_dir) = dirs::config_dir() {
            paths.push(cfg_dir.join("novashell").join("config.yaml"));
        }

        // System-wide config
        paths.push(PathBuf::from("/etc/novashell/config.yaml"));

        paths
    }

    /// Returns the config directory (parent of config.yaml).
    pub fn config_dir(&self) -> PathBuf {
        self.path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    }
}

/// Expand `~` in a path to the actual home directory.
pub fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with("~/") || path == "~" {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/root"));
        let stripped = path.trim_start_matches("~/");
        if stripped.is_empty() {
            home
        } else {
            home.join(stripped)
        }
    } else {
        PathBuf::from(path)
    }
}
