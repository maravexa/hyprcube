use hyprcube_core::layout::Rect;
use hyprcube_core::widget::{Constraints, Event, EventResponse, Size};

use crate::{FieldType, PanelError, PanelField, SettingsPanel};

/// Settings panel for HyprSaver (screensaver / lock screen).
pub struct HyprsaverPanel {
    config: Option<toml::Table>,
}

impl HyprsaverPanel {
    pub fn new() -> Self {
        let config = load_hyprsaver_config();
        Self { config }
    }
}

impl Default for HyprsaverPanel {
    fn default() -> Self {
        Self::new()
    }
}

fn load_hyprsaver_config() -> Option<toml::Table> {
    let path = hyprcube_core::hypr_config_dir().join("hyprsaver.toml");
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

impl SettingsPanel for HyprsaverPanel {
    fn title(&self) -> &str {
        "HyprSaver"
    }

    fn icon(&self) -> &str {
        "preferences-desktop-screensaver"
    }

    fn available(&self) -> bool {
        hyprcube_core::hypr_config_dir().join("hyprsaver.toml").is_file()
    }

    fn fields(&self) -> Vec<PanelField> {
        let table = match &self.config {
            Some(t) => t,
            None => {
                return vec![PanelField {
                    key: "_not_installed".into(),
                    section: None,
                    label: "HyprSaver is not installed".into(),
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
                key: "timeout".into(),
                section: None,
                label: "Timeout".into(),
                description: "Seconds of inactivity before the screensaver activates.".into(),
                field_type: FieldType::Integer {
                    min: Some(0),
                    max: Some(3600),
                },
                current_value: table_get(table, "timeout"),
                original_value: table_get_opt(table, "timeout"),
                dirty: false,
            },
            PanelField {
                key: "lock_on_sleep".into(),
                section: None,
                label: "Lock on Sleep".into(),
                description: "Lock the screen when the system suspends.".into(),
                field_type: FieldType::Boolean,
                current_value: table_get(table, "lock_on_sleep"),
                original_value: table_get_opt(table, "lock_on_sleep"),
                dirty: false,
            },
            PanelField {
                key: "background".into(),
                section: None,
                label: "Background".into(),
                description: "Background color or image path for the lock screen.".into(),
                field_type: FieldType::Text,
                current_value: table_get(table, "background"),
                original_value: table_get_opt(table, "background"),
                dirty: false,
            },
        ]
    }

    fn set_value(&mut self, key: &str, value: &str) -> Result<String, PanelError> {
        let table = self
            .config
            .as_mut()
            .ok_or_else(|| PanelError::Config("hyprsaver config not loaded".into()))?;

        match key {
            "timeout" | "lock_on_sleep" | "background" => {
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
        let panel = HyprsaverPanel { config: None };
        let fields = panel.fields();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].key, "_not_installed");
        assert!(fields[0].original_value.is_none());
    }

    #[test]
    fn fields_from_toml() {
        let mut table = toml::Table::new();
        table.insert("timeout".into(), toml::Value::String("300".into()));
        table.insert("lock_on_sleep".into(), toml::Value::String("true".into()));
        table.insert("background".into(), toml::Value::String("#000000".into()));

        let panel = HyprsaverPanel { config: Some(table) };
        let fields = panel.fields();
        assert_eq!(fields.len(), 3);

        let timeout = fields.iter().find(|f| f.key == "timeout").unwrap();
        assert_eq!(timeout.current_value, "300");
        assert_eq!(timeout.original_value, Some("300".into()));
        assert!(!timeout.dirty);
    }

    #[test]
    fn missing_toml_key_has_none_original_value() {
        let table = toml::Table::new();
        let panel = HyprsaverPanel { config: Some(table) };
        let fields = panel.fields();
        let timeout = fields.iter().find(|f| f.key == "timeout").unwrap();
        assert_eq!(timeout.original_value, None);
        assert_eq!(timeout.current_value, "");
    }
}
