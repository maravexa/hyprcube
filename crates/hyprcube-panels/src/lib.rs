pub mod about;
pub mod appearance;
pub mod display;
pub mod form;
pub mod hyprdeck;
pub mod hyprland;
pub mod hyprpaper;
pub mod hyprsaver;
pub mod input;
pub mod keymap;

use hyprcube_core::layout::Rect;
use hyprcube_core::text::RenderContext;
use hyprcube_core::widget::{Constraints, Event, EventResponse, Size};

// ---------------------------------------------------------------------------
// PanelError
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum PanelError {
    #[error("unknown key: {0}")]
    UnknownKey(String),
    #[error("invalid value for {key}: {reason}")]
    InvalidValue { key: String, reason: String },
    #[error("config error: {0}")]
    Config(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

// ---------------------------------------------------------------------------
// FieldType / PanelField
// ---------------------------------------------------------------------------

/// The type of a configurable field, used to select the appropriate widget.
#[derive(Debug, Clone)]
pub enum FieldType {
    Text,
    /// A momentary action button; its label is rendered inside the control.
    Action {
        label: String,
    },
    Integer {
        min: Option<i64>,
        max: Option<i64>,
    },
    Float {
        min: Option<f64>,
        max: Option<f64>,
    },
    Boolean,
    Choice {
        options: Vec<String>,
    },
    /// Like `Choice` but renders as a searchable dropdown with type-to-filter.
    /// Each entry is `(value, display_label)`.
    SearchableChoice {
        options: Vec<(String, String)>,
    },
    Color,
    KeyBind,
}

/// Runtime description of a single configurable field within a panel.
#[derive(Debug, Clone)]
pub struct PanelField {
    pub key: String,
    /// Hyprland config section for IPC keyword routing, e.g. `"general"` or
    /// `"input:touchpad"`.  `None` for fields that don't map to Hyprland IPC.
    pub section: Option<String>,
    pub label: String,
    pub description: String,
    pub field_type: FieldType,
    pub current_value: String,
    /// The value that was in the config when the panel loaded.
    /// `None` means the key did not exist in the original config.
    pub original_value: Option<String>,
    /// Set to `true` when the user explicitly changes this field's value.
    pub dirty: bool,
}

/// A panel-local operation changed configuration files outside the ordinary
/// Save/Revert pipeline and the application must reload its shared state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalConfigChange {
    pub message: String,
    pub reload_hyprland: bool,
}

// ---------------------------------------------------------------------------
// SettingsPanel trait
// ---------------------------------------------------------------------------

/// A settings panel that can be displayed in the sidebar and renders a
/// scrollable form of configurable fields.
pub trait SettingsPanel {
    /// Human-readable title for the sidebar.
    fn title(&self) -> &str;

    /// Freedesktop icon name.
    fn icon(&self) -> &str;

    /// Whether this panel should appear (false if the app it configures isn't
    /// installed).
    fn available(&self) -> bool {
        true
    }

    /// Collect all configurable fields for this panel.
    fn fields(&self) -> Vec<PanelField>;

    /// Apply a value change. Returns the old value for undo.
    fn set_value(&mut self, key: &str, value: &str) -> Result<String, PanelError>;

    /// Whether this panel owns edits that have not been written to disk.
    ///
    /// Most panels are backed by the shared Hyprland preview stack, so they
    /// use the default. Standalone configuration editors (such as HyprDeck)
    /// override this to participate in the common Save/Revert action bar.
    fn has_unsaved_changes(&self) -> bool {
        false
    }

    /// Persist panel-local configuration. Called only after the shared
    /// configuration files have been written successfully.
    fn save(&mut self) -> Result<(), PanelError> {
        Ok(())
    }

    /// Discard panel-local edits by rereading their backing configuration.
    fn reload(&mut self) -> Result<(), PanelError> {
        Ok(())
    }

    /// User-facing follow-up required after a successful save, if any.
    fn save_notice(&self) -> Option<&str> {
        None
    }

    /// Whether this panel paints and handles its own content instead of using
    /// the generated settings form.
    fn uses_custom_layout(&self) -> bool {
        false
    }

    /// Whether interaction with this panel may replace configuration files and
    /// therefore must be blocked while another panel has unsaved edits.
    fn requires_clean_state(&self) -> bool {
        false
    }

    /// Retrieve a completed panel-local configuration operation, such as a
    /// history restore, so the application can reload shared state.
    fn take_external_config_change(&mut self) -> Option<ExternalConfigChange> {
        None
    }

    /// Refresh state sourced from files or another external store.
    fn refresh_external_state(&mut self) {}

    /// Widget measure hook.
    fn measure(&self, constraints: Constraints) -> Size;

    /// Widget paint hook.
    fn paint(
        &self,
        bounds: Rect,
        pixmap: &mut tiny_skia::PixmapMut<'_>,
        ctx: &mut RenderContext<'_>,
    );

    /// Widget event hook.
    fn event(&mut self, event: &Event, bounds: Rect) -> EventResponse;

    /// Build a scrollable form for this panel's fields.
    /// Panels that need a custom layout (e.g. Display) can override this.
    fn build_form(&self, content_width: f32) -> form::FormLayout {
        form::FormLayout::from_fields(self.fields(), content_width)
    }
}
