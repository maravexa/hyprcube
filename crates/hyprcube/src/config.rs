use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// Active palette name.
    pub palette: String,
    /// Default window width.
    pub window_width: u32,
    /// Default window height.
    pub window_height: u32,
    /// Last selected sidebar panel index.
    pub last_panel: usize,
    /// Apply changes via IPC immediately.
    pub live_preview: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            palette: "default".into(),
            window_width: 900,
            window_height: 640,
            last_panel: 0,
            live_preview: true,
        }
    }
}

impl AppConfig {
    /// Return the config file path, respecting `XDG_CONFIG_HOME`.
    fn config_path() -> PathBuf {
        let config_dir = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
                PathBuf::from(home).join(".config")
            });
        config_dir.join("hyprcube").join("config.toml")
    }

    /// Load the app config from the default XDG path.
    /// Returns `Default` if the file doesn't exist or can't be parsed.
    pub fn load() -> Self {
        Self::load_from(&Self::config_path())
    }

    fn load_from(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(contents) => toml::from_str(&contents).unwrap_or_else(|e| {
                tracing::warn!("failed to parse config, using defaults: {e}");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    /// Save the app config to the default XDG path.
    /// Creates the config directory if it doesn't exist.
    pub fn save(&self) -> Result<(), std::io::Error> {
        self.save_to(&Self::config_path())
    }

    fn save_to(&self, path: &Path) -> Result<(), std::io::Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, contents)
    }

    /// Delete the config file (for --reset-config).
    pub fn delete() -> Result<(), std::io::Error> {
        let path = Self::config_path();
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }
}
