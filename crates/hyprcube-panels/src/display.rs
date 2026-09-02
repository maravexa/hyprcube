use hyprcube_core::color::Color;
use hyprcube_core::layout::Rect;
use hyprcube_core::render::fill_rounded_rect;
use hyprcube_core::widget::{
    Constraints, Dropdown, Event, EventResponse, Size, Slider, Toggle, Widget,
};

use crate::{form, PanelError, PanelField, SettingsPanel};

// ── Layout constants ──────────────────────────────────────────────────────────

const VIZ_H: f32 = 300.0;
const VIZ_PAD: f32 = 20.0;
const H_PAD: f32 = 16.0;
const ROW_H: f32 = 40.0;
const ROW_GAP: f32 = 8.0;
const WIDGET_W: f32 = 200.0;
const SNAP_PX: i32 = 20;

/// Y offset from bounds.y where the settings form begins (viz + separator + header).
const FORM_TOP: f32 = VIZ_H + 84.0;

// ── Option tables ─────────────────────────────────────────────────────────────

const TRANSFORMS: &[&str] = &[
    "Normal",
    "90°",
    "180°",
    "270°",
    "Flipped",
    "Flipped 90°",
    "Flipped 180°",
    "Flipped 270°",
];

const COMMON_RESOLUTIONS: &[&str] = &[
    "1280x720",
    "1920x1080",
    "1920x1200",
    "2560x1440",
    "2560x1600",
    "3440x1440",
    "3840x2160",
];

const FALLBACK_REFRESH: &[&str] = &["30", "60", "75", "120", "144", "165", "240"];

// ── Data model ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MonitorMode {
    pub width: u32,
    pub height: u32,
    pub refresh: f32,
}

#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub name: String,
    pub description: String,
    pub make: String,
    pub model: String,
    pub serial: String,
    pub width: u32,
    pub height: u32,
    pub refresh: f32,
    pub x: i32,
    pub y: i32,
    pub scale: f32,
    /// 0=normal, 1=90°, 2=180°, 3=270°, 4..7=flipped variants.
    pub transform: u32,
    pub enabled: bool,
    pub current_format: String,
    pub vrr: bool,
    pub available_modes: Vec<MonitorMode>,
}

impl MonitorInfo {
    /// Format the `monitor =` config line for this monitor.
    pub fn config_line(&self) -> String {
        if !self.enabled {
            return format!("monitor = {}, disable", self.name);
        }
        if self.transform != 0 {
            format!(
                "monitor = {},{w}x{h}@{r},{x}x{y},{s}, transform, {t}",
                self.name,
                w = self.width,
                h = self.height,
                r = self.refresh,
                x = self.x,
                y = self.y,
                s = self.scale,
                t = self.transform,
            )
        } else {
            format!(
                "monitor = {},{w}x{h}@{r},{x}x{y},{s}",
                self.name,
                w = self.width,
                h = self.height,
                r = self.refresh,
                x = self.x,
                y = self.y,
                s = self.scale,
            )
        }
    }
}

// ── Drag state ────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct DragState {
    monitor_idx: usize,
    down_x: f32,
    down_y: f32,
    monitor_start_x: i32,
    monitor_start_y: i32,
    viz_scale: f32,
}

// ── DisplayPanel ──────────────────────────────────────────────────────────────

pub struct DisplayPanel {
    pub monitors: Vec<MonitorInfo>,
    selected: Option<usize>,
    drag_state: Option<DragState>,
    /// Live position during a drag (before PointerUp commits it).
    drag_live_x: i32,
    drag_live_y: i32,
    resolution_dropdown: Dropdown,
    refresh_dropdown: Dropdown,
    scale_slider: Slider,
    transform_dropdown: Dropdown,
    enabled_toggle: Toggle,
}

// ── Sync IPC helpers ──────────────────────────────────────────────────────────

fn ipc_socket_path() -> Option<std::path::PathBuf> {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").ok()?;
    let instance_sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").ok()?;
    let path = std::path::PathBuf::from(runtime_dir)
        .join("hypr")
        .join(&instance_sig)
        .join(".socket.sock");
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

fn ipc_request_sync(socket_path: &std::path::Path, command: &str) -> Option<String> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    let mut stream = UnixStream::connect(socket_path).ok()?;
    stream.write_all(command.as_bytes()).ok()?;
    stream.shutdown(std::net::Shutdown::Write).ok()?;
    let mut buf = String::new();
    stream.read_to_string(&mut buf).ok()?;
    Some(buf)
}

fn ipc_query_json(command: &str) -> Option<serde_json::Value> {
    let path = ipc_socket_path()?;
    let raw = ipc_request_sync(&path, &format!("j/{command}"))?;
    serde_json::from_str(&raw).ok()
}

fn apply_monitor_ipc(mon: &MonitorInfo) {
    if let Some(path) = ipc_socket_path() {
        let value = if !mon.enabled {
            format!("{}, disable", mon.name)
        } else {
            let transform = if mon.transform == 0 {
                String::new()
            } else {
                format!(",transform,{}", mon.transform)
            };
            format!(
                "{},{w}x{h}@{r},{x}x{y},{s}{transform}",
                mon.name,
                w = mon.width,
                h = mon.height,
                r = mon.refresh,
                x = mon.x,
                y = mon.y,
                s = mon.scale,
            )
        };
        tracing::debug!("IPC: keyword monitor {}", value);
        let _ = ipc_request_sync(&path, &format!("keyword monitor {value}"));
    }
}

// ── JSON parsing ──────────────────────────────────────────────────────────────

fn parse_mode_json(v: &serde_json::Value) -> Option<MonitorMode> {
    // availableModes items may be strings like "2560x1440@165.00Hz"
    // or objects with width/height/refreshRate fields.
    if let Some(s) = v.as_str() {
        // "WxH@R.HzHz" or "WxH@R"
        let s = s.trim_end_matches("Hz");
        let (res, rate) = s.split_once('@')?;
        let (w, h) = res.split_once('x')?;
        return Some(MonitorMode {
            width: w.parse().ok()?,
            height: h.parse().ok()?,
            refresh: rate.parse().ok()?,
        });
    }
    Some(MonitorMode {
        width: v["width"].as_u64()? as u32,
        height: v["height"].as_u64()? as u32,
        refresh: v["refreshRate"].as_f64()? as f32,
    })
}

fn parse_monitor_json(v: &serde_json::Value) -> Option<MonitorInfo> {
    let modes = v["availableModes"]
        .as_array()
        .map(|arr| arr.iter().filter_map(parse_mode_json).collect())
        .unwrap_or_default();

    Some(MonitorInfo {
        name: v["name"].as_str()?.to_string(),
        description: v["description"].as_str().unwrap_or("").to_string(),
        make: v["make"].as_str().unwrap_or("").to_string(),
        model: v["model"].as_str().unwrap_or("").to_string(),
        serial: v["serial"].as_str().unwrap_or("").to_string(),
        width: v["width"].as_u64()? as u32,
        height: v["height"].as_u64()? as u32,
        refresh: v["refreshRate"].as_f64()? as f32,
        x: v["x"].as_i64()? as i32,
        y: v["y"].as_i64()? as i32,
        scale: v["scale"].as_f64()? as f32,
        transform: v["transform"].as_u64().unwrap_or(0) as u32,
        enabled: v["dpmsStatus"].as_bool().unwrap_or(true),
        current_format: v["currentFormat"].as_str().unwrap_or("unknown").to_string(),
        vrr: v["vrr"].as_bool().unwrap_or(false),
        available_modes: modes,
    })
}

fn parse_resolution(s: &str) -> Option<(u32, u32)> {
    let (w, h) = s.split_once('x')?;
    Some((w.parse().ok()?, h.parse().ok()?))
}

fn parse_hex(hex: &str) -> Color {
    Color::from_hex(hex).unwrap_or(Color::new(0.5, 0.5, 0.5, 1.0))
}

// ── impl DisplayPanel ─────────────────────────────────────────────────────────

impl DisplayPanel {
    pub fn new() -> Self {
        let monitors = Self::load_monitors();
        let selected = if monitors.is_empty() { None } else { Some(0) };

        let res_opts: Vec<String> = COMMON_RESOLUTIONS.iter().map(|s| s.to_string()).collect();
        let ref_opts: Vec<String> = FALLBACK_REFRESH.iter().map(|s| s.to_string()).collect();
        let trn_opts: Vec<String> = TRANSFORMS.iter().map(|s| s.to_string()).collect();

        let mut panel = Self {
            monitors,
            selected,
            drag_state: None,
            drag_live_x: 0,
            drag_live_y: 0,
            resolution_dropdown: Dropdown::new("monitor.resolution", res_opts, 1),
            refresh_dropdown: Dropdown::new("monitor.refresh", ref_opts, 1),
            scale_slider: Slider::new("monitor.scale", 1.0, 0.5, 3.0, 0.25),
            transform_dropdown: Dropdown::new("monitor.transform", trn_opts, 0),
            enabled_toggle: Toggle::new("monitor.enabled", true),
        };
        panel.sync_widgets();
        panel
    }

    /// Query IPC for live monitor data, falling back to an empty list.
    fn load_monitors() -> Vec<MonitorInfo> {
        let json = ipc_query_json("monitors").or_else(|| {
            let output = std::process::Command::new("hyprctl")
                .args(["-j", "monitors"])
                .output()
                .ok()?;
            output
                .status
                .success()
                .then(|| serde_json::from_slice(&output.stdout).ok())
                .flatten()
        });
        if let Some(json) = json {
            if let Some(arr) = json.as_array() {
                let monitors: Vec<MonitorInfo> =
                    arr.iter().filter_map(parse_monitor_json).collect();
                if !monitors.is_empty() {
                    return monitors;
                }
            }
        }
        Vec::new()
    }

    /// Replace the monitor list (e.g. after a fresh IPC query).
    pub fn set_monitors(&mut self, monitors: Vec<MonitorInfo>) {
        let was_none = self.selected.is_none();
        self.monitors = monitors;
        if self.selected.is_some_and(|i| i >= self.monitors.len()) {
            self.selected = None;
        }
        if was_none && !self.monitors.is_empty() {
            self.selected = Some(0);
        }
        self.sync_widgets();
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    /// Build resolution options for the selected monitor from its available modes.
    fn resolution_options(mon: &MonitorInfo) -> Vec<String> {
        if mon.available_modes.is_empty() {
            return COMMON_RESOLUTIONS.iter().map(|s| s.to_string()).collect();
        }
        let mut seen = std::collections::HashSet::new();
        let mut opts = Vec::new();
        for m in &mon.available_modes {
            let key = format!("{}x{}", m.width, m.height);
            if seen.insert(key.clone()) {
                opts.push(key);
            }
        }
        opts
    }

    /// Build refresh rate options for the given resolution from available modes.
    fn refresh_options(mon: &MonitorInfo, res: &str) -> Vec<String> {
        if mon.available_modes.is_empty() {
            return FALLBACK_REFRESH.iter().map(|s| s.to_string()).collect();
        }
        let Some((w, h)) = parse_resolution(res) else {
            return FALLBACK_REFRESH.iter().map(|s| s.to_string()).collect();
        };
        let mut rates: Vec<f32> = mon
            .available_modes
            .iter()
            .filter(|m| m.width == w && m.height == h)
            .map(|m| m.refresh)
            .collect();
        rates.dedup();
        rates.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        if rates.is_empty() {
            return FALLBACK_REFRESH.iter().map(|s| s.to_string()).collect();
        }
        rates.iter().map(|r| format!("{}", *r as u32)).collect()
    }

    /// Push the currently selected monitor's values into the form widgets.
    fn sync_widgets(&mut self) {
        let Some(idx) = self.selected else { return };
        let Some(mon) = self.monitors.get(idx) else {
            return;
        };

        // Resolution options
        let res_opts = Self::resolution_options(mon);
        let current_res = format!("{}x{}", mon.width, mon.height);
        let res_sel = res_opts.iter().position(|o| o == &current_res).unwrap_or(0);
        self.resolution_dropdown = Dropdown::new("monitor.resolution", res_opts.clone(), res_sel);

        // Refresh options (filtered by selected resolution)
        let active_res = res_opts.get(res_sel).cloned().unwrap_or(current_res);
        let ref_opts = Self::refresh_options(mon, &active_res);
        let current_ref = format!("{}", mon.refresh as u32);
        let ref_sel = ref_opts.iter().position(|o| o == &current_ref).unwrap_or(0);
        self.refresh_dropdown = Dropdown::new("monitor.refresh", ref_opts, ref_sel);

        self.scale_slider.set_value(mon.scale as f64);
        self.transform_dropdown.selected = (mon.transform as usize).min(TRANSFORMS.len() - 1);
        self.enabled_toggle.value = mon.enabled;
    }

    /// When the resolution widget changes, rebuild the refresh dropdown to match.
    fn rebuild_refresh_for_selected_res(&mut self) {
        let Some(idx) = self.selected else { return };
        let Some(mon) = self.monitors.get(idx) else {
            return;
        };

        let res = self
            .resolution_dropdown
            .options
            .get(self.resolution_dropdown.selected)
            .cloned()
            .unwrap_or_default();

        let ref_opts = Self::refresh_options(mon, &res);
        let current_ref = format!("{}", mon.refresh as u32);
        let ref_sel = ref_opts.iter().position(|o| o == &current_ref).unwrap_or(0);
        self.refresh_dropdown = Dropdown::new("monitor.refresh", ref_opts, ref_sel);
    }

    /// Read form widgets, write them to the selected monitor, send IPC.
    fn apply_selected(&mut self) {
        let Some(idx) = self.selected else { return };

        let res = self
            .resolution_dropdown
            .options
            .get(self.resolution_dropdown.selected)
            .cloned()
            .unwrap_or_default();
        let ref_s = self
            .refresh_dropdown
            .options
            .get(self.refresh_dropdown.selected)
            .cloned()
            .unwrap_or_default();
        let scale = self.scale_slider.value as f32;
        let transform = self.transform_dropdown.selected as u32;
        let enabled = self.enabled_toggle.value;

        if let Some(mon) = self.monitors.get_mut(idx) {
            if let Some((w, h)) = parse_resolution(&res) {
                mon.width = w;
                mon.height = h;
            }
            if let Ok(r) = ref_s.parse::<f32>() {
                mon.refresh = r;
            }
            mon.scale = scale;
            mon.transform = transform;
            mon.enabled = enabled;
        }

        if let Some(mon) = self.monitors.get(idx).cloned() {
            apply_monitor_ipc(&mon);
        }
    }

    /// Compute (scale, offset_x, offset_y) mapping logical monitor coords →
    /// visualization pixel coords so all monitors fit within `viz` with padding.
    fn viz_transform(&self, viz: &Rect) -> (f32, f32, f32) {
        if self.monitors.is_empty() {
            return (1.0, viz.x + VIZ_PAD, viz.y + VIZ_PAD);
        }
        let min_x = self.monitors.iter().map(|m| m.x).min().unwrap() as f32;
        let min_y = self.monitors.iter().map(|m| m.y).min().unwrap() as f32;
        let max_x = self
            .monitors
            .iter()
            .map(|m| m.x + m.width as i32)
            .max()
            .unwrap() as f32;
        let max_y = self
            .monitors
            .iter()
            .map(|m| m.y + m.height as i32)
            .max()
            .unwrap() as f32;

        let total_w = (max_x - min_x).max(1.0);
        let total_h = (max_y - min_y).max(1.0);
        let avail_w = viz.width - 2.0 * VIZ_PAD;
        let avail_h = viz.height - 2.0 * VIZ_PAD;

        let scale = (avail_w / total_w).min(avail_h / total_h);
        let ox = viz.x + VIZ_PAD + (avail_w - total_w * scale) / 2.0 - min_x * scale;
        let oy = viz.y + VIZ_PAD + (avail_h - total_h * scale) / 2.0 - min_y * scale;
        (scale, ox, oy)
    }

    fn monitor_rect(mon: &MonitorInfo, scale: f32, ox: f32, oy: f32) -> Rect {
        Rect {
            x: ox + mon.x as f32 * scale,
            y: oy + mon.y as f32 * scale,
            width: (mon.width as f32 * scale).max(1.0),
            height: (mon.height as f32 * scale).max(1.0),
        }
    }

    /// Snap (nx, ny) for monitor `idx` to the edges of its neighbours.
    fn snap(&self, idx: usize, nx: i32, ny: i32) -> (i32, i32) {
        let mon = &self.monitors[idx];
        let (mut x, mut y) = (nx, ny);
        let s = SNAP_PX;
        let mw = mon.width as i32;
        let mh = mon.height as i32;

        for (i, other) in self.monitors.iter().enumerate() {
            if i == idx {
                continue;
            }
            let ow = other.width as i32;
            let oh = other.height as i32;
            // Horizontal edges
            if (x + mw - other.x).abs() < s {
                x = other.x - mw;
            }
            if (x - (other.x + ow)).abs() < s {
                x = other.x + ow;
            }
            if (x - other.x).abs() < s {
                x = other.x;
            }
            // Vertical edges
            if (y + mh - other.y).abs() < s {
                y = other.y - mh;
            }
            if (y - (other.y + oh)).abs() < s {
                y = other.y + oh;
            }
            if (y - other.y).abs() < s {
                y = other.y;
            }
        }
        (x, y)
    }

    fn widget_rect(&self, bounds: Rect, row: usize) -> Rect {
        let row_top = bounds.y + FORM_TOP + row as f32 * (ROW_H + ROW_GAP);
        Rect {
            x: bounds.x + bounds.width - WIDGET_W - H_PAD,
            y: row_top + (ROW_H - 32.0) / 2.0,
            width: WIDGET_W,
            height: 32.0,
        }
    }

    fn label_rect(&self, bounds: Rect, row: usize, font_size: f32) -> Rect {
        let row_top = bounds.y + FORM_TOP + row as f32 * (ROW_H + ROW_GAP);
        Rect {
            x: bounds.x + H_PAD,
            y: row_top + (ROW_H - font_size * 1.3) / 2.0,
            width: bounds.width * 0.45,
            height: font_size * 1.3,
        }
    }
}

impl Default for DisplayPanel {
    fn default() -> Self {
        Self::new()
    }
}

// ── SettingsPanel impl ────────────────────────────────────────────────────────

impl SettingsPanel for DisplayPanel {
    fn title(&self) -> &str {
        "Display"
    }
    fn icon(&self) -> &str {
        "preferences-desktop-display"
    }

    fn uses_custom_layout(&self) -> bool {
        true
    }

    fn fields(&self) -> Vec<PanelField> {
        Vec::new()
    }

    fn set_value(&mut self, key: &str, _value: &str) -> Result<String, PanelError> {
        Err(PanelError::UnknownKey(key.to_string()))
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
        let surface = parse_hex(&ctx.palette.colors.surface);
        let overlay = parse_hex(&ctx.palette.colors.overlay);
        let accent = parse_hex(&ctx.palette.colors.accent);
        let fg = parse_hex(&ctx.palette.colors.foreground);
        let bg = parse_hex(&ctx.palette.colors.background);
        let font_size = ctx.palette.fonts.size as f32;

        let viz = Rect {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: VIZ_H,
        };

        // ── Visualization background ──────────────────────────────────────────
        fill_rounded_rect(pixmap, viz.x, viz.y, viz.width, viz.height, 8.0, surface);

        // Dotted grid (1px dots every 40px, overlay at 30% opacity)
        paint_dotted_grid(
            pixmap,
            viz,
            Color::new(overlay.r, overlay.g, overlay.b, overlay.a * 0.3),
        );

        if self.monitors.is_empty() {
            let msg = "No monitors detected";
            let mx = viz.x + (viz.width - msg.len() as f32 * font_size * 0.55) / 2.0;
            let my = viz.y + (viz.height - font_size * 1.3) / 2.0;
            ctx.text
                .draw_text(pixmap, msg, mx, my, font_size, overlay, viz.width);
        } else {
            let (vs, vox, voy) = self.viz_transform(&viz);

            // ── Snap guide lines (drawn behind monitors) ──────────────────────
            if let Some(ref drag) = self.drag_state {
                paint_snap_guides(
                    pixmap,
                    drag.monitor_idx,
                    self.drag_live_x,
                    self.drag_live_y,
                    &self.monitors,
                    vs,
                    vox,
                    voy,
                    accent,
                    viz,
                );
            }

            // ── Monitor rectangles ────────────────────────────────────────────
            for (i, mon) in self.monitors.iter().enumerate() {
                // During a drag, show the dragged monitor at its live position.
                let draw_mon;
                let mon_ref = if self.drag_state.as_ref().is_some_and(|d| d.monitor_idx == i) {
                    draw_mon = MonitorInfo {
                        x: self.drag_live_x,
                        y: self.drag_live_y,
                        ..mon.clone()
                    };
                    &draw_mon
                } else {
                    mon
                };

                let mr = Self::monitor_rect(mon_ref, vs, vox, voy);
                let is_sel = self.selected == Some(i);

                // Border ring (2 px outer)
                let border_c = if is_sel { accent } else { overlay };
                fill_rounded_rect(
                    pixmap,
                    mr.x - 2.0,
                    mr.y - 2.0,
                    mr.width + 4.0,
                    mr.height + 4.0,
                    6.0,
                    border_c,
                );

                // Monitor fill — dimmed when disabled
                let fill_c = if mon.enabled {
                    if is_sel {
                        Color::new(
                            accent.r * 0.15 + bg.r * 0.85,
                            accent.g * 0.15 + bg.g * 0.85,
                            accent.b * 0.15 + bg.b * 0.85,
                            1.0,
                        )
                    } else {
                        bg
                    }
                } else {
                    Color::new(bg.r * 0.5, bg.g * 0.5, bg.b * 0.5, 1.0)
                };
                fill_rounded_rect(pixmap, mr.x, mr.y, mr.width, mr.height, 4.0, fill_c);

                // Hash lines over disabled monitors
                if !mon.enabled {
                    paint_hash_lines(pixmap, mr, overlay);
                }

                // Labels
                let lfs = (mr.height / 4.5).clamp(8.0, 13.0);
                let lx = mr.x + 4.0;
                let ly = mr.y + mr.height / 2.0 - lfs * 1.4;
                ctx.text
                    .draw_text(pixmap, &mon.name, lx, ly, lfs, fg, mr.width - 8.0);
                let res_str = format!("{}×{}@{}", mon.width, mon.height, mon.refresh as u32);
                ctx.text.draw_text(
                    pixmap,
                    &res_str,
                    lx,
                    ly + lfs * 1.5,
                    (lfs * 0.85).max(7.0),
                    overlay,
                    mr.width - 8.0,
                );
                if (mon.scale - 1.0).abs() > 0.01 {
                    let sc_str = format!("×{:.2}", mon.scale);
                    ctx.text.draw_text(
                        pixmap,
                        &sc_str,
                        lx,
                        ly + lfs * 3.0,
                        (lfs * 0.8).max(7.0),
                        accent,
                        mr.width - 8.0,
                    );
                }
            }
        }

        // ── Separator ─────────────────────────────────────────────────────────
        fill_rounded_rect(
            pixmap,
            bounds.x,
            bounds.y + VIZ_H + 4.0,
            bounds.width,
            1.0,
            0.0,
            overlay,
        );

        // ── Section header ────────────────────────────────────────────────────
        let header_text = match self.selected.and_then(|i| self.monitors.get(i)) {
            Some(mon) => format!("Monitor: {}", mon.name),
            None if self.monitors.is_empty() => "Connect a monitor to configure".to_string(),
            None => "Click a monitor to select".to_string(),
        };
        ctx.text.draw_text(
            pixmap,
            &header_text,
            bounds.x + H_PAD,
            bounds.y + VIZ_H + 8.0,
            font_size + 2.0,
            fg,
            bounds.width - H_PAD * 2.0,
        );

        if let Some(mon) = self.selected.and_then(|index| self.monitors.get(index)) {
            let identity = if mon.description.is_empty() {
                format!("{} {}", mon.make, mon.model)
            } else {
                mon.description.clone()
            };
            ctx.text.draw_text(
                pixmap,
                &identity,
                bounds.x + H_PAD,
                bounds.y + VIZ_H + 34.0,
                (font_size - 1.0).max(9.0),
                overlay,
                bounds.width - H_PAD * 2.0,
            );
            let graphics = format!(
                "{} · {} Hz · VRR {} · scale {:.2} · serial {}",
                mon.current_format,
                mon.refresh,
                if mon.vrr { "on" } else { "off" },
                mon.scale,
                if mon.serial.is_empty() {
                    "unknown"
                } else {
                    &mon.serial
                }
            );
            ctx.text.draw_text(
                pixmap,
                &graphics,
                bounds.x + H_PAD,
                bounds.y + VIZ_H + 54.0,
                (font_size - 2.0).max(8.0),
                overlay,
                bounds.width - H_PAD * 2.0,
            );
        }

        // ── Settings form ─────────────────────────────────────────────────────
        if self.selected.is_some() {
            let row_labels = [
                "Resolution",
                "Refresh Rate",
                "Scale",
                "Transform",
                "Enabled",
            ];
            for (row, lbl) in row_labels.iter().enumerate() {
                let lr = self.label_rect(bounds, row, font_size);
                ctx.text
                    .draw_text(pixmap, lbl, lr.x, lr.y, font_size, overlay, lr.width);
            }
            self.resolution_dropdown
                .paint(pixmap, self.widget_rect(bounds, 0), ctx);
            self.refresh_dropdown
                .paint(pixmap, self.widget_rect(bounds, 1), ctx);
            self.scale_slider
                .paint(pixmap, self.widget_rect(bounds, 2), ctx);
            self.transform_dropdown
                .paint(pixmap, self.widget_rect(bounds, 3), ctx);
            self.enabled_toggle
                .paint(pixmap, self.widget_rect(bounds, 4), ctx);

            // Overlay pass for open dropdowns
            self.resolution_dropdown
                .paint_overlay(pixmap, self.widget_rect(bounds, 0), ctx);
            self.refresh_dropdown
                .paint_overlay(pixmap, self.widget_rect(bounds, 1), ctx);
            self.transform_dropdown
                .paint_overlay(pixmap, self.widget_rect(bounds, 3), ctx);
        }
    }

    fn event(&mut self, event: &Event, bounds: Rect) -> EventResponse {
        let viz = Rect {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: VIZ_H,
        };
        let (vs, vox, voy) = self.viz_transform(&viz);

        match event {
            // ── PointerDown inside viz → select / start drag ──────────────────
            Event::PointerDown { x, y } if viz.contains(*x, *y) => {
                for (i, mon) in self.monitors.iter().enumerate() {
                    let mr = Self::monitor_rect(mon, vs, vox, voy);
                    if mr.contains(*x, *y) {
                        let was_sel = self.selected == Some(i);
                        self.selected = Some(i);
                        self.drag_live_x = mon.x;
                        self.drag_live_y = mon.y;
                        self.drag_state = Some(DragState {
                            monitor_idx: i,
                            down_x: *x,
                            down_y: *y,
                            monitor_start_x: mon.x,
                            monitor_start_y: mon.y,
                            viz_scale: vs,
                        });
                        if !was_sel {
                            self.sync_widgets();
                        }
                        return EventResponse {
                            consumed: true,
                            action: None,
                        };
                    }
                }
                EventResponse::ignored()
            }

            // ── PointerMove → live drag update ────────────────────────────────
            Event::PointerMove { x, y } => {
                if let Some(ref drag) = self.drag_state {
                    let dx = (*x - drag.down_x) / drag.viz_scale;
                    let dy = (*y - drag.down_y) / drag.viz_scale;
                    let nx = drag.monitor_start_x + dx as i32;
                    let ny = drag.monitor_start_y + dy as i32;
                    let (sx, sy) = self.snap(drag.monitor_idx, nx, ny);
                    self.drag_live_x = sx;
                    self.drag_live_y = sy;
                    return EventResponse {
                        consumed: true,
                        action: None,
                    };
                }
                self.route_form(event, bounds)
            }

            // ── PointerUp → commit drag or route to form ──────────────────────
            Event::PointerUp { x, y } => {
                if let Some(drag) = self.drag_state.take() {
                    let dx = (*x - drag.down_x) / drag.viz_scale;
                    let dy = (*y - drag.down_y) / drag.viz_scale;

                    if dx.abs() > 5.0 || dy.abs() > 5.0 {
                        if let Some(mon) = self.monitors.get_mut(drag.monitor_idx) {
                            mon.x = self.drag_live_x;
                            mon.y = self.drag_live_y;
                        }
                        if let Some(mon) = self.monitors.get(drag.monitor_idx).cloned() {
                            apply_monitor_ipc(&mon);
                        }
                    }
                    return EventResponse {
                        consumed: true,
                        action: None,
                    };
                }
                self.route_form(event, bounds)
            }

            // ── PointerDown outside viz → form ────────────────────────────────
            Event::PointerDown { x, y } if !viz.contains(*x, *y) => self.route_form(event, bounds),

            // Slider value boxes and searchable/dropdown fields keep their
            // own keyboard-focus state, so keyboard input must reach the
            // form even though it has no pointer coordinates.
            Event::KeyPress { .. } => self.route_form(event, bounds),

            _ => EventResponse::ignored(),
        }
    }
}

// ── Form event routing ────────────────────────────────────────────────────────

impl DisplayPanel {
    fn route_form(&mut self, event: &Event, bounds: Rect) -> EventResponse {
        if self.selected.is_none() {
            return EventResponse::ignored();
        }

        let rects = [
            self.widget_rect(bounds, 0),
            self.widget_rect(bounds, 1),
            self.widget_rect(bounds, 2),
            self.widget_rect(bounds, 3),
            self.widget_rect(bounds, 4),
        ];

        // Resolution dropdown — rebuild refresh list on change
        {
            let resp = self.resolution_dropdown.event(event, &rects[0]);
            if resp.consumed {
                self.rebuild_refresh_for_selected_res();
                self.apply_selected();
                return resp;
            }
        }
        // Refresh dropdown
        {
            let resp = self.refresh_dropdown.event(event, &rects[1]);
            if resp.consumed {
                self.apply_selected();
                return resp;
            }
        }
        // Scale slider
        {
            let resp = self.scale_slider.event(event, &rects[2]);
            if resp.consumed {
                self.apply_selected();
                return resp;
            }
        }
        // Transform dropdown
        {
            let resp = self.transform_dropdown.event(event, &rects[3]);
            if resp.consumed {
                self.apply_selected();
                return resp;
            }
        }
        // Enabled toggle
        {
            let resp = self.enabled_toggle.event(event, &rects[4]);
            if resp.consumed {
                self.apply_selected();
                return resp;
            }
        }

        EventResponse::ignored()
    }
}

// ── Drawing helpers ───────────────────────────────────────────────────────────

/// Draw a 1×1 dot grid over the visualization area.
fn paint_dotted_grid(pixmap: &mut tiny_skia::PixmapMut<'_>, viz: Rect, color: Color) {
    use tiny_skia::{FillRule, Paint, PathBuilder, Transform};
    let mut paint = Paint::default();
    paint.set_color(color.to_skia());
    paint.anti_alias = false;

    let step = 40.0f32;
    let mut gx = viz.x + step;
    while gx < viz.x + viz.width - 1.0 {
        let mut gy = viz.y + step;
        while gy < viz.y + viz.height - 1.0 {
            if let Some(r) = tiny_skia::Rect::from_xywh(gx.floor(), gy.floor(), 1.0, 1.0) {
                let path = PathBuilder::from_rect(r);
                pixmap.fill_path(
                    &path,
                    &paint,
                    FillRule::Winding,
                    Transform::identity(),
                    None,
                );
            }
            gy += step;
        }
        gx += step;
    }
}

/// Draw diagonal hash lines over a disabled monitor rectangle.
fn paint_hash_lines(pixmap: &mut tiny_skia::PixmapMut<'_>, mr: Rect, color: Color) {
    use tiny_skia::{Paint, PathBuilder, Stroke, Transform};
    let mut paint = Paint::default();
    paint.set_color(Color::new(color.r, color.g, color.b, color.a * 0.4).to_skia());
    paint.anti_alias = true;

    let stroke = Stroke {
        width: 1.0,
        ..Default::default()
    };
    let step = 12.0f32;
    let span = mr.width + mr.height;
    let mut offset = 0.0f32;

    while offset < span {
        let x0 = mr.x + offset;
        let y0 = mr.y;
        let x1 = mr.x;
        let y1 = mr.y + offset;

        let ax = x0.min(mr.x + mr.width);
        let ay = (y0 + (ax - x0)).max(mr.y);
        let bx = x1.max(mr.x);
        let by = (y1 + (mr.x - x1)).min(mr.y + mr.height);

        if ax <= mr.x + mr.width && by >= mr.y {
            let mut pb = PathBuilder::new();
            pb.move_to(ax, ay);
            pb.line_to(bx, by);
            if let Some(path) = pb.finish() {
                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }
        offset += step;
    }
}

/// Draw accent-colored guide lines where the dragged monitor snaps to neighbours.
#[allow(clippy::too_many_arguments)] // Snap-guide geometry is intentionally explicit at the render boundary.
fn paint_snap_guides(
    pixmap: &mut tiny_skia::PixmapMut<'_>,
    drag_idx: usize,
    lx: i32,
    ly: i32,
    monitors: &[MonitorInfo],
    scale: f32,
    ox: f32,
    oy: f32,
    accent: Color,
    viz: Rect,
) {
    use tiny_skia::{Paint, PathBuilder, Stroke, Transform};
    let mon = &monitors[drag_idx];
    let mw = mon.width as i32;
    let mh = mon.height as i32;
    let s = SNAP_PX;

    let mut paint = Paint::default();
    paint.set_color(Color::new(accent.r, accent.g, accent.b, 0.7).to_skia());
    paint.anti_alias = false;
    let stroke = Stroke {
        width: 1.0,
        ..Default::default()
    };

    let draw_vline = |pixmap: &mut tiny_skia::PixmapMut<'_>, lx_log: i32| {
        let sx = ox + lx_log as f32 * scale;
        if sx < viz.x || sx > viz.x + viz.width {
            return;
        }
        let mut pb = PathBuilder::new();
        pb.move_to(sx, viz.y);
        pb.line_to(sx, viz.y + viz.height);
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    };
    let draw_hline = |pixmap: &mut tiny_skia::PixmapMut<'_>, ly_log: i32| {
        let sy = oy + ly_log as f32 * scale;
        if sy < viz.y || sy > viz.y + viz.height {
            return;
        }
        let mut pb = PathBuilder::new();
        pb.move_to(viz.x, sy);
        pb.line_to(viz.x + viz.width, sy);
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    };

    for (i, other) in monitors.iter().enumerate() {
        if i == drag_idx {
            continue;
        }
        let ow = other.width as i32;
        let oh = other.height as i32;
        if (lx + mw - other.x).abs() < s {
            draw_vline(pixmap, other.x);
        }
        if (lx - (other.x + ow)).abs() < s {
            draw_vline(pixmap, other.x + ow);
        }
        if (lx - other.x).abs() < s {
            draw_vline(pixmap, other.x);
        }
        if (ly + mh - other.y).abs() < s {
            draw_hline(pixmap, other.y);
        }
        if (ly - (other.y + oh)).abs() < s {
            draw_hline(pixmap, other.y + oh);
        }
        if (ly - other.y).abs() < s {
            draw_hline(pixmap, other.y);
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn mon(name: &str, w: u32, h: u32, x: i32, y: i32) -> MonitorInfo {
        MonitorInfo {
            name: name.into(),
            description: String::new(),
            make: String::new(),
            model: String::new(),
            serial: String::new(),
            width: w,
            height: h,
            refresh: 60.0,
            x,
            y,
            scale: 1.0,
            transform: 0,
            enabled: true,
            current_format: "XRGB8888".into(),
            vrr: false,
            available_modes: Vec::new(),
        }
    }

    #[test]
    fn new_has_no_fields() {
        let panel = DisplayPanel::new();
        assert!(panel.fields().is_empty());
    }

    #[test]
    fn set_monitors_selects_first() {
        let mut panel = DisplayPanel::new();
        panel.set_monitors(vec![mon("DP-1", 1920, 1080, 0, 0)]);
        assert_eq!(panel.monitors.len(), 1);
        assert_eq!(panel.selected, Some(0));
    }

    #[test]
    fn set_monitors_clears_out_of_range_selection() {
        let mut panel = DisplayPanel::new();
        panel.set_monitors(vec![
            mon("A", 1920, 1080, 0, 0),
            mon("B", 1920, 1080, 1920, 0),
        ]);
        panel.selected = Some(1);
        panel.set_monitors(vec![mon("A", 1920, 1080, 0, 0)]);
        assert_eq!(panel.selected, None);
    }

    #[test]
    fn config_line_normal() {
        let m = mon("DP-1", 1920, 1080, 0, 0);
        let line = m.config_line();
        assert!(line.starts_with("monitor = DP-1,"));
        assert!(line.contains("1920x1080"));
    }

    #[test]
    fn config_line_disabled() {
        let m = MonitorInfo {
            enabled: false,
            ..mon("DP-1", 1920, 1080, 0, 0)
        };
        assert_eq!(m.config_line(), "monitor = DP-1, disable");
    }

    #[test]
    fn config_line_transform() {
        let m = MonitorInfo {
            transform: 1,
            ..mon("DP-1", 1920, 1080, 0, 0)
        };
        assert!(m.config_line().contains("transform, 1"));
    }

    #[test]
    fn snap_aligns_right_edge_to_neighbour() {
        let mut panel = DisplayPanel::new();
        panel.set_monitors(vec![
            mon("A", 1920, 1080, 0, 0),
            mon("B", 1920, 1080, 1920, 0),
        ]);
        let (sx, _) = panel.snap(1, 1914, 0);
        assert_eq!(sx, 1920);
    }

    #[test]
    fn snap_does_not_fire_beyond_threshold() {
        let mut panel = DisplayPanel::new();
        panel.set_monitors(vec![
            mon("A", 1920, 1080, 0, 0),
            mon("B", 1920, 1080, 1920, 0),
        ]);
        let (sx, _) = panel.snap(1, 2000, 0);
        assert_eq!(sx, 2000);
    }

    #[test]
    fn viz_transform_nonempty() {
        let mut panel = DisplayPanel::new();
        panel.set_monitors(vec![mon("M", 1920, 1080, 0, 0)]);
        let viz = Rect {
            x: 0.0,
            y: 0.0,
            width: 600.0,
            height: 300.0,
        };
        let (scale, ox, oy) = panel.viz_transform(&viz);
        assert!(scale > 0.0);
        assert!(ox >= VIZ_PAD);
        assert!(oy >= VIZ_PAD);
    }

    #[test]
    fn parse_resolution_ok() {
        assert_eq!(parse_resolution("1920x1080"), Some((1920, 1080)));
        assert_eq!(parse_resolution("3840x2160"), Some((3840, 2160)));
        assert_eq!(parse_resolution("bad"), None);
    }

    #[test]
    fn set_value_always_errors() {
        let mut panel = DisplayPanel::new();
        assert!(panel.set_value("monitor.resolution", "1920x1080").is_err());
    }

    #[test]
    fn resolution_options_fallback_when_no_modes() {
        let m = mon("DP-1", 1920, 1080, 0, 0);
        let opts = DisplayPanel::resolution_options(&m);
        assert!(!opts.is_empty());
        assert!(opts.iter().any(|o| o.contains("1920")));
    }

    #[test]
    fn resolution_options_from_modes() {
        let mut m = mon("DP-1", 2560, 1440, 0, 0);
        m.available_modes = vec![
            MonitorMode {
                width: 2560,
                height: 1440,
                refresh: 165.0,
            },
            MonitorMode {
                width: 1920,
                height: 1080,
                refresh: 60.0,
            },
        ];
        let opts = DisplayPanel::resolution_options(&m);
        assert_eq!(opts.len(), 2);
        assert!(opts.contains(&"2560x1440".to_string()));
    }

    #[test]
    fn refresh_options_filtered_by_resolution() {
        let mut m = mon("DP-1", 2560, 1440, 0, 0);
        m.available_modes = vec![
            MonitorMode {
                width: 2560,
                height: 1440,
                refresh: 165.0,
            },
            MonitorMode {
                width: 2560,
                height: 1440,
                refresh: 60.0,
            },
            MonitorMode {
                width: 1920,
                height: 1080,
                refresh: 144.0,
            },
        ];
        let opts = DisplayPanel::refresh_options(&m, "2560x1440");
        assert_eq!(opts.len(), 2);
        assert!(!opts.contains(&"144".to_string()));
    }
}
