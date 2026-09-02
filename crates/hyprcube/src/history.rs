//! Git-backed configuration history and its custom settings panel.

use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use hyprcube_core::color::Color;
use hyprcube_core::layout::Rect;
use hyprcube_core::render::fill_rounded_rect;
use hyprcube_core::text::RenderContext;
use hyprcube_core::widget::{Constraints, Event, EventResponse, Size};
use hyprcube_panels::{ExternalConfigChange, PanelError, PanelField, SettingsPanel};

const MANAGED_FILES: &[&str] = &[
    "hyprland.conf",
    "hyprcube-palette.toml",
    "hyprcube.toml",
    "hyprdeck.toml",
    "hyprsaver.toml",
    "hyprpaper.conf",
    "hyprpaper-hyprcube.conf",
    "hyprpaper-hyprcube-theme.conf",
];
const MAX_REVISIONS: usize = 100;
const REVISION_ROW_HEIGHT: f32 = 38.0;

#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("git is required for configuration history: {0}")]
    GitUnavailable(std::io::Error),
    #[error("git command failed: {0}")]
    Git(String),
    #[error("configuration history I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid history revision `{0}`")]
    InvalidRevision(String),
    #[error("snapshot name must contain a letter or number")]
    InvalidSnapshotName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revision {
    pub hash: String,
    pub short_hash: String,
    pub timestamp: u64,
    pub summary: String,
    pub snapshots: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct HistoryStore {
    repo: PathBuf,
    config_dir: PathBuf,
}

impl HistoryStore {
    pub fn open_default() -> Result<Self, HistoryError> {
        let state_home = std::env::var_os("XDG_STATE_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .filter(|value| !value.is_empty())
                    .map(|home| PathBuf::from(home).join(".local/state"))
            })
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        Self::open(
            state_home.join("hyprcube/config-history"),
            hyprcube_core::hypr_config_dir(),
        )
    }

    pub fn open(repo: PathBuf, config_dir: PathBuf) -> Result<Self, HistoryError> {
        Command::new("git")
            .arg("--version")
            .output()
            .map_err(HistoryError::GitUnavailable)?;
        fs::create_dir_all(&repo)?;
        fs::set_permissions(&repo, fs::Permissions::from_mode(0o700))?;
        let store = Self { repo, config_dir };
        if !store.repo.join(".git").is_dir() {
            store.run_checked(["init", "--initial-branch=main"])?;
            store.run_checked(["config", "user.name", "HyprCube configuration history"])?;
            store.run_checked(["config", "user.email", "history@hyprcube.local"])?;
            store.run_checked(["config", "core.autocrlf", "false"])?;
        }
        Ok(store)
    }

    pub fn capture(&self, message: &str) -> Result<Option<String>, HistoryError> {
        self.sync_from_live_config()?;
        self.run_checked(["add", "-A", "--", "configs"])?;

        let has_head = self.git_success(["rev-parse", "--verify", "HEAD"])?;
        if has_head && self.git_success(["diff", "--cached", "--quiet"])? {
            return Ok(None);
        }

        let message = message.replace(['\n', '\r'], " ");
        let mut args = vec!["commit", "--quiet", "-m", message.trim()];
        if !has_head {
            args.push("--allow-empty");
        }
        self.run_checked(args)?;
        let hash = self.run_checked(["rev-parse", "HEAD"])?;
        Ok(Some(
            String::from_utf8_lossy(&hash.stdout).trim().to_owned(),
        ))
    }

    pub fn revisions(&self) -> Result<Vec<Revision>, HistoryError> {
        if !self.git_success(["rev-parse", "--verify", "HEAD"])? {
            return Ok(Vec::new());
        }
        let output = self.run_checked([
            "log",
            "--first-parent",
            &format!("--max-count={MAX_REVISIONS}"),
            "--format=%H%x1f%h%x1f%ct%x1f%s",
        ])?;
        let snapshots = self.snapshot_refs()?;
        let mut revisions = Vec::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let mut fields = line.splitn(4, '\x1f');
            let Some(hash) = fields.next() else { continue };
            let Some(short_hash) = fields.next() else {
                continue;
            };
            let timestamp = fields
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0);
            let summary = fields.next().unwrap_or_default().to_owned();
            let revision_snapshots = snapshots
                .iter()
                .filter(|(_, target)| target == hash)
                .map(|(name, _)| name.clone())
                .collect();
            revisions.push(Revision {
                hash: hash.to_owned(),
                short_hash: short_hash.to_owned(),
                timestamp,
                summary,
                snapshots: revision_snapshots,
            });
        }
        Ok(revisions)
    }

    pub fn diff(&self, revision: &str, section: &str) -> Result<String, HistoryError> {
        validate_revision(revision)?;
        let mut args = vec![
            "diff",
            "--no-ext-diff",
            "--unified=3",
            "HEAD",
            revision,
            "--",
        ];
        args.extend(section_paths(section));
        let output = self.run_checked(args)?;
        let text = String::from_utf8_lossy(&output.stdout).into_owned();
        Ok(if text.trim().is_empty() {
            "No differences between the selected and current configuration in this section.".into()
        } else {
            text
        })
    }

    pub fn create_snapshot(&self, name: &str, revision: &str) -> Result<String, HistoryError> {
        validate_revision(revision)?;
        let slug = snapshot_slug(name).ok_or(HistoryError::InvalidSnapshotName)?;
        self.run_checked(["tag", &format!("snapshot/{slug}"), revision])?;
        Ok(slug)
    }

    pub fn restore(&self, revision: &str) -> Result<String, HistoryError> {
        validate_revision(revision)?;
        self.capture("Configuration before history restore")?;

        let desired = MANAGED_FILES
            .iter()
            .map(|name| Ok(((*name).to_owned(), self.file_at_revision(revision, name)?)))
            .collect::<Result<Vec<_>, HistoryError>>()?;
        let original = MANAGED_FILES
            .iter()
            .map(|name| {
                let path = self.config_dir.join(name);
                Ok(((*name).to_owned(), fs::read(path).ok()))
            })
            .collect::<Result<Vec<_>, std::io::Error>>()?;

        if let Err(error) = apply_file_set(&self.config_dir, &desired) {
            let _ = apply_file_set(&self.config_dir, &original);
            return Err(error.into());
        }

        let short = revision.chars().take(12).collect::<String>();
        self.capture(&format!("Restore configuration {short}"))?;
        Ok(short)
    }

    fn sync_from_live_config(&self) -> Result<(), HistoryError> {
        let mirror = self.repo.join("configs");
        fs::create_dir_all(&mirror)?;
        for name in MANAGED_FILES {
            let source = self.config_dir.join(name);
            let destination = mirror.join(name);
            if source.is_file() {
                fs::copy(source, destination)?;
            } else if destination.exists() {
                fs::remove_file(destination)?;
            }
        }
        Ok(())
    }

    fn file_at_revision(
        &self,
        revision: &str,
        name: &str,
    ) -> Result<Option<Vec<u8>>, HistoryError> {
        let object = format!("{revision}:configs/{name}");
        if !self.git_success(["cat-file", "-e", &object])? {
            return Ok(None);
        }
        Ok(Some(self.run_checked(["show", &object])?.stdout))
    }

    fn snapshot_refs(&self) -> Result<Vec<(String, String)>, HistoryError> {
        let output = self.run_checked([
            "for-each-ref",
            "--format=%(refname:strip=3)%00%(objectname)",
            "refs/tags/snapshot/",
        ])?;
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| {
                let (name, hash) = line.split_once('\0')?;
                Some((name.to_owned(), hash.to_owned()))
            })
            .collect())
    }

    fn run_checked<I, S>(&self, args: I) -> Result<Output, HistoryError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repo)
            .output()
            .map_err(HistoryError::GitUnavailable)?;
        if output.status.success() {
            Ok(output)
        } else {
            Err(HistoryError::Git(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ))
        }
    }

    fn git_success<I, S>(&self, args: I) -> Result<bool, HistoryError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repo)
            .output()
            .map_err(HistoryError::GitUnavailable)?;
        Ok(output.status.success())
    }
}

fn validate_revision(revision: &str) -> Result<(), HistoryError> {
    if revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(HistoryError::InvalidRevision(revision.into()))
    }
}

fn snapshot_slug(name: &str) -> Option<String> {
    let mut slug = String::new();
    let mut separator = false;
    for character in name.trim().chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            separator = false;
        } else if !separator && !slug.is_empty() {
            slug.push('-');
            separator = true;
        }
        if slug.len() >= 48 {
            break;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    (!slug.is_empty()).then_some(slug)
}

fn section_paths(section: &str) -> Vec<&'static str> {
    match section {
        "Hyprland" => vec!["configs/hyprland.conf"],
        "Appearance" => vec!["configs/hyprcube-palette.toml"],
        "HyprDeck" => vec!["configs/hyprdeck.toml"],
        "HyprSaver" => vec!["configs/hyprsaver.toml"],
        "Hyprpaper" => vec![
            "configs/hyprpaper.conf",
            "configs/hyprpaper-hyprcube.conf",
            "configs/hyprpaper-hyprcube-theme.conf",
        ],
        "HyprCube" => vec!["configs/hyprcube.toml"],
        _ => vec!["configs"],
    }
}

fn apply_file_set(config_dir: &Path, files: &[(String, Option<Vec<u8>>)]) -> std::io::Result<()> {
    fs::create_dir_all(config_dir)?;
    for (name, contents) in files {
        let path = config_dir.join(name);
        match contents {
            Some(contents) => atomic_write(&path, contents)?,
            None if path.exists() => fs::remove_file(path)?,
            None => {}
        }
    }
    Ok(())
}

fn atomic_write(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = path.file_name().and_then(OsStr::to_str).unwrap_or("config");
    let temp = path.with_file_name(format!(".{name}.history-{}-{stamp}", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        if let Ok(metadata) = fs::metadata(path) {
            file.set_permissions(metadata.permissions())?;
        }
        file.write_all(contents)?;
        file.sync_all()?;
        fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

pub struct HistoryPanel {
    store: Option<HistoryStore>,
    revisions: Vec<Revision>,
    selected: usize,
    filter: String,
    diff: String,
    diff_scroll: usize,
    snapshot_name: String,
    snapshot_focused: bool,
    status: String,
    external_change: Option<ExternalConfigChange>,
}

impl HistoryPanel {
    pub fn new(store: Option<HistoryStore>, error: Option<String>) -> Self {
        let mut panel = Self {
            store,
            revisions: Vec::new(),
            selected: 0,
            filter: "All".into(),
            diff: String::new(),
            diff_scroll: 0,
            snapshot_name: String::new(),
            snapshot_focused: false,
            status: error.unwrap_or_default(),
            external_change: None,
        };
        panel.refresh();
        panel
    }

    fn refresh(&mut self) {
        let selected_hash = self
            .revisions
            .get(self.selected)
            .map(|revision| revision.hash.clone());
        let Some(store) = &self.store else { return };
        match store.revisions() {
            Ok(revisions) => {
                self.revisions = revisions;
                self.selected = selected_hash
                    .and_then(|hash| {
                        self.revisions
                            .iter()
                            .position(|revision| revision.hash == hash)
                    })
                    .unwrap_or(0)
                    .min(self.revisions.len().saturating_sub(1));
                self.refresh_diff();
            }
            Err(error) => self.status = error.to_string(),
        }
    }

    fn refresh_diff(&mut self) {
        self.diff_scroll = 0;
        let Some(store) = &self.store else { return };
        let Some(revision) = self.revisions.get(self.selected) else {
            self.diff = "No configuration versions have been recorded yet.".into();
            return;
        };
        match store.diff(&revision.hash, &self.filter) {
            Ok(diff) => self.diff = diff,
            Err(error) => self.status = error.to_string(),
        }
    }

    fn restore_selected(&mut self) {
        let Some(store) = &self.store else { return };
        let Some(revision) = self.revisions.get(self.selected) else {
            return;
        };
        match store.restore(&revision.hash) {
            Ok(short) => {
                self.status = format!(
                    "Restored {short}; HyprDeck, HyprSaver, and Hyprpaper may need refresh."
                );
                self.external_change = Some(ExternalConfigChange {
                    message: self.status.clone(),
                    reload_hyprland: true,
                });
                self.refresh();
            }
            Err(error) => self.status = format!("Restore failed: {error}"),
        }
    }

    fn snapshot_selected(&mut self) {
        let Some(store) = &self.store else { return };
        let Some(revision) = self.revisions.get(self.selected) else {
            return;
        };
        match store.create_snapshot(&self.snapshot_name, &revision.hash) {
            Ok(name) => {
                self.status = format!("Saved snapshot “{name}”.");
                self.snapshot_name.clear();
                self.snapshot_focused = false;
                self.refresh();
            }
            Err(error) => self.status = format!("Snapshot failed: {error}"),
        }
    }

    fn controls_y(bounds: Rect) -> f32 {
        bounds.y + 42.0
    }
    fn snapshot_y(bounds: Rect) -> f32 {
        bounds.y + 84.0
    }
    fn filters_y(bounds: Rect) -> f32 {
        bounds.y + 126.0
    }
    fn content_y(bounds: Rect) -> f32 {
        bounds.y + 172.0
    }

    fn button_rect(bounds: Rect, index: usize) -> Rect {
        Rect {
            x: bounds.x + index as f32 * 112.0,
            y: Self::controls_y(bounds),
            width: 104.0,
            height: 30.0,
        }
    }

    fn snapshot_input_rect(bounds: Rect) -> Rect {
        Rect {
            x: bounds.x,
            y: Self::snapshot_y(bounds),
            width: 230.0,
            height: 30.0,
        }
    }

    fn snapshot_button_rect(bounds: Rect) -> Rect {
        Rect {
            x: bounds.x + 238.0,
            y: Self::snapshot_y(bounds),
            width: 130.0,
            height: 30.0,
        }
    }

    fn filter_rect(bounds: Rect, index: usize) -> Rect {
        let width = ((bounds.width - 6.0 * 6.0) / 7.0).max(58.0);
        Rect {
            x: bounds.x + index as f32 * (width + 6.0),
            y: Self::filters_y(bounds),
            width,
            height: 28.0,
        }
    }

    fn revision_list_rect(bounds: Rect) -> Rect {
        Rect {
            x: bounds.x,
            y: Self::content_y(bounds),
            width: (bounds.width * 0.34).clamp(210.0, 290.0),
            height: (bounds.y + bounds.height - Self::content_y(bounds)).max(0.0),
        }
    }
}

impl SettingsPanel for HistoryPanel {
    fn title(&self) -> &str {
        "History"
    }
    fn icon(&self) -> &str {
        "document-open-recent"
    }
    fn fields(&self) -> Vec<PanelField> {
        Vec::new()
    }
    fn set_value(&mut self, key: &str, _value: &str) -> Result<String, PanelError> {
        Err(PanelError::UnknownKey(key.into()))
    }
    fn uses_custom_layout(&self) -> bool {
        true
    }
    fn requires_clean_state(&self) -> bool {
        true
    }
    fn take_external_config_change(&mut self) -> Option<ExternalConfigChange> {
        self.external_change.take()
    }
    fn refresh_external_state(&mut self) {
        self.refresh();
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
        ctx: &mut RenderContext<'_>,
    ) {
        let fg = Color::from_hex(&ctx.palette.colors.foreground)
            .unwrap_or(Color::new(0.8, 0.84, 0.96, 1.0));
        let surface = Color::from_hex(&ctx.palette.colors.surface)
            .unwrap_or(Color::new(0.19, 0.2, 0.27, 1.0));
        let overlay = Color::from_hex(&ctx.palette.colors.overlay)
            .unwrap_or(Color::new(0.42, 0.44, 0.53, 1.0));
        let accent = Color::from_hex(&ctx.palette.colors.accent)
            .unwrap_or(Color::new(0.54, 0.71, 0.98, 1.0));
        let urgent = Color::from_hex(&ctx.palette.colors.urgent)
            .unwrap_or(Color::new(0.95, 0.55, 0.66, 1.0));
        let font = (ctx.palette.fonts.size as f32).clamp(9.0, 24.0);

        ctx.text.draw_text(
            pixmap,
            "Configuration History",
            bounds.x,
            bounds.y + 4.0,
            font + 7.0,
            fg,
            bounds.width,
        );
        for (index, label) in ["Older", "Newer", "Restore selected"].iter().enumerate() {
            paint_button(
                pixmap,
                ctx,
                Self::button_rect(bounds, index),
                label,
                surface,
                fg,
                font,
            );
        }

        let input = Self::snapshot_input_rect(bounds);
        fill_rounded_rect(
            pixmap,
            input.x,
            input.y,
            input.width,
            input.height,
            5.0,
            if self.snapshot_focused {
                overlay
            } else {
                surface
            },
        );
        let input_text = if self.snapshot_name.is_empty() {
            "Snapshot name…"
        } else {
            &self.snapshot_name
        };
        ctx.text.draw_text(
            pixmap,
            input_text,
            input.x + 8.0,
            input.y + 6.0,
            font,
            if self.snapshot_name.is_empty() {
                overlay
            } else {
                fg
            },
            input.width - 16.0,
        );
        paint_button(
            pixmap,
            ctx,
            Self::snapshot_button_rect(bounds),
            "Save snapshot",
            accent,
            Color::new(1.0, 1.0, 1.0, 1.0),
            font,
        );

        let filters = [
            "All",
            "Hyprland",
            "Appearance",
            "HyprDeck",
            "HyprSaver",
            "Hyprpaper",
            "HyprCube",
        ];
        for (index, filter) in filters.iter().enumerate() {
            paint_button(
                pixmap,
                ctx,
                Self::filter_rect(bounds, index),
                filter,
                if self.filter == *filter {
                    accent
                } else {
                    surface
                },
                if self.filter == *filter {
                    Color::new(1.0, 1.0, 1.0, 1.0)
                } else {
                    fg
                },
                font - 1.0,
            );
        }

        let list = Self::revision_list_rect(bounds);
        let diff = Rect {
            x: list.x + list.width + 10.0,
            y: list.y,
            width: (bounds.x + bounds.width - list.x - list.width - 10.0).max(0.0),
            height: list.height,
        };
        fill_rounded_rect(
            pixmap,
            list.x,
            list.y,
            list.width,
            list.height,
            6.0,
            surface,
        );
        fill_rounded_rect(
            pixmap,
            diff.x,
            diff.y,
            diff.width,
            diff.height,
            6.0,
            surface,
        );

        for (index, revision) in self
            .revisions
            .iter()
            .take((list.height / REVISION_ROW_HEIGHT) as usize)
            .enumerate()
        {
            let row = Rect {
                x: list.x + 4.0,
                y: list.y + 4.0 + index as f32 * REVISION_ROW_HEIGHT,
                width: list.width - 8.0,
                height: REVISION_ROW_HEIGHT - 3.0,
            };
            if index == self.selected {
                fill_rounded_rect(pixmap, row.x, row.y, row.width, row.height, 4.0, accent);
            }
            let age = revision_age(revision.timestamp);
            let snapshots = if revision.snapshots.is_empty() {
                String::new()
            } else {
                format!(" ★ {}", revision.snapshots.join(", "))
            };
            ctx.text.draw_text(
                pixmap,
                &format!("{}  {}{}", revision.short_hash, age, snapshots),
                row.x + 6.0,
                row.y + 3.0,
                font - 2.0,
                fg,
                row.width - 12.0,
            );
            ctx.text.draw_text(
                pixmap,
                &revision.summary,
                row.x + 6.0,
                row.y + 18.0,
                font - 2.0,
                overlay,
                row.width - 12.0,
            );
        }

        ctx.text.draw_text(
            pixmap,
            &format!("Current → selected diff — {}", self.filter),
            diff.x + 8.0,
            diff.y + 6.0,
            font + 1.0,
            fg,
            diff.width - 16.0,
        );
        let max_lines = ((diff.height - 34.0) / (font + 3.0)).max(0.0) as usize;
        for (line_index, line) in self
            .diff
            .lines()
            .skip(self.diff_scroll)
            .take(max_lines)
            .enumerate()
        {
            let color = if line.starts_with('+') && !line.starts_with("+++") {
                Color::new(0.65, 0.9, 0.65, 1.0)
            } else if line.starts_with('-') && !line.starts_with("---") {
                urgent
            } else if line.starts_with("@@") || line.starts_with("diff ") {
                accent
            } else {
                fg
            };
            ctx.text.draw_text(
                pixmap,
                line,
                diff.x + 8.0,
                diff.y + 30.0 + line_index as f32 * (font + 3.0),
                (font - 2.0).max(8.0),
                color,
                diff.width - 16.0,
            );
        }
        if !self.status.is_empty() {
            ctx.text.draw_text(
                pixmap,
                &self.status,
                bounds.x + 380.0,
                Self::snapshot_y(bounds) + 6.0,
                font - 1.0,
                overlay,
                (bounds.width - 380.0).max(0.0),
            );
        }
    }

    fn event(&mut self, event: &Event, bounds: Rect) -> EventResponse {
        match event {
            Event::PointerDown { x, y } => {
                let point = (*x, *y);
                if Self::button_rect(bounds, 0).contains(point.0, point.1) {
                    self.selected = (self.selected + 1).min(self.revisions.len().saturating_sub(1));
                    self.refresh_diff();
                    return handled();
                }
                if Self::button_rect(bounds, 1).contains(point.0, point.1) {
                    self.selected = self.selected.saturating_sub(1);
                    self.refresh_diff();
                    return handled();
                }
                if Self::button_rect(bounds, 2).contains(point.0, point.1) {
                    self.restore_selected();
                    return handled();
                }
                if Self::snapshot_input_rect(bounds).contains(point.0, point.1) {
                    self.snapshot_focused = true;
                    return handled();
                }
                self.snapshot_focused = false;
                if Self::snapshot_button_rect(bounds).contains(point.0, point.1) {
                    self.snapshot_selected();
                    return handled();
                }
                for (index, filter) in [
                    "All",
                    "Hyprland",
                    "Appearance",
                    "HyprDeck",
                    "HyprSaver",
                    "Hyprpaper",
                    "HyprCube",
                ]
                .iter()
                .enumerate()
                {
                    if Self::filter_rect(bounds, index).contains(point.0, point.1) {
                        self.filter = (*filter).into();
                        self.refresh_diff();
                        return handled();
                    }
                }
                let list = Self::revision_list_rect(bounds);
                if list.contains(point.0, point.1) {
                    let index = ((point.1 - list.y - 4.0) / REVISION_ROW_HEIGHT)
                        .floor()
                        .max(0.0) as usize;
                    if index < self.revisions.len() {
                        self.selected = index;
                        self.refresh_diff();
                    }
                    return handled();
                }
            }
            Event::KeyPress { key, utf8 } if self.snapshot_focused => {
                match key.as_str() {
                    "BackSpace" => {
                        self.snapshot_name.pop();
                    }
                    "Return" | "KP_Enter" => self.snapshot_selected(),
                    _ => {
                        if let Some(text) = utf8 {
                            for character in
                                text.chars().filter(|character| !character.is_control())
                            {
                                if self.snapshot_name.chars().count() < 48 {
                                    self.snapshot_name.push(character);
                                }
                            }
                        }
                    }
                }
                return handled();
            }
            Event::Scroll { dy, .. } => {
                let line_count = self.diff.lines().count();
                if *dy > 0.0 {
                    self.diff_scroll = (self.diff_scroll + 3).min(line_count.saturating_sub(1));
                } else {
                    self.diff_scroll = self.diff_scroll.saturating_sub(3);
                }
                return handled();
            }
            _ => {}
        }
        EventResponse::ignored()
    }
}

fn paint_button(
    pixmap: &mut tiny_skia::PixmapMut<'_>,
    ctx: &mut RenderContext<'_>,
    rect: Rect,
    label: &str,
    background: Color,
    foreground: Color,
    font_size: f32,
) {
    fill_rounded_rect(
        pixmap,
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        5.0,
        background,
    );
    ctx.text.draw_text(
        pixmap,
        label,
        rect.x + 8.0,
        rect.y + 6.0,
        font_size,
        foreground,
        rect.width - 16.0,
    );
}

fn handled() -> EventResponse {
    EventResponse {
        consumed: true,
        action: None,
    }
}

fn revision_age(timestamp: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let elapsed = now.saturating_sub(timestamp);
    match elapsed {
        0..=59 => "now".into(),
        60..=3599 => format!("{}m", elapsed / 60),
        3600..=86_399 => format!("{}h", elapsed / 3600),
        _ => format!("{}d", elapsed / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_paths(name: &str) -> (PathBuf, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "hyprcube-history-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        (root.join("repo"), root.join("config"), root)
    }

    #[test]
    fn captures_diffs_snapshots_and_restores_versions() {
        let (repo, config, root) = temp_paths("roundtrip");
        fs::create_dir_all(&config).unwrap();
        fs::write(config.join("hyprland.conf"), "general { gaps_in = 4 }\n").unwrap();
        let store = HistoryStore::open(repo, config.clone()).unwrap();
        let first = store.capture("Initial configuration").unwrap().unwrap();
        fs::write(config.join("hyprland.conf"), "general { gaps_in = 9 }\n").unwrap();
        store.capture("Changed gaps").unwrap();

        let revisions = store.revisions().unwrap();
        assert_eq!(revisions.len(), 2);
        let diff = store.diff(&revisions[1].hash, "Hyprland").unwrap();
        assert!(diff.contains("gaps_in = 4"));
        assert!(diff.contains("gaps_in = 9"));
        assert_eq!(
            store.create_snapshot("Cozy Theme", &first).unwrap(),
            "cozy-theme"
        );
        assert!(store.create_snapshot("Cozy Theme", &first).is_err());
        store.restore(&first).unwrap();
        assert_eq!(
            fs::read_to_string(config.join("hyprland.conf")).unwrap(),
            "general { gaps_in = 4 }\n"
        );
        assert!(store.revisions().unwrap().len() >= 3);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn section_filter_maps_to_managed_files() {
        assert_eq!(section_paths("HyprDeck"), ["configs/hyprdeck.toml"]);
        assert_eq!(section_paths("All"), ["configs"]);
    }

    #[test]
    fn snapshot_names_are_safe_git_refs() {
        assert_eq!(
            snapshot_slug("  Warm / Work Theme  ").as_deref(),
            Some("warm-work-theme")
        );
        assert_eq!(snapshot_slug("///"), None);
    }
}
