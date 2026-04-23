use hyprcube_core::layout::Rect;
use hyprcube_core::widget::{Constraints, Event, EventResponse, Size};

use crate::{FieldType, PanelError, PanelField, SettingsPanel};

/// Settings panel for HyprDeck (application launcher / dock).
pub struct HyprdeckPanel {
    config: Option<toml::Table>,
}

impl HyprdeckPanel {
    pub fn new() -> Self {
        let config = load_hyprdeck_config();
        Self { config }
    }
}

impl Default for HyprdeckPanel {
    fn default() -> Self {
        Self::new()
    }
}

fn load_hyprdeck_config() -> Option<toml::Table> {
    let path = hyprcube_core::hypr_config_dir().join("hyprdeck.toml");
    let content = std::fs::read_to_string(path).ok()?;
    content.parse::<toml::Table>().ok()
}

fn table_get(table: &toml::Table, key: &str) -> String {
    table
        .get(key)
        .map(|v| match v {
            toml::Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .unwrap_or_default()
}

fn table_get_opt(table: &toml::Table, key: &str) -> Option<String> {
    table.get(key).map(|v| match v {
        toml::Value::String(s) => s.clone(),
        other => other.to_string(),
    })
}

impl SettingsPanel for HyprdeckPanel {
    fn title(&self) -> &str {
        "HyprDeck"
    }

    fn icon(&self) -> &str {
        "application-menu"
    }

    fn available(&self) -> bool {
        hyprcube_core::hypr_config_dir().join("hyprdeck.toml").is_file()
    }

    fn fields(&self) -> Vec<PanelField> {
        let table = match &self.config {
            Some(t) => t,
            None => {
                return vec![PanelField {
                    key: "_not_installed".into(),
                    section: None,
                    label: "HyprDeck is not installed".into(),
                    description: String::new(),
                    field_type: FieldType::Text,
                    current_value: String::new(),
                    original_value: None,
                    dirty: false,
                }];
            }
        };

        vec![
            PanelField {
                key: "icon_size".into(),
                section: None,
                label: "Icon Size".into(),
                description: "Size of dock icons (px).".into(),
                field_type: FieldType::Integer {
                    min: Some(16),
                    max: Some(128),
                },
                current_value: table_get(table, "icon_size"),
                original_value: table_get_opt(table, "icon_size"),
                dirty: false,
            },
            PanelField {
                key: "position".into(),
                section: None,
                label: "Position".into(),
                description: "Dock position on screen.".into(),
                field_type: FieldType::Choice {
                    options: vec![
                        "bottom".into(),
                        "top".into(),
                        "left".into(),
                        "right".into(),
                    ],
                },
                current_value: table_get(table, "position"),
                original_value: table_get_opt(table, "position"),
                dirty: false,
            },
            PanelField {
                key: "auto_hide".into(),
                section: None,
                label: "Auto-Hide".into(),
                description: "Automatically hide the dock when not in use.".into(),
                field_type: FieldType::Boolean,
                current_value: table_get(table, "auto_hide"),
                original_value: table_get_opt(table, "auto_hide"),
                dirty: false,
            },
        ]
    }

    fn set_value(&mut self, key: &str, value: &str) -> Result<String, PanelError> {
        let table = self
            .config
            .as_mut()
            .ok_or_else(|| PanelError::Config("hyprdeck config not loaded".into()))?;

        match key {
            "icon_size" | "position" | "auto_hide" => {
                let old = table_get(table, key);
                table.insert(key.to_string(), toml::Value::String(value.to_string()));
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
        // Will be wired up to the widget system later.
    }

    fn event(&mut self, _event: &Event, _bounds: Rect) -> EventResponse {
        EventResponse::ignored()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_installed_message_when_config_missing() {
        let panel = HyprdeckPanel { config: None };
        let fields = panel.fields();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].key, "_not_installed");
        assert!(fields[0].original_value.is_none());
    }

    #[test]
    fn fields_from_toml() {
        let mut table = toml::Table::new();
        table.insert("icon_size".into(), toml::Value::String("48".into()));
        table.insert("position".into(), toml::Value::String("bottom".into()));
        table.insert("auto_hide".into(), toml::Value::String("true".into()));

        let panel = HyprdeckPanel { config: Some(table) };
        let fields = panel.fields();
        assert_eq!(fields.len(), 3);

        let icon = fields.iter().find(|f| f.key == "icon_size").unwrap();
        assert_eq!(icon.current_value, "48");
        assert_eq!(icon.original_value, Some("48".into()));
        assert!(!icon.dirty);
    }

    #[test]
    fn missing_toml_key_has_none_original_value() {
        let table = toml::Table::new();
        let panel = HyprdeckPanel { config: Some(table) };
        let fields = panel.fields();
        let icon = fields.iter().find(|f| f.key == "icon_size").unwrap();
        assert_eq!(icon.original_value, None);
        assert_eq!(icon.current_value, "");
    }

    #[test]
    fn set_value_in_table() {
        let mut table = toml::Table::new();
        table.insert("position".into(), toml::Value::String("bottom".into()));

        let mut panel = HyprdeckPanel { config: Some(table) };
        let old = panel.set_value("position", "top").unwrap();
        assert_eq!(old, "bottom");
    }
}
