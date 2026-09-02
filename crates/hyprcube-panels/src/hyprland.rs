use std::sync::{Arc, Mutex};

use hyprcube_core::color::Color;
use hyprcube_core::layout::Rect;
use hyprcube_core::widget::{Constraints, Event, EventResponse, Size};
use hyprcube_hyprconf::HyprlandConfig;

use crate::{FieldType, PanelError, PanelField, SettingsPanel};

/// Keys in the `general` section that use Hyprland's `rgba()` color format.
const RGBA_KEYS: &[&str] = &["col.active_border", "col.inactive_border"];

/// Normalize a raw config value for a Color field to a hex string the picker can parse.
/// Handles both `rgba(rrggbbaa)` and `#rrggbb`/`#rrggbbaa` formats.
fn normalize_to_hex(raw: &str) -> String {
    // Try rgba() first, then plain hex.
    Color::from_hypr_rgba(raw)
        .or_else(|_| Color::from_hex(raw))
        .map(|c| c.to_hex())
        .unwrap_or_else(|_| raw.to_string())
}

/// Convert a hex color string back to `rgba()` format for keys that require it.
fn hex_to_rgba(hex: &str) -> String {
    Color::from_hex(hex)
        .or_else(|_| Color::from_hypr_rgba(hex))
        .map(|c| c.to_hypr_rgba())
        .unwrap_or_else(|_| hex.to_string())
}

/// Settings panel for core Hyprland general options.
pub struct HyprlandPanel {
    config: Arc<Mutex<HyprlandConfig>>,
}

impl HyprlandPanel {
    /// Create a new panel loading `hyprland.conf` from the default location.
    /// Falls back to an empty config if the file cannot be read.
    pub fn new() -> Self {
        let config = HyprlandConfig::load_default().unwrap_or_else(|e| {
            tracing::warn!("failed to load hyprland.conf, using empty config: {e}");
            HyprlandConfig { items: Vec::new() }
        });
        Self::with_arc(Arc::new(Mutex::new(config)))
    }

    /// Construct from a shared config Arc (used by the registry so that
    /// HyprlandPanel and InputPanel operate on the same in-memory config).
    pub fn with_arc(config: Arc<Mutex<HyprlandConfig>>) -> Self {
        Self { config }
    }

    fn general_set(&mut self, key: &str, value: &str) -> String {
        let mut cfg = self.config.lock().unwrap();
        let old = cfg.get_in_section("general", key).unwrap_or("").to_string();
        cfg.set_in_section("general", key, value);
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
        let cfg = self.config.lock().unwrap();
        let gen = |key: &str, section: &str| -> Option<String> {
            cfg.get_in_section(section, key).map(|s| s.to_string())
        };

        let make = |key: &'static str, label: &'static str, desc: &'static str, ft: FieldType| {
            let original = gen(key, "general");
            let raw = original.clone().unwrap_or_default();
            // Normalize color values so the picker always receives a hex string.
            let current = if matches!(ft, FieldType::Color) {
                normalize_to_hex(&raw)
            } else {
                raw.clone()
            };
            let original_normalized = original.map(|v| {
                if matches!(ft, FieldType::Color) {
                    normalize_to_hex(&v)
                } else {
                    v
                }
            });
            PanelField {
                key: key.into(),
                section: Some("general".into()),
                label: label.into(),
                description: desc.into(),
                field_type: ft,
                current_value: current,
                original_value: original_normalized,
                dirty: false,
            }
        };

        vec![
            make(
                "gaps_in",
                "Inner Gaps",
                "Gap size between tiled windows (px).",
                FieldType::Integer {
                    min: Some(0),
                    max: Some(100),
                },
            ),
            make(
                "gaps_out",
                "Outer Gaps",
                "Gap size between windows and monitor edges (px).",
                FieldType::Integer {
                    min: Some(0),
                    max: Some(100),
                },
            ),
            make(
                "border_size",
                "Border Size",
                "Window border thickness (px).",
                FieldType::Integer {
                    min: Some(0),
                    max: Some(20),
                },
            ),
            make(
                "col.active_border",
                "Active Border Color",
                "Border color of the focused window.",
                FieldType::Color,
            ),
            make(
                "col.inactive_border",
                "Inactive Border Color",
                "Border color of unfocused windows.",
                FieldType::Color,
            ),
            make(
                "layout",
                "Layout",
                "Tiling layout algorithm.",
                FieldType::Choice {
                    options: vec!["dwindle".into(), "master".into()],
                },
            ),
            make(
                "resize_on_border",
                "Resize on Border",
                "Allow resizing windows by dragging their border.",
                FieldType::Boolean,
            ),
        ]
    }

    fn set_value(&mut self, key: &str, value: &str) -> Result<String, PanelError> {
        match key {
            "gaps_in" | "gaps_out" | "border_size" | "layout" | "resize_on_border" => {
                Ok(self.general_set(key, value))
            }
            "col.active_border" | "col.inactive_border" => {
                // Config stores rgba() format; the picker emits hex.
                let config_value = if RGBA_KEYS.contains(&key) {
                    hex_to_rgba(value)
                } else {
                    value.to_string()
                };
                let raw_old = self.general_set(key, &config_value);
                // Return normalized hex so the undo stack has consistent values.
                Ok(normalize_to_hex(&raw_old))
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

    fn panel_with_config(input: &str) -> HyprlandPanel {
        let config = HyprlandConfig::parse(input).unwrap();
        HyprlandPanel {
            config: Arc::new(Mutex::new(config)),
        }
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
    fn fields_have_section_tags() {
        let panel = panel_with_config("general {\n    gaps_in = 5\n}");
        let fields = panel.fields();
        for f in &fields {
            assert_eq!(
                f.section.as_deref(),
                Some("general"),
                "field {} missing section",
                f.key
            );
        }
    }

    #[test]
    fn set_value_returns_old() {
        let mut panel = panel_with_config("general {\n    gaps_in = 5\n}");
        let old = panel.set_value("gaps_in", "8").unwrap();
        assert_eq!(old, "5");
        let fields = panel.fields();
        let f = fields.iter().find(|f| f.key == "gaps_in").unwrap();
        assert_eq!(f.current_value, "8");
    }

    #[test]
    fn set_value_unknown_key() {
        let mut panel = panel_with_config("");
        let err = panel.set_value("nonexistent", "val").unwrap_err();
        assert!(matches!(err, PanelError::UnknownKey(_)));
    }
}
