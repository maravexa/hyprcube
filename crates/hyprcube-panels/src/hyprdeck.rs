use std::path::PathBuf;

use hyprcube_core::layout::Rect;
use hyprcube_core::widget::{Constraints, Event, EventResponse, Size};

use crate::{FieldType, PanelError, PanelField, SettingsPanel};

/// Settings panel for HyprDeck (application launcher / dock).
pub struct HyprdeckPanel {
    config: Option<toml::Table>,
}

impl HyprdeckPanel {
    pub fn new() -> Self {
        let config_dir = hyprdeck_config_dir();
        let config = if config_dir.is_dir() { load_hyprdeck_config(&config_dir) } else { None };
        Self { config }
    }
}

impl Default for HyprdeckPanel {
    fn default() -> Self {
        Self::new()
    }
}

fn hyprdeck_config_dir() -> PathBuf {
    let config_dir = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            PathBuf::from(home).join(".config")
        });
    config_dir.join("hyprdeck")
}

fn load_hyprdeck_config(dir: &PathBuf) -> Option<toml::Table> {
    let path = dir.join("config.toml");
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

impl SettingsPanel for HyprdeckPanel {
    fn title(&self) -> &str {
        "HyprDeck"
    }

    fn icon(&self) -> &str {
        "application-menu"
    }

    fn fields(&self) -> Vec<PanelField> {
        let table = match &self.config {
            Some(t) => t,
            None => {
                return vec![PanelField {
                    key: "_not_installed".into(),
                    label: "HyprDeck is not installed".into(),
                    description: String::new(),
                    field_type: FieldType::Text,
                    current_value: String::new(),
                }];
            }
        };

        vec![
            PanelField {
                key: "icon_size".into(),
                label: "Icon Size".into(),
                description: "Size of dock icons (px).".into(),
                field_type: FieldType::Integer {
                    min: Some(16),
                    max: Some(128),
                },
                current_value: table_get(table, "icon_size"),
            },
            PanelField {
                key: "position".into(),
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
            },
            PanelField {
                key: "auto_hide".into(),
                label: "Auto-Hide".into(),
                description: "Automatically hide the dock when not in use.".into(),
                field_type: FieldType::Boolean,
                current_value: table_get(table, "auto_hide"),
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
    fn not_installed_message_when_dir_missing() {
        let panel = HyprdeckPanel { config: None };
        let fields = panel.fields();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].key, "_not_installed");
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
