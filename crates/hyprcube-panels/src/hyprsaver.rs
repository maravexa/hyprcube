use std::path::PathBuf;

use hyprcube_core::layout::Rect;
use hyprcube_core::widget::{Constraints, Event, EventResponse, Size};

use crate::{FieldType, PanelError, PanelField, SettingsPanel};

/// Settings panel for HyprSaver (screensaver / lock screen).
pub struct HyprsaverPanel {
    config_dir: PathBuf,
    config: Option<toml::Table>,
}

impl HyprsaverPanel {
    pub fn new() -> Self {
        let config_dir = hyprsaver_config_dir();
        let config = if config_dir.is_dir() {
            load_hyprsaver_config(&config_dir)
        } else {
            None
        };
        Self { config_dir, config }
    }
}

impl Default for HyprsaverPanel {
    fn default() -> Self {
        Self::new()
    }
}

fn hyprsaver_config_dir() -> PathBuf {
    let config_dir = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            PathBuf::from(home).join(".config")
        });
    config_dir.join("hyprsaver")
}

fn load_hyprsaver_config(dir: &PathBuf) -> Option<toml::Table> {
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

impl SettingsPanel for HyprsaverPanel {
    fn title(&self) -> &str {
        "HyprSaver"
    }

    fn icon(&self) -> &str {
        "preferences-desktop-screensaver"
    }

    fn available(&self) -> bool {
        self.config_dir.is_dir()
    }

    fn fields(&self) -> Vec<PanelField> {
        let table = match &self.config {
            Some(t) => t,
            None => return Vec::new(),
        };

        vec![
            PanelField {
                key: "timeout".into(),
                label: "Timeout".into(),
                description: "Seconds of inactivity before the screensaver activates.".into(),
                field_type: FieldType::Integer {
                    min: Some(0),
                    max: Some(3600),
                },
                current_value: table_get(table, "timeout"),
            },
            PanelField {
                key: "lock_on_sleep".into(),
                label: "Lock on Sleep".into(),
                description: "Lock the screen when the system suspends.".into(),
                field_type: FieldType::Boolean,
                current_value: table_get(table, "lock_on_sleep"),
            },
            PanelField {
                key: "background".into(),
                label: "Background".into(),
                description: "Background color or image path for the lock screen.".into(),
                field_type: FieldType::Text,
                current_value: table_get(table, "background"),
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

    #[test]
    fn unavailable_when_dir_missing() {
        let panel = HyprsaverPanel {
            config_dir: PathBuf::from("/nonexistent/hyprsaver"),
            config: None,
        };
        assert!(!panel.available());
        assert!(panel.fields().is_empty());
    }

    #[test]
    fn fields_from_toml() {
        let mut table = toml::Table::new();
        table.insert("timeout".into(), toml::Value::String("300".into()));
        table.insert("lock_on_sleep".into(), toml::Value::String("true".into()));
        table.insert(
            "background".into(),
            toml::Value::String("#000000".into()),
        );

        let panel = HyprsaverPanel {
            config_dir: PathBuf::from("/tmp"),
            config: Some(table),
        };
        let fields = panel.fields();
        assert_eq!(fields.len(), 3);

        let timeout = fields.iter().find(|f| f.key == "timeout").unwrap();
        assert_eq!(timeout.current_value, "300");
    }
}
