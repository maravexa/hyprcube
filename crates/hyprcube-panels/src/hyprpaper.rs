//! Dynamic Hyprpaper editor with per-output visual previews.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use hyprcube_core::color::Color;
use hyprcube_core::layout::Rect;
use hyprcube_core::render::fill_rounded_rect;
use hyprcube_core::widget::{
    Button, Constraints, Dropdown, Event, EventResponse, Size, TextInput, Toggle, Widget,
};
use tiny_skia::{PixmapPaint, Transform};

use crate::{form, PanelError, PanelField, SettingsPanel};

const VIZ_H: f32 = 270.0;
const FORM_TOP: f32 = VIZ_H + 52.0;
const ROW_H: f32 = 44.0;
const H_PAD: f32 = 16.0;
const CONTROL_W: f32 = 360.0;
const SOURCE_LINE: &str = "source = ~/.config/hypr/hyprpaper-hyprcube.conf";

#[derive(Debug)]
struct WallpaperMonitor {
    name: String,
    description: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    path: String,
    fit_mode: String,
    preview: Option<image::RgbaImage>,
}

pub struct HyprpaperPanel {
    config_path: PathBuf,
    managed_path: PathBuf,
    installed: bool,
    monitors: Vec<WallpaperMonitor>,
    selected: Option<usize>,
    path_input: TextInput,
    fit_dropdown: Dropdown,
    apply_button: Button,
    splash_toggle: Toggle,
    ipc_toggle: Toggle,
    dirty: bool,
    status: String,
}

impl HyprpaperPanel {
    pub fn new() -> Self {
        let config_dir = hyprcube_core::hypr_config_dir();
        let config_path = config_dir.join("hyprpaper.conf");
        let managed_path = config_dir.join("hyprpaper-hyprcube.conf");
        let installed = executable_exists("hyprpaper");
        let source = [
            fs::read_to_string(&config_path).unwrap_or_default(),
            fs::read_to_string(&managed_path).unwrap_or_default(),
        ]
        .join("\n");
        let assignments = parse_wallpaper_assignments(&source);
        let active = active_wallpapers();
        let mut monitors = connected_monitors();
        for monitor in &mut monitors {
            let configured = assignments
                .iter()
                .rev()
                .find(|assignment| assignment.0 == monitor.name);
            let active_path = active
                .iter()
                .find(|(name, _)| name == &monitor.name)
                .map(|(_, path)| path.as_str());
            if let Some((_, path, mode)) = configured {
                monitor.path = path.clone();
                monitor.fit_mode = mode.clone();
            } else if let Some(path) = active_path {
                monitor.path = path.to_owned();
            }
            monitor.preview = load_preview(&monitor.path);
        }
        let selected = (!monitors.is_empty()).then_some(0);
        let mut panel = Self {
            config_path,
            managed_path,
            installed,
            monitors,
            selected,
            path_input: TextInput::new("wallpaper.path", ""),
            fit_dropdown: Dropdown::new(
                "wallpaper.fit_mode",
                vec![
                    "cover".into(),
                    "contain".into(),
                    "fill".into(),
                    "tile".into(),
                ],
                0,
            ),
            apply_button: Button::new("wallpaper.apply", "Apply Live"),
            splash_toggle: Toggle::new("splash", parse_bool_setting(&source, "splash", false)),
            ipc_toggle: Toggle::new("ipc", parse_bool_setting(&source, "ipc", true)),
            dirty: false,
            status: String::new(),
        };
        panel.sync_controls();
        panel
    }

    fn sync_controls(&mut self) {
        let Some(monitor) = self.selected.and_then(|index| self.monitors.get(index)) else {
            return;
        };
        self.path_input.value = monitor.path.clone();
        self.path_input.cursor_pos = self.path_input.value.chars().count();
        self.fit_dropdown.selected = self
            .fit_dropdown
            .options
            .iter()
            .position(|mode| mode == &monitor.fit_mode)
            .unwrap_or(0);
    }

    fn control_rect(&self, bounds: Rect, row: usize) -> Rect {
        Rect {
            x: bounds.x + bounds.width - CONTROL_W.min(bounds.width * 0.58) - H_PAD,
            y: bounds.y + FORM_TOP + row as f32 * ROW_H,
            width: CONTROL_W.min(bounds.width * 0.58),
            height: 32.0,
        }
    }

    fn viz_transform(&self, bounds: Rect) -> (f32, f32, f32) {
        if self.monitors.is_empty() {
            return (1.0, bounds.x + 20.0, bounds.y + 20.0);
        }
        let min_x = self.monitors.iter().map(|monitor| monitor.x).min().unwrap() as f32;
        let min_y = self.monitors.iter().map(|monitor| monitor.y).min().unwrap() as f32;
        let max_x = self
            .monitors
            .iter()
            .map(|monitor| monitor.x + monitor.width as i32)
            .max()
            .unwrap() as f32;
        let max_y = self
            .monitors
            .iter()
            .map(|monitor| monitor.y + monitor.height as i32)
            .max()
            .unwrap() as f32;
        let scale = ((bounds.width - 40.0) / (max_x - min_x).max(1.0))
            .min((VIZ_H - 40.0) / (max_y - min_y).max(1.0));
        (
            scale,
            bounds.x + 20.0 - min_x * scale,
            bounds.y + 20.0 - min_y * scale,
        )
    }

    fn monitor_rect(monitor: &WallpaperMonitor, scale: f32, ox: f32, oy: f32) -> Rect {
        Rect {
            x: ox + monitor.x as f32 * scale,
            y: oy + monitor.y as f32 * scale,
            width: monitor.width as f32 * scale,
            height: monitor.height as f32 * scale,
        }
    }

    fn apply_live(&mut self) {
        let Some(monitor) = self.selected.and_then(|index| self.monitors.get(index)) else {
            return;
        };
        if monitor.path.is_empty() {
            self.status = "Choose an image before applying it.".into();
            return;
        }
        let assignment = format!("{}, {}, {}", monitor.name, monitor.path, monitor.fit_mode);
        match Command::new("hyprctl")
            .args(["hyprpaper", "wallpaper", &assignment])
            .output()
        {
            Ok(output) if output.status.success() => {
                self.status = format!("Applied wallpaper to {}", monitor.name)
            }
            Ok(output) => {
                self.status = String::from_utf8_lossy(&output.stderr).trim().to_owned();
                if self.status.is_empty() {
                    self.status = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                }
            }
            Err(error) => self.status = format!("Could not run hyprctl: {error}"),
        }
    }

    fn save_managed_config(&self) -> Result<(), PanelError> {
        let mut encoded = format!(
            "# Managed by HyprCube. Edit through HyprCube or remove the source line.\n\nipc = {}\nsplash = {}\n",
            self.ipc_toggle.value, self.splash_toggle.value
        );
        for monitor in &self.monitors {
            if monitor.path.is_empty() {
                continue;
            }
            encoded.push_str(&format!(
                "\nwallpaper {{\n    monitor = {}\n    path = {}\n    fit_mode = {}\n}}\n",
                monitor.name, monitor.path, monitor.fit_mode
            ));
        }
        atomic_write(&self.managed_path, encoded.as_bytes())?;

        let mut root = fs::read_to_string(&self.config_path).unwrap_or_default();
        if !root.lines().any(|line| line.trim() == SOURCE_LINE) {
            if !root.ends_with('\n') && !root.is_empty() {
                root.push('\n');
            }
            root.push_str("\n# HyprCube-managed dynamic wallpaper assignments\n");
            root.push_str(SOURCE_LINE);
            root.push('\n');
            atomic_write(&self.config_path, root.as_bytes())?;
        }
        Ok(())
    }
}

impl Default for HyprpaperPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsPanel for HyprpaperPanel {
    fn title(&self) -> &str {
        "Hyprpaper"
    }

    fn icon(&self) -> &str {
        "preferences-desktop-wallpaper"
    }

    fn available(&self) -> bool {
        self.installed || self.config_path.is_file()
    }

    fn uses_custom_layout(&self) -> bool {
        true
    }

    fn fields(&self) -> Vec<PanelField> {
        Vec::new()
    }

    fn set_value(&mut self, key: &str, _value: &str) -> Result<String, PanelError> {
        Err(PanelError::UnknownKey(key.into()))
    }

    fn has_unsaved_changes(&self) -> bool {
        self.dirty
    }

    fn save(&mut self) -> Result<(), PanelError> {
        if self.dirty {
            self.save_managed_config()?;
            self.dirty = false;
        }
        Ok(())
    }

    fn reload(&mut self) -> Result<(), PanelError> {
        *self = Self::new();
        Ok(())
    }

    fn save_notice(&self) -> Option<&str> {
        Some("Hyprpaper configuration saved; live assignments are available immediately.")
    }

    fn build_form(&self, content_width: f32) -> form::FormLayout {
        form::FormLayout::from_fields(Vec::new(), content_width)
    }

    fn measure(&self, constraints: Constraints) -> Size {
        Size {
            width: constraints.max_width,
            height: constraints.max_height,
        }
    }

    fn paint(
        &self,
        bounds: Rect,
        pixmap: &mut tiny_skia::PixmapMut<'_>,
        ctx: &mut hyprcube_core::text::RenderContext<'_>,
    ) {
        let foreground = parse_color(&ctx.palette.colors.foreground);
        let overlay = parse_color(&ctx.palette.colors.overlay);
        let accent = parse_color(&ctx.palette.colors.accent);
        let surface = parse_color(&ctx.palette.colors.surface);
        let font_size = ctx.palette.fonts.size as f32;
        let (scale, ox, oy) = self.viz_transform(bounds);

        fill_rounded_rect(
            pixmap,
            bounds.x,
            bounds.y,
            bounds.width,
            VIZ_H,
            8.0,
            surface,
        );
        if self.monitors.is_empty() {
            ctx.text.draw_text(
                pixmap,
                "No connected displays reported by Hyprland",
                bounds.x + 20.0,
                bounds.y + 24.0,
                font_size,
                overlay,
                bounds.width - 40.0,
            );
        }
        for (index, monitor) in self.monitors.iter().enumerate() {
            let rect = Self::monitor_rect(monitor, scale, ox, oy);
            fill_rounded_rect(
                pixmap,
                rect.x - 2.0,
                rect.y - 2.0,
                rect.width + 4.0,
                rect.height + 4.0,
                6.0,
                if self.selected == Some(index) {
                    accent
                } else {
                    overlay
                },
            );
            fill_rounded_rect(
                pixmap,
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                4.0,
                surface,
            );
            if let Some(image) = &monitor.preview {
                draw_image(pixmap, image, rect);
            }
            ctx.text.draw_text(
                pixmap,
                &monitor.name,
                rect.x + 6.0,
                rect.y + 6.0,
                font_size.clamp(10.0, 16.0),
                foreground,
                rect.width - 12.0,
            );
            ctx.text.draw_text(
                pixmap,
                &monitor.description,
                rect.x + 6.0,
                rect.y + rect.height - font_size * 1.5,
                (font_size - 2.0).clamp(8.0, 14.0),
                foreground,
                rect.width - 12.0,
            );
        }

        let selected = self
            .selected
            .and_then(|index| self.monitors.get(index))
            .map_or("Select a display", |monitor| monitor.name.as_str());
        ctx.text.draw_text(
            pixmap,
            &format!("Wallpaper for {selected}"),
            bounds.x + H_PAD,
            bounds.y + VIZ_H + 14.0,
            font_size + 2.0,
            foreground,
            bounds.width - H_PAD * 2.0,
        );
        if self.selected.is_none() {
            return;
        }
        for (row, label) in ["Image", "Fit mode", "Apply", "Splash", "IPC"]
            .iter()
            .enumerate()
        {
            ctx.text.draw_text(
                pixmap,
                label,
                bounds.x + H_PAD,
                bounds.y + FORM_TOP + row as f32 * ROW_H + 5.0,
                font_size,
                overlay,
                bounds.width * 0.35,
            );
        }
        self.path_input
            .paint(pixmap, self.control_rect(bounds, 0), ctx);
        self.fit_dropdown
            .paint(pixmap, self.control_rect(bounds, 1), ctx);
        self.apply_button
            .paint(pixmap, self.control_rect(bounds, 2), ctx);
        self.splash_toggle
            .paint(pixmap, self.control_rect(bounds, 3), ctx);
        self.ipc_toggle
            .paint(pixmap, self.control_rect(bounds, 4), ctx);
        self.fit_dropdown
            .paint_overlay(pixmap, self.control_rect(bounds, 1), ctx);
        if !self.status.is_empty() {
            ctx.text.draw_text(
                pixmap,
                &self.status,
                bounds.x + H_PAD,
                bounds.y + FORM_TOP + 5.0 * ROW_H + 6.0,
                (font_size - 1.0).max(9.0),
                overlay,
                bounds.width - H_PAD * 2.0,
            );
        }
    }

    fn event(&mut self, event: &Event, bounds: Rect) -> EventResponse {
        let (scale, ox, oy) = self.viz_transform(bounds);
        if let Event::PointerUp { x, y } = event {
            for (index, monitor) in self.monitors.iter().enumerate() {
                if Self::monitor_rect(monitor, scale, ox, oy).contains(*x, *y) {
                    self.selected = Some(index);
                    self.sync_controls();
                    return consumed();
                }
            }
        }
        if self.selected.is_none() {
            return EventResponse::ignored();
        }

        let path_response = self.path_input.event(event, &self.control_rect(bounds, 0));
        if path_response.consumed {
            if let Some(index) = self.selected {
                self.monitors[index].path = self.path_input.value.clone();
                if let Some(preview) = load_preview(&self.path_input.value) {
                    self.monitors[index].preview = Some(preview);
                }
                self.dirty = true;
            }
            return consumed();
        }
        let fit_response = self
            .fit_dropdown
            .event(event, &self.control_rect(bounds, 1));
        if fit_response.consumed {
            if let Some(index) = self.selected {
                self.monitors[index].fit_mode = self
                    .fit_dropdown
                    .options
                    .get(self.fit_dropdown.selected)
                    .cloned()
                    .unwrap_or_else(|| "cover".into());
                self.dirty = true;
            }
            return consumed();
        }
        let apply_response = self
            .apply_button
            .event(event, &self.control_rect(bounds, 2));
        if apply_response.action.is_some() {
            self.apply_live();
            return consumed();
        }
        if apply_response.consumed {
            return consumed();
        }
        let splash_response = self
            .splash_toggle
            .event(event, &self.control_rect(bounds, 3));
        if splash_response.consumed {
            self.dirty = true;
            return consumed();
        }
        let ipc_response = self.ipc_toggle.event(event, &self.control_rect(bounds, 4));
        if ipc_response.consumed {
            self.dirty = true;
            return consumed();
        }
        EventResponse::ignored()
    }
}

fn consumed() -> EventResponse {
    EventResponse {
        consumed: true,
        action: None,
    }
}

fn connected_monitors() -> Vec<WallpaperMonitor> {
    let output = Command::new("hyprctl").args(["-j", "monitors"]).output();
    let Ok(output) = output else {
        return Vec::new();
    };
    let Ok(values) = serde_json::from_slice::<Vec<serde_json::Value>>(&output.stdout) else {
        return Vec::new();
    };
    values
        .into_iter()
        .filter_map(|value| {
            Some(WallpaperMonitor {
                name: value["name"].as_str()?.into(),
                description: value["description"].as_str().unwrap_or("").into(),
                x: value["x"].as_i64()? as i32,
                y: value["y"].as_i64()? as i32,
                width: value["width"].as_u64()? as u32,
                height: value["height"].as_u64()? as u32,
                path: String::new(),
                fit_mode: "cover".into(),
                preview: None,
            })
        })
        .collect()
}

fn active_wallpapers() -> Vec<(String, String)> {
    Command::new("hyprctl")
        .args(["hyprpaper", "listactive"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter_map(|line| line.split_once(':'))
                .map(|(monitor, path)| (monitor.trim().into(), path.trim().into()))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_wallpaper_assignments(source: &str) -> Vec<(String, String, String)> {
    let mut assignments = Vec::new();
    let mut in_block = false;
    let mut monitor = String::new();
    let mut path = String::new();
    let mut mode = "cover".to_owned();
    for line in source.lines().map(str::trim) {
        if line.starts_with("wallpaper") && line.ends_with('{') {
            in_block = true;
            monitor.clear();
            path.clear();
            mode = "cover".into();
            continue;
        }
        if in_block && line == "}" {
            if !monitor.is_empty() && !path.is_empty() {
                assignments.push((monitor.clone(), path.clone(), mode.clone()));
            }
            in_block = false;
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        if in_block {
            match key {
                "monitor" => monitor = value.into(),
                "path" => path = value.into(),
                "fit_mode" => mode = value.into(),
                _ => {}
            }
        } else if key == "wallpaper" {
            let mut parts = value.split(',').map(str::trim);
            if let (Some(mon), Some(wallpaper)) = (parts.next(), parts.next()) {
                assignments.push((mon.into(), wallpaper.into(), "cover".into()));
            }
        }
    }
    assignments
}

fn parse_bool_setting(source: &str, wanted: &str, default: bool) -> bool {
    source
        .lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(key, _)| key.trim() == wanted)
        .filter_map(|(_, value)| value.trim().parse().ok())
        .next_back()
        .unwrap_or(default)
}

fn executable_exists(executable: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|directory| directory.join(executable).is_file())
    })
}

fn load_preview(path: &str) -> Option<image::RgbaImage> {
    (!path.is_empty())
        .then(|| {
            image::open(Path::new(path))
                .ok()
                .map(|image| image.to_rgba8())
        })
        .flatten()
}

fn draw_image(pixmap: &mut tiny_skia::PixmapMut<'_>, image: &image::RgbaImage, rect: Rect) {
    let (width, height) = image.dimensions();
    let mut source = tiny_skia::Pixmap::new(width, height).expect("non-zero wallpaper image");
    for (input, output) in image.pixels().zip(source.data_mut().chunks_exact_mut(4)) {
        let [red, green, blue, alpha] = input.0;
        let factor = alpha as f32 / 255.0;
        output.copy_from_slice(&[
            (red as f32 * factor) as u8,
            (green as f32 * factor) as u8,
            (blue as f32 * factor) as u8,
            alpha,
        ]);
    }
    pixmap.draw_pixmap(
        rect.x as i32,
        rect.y as i32,
        source.as_ref(),
        &PixmapPaint::default(),
        Transform::from_scale(rect.width / width as f32, rect.height / height as f32),
        None,
    );
}

fn parse_color(value: &str) -> Color {
    Color::from_hex(value).unwrap_or(Color::new(0.5, 0.5, 0.5, 1.0))
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
            .unwrap_or("hyprpaper"),
        std::process::id()
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
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
    fn parses_modern_per_monitor_blocks() {
        let source = "wallpaper {\n monitor = DP-1\n path = /tmp/a.jpg\n fit_mode = contain\n}\n";
        assert_eq!(
            parse_wallpaper_assignments(source),
            vec![("DP-1".into(), "/tmp/a.jpg".into(), "contain".into())]
        );
    }

    #[test]
    fn parses_boolean_settings_using_last_value() {
        assert!(!parse_bool_setting(
            "splash = true\nsplash = false",
            "splash",
            true
        ));
    }
}
