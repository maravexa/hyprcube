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
        hyprcube_core::hypr_config_dir().join("hyprcube.toml")
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
        let contents = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_partial_config_keeps_defaults_and_accepts_unknown_fields() {
        let config: AppConfig = toml::from_str(
            r#"
            window_width = 1234
            palette = 'custom \"palette'
            future_option = { enabled = true }
        "#,
        )
        .unwrap();
        assert_eq!(config.window_width, 1234);
        assert_eq!(config.window_height, 640);
        assert_eq!(config.last_panel, 0);
        assert!(config.live_preview);
        assert_eq!(config.palette, "custom \\\"palette");
    }

    #[test]
    fn toml_app_config_round_trip_and_invalid_types() {
        let config = AppConfig {
            palette: "Unicode: 雪, quotes: \" and newline\n".into(),
            window_width: 1111,
            window_height: 777,
            last_panel: 6,
            live_preview: false,
        };
        let encoded = toml::to_string_pretty(&config).unwrap();
        let decoded: AppConfig = toml::from_str(&encoded).unwrap();
        assert_eq!(decoded.palette, config.palette);
        assert_eq!(decoded.window_width, 1111);
        assert_eq!(decoded.window_height, 777);
        assert_eq!(decoded.last_panel, 6);
        assert!(!decoded.live_preview);
        for invalid in [
            "window_width = -1",
            "window_width = 1.5",
            "live_preview = 'false'",
            "last_panel = 1\nlast_panel = 2",
        ] {
            assert!(toml::from_str::<AppConfig>(invalid).is_err(), "{invalid}");
        }
    }
}
