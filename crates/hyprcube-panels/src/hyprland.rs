use hyprcube_core::ipc::HyprlandIpc;
use hyprcube_core::layout::Rect;
use hyprcube_core::widget::{Constraints, Event, EventResponse, Size};
use hyprcube_hyprconf::HyprlandConfig;

use crate::{FieldType, PanelError, PanelField, SettingsPanel};

/// Settings panel for core Hyprland general options.
pub struct HyprlandPanel {
    config: HyprlandConfig,
    ipc: Option<HyprlandIpc>,
}

impl HyprlandPanel {
    /// Create a new panel, loading `hyprland.conf` from the default location.
    /// Falls back to an empty config if the file cannot be read.
    pub fn new() -> Self {
        let config = HyprlandConfig::load_default().unwrap_or_else(|e| {
            tracing::warn!("failed to load hyprland.conf, using empty config: {e}");
            HyprlandConfig { items: Vec::new() }
        });

        let ipc = HyprlandIpc::connect().ok();
        if ipc.is_none() {
            tracing::warn!("hyprland IPC not available — live preview disabled");
        }

        Self { config, ipc }
    }

    /// Borrow the underlying config.
    pub fn config(&self) -> &HyprlandConfig {
        &self.config
    }

    /// Borrow the IPC handle, if connected.
    pub fn ipc(&self) -> Option<&HyprlandIpc> {
        self.ipc.as_ref()
    }

    fn general_get(&self, key: &str) -> String {
        self.config
            .get_in_section("general", key)
            .unwrap_or("")
            .to_string()
    }

    fn general_set(&mut self, key: &str, value: &str) -> String {
        let old = self.general_get(key);
        self.config.set_in_section("general", key, value);
        old
    }
}

impl Default for HyprlandPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsPanel for HyprlandPanel {
    fn title(&self) -> &str {
        "Hyprland"
    }

    fn icon(&self) -> &str {
        "preferences-desktop"
    }

    fn fields(&self) -> Vec<PanelField> {
        vec![
            PanelField {
                key: "gaps_in".into(),
                label: "Inner Gaps".into(),
                description: "Gap size between tiled windows (px).".into(),
                field_type: FieldType::Integer {
                    min: Some(0),
                    max: Some(100),
                },
                current_value: self.general_get("gaps_in"),
            },
            PanelField {
                key: "gaps_out".into(),
                label: "Outer Gaps".into(),
                description: "Gap size between windows and monitor edges (px).".into(),
                field_type: FieldType::Integer {
                    min: Some(0),
                    max: Some(100),
                },
                current_value: self.general_get("gaps_out"),
            },
            PanelField {
                key: "border_size".into(),
                label: "Border Size".into(),
                description: "Window border thickness (px).".into(),
                field_type: FieldType::Integer {
                    min: Some(0),
                    max: Some(20),
                },
                current_value: self.general_get("border_size"),
            },
            PanelField {
                key: "col.active_border".into(),
                label: "Active Border Color".into(),
                description: "Border color of the focused window.".into(),
                field_type: FieldType::Color,
                current_value: self.general_get("col.active_border"),
            },
            PanelField {
                key: "col.inactive_border".into(),
                label: "Inactive Border Color".into(),
                description: "Border color of unfocused windows.".into(),
                field_type: FieldType::Color,
                current_value: self.general_get("col.inactive_border"),
            },
            PanelField {
                key: "layout".into(),
                label: "Layout".into(),
                description: "Tiling layout algorithm.".into(),
                field_type: FieldType::Choice {
                    options: vec!["dwindle".into(), "master".into()],
                },
                current_value: self.general_get("layout"),
            },
            PanelField {
                key: "resize_on_border".into(),
                label: "Resize on Border".into(),
                description: "Allow resizing windows by dragging their border.".into(),
                field_type: FieldType::Boolean,
                current_value: self.general_get("resize_on_border"),
            },
        ]
    }

    fn set_value(&mut self, key: &str, value: &str) -> Result<String, PanelError> {
        match key {
            "gaps_in" | "gaps_out" | "border_size" | "col.active_border"
            | "col.inactive_border" | "layout" | "resize_on_border" => {
                Ok(self.general_set(key, value))
            }
            _ => Err(PanelError::UnknownKey(key.to_string())),
        }
    }

    fn measure(&self, constraints: Constraints) -> Size {
        Size {
            width: constraints.max_width,
            height: constraints.max_height,
        }
    }

    fn paint(&self, _bounds: Rect, _pixmap: &mut tiny_skia::PixmapMut<'_>) {
        // Will be wired up to the widget system later.
    }

    fn event(&mut self, _event: &Event, _bounds: Rect) -> EventResponse {
        EventResponse::ignored()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel_with_config(input: &str) -> HyprlandPanel {
        let config = HyprlandConfig::parse(input).unwrap();
        HyprlandPanel { config, ipc: None }
    }

    #[test]
    fn fields_reflect_config() {
        let panel = panel_with_config(
            "general {\n    gaps_in = 5\n    gaps_out = 10\n    border_size = 2\n    layout = dwindle\n}",
        );
        let fields = panel.fields();
        assert_eq!(fields.len(), 7);

        let gaps_in = fields.iter().find(|f| f.key == "gaps_in").unwrap();
        assert_eq!(gaps_in.current_value, "5");

        let layout = fields.iter().find(|f| f.key == "layout").unwrap();
        assert_eq!(layout.current_value, "dwindle");
    }

    #[test]
    fn set_value_returns_old() {
        let mut panel = panel_with_config("general {\n    gaps_in = 5\n}");
        let old = panel.set_value("gaps_in", "8").unwrap();
        assert_eq!(old, "5");
        assert_eq!(panel.general_get("gaps_in"), "8");
    }

    #[test]
    fn set_value_unknown_key() {
        let mut panel = panel_with_config("");
        let err = panel.set_value("nonexistent", "val").unwrap_err();
        assert!(matches!(err, PanelError::UnknownKey(_)));
    }
}
