use std::sync::{Arc, Mutex};

use hyprcube_core::layout::Rect;
use hyprcube_core::widget::{Constraints, Event, EventResponse, Size};
use hyprcube_hyprconf::HyprlandConfig;

use crate::{FieldType, PanelError, PanelField, SettingsPanel};

/// Settings panel for Hyprland input configuration (keyboard, mouse,
/// touchpad).
pub struct InputPanel {
    config: Arc<Mutex<HyprlandConfig>>,
}

impl InputPanel {
    pub fn new() -> Self {
        let config = HyprlandConfig::load_default().unwrap_or_else(|e| {
            tracing::warn!("failed to load hyprland.conf for input panel: {e}");
            HyprlandConfig { items: Vec::new() }
        });
        Self::with_arc(Arc::new(Mutex::new(config)))
    }

    /// Construct from a shared config Arc (used by the registry so that
    /// HyprlandPanel and InputPanel operate on the same in-memory config).
    pub fn with_arc(config: Arc<Mutex<HyprlandConfig>>) -> Self {
        Self { config }
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
        let cfg = self.config.lock().unwrap();

        let inp = |key: &str| -> Option<String> {
            cfg.get_in_section("input", key).map(|s| s.to_string())
        };
        let tpad = |key: &str| -> Option<String> {
            cfg.get_in_nested(&["input", "touchpad"], key).map(|s| s.to_string())
        };

        let make_inp = |key: &'static str, label: &'static str, desc: &'static str, ft: FieldType| {
            let original = inp(key);
            let current = original.clone().unwrap_or_default();
            PanelField {
                key: key.into(),
                section: Some("input".into()),
                label: label.into(),
                description: desc.into(),
                field_type: ft,
                current_value: current,
                original_value: original,
                dirty: false,
            }
        };

        let make_tpad = |panel_key: &'static str, real_key: &'static str, label: &'static str, desc: &'static str, ft: FieldType| {
            let original = tpad(real_key);
            let current = original.clone().unwrap_or_default();
            PanelField {
                key: panel_key.into(),
                section: Some("input:touchpad".into()),
                label: label.into(),
                description: desc.into(),
                field_type: ft,
                current_value: current,
                original_value: original,
                dirty: false,
            }
        };

        vec![
            make_inp("kb_layout", "Keyboard Layout", "XKB keyboard layout (e.g. us, de, fr).",
                FieldType::Text),
            make_inp("follow_mouse", "Follow Mouse", "Focus behavior when the cursor enters a window.",
                FieldType::Choice { options: vec!["0".into(), "1".into(), "2".into(), "3".into()] }),
            make_inp("sensitivity", "Sensitivity", "Pointer sensitivity (-1.0 to 1.0).",
                FieldType::Float { min: Some(-1.0), max: Some(1.0) }),
            make_inp("accel_profile", "Acceleration Profile", "Pointer acceleration profile.",
                FieldType::Choice { options: vec!["flat".into(), "adaptive".into()] }),
            make_inp("scroll_method", "Scroll Method", "Scroll input method.",
                FieldType::Choice { options: vec!["2fg".into(), "edge".into(), "on_button_down".into(), "no_scroll".into()] }),
            make_tpad("touchpad.natural_scroll", "natural_scroll",
                "Natural Scroll", "Invert touchpad scroll direction.", FieldType::Boolean),
            make_tpad("touchpad.tap-to-click", "tap-to-click",
                "Tap to Click", "Enable tap-to-click on the touchpad.", FieldType::Boolean),
        ]
    }

    fn set_value(&mut self, key: &str, value: &str) -> Result<String, PanelError> {
        match key {
            "kb_layout" | "follow_mouse" | "sensitivity" | "accel_profile" | "scroll_method" => {
                let mut cfg = self.config.lock().unwrap();
                let old = cfg.get_in_section("input", key).unwrap_or("").to_string();
                cfg.set_in_section("input", key, value);
                Ok(old)
            }
            "touchpad.natural_scroll" => {
                let real_key = "natural_scroll";
                let mut cfg = self.config.lock().unwrap();
                let old = cfg
                    .get_in_nested(&["input", "touchpad"], real_key)
                    .unwrap_or("")
                    .to_string();
                cfg.set_in_nested(&["input", "touchpad"], real_key, value);
                Ok(old)
            }
            "touchpad.tap-to-click" => {
                let real_key = "tap-to-click";
                let mut cfg = self.config.lock().unwrap();
                let old = cfg
                    .get_in_nested(&["input", "touchpad"], real_key)
                    .unwrap_or("")
                    .to_string();
                cfg.set_in_nested(&["input", "touchpad"], real_key, value);
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

    fn paint(
        &self,
        _bounds: Rect,
        _pixmap: &mut tiny_skia::PixmapMut<'_>,
        _ctx: &mut hyprcube_core::text::RenderContext<'_>,
    ) {
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
        InputPanel { config: Arc::new(Mutex::new(config)) }
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
        assert_eq!(kb.section.as_deref(), Some("input"));

        let ns = fields
            .iter()
            .find(|f| f.key == "touchpad.natural_scroll")
            .unwrap();
        assert_eq!(ns.current_value, "true");
        assert_eq!(ns.section.as_deref(), Some("input:touchpad"));
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
        let fields = panel.fields();
        let ns = fields.iter().find(|f| f.key == "touchpad.natural_scroll").unwrap();
        assert_eq!(ns.current_value, "true");
    }
}
