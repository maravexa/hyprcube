pub mod about;
pub mod appearance;
pub mod display;
pub mod form;
pub mod hyprdeck;
pub mod hyprland;
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
    Integer { min: Option<i64>, max: Option<i64> },
    Float { min: Option<f64>, max: Option<f64> },
    Boolean,
    Choice { options: Vec<String> },
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
