use hyprcube_panels::SettingsPanel;
use hyprcube_panels::about::AboutPanel;
use hyprcube_panels::appearance::AppearancePanel;
use hyprcube_panels::display::DisplayPanel;
use hyprcube_panels::hyprdeck::HyprdeckPanel;
use hyprcube_panels::hyprland::HyprlandPanel;
use hyprcube_panels::hyprsaver::HyprsaverPanel;
use hyprcube_panels::input::InputPanel;

/// Manages the list of available settings panels.
pub struct PanelRegistry {
    panels: Vec<Box<dyn SettingsPanel>>,
    active_index: usize,
}

impl PanelRegistry {
    /// Instantiate all panels, keeping only those where `available()` is true.
    pub fn new() -> Self {
        let all: Vec<Box<dyn SettingsPanel>> = vec![
            Box::new(AppearancePanel::new()),
            Box::new(HyprlandPanel::new()),
            Box::new(InputPanel::new()),
            Box::new(DisplayPanel::new()),
            Box::new(HyprdeckPanel::new()),
            Box::new(HyprsaverPanel::new()),
            Box::new(AboutPanel::new()),
        ];

        let panels: Vec<Box<dyn SettingsPanel>> = all
            .into_iter()
            .filter(|p| p.available())
            .collect();

        Self {
            panels,
            active_index: 0,
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
    ///
    /// # Panics
    /// Panics if `index` is out of bounds.
    pub fn set_active(&mut self, index: usize) {
        assert!(index < self.panels.len(), "panel index {index} out of bounds");
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
}
