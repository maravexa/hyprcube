use hyprcube_core::layout::Rect;
use hyprcube_core::widget::{Constraints, Event, EventResponse, Size};

use crate::{FieldType, PanelError, PanelField, SettingsPanel};

/// Monitor / display layout panel.
///
/// This panel will eventually provide a drag-and-drop monitor arrangement UI.
/// For now it exposes placeholder fields noting that monitor configuration is
/// read from the `monitor =` lines in `hyprland.conf`.
pub struct DisplayPanel {
    monitors: Vec<MonitorInfo>,
}

/// Minimal monitor descriptor (will be populated from IPC later).
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub name: String,
    pub description: String,
}

impl DisplayPanel {
    pub fn new() -> Self {
        Self {
            monitors: Vec::new(),
        }
    }

    /// Replace the monitor list (e.g. after an IPC query).
    pub fn set_monitors(&mut self, monitors: Vec<MonitorInfo>) {
        self.monitors = monitors;
    }
}

impl Default for DisplayPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsPanel for DisplayPanel {
    fn title(&self) -> &str {
        "Display"
    }

    fn icon(&self) -> &str {
        "preferences-desktop-display"
    }

    fn fields(&self) -> Vec<PanelField> {
        let mut fields = vec![PanelField {
            key: "_monitor_note".into(),
            section: None,
            label: "Monitor Configuration".into(),
            description: "Monitor layout is read from the 'monitor' lines in hyprland.conf. \
                          A drag-and-drop editor will be available in a future release."
                .into(),
            field_type: FieldType::Text,
            current_value: String::new(),
        }];

        for mon in &self.monitors {
            fields.push(PanelField {
                key: format!("monitor.{}", mon.name),
                section: None,
                label: mon.name.clone(),
                description: mon.description.clone(),
                field_type: FieldType::Text,
                current_value: mon.description.clone(),
            });
        }

        fields
    }

    fn set_value(&mut self, key: &str, _value: &str) -> Result<String, PanelError> {
        // Display configuration is not directly editable through simple
        // key-value pairs yet.
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
        // Will render the monitor drag-and-drop layout later.
    }

    fn event(&mut self, _event: &Event, _bounds: Rect) -> EventResponse {
        EventResponse::ignored()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_note_field() {
        let panel = DisplayPanel::new();
        let fields = panel.fields();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].key, "_monitor_note");
    }

    #[test]
    fn monitors_appear_as_fields() {
        let mut panel = DisplayPanel::new();
        panel.set_monitors(vec![
            MonitorInfo {
                name: "DP-1".into(),
                description: "1920x1080@144".into(),
            },
            MonitorInfo {
                name: "HDMI-A-1".into(),
                description: "2560x1440@60".into(),
            },
        ]);
        let fields = panel.fields();
        // 1 note + 2 monitors
        assert_eq!(fields.len(), 3);
    }
}
