pub mod color;
pub mod color_picker;
pub mod ipc;
pub mod layout;
pub mod palette;
pub mod render;
pub mod text;
pub mod undo;
pub mod widget;

use std::path::PathBuf;

/// Return the shared Hypr ecosystem config directory, respecting
/// `$XDG_CONFIG_HOME` (defaults to `~/.config/hypr`).
pub fn hypr_config_dir() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(home).join(".config")
        });
    base.join("hypr")
}
