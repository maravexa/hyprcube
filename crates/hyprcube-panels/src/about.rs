use hyprcube_core::color::Color;
use hyprcube_core::layout::Rect;
use hyprcube_core::render::fill_rounded_rect;
use hyprcube_core::widget::{Constraints, Event, EventResponse, Size};

use crate::{form, PanelError, PanelField, SettingsPanel};

// ── Layout constants ─────────────────────────────────────────────────────────

const H_PAD: f32 = 24.0;
const V_PAD: f32 = 24.0;
const ROW_H: f32 = 26.0;
const ROW_GAP: f32 = 6.0;
const SECTION_GAP: f32 = 16.0;
/// Fixed width of the left (label) column inside the info card.
const LABEL_W: f32 = 140.0;

// ── AppStatus ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum AppStatus {
    Installed,
    NotFound,
}

impl AppStatus {
    fn as_str(&self) -> &'static str {
        match self {
            AppStatus::Installed => "installed",
            AppStatus::NotFound => "not found",
        }
    }
}

// ── Sync IPC helpers (no external processes) ──────────────────────────────────

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

// ── SystemInfo ────────────────────────────────────────────────────────────────

/// Snapshot of system information collected once at panel creation (or refresh).
pub struct SystemInfo {
    pub hyprcube_version: String,
    pub hyprland_version: Option<String>,
    pub kernel: String,
    pub hostname: String,
    pub uptime: String,
    pub cpu: String,
    pub memory_used: String,
    pub memory_total: String,
    pub gpu: String,
    pub session_type: String,
}

impl SystemInfo {
    /// Collect all system information (best-effort; graceful fallbacks).
    pub fn collect() -> Self {
        let (memory_used, memory_total) = memory_info();
        Self {
            hyprcube_version: env!("CARGO_PKG_VERSION").to_string(),
            hyprland_version: hyprland_version(),
            kernel: kernel_version(),
            hostname: hostname(),
            uptime: uptime(),
            cpu: cpu_model(),
            memory_used,
            memory_total,
            gpu: gpu_info(),
            session_type: session_type(),
        }
    }
}

// ── System-info helpers (no Command::new) ─────────────────────────────────────

fn hyprland_version() -> Option<String> {
    let json = ipc_query_json("version")?;
    // Hyprland returns { "tag": "v0.42.0", ... }
    let tag = json["tag"].as_str()?;
    Some(tag.trim_start_matches('v').to_string())
}

fn kernel_version() -> String {
    // /proc/version: "Linux version 6.8.0-arch1 (builder@…) …"
    if let Ok(content) = std::fs::read_to_string("/proc/version") {
        if let Some(v) = content.split_whitespace().nth(2) {
            return v.to_string();
        }
    }
    "unknown".to_string()
}

fn hostname() -> String {
    if let Ok(h) = std::fs::read_to_string("/etc/hostname") {
        let h = h.trim().to_string();
        if !h.is_empty() {
            return h;
        }
    }
    "unknown".to_string()
}

fn uptime() -> String {
    // /proc/uptime: "<total_seconds> <idle_seconds>"
    if let Ok(content) = std::fs::read_to_string("/proc/uptime") {
        if let Some(s) = content.split_whitespace().next() {
            if let Ok(secs) = s.parse::<f64>() {
                return format_uptime(secs);
            }
        }
    }
    "unknown".to_string()
}

/// Format a seconds value as "Xd Xh Xm" (or "Xh Xm" when < 1 day).
fn format_uptime(secs: f64) -> String {
    let total = secs as u64;
    let days = total / 86_400;
    let hours = (total % 86_400) / 3_600;
    let mins = (total % 3_600) / 60;
    if days > 0 {
        format!("{}d {}h {}m", days, hours, mins)
    } else {
        format!("{}h {}m", hours, mins)
    }
}

fn cpu_model() -> String {
    if let Ok(content) = std::fs::read_to_string("/proc/cpuinfo") {
        for line in content.lines() {
            if line.starts_with("model name") {
                if let Some((_, name)) = line.split_once(':') {
                    return name.trim().to_string();
                }
            }
        }
    }
    "unknown".to_string()
}

/// Returns (used_gb, total_gb) formatted strings.
fn memory_info() -> (String, String) {
    let mut total_kb = 0u64;
    let mut avail_kb = 0u64;

    if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                if let Some(n) = line.split_whitespace().nth(1) {
                    total_kb = n.parse().unwrap_or(0);
                }
            } else if line.starts_with("MemAvailable:") {
                if let Some(n) = line.split_whitespace().nth(1) {
                    avail_kb = n.parse().unwrap_or(0);
                }
            }
        }
        if total_kb > 0 {
            let used_gb = (total_kb.saturating_sub(avail_kb)) as f64 / 1_048_576.0;
            let total_gb = total_kb as f64 / 1_048_576.0;
            return (format!("{:.1} GB", used_gb), format!("{:.1} GB", total_gb));
        }
    }
    ("unknown".to_string(), "unknown".to_string())
}

fn gpu_info() -> String {
    // Try sysfs first: /sys/class/drm/card*/device/uevent
    if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
        let mut names: Vec<String> = entries
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                let name = p.file_name()?.to_string_lossy().into_owned();
                // card0, card1, … (no dash → top-level card node)
                if !name.starts_with("card") || name.contains('-') {
                    return None;
                }
                // Try device name from uevent
                let uevent = std::fs::read_to_string(p.join("device/uevent")).ok()?;
                for line in uevent.lines() {
                    if let Some(val) = line.strip_prefix("DRIVER=") {
                        return Some(val.to_string());
                    }
                }
                None
            })
            .collect();
        names.dedup();
        if !names.is_empty() {
            return names.join(", ");
        }
    }
    "unknown".to_string()
}

fn session_type() -> String {
    if let Ok(t) = std::env::var("XDG_SESSION_TYPE") {
        if !t.is_empty() {
            return t;
        }
    }
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        return "Wayland".to_string();
    }
    "unknown".to_string()
}

/// Check whether a sibling app config file exists.
fn ecosystem_status(config_filename: &str) -> AppStatus {
    let config_path = hyprcube_core::hypr_config_dir().join(config_filename);
    if config_path.is_file() {
        AppStatus::Installed
    } else {
        AppStatus::NotFound
    }
}

fn parse_hex(hex: &str) -> Color {
    Color::from_hex(hex).unwrap_or(Color::new(0.5, 0.5, 0.5, 1.0))
}

// ── AboutPanel ────────────────────────────────────────────────────────────────

pub struct AboutPanel {
    info: SystemInfo,
    hyprdeck_status: AppStatus,
    hyprsaver_status: AppStatus,
    /// Screen-space bounds of the "↻ Refresh" button (updated each paint).
    refresh_btn: std::cell::Cell<Rect>,
}

impl AboutPanel {
    pub fn new() -> Self {
        Self {
            info: SystemInfo::collect(),
            hyprdeck_status: ecosystem_status("hyprdeck.toml"),
            hyprsaver_status: ecosystem_status("hyprsaver.toml"),
            refresh_btn: std::cell::Cell::new(Rect::default()),
        }
    }

    /// Re-collect all system info (e.g. uptime, memory).
    fn refresh(&mut self) {
        self.info = SystemInfo::collect();
        self.hyprdeck_status = ecosystem_status("hyprdeck.toml");
        self.hyprsaver_status = ecosystem_status("hyprsaver.toml");
    }

    /// Access the collected system info.
    pub fn system_info(&self) -> &SystemInfo {
        &self.info
    }
}

impl Default for AboutPanel {
    fn default() -> Self {
        Self::new()
    }
}

// ── SettingsPanel impl ────────────────────────────────────────────────────────

impl SettingsPanel for AboutPanel {
    fn title(&self) -> &str {
        "About"
    }
    fn icon(&self) -> &str {
        "help-about"
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
        let overlay = parse_hex(&ctx.palette.colors.overlay);
        let accent = parse_hex(&ctx.palette.colors.accent);
        let fg = parse_hex(&ctx.palette.colors.foreground);
        let font_size = ctx.palette.fonts.size as f32;
        let title_fs = font_size + 8.0;

        let x = bounds.x + H_PAD;
        let avail_w = bounds.width - H_PAD * 2.0;
        let mut cy = bounds.y + V_PAD;

        // ── Title ─────────────────────────────────────────────────────────────
        ctx.text
            .draw_text(pixmap, "HyprCube", x, cy, title_fs, accent, avail_w * 0.55);
        let ver_str = format!("v{}", self.info.hyprcube_version);
        let ver_x = x + title_fs * 5.2;
        ctx.text.draw_text(
            pixmap,
            &ver_str,
            ver_x,
            cy + (title_fs - font_size) * 0.5,
            font_size,
            overlay,
            avail_w * 0.4,
        );
        cy += title_fs * 1.6;

        // ── Subtitle ─────────────────────────────────────────────────────────
        ctx.text.draw_text(
            pixmap,
            "Unified Settings for Hyprland",
            x,
            cy,
            font_size,
            overlay,
            avail_w,
        );
        cy += font_size * 1.5 + SECTION_GAP;

        // ── Refresh button (top-right corner) ─────────────────────────────────
        let btn_text = "\u{21BB} Refresh";
        let btn_w = font_size * 5.5;
        let btn_x = bounds.x + bounds.width - H_PAD - btn_w;
        let btn_y = bounds.y + V_PAD;
        let btn_rect = Rect {
            x: btn_x,
            y: btn_y,
            width: btn_w,
            height: font_size * 1.6,
        };
        self.refresh_btn.set(btn_rect);
        ctx.text.draw_text(
            pixmap,
            btn_text,
            btn_x,
            btn_y,
            font_size,
            overlay,
            btn_w + 4.0,
        );

        // ── Separator ─────────────────────────────────────────────────────────
        fill_rounded_rect(pixmap, x, cy, avail_w, 1.0, 0.0, overlay);
        cy += 1.0 + SECTION_GAP;

        // ── System info rows ──────────────────────────────────────────────────
        let hyprland_val = self
            .info
            .hyprland_version
            .as_deref()
            .unwrap_or("not connected");
        let memory_val = if self.info.memory_used != "unknown" {
            format!("{} / {}", self.info.memory_used, self.info.memory_total)
        } else {
            "unknown".to_string()
        };

        let rows: &[(&str, &str)] = &[
            ("Hyprland", hyprland_val),
            ("Kernel", &self.info.kernel),
            ("Hostname", &self.info.hostname),
            ("Uptime", &self.info.uptime),
            ("CPU", &self.info.cpu),
            ("Memory", &memory_val),
            ("GPU", &self.info.gpu),
            ("Session", &self.info.session_type),
        ];

        let val_x = x + LABEL_W;
        let val_w = (avail_w - LABEL_W).max(0.0);
        for (label, value) in rows {
            if *value == "unknown" {
                continue;
            }
            ctx.text
                .draw_text(pixmap, label, x, cy, font_size, overlay, LABEL_W);
            ctx.text
                .draw_text(pixmap, value, val_x, cy, font_size, fg, val_w);
            cy += ROW_H + ROW_GAP;
        }

        cy += SECTION_GAP;

        // ── Separator ─────────────────────────────────────────────────────────
        fill_rounded_rect(pixmap, x, cy, avail_w, 1.0, 0.0, overlay);
        cy += 1.0 + SECTION_GAP;

        // ── Ecosystem section ─────────────────────────────────────────────────
        ctx.text
            .draw_text(pixmap, "Ecosystem", x, cy, font_size + 2.0, fg, avail_w);
        cy += (font_size + 2.0) * 1.6 + ROW_GAP;

        let eco: &[(&str, &AppStatus)] = &[
            ("HyprDeck", &self.hyprdeck_status),
            ("HyprSaver", &self.hyprsaver_status),
        ];
        for (label, status) in eco {
            let val_color = if **status == AppStatus::Installed {
                accent
            } else {
                overlay
            };
            ctx.text
                .draw_text(pixmap, label, x, cy, font_size, overlay, LABEL_W);
            ctx.text.draw_text(
                pixmap,
                status.as_str(),
                val_x,
                cy,
                font_size,
                val_color,
                val_w,
            );
            cy += ROW_H + ROW_GAP;
        }

        cy += SECTION_GAP;
        fill_rounded_rect(pixmap, x, cy, avail_w, 1.0, 0.0, overlay);
        cy += 1.0 + SECTION_GAP;

        // ── Project and attribution section ─────────────────────────────────
        ctx.text.draw_text(
            pixmap,
            "Project & Licenses",
            x,
            cy,
            font_size + 2.0,
            fg,
            avail_w,
        );
        cy += (font_size + 2.0) * 1.6;
        let notices = [
            "HyprCube — MIT License — github.com/maravexa/hyprcube",
            "HyprDeck and HyprSaver — MIT License",
            "Hyprland and Hyprpaper — independent Hypr ecosystem projects",
            "Built with Rust, Smithay Client Toolkit, tiny-skia, and cosmic-text",
            "Complete dependency license texts are distributed with their source packages.",
        ];
        for notice in notices {
            ctx.text.draw_text(
                pixmap,
                notice,
                x,
                cy,
                (font_size - 1.0).max(9.0),
                overlay,
                avail_w,
            );
            cy += (font_size * 1.35).max(18.0);
        }
    }

    fn event(&mut self, event: &Event, _bounds: Rect) -> EventResponse {
        if let Event::PointerUp { x, y } = event {
            if self.refresh_btn.get().contains(*x, *y) {
                self.refresh();
                return EventResponse {
                    consumed: true,
                    action: None,
                };
            }
        }
        EventResponse::ignored()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_info_has_version() {
        let info = SystemInfo::collect();
        assert_eq!(info.hyprcube_version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn fields_empty_for_custom_panel() {
        let panel = AboutPanel::new();
        assert!(panel.fields().is_empty());
    }

    #[test]
    fn set_value_always_errors() {
        let mut panel = AboutPanel::new();
        assert!(panel.set_value("anything", "val").is_err());
    }

    #[test]
    fn format_uptime_with_days() {
        let s = format_uptime(93_780.0);
        assert!(s.contains('d'), "expected days in '{s}'");
        assert!(s.contains('h'), "expected hours in '{s}'");
    }

    #[test]
    fn format_uptime_hours_only() {
        let s = format_uptime(7_500.0);
        assert!(!s.contains('d'), "unexpected days in '{s}'");
        assert!(s.contains('h'), "expected hours in '{s}'");
    }

    #[test]
    fn memory_info_format() {
        let (used, total) = memory_info();
        assert!(
            used.contains("GB") || used == "unknown",
            "unexpected: '{used}'"
        );
        assert!(
            total.contains("GB") || total == "unknown",
            "unexpected: '{total}'"
        );
    }

    #[test]
    fn kernel_version_not_empty() {
        let k = kernel_version();
        assert!(!k.is_empty());
    }

    #[test]
    fn app_status_strings() {
        assert_eq!(AppStatus::Installed.as_str(), "installed");
        assert_eq!(AppStatus::NotFound.as_str(), "not found");
    }
}
