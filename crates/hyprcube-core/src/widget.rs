use tiny_skia::{Paint, PathBuilder, PixmapMut, Transform};

use crate::color::Color;
use crate::layout::Rect;
use crate::palette::Palette;
use crate::render::fill_rounded_rect;
use crate::text::RenderContext;

/// Unique widget identifier.
pub type WidgetId = u64;

/// Input events delivered to widgets.
#[derive(Debug, Clone)]
pub enum Event {
    PointerEnter,
    PointerLeave,
    PointerDown { x: f32, y: f32 },
    PointerMove { x: f32, y: f32 },
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
    /// For widgets with overlay content (e.g. open dropdowns), only paint the
    /// base/collapsed state here. The overlay is painted in `paint_overlay`.
    fn paint(&self, pixmap: &mut PixmapMut<'_>, bounds: Rect, ctx: &mut RenderContext<'_>);

    /// Handle an input event. The default implementation ignores all events.
    fn event(&mut self, _event: &Event, _bounds: &Rect) -> EventResponse {
        EventResponse::ignored()
    }

    /// Returns true when this widget has overlay content (e.g. an open dropdown
    /// option list) that must be painted above all other widgets.
    fn has_overlay(&self) -> bool {
        false
    }

    /// Height of the overlay content in logical pixels. Used to compute hit
    /// bounds for click-outside dismiss. Returns 0.0 if no overlay.
    fn overlay_height(&self) -> f32 {
        0.0
    }

    /// Paint the overlay content (e.g. dropdown option list) that must render
    /// above all other widgets. Called after all base widgets are painted.
    /// `bounds` is the same base widget bounds passed to `paint`.
    fn paint_overlay(
        &self,
        _pixmap: &mut PixmapMut<'_>,
        _bounds: Rect,
        _ctx: &mut RenderContext<'_>,
    ) {
    }

    /// Close/collapse any open overlay state. Called when the user clicks
    /// outside this widget.
    fn dismiss_overlay(&mut self) {}
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn draw_chevron(pixmap: &mut PixmapMut<'_>, cx: f32, cy: f32, color: Color) {
    let mut pb = PathBuilder::new();
    pb.move_to(cx - 5.0, cy - 2.0);
    pb.line_to(cx + 5.0, cy - 2.0);
    pb.line_to(cx, cy + 3.0);
    pb.close();
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(color.to_skia());
        paint.anti_alias = true;
        pixmap.fill_path(
            &path,
            &paint,
            tiny_skia::FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn parse_color(hex: &str, fallback: Color) -> Color {
    Color::from_hex(hex).unwrap_or(fallback)
}

// ---------------------------------------------------------------------------
// Concrete widgets
// ---------------------------------------------------------------------------

/// A simple text label.
pub struct Label {
    pub text: String,
    /// `None` uses the palette foreground color.
    pub color: Option<Color>,
    pub font_size: f32,
}

impl Label {
    pub fn new(text: impl Into<String>, font_size: f32) -> Self {
        Self {
            text: text.into(),
            color: None,
            font_size,
        }
    }

    pub fn with_color(text: impl Into<String>, color: Color, font_size: f32) -> Self {
        Self {
            text: text.into(),
            color: Some(color),
            font_size,
        }
    }
}

impl Widget for Label {
    fn measure(&self, constraints: Constraints) -> Size {
        let width = (self.text.len() as f32 * self.font_size * 0.6)
            .clamp(constraints.min_width, constraints.max_width);
        let height = (self.font_size * 1.3).clamp(constraints.min_height, constraints.max_height);
        Size { width, height }
    }

    fn paint(&self, pixmap: &mut PixmapMut<'_>, bounds: Rect, ctx: &mut RenderContext<'_>) {
        let color = self.color.unwrap_or_else(|| {
            parse_color(
                &ctx.palette.colors.foreground,
                Color::new(0.804, 0.839, 0.957, 1.0),
            )
        });
        ctx.text.draw_text(
            pixmap,
            &self.text,
            bounds.x,
            bounds.y,
            self.font_size,
            color,
            bounds.width,
        );
    }
}

/// A push button that emits a value-change action when activated.
pub struct Button {
    pub key: String,
    pub label: String,
    pressed: bool,
}

impl Button {
    pub fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            pressed: false,
        }
    }
}

impl Widget for Button {
    fn measure(&self, constraints: Constraints) -> Size {
        Size {
            width: 160.0_f32.clamp(constraints.min_width, constraints.max_width),
            height: 32.0_f32.clamp(constraints.min_height, constraints.max_height),
        }
    }

    fn paint(&self, pixmap: &mut PixmapMut<'_>, bounds: Rect, ctx: &mut RenderContext<'_>) {
        let accent = parse_color(
            &ctx.palette.colors.accent,
            Color::new(0.537, 0.706, 0.98, 1.0),
        );
        let foreground = parse_color(
            &ctx.palette.colors.foreground,
            Color::new(0.804, 0.839, 0.957, 1.0),
        );
        let color = if self.pressed {
            Color::new(accent.r * 0.8, accent.g * 0.8, accent.b * 0.8, accent.a)
        } else {
            accent
        };
        fill_rounded_rect(
            pixmap,
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
            6.0,
            color,
        );
        let font_size = ctx.palette.fonts.size as f32;
        ctx.text.draw_text(
            pixmap,
            &self.label,
            bounds.x + 10.0,
            bounds.y + (bounds.height - font_size * 1.3) / 2.0,
            font_size,
            foreground,
            bounds.width - 20.0,
        );
    }

    fn event(&mut self, event: &Event, bounds: &Rect) -> EventResponse {
        match event {
            Event::PointerDown { x, y } if bounds.contains(*x, *y) => {
                self.pressed = true;
                EventResponse {
                    consumed: true,
                    action: None,
                }
            }
            Event::PointerUp { x, y } => {
                let activate = self.pressed && bounds.contains(*x, *y);
                self.pressed = false;
                if activate {
                    EventResponse {
                        consumed: true,
                        action: Some(WidgetAction::ValueChanged {
                            key: self.key.clone(),
                            value: "true".into(),
                        }),
                    }
                } else {
                    EventResponse::ignored()
                }
            }
            _ => EventResponse::ignored(),
        }
    }
}

/// A boolean toggle switch (44×24px).
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
        let width = 44.0_f32.clamp(constraints.min_width, constraints.max_width);
        let height = 24.0_f32.clamp(constraints.min_height, constraints.max_height);
        Size { width, height }
    }

    fn paint(&self, pixmap: &mut PixmapMut<'_>, bounds: Rect, ctx: &mut RenderContext<'_>) {
        let track_color = if self.value {
            parse_color(
                &ctx.palette.colors.accent,
                Color::new(0.537, 0.706, 0.98, 1.0),
            )
        } else {
            parse_color(
                &ctx.palette.colors.overlay,
                Color::new(0.424, 0.439, 0.525, 1.0),
            )
        };
        let thumb_color = if self.value {
            Color::new(1.0, 1.0, 1.0, 1.0)
        } else {
            parse_color(
                &ctx.palette.colors.surface,
                Color::new(0.192, 0.196, 0.267, 1.0),
            )
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
            thumb_color,
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

/// A horizontal range slider (200×32px).
pub struct Slider {
    pub value: f64,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub key: String,
    editing: bool,
    dragging: bool,
    text_value: String,
    cursor: usize,
}

impl Slider {
    pub fn new(key: impl Into<String>, value: f64, min: f64, max: f64, step: f64) -> Self {
        let text_value = format_slider_value(value, step);
        let cursor = text_value.chars().count();
        Self {
            key: key.into(),
            value,
            min,
            max,
            step,
            editing: false,
            dragging: false,
            text_value,
            cursor,
        }
    }

    /// Update both the slider position and its coupled numeric text field.
    pub fn set_value(&mut self, value: f64) {
        self.value = value.clamp(self.min, self.max);
        self.text_value = format_slider_value(self.value, self.step);
        self.cursor = self.text_value.chars().count();
    }

    const VALUE_WIDTH: f32 = 72.0;
    const VALUE_GAP: f32 = 12.0;

    fn value_bounds(bounds: Rect) -> Rect {
        Rect {
            x: bounds.x + bounds.width - Self::VALUE_WIDTH,
            y: bounds.y,
            width: Self::VALUE_WIDTH,
            height: bounds.height,
        }
    }

    fn track_bounds(bounds: Rect) -> Rect {
        Rect {
            x: bounds.x,
            y: bounds.y,
            width: (bounds.width - Self::VALUE_WIDTH - Self::VALUE_GAP).max(24.0),
            height: bounds.height,
        }
    }

    fn set_from_x(&mut self, x: f32, bounds: Rect) {
        let fraction = ((x - bounds.x) / bounds.width).clamp(0.0, 1.0) as f64;
        let raw = self.min + (self.max - self.min) * fraction;
        let stepped = if self.step > 0.0 {
            ((raw - self.min) / self.step).round() * self.step + self.min
        } else {
            raw
        };
        self.value = stepped.clamp(self.min, self.max);
        self.text_value = format_slider_value(self.value, self.step);
        self.cursor = self.text_value.chars().count();
    }

    fn value_changed(&self) -> EventResponse {
        EventResponse {
            consumed: true,
            action: Some(WidgetAction::ValueChanged {
                key: self.key.clone(),
                value: self.value.to_string(),
            }),
        }
    }

    fn accept_text_value(&mut self) -> Option<EventResponse> {
        let parsed = self.text_value.parse::<f64>().ok()?;
        if !parsed.is_finite() {
            return None;
        }
        self.value = parsed.clamp(self.min, self.max);
        Some(self.value_changed())
    }
}

fn format_slider_value(value: f64, step: f64) -> String {
    let precision = if step >= 1.0 {
        0
    } else if step >= 0.1 {
        1
    } else if step >= 0.01 {
        2
    } else {
        3
    };
    format!("{value:.precision$}")
}

impl Widget for Slider {
    fn measure(&self, constraints: Constraints) -> Size {
        let width = 200.0_f32.clamp(constraints.min_width, constraints.max_width);
        let height = 32.0_f32.clamp(constraints.min_height, constraints.max_height);
        Size { width, height }
    }

    fn paint(&self, pixmap: &mut PixmapMut<'_>, bounds: Rect, ctx: &mut RenderContext<'_>) {
        let surface = parse_color(
            &ctx.palette.colors.surface,
            Color::new(0.192, 0.196, 0.267, 1.0),
        );
        let accent = parse_color(
            &ctx.palette.colors.accent,
            Color::new(0.537, 0.706, 0.98, 1.0),
        );

        let value_bounds = Self::value_bounds(bounds);
        let track_bounds = Self::track_bounds(bounds);
        let overlay = parse_color(
            &ctx.palette.colors.overlay,
            Color::new(0.424, 0.439, 0.525, 1.0),
        );
        let foreground = parse_color(
            &ctx.palette.colors.foreground,
            Color::new(0.804, 0.839, 0.957, 1.0),
        );
        let track_h = 4.0;
        let thumb_size = 16.0;
        let track_y = track_bounds.y + (track_bounds.height - track_h) / 2.0;

        // Track background
        fill_rounded_rect(
            pixmap,
            track_bounds.x,
            track_y,
            track_bounds.width,
            track_h,
            track_h / 2.0,
            surface,
        );

        let range = (self.max - self.min) as f32;
        let fraction = if range > 0.0 {
            ((self.value - self.min) as f32 / range).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let fill_w = fraction * track_bounds.width;

        // Filled portion
        if fill_w > 0.0 {
            fill_rounded_rect(
                pixmap,
                track_bounds.x,
                track_y,
                fill_w,
                track_h,
                track_h / 2.0,
                accent,
            );
        }

        // Thumb circle
        let thumb_x = track_bounds.x + fill_w - thumb_size / 2.0;
        let thumb_y = track_bounds.y + (track_bounds.height - thumb_size) / 2.0;
        fill_rounded_rect(
            pixmap,
            thumb_x,
            thumb_y,
            thumb_size,
            thumb_size,
            thumb_size / 2.0,
            accent,
        );

        let border = if self.editing { accent } else { overlay };
        fill_rounded_rect(
            pixmap,
            value_bounds.x,
            value_bounds.y,
            value_bounds.width,
            value_bounds.height,
            5.0,
            border,
        );
        fill_rounded_rect(
            pixmap,
            value_bounds.x + 1.0,
            value_bounds.y + 1.0,
            value_bounds.width - 2.0,
            value_bounds.height - 2.0,
            4.0,
            surface,
        );
        let font_size = (ctx.palette.fonts.size as f32 - 1.0).max(9.0);
        ctx.text.draw_text(
            pixmap,
            &self.text_value,
            value_bounds.x + 7.0,
            value_bounds.y + (value_bounds.height - font_size * 1.3) / 2.0,
            font_size,
            foreground,
            value_bounds.width - 14.0,
        );
    }

    fn event(&mut self, event: &Event, bounds: &Rect) -> EventResponse {
        let track_bounds = Self::track_bounds(*bounds);
        let value_bounds = Self::value_bounds(*bounds);
        match event {
            Event::PointerDown { x, y } if value_bounds.contains(*x, *y) => {
                self.editing = true;
                self.dragging = false;
                self.cursor = self.text_value.chars().count();
                EventResponse {
                    consumed: true,
                    action: None,
                }
            }
            Event::PointerUp { x, y } if value_bounds.contains(*x, *y) => EventResponse {
                consumed: true,
                action: None,
            },
            Event::PointerDown { x, y } if track_bounds.contains(*x, *y) => {
                self.editing = false;
                self.dragging = true;
                self.set_from_x(*x, track_bounds);
                self.value_changed()
            }
            Event::PointerMove { x, .. } if self.dragging => {
                self.set_from_x(*x, track_bounds);
                self.value_changed()
            }
            Event::PointerUp { x, .. } if self.dragging => {
                self.dragging = false;
                self.set_from_x(*x, track_bounds);
                self.value_changed()
            }
            Event::PointerUp { x, y } if track_bounds.contains(*x, *y) => {
                self.editing = false;
                self.set_from_x(*x, track_bounds);
                self.value_changed()
            }
            Event::KeyPress { key, utf8 } if self.editing => {
                match key.as_str() {
                    "Escape" => {
                        self.editing = false;
                        self.text_value = format_slider_value(self.value, self.step);
                        self.cursor = self.text_value.chars().count();
                        return EventResponse {
                            consumed: true,
                            action: None,
                        };
                    }
                    "Return" => {
                        self.editing = false;
                        if let Some(response) = self.accept_text_value() {
                            self.text_value = format_slider_value(self.value, self.step);
                            self.cursor = self.text_value.chars().count();
                            return response;
                        }
                    }
                    "BackSpace" if self.cursor > 0 => {
                        let byte = self
                            .text_value
                            .char_indices()
                            .nth(self.cursor - 1)
                            .map(|(index, _)| index)
                            .unwrap_or(0);
                        self.text_value.remove(byte);
                        self.cursor -= 1;
                    }
                    "Delete" if self.cursor < self.text_value.chars().count() => {
                        let byte = self
                            .text_value
                            .char_indices()
                            .nth(self.cursor)
                            .map(|(index, _)| index)
                            .unwrap_or(self.text_value.len());
                        self.text_value.remove(byte);
                    }
                    "Left" => self.cursor = self.cursor.saturating_sub(1),
                    "Right" => self.cursor = (self.cursor + 1).min(self.text_value.chars().count()),
                    "Home" => self.cursor = 0,
                    "End" => self.cursor = self.text_value.chars().count(),
                    _ => {
                        if let Some(text) = utf8 {
                            for character in text.chars().filter(|character| {
                                character.is_ascii_digit()
                                    || matches!(character, '.' | '-' | '+' | 'e' | 'E')
                            }) {
                                let byte = self
                                    .text_value
                                    .char_indices()
                                    .nth(self.cursor)
                                    .map(|(index, _)| index)
                                    .unwrap_or(self.text_value.len());
                                self.text_value.insert(byte, character);
                                self.cursor += 1;
                            }
                        }
                    }
                }
                self.accept_text_value().unwrap_or(EventResponse {
                    consumed: true,
                    action: None,
                })
            }
            _ => EventResponse::ignored(),
        }
    }
}

/// A dropdown selector (200×32px closed, expands when open).
pub struct Dropdown {
    pub options: Vec<String>,
    pub selected: usize,
    pub key: String,
    pub open: bool,
}

impl Dropdown {
    pub fn new(key: impl Into<String>, options: Vec<String>, selected: usize) -> Self {
        Self {
            key: key.into(),
            options,
            selected,
            open: false,
        }
    }

    const CLOSED_H: f32 = 32.0;
    const OPTION_H: f32 = 28.0;
}

impl Widget for Dropdown {
    fn measure(&self, constraints: Constraints) -> Size {
        let width = 200.0_f32.clamp(constraints.min_width, constraints.max_width);
        let height = Self::CLOSED_H.clamp(constraints.min_height, constraints.max_height);
        Size { width, height }
    }

    fn paint(&self, pixmap: &mut PixmapMut<'_>, bounds: Rect, ctx: &mut RenderContext<'_>) {
        let surface = parse_color(
            &ctx.palette.colors.surface,
            Color::new(0.192, 0.196, 0.267, 1.0),
        );
        let fg = parse_color(
            &ctx.palette.colors.foreground,
            Color::new(0.804, 0.839, 0.957, 1.0),
        );
        let overlay = parse_color(
            &ctx.palette.colors.overlay,
            Color::new(0.424, 0.439, 0.525, 1.0),
        );
        let font_size = ctx.palette.fonts.size as f32;

        // Closed header box (always painted here; option list is in paint_overlay)
        fill_rounded_rect(
            pixmap,
            bounds.x,
            bounds.y,
            bounds.width,
            Self::CLOSED_H,
            6.0,
            surface,
        );

        let selected_text = self
            .options
            .get(self.selected)
            .map(|s| s.as_str())
            .unwrap_or("");
        let text_y = bounds.y + (Self::CLOSED_H - font_size * 1.3) / 2.0;
        ctx.text.draw_text(
            pixmap,
            selected_text,
            bounds.x + 8.0,
            text_y,
            font_size,
            fg,
            bounds.width - 32.0,
        );

        let chevron_cx = bounds.x + bounds.width - 14.0;
        let chevron_cy = bounds.y + Self::CLOSED_H / 2.0;
        draw_chevron(pixmap, chevron_cx, chevron_cy, overlay);
    }

    fn paint_overlay(&self, pixmap: &mut PixmapMut<'_>, bounds: Rect, ctx: &mut RenderContext<'_>) {
        if !self.open {
            return;
        }
        let surface = parse_color(
            &ctx.palette.colors.surface,
            Color::new(0.192, 0.196, 0.267, 1.0),
        );
        let fg = parse_color(
            &ctx.palette.colors.foreground,
            Color::new(0.804, 0.839, 0.957, 1.0),
        );
        let accent = parse_color(
            &ctx.palette.colors.accent,
            Color::new(0.537, 0.706, 0.98, 1.0),
        );
        let overlay = parse_color(
            &ctx.palette.colors.overlay,
            Color::new(0.424, 0.439, 0.525, 1.0),
        );
        let font_size = ctx.palette.fonts.size as f32;

        let list_y = bounds.y + Self::CLOSED_H;
        let list_h = self.options.len() as f32 * Self::OPTION_H;

        // Drop shadow (offset dark rect behind the list)
        fill_rounded_rect(
            pixmap,
            bounds.x + 2.0,
            list_y + 2.0,
            bounds.width,
            list_h,
            6.0,
            Color::new(0.0, 0.0, 0.0, 0.3),
        );

        // 1px border using overlay color
        fill_rounded_rect(
            pixmap,
            bounds.x - 1.0,
            list_y - 1.0,
            bounds.width + 2.0,
            list_h + 2.0,
            7.0,
            overlay,
        );

        // Solid background
        fill_rounded_rect(pixmap, bounds.x, list_y, bounds.width, list_h, 6.0, surface);

        for (i, option) in self.options.iter().enumerate() {
            let oy = list_y + i as f32 * Self::OPTION_H;
            let is_selected = i == self.selected;

            if is_selected {
                // 40% accent background
                fill_rounded_rect(
                    pixmap,
                    bounds.x,
                    oy,
                    bounds.width,
                    Self::OPTION_H,
                    0.0,
                    Color::new(accent.r, accent.g, accent.b, 0.4),
                );
                // Selection dot
                fill_rounded_rect(
                    pixmap,
                    bounds.x + 6.0,
                    oy + Self::OPTION_H / 2.0 - 3.0,
                    6.0,
                    6.0,
                    3.0,
                    accent,
                );
            }

            let text_oy = oy + (Self::OPTION_H - font_size * 1.3) / 2.0;
            let text_x = bounds.x + if is_selected { 20.0 } else { 8.0 };
            let color = if is_selected { accent } else { fg };
            ctx.text.draw_text(
                pixmap,
                option,
                text_x,
                text_oy,
                font_size,
                color,
                bounds.width - 24.0,
            );
        }
    }

    fn has_overlay(&self) -> bool {
        self.open
    }

    fn overlay_height(&self) -> f32 {
        self.options.len() as f32 * Self::OPTION_H
    }

    fn dismiss_overlay(&mut self) {
        self.open = false;
    }

    fn event(&mut self, event: &Event, bounds: &Rect) -> EventResponse {
        if let Event::PointerUp { x, y } = event {
            let header = Rect {
                x: bounds.x,
                y: bounds.y,
                width: bounds.width,
                height: Self::CLOSED_H,
            };
            if header.contains(*x, *y) {
                self.open = !self.open;
                return EventResponse {
                    consumed: true,
                    action: None,
                };
            }
            if self.open {
                let list_y = bounds.y + Self::CLOSED_H;
                for (i, _) in self.options.iter().enumerate() {
                    let oy = list_y + i as f32 * Self::OPTION_H;
                    let opt_rect = Rect {
                        x: bounds.x,
                        y: oy,
                        width: bounds.width,
                        height: Self::OPTION_H,
                    };
                    if opt_rect.contains(*x, *y) {
                        self.selected = i;
                        self.open = false;
                        return EventResponse {
                            consumed: true,
                            action: Some(WidgetAction::ValueChanged {
                                key: self.key.clone(),
                                value: self.options[i].clone(),
                            }),
                        };
                    }
                }
            }
        }
        EventResponse::ignored()
    }
}

/// A searchable dropdown with type-to-filter and an "Other..." manual entry.
///
/// `options` is a list of `(value, display_label)` pairs. The value stored and
/// emitted in `ValueChanged` is always the first element of the pair.
pub struct SearchableDropdown {
    pub options: Vec<(String, String)>,
    pub selected_value: String,
    pub key: String,
    pub open: bool,
    pub filter_text: String,
    pub filtered_indices: Vec<usize>,
    pub custom_mode: bool,
    pub custom_value: String,
    pub custom_cursor: usize,
}

impl SearchableDropdown {
    pub fn new(
        key: impl Into<String>,
        options: Vec<(String, String)>,
        initial_value: impl Into<String>,
    ) -> Self {
        let selected_value = initial_value.into();
        let n = options.len();
        let mut sd = Self {
            options,
            selected_value,
            key: key.into(),
            open: false,
            filter_text: String::new(),
            filtered_indices: (0..n).collect(),
            custom_mode: false,
            custom_value: String::new(),
            custom_cursor: 0,
        };
        sd.rebuild_filter();
        sd
    }

    fn display_label(&self) -> &str {
        for (value, label) in &self.options {
            if value == &self.selected_value {
                return label.as_str();
            }
        }
        self.selected_value.as_str()
    }

    fn rebuild_filter(&mut self) {
        let filter = self.filter_text.to_lowercase();
        self.filtered_indices = self
            .options
            .iter()
            .enumerate()
            .filter(|(_, (val, label))| {
                filter.is_empty()
                    || val.to_lowercase().contains(&filter)
                    || label.to_lowercase().contains(&filter)
            })
            .map(|(i, _)| i)
            .collect();
    }

    const CLOSED_H: f32 = 32.0;
    const FILTER_H: f32 = 32.0;
    const OPTION_H: f32 = 28.0;
    const MAX_VISIBLE: usize = 8;
}

impl Widget for SearchableDropdown {
    fn measure(&self, constraints: Constraints) -> Size {
        let width = 200.0_f32.clamp(constraints.min_width, constraints.max_width);
        let height = Self::CLOSED_H.clamp(constraints.min_height, constraints.max_height);
        Size { width, height }
    }

    fn paint(&self, pixmap: &mut PixmapMut<'_>, bounds: Rect, ctx: &mut RenderContext<'_>) {
        let surface = parse_color(
            &ctx.palette.colors.surface,
            Color::new(0.192, 0.196, 0.267, 1.0),
        );
        let fg = parse_color(
            &ctx.palette.colors.foreground,
            Color::new(0.804, 0.839, 0.957, 1.0),
        );
        let overlay = parse_color(
            &ctx.palette.colors.overlay,
            Color::new(0.424, 0.439, 0.525, 1.0),
        );
        let font_size = ctx.palette.fonts.size as f32;

        fill_rounded_rect(
            pixmap,
            bounds.x,
            bounds.y,
            bounds.width,
            Self::CLOSED_H,
            6.0,
            surface,
        );

        let label = self.display_label();
        let text_y = bounds.y + (Self::CLOSED_H - font_size * 1.3) / 2.0;
        ctx.text.draw_text(
            pixmap,
            label,
            bounds.x + 8.0,
            text_y,
            font_size,
            fg,
            bounds.width - 32.0,
        );

        let chevron_cx = bounds.x + bounds.width - 14.0;
        let chevron_cy = bounds.y + Self::CLOSED_H / 2.0;
        draw_chevron(pixmap, chevron_cx, chevron_cy, overlay);
    }

    fn paint_overlay(&self, pixmap: &mut PixmapMut<'_>, bounds: Rect, ctx: &mut RenderContext<'_>) {
        if !self.open {
            return;
        }
        let surface = parse_color(
            &ctx.palette.colors.surface,
            Color::new(0.192, 0.196, 0.267, 1.0),
        );
        let fg = parse_color(
            &ctx.palette.colors.foreground,
            Color::new(0.804, 0.839, 0.957, 1.0),
        );
        let accent = parse_color(
            &ctx.palette.colors.accent,
            Color::new(0.537, 0.706, 0.98, 1.0),
        );
        let overlay_color = parse_color(
            &ctx.palette.colors.overlay,
            Color::new(0.424, 0.439, 0.525, 1.0),
        );
        let font_size = ctx.palette.fonts.size as f32;

        let visible_count = self.filtered_indices.len().min(Self::MAX_VISIBLE);
        let panel_y = bounds.y + Self::CLOSED_H;
        // +1 row for "Other..." entry
        let panel_h = Self::FILTER_H + (visible_count + 1) as f32 * Self::OPTION_H;

        // Drop shadow
        fill_rounded_rect(
            pixmap,
            bounds.x + 2.0,
            panel_y + 2.0,
            bounds.width,
            panel_h,
            6.0,
            Color::new(0.0, 0.0, 0.0, 0.3),
        );

        // Border
        fill_rounded_rect(
            pixmap,
            bounds.x - 1.0,
            panel_y - 1.0,
            bounds.width + 2.0,
            panel_h + 2.0,
            7.0,
            overlay_color,
        );

        // Background
        fill_rounded_rect(
            pixmap,
            bounds.x,
            panel_y,
            bounds.width,
            panel_h,
            6.0,
            surface,
        );

        // Filter input row
        let filter_border = if !self.custom_mode {
            accent
        } else {
            overlay_color
        };
        fill_rounded_rect(
            pixmap,
            bounds.x + 4.0,
            panel_y + 4.0,
            bounds.width - 8.0,
            Self::FILTER_H - 8.0,
            4.0,
            filter_border,
        );
        fill_rounded_rect(
            pixmap,
            bounds.x + 5.0,
            panel_y + 5.0,
            bounds.width - 10.0,
            Self::FILTER_H - 10.0,
            3.0,
            surface,
        );
        let (filter_display, filter_fg) = if self.filter_text.is_empty() {
            ("Type to filter...", overlay_color)
        } else {
            (self.filter_text.as_str(), fg)
        };
        let filter_y = panel_y + (Self::FILTER_H - font_size * 1.3) / 2.0;
        ctx.text.draw_text(
            pixmap,
            filter_display,
            bounds.x + 12.0,
            filter_y,
            font_size,
            filter_fg,
            bounds.width - 24.0,
        );

        // Option rows
        let opts_start_y = panel_y + Self::FILTER_H;
        for (slot, &idx) in self
            .filtered_indices
            .iter()
            .take(Self::MAX_VISIBLE)
            .enumerate()
        {
            let (value, label) = &self.options[idx];
            let oy = opts_start_y + slot as f32 * Self::OPTION_H;
            let is_selected = *value == self.selected_value;

            if is_selected {
                fill_rounded_rect(
                    pixmap,
                    bounds.x,
                    oy,
                    bounds.width,
                    Self::OPTION_H,
                    0.0,
                    Color::new(accent.r, accent.g, accent.b, 0.4),
                );
                fill_rounded_rect(
                    pixmap,
                    bounds.x + 6.0,
                    oy + Self::OPTION_H / 2.0 - 3.0,
                    6.0,
                    6.0,
                    3.0,
                    accent,
                );
            }

            let text_oy = oy + (Self::OPTION_H - font_size * 1.3) / 2.0;
            let text_x = bounds.x + if is_selected { 20.0 } else { 8.0 };
            let color = if is_selected { accent } else { fg };
            ctx.text.draw_text(
                pixmap,
                label,
                text_x,
                text_oy,
                font_size,
                color,
                bounds.width - 24.0,
            );
        }

        // "Other..." row
        let other_slot = visible_count;
        let other_y = opts_start_y + other_slot as f32 * Self::OPTION_H;
        if self.custom_mode {
            fill_rounded_rect(
                pixmap,
                bounds.x,
                other_y,
                bounds.width,
                Self::OPTION_H,
                0.0,
                Color::new(0.0, 0.0, 0.0, 0.15),
            );
            let text_oy = other_y + (Self::OPTION_H - font_size * 1.3) / 2.0;
            ctx.text.draw_text(
                pixmap,
                &self.custom_value,
                bounds.x + 8.0,
                text_oy,
                font_size,
                fg,
                bounds.width - 24.0,
            );
        } else {
            let text_oy = other_y + (Self::OPTION_H - font_size * 1.3) / 2.0;
            ctx.text.draw_text(
                pixmap,
                "Other...",
                bounds.x + 8.0,
                text_oy,
                font_size,
                overlay_color,
                bounds.width - 24.0,
            );
        }
    }

    fn has_overlay(&self) -> bool {
        self.open
    }

    fn overlay_height(&self) -> f32 {
        if self.open {
            let visible_count = self.filtered_indices.len().min(Self::MAX_VISIBLE);
            Self::FILTER_H + (visible_count + 1) as f32 * Self::OPTION_H
        } else {
            0.0
        }
    }

    fn dismiss_overlay(&mut self) {
        self.open = false;
        self.filter_text.clear();
        self.custom_mode = false;
        self.rebuild_filter();
    }

    fn event(&mut self, event: &Event, bounds: &Rect) -> EventResponse {
        match event {
            Event::PointerUp { x, y } => {
                // Header click: toggle open/closed
                let header = Rect {
                    x: bounds.x,
                    y: bounds.y,
                    width: bounds.width,
                    height: Self::CLOSED_H,
                };
                if header.contains(*x, *y) {
                    if self.open {
                        self.dismiss_overlay();
                    } else {
                        self.open = true;
                    }
                    return EventResponse {
                        consumed: true,
                        action: None,
                    };
                }

                if self.open {
                    let panel_y = bounds.y + Self::CLOSED_H;
                    let opts_start_y = panel_y + Self::FILTER_H;
                    let visible_count = self.filtered_indices.len().min(Self::MAX_VISIBLE);

                    // Option row clicks
                    for slot in 0..visible_count {
                        let idx = self.filtered_indices[slot];
                        let oy = opts_start_y + slot as f32 * Self::OPTION_H;
                        let opt_rect = Rect {
                            x: bounds.x,
                            y: oy,
                            width: bounds.width,
                            height: Self::OPTION_H,
                        };
                        if opt_rect.contains(*x, *y) {
                            let (value, _) = &self.options[idx];
                            let emitted = value.clone();
                            self.selected_value = emitted.clone();
                            self.dismiss_overlay();
                            return EventResponse {
                                consumed: true,
                                action: Some(WidgetAction::ValueChanged {
                                    key: self.key.clone(),
                                    value: emitted,
                                }),
                            };
                        }
                    }

                    // "Other..." row click
                    let other_y = opts_start_y + visible_count as f32 * Self::OPTION_H;
                    let other_rect = Rect {
                        x: bounds.x,
                        y: other_y,
                        width: bounds.width,
                        height: Self::OPTION_H,
                    };
                    if other_rect.contains(*x, *y) {
                        if self.custom_mode {
                            // Commit custom value
                            let emitted = self.custom_value.clone();
                            self.selected_value = emitted.clone();
                            self.dismiss_overlay();
                            return EventResponse {
                                consumed: true,
                                action: Some(WidgetAction::ValueChanged {
                                    key: self.key.clone(),
                                    value: emitted,
                                }),
                            };
                        } else {
                            self.custom_mode = true;
                            self.custom_value = self.selected_value.clone();
                            self.custom_cursor = self.custom_value.chars().count();
                            return EventResponse {
                                consumed: true,
                                action: None,
                            };
                        }
                    }
                }
            }

            Event::KeyPress { key, utf8 } if self.open => match key.as_str() {
                "BackSpace" => {
                    if self.custom_mode {
                        if self.custom_cursor > 0 {
                            let byte_pos = self
                                .custom_value
                                .char_indices()
                                .nth(self.custom_cursor - 1)
                                .map(|(i, _)| i)
                                .unwrap_or(0);
                            self.custom_value.remove(byte_pos);
                            self.custom_cursor -= 1;
                        }
                    } else {
                        self.filter_text.pop();
                        self.rebuild_filter();
                    }
                    return EventResponse {
                        consumed: true,
                        action: None,
                    };
                }
                "Return" | "KP_Enter" => {
                    if self.custom_mode {
                        let emitted = self.custom_value.clone();
                        self.selected_value = emitted.clone();
                        self.dismiss_overlay();
                        return EventResponse {
                            consumed: true,
                            action: Some(WidgetAction::ValueChanged {
                                key: self.key.clone(),
                                value: emitted,
                            }),
                        };
                    } else if !self.filtered_indices.is_empty() {
                        let idx = self.filtered_indices[0];
                        let (value, _) = &self.options[idx];
                        let emitted = value.clone();
                        self.selected_value = emitted.clone();
                        self.dismiss_overlay();
                        return EventResponse {
                            consumed: true,
                            action: Some(WidgetAction::ValueChanged {
                                key: self.key.clone(),
                                value: emitted,
                            }),
                        };
                    }
                }
                _ => {
                    if let Some(s) = utf8 {
                        for ch in s.chars() {
                            if !ch.is_control() {
                                if self.custom_mode {
                                    let byte_pos = self
                                        .custom_value
                                        .char_indices()
                                        .nth(self.custom_cursor)
                                        .map(|(i, _)| i)
                                        .unwrap_or(self.custom_value.len());
                                    self.custom_value.insert(byte_pos, ch);
                                    self.custom_cursor += 1;
                                } else {
                                    self.filter_text.push(ch);
                                    self.rebuild_filter();
                                }
                            }
                        }
                        return EventResponse {
                            consumed: true,
                            action: None,
                        };
                    }
                }
            },

            _ => {}
        }
        EventResponse::ignored()
    }
}

/// A single-line text input field (200×32px).
pub struct TextInput {
    pub value: String,
    pub key: String,
    pub focused: bool,
    pub cursor_pos: usize,
}

impl TextInput {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            focused: false,
            cursor_pos: 0,
        }
    }
}

impl Widget for TextInput {
    fn measure(&self, constraints: Constraints) -> Size {
        let width = 200.0_f32.clamp(constraints.min_width, constraints.max_width);
        let height = 32.0_f32.clamp(constraints.min_height, constraints.max_height);
        Size { width, height }
    }

    fn paint(&self, pixmap: &mut PixmapMut<'_>, bounds: Rect, ctx: &mut RenderContext<'_>) {
        let surface = parse_color(
            &ctx.palette.colors.surface,
            Color::new(0.192, 0.196, 0.267, 1.0),
        );
        let fg = parse_color(
            &ctx.palette.colors.foreground,
            Color::new(0.804, 0.839, 0.957, 1.0),
        );
        let accent = parse_color(
            &ctx.palette.colors.accent,
            Color::new(0.537, 0.706, 0.98, 1.0),
        );
        let overlay = parse_color(
            &ctx.palette.colors.overlay,
            Color::new(0.424, 0.439, 0.525, 1.0),
        );
        let font_size = ctx.palette.fonts.size as f32;

        let border_color = if self.focused { accent } else { overlay };

        // Border + background (draw border by painting bg slightly inside border)
        fill_rounded_rect(
            pixmap,
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
            6.0,
            border_color,
        );
        fill_rounded_rect(
            pixmap,
            bounds.x + 1.0,
            bounds.y + 1.0,
            bounds.width - 2.0,
            bounds.height - 2.0,
            5.0,
            surface,
        );

        let text_y = bounds.y + (bounds.height - font_size * 1.3) / 2.0;
        ctx.text.draw_text(
            pixmap,
            &self.value,
            bounds.x + 8.0,
            text_y,
            font_size,
            fg,
            bounds.width - 24.0,
        );

        if self.focused {
            let char_w = font_size * 0.6;
            let cursor_chars = self.cursor_pos.min(self.value.chars().count());
            let cursor_x = bounds.x + 8.0 + cursor_chars as f32 * char_w;
            let cursor_h = font_size * 1.3;
            fill_rounded_rect(pixmap, cursor_x, text_y, 1.5, cursor_h, 0.0, fg);
        }
    }

    fn event(&mut self, event: &Event, bounds: &Rect) -> EventResponse {
        match event {
            Event::PointerDown { x, y } => {
                let was_focused = self.focused;
                self.focused = bounds.contains(*x, *y);
                if was_focused != self.focused {
                    return EventResponse {
                        consumed: self.focused,
                        action: None,
                    };
                }
            }
            Event::KeyPress { key, utf8 } if self.focused => {
                match key.as_str() {
                    "BackSpace" => {
                        if self.cursor_pos > 0 {
                            let byte_pos = self
                                .value
                                .char_indices()
                                .nth(self.cursor_pos - 1)
                                .map(|(i, _)| i)
                                .unwrap_or(0);
                            self.value.remove(byte_pos);
                            self.cursor_pos -= 1;
                        }
                    }
                    "Delete" => {
                        let char_count = self.value.chars().count();
                        if self.cursor_pos < char_count {
                            let byte_pos = self
                                .value
                                .char_indices()
                                .nth(self.cursor_pos)
                                .map(|(i, _)| i)
                                .unwrap_or(self.value.len());
                            self.value.remove(byte_pos);
                        }
                    }
                    "Left" => {
                        if self.cursor_pos > 0 {
                            self.cursor_pos -= 1;
                        }
                    }
                    "Right" => {
                        if self.cursor_pos < self.value.chars().count() {
                            self.cursor_pos += 1;
                        }
                    }
                    "Home" => self.cursor_pos = 0,
                    "End" => self.cursor_pos = self.value.chars().count(),
                    _ => {
                        if let Some(s) = utf8 {
                            for ch in s.chars() {
                                if !ch.is_control() {
                                    let byte_pos = self
                                        .value
                                        .char_indices()
                                        .nth(self.cursor_pos)
                                        .map(|(i, _)| i)
                                        .unwrap_or(self.value.len());
                                    self.value.insert(byte_pos, ch);
                                    self.cursor_pos += 1;
                                }
                            }
                        }
                    }
                }
                return EventResponse {
                    consumed: true,
                    action: Some(WidgetAction::ValueChanged {
                        key: self.key.clone(),
                        value: self.value.clone(),
                    }),
                };
            }
            _ => {}
        }
        EventResponse::ignored()
    }
}

/// A color preview and hex-string display (200×32px).
///
/// Clicking will eventually open an expanded picker; for now it shows the
/// preview square and hex string.
pub struct ColorPicker {
    pub value: Color,
    pub key: String,
}

impl ColorPicker {
    pub fn new(key: impl Into<String>, value: Color) -> Self {
        Self {
            key: key.into(),
            value,
        }
    }
}

impl Widget for ColorPicker {
    fn measure(&self, constraints: Constraints) -> Size {
        let width = 200.0_f32.clamp(constraints.min_width, constraints.max_width);
        let height = 32.0_f32.clamp(constraints.min_height, constraints.max_height);
        Size { width, height }
    }

    fn paint(&self, pixmap: &mut PixmapMut<'_>, bounds: Rect, ctx: &mut RenderContext<'_>) {
        let fg = parse_color(
            &ctx.palette.colors.foreground,
            Color::new(0.804, 0.839, 0.957, 1.0),
        );
        let font_size = ctx.palette.fonts.size as f32;

        let square_size = 24.0;
        let square_x = bounds.x + 4.0;
        let square_y = bounds.y + (bounds.height - square_size) / 2.0;

        fill_rounded_rect(
            pixmap,
            square_x,
            square_y,
            square_size,
            square_size,
            4.0,
            self.value,
        );

        let hex = self.value.to_hex();
        let text_x = square_x + square_size + 8.0;
        let text_y = bounds.y + (bounds.height - font_size * 1.3) / 2.0;
        let text_max_w = (bounds.x + bounds.width) - text_x;
        ctx.text.draw_text(
            pixmap,
            &hex,
            text_x,
            text_y,
            font_size,
            fg,
            text_max_w.max(0.0),
        );
    }
}

/// A labelled section divider with a separator line (full-width × 36px).
pub struct SectionHeader {
    pub title: String,
}

impl SectionHeader {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
        }
    }
}

impl Widget for SectionHeader {
    fn measure(&self, constraints: Constraints) -> Size {
        let width = constraints.max_width;
        let height = 36.0_f32.clamp(constraints.min_height, constraints.max_height);
        Size { width, height }
    }

    fn paint(&self, pixmap: &mut PixmapMut<'_>, bounds: Rect, ctx: &mut RenderContext<'_>) {
        let fg = parse_color(
            &ctx.palette.colors.foreground,
            Color::new(0.804, 0.839, 0.957, 1.0),
        );
        let overlay = parse_color(
            &ctx.palette.colors.overlay,
            Color::new(0.424, 0.439, 0.525, 1.0),
        );
        let font_size = ctx.palette.fonts.size as f32 + 2.0;

        let text_y = bounds.y + (bounds.height - 1.0 - font_size * 1.3) / 2.0;
        ctx.text.draw_text(
            pixmap,
            &self.title,
            bounds.x,
            text_y,
            font_size,
            fg,
            bounds.width,
        );

        // Separator line at the bottom
        fill_rounded_rect(
            pixmap,
            bounds.x,
            bounds.y + bounds.height - 1.0,
            bounds.width,
            1.0,
            0.0,
            overlay,
        );
    }
}

// ---------------------------------------------------------------------------
// ScrollArea
// ---------------------------------------------------------------------------

/// Tracks scroll state for a scrollable content region.
pub struct ScrollArea {
    pub scroll_offset: f32,
    pub content_height: f32,
    pub viewport_height: f32,
}

impl ScrollArea {
    pub fn new(viewport_height: f32) -> Self {
        Self {
            scroll_offset: 0.0,
            content_height: 0.0,
            viewport_height,
        }
    }

    /// Scroll by `dy` pixels, clamped to valid range.
    pub fn scroll_by(&mut self, dy: f32) {
        let max = (self.content_height - self.viewport_height).max(0.0);
        self.scroll_offset = (self.scroll_offset + dy).clamp(0.0, max);
    }

    /// Return `(top, bottom)` of the currently visible region in content space.
    pub fn visible_range(&self) -> (f32, f32) {
        (
            self.scroll_offset,
            self.scroll_offset + self.viewport_height,
        )
    }

    /// Draw the scrollbar track and thumb if content overflows the viewport.
    pub fn paint_scrollbar(&self, bounds: Rect, pixmap: &mut PixmapMut<'_>, palette: &Palette) {
        if self.content_height <= self.viewport_height {
            return;
        }

        let overlay = parse_color(
            &palette.colors.overlay,
            Color::new(0.424, 0.439, 0.525, 0.5),
        );
        let accent = parse_color(&palette.colors.accent, Color::new(0.537, 0.706, 0.98, 1.0));

        let sb_w = 6.0;
        let sb_x = bounds.x + bounds.width - sb_w;

        // Track
        fill_rounded_rect(
            pixmap,
            sb_x,
            bounds.y,
            sb_w,
            bounds.height,
            sb_w / 2.0,
            overlay,
        );

        // Thumb
        let thumb_h = (self.viewport_height / self.content_height * bounds.height).max(20.0);
        let max_scroll = (self.content_height - self.viewport_height).max(1.0);
        let thumb_y =
            bounds.y + self.scroll_offset / max_scroll * (bounds.height - thumb_h).max(0.0);
        fill_rounded_rect(pixmap, sb_x, thumb_y, sb_w, thumb_h, sb_w / 2.0, accent);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_measure_approximate() {
        let label = Label::new("hello", 14.0);
        let size = label.measure(Constraints::loose(400.0, 200.0));
        assert!(size.width > 0.0);
        assert!(size.height > 0.0);
    }

    #[test]
    fn toggle_measure() {
        let toggle = Toggle::new("key", false);
        let size = toggle.measure(Constraints::loose(200.0, 100.0));
        assert_eq!(size.width, 44.0);
        assert_eq!(size.height, 24.0);
    }

    #[test]
    fn toggle_event_toggles_value() {
        let mut toggle = Toggle::new("brightness", false);
        let bounds = Rect {
            x: 0.0,
            y: 0.0,
            width: 44.0,
            height: 24.0,
        };
        let resp = toggle.event(&Event::PointerUp { x: 22.0, y: 12.0 }, &bounds);
        assert!(resp.consumed);
        assert!(toggle.value);
    }

    #[test]
    fn slider_measure() {
        let slider = Slider::new("vol", 0.5, 0.0, 1.0, 0.0);
        let size = slider.measure(Constraints::loose(400.0, 100.0));
        assert_eq!(size.width, 200.0);
        assert_eq!(size.height, 32.0);
    }

    #[test]
    fn slider_event_updates_value() {
        let mut slider = Slider::new("vol", 0.0, 0.0, 100.0, 1.0);
        let bounds = Rect {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 32.0,
        };
        let resp = slider.event(&Event::PointerUp { x: 58.0, y: 16.0 }, &bounds);
        assert!(resp.consumed);
        assert!((slider.value - 50.0).abs() < 1.0);
    }

    #[test]
    fn slider_text_entry_updates_numeric_value() {
        let mut slider = Slider::new("scale", 1.0, 0.5, 3.0, 0.25);
        let bounds = Rect {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 32.0,
        };
        slider.event(&Event::PointerDown { x: 170.0, y: 16.0 }, &bounds);
        for _ in 0..3 {
            slider.event(
                &Event::KeyPress {
                    key: "BackSpace".into(),
                    utf8: None,
                },
                &bounds,
            );
        }
        slider.event(
            &Event::KeyPress {
                key: "2".into(),
                utf8: Some("2".into()),
            },
            &bounds,
        );
        slider.event(
            &Event::KeyPress {
                key: ".".into(),
                utf8: Some(".".into()),
            },
            &bounds,
        );
        let response = slider.event(
            &Event::KeyPress {
                key: "5".into(),
                utf8: Some("5".into()),
            },
            &bounds,
        );
        assert!((slider.value - 2.5).abs() < f64::EPSILON);
        assert!(matches!(
            response.action,
            Some(WidgetAction::ValueChanged { value, .. }) if value == "2.5"
        ));
    }

    #[test]
    fn dropdown_toggles_open() {
        let mut dd = Dropdown::new("layout", vec!["dwindle".into(), "master".into()], 0);
        let bounds = Rect {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 32.0,
        };
        dd.event(&Event::PointerUp { x: 50.0, y: 16.0 }, &bounds);
        assert!(dd.open);
    }

    #[test]
    fn text_input_inserts_text() {
        let mut input = TextInput::new("name", "");
        input.focused = true;
        let bounds = Rect {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 32.0,
        };
        input.event(
            &Event::KeyPress {
                key: "a".into(),
                utf8: Some("a".into()),
            },
            &bounds,
        );
        assert_eq!(input.value, "a");
        assert_eq!(input.cursor_pos, 1);
    }

    #[test]
    fn text_input_backspace() {
        let mut input = TextInput::new("name", "hello");
        input.focused = true;
        input.cursor_pos = 5;
        let bounds = Rect {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 32.0,
        };
        input.event(
            &Event::KeyPress {
                key: "BackSpace".into(),
                utf8: None,
            },
            &bounds,
        );
        assert_eq!(input.value, "hell");
        assert_eq!(input.cursor_pos, 4);
    }

    #[test]
    fn scroll_area_clamps() {
        let mut sa = ScrollArea::new(100.0);
        sa.content_height = 300.0;
        sa.scroll_by(500.0);
        assert_eq!(sa.scroll_offset, 200.0);
        sa.scroll_by(-999.0);
        assert_eq!(sa.scroll_offset, 0.0);
    }

    #[test]
    fn scroll_area_visible_range() {
        let mut sa = ScrollArea::new(100.0);
        sa.content_height = 300.0;
        sa.scroll_by(50.0);
        let (top, bottom) = sa.visible_range();
        assert_eq!(top, 50.0);
        assert_eq!(bottom, 150.0);
    }

    #[test]
    fn color_picker_new() {
        let cp = ColorPicker::new("bg", Color::new(1.0, 0.0, 0.0, 1.0));
        assert_eq!(cp.key, "bg");
        let size = cp.measure(Constraints::loose(400.0, 100.0));
        assert_eq!(size.width, 200.0);
        assert_eq!(size.height, 32.0);
    }

    #[test]
    fn section_header_height() {
        let h = SectionHeader::new("General");
        let size = h.measure(Constraints::loose(400.0, 100.0));
        assert_eq!(size.height, 36.0);
    }
}
