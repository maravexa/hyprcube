use std::process::Command;

use hyprcube_core::color::Color;
use hyprcube_core::layout::Rect;
use hyprcube_core::render::fill_rounded_rect;
use hyprcube_core::widget::{
    Constraints, Dropdown, Event, EventResponse, Size, Slider, Toggle, Widget,
};

use crate::{form, PanelError, PanelField, SettingsPanel};

// ── Layout constants ─────────────────────────────────────────────────────────

/// Height of the monitor-layout visualization area.
const VIZ_H: f32 = 300.0;
/// Padding inside the visualization area on all sides.
const VIZ_PAD: f32 = 20.0;
/// Horizontal padding for the settings form below.
const H_PAD: f32 = 16.0;
/// Height of each form row.
const ROW_H: f32 = 40.0;
/// Gap between form rows.
const ROW_GAP: f32 = 8.0;
/// Width of right-aligned form widgets.
const WIDGET_W: f32 = 200.0;
/// Magnetic-snap threshold in logical pixels.
const SNAP_PX: f32 = 10.0;

/// Y offset from bounds.y where the first form row begins:
///   VIZ_H  +  8px gap  +  36px section-header text  =  344
const FORM_TOP: f32 = VIZ_H + 44.0;

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
    "2560x1440",
    "3840x2160",
    "2560x1080",
    "3440x1440",
];

const REFRESH_RATES: &[&str] = &["30", "60", "75", "120", "144", "165", "240"];

// ── Data model ────────────────────────────────────────────────────────────────

/// Full monitor descriptor, matching Hyprland's data model.
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Refresh rate in Hz, e.g. `144.0`.
    pub refresh: f32,
    pub x: i32,
    pub y: i32,
    pub scale: f32,
    /// 0=normal, 1=90°, 2=180°, 3=270°, 4..7=flipped variants.
    pub transform: u8,
    pub enabled: bool,
}

impl MonitorInfo {
    /// Format the `monitor =` config line for this monitor.
    pub fn config_line(&self) -> String {
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

// ── Drag state ────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct DragState {
    monitor_idx: usize,
    /// Screen position where the pointer went down.
    down_x: f32,
    down_y: f32,
    /// Monitor's logical position at drag start.
    monitor_start_x: i32,
    monitor_start_y: i32,
    /// Viz scale factor at drag start (screen px per logical px).
    viz_scale: f32,
}

// ── DisplayPanel ──────────────────────────────────────────────────────────────

pub struct DisplayPanel {
    pub monitors: Vec<MonitorInfo>,
    selected: Option<usize>,
    drag_state: Option<DragState>,
    resolution_dropdown: Dropdown,
    refresh_dropdown: Dropdown,
    scale_slider: Slider,
    transform_dropdown: Dropdown,
    enabled_toggle: Toggle,
}

// ── JSON / IPC helpers ────────────────────────────────────────────────────────

fn parse_monitor_json(v: &serde_json::Value) -> Option<MonitorInfo> {
    Some(MonitorInfo {
        name:      v["name"].as_str()?.to_string(),
        width:     v["width"].as_u64()? as u32,
        height:    v["height"].as_u64()? as u32,
        refresh:   v["refreshRate"].as_f64()? as f32,
        x:         v["x"].as_i64()? as i32,
        y:         v["y"].as_i64()? as i32,
        scale:     v["scale"].as_f64()? as f32,
        transform: v["transform"].as_u64()? as u8,
        enabled:   v["dpmsStatus"].as_bool().unwrap_or(true),
    })
}

fn parse_resolution(s: &str) -> Option<(u32, u32)> {
    let mut it = s.splitn(2, 'x');
    let w = it.next()?.parse().ok()?;
    let h = it.next()?.parse().ok()?;
    Some((w, h))
}

fn apply_monitor_ipc(mon: &MonitorInfo) {
    let value = format!(
        "{},{w}x{h}@{r},{x}x{y},{s}",
        mon.name,
        w = mon.width,
        h = mon.height,
        r = mon.refresh,
        x = mon.x,
        y = mon.y,
        s = mon.scale,
    );
    tracing::debug!("IPC: keyword monitor {}", value);
    let _ = Command::new("hyprctl")
        .args(["keyword", "monitor", &value])
        .output();
}

fn parse_hex(hex: &str) -> Color {
    Color::from_hex(hex).unwrap_or(Color::new(0.5, 0.5, 0.5, 1.0))
}

// ── impl DisplayPanel ─────────────────────────────────────────────────────────

impl DisplayPanel {
    pub fn new() -> Self {
        let monitors = Self::load_monitors();
        let selected = if monitors.is_empty() { None } else { Some(0) };

        let res_opts: Vec<String> =
            COMMON_RESOLUTIONS.iter().map(|s| s.to_string()).collect();
        let ref_opts: Vec<String> =
            REFRESH_RATES.iter().map(|s| s.to_string()).collect();
        let trn_opts: Vec<String> =
            TRANSFORMS.iter().map(|s| s.to_string()).collect();

        let mut panel = Self {
            monitors,
            selected,
            drag_state: None,
            // Default selections: 1920×1080 @ 60 Hz, scale 1.0, Normal transform
            resolution_dropdown: Dropdown::new("monitor.resolution", res_opts, 1),
            refresh_dropdown:    Dropdown::new("monitor.refresh",    ref_opts, 1),
            scale_slider:        Slider::new("monitor.scale", 1.0, 0.5, 3.0, 0.25),
            transform_dropdown:  Dropdown::new("monitor.transform",  trn_opts, 0),
            enabled_toggle:      Toggle::new("monitor.enabled", true),
        };
        panel.sync_widgets();
        panel
    }

    /// Query `hyprctl -j monitors` and return parsed monitor data (best-effort).
    fn load_monitors() -> Vec<MonitorInfo> {
        let Ok(out) = Command::new("hyprctl").args(["-j", "monitors"]).output() else {
            return Vec::new();
        };
        if !out.status.success() {
            return Vec::new();
        }
        let json: serde_json::Value = match serde_json::from_slice(&out.stdout) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("hyprctl -j monitors parse error: {e}");
                return Vec::new();
            }
        };
        json.as_array()
            .map(|arr| arr.iter().filter_map(parse_monitor_json).collect())
            .unwrap_or_default()
    }

    /// Replace the monitor list (e.g. after a fresh IPC query from the caller).
    pub fn set_monitors(&mut self, monitors: Vec<MonitorInfo>) {
        let was_none = self.selected.is_none();
        self.monitors = monitors;
        // Drop selection if it became out-of-range.
        if self.selected.map_or(false, |i| i >= self.monitors.len()) {
            self.selected = None;
        }
        // Auto-select first monitor if nothing was previously selected.
        if was_none && !self.monitors.is_empty() {
            self.selected = Some(0);
        }
        self.sync_widgets();
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    /// Push the currently selected monitor's values into the form widgets.
    fn sync_widgets(&mut self) {
        let Some(idx) = self.selected else { return };
        let Some(mon) = self.monitors.get(idx) else { return };

        let res = format!("{}x{}", mon.width, mon.height);
        if let Some(p) = self.resolution_dropdown.options.iter().position(|o| o == &res) {
            self.resolution_dropdown.selected = p;
        }
        let ref_s = format!("{}", mon.refresh as u32);
        if let Some(p) = self.refresh_dropdown.options.iter().position(|o| o == &ref_s) {
            self.refresh_dropdown.selected = p;
        }
        self.scale_slider.value = mon.scale as f64;
        self.transform_dropdown.selected =
            (mon.transform as usize).min(TRANSFORMS.len() - 1);
        self.enabled_toggle.value = mon.enabled;
    }

    /// Read form widget values, write them to the selected monitor, then send IPC.
    fn apply_selected(&mut self) {
        let Some(idx) = self.selected else { return };

        // Collect values from widgets first (before mutably borrowing monitors).
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
        let scale     = self.scale_slider.value as f32;
        let transform = self.transform_dropdown.selected as u8;
        let enabled   = self.enabled_toggle.value;

        if let Some(mon) = self.monitors.get_mut(idx) {
            if let Some((w, h)) = parse_resolution(&res) {
                mon.width  = w;
                mon.height = h;
            }
            if let Ok(r) = ref_s.parse::<f32>() {
                mon.refresh = r;
            }
            mon.scale     = scale;
            mon.transform = transform;
            mon.enabled   = enabled;
        }

        if let Some(mon) = self.monitors.get(idx).cloned() {
            apply_monitor_ipc(&mon);
        }
    }

    /// Compute (scale, offset_x, offset_y) mapping monitor logical coords →
    /// visualization pixel coords so all monitors fit within `viz` with padding.
    fn viz_transform(&self, viz: &Rect) -> (f32, f32, f32) {
        if self.monitors.is_empty() {
            return (1.0, viz.x + VIZ_PAD, viz.y + VIZ_PAD);
        }
        let min_x =
            self.monitors.iter().map(|m| m.x).min().unwrap() as f32;
        let min_y =
            self.monitors.iter().map(|m| m.y).min().unwrap() as f32;
        let max_x =
            self.monitors.iter().map(|m| m.x + m.width as i32).max().unwrap() as f32;
        let max_y =
            self.monitors.iter().map(|m| m.y + m.height as i32).max().unwrap() as f32;

        let total_w = (max_x - min_x).max(1.0);
        let total_h = (max_y - min_y).max(1.0);
        let avail_w = viz.width  - 2.0 * VIZ_PAD;
        let avail_h = viz.height - 2.0 * VIZ_PAD;

        let scale = (avail_w / total_w).min(avail_h / total_h);
        let ox = viz.x + VIZ_PAD + (avail_w - total_w * scale) / 2.0 - min_x * scale;
        let oy = viz.y + VIZ_PAD + (avail_h - total_h * scale) / 2.0 - min_y * scale;
        (scale, ox, oy)
    }

    fn monitor_rect(mon: &MonitorInfo, scale: f32, ox: f32, oy: f32) -> Rect {
        Rect {
            x:      ox + mon.x as f32 * scale,
            y:      oy + mon.y as f32 * scale,
            width:  (mon.width  as f32 * scale).max(1.0),
            height: (mon.height as f32 * scale).max(1.0),
        }
    }

    /// Snap (nx, ny) for monitor `idx` to the edges of its neighbours.
    fn snap(&self, idx: usize, nx: i32, ny: i32) -> (i32, i32) {
        let mon = &self.monitors[idx];
        let (mut x, mut y) = (nx, ny);
        let s  = SNAP_PX as i32;
        let mw = mon.width  as i32;
        let mh = mon.height as i32;

        for (i, other) in self.monitors.iter().enumerate() {
            if i == idx { continue; }
            let ow = other.width  as i32;
            let oh = other.height as i32;

            // Horizontal
            if (x + mw - other.x).abs()       < s { x = other.x - mw; }
            if (x - (other.x + ow)).abs()      < s { x = other.x + ow; }
            if (x - other.x).abs()             < s { x = other.x; }

            // Vertical
            if (y + mh - other.y).abs()        < s { y = other.y - mh; }
            if (y - (other.y + oh)).abs()       < s { y = other.y + oh; }
            if (y - other.y).abs()              < s { y = other.y; }
        }
        (x, y)
    }

    /// Bounds of the form widget in row `row` within `bounds`.
    fn widget_rect(&self, bounds: Rect, row: usize) -> Rect {
        let row_top = bounds.y + FORM_TOP + row as f32 * (ROW_H + ROW_GAP);
        Rect {
            x:      bounds.x + bounds.width - WIDGET_W - H_PAD,
            y:      row_top + (ROW_H - 32.0) / 2.0,
            width:  WIDGET_W,
            height: 32.0,
        }
    }

    /// Bounds of the label text in form row `row` within `bounds`.
    fn label_rect(&self, bounds: Rect, row: usize, font_size: f32) -> Rect {
        let row_top = bounds.y + FORM_TOP + row as f32 * (ROW_H + ROW_GAP);
        Rect {
            x:      bounds.x + H_PAD,
            y:      row_top + (ROW_H - font_size * 1.3) / 2.0,
            width:  bounds.width * 0.45,
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
    fn title(&self) -> &str { "Display" }
    fn icon(&self) -> &str  { "preferences-desktop-display" }

    /// Rendering is fully custom; no form fields are exposed.
    fn fields(&self) -> Vec<PanelField> {
        Vec::new()
    }

    fn set_value(&mut self, key: &str, _value: &str) -> Result<String, PanelError> {
        Err(PanelError::UnknownKey(key.to_string()))
    }

    /// Return an empty form — layout and events are handled by `paint`/`event`.
    fn build_form(&self, content_width: f32) -> form::FormLayout {
        form::FormLayout::from_fields(Vec::new(), content_width)
    }

    fn measure(&self, constraints: Constraints) -> Size {
        Size { width: constraints.max_width, height: constraints.max_height }
    }

    fn paint(
        &self,
        bounds: Rect,
        pixmap: &mut tiny_skia::PixmapMut<'_>,
        ctx: &mut hyprcube_core::text::RenderContext<'_>,
    ) {
        let surface   = parse_hex(&ctx.palette.colors.surface);
        let overlay   = parse_hex(&ctx.palette.colors.overlay);
        let accent    = parse_hex(&ctx.palette.colors.accent);
        let fg        = parse_hex(&ctx.palette.colors.foreground);
        let bg        = parse_hex(&ctx.palette.colors.background);
        let font_size = ctx.palette.fonts.size as f32;

        let viz = Rect {
            x: bounds.x, y: bounds.y,
            width: bounds.width, height: VIZ_H,
        };

        // ── Visualization background ─────────────────────────────────────────
        fill_rounded_rect(pixmap, viz.x, viz.y, viz.width, viz.height, 8.0, surface);

        if self.monitors.is_empty() {
            let msg = "No monitors detected";
            let mx = viz.x + (viz.width - msg.len() as f32 * font_size * 0.55) / 2.0;
            let my = viz.y + (viz.height - font_size * 1.3) / 2.0;
            ctx.text.draw_text(pixmap, msg, mx, my, font_size, overlay, viz.width);
        } else {
            let (vs, vox, voy) = self.viz_transform(&viz);

            for (i, mon) in self.monitors.iter().enumerate() {
                let mr        = Self::monitor_rect(mon, vs, vox, voy);
                let is_sel    = self.selected == Some(i);
                let border_c  = if is_sel { accent } else { overlay };

                // Border (drawn as a 2 px outer ring using a slightly larger rect)
                fill_rounded_rect(
                    pixmap,
                    mr.x - 2.0, mr.y - 2.0,
                    mr.width + 4.0, mr.height + 4.0,
                    6.0, border_c,
                );
                // Monitor fill
                fill_rounded_rect(pixmap, mr.x, mr.y, mr.width, mr.height, 4.0, bg);

                // Labels: name on one line, resolution+refresh on the next
                let lfs = (mr.height / 4.5).clamp(8.0, 13.0);
                let lx  = mr.x + 4.0;
                let ly  = mr.y + mr.height / 2.0 - lfs * 1.4;
                ctx.text.draw_text(
                    pixmap, &mon.name, lx, ly, lfs, fg, mr.width - 8.0,
                );
                let res_str = format!("{}×{}@{}", mon.width, mon.height, mon.refresh as u32);
                ctx.text.draw_text(
                    pixmap, &res_str,
                    lx, ly + lfs * 1.5,
                    (lfs * 0.85).max(7.0), overlay, mr.width - 8.0,
                );
                // Scale indicator when ≠ 1.0
                if (mon.scale - 1.0).abs() > 0.01 {
                    let sc_str = format!("×{:.2}", mon.scale);
                    ctx.text.draw_text(
                        pixmap, &sc_str,
                        lx, ly + lfs * 3.0,
                        (lfs * 0.8).max(7.0), accent, mr.width - 8.0,
                    );
                }
            }
        }

        // ── Separator ────────────────────────────────────────────────────────
        fill_rounded_rect(
            pixmap,
            bounds.x, bounds.y + VIZ_H + 4.0,
            bounds.width, 1.0,
            0.0, overlay,
        );

        // ── Section header ───────────────────────────────────────────────────
        let header_y    = bounds.y + VIZ_H + 8.0;
        let header_text = match self.selected.and_then(|i| self.monitors.get(i)) {
            Some(mon) => format!("Monitor: {}", mon.name),
            None if self.monitors.is_empty() => {
                "Connect a monitor to configure".to_string()
            }
            None => "Click a monitor to select".to_string(),
        };
        ctx.text.draw_text(
            pixmap, &header_text,
            bounds.x + H_PAD, header_y,
            font_size + 2.0, fg,
            bounds.width - H_PAD * 2.0,
        );

        // ── Settings form (only when a monitor is selected) ──────────────────
        if self.selected.is_some() {
            let row_labels = ["Resolution", "Refresh Rate", "Scale", "Transform", "Enabled"];
            for (row, lbl) in row_labels.iter().enumerate() {
                let lr = self.label_rect(bounds, row, font_size);
                ctx.text.draw_text(pixmap, lbl, lr.x, lr.y, font_size, overlay, lr.width);
            }

            self.resolution_dropdown.paint(pixmap, self.widget_rect(bounds, 0), ctx);
            self.refresh_dropdown   .paint(pixmap, self.widget_rect(bounds, 1), ctx);
            self.scale_slider       .paint(pixmap, self.widget_rect(bounds, 2), ctx);
            self.transform_dropdown .paint(pixmap, self.widget_rect(bounds, 3), ctx);
            self.enabled_toggle     .paint(pixmap, self.widget_rect(bounds, 4), ctx);
        }
    }

    fn event(&mut self, event: &Event, bounds: Rect) -> EventResponse {
        let viz = Rect {
            x: bounds.x, y: bounds.y,
            width: bounds.width, height: VIZ_H,
        };
        let (vs, vox, voy) = self.viz_transform(&viz);

        match event {
            // ── Pointer down inside the visualization → select / start drag ──
            Event::PointerDown { x, y } if viz.contains(*x, *y) => {
                for (i, mon) in self.monitors.iter().enumerate() {
                    let mr = Self::monitor_rect(mon, vs, vox, voy);
                    if mr.contains(*x, *y) {
                        let was_sel = self.selected == Some(i);
                        self.selected  = Some(i);
                        self.drag_state = Some(DragState {
                            monitor_idx:     i,
                            down_x:          *x,
                            down_y:          *y,
                            monitor_start_x: mon.x,
                            monitor_start_y: mon.y,
                            viz_scale:       vs,
                        });
                        if !was_sel { self.sync_widgets(); }
                        return EventResponse { consumed: true, action: None };
                    }
                }
                EventResponse::ignored()
            }

            // ── Pointer up: finish drag or route to form widgets ─────────────
            Event::PointerUp { x, y } => {
                if let Some(drag) = self.drag_state.take() {
                    // Convert screen-pixel delta to logical-pixel delta.
                    let dx = (*x - drag.down_x) / drag.viz_scale;
                    let dy = (*y - drag.down_y) / drag.viz_scale;

                    if dx.abs() > 5.0 || dy.abs() > 5.0 {
                        let nx = drag.monitor_start_x + dx as i32;
                        let ny = drag.monitor_start_y + dy as i32;
                        let (sx, sy) = self.snap(drag.monitor_idx, nx, ny);

                        if let Some(mon) = self.monitors.get_mut(drag.monitor_idx) {
                            mon.x = sx;
                            mon.y = sy;
                        }
                        if let Some(mon) = self.monitors.get(drag.monitor_idx).cloned() {
                            apply_monitor_ipc(&mon);
                        }
                    }
                    return EventResponse { consumed: true, action: None };
                }

                self.route_to_form_widgets(event, bounds)
            }

            // ── Pointer down outside the visualization → form widgets ─────────
            Event::PointerDown { x, y } if !viz.contains(*x, *y) => {
                self.route_to_form_widgets(event, bounds)
            }

            _ => EventResponse::ignored(),
        }
    }
}

impl DisplayPanel {
    /// Route a pointer event to the correct form widget and apply changes on hit.
    fn route_to_form_widgets(&mut self, event: &Event, bounds: Rect) -> EventResponse {
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

        macro_rules! route {
            ($widget:expr, $r:expr) => {{
                let resp = $widget.event(event, &$r);
                if resp.consumed {
                    self.apply_selected();
                    return resp;
                }
            }};
        }

        route!(self.resolution_dropdown, rects[0]);
        route!(self.refresh_dropdown,    rects[1]);
        route!(self.scale_slider,        rects[2]);
        route!(self.transform_dropdown,  rects[3]);
        route!(self.enabled_toggle,      rects[4]);

        EventResponse::ignored()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn mon(name: &str, w: u32, h: u32, x: i32, y: i32) -> MonitorInfo {
        MonitorInfo {
            name: name.into(),
            width: w, height: h,
            refresh: 60.0,
            x, y,
            scale: 1.0,
            transform: 0,
            enabled: true,
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
        // Remove second monitor — selection 1 is now out of range.
        panel.set_monitors(vec![mon("A", 1920, 1080, 0, 0)]);
        assert_eq!(panel.selected, None);
    }

    #[test]
    fn config_line_format() {
        let m = mon("DP-1", 1920, 1080, 0, 0);
        let line = m.config_line();
        assert!(line.starts_with("monitor = DP-1,"));
        assert!(line.contains("1920x1080"));
    }

    #[test]
    fn snap_aligns_right_edge_to_neighbour() {
        let mut panel = DisplayPanel::new();
        panel.set_monitors(vec![
            mon("A", 1920, 1080,    0, 0),
            mon("B", 1920, 1080, 1920, 0),
        ]);
        // Move B to x=1914 (6 px from A's right edge of 1920) → should snap to 1920.
        let (sx, _) = panel.snap(1, 1914, 0);
        assert_eq!(sx, 1920);
    }

    #[test]
    fn snap_does_not_fire_beyond_threshold() {
        let mut panel = DisplayPanel::new();
        panel.set_monitors(vec![
            mon("A", 1920, 1080,    0, 0),
            mon("B", 1920, 1080, 1920, 0),
        ]);
        // Move B to x=2000 (80 px from A's right edge) → no snap.
        let (sx, _) = panel.snap(1, 2000, 0);
        assert_eq!(sx, 2000);
    }

    #[test]
    fn viz_transform_nonempty() {
        let mut panel = DisplayPanel::new();
        panel.set_monitors(vec![mon("M", 1920, 1080, 0, 0)]);
        let viz = Rect { x: 0.0, y: 0.0, width: 600.0, height: 300.0 };
        let (scale, ox, oy) = panel.viz_transform(&viz);
        assert!(scale > 0.0);
        assert!(ox >= VIZ_PAD);
        assert!(oy >= VIZ_PAD);
    }

    #[test]
    fn parse_resolution_ok() {
        assert_eq!(parse_resolution("1920x1080"), Some((1920, 1080)));
        assert_eq!(parse_resolution("3840x2160"), Some((3840, 2160)));
        assert_eq!(parse_resolution("bad"),       None);
    }

    #[test]
    fn set_value_always_errors() {
        let mut panel = DisplayPanel::new();
        assert!(panel.set_value("monitor.resolution", "1920x1080").is_err());
    }
}
