use hyprcube_core::palette::Palette;
use hyprcube_hyprconf::HyprlandConfig;
use hyprcube_panels::about::AboutPanel;
use hyprcube_panels::appearance::AppearancePanel;
use hyprcube_panels::display::DisplayPanel;
use hyprcube_panels::hyprdeck::HyprdeckPanel;
use hyprcube_panels::hyprland::HyprlandPanel;
use hyprcube_panels::hyprpaper::HyprpaperPanel;
use hyprcube_panels::hyprsaver::HyprsaverPanel;
use hyprcube_panels::input::InputPanel;
use hyprcube_panels::SettingsPanel;

use crate::history::{HistoryPanel, HistoryStore};
use crate::shared_config::SharedConfigs;

/// Manages the list of available settings panels and the configs they share.
pub struct PanelRegistry {
    panels: Vec<Box<dyn SettingsPanel>>,
    active_index: usize,
    pub shared: SharedConfigs,
    history: Option<HistoryStore>,
}

impl PanelRegistry {
    /// Instantiate all panels, keeping only those where `available()` is true.
    ///
    /// HyprlandPanel and InputPanel receive a clone of the same
    /// `Arc<Mutex<HyprlandConfig>>` so their in-memory edits are always
    /// consistent.  AppearancePanel gets a shared palette Arc.
    pub fn new() -> Self {
        // Load shared configs once.
        let hyprland = HyprlandConfig::load_default().unwrap_or_else(|e| {
            tracing::warn!("failed to load hyprland.conf: {e}");
            HyprlandConfig { items: Vec::new() }
        });
        let palette = Palette::load(None).unwrap_or_else(|_| {
            tracing::info!("no palette found, saving defaults");
            let p = Palette::default();
            if let Err(e) = p.save(None) {
                tracing::warn!("failed to save default palette: {e}");
            }
            p
        });
        let shared = SharedConfigs::new(hyprland, palette);
        let (history, history_error) = match HistoryStore::open_default() {
            Ok(store) => {
                let error = store
                    .capture("Initial configuration")
                    .err()
                    .map(|error| error.to_string());
                (Some(store), error)
            }
            Err(error) => (None, Some(error.to_string())),
        };

        // Distribute Arc clones to the panels that share configs.
        let all: Vec<Box<dyn SettingsPanel>> = vec![
            Box::new(AppearancePanel::with_arc(shared.palette.clone())),
            Box::new(HyprlandPanel::with_arc(shared.hyprland.clone())),
            Box::new(InputPanel::with_arc(shared.hyprland.clone())),
            Box::new(DisplayPanel::new()),
            Box::new(HyprpaperPanel::new()),
            Box::new(HyprdeckPanel::new()),
            Box::new(HyprsaverPanel::new()),
            Box::new(HistoryPanel::new(history.clone(), history_error)),
            Box::new(AboutPanel::new()),
        ];

        let panels: Vec<Box<dyn SettingsPanel>> =
            all.into_iter().filter(|p| p.available()).collect();

        Self {
            panels,
            active_index: 0,
            shared,
            history,
        }
    }

    /// Returns (index, title, icon) for each available panel.
    pub fn available_panels(&self) -> Vec<(usize, &str, &str)> {
        self.panels
            .iter()
            .enumerate()
            .map(|(i, p)| (i, p.title(), p.icon()))
            .collect()
    }

    /// Reference to the currently active panel.
    pub fn active_panel(&self) -> &dyn SettingsPanel {
        &*self.panels[self.active_index]
    }

    /// Mutable reference to the currently active panel.
    pub fn active_panel_mut(&mut self) -> &mut dyn SettingsPanel {
        &mut *self.panels[self.active_index]
    }

    /// Switch the active panel by index.
    pub fn set_active(&mut self, index: usize) {
        assert!(
            index < self.panels.len(),
            "panel index {index} out of bounds"
        );
        self.active_index = index;
    }

    /// The index of the currently active panel.
    pub fn active_index(&self) -> usize {
        self.active_index
    }

    /// Number of available panels.
    pub fn len(&self) -> usize {
        self.panels.len()
    }

    /// Find a panel index by title (case-insensitive).
    pub fn find_by_title(&self, title: &str) -> Option<usize> {
        let lower = title.to_lowercase();
        self.panels
            .iter()
            .position(|p| p.title().to_lowercase() == lower)
    }

    /// Return whether any panel-local editor has unsaved configuration.
    pub fn has_unsaved_changes(&self) -> bool {
        self.panels.iter().any(|panel| panel.has_unsaved_changes())
    }

    /// Write all shared and panel-local configs to disk.
    ///
    /// `hyprland.conf` is backed up to `.bak` before overwriting.
    ///
    /// A panel save is allowed to reject its own configuration. Callers must
    /// treat any error as a failed save and retain their undo state.
    pub fn save_all(&mut self) -> Result<Vec<String>, hyprcube_panels::PanelError> {
        self.shared.save_hyprland()?;
        self.shared
            .save_palette()
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        let mut notices = Vec::new();
        for panel in &mut self.panels {
            if !panel.has_unsaved_changes() {
                continue;
            }
            let notice = panel.save_notice().map(ToString::to_string);
            panel.save()?;
            if let Some(notice) = notice {
                notices.push(notice);
            }
        }
        if let Some(history) = &self.history {
            match history.capture("Saved configuration from HyprCube") {
                Ok(_) => {}
                Err(error) => notices.push(format!("History capture failed: {error}")),
            }
        }
        for panel in &mut self.panels {
            panel.refresh_external_state();
        }
        Ok(notices)
    }

    /// Discard all in-memory config mutations by reloading from disk.
    /// Called after `PreviewEngine::revert_all()` to keep the in-memory
    /// state consistent with what IPC has been told.
    pub fn reload_from_disk(&mut self) {
        if let Err(e) = self.shared.reload_hyprland() {
            tracing::warn!("failed to reload hyprland.conf: {e}");
        }
        if let Err(e) = self.shared.reload_palette() {
            tracing::warn!("failed to reload palette: {e}");
        }
        for panel in &mut self.panels {
            if let Err(error) = panel.reload() {
                tracing::warn!("failed to reload {} settings: {error}", panel.title());
            }
            panel.refresh_external_state();
        }
    }

    /// Clone of the current palette — used by the renderer.
    pub fn current_palette(&self) -> Palette {
        self.shared.current_palette()
    }
}
