use hyprcube_core::layout::Rect;
use hyprcube_core::widget::{Constraints, Event, EventResponse, Size};
use hyprcube_hyprconf::HyprlandConfig;

use crate::{FieldType, PanelError, PanelField, SettingsPanel};

/// Settings panel for Hyprland input configuration (keyboard, mouse,
/// touchpad).
pub struct InputPanel {
    config: HyprlandConfig,
}

impl InputPanel {
    pub fn new() -> Self {
        let config = HyprlandConfig::load_default().unwrap_or_else(|e| {
            tracing::warn!("failed to load hyprland.conf for input panel: {e}");
            HyprlandConfig { items: Vec::new() }
        });
        Self { config }
    }

    fn input_get(&self, key: &str) -> String {
        self.config
            .get_in_section("input", key)
            .unwrap_or("")
            .to_string()
    }

    fn touchpad_get(&self, key: &str) -> String {
        self.config
            .get_in_nested(&["input", "touchpad"], key)
            .unwrap_or("")
            .to_string()
    }
}

impl Default for InputPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsPanel for InputPanel {
    fn title(&self) -> &str {
        "Input"
    }

    fn icon(&self) -> &str {
        "input-keyboard"
    }

    fn fields(&self) -> Vec<PanelField> {
        vec![
            PanelField {
                key: "kb_layout".into(),
                label: "Keyboard Layout".into(),
                description: "XKB keyboard layout (e.g. us, de, fr).".into(),
                field_type: FieldType::Text,
                current_value: self.input_get("kb_layout"),
            },
            PanelField {
                key: "follow_mouse".into(),
                label: "Follow Mouse".into(),
                description: "Focus behavior when the cursor enters a window.".into(),
                field_type: FieldType::Choice {
                    options: vec!["0".into(), "1".into(), "2".into(), "3".into()],
                },
                current_value: self.input_get("follow_mouse"),
            },
            PanelField {
                key: "sensitivity".into(),
                label: "Sensitivity".into(),
                description: "Pointer sensitivity (-1.0 to 1.0).".into(),
                field_type: FieldType::Float {
                    min: Some(-1.0),
                    max: Some(1.0),
                },
                current_value: self.input_get("sensitivity"),
            },
            PanelField {
                key: "accel_profile".into(),
                label: "Acceleration Profile".into(),
                description: "Pointer acceleration profile.".into(),
                field_type: FieldType::Choice {
                    options: vec!["flat".into(), "adaptive".into()],
                },
                current_value: self.input_get("accel_profile"),
            },
            PanelField {
                key: "scroll_method".into(),
                label: "Scroll Method".into(),
                description: "Scroll input method.".into(),
                field_type: FieldType::Choice {
                    options: vec![
                        "2fg".into(),
                        "edge".into(),
                        "on_button_down".into(),
                        "no_scroll".into(),
                    ],
                },
                current_value: self.input_get("scroll_method"),
            },
            PanelField {
                key: "touchpad.natural_scroll".into(),
                label: "Natural Scroll".into(),
                description: "Invert touchpad scroll direction.".into(),
                field_type: FieldType::Boolean,
                current_value: self.touchpad_get("natural_scroll"),
            },
            PanelField {
                key: "touchpad.tap-to-click".into(),
                label: "Tap to Click".into(),
                description: "Enable tap-to-click on the touchpad.".into(),
                field_type: FieldType::Boolean,
                current_value: self.touchpad_get("tap-to-click"),
            },
        ]
    }

    fn set_value(&mut self, key: &str, value: &str) -> Result<String, PanelError> {
        match key {
            "kb_layout" | "follow_mouse" | "sensitivity" | "accel_profile" | "scroll_method" => {
                let old = self.input_get(key);
                self.config.set_in_section("input", key, value);
                Ok(old)
            }
            "touchpad.natural_scroll" => {
                let real_key = "natural_scroll";
                let old = self.touchpad_get(real_key);
                self.config
                    .set_in_nested(&["input", "touchpad"], real_key, value);
                Ok(old)
            }
            "touchpad.tap-to-click" => {
                let real_key = "tap-to-click";
                let old = self.touchpad_get(real_key);
                self.config
                    .set_in_nested(&["input", "touchpad"], real_key, value);
                Ok(old)
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

    fn panel_with_config(input: &str) -> InputPanel {
        let config = HyprlandConfig::parse(input).unwrap();
        InputPanel { config }
    }

    #[test]
    fn fields_from_config() {
        let panel = panel_with_config(
            "input {\n    kb_layout = us\n    sensitivity = 0.5\n    touchpad {\n        natural_scroll = true\n    }\n}",
        );
        let fields = panel.fields();
        assert_eq!(fields.len(), 7);

        let kb = fields.iter().find(|f| f.key == "kb_layout").unwrap();
        assert_eq!(kb.current_value, "us");

        let ns = fields
            .iter()
            .find(|f| f.key == "touchpad.natural_scroll")
            .unwrap();
        assert_eq!(ns.current_value, "true");
    }

    #[test]
    fn set_input_value() {
        let mut panel = panel_with_config("input {\n    sensitivity = 0.0\n}");
        let old = panel.set_value("sensitivity", "0.5").unwrap();
        assert_eq!(old, "0.0");
    }

    #[test]
    fn set_touchpad_value() {
        let mut panel = panel_with_config(
            "input {\n    touchpad {\n        natural_scroll = false\n    }\n}",
        );
        let old = panel
            .set_value("touchpad.natural_scroll", "true")
            .unwrap();
        assert_eq!(old, "false");
        assert_eq!(panel.touchpad_get("natural_scroll"), "true");
    }
}
