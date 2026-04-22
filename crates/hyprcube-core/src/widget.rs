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
    fn paint(&self, pixmap: &mut PixmapMut<'_>, bounds: Rect, ctx: &mut RenderContext<'_>);

    /// Handle an input event. The default implementation ignores all events.
    fn event(&mut self, _event: &Event, _bounds: &Rect) -> EventResponse {
        EventResponse::ignored()
    }
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
        ctx.text
            .draw_text(pixmap, &self.text, bounds.x, bounds.y, self.font_size, color, bounds.width);
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
            parse_color(&ctx.palette.colors.accent, Color::new(0.537, 0.706, 0.98, 1.0))
        } else {
            parse_color(&ctx.palette.colors.overlay, Color::new(0.424, 0.439, 0.525, 1.0))
        };
        let thumb_color = if self.value {
            Color::new(1.0, 1.0, 1.0, 1.0)
        } else {
            parse_color(&ctx.palette.colors.surface, Color::new(0.192, 0.196, 0.267, 1.0))
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
}

impl Slider {
    pub fn new(key: impl Into<String>, value: f64, min: f64, max: f64, step: f64) -> Self {
        Self {
            key: key.into(),
            value,
            min,
            max,
            step,
        }
    }
}

impl Widget for Slider {
    fn measure(&self, constraints: Constraints) -> Size {
        let width = 200.0_f32.clamp(constraints.min_width, constraints.max_width);
        let height = 32.0_f32.clamp(constraints.min_height, constraints.max_height);
        Size { width, height }
    }

    fn paint(&self, pixmap: &mut PixmapMut<'_>, bounds: Rect, ctx: &mut RenderContext<'_>) {
        let surface = parse_color(&ctx.palette.colors.surface, Color::new(0.192, 0.196, 0.267, 1.0));
        let accent = parse_color(&ctx.palette.colors.accent, Color::new(0.537, 0.706, 0.98, 1.0));

        let track_h = 4.0;
        let thumb_size = 16.0;
        let track_y = bounds.y + (bounds.height - track_h) / 2.0;

        // Track background
        fill_rounded_rect(pixmap, bounds.x, track_y, bounds.width, track_h, track_h / 2.0, surface);

        let range = (self.max - self.min) as f32;
        let fraction = if range > 0.0 {
            ((self.value - self.min) as f32 / range).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let fill_w = fraction * bounds.width;

        // Filled portion
        if fill_w > 0.0 {
            fill_rounded_rect(pixmap, bounds.x, track_y, fill_w, track_h, track_h / 2.0, accent);
        }

        // Thumb circle
        let thumb_x = bounds.x + fill_w - thumb_size / 2.0;
        let thumb_y = bounds.y + (bounds.height - thumb_size) / 2.0;
        fill_rounded_rect(pixmap, thumb_x, thumb_y, thumb_size, thumb_size, thumb_size / 2.0, accent);
    }

    fn event(&mut self, event: &Event, bounds: &Rect) -> EventResponse {
        let x = match event {
            Event::PointerDown { x, y } | Event::PointerUp { x, y } if bounds.contains(*x, *y) => *x,
            _ => return EventResponse::ignored(),
        };
        let fraction = ((x - bounds.x) / bounds.width).clamp(0.0, 1.0) as f64;
        let raw = self.min + (self.max - self.min) * fraction;
        let stepped = if self.step > 0.0 {
            (raw / self.step).round() * self.step
        } else {
            raw
        };
        self.value = stepped.clamp(self.min, self.max);
        EventResponse {
            consumed: true,
            action: Some(WidgetAction::ValueChanged {
                key: self.key.clone(),
                value: self.value.to_string(),
            }),
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
        let h = if self.open {
            Self::CLOSED_H + self.options.len() as f32 * Self::OPTION_H
        } else {
            Self::CLOSED_H
        };
        let width = 200.0_f32.clamp(constraints.min_width, constraints.max_width);
        let height = h.clamp(constraints.min_height, constraints.max_height);
        Size { width, height }
    }

    fn paint(&self, pixmap: &mut PixmapMut<'_>, bounds: Rect, ctx: &mut RenderContext<'_>) {
        let surface = parse_color(&ctx.palette.colors.surface, Color::new(0.192, 0.196, 0.267, 1.0));
        let fg = parse_color(&ctx.palette.colors.foreground, Color::new(0.804, 0.839, 0.957, 1.0));
        let accent = parse_color(&ctx.palette.colors.accent, Color::new(0.537, 0.706, 0.98, 1.0));
        let overlay = parse_color(&ctx.palette.colors.overlay, Color::new(0.424, 0.439, 0.525, 1.0));
        let font_size = ctx.palette.fonts.size as f32;

        // Closed header box
        fill_rounded_rect(pixmap, bounds.x, bounds.y, bounds.width, Self::CLOSED_H, 6.0, surface);

        // Selected text
        let selected_text = self.options.get(self.selected).map(|s| s.as_str()).unwrap_or("");
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

        // Chevron arrow
        let chevron_cx = bounds.x + bounds.width - 14.0;
        let chevron_cy = bounds.y + Self::CLOSED_H / 2.0;
        draw_chevron(pixmap, chevron_cx, chevron_cy, overlay);

        if self.open {
            let dropdown_y = bounds.y + Self::CLOSED_H;
            let list_h = self.options.len() as f32 * Self::OPTION_H;
            fill_rounded_rect(pixmap, bounds.x, dropdown_y, bounds.width, list_h, 6.0, surface);

            for (i, option) in self.options.iter().enumerate() {
                let oy = dropdown_y + i as f32 * Self::OPTION_H;
                let text_oy = oy + (Self::OPTION_H - font_size * 1.3) / 2.0;
                let color = if i == self.selected { accent } else { fg };
                ctx.text.draw_text(pixmap, option, bounds.x + 8.0, text_oy, font_size, color, bounds.width - 16.0);
            }
        }
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
                return EventResponse { consumed: true, action: None };
            }
            if self.open {
                let dropdown_y = bounds.y + Self::CLOSED_H;
                for (i, _) in self.options.iter().enumerate() {
                    let oy = dropdown_y + i as f32 * Self::OPTION_H;
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
        let surface = parse_color(&ctx.palette.colors.surface, Color::new(0.192, 0.196, 0.267, 1.0));
        let fg = parse_color(&ctx.palette.colors.foreground, Color::new(0.804, 0.839, 0.957, 1.0));
        let accent = parse_color(&ctx.palette.colors.accent, Color::new(0.537, 0.706, 0.98, 1.0));
        let overlay = parse_color(&ctx.palette.colors.overlay, Color::new(0.424, 0.439, 0.525, 1.0));
        let font_size = ctx.palette.fonts.size as f32;

        let border_color = if self.focused { accent } else { overlay };

        // Border + background (draw border by painting bg slightly inside border)
        fill_rounded_rect(pixmap, bounds.x, bounds.y, bounds.width, bounds.height, 6.0, border_color);
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
        ctx.text
            .draw_text(pixmap, &self.value, bounds.x + 8.0, text_y, font_size, fg, bounds.width - 24.0);

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
                    return EventResponse { consumed: self.focused, action: None };
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
        let fg = parse_color(&ctx.palette.colors.foreground, Color::new(0.804, 0.839, 0.957, 1.0));
        let font_size = ctx.palette.fonts.size as f32;

        let square_size = 24.0;
        let square_x = bounds.x + 4.0;
        let square_y = bounds.y + (bounds.height - square_size) / 2.0;

        fill_rounded_rect(pixmap, square_x, square_y, square_size, square_size, 4.0, self.value);

        let hex = self.value.to_hex();
        let text_x = square_x + square_size + 8.0;
        let text_y = bounds.y + (bounds.height - font_size * 1.3) / 2.0;
        let text_max_w = (bounds.x + bounds.width) - text_x;
        ctx.text.draw_text(pixmap, &hex, text_x, text_y, font_size, fg, text_max_w.max(0.0));
    }
}

/// A labelled section divider with a separator line (full-width × 36px).
pub struct SectionHeader {
    pub title: String,
}

impl SectionHeader {
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into() }
    }
}

impl Widget for SectionHeader {
    fn measure(&self, constraints: Constraints) -> Size {
        let width = constraints.max_width;
        let height = 36.0_f32.clamp(constraints.min_height, constraints.max_height);
        Size { width, height }
    }

    fn paint(&self, pixmap: &mut PixmapMut<'_>, bounds: Rect, ctx: &mut RenderContext<'_>) {
        let fg = parse_color(&ctx.palette.colors.foreground, Color::new(0.804, 0.839, 0.957, 1.0));
        let overlay =
            parse_color(&ctx.palette.colors.overlay, Color::new(0.424, 0.439, 0.525, 1.0));
        let font_size = ctx.palette.fonts.size as f32 + 2.0;

        let text_y = bounds.y + (bounds.height - 1.0 - font_size * 1.3) / 2.0;
        ctx.text.draw_text(pixmap, &self.title, bounds.x, text_y, font_size, fg, bounds.width);

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
        (self.scroll_offset, self.scroll_offset + self.viewport_height)
    }

    /// Draw the scrollbar track and thumb if content overflows the viewport.
    pub fn paint_scrollbar(&self, bounds: Rect, pixmap: &mut PixmapMut<'_>, palette: &Palette) {
        if self.content_height <= self.viewport_height {
            return;
        }

        let overlay = parse_color(&palette.colors.overlay, Color::new(0.424, 0.439, 0.525, 0.5));
        let accent = parse_color(&palette.colors.accent, Color::new(0.537, 0.706, 0.98, 1.0));

        let sb_w = 6.0;
        let sb_x = bounds.x + bounds.width - sb_w;

        // Track
        fill_rounded_rect(pixmap, sb_x, bounds.y, sb_w, bounds.height, sb_w / 2.0, overlay);

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
        let bounds = Rect { x: 0.0, y: 0.0, width: 44.0, height: 24.0 };
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
        let bounds = Rect { x: 0.0, y: 0.0, width: 200.0, height: 32.0 };
        let resp = slider.event(&Event::PointerUp { x: 100.0, y: 16.0 }, &bounds);
        assert!(resp.consumed);
        assert!((slider.value - 50.0).abs() < 1.0);
    }

    #[test]
    fn dropdown_toggles_open() {
        let mut dd = Dropdown::new("layout", vec!["dwindle".into(), "master".into()], 0);
        let bounds = Rect { x: 0.0, y: 0.0, width: 200.0, height: 32.0 };
        dd.event(&Event::PointerUp { x: 50.0, y: 16.0 }, &bounds);
        assert!(dd.open);
    }

    #[test]
    fn text_input_inserts_text() {
        let mut input = TextInput::new("name", "");
        input.focused = true;
        let bounds = Rect { x: 0.0, y: 0.0, width: 200.0, height: 32.0 };
        input.event(
            &Event::KeyPress { key: "a".into(), utf8: Some("a".into()) },
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
        let bounds = Rect { x: 0.0, y: 0.0, width: 200.0, height: 32.0 };
        input.event(&Event::KeyPress { key: "BackSpace".into(), utf8: None }, &bounds);
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
