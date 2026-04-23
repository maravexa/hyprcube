use std::path::PathBuf;
use std::process::Command;

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

// ── SystemInfo ────────────────────────────────────────────────────────────────

/// Snapshot of system information collected once at panel creation.
pub struct SystemInfo {
    pub hyprcube_version: String,
    pub hyprland_version: String,
    pub kernel: String,
    pub hostname: String,
    pub uptime: String,
    pub compositor: String,
    pub display_server: String,
    pub cpu: String,
    pub memory: String,
    pub gpu: String,
}

impl SystemInfo {
    /// Collect all system information (best-effort; graceful fallbacks).
    pub fn collect() -> Self {
        Self {
            hyprcube_version: env!("CARGO_PKG_VERSION").to_string(),
            hyprland_version: hyprland_version(),
            kernel:           kernel_version(),
            hostname:         hostname(),
            uptime:           uptime(),
            compositor:       "Hyprland".to_string(),
            display_server:   "Wayland".to_string(),
            cpu:              cpu_model(),
            memory:           memory_info(),
            gpu:              gpu_info(),
        }
    }
}

// ── System-info helpers ───────────────────────────────────────────────────────

fn xdg_config_home() -> PathBuf {
    std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config")
        })
}

fn hyprland_version() -> String {
    let Ok(out) = Command::new("hyprctl").arg("version").output() else {
        return "not connected".to_string();
    };
    if !out.status.success() {
        return "not connected".to_string();
    }
    // First line is typically: "Hyprland v0.42.0 ..."
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .map(|v| v.trim_start_matches('v').to_string())
        .unwrap_or_else(|| "not connected".to_string())
}

fn kernel_version() -> String {
    // /proc/version: "Linux version 6.8.0-arch1 (...)  …"
    if let Ok(content) = std::fs::read_to_string("/proc/version") {
        if let Some(v) = content.split_whitespace().nth(2) {
            return v.to_string();
        }
    }
    if let Ok(out) = Command::new("uname").arg("-r").output() {
        if out.status.success() {
            return String::from_utf8_lossy(&out.stdout).trim().to_string();
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
    if let Ok(out) = Command::new("hostname").output() {
        if out.status.success() {
            return String::from_utf8_lossy(&out.stdout).trim().to_string();
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
    let days  = total / 86_400;
    let hours = (total % 86_400) / 3_600;
    let mins  = (total % 3_600)  / 60;
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
                if let Some(name) = line.splitn(2, ':').nth(1) {
                    return name.trim().to_string();
                }
            }
        }
    }
    "unknown".to_string()
}

fn memory_info() -> String {
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
            let used_gb  = (total_kb.saturating_sub(avail_kb)) as f64 / 1_048_576.0;
            let total_gb = total_kb as f64 / 1_048_576.0;
            return format!("{:.1} GB / {:.1} GB", used_gb, total_gb);
        }
    }
    "unknown".to_string()
}

fn gpu_info() -> String {
    // Primary: lspci
    if let Ok(out) = Command::new("lspci").output() {
        if out.status.success() {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                if line.contains("VGA compatible controller")
                    || line.contains("3D controller")
                    || line.contains("Display controller")
                {
                    if let Some(desc) = line.splitn(2, ": ").nth(1) {
                        return desc.trim().to_string();
                    }
                }
            }
        }
    }
    // Fallback: read driver name from sysfs
    if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
        for entry in entries.flatten() {
            let p    = entry.path();
            let name = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
            // card0, card1, … (no dash → top-level card node)
            if name.starts_with("card") && !name.contains('-') {
                if let Ok(content) = std::fs::read_to_string(p.join("device/uevent")) {
                    for line in content.lines() {
                        if let Some(driver) = line.strip_prefix("DRIVER=") {
                            return driver.to_string();
                        }
                    }
                }
            }
        }
    }
    "unknown".to_string()
}

/// Check whether a sibling app is present and return a short status string.
fn ecosystem_status(bin: &str, config_subdir: &str) -> String {
    let config_path = xdg_config_home().join(config_subdir).join("config.toml");
    if config_path.exists() {
        // Try to get version string from the binary.
        if let Ok(out) = Command::new(bin).arg("--version").output() {
            if out.status.success() {
                let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !v.is_empty() {
                    return format!("installed ({})", v);
                }
            }
        }
        "installed".to_string()
    } else {
        "not found".to_string()
    }
}

fn parse_hex(hex: &str) -> Color {
    Color::from_hex(hex).unwrap_or(Color::new(0.5, 0.5, 0.5, 1.0))
}

// ── AboutPanel ────────────────────────────────────────────────────────────────

pub struct AboutPanel {
    info: SystemInfo,
    hyprdeck_status: String,
    hyprsaver_status: String,
}

impl AboutPanel {
    pub fn new() -> Self {
        Self {
            info:             SystemInfo::collect(),
            hyprdeck_status:  ecosystem_status("hyprdeck",  "hyprdeck"),
            hyprsaver_status: ecosystem_status("hyprsaver", "hyprsaver"),
        }
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
    fn title(&self) -> &str { "About" }
    fn icon(&self) -> &str  { "help-about" }

    /// Rendering is fully custom; no form fields are exposed.
    fn fields(&self) -> Vec<PanelField> {
        Vec::new()
    }

    fn set_value(&mut self, key: &str, _value: &str) -> Result<String, PanelError> {
        Err(PanelError::UnknownKey(key.to_string()))
    }

    /// Return an empty form — this panel paints itself directly.
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
        let overlay   = parse_hex(&ctx.palette.colors.overlay);
        let accent    = parse_hex(&ctx.palette.colors.accent);
        let fg        = parse_hex(&ctx.palette.colors.foreground);
        let font_size = ctx.palette.fonts.size as f32;
        let title_fs  = font_size + 6.0;

        let x      = bounds.x + H_PAD;
        let avail_w = bounds.width - H_PAD * 2.0;
        let mut cy = bounds.y + V_PAD;

        // ── Title ─────────────────────────────────────────────────────────────
        // "HyprCube" in large accent text
        ctx.text.draw_text(pixmap, "HyprCube", x, cy, title_fs, accent, avail_w * 0.55);
        // Version string, inlined to the right of the title
        let ver_str = format!("v{}", self.info.hyprcube_version);
        // Approximate the title's glyph width to place the version right after it.
        let ver_x = x + title_fs * 5.0;
        ctx.text.draw_text(pixmap, &ver_str, ver_x, cy, title_fs, accent, avail_w * 0.4);
        cy += title_fs * 1.6;

        // ── Subtitle ─────────────────────────────────────────────────────────
        ctx.text.draw_text(
            pixmap, "Unified Settings for Hyprland",
            x, cy, font_size, overlay, avail_w,
        );
        cy += font_size * 1.5 + SECTION_GAP;

        // ── Separator ─────────────────────────────────────────────────────────
        fill_rounded_rect(pixmap, x, cy, avail_w, 1.0, 0.0, overlay);
        cy += 1.0 + SECTION_GAP;

        // ── System info rows ──────────────────────────────────────────────────
        let rows: &[(&str, &str)] = &[
            ("Hyprland",       &self.info.hyprland_version),
            ("Kernel",         &self.info.kernel),
            ("Hostname",       &self.info.hostname),
            ("Uptime",         &self.info.uptime),
            ("CPU",            &self.info.cpu),
            ("Memory",         &self.info.memory),
            ("GPU",            &self.info.gpu),
            ("Compositor",     &self.info.compositor),
            ("Display server", &self.info.display_server),
        ];

        let val_x = x + LABEL_W;
        let val_w = (avail_w - LABEL_W).max(0.0);
        for (label, value) in rows {
            ctx.text.draw_text(pixmap, label, x,     cy, font_size, overlay, LABEL_W);
            ctx.text.draw_text(pixmap, value, val_x, cy, font_size, fg,      val_w);
            cy += ROW_H + ROW_GAP;
        }

        cy += SECTION_GAP;

        // ── Separator ─────────────────────────────────────────────────────────
        fill_rounded_rect(pixmap, x, cy, avail_w, 1.0, 0.0, overlay);
        cy += 1.0 + SECTION_GAP;

        // ── Ecosystem section ─────────────────────────────────────────────────
        ctx.text.draw_text(
            pixmap, "Ecosystem",
            x, cy, font_size + 2.0, fg, avail_w,
        );
        cy += (font_size + 2.0) * 1.6 + ROW_GAP;

        let eco: &[(&str, &str)] = &[
            ("HyprDeck",  &self.hyprdeck_status),
            ("HyprSaver", &self.hyprsaver_status),
        ];
        for (label, value) in eco {
            ctx.text.draw_text(pixmap, label, x,     cy, font_size, overlay, LABEL_W);
            ctx.text.draw_text(pixmap, value, val_x, cy, font_size, fg,      val_w);
            cy += ROW_H + ROW_GAP;
        }
    }

    fn event(&mut self, _event: &Event, _bounds: Rect) -> EventResponse {
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
    fn system_info_display_server_is_wayland() {
        let info = SystemInfo::collect();
        assert_eq!(info.display_server, "Wayland");
    }

    #[test]
    fn system_info_compositor_is_hyprland() {
        let info = SystemInfo::collect();
        assert_eq!(info.compositor, "Hyprland");
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
        // 1 day + 2 h + 3 min = 93780 s
        let s = format_uptime(93_780.0);
        assert!(s.contains('d'), "expected days in '{s}'");
        assert!(s.contains('h'), "expected hours in '{s}'");
    }

    #[test]
    fn format_uptime_hours_only() {
        // 2 h 5 min = 7500 s
        let s = format_uptime(7_500.0);
        assert!(!s.contains('d'), "unexpected days in '{s}'");
        assert!(s.contains('h'), "expected hours in '{s}'");
    }

    #[test]
    fn memory_info_format() {
        let m = memory_info();
        // Either "X.Y GB / Z.W GB" or "unknown"
        assert!(m.contains("GB") || m == "unknown", "unexpected memory string: '{m}'");
    }

    #[test]
    fn kernel_version_not_empty() {
        // /proc/version or uname -r should always succeed in CI.
        let k = kernel_version();
        assert!(!k.is_empty());
    }
}
