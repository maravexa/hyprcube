use std::sync::{Arc, Mutex};

use hyprcube_core::layout::Rect;
use hyprcube_core::palette::Palette;
use hyprcube_core::widget::{Constraints, Event, EventResponse, Size};

use crate::{FieldType, PanelError, PanelField, SettingsPanel};

/// Settings panel for visual appearance (palette, fonts, style).
pub struct AppearancePanel {
    palette: Arc<Mutex<Palette>>,
}

impl AppearancePanel {
    /// Load the palette from the default location, falling back to built-in
    /// defaults.
    pub fn new() -> Self {
        let palette = Palette::load(None).unwrap_or_else(|e| {
            tracing::warn!("failed to load palette, using defaults: {e}");
            Palette::default()
        });
        Self::with_arc(Arc::new(Mutex::new(palette)))
    }

    /// Construct from a shared palette Arc (used by the registry).
    pub fn with_arc(palette: Arc<Mutex<Palette>>) -> Self {
        Self { palette }
    }
}

impl Default for AppearancePanel {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsPanel for AppearancePanel {
    fn title(&self) -> &str {
        "Appearance"
    }

    fn icon(&self) -> &str {
        "preferences-desktop-theme"
    }

    fn fields(&self) -> Vec<PanelField> {
        let pal = self.palette.lock().unwrap();
        vec![
            PanelField {
                key: "colors.background".into(),
                section: None,
                label: "Background".into(),
                description: "Primary background color.".into(),
                field_type: FieldType::Color,
                current_value: pal.colors.background.clone(),
            },
            PanelField {
                key: "colors.foreground".into(),
                section: None,
                label: "Foreground".into(),
                description: "Primary text color.".into(),
                field_type: FieldType::Color,
                current_value: pal.colors.foreground.clone(),
            },
            PanelField {
                key: "colors.accent".into(),
                section: None,
                label: "Accent".into(),
                description: "Accent / highlight color.".into(),
                field_type: FieldType::Color,
                current_value: pal.colors.accent.clone(),
            },
            PanelField {
                key: "colors.urgent".into(),
                section: None,
                label: "Urgent".into(),
                description: "Color for urgent / error states.".into(),
                field_type: FieldType::Color,
                current_value: pal.colors.urgent.clone(),
            },
            PanelField {
                key: "colors.surface".into(),
                section: None,
                label: "Surface".into(),
                description: "Surface / card background color.".into(),
                field_type: FieldType::Color,
                current_value: pal.colors.surface.clone(),
            },
            PanelField {
                key: "colors.overlay".into(),
                section: None,
                label: "Overlay".into(),
                description: "Overlay / muted text color.".into(),
                field_type: FieldType::Color,
                current_value: pal.colors.overlay.clone(),
            },
            PanelField {
                key: "fonts.family".into(),
                section: None,
                label: "Font Family".into(),
                description: "Primary UI font family.".into(),
                field_type: FieldType::Text,
                current_value: pal.fonts.family.clone(),
            },
            PanelField {
                key: "fonts.mono_family".into(),
                section: None,
                label: "Monospace Font".into(),
                description: "Monospace font for code and config values.".into(),
                field_type: FieldType::Text,
                current_value: pal.fonts.mono_family.clone(),
            },
            PanelField {
                key: "fonts.size".into(),
                section: None,
                label: "Font Size".into(),
                description: "Base font size (pt).".into(),
                field_type: FieldType::Float {
                    min: Some(6.0),
                    max: Some(48.0),
                },
                current_value: pal.fonts.size.to_string(),
            },
            PanelField {
                key: "style.border_radius".into(),
                section: None,
                label: "Border Radius".into(),
                description: "Corner rounding for UI elements (px).".into(),
                field_type: FieldType::Float {
                    min: Some(0.0),
                    max: Some(32.0),
                },
                current_value: pal.style.border_radius.to_string(),
            },
            PanelField {
                key: "style.opacity".into(),
                section: None,
                label: "Opacity".into(),
                description: "Window opacity (0.0 – 1.0).".into(),
                field_type: FieldType::Float {
                    min: Some(0.0),
                    max: Some(1.0),
                },
                current_value: pal.style.opacity.to_string(),
            },
        ]
    }

    fn set_value(&mut self, key: &str, value: &str) -> Result<String, PanelError> {
        let mut pal = self.palette.lock().unwrap();
        match key {
            "colors.background" => {
                let old = pal.colors.background.clone();
                pal.colors.background = value.to_string();
                Ok(old)
            }
            "colors.foreground" => {
                let old = pal.colors.foreground.clone();
                pal.colors.foreground = value.to_string();
                Ok(old)
            }
            "colors.accent" => {
                let old = pal.colors.accent.clone();
                pal.colors.accent = value.to_string();
                Ok(old)
            }
            "colors.urgent" => {
                let old = pal.colors.urgent.clone();
                pal.colors.urgent = value.to_string();
                Ok(old)
            }
            "colors.surface" => {
                let old = pal.colors.surface.clone();
                pal.colors.surface = value.to_string();
                Ok(old)
            }
            "colors.overlay" => {
                let old = pal.colors.overlay.clone();
                pal.colors.overlay = value.to_string();
                Ok(old)
            }
            "fonts.family" => {
                let old = pal.fonts.family.clone();
                pal.fonts.family = value.to_string();
                Ok(old)
            }
            "fonts.mono_family" => {
                let old = pal.fonts.mono_family.clone();
                pal.fonts.mono_family = value.to_string();
                Ok(old)
            }
            "fonts.size" => {
                let new: f64 = value.parse().map_err(|_| PanelError::InvalidValue {
                    key: key.to_string(),
                    reason: "expected a number".into(),
                })?;
                let old = pal.fonts.size.to_string();
                pal.fonts.size = new;
                Ok(old)
            }
            "style.border_radius" => {
                let new: f64 = value.parse().map_err(|_| PanelError::InvalidValue {
                    key: key.to_string(),
                    reason: "expected a number".into(),
                })?;
                let old = pal.style.border_radius.to_string();
                pal.style.border_radius = new;
                Ok(old)
            }
            "style.opacity" => {
                let new: f64 = value.parse().map_err(|_| PanelError::InvalidValue {
                    key: key.to_string(),
                    reason: "expected a number".into(),
                })?;
                let old = pal.style.opacity.to_string();
                pal.style.opacity = new;
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

    #[test]
    fn default_palette_fields() {
        let panel = AppearancePanel {
            palette: Arc::new(Mutex::new(Palette::default())),
        };
        let fields = panel.fields();
        assert_eq!(fields.len(), 11);

        let bg = fields.iter().find(|f| f.key == "colors.background").unwrap();
        assert_eq!(bg.current_value, "#1e1e2e");
    }

    #[test]
    fn set_color_value() {
        let mut panel = AppearancePanel {
            palette: Arc::new(Mutex::new(Palette::default())),
        };
        let old = panel.set_value("colors.accent", "#ff0000").unwrap();
        assert_eq!(old, "#89b4fa");
        assert_eq!(panel.palette.lock().unwrap().colors.accent, "#ff0000");
    }

    #[test]
    fn set_float_value() {
        let mut panel = AppearancePanel {
            palette: Arc::new(Mutex::new(Palette::default())),
        };
        let old = panel.set_value("style.opacity", "0.95").unwrap();
        assert_eq!(old, "0.85");
        assert!((panel.palette.lock().unwrap().style.opacity - 0.95).abs() < f64::EPSILON);
    }

    #[test]
    fn set_invalid_float() {
        let mut panel = AppearancePanel {
            palette: Arc::new(Mutex::new(Palette::default())),
        };
        let err = panel.set_value("fonts.size", "not_a_number").unwrap_err();
        assert!(matches!(err, PanelError::InvalidValue { .. }));
    }
}
