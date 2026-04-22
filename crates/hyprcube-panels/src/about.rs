use hyprcube_core::layout::Rect;
use hyprcube_core::widget::{Constraints, Event, EventResponse, Size};

use crate::{PanelError, PanelField, SettingsPanel};

/// The About panel shows system information rather than editable fields.
pub struct AboutPanel {
    _priv: (),
}

impl AboutPanel {
    pub fn new() -> Self {
        Self { _priv: () }
    }

    /// Collect system information as key-value pairs.
    pub fn system_info(&self) -> Vec<(String, String)> {
        let mut info = Vec::new();

        // HyprCube version
        info.push((
            "HyprCube".into(),
            env!("CARGO_PKG_VERSION").to_string(),
        ));

        // Hyprland version (try hyprctl)
        if let Ok(output) = std::process::Command::new("hyprctl").arg("version").output() {
            if output.status.success() {
                let raw = String::from_utf8_lossy(&output.stdout);
                // First line usually contains the version string.
                if let Some(line) = raw.lines().next() {
                    info.push(("Hyprland".into(), line.trim().to_string()));
                }
            }
        }

        // Kernel version
        if let Ok(output) = std::process::Command::new("uname").arg("-r").output() {
            if output.status.success() {
                let kernel = String::from_utf8_lossy(&output.stdout).trim().to_string();
                info.push(("Kernel".into(), kernel));
            }
        }

        // Hostname
        if let Ok(output) = std::process::Command::new("hostname").output() {
            if output.status.success() {
                let hostname = String::from_utf8_lossy(&output.stdout).trim().to_string();
                info.push(("Hostname".into(), hostname));
            }
        }

        info
    }
}

impl Default for AboutPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsPanel for AboutPanel {
    fn title(&self) -> &str {
        "About"
    }

    fn icon(&self) -> &str {
        "help-about"
    }

    fn fields(&self) -> Vec<PanelField> {
        Vec::new()
    }

    fn set_value(&mut self, key: &str, _value: &str) -> Result<String, PanelError> {
        Err(PanelError::UnknownKey(key.to_string()))
    }

    fn measure(&self, constraints: Constraints) -> Size {
        Size {
            width: constraints.max_width,
            height: constraints.max_height,
        }
    }

    fn paint(
        &self,
        _bounds: Rect,
        _pixmap: &mut tiny_skia::PixmapMut<'_>,
        _ctx: &mut hyprcube_core::text::RenderContext<'_>,
    ) {
        // Will render system info cards later.
    }

    fn event(&mut self, _event: &Event, _bounds: Rect) -> EventResponse {
        EventResponse::ignored()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn about_has_no_editable_fields() {
        let panel = AboutPanel::new();
        assert!(panel.fields().is_empty());
    }

    #[test]
    fn set_value_always_errors() {
        let mut panel = AboutPanel::new();
        assert!(panel.set_value("anything", "val").is_err());
    }

    #[test]
    fn system_info_includes_version() {
        let panel = AboutPanel::new();
        let info = panel.system_info();
        let version = info.iter().find(|(k, _)| k == "HyprCube");
        assert!(version.is_some());
        assert_eq!(version.unwrap().1, env!("CARGO_PKG_VERSION"));
    }
}
