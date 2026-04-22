use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

// Preset palettes embedded at compile time.
const PRESET_MOCHA: &str =
    include_str!("../../../themes/catppuccin-mocha/palette.toml");
const PRESET_LATTE: &str =
    include_str!("../../../themes/catppuccin-latte/palette.toml");
const PRESET_NORD: &str =
    include_str!("../../../themes/nord/palette.toml");
const PRESET_GRUVBOX: &str =
    include_str!("../../../themes/gruvbox-dark/palette.toml");
const PRESET_TOKYO: &str =
    include_str!("../../../themes/tokyo-night/palette.toml");
const PRESET_DRACULA: &str =
    include_str!("../../../themes/dracula/palette.toml");

#[derive(Debug, Error)]
pub enum PaletteError {
    #[error("failed to read palette file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse palette TOML: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("failed to serialize palette: {0}")]
    Serialize(#[from] toml::ser::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Palette {
    pub colors: PaletteColors,
    pub fonts: PaletteFonts,
    pub style: PaletteStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaletteColors {
    pub background: String,
    pub foreground: String,
    pub accent: String,
    pub urgent: String,
    pub surface: String,
    pub overlay: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaletteFonts {
    pub family: String,
    pub mono_family: String,
    pub size: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaletteStyle {
    pub border_radius: f64,
    pub opacity: f64,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            colors: PaletteColors {
                background: "#1e1e2e".into(),
                foreground: "#cdd6f4".into(),
                accent: "#89b4fa".into(),
                urgent: "#f38ba8".into(),
                surface: "#313244".into(),
                overlay: "#6c7086".into(),
            },
            fonts: PaletteFonts {
                family: "Inter".into(),
                mono_family: "JetBrains Mono".into(),
                size: 14.0,
            },
            style: PaletteStyle {
                border_radius: 8.0,
                opacity: 0.85,
            },
        }
    }
}

impl Palette {
    /// Return the default palette config path, respecting `XDG_CONFIG_HOME`.
    fn default_path() -> PathBuf {
        let config_dir = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
                PathBuf::from(home).join(".config")
            });
        config_dir.join("hyprcube").join("palette.toml")
    }

    /// Load a palette from the given path, or the default XDG path if `None`.
    pub fn load(path: Option<&Path>) -> Result<Self, PaletteError> {
        let path = match path {
            Some(p) => p.to_path_buf(),
            None => Self::default_path(),
        };
        let contents = std::fs::read_to_string(&path)?;
        let palette: Palette = toml::from_str(&contents)?;
        Ok(palette)
    }

    /// Return all built-in preset palettes as `(name, palette)` pairs.
    /// Ordering matches the display order in the Appearance panel dropdown.
    pub fn builtin_presets() -> Vec<(&'static str, Palette)> {
        let parse = |s: &'static str| -> Palette {
            toml::from_str(s).unwrap_or_default()
        };
        vec![
            ("Catppuccin Mocha", parse(PRESET_MOCHA)),
            ("Catppuccin Latte", parse(PRESET_LATTE)),
            ("Nord", parse(PRESET_NORD)),
            ("Gruvbox Dark", parse(PRESET_GRUVBOX)),
            ("Tokyo Night", parse(PRESET_TOKYO)),
            ("Dracula", parse(PRESET_DRACULA)),
        ]
    }

    /// Save the palette to the given path, or the default XDG path if `None`.
    pub fn save(&self, path: Option<&Path>) -> Result<(), PaletteError> {
        let path = match path {
            Some(p) => p.to_path_buf(),
            None => Self::default_path(),
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(&path, contents)?;
        Ok(())
    }
}
