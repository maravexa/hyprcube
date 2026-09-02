use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use hyprcube_core::layout::Rect;
use hyprcube_core::widget::{Constraints, Event, EventResponse, Size};
use serde::Deserialize;

use crate::{FieldType, PanelError, PanelField, SettingsPanel};

const CONFIG_CONTRACT_VERSION: u32 = 1;
const RESTART_NOTICE: &str = "Restart HyprDeck to apply these settings.";

/// HyprDeck's configuration editor.
///
/// Module controls are read from `hyprdeck --print-config-schema` so this UI
/// cannot drift from HyprDeck's versioned configuration contract.
pub struct HyprdeckPanel {
    path: PathBuf,
    config: Option<toml::Table>,
    config_error: Option<String>,
    schema: Option<ConfigSchema>,
    schema_error: Option<String>,
    dirty: bool,
}

impl HyprdeckPanel {
    pub fn new() -> Self {
        let path = hyprcube_core::hypr_config_dir().join("hyprdeck.toml");
        let (config, config_error) = load_initial_config(&path);
        let (schema, schema_error) = match load_schema() {
            Ok(schema) => (Some(schema), None),
            Err(error) => (None, Some(error)),
        };
        Self {
            path,
            config,
            config_error,
            schema,
            schema_error,
            dirty: false,
        }
    }

    #[cfg(test)]
    fn with_parts(path: PathBuf, config: toml::Table, schema: ConfigSchema) -> Self {
        Self {
            path,
            config: Some(config),
            config_error: None,
            schema: Some(schema),
            schema_error: None,
            dirty: false,
        }
    }

    fn config(&self) -> Result<&toml::Table, PanelError> {
        self.config
            .as_ref()
            .ok_or_else(|| PanelError::Config("HyprDeck config not loaded".into()))
    }

    fn config_mut(&mut self) -> Result<&mut toml::Table, PanelError> {
        self.config
            .as_mut()
            .ok_or_else(|| PanelError::Config("HyprDeck config not loaded".into()))
    }

    fn field_spec(&self, key: &str) -> Option<FieldSpec> {
        self.schema
            .as_ref()?
            .fields
            .iter()
            .find(|field| field.key == key)
            .map(|field| FieldSpec::from_schema(&field.field_type))
            .or_else(|| self.module_field_spec(key))
    }

    fn module_field_spec(&self, key: &str) -> Option<FieldSpec> {
        let (module_id, field_key) = parse_module_key(key)?;
        self.schema
            .as_ref()?
            .modules
            .iter()
            .find(|module| module.module_id == module_id)?
            .fields
            .iter()
            .find(|field| field.key == field_key)
            .map(|field| FieldSpec::from_schema(&field.field_type))
    }

    fn value_for(&self, key: &str) -> Option<&toml::Value> {
        let table = self.config.as_ref()?;
        if let Some((module_id, field_key)) = parse_module_key(key) {
            let module = table_get_path(table, "modules")?
                .as_table()?
                .get(module_id)?
                .as_table()?;
            return table_get_path(module, field_key);
        }
        table_get_path(table, key)
    }

    fn set_config_value(&mut self, key: &str, value: toml::Value) -> Result<(), PanelError> {
        let table = self.config_mut()?;
        if let Some((module_id, field_key)) = parse_module_key(key) {
            let modules = table
                .entry("modules")
                .or_insert_with(empty_table)
                .as_table_mut()
                .ok_or_else(|| PanelError::Config("`modules` must be a TOML table".into()))?;
            let module = modules
                .entry(module_id)
                .or_insert_with(empty_table)
                .as_table_mut()
                .ok_or_else(|| {
                    PanelError::Config(format!("`modules.{module_id}` must be a TOML table"))
                })?;
            return table_set_path(module, field_key, value);
        }
        table_set_path(table, key, value)
    }

    fn make_field(&self, field: &ConfigField) -> PanelField {
        let spec = FieldSpec::from_schema(&field.field_type);
        let key = &field.key;
        let original = self.value_for(key).map(value_string);
        PanelField {
            key: key.into(),
            section: None,
            label: field.label.clone(),
            description: field.description.clone(),
            field_type: spec.field_type(),
            current_value: original.clone().unwrap_or_else(|| spec.default_string()),
            original_value: original,
            dirty: self.dirty,
        }
    }

    fn common_fields(&self) -> Vec<PanelField> {
        self.schema
            .as_ref()
            .map(|schema| {
                schema
                    .fields
                    .iter()
                    .map(|field| self.make_field(field))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn module_fields(&self) -> Vec<PanelField> {
        let Some(schema) = &self.schema else {
            return Vec::new();
        };
        let active_modules = self
            .value_for("theme")
            .and_then(toml::Value::as_str)
            .and_then(|theme| schema.themes.iter().find(|metadata| metadata.id == theme))
            .map(|metadata| metadata.modules.as_slice())
            .filter(|modules| !modules.is_empty());
        schema
            .modules
            .iter()
            .filter(|module| {
                active_modules.is_none_or(|modules| modules.contains(&module.module_id))
            })
            .flat_map(|module| {
                module.fields.iter().map(move |field| {
                    let key = module_key(&module.module_id, &field.key);
                    let spec = FieldSpec::from_schema(&field.field_type);
                    let original = self.value_for(&key).map(value_string);
                    PanelField {
                        key,
                        section: None,
                        label: format!("{} — {}", module.module_id, field.label),
                        description: field.description.clone(),
                        field_type: spec.field_type(),
                        current_value: original.clone().unwrap_or_else(|| spec.default_string()),
                        original_value: original,
                        dirty: self.dirty,
                    }
                })
            })
            .collect()
    }

    fn save_atomic(&self) -> Result<(), PanelError> {
        let encoded = toml::to_string_pretty(self.config()?).map_err(|error| {
            PanelError::Config(format!("could not encode HyprDeck config: {error}"))
        })?;
        save_atomically(&self.path, &encoded, validate_with_hyprdeck)
    }
}

impl Default for HyprdeckPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsPanel for HyprdeckPanel {
    fn title(&self) -> &str {
        "HyprDeck"
    }
    fn icon(&self) -> &str {
        "application-menu"
    }
    fn available(&self) -> bool {
        self.config.is_some() || self.config_error.is_some()
    }

    fn fields(&self) -> Vec<PanelField> {
        if let Some(error) = &self.config_error {
            return status_field(
                "_config_invalid",
                &format!("Could not load HyprDeck configuration: {error}"),
            );
        }
        if self.config.is_none() {
            return status_field("_not_installed", "HyprDeck is not installed");
        }
        if let Some(error) = &self.schema_error {
            return status_field(
                "_schema_unavailable",
                &format!("Could not load HyprDeck configuration schema: {error}"),
            );
        }
        let mut fields = self.common_fields();
        fields.extend(self.module_fields());
        fields
    }

    fn set_value(&mut self, key: &str, value: &str) -> Result<String, PanelError> {
        let spec = self
            .field_spec(key)
            .ok_or_else(|| PanelError::UnknownKey(key.into()))?;
        let old = self
            .value_for(key)
            .map(value_string)
            .unwrap_or_else(|| spec.default_string());
        self.set_config_value(key, spec.parse(value, key)?)?;
        self.dirty = true;
        Ok(old)
    }

    fn has_unsaved_changes(&self) -> bool {
        self.dirty
    }
    fn save(&mut self) -> Result<(), PanelError> {
        if self.dirty {
            self.save_atomic()?;
            self.dirty = false;
        }
        Ok(())
    }
    fn reload(&mut self) -> Result<(), PanelError> {
        self.config = Some(load_config(&self.path)?);
        self.config_error = None;
        self.dirty = false;
        Ok(())
    }
    fn save_notice(&self) -> Option<&str> {
        Some(RESTART_NOTICE)
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

fn empty_table() -> toml::Value {
    toml::Value::Table(toml::Table::new())
}

fn table_get_path<'a>(table: &'a toml::Table, path: &str) -> Option<&'a toml::Value> {
    let mut parts = path.split('.');
    let first = parts.next()?;
    let mut value = table.get(first)?;
    for part in parts {
        value = value.as_table()?.get(part)?;
    }
    Some(value)
}

fn table_set_path(
    table: &mut toml::Table,
    path: &str,
    value: toml::Value,
) -> Result<(), PanelError> {
    let mut parts = path.split('.').peekable();
    let mut current = table;
    while let Some(part) = parts.next() {
        if part.is_empty() {
            return Err(PanelError::Config(
                "HyprDeck schema contains an empty path segment".into(),
            ));
        }
        if parts.peek().is_none() {
            current.insert(part.into(), value);
            return Ok(());
        }
        current = current
            .entry(part)
            .or_insert_with(empty_table)
            .as_table_mut()
            .ok_or_else(|| {
                PanelError::Config(format!(
                    "`{part}` must be a TOML table for nested configuration"
                ))
            })?;
    }
    Err(PanelError::Config(
        "HyprDeck schema contains an empty path".into(),
    ))
}

fn status_field(key: &str, message: &str) -> Vec<PanelField> {
    vec![PanelField {
        key: key.into(),
        section: None,
        label: message.into(),
        description: String::new(),
        field_type: FieldType::Text,
        current_value: String::new(),
        original_value: None,
        dirty: false,
    }]
}

fn load_config(path: &Path) -> Result<toml::Table, PanelError> {
    fs::read_to_string(path)?
        .parse::<toml::Table>()
        .map_err(|error| PanelError::Config(format!("could not parse {}: {error}", path.display())))
}

fn load_initial_config(path: &Path) -> (Option<toml::Table>, Option<String>) {
    match load_config(path) {
        Ok(config) => (Some(config), None),
        Err(PanelError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => (None, None),
        Err(error) => (None, Some(error.to_string())),
    }
}

fn hyprdeck_binary() -> String {
    std::env::var("HYPRDECK_BIN").unwrap_or_else(|_| "hyprdeck".into())
}

fn load_schema() -> Result<ConfigSchema, String> {
    let output = Command::new(hyprdeck_binary())
        .arg("--print-config-schema")
        .output()
        .map_err(|error| format!("could not run `hyprdeck --print-config-schema`: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    let schema: ConfigSchema = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("invalid HyprDeck schema JSON: {error}"))?;
    if schema.contract_version != CONFIG_CONTRACT_VERSION {
        return Err(format!(
            "HyprDeck config contract v{} is unsupported (HyprCube supports v{})",
            schema.contract_version, CONFIG_CONTRACT_VERSION
        ));
    }
    Ok(schema)
}

fn validate_with_hyprdeck(path: &Path) -> Result<(), PanelError> {
    let output = Command::new(hyprdeck_binary())
        .arg("--validate-config")
        .arg(path)
        .output()
        .map_err(|error| {
            PanelError::Config(format!("could not run HyprDeck validation: {error}"))
        })?;
    interpret_validation_result(output.status.success(), &output.stdout, &output.stderr)
}

fn interpret_validation_result(
    success: bool,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<(), PanelError> {
    let diagnostics: Result<Vec<ConfigDiagnostic>, _> = serde_json::from_slice(stdout);
    if let Ok(diagnostics) = diagnostics {
        return report_diagnostics(success, diagnostics, stderr);
    }
    let stderr = String::from_utf8_lossy(stderr).trim().to_owned();
    if success {
        return Err(PanelError::Config(
            "HyprDeck returned invalid validation JSON".into(),
        ));
    }
    if stderr.is_empty() {
        Err(PanelError::Config(
            "HyprDeck validation failed without diagnostics".into(),
        ))
    } else {
        Err(PanelError::Config(format!(
            "HyprDeck validation failed: {stderr}"
        )))
    }
}

fn report_diagnostics(
    success: bool,
    diagnostics: Vec<ConfigDiagnostic>,
    stderr: &[u8],
) -> Result<(), PanelError> {
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == "error")
        .map(|diagnostic| format!("{}: {}", diagnostic.path, diagnostic.message))
        .collect();
    if !errors.is_empty() {
        return Err(PanelError::Config(format!(
            "HyprDeck validation rejected the config: {}",
            errors.join("; ")
        )));
    }
    if success {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(stderr).trim().to_owned();
        let message = if stderr.is_empty() {
            "HyprDeck validation failed without error diagnostics".into()
        } else {
            format!("HyprDeck validation failed: {stderr}")
        };
        Err(PanelError::Config(message))
    }
}

fn save_atomically(
    path: &Path,
    encoded: &str,
    validate: impl FnOnce(&Path) -> Result<(), PanelError>,
) -> Result<(), PanelError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("hyprdeck.toml");
    let temp_path = parent.join(format!(".{name}.tmp-{}-{stamp}", std::process::id()));
    let result = (|| {
        let mut temp = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        if let Ok(metadata) = fs::metadata(path) {
            temp.set_permissions(metadata.permissions())?;
        }
        temp.write_all(encoded.as_bytes())?;
        temp.sync_all()?;
        validate(&temp_path)?;
        if path.exists() {
            fs::copy(path, path.with_file_name(format!("{name}.bak")))?;
        }
        fs::rename(&temp_path, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn module_key(module_id: &str, field_key: &str) -> String {
    format!("module:{module_id}:{field_key}")
}
fn parse_module_key(key: &str) -> Option<(&str, &str)> {
    key.strip_prefix("module:")?.split_once(':')
}
fn value_string(value: &toml::Value) -> String {
    match value {
        toml::Value::String(value) => value.clone(),
        value => value.to_string(),
    }
}

#[derive(Clone)]
enum FieldSpec {
    Text(String),
    Integer {
        default: i64,
        min: Option<i64>,
        max: Option<i64>,
    },
    Float {
        default: f64,
        min: Option<f64>,
        max: Option<f64>,
    },
    Boolean(bool),
    Choice {
        options: Vec<String>,
        default: String,
    },
    Color(String),
}

impl FieldSpec {
    fn from_schema(field: &ConfigFieldType) -> Self {
        match field {
            ConfigFieldType::Text { default } => Self::Text(default.clone()),
            ConfigFieldType::Integer { default, min, max } => Self::Integer {
                default: *default,
                min: *min,
                max: *max,
            },
            ConfigFieldType::Float { default, min, max } => Self::Float {
                default: *default,
                min: *min,
                max: *max,
            },
            ConfigFieldType::Boolean { default } => Self::Boolean(*default),
            ConfigFieldType::Choice { options, default }
            | ConfigFieldType::LabeledChoice {
                options, default, ..
            } => Self::Choice {
                options: options.clone(),
                default: default.clone(),
            },
            ConfigFieldType::Color { default } => Self::Color(default.clone()),
        }
    }
    fn field_type(&self) -> FieldType {
        match self {
            Self::Text(_) => FieldType::Text,
            Self::Integer { min, max, .. } => FieldType::Integer {
                min: *min,
                max: *max,
            },
            Self::Float { min, max, .. } => FieldType::Float {
                min: *min,
                max: *max,
            },
            Self::Boolean(_) => FieldType::Boolean,
            Self::Choice { options, .. } => FieldType::Choice {
                options: options.clone(),
            },
            Self::Color(_) => FieldType::Color,
        }
    }
    fn default_string(&self) -> String {
        match self {
            Self::Text(value) | Self::Choice { default: value, .. } | Self::Color(value) => {
                value.clone()
            }
            Self::Integer { default, .. } => default.to_string(),
            Self::Float { default, .. } => default.to_string(),
            Self::Boolean(default) => default.to_string(),
        }
    }
    fn parse(&self, value: &str, key: &str) -> Result<toml::Value, PanelError> {
        match self {
            Self::Text(_) | Self::Color(_) => Ok(toml::Value::String(value.into())),
            Self::Boolean(_) => value
                .parse::<bool>()
                .map(toml::Value::Boolean)
                .map_err(|_| PanelError::InvalidValue {
                    key: key.into(),
                    reason: "expected true or false".into(),
                }),
            Self::Integer { min, max, .. } => {
                let number = value.parse::<i64>().map_err(|_| PanelError::InvalidValue {
                    key: key.into(),
                    reason: "expected an integer".into(),
                })?;
                if min.is_some_and(|min| number < min) || max.is_some_and(|max| number > max) {
                    return Err(PanelError::InvalidValue {
                        key: key.into(),
                        reason: "outside the supported range".into(),
                    });
                }
                Ok(toml::Value::Integer(number))
            }
            Self::Float { min, max, .. } => {
                let number = value.parse::<f64>().map_err(|_| PanelError::InvalidValue {
                    key: key.into(),
                    reason: "expected a finite number".into(),
                })?;
                if !number.is_finite()
                    || min.is_some_and(|min| number < min)
                    || max.is_some_and(|max| number > max)
                {
                    return Err(PanelError::InvalidValue {
                        key: key.into(),
                        reason: "outside the supported range".into(),
                    });
                }
                Ok(toml::Value::Float(number))
            }
            Self::Choice { options, .. } if options.iter().any(|option| option == value) => {
                Ok(toml::Value::String(value.into()))
            }
            Self::Choice { .. } => Err(PanelError::InvalidValue {
                key: key.into(),
                reason: "not one of the supported choices".into(),
            }),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ConfigSchema {
    contract_version: u32,
    #[serde(default)]
    fields: Vec<ConfigField>,
    #[serde(default)]
    themes: Vec<ThemeMetadata>,
    modules: Vec<ModuleConfigSchema>,
}
#[derive(Debug, Clone, Deserialize)]
struct ThemeMetadata {
    id: String,
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    description: String,
    #[serde(default)]
    modules: Vec<String>,
}
#[derive(Debug, Clone, Deserialize)]
struct ModuleConfigSchema {
    module_id: String,
    fields: Vec<ConfigField>,
}
#[derive(Debug, Clone, Deserialize)]
struct ConfigField {
    key: String,
    label: String,
    description: String,
    field_type: ConfigFieldType,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ConfigFieldType {
    Text {
        default: String,
    },
    Integer {
        default: i64,
        min: Option<i64>,
        max: Option<i64>,
    },
    Float {
        default: f64,
        min: Option<f64>,
        max: Option<f64>,
    },
    Boolean {
        default: bool,
    },
    Choice {
        options: Vec<String>,
        default: String,
    },
    LabeledChoice {
        options: Vec<String>,
        #[serde(rename = "labels")]
        _labels: Vec<String>,
        default: String,
    },
    Color {
        default: String,
    },
}
#[derive(Debug, Deserialize)]
struct ConfigDiagnostic {
    severity: String,
    path: String,
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn schema() -> ConfigSchema {
        ConfigSchema {
            contract_version: CONFIG_CONTRACT_VERSION,
            fields: vec![
                ConfigField {
                    key: "theme".into(),
                    label: "Theme".into(),
                    description: "Theme choice".into(),
                    field_type: ConfigFieldType::Choice {
                        options: vec!["win7".into(), "gnome_classic".into()],
                        default: "win7".into(),
                    },
                },
                ConfigField {
                    key: "theme_overrides.bar_opacity".into(),
                    label: "Bar opacity".into(),
                    description: "Theme override".into(),
                    field_type: ConfigFieldType::Float {
                        default: 1.0,
                        min: Some(0.0),
                        max: Some(1.0),
                    },
                },
            ],
            themes: vec![
                ThemeMetadata {
                    id: "win7".into(),
                    name: "Windows 7".into(),
                    description: String::new(),
                    modules: vec!["clock".into()],
                },
                ThemeMetadata {
                    id: "gnome_classic".into(),
                    name: "Classic GNOME".into(),
                    description: String::new(),
                    modules: vec!["clock".into()],
                },
            ],
            modules: vec![ModuleConfigSchema {
                module_id: "clock".into(),
                fields: vec![
                    ConfigField {
                        key: "format".into(),
                        label: "Format".into(),
                        description: "Clock format".into(),
                        field_type: ConfigFieldType::Text {
                            default: "%H:%M".into(),
                        },
                    },
                    ConfigField {
                        key: "show_seconds".into(),
                        label: "Seconds".into(),
                        description: "Show seconds".into(),
                        field_type: ConfigFieldType::Boolean { default: false },
                    },
                    ConfigField {
                        key: "commands.refresh".into(),
                        label: "Refresh command".into(),
                        description: "Nested module field".into(),
                        field_type: ConfigFieldType::Text {
                            default: "true".into(),
                        },
                    },
                ],
            }],
        }
    }
    fn panel() -> HyprdeckPanel {
        let mut config = toml::Table::new();
        config.insert("theme".into(), toml::Value::String("win7".into()));
        HyprdeckPanel::with_parts(
            std::env::temp_dir().join("hyprdeck-test.toml"),
            config,
            schema(),
        )
    }
    #[test]
    fn exposes_contract_module_fields_with_defaults() {
        let fields = panel().fields();
        let theme = fields.iter().find(|field| field.key == "theme").unwrap();
        assert!(matches!(&theme.field_type, FieldType::Choice { options }
            if options.iter().map(String::as_str).eq(["win7", "gnome_classic"])));
        let format = fields
            .iter()
            .find(|field| field.key == "module:clock:format")
            .unwrap();
        assert_eq!(format.current_value, "%H:%M");
        assert_eq!(format.original_value, None);
        assert!(fields
            .iter()
            .any(|field| field.key == "theme_overrides.bar_opacity"));
    }
    #[test]
    fn writes_values_with_toml_types() {
        let mut panel = panel();
        panel.set_value("theme", "gnome_classic").unwrap();
        panel
            .set_value("module:clock:show_seconds", "true")
            .unwrap();
        panel
            .set_value("theme_overrides.bar_opacity", "0.75")
            .unwrap();
        panel
            .set_value("module:clock:commands.refresh", "hyprctl reload")
            .unwrap();
        let config = panel.config().unwrap();
        assert_eq!(config["theme"].as_str(), Some("gnome_classic"));
        assert_eq!(
            config["modules"]["clock"]["show_seconds"].as_bool(),
            Some(true)
        );
        assert_eq!(
            config["theme_overrides"]["bar_opacity"].as_float(),
            Some(0.75)
        );
        assert_eq!(
            config["modules"]["clock"]["commands"]["refresh"].as_str(),
            Some("hyprctl reload")
        );
    }
    #[test]
    fn rejects_invalid_schema_typed_values() {
        let mut panel = panel();
        assert!(panel.set_value("module:clock:show_seconds", "yes").is_err());
        assert!(panel
            .set_value("theme_overrides.bar_opacity", "1.5")
            .is_err());
    }

    #[test]
    fn validation_reports_structured_stdout_diagnostics_on_failure() {
        let error = interpret_validation_result(
            false,
            br#"[{"severity":"error","path":"theme","message":"unknown theme"}]"#,
            b"",
        )
        .unwrap_err();
        assert!(error.to_string().contains("theme: unknown theme"));
    }

    #[test]
    fn malformed_config_remains_visible_as_an_error() {
        let directory = std::env::temp_dir().join(format!(
            "hyprcube-hyprdeck-invalid-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("hyprdeck.toml");
        fs::write(&path, "theme = [not valid").unwrap();
        let (config, config_error) = load_initial_config(&path);
        let panel = HyprdeckPanel {
            path,
            config,
            config_error,
            schema: Some(schema()),
            schema_error: None,
            dirty: false,
        };
        assert!(panel.available());
        assert_eq!(panel.fields()[0].key, "_config_invalid");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn atomic_save_validates_before_replacing() {
        let directory = std::env::temp_dir().join(format!(
            "hyprcube-hyprdeck-panel-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("hyprdeck.toml");
        fs::write(&path, "theme = \"old\"\n").unwrap();
        save_atomically(&path, "theme = \"new\"\n", |_| Ok(())).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "theme = \"new\"\n");
        assert_eq!(
            fs::read_to_string(path.with_file_name("hyprdeck.toml.bak")).unwrap(),
            "theme = \"old\"\n"
        );
        assert!(
            save_atomically(&path, "theme = \"invalid\"\n", |_| Err(PanelError::Config(
                "no".into()
            )))
            .is_err()
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "theme = \"new\"\n");
        fs::remove_dir_all(directory).unwrap();
    }
}
