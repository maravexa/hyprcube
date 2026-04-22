use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use hyprcube_core::palette::{Palette, PaletteError};
use hyprcube_hyprconf::HyprlandConfig;

/// Configs shared across multiple panels that edit the same files.
///
/// - `hyprland` — shared by HyprlandPanel and InputPanel (both write
///   `hyprland.conf`); using the same Arc ensures neither panel's changes
///   overwrite the other's.
/// - `palette`  — shared by AppearancePanel and the app-level renderer so
///   that saving the palette is a single atomic write.
pub struct SharedConfigs {
    pub hyprland: Arc<Mutex<HyprlandConfig>>,
    pub palette: Arc<Mutex<Palette>>,
}

impl SharedConfigs {
    pub fn new(hyprland: HyprlandConfig, palette: Palette) -> Self {
        Self {
            hyprland: Arc::new(Mutex::new(hyprland)),
            palette: Arc::new(Mutex::new(palette)),
        }
    }

    /// Canonical path for `hyprland.conf`, respecting `$XDG_CONFIG_HOME`.
    pub fn hyprland_path() -> PathBuf {
        let config_dir = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            format!("{home}/.config")
        });
        PathBuf::from(config_dir).join("hypr/hyprland.conf")
    }

    /// Write `hyprland.conf` to disk, creating a `.bak` copy of the existing
    /// file first.
    pub fn save_hyprland(&self) -> io::Result<()> {
        let path = Self::hyprland_path();

        // Backup before overwriting.
        if path.exists() {
            let bak = path.with_file_name("hyprland.conf.bak");
            std::fs::copy(&path, bak)?;
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = self.hyprland.lock().unwrap().to_string_lossy();
        std::fs::write(&path, content)
    }

    /// Write `palette.toml` to disk.
    pub fn save_palette(&self) -> Result<(), PaletteError> {
        self.palette.lock().unwrap().save(None)
    }

    /// Reload `hyprland.conf` from disk into the shared config, discarding
    /// any unsaved in-memory mutations.
    pub fn reload_hyprland(&self) -> io::Result<()> {
        let path = Self::hyprland_path();
        let cfg = HyprlandConfig::load(&path)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        *self.hyprland.lock().unwrap() = cfg;
        Ok(())
    }

    /// Reload `palette.toml` from disk, discarding unsaved mutations.
    pub fn reload_palette(&self) -> io::Result<()> {
        let pal = Palette::load(None)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        *self.palette.lock().unwrap() = pal;
        Ok(())
    }

    /// Return a clone of the current palette (cheap for rendering callers).
    pub fn current_palette(&self) -> Palette {
        self.palette.lock().unwrap().clone()
    }
}
