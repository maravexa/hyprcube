use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use hyprcube_core::layout::Rect;
use hyprcube_core::widget::{Constraints, Event, EventResponse, Size};

use crate::{FieldType, PanelError, PanelField, SettingsPanel};

const SAVE_NOTICE: &str = "HyprSaver settings saved. Open the live preview to inspect them.";
const HYPRPAPER_THEME_SOURCE: &str = "source = ~/.config/hypr/hyprpaper-hyprcube-theme.conf";

/// Full HyprSaver configuration editor backed by its native TOML structure.
pub struct HyprsaverPanel {
    path: PathBuf,
    config: Option<toml::Table>,
    installed: bool,
    dirty: bool,
    shaders: Vec<(String, String)>,
    palettes: Vec<(String, String)>,
    shader_playlists: Vec<(String, String)>,
    palette_playlists: Vec<(String, String)>,
}

impl HyprsaverPanel {
    pub fn new() -> Self {
        let path = hyprcube_core::hypr_config_dir().join("hyprsaver.toml");
        let installed = executable_exists(&hyprsaver_binary());
        let config = match load_config(&path) {
            Ok(config) => Some(config),
            Err(PanelError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                installed.then(toml::Table::new)
            }
            Err(error) => {
                tracing::warn!(%error, "could not load HyprSaver configuration");
                None
            }
        };
        Self {
            path,
            config,
            installed,
            dirty: false,
            shaders: query_named_items("--list-shaders", &["cycle", "random"]),
            palettes: query_named_items("--list-palettes", &["cycle", "random"]),
            shader_playlists: query_playlists("--list-shader-playlists"),
            palette_playlists: query_playlists("--list-palette-playlists"),
        }
    }

    #[cfg(test)]
    fn with_config(path: PathBuf, config: toml::Table) -> Self {
        Self {
            path,
            config: Some(config),
            installed: true,
            dirty: false,
            shaders: choices(&["cycle", "random", "plasma"]),
            palettes: choices(&["cycle", "random", "aurora"]),
            shader_playlists: choices(&["default", "calm"]),
            palette_playlists: choices(&["default", "calm"]),
        }
    }

    fn table(&self) -> Result<&toml::Table, PanelError> {
        self.config
            .as_ref()
            .ok_or_else(|| PanelError::Config("HyprSaver configuration is unavailable".into()))
    }

    fn table_mut(&mut self) -> Result<&mut toml::Table, PanelError> {
        self.config
            .as_mut()
            .ok_or_else(|| PanelError::Config("HyprSaver configuration is unavailable".into()))
    }

    fn value(&self, key: &str) -> Option<&toml::Value> {
        table_get_path(self.config.as_ref()?, key)
    }

    fn field(
        &self,
        key: &str,
        label: &str,
        description: &str,
        field_type: FieldType,
        default: &str,
    ) -> PanelField {
        let original = self.value(key).map(display_value);
        PanelField {
            key: key.into(),
            section: None,
            label: label.into(),
            description: description.into(),
            field_type,
            current_value: original.clone().unwrap_or_else(|| default.into()),
            original_value: original,
            dirty: self.dirty,
        }
    }

    fn apply_override(&mut self) -> Result<(), PanelError> {
        let enabled = self
            .value("hyprcube_override.enabled")
            .and_then(toml::Value::as_bool)
            .unwrap_or(false);
        if !enabled {
            return Ok(());
        }
        for (override_key, general_key) in [
            ("hyprcube_override.palette", "general.palette"),
            (
                "hyprcube_override.shader_playlist",
                "general.shader_playlist",
            ),
            (
                "hyprcube_override.palette_playlist",
                "general.palette_playlist",
            ),
        ] {
            let Some(value) = self
                .value(override_key)
                .and_then(toml::Value::as_str)
                .filter(|value| !value.is_empty() && *value != "inherit")
                .map(ToOwned::to_owned)
            else {
                continue;
            };
            table_set_path(self.table_mut()?, general_key, toml::Value::String(value))?;
        }
        Ok(())
    }

    fn launch_preview(&self) -> Result<(), PanelError> {
        Command::new(hyprsaver_binary())
            .args(["--preview", "--config"])
            .arg(&self.path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|error| {
                PanelError::Config(format!("could not open `hyprsaver --preview`: {error}"))
            })
    }

    fn apply_wallpaper_override(&self) -> Result<(), PanelError> {
        let enabled = self
            .value("hyprcube_override.enabled")
            .and_then(toml::Value::as_bool)
            .unwrap_or(false);
        let wallpaper = self
            .value("hyprcube_override.wallpaper")
            .and_then(toml::Value::as_str)
            .filter(|wallpaper| !wallpaper.trim().is_empty());
        let config_dir = hyprcube_core::hypr_config_dir();
        let theme_path = config_dir.join("hyprpaper-hyprcube-theme.conf");
        let root_path = config_dir.join("hyprpaper.conf");
        let root = fs::read_to_string(&root_path).unwrap_or_default();
        let mut lines: Vec<_> = root
            .lines()
            .filter(|line| {
                let line = line.trim();
                line != HYPRPAPER_THEME_SOURCE
                    && line != "# Active HyprCube theme wallpaper override"
            })
            .map(ToOwned::to_owned)
            .collect();
        let (true, Some(wallpaper)) = (enabled, wallpaper) else {
            if root.lines().count() != lines.len() {
                atomic_write(&root_path, format!("{}\n", lines.join("\n")).as_bytes())?;
            }
            atomic_write(
                &theme_path,
                b"# HyprCube theme wallpaper override disabled\n",
            )?;
            return Ok(());
        };

        let monitors = connected_monitor_names();
        let mut theme = "# HyprCube theme wallpaper override\n".to_owned();
        if monitors.is_empty() {
            theme.push_str(&format!(
                "\nwallpaper {{\n    monitor =\n    path = {wallpaper}\n    fit_mode = cover\n}}\n"
            ));
        } else {
            for monitor in &monitors {
                theme.push_str(&format!(
                    "\nwallpaper {{\n    monitor = {monitor}\n    path = {wallpaper}\n    fit_mode = cover\n}}\n"
                ));
            }
        }
        atomic_write(&theme_path, theme.as_bytes())?;

        lines.push("# Active HyprCube theme wallpaper override".into());
        lines.push(HYPRPAPER_THEME_SOURCE.into());
        atomic_write(&root_path, format!("{}\n", lines.join("\n")).as_bytes())?;

        for monitor in monitors {
            let assignment = format!("{monitor}, {wallpaper}, cover");
            if let Err(error) = Command::new("hyprctl")
                .args(["hyprpaper", "wallpaper", &assignment])
                .output()
            {
                tracing::debug!(%error, "Hyprpaper is not available for live theme update");
            }
        }
        Ok(())
    }
}

impl Default for HyprsaverPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsPanel for HyprsaverPanel {
    fn title(&self) -> &str {
        "HyprSaver"
    }

    fn icon(&self) -> &str {
        "preferences-desktop-screensaver"
    }

    fn available(&self) -> bool {
        self.installed || self.path.is_file()
    }

    fn fields(&self) -> Vec<PanelField> {
        if self.config.is_none() {
            return vec![self.field(
                "_unavailable",
                "HyprSaver configuration could not be loaded",
                "Check hyprsaver.toml for invalid TOML.",
                FieldType::Text,
                "",
            )];
        }

        let mut fields = vec![
            self.field(
                "_preview",
                "Live preview",
                "Opens HyprSaver's native accelerated preview window with this configuration.",
                FieldType::Action {
                    label: "Open Preview".into(),
                },
                "",
            ),
            self.field(
                "general.fps",
                "Frame rate",
                "Target render frames per second.",
                FieldType::Integer {
                    min: Some(1),
                    max: Some(240),
                },
                "30",
            ),
            self.field(
                "general.shader",
                "Shader",
                "Built-in or user shader; cycle and random are also supported.",
                FieldType::SearchableChoice {
                    options: self.shaders.clone(),
                },
                "cycle",
            ),
            self.field(
                "general.palette",
                "Palette",
                "Built-in, user, LUT, or gradient palette.",
                FieldType::SearchableChoice {
                    options: self.palettes.clone(),
                },
                "cycle",
            ),
            self.field(
                "general.shader_cycle_interval",
                "Shader cycle interval",
                "Seconds between shaders in cycle mode.",
                FieldType::Integer {
                    min: Some(1),
                    max: Some(86_400),
                },
                "300",
            ),
            self.field(
                "general.palette_cycle_interval",
                "Palette cycle interval",
                "Seconds between palettes in cycle mode.",
                FieldType::Integer {
                    min: Some(1),
                    max: Some(86_400),
                },
                "60",
            ),
            self.field(
                "general.palette_transition_duration",
                "Palette transition",
                "Cross-fade duration in seconds.",
                FieldType::Float {
                    min: Some(0.0),
                    max: Some(30.0),
                },
                "0",
            ),
            self.field(
                "general.shader_playlist",
                "Shader playlist",
                "Named shader collection used in cycle mode.",
                FieldType::SearchableChoice {
                    options: self.shader_playlists.clone(),
                },
                "default",
            ),
            self.field(
                "general.palette_playlist",
                "Palette playlist",
                "Named palette collection used in cycle mode.",
                FieldType::SearchableChoice {
                    options: self.palette_playlists.clone(),
                },
                "default",
            ),
            self.field(
                "general.cycle_order",
                "Cycle order",
                "Choose randomized or sequential playlist traversal.",
                FieldType::Choice {
                    options: vec!["random".into(), "sequential".into()],
                },
                "random",
            ),
            self.field(
                "general.synced",
                "Synchronize displays",
                "Cycle every monitor together.",
                FieldType::Boolean,
                "true",
            ),
            self.field(
                "general.preview_width",
                "Preview width",
                "Remembered native preview window width.",
                FieldType::Integer {
                    min: Some(320),
                    max: Some(7680),
                },
                "1280",
            ),
            self.field(
                "general.preview_height",
                "Preview height",
                "Remembered native preview window height.",
                FieldType::Integer {
                    min: Some(240),
                    max: Some(4320),
                },
                "720",
            ),
            self.field(
                "behavior.fade_in_ms",
                "Fade in",
                "Overlay fade-in duration in milliseconds.",
                FieldType::Integer {
                    min: Some(0),
                    max: Some(60_000),
                },
                "800",
            ),
            self.field(
                "behavior.fade_out_ms",
                "Fade out",
                "Overlay fade-out duration in milliseconds.",
                FieldType::Integer {
                    min: Some(0),
                    max: Some(60_000),
                },
                "400",
            ),
            self.field(
                "behavior.dismiss_on",
                "Dismiss events",
                "Comma-separated: key, mouse_move, mouse_click, touch.",
                FieldType::Text,
                "key, mouse_move, mouse_click, touch",
            ),
            self.field(
                "behavior.exclusive_keyboard",
                "Exclusive keyboard",
                "Allow key presses to dismiss the screensaver.",
                FieldType::Boolean,
                "true",
            ),
            self.field(
                "hyprcube_override.enabled",
                "Use HyprCube theme override",
                "Apply this snapshot-friendly collection to HyprSaver whenever it is saved.",
                FieldType::Boolean,
                "false",
            ),
            self.field(
                "hyprcube_override.palette",
                "Override palette",
                "Palette associated with the current HyprCube theme collection.",
                FieldType::SearchableChoice {
                    options: with_inherit(&self.palettes),
                },
                "inherit",
            ),
            self.field(
                "hyprcube_override.shader_playlist",
                "Override shader playlist",
                "Shader playlist associated with the current HyprCube theme collection.",
                FieldType::SearchableChoice {
                    options: with_inherit(&self.shader_playlists),
                },
                "inherit",
            ),
            self.field(
                "hyprcube_override.palette_playlist",
                "Override palette playlist",
                "Palette playlist associated with the current HyprCube theme collection.",
                FieldType::SearchableChoice {
                    options: with_inherit(&self.palette_playlists),
                },
                "inherit",
            ),
            self.field(
                "hyprcube_override.wallpaper",
                "Override wallpaper",
                "Wallpaper path retained with this collection for the Hyprpaper integration.",
                FieldType::Text,
                "",
            ),
        ];

        // Preserve and expose every named unified playlist as editable lists.
        if let Some(playlists) = self.value("playlists").and_then(toml::Value::as_table) {
            let mut names: Vec<_> = playlists.keys().cloned().collect();
            names.sort();
            for name in names {
                fields.push(self.field(
                    &format!("playlists.{name}.shaders"),
                    &format!("{name} shaders"),
                    "Comma-separated ordered shader names.",
                    FieldType::Text,
                    "all",
                ));
                fields.push(self.field(
                    &format!("playlists.{name}.palettes"),
                    &format!("{name} palettes"),
                    "Comma-separated ordered palette names.",
                    FieldType::Text,
                    "all",
                ));
            }
        }
        fields
    }

    fn set_value(&mut self, key: &str, value: &str) -> Result<String, PanelError> {
        if key == "_preview" {
            self.launch_preview()?;
            return Ok(String::new());
        }
        if key.starts_with('_') {
            return Err(PanelError::UnknownKey(key.into()));
        }
        let old = self.value(key).map(display_value).unwrap_or_default();
        let parsed = parse_field_value(key, value)?;
        table_set_path(self.table_mut()?, key, parsed)?;
        self.dirty = true;
        Ok(old)
    }

    fn has_unsaved_changes(&self) -> bool {
        self.dirty
    }

    fn save(&mut self) -> Result<(), PanelError> {
        if self.dirty {
            self.apply_override()?;
            let encoded = toml::to_string_pretty(self.table()?)
                .map_err(|error| PanelError::Config(error.to_string()))?;
            atomic_write(&self.path, encoded.as_bytes())?;
            self.apply_wallpaper_override()?;
            self.dirty = false;
        }
        Ok(())
    }

    fn reload(&mut self) -> Result<(), PanelError> {
        self.config = Some(load_config(&self.path)?);
        self.dirty = false;
        Ok(())
    }

    fn save_notice(&self) -> Option<&str> {
        Some(SAVE_NOTICE)
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

fn hyprsaver_binary() -> String {
    std::env::var("HYPRSAVER_BIN").unwrap_or_else(|_| "hyprsaver".into())
}

fn executable_exists(executable: &str) -> bool {
    let path = Path::new(executable);
    if path.components().count() > 1 {
        return path.is_file();
    }
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|directory| directory.join(executable).is_file())
    })
}

fn connected_monitor_names() -> Vec<String> {
    Command::new("hyprctl")
        .args(["-j", "monitors"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| serde_json::from_slice::<Vec<serde_json::Value>>(&output.stdout).ok())
        .map(|monitors| {
            monitors
                .into_iter()
                .filter_map(|monitor| monitor["name"].as_str().map(ToOwned::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn query_named_items(argument: &str, special: &[&str]) -> Vec<(String, String)> {
    let mut items = choices(special);
    let output = Command::new(hyprsaver_binary()).arg(argument).output();
    if let Ok(output) = output {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if !line.starts_with("  ") || line.trim_start().starts_with('(') {
                continue;
            }
            let line = line.trim();
            let Some(name) = line.split_whitespace().next() else {
                continue;
            };
            if !items.iter().any(|(value, _)| value == name) {
                items.push((name.into(), line.into()));
            }
        }
    }
    items
}

fn query_playlists(argument: &str) -> Vec<(String, String)> {
    let mut items = choices(&["default"]);
    if let Ok(output) = Command::new(hyprsaver_binary()).arg(argument).output() {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let line = line.trim();
            let Some((name, values)) = line.split_once(':') else {
                continue;
            };
            if name.contains(' ') || name.is_empty() {
                continue;
            }
            if !items.iter().any(|(value, _)| value == name) {
                items.push((name.into(), format!("{name}: {values}")));
            }
        }
    }
    items
}

fn choices(values: &[&str]) -> Vec<(String, String)> {
    values
        .iter()
        .map(|value| ((*value).into(), (*value).into()))
        .collect()
}

fn with_inherit(options: &[(String, String)]) -> Vec<(String, String)> {
    std::iter::once(("inherit".into(), "Inherit HyprSaver setting".into()))
        .chain(options.iter().cloned())
        .collect()
}

fn load_config(path: &Path) -> Result<toml::Table, PanelError> {
    fs::read_to_string(path)?
        .parse()
        .map_err(|error: toml::de::Error| PanelError::Config(error.to_string()))
}

fn table_get_path<'a>(table: &'a toml::Table, path: &str) -> Option<&'a toml::Value> {
    let mut parts = path.split('.');
    let mut value = table.get(parts.next()?)?;
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
        if parts.peek().is_none() {
            current.insert(part.into(), value);
            return Ok(());
        }
        current = current
            .entry(part)
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .ok_or_else(|| PanelError::Config(format!("`{part}` must be a table")))?;
    }
    Err(PanelError::UnknownKey(path.into()))
}

fn display_value(value: &toml::Value) -> String {
    match value {
        toml::Value::String(value) => value.clone(),
        toml::Value::Array(values) => values
            .iter()
            .map(display_value)
            .collect::<Vec<_>>()
            .join(", "),
        value => value.to_string(),
    }
}

fn parse_field_value(key: &str, value: &str) -> Result<toml::Value, PanelError> {
    let invalid = |reason: &str| PanelError::InvalidValue {
        key: key.into(),
        reason: reason.into(),
    };
    if matches!(
        key,
        "general.fps"
            | "general.shader_cycle_interval"
            | "general.palette_cycle_interval"
            | "general.preview_width"
            | "general.preview_height"
            | "behavior.fade_in_ms"
            | "behavior.fade_out_ms"
    ) {
        return value
            .parse::<i64>()
            .map(toml::Value::Integer)
            .map_err(|_| invalid("expected a whole number"));
    }
    if key == "general.palette_transition_duration" {
        return value
            .parse::<f64>()
            .map(toml::Value::Float)
            .map_err(|_| invalid("expected a number"));
    }
    if matches!(
        key,
        "general.synced" | "behavior.exclusive_keyboard" | "hyprcube_override.enabled"
    ) {
        return value
            .parse::<bool>()
            .map(toml::Value::Boolean)
            .map_err(|_| invalid("expected true or false"));
    }
    if key == "behavior.dismiss_on"
        || (key.starts_with("playlists.")
            && (key.ends_with(".shaders") || key.ends_with(".palettes")))
    {
        return Ok(toml::Value::Array(
            value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| toml::Value::String(value.into()))
                .collect(),
        ));
    }
    Ok(toml::Value::String(value.into()))
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), PanelError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp = path.with_file_name(format!(
        ".{}.hyprcube-{}-{stamp}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("hyprsaver"),
        std::process::id()
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(contents)?;
        file.sync_all()?;
        fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result.map_err(PanelError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_native_general_and_behavior_options() {
        let mut table = toml::Table::new();
        table_set_path(&mut table, "general.fps", toml::Value::Integer(30)).unwrap();
        let panel = HyprsaverPanel::with_config(PathBuf::from("/tmp/test.toml"), table);
        let fields = panel.fields();
        assert!(fields.iter().any(|field| field.key == "general.shader"));
        assert!(fields
            .iter()
            .any(|field| field.key == "behavior.fade_in_ms"));
        assert!(fields.iter().any(|field| field.key == "_preview"));
        assert!(fields
            .iter()
            .any(|field| field.key == "hyprcube_override.palette"));
    }

    #[test]
    fn set_value_preserves_native_toml_types() {
        let mut panel =
            HyprsaverPanel::with_config(PathBuf::from("/tmp/test.toml"), toml::Table::new());
        panel.set_value("general.fps", "60").unwrap();
        panel.set_value("general.synced", "false").unwrap();
        panel
            .set_value("behavior.dismiss_on", "key, mouse_click")
            .unwrap();
        assert_eq!(
            panel.value("general.fps").and_then(toml::Value::as_integer),
            Some(60)
        );
        assert_eq!(
            panel.value("general.synced").and_then(toml::Value::as_bool),
            Some(false)
        );
        assert_eq!(
            panel
                .value("behavior.dismiss_on")
                .and_then(toml::Value::as_array)
                .map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn enabled_override_updates_general_selection() {
        let mut panel =
            HyprsaverPanel::with_config(PathBuf::from("/tmp/test.toml"), toml::Table::new());
        panel
            .set_value("hyprcube_override.enabled", "true")
            .unwrap();
        panel
            .set_value("hyprcube_override.palette", "aurora")
            .unwrap();
        panel.apply_override().unwrap();
        assert_eq!(
            panel.value("general.palette").and_then(toml::Value::as_str),
            Some("aurora")
        );
    }
}
