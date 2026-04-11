use tiny_skia::PixmapMut;

use crate::color::Color;
use crate::layout::Rect;
use crate::render::fill_rounded_rect;

/// Unique widget identifier.
pub type WidgetId = u64;

/// Input events delivered to widgets.
#[derive(Debug, Clone)]
pub enum Event {
    PointerEnter,
    PointerLeave,
    PointerDown { x: f32, y: f32 },
    PointerUp { x: f32, y: f32 },
    KeyPress { key: String, utf8: Option<String> },
    Scroll { dx: f32, dy: f32 },
}

/// An action emitted by a widget in response to an event.
#[derive(Debug, Clone)]
pub enum WidgetAction {
    ValueChanged { key: String, value: String },
    ButtonClicked { id: WidgetId },
    Navigate { panel: String },
}

/// The result of delivering an event to a widget.
#[derive(Debug, Clone)]
pub struct EventResponse {
    pub consumed: bool,
    pub action: Option<WidgetAction>,
}

impl EventResponse {
    pub fn ignored() -> Self {
        Self {
            consumed: false,
            action: None,
        }
    }
}

/// Layout constraints passed to `Widget::measure`.
#[derive(Debug, Clone, Copy)]
pub struct Constraints {
    pub min_width: f32,
    pub min_height: f32,
    pub max_width: f32,
    pub max_height: f32,
}

impl Constraints {
    /// Tight constraints that force a specific size.
    pub fn tight(width: f32, height: f32) -> Self {
        Self {
            min_width: width,
            min_height: height,
            max_width: width,
            max_height: height,
        }
    }

    /// Loose constraints that allow anything up to max.
    pub fn loose(max_width: f32, max_height: f32) -> Self {
        Self {
            min_width: 0.0,
            min_height: 0.0,
            max_width,
            max_height,
        }
    }
}

/// Measured size returned by `Widget::measure`.
#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

/// Core widget trait.
pub trait Widget {
    /// Measure the widget and return its desired size within the given constraints.
    fn measure(&self, constraints: Constraints) -> Size;

    /// Paint the widget into the given pixmap at the specified bounds.
    fn paint(&self, pixmap: &mut PixmapMut<'_>, bounds: Rect);

    /// Handle an input event. The default implementation ignores all events.
    fn event(&mut self, _event: &Event, _bounds: &Rect) -> EventResponse {
        EventResponse::ignored()
    }
}

// ---------------------------------------------------------------------------
// Concrete widgets
// ---------------------------------------------------------------------------

/// A simple text label.
pub struct Label {
    pub text: String,
    pub color: Color,
    pub font_size: f32,
}

impl Label {
    pub fn new(text: impl Into<String>, color: Color, font_size: f32) -> Self {
        Self {
            text: text.into(),
            color,
            font_size,
        }
    }
}

impl Widget for Label {
    fn measure(&self, constraints: Constraints) -> Size {
        // Approximate: 0.6 × font_size per character width, 1.2 × font_size height.
        let width = (self.text.len() as f32 * self.font_size * 0.6)
            .clamp(constraints.min_width, constraints.max_width);
        let height = (self.font_size * 1.2).clamp(constraints.min_height, constraints.max_height);
        Size { width, height }
    }

    fn paint(&self, _pixmap: &mut PixmapMut<'_>, _bounds: Rect) {
        // Text rendering requires a full cosmic-text pipeline;
        // actual glyph painting will be wired up later.
    }
}

/// A boolean toggle switch.
pub struct Toggle {
    pub value: bool,
    pub key: String,
}

impl Toggle {
    pub fn new(key: impl Into<String>, value: bool) -> Self {
        Self {
            key: key.into(),
            value,
        }
    }
}

impl Widget for Toggle {
    fn measure(&self, constraints: Constraints) -> Size {
        let width = 48.0_f32.clamp(constraints.min_width, constraints.max_width);
        let height = 24.0_f32.clamp(constraints.min_height, constraints.max_height);
        Size { width, height }
    }

    fn paint(&self, pixmap: &mut PixmapMut<'_>, bounds: Rect) {
        let track_color = if self.value {
            Color::from_hex("#89b4fa").unwrap()
        } else {
            Color::from_hex("#6c7086").unwrap()
        };
        fill_rounded_rect(
            pixmap,
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
            bounds.height / 2.0,
            track_color,
        );

        // Knob
        let knob_r = bounds.height * 0.4;
        let knob_x = if self.value {
            bounds.x + bounds.width - bounds.height / 2.0
        } else {
            bounds.x + bounds.height / 2.0
        };
        let knob_y = bounds.y + bounds.height / 2.0;
        fill_rounded_rect(
            pixmap,
            knob_x - knob_r,
            knob_y - knob_r,
            knob_r * 2.0,
            knob_r * 2.0,
            knob_r,
            Color::new(1.0, 1.0, 1.0, 1.0),
        );
    }

    fn event(&mut self, event: &Event, bounds: &Rect) -> EventResponse {
        if let Event::PointerUp { x, y } = event {
            if bounds.contains(*x, *y) {
                self.value = !self.value;
                return EventResponse {
                    consumed: true,
                    action: Some(WidgetAction::ValueChanged {
                        key: self.key.clone(),
                        value: self.value.to_string(),
                    }),
                };
            }
        }
        EventResponse::ignored()
    }
}
