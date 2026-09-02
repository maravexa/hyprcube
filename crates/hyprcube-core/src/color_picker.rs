use tiny_skia::{
    GradientStop, LinearGradient, Paint, PathBuilder, PixmapMut, SpreadMode, Transform,
};

use crate::color::Color;
use crate::layout::Rect;
use crate::render::fill_rounded_rect;
use crate::text::RenderContext;
use crate::widget::{Constraints, Event, EventResponse, Size, Widget, WidgetAction};

// Layout constants (all in logical pixels)
const COLLAPSED_H: f32 = 32.0;
const SV_W: f32 = 160.0;
const SV_H: f32 = 120.0;
const BAR_W: f32 = 160.0;
const BAR_H: f32 = 16.0;
const GAP: f32 = 6.0;
const H_PAD: f32 = 4.0;

/// Total height of the picker in expanded state.
pub const EXPANDED_H: f32 =
    COLLAPSED_H + GAP + SV_H + GAP + BAR_H + GAP + BAR_H + GAP + COLLAPSED_H;

/// Height of the overlay portion (everything below the collapsed header).
const OVERLAY_H: f32 = EXPANDED_H - COLLAPSED_H;

#[derive(Debug, Clone, Copy, PartialEq)]
enum DragTarget {
    None,
    SvSquare,
    HueBar,
    AlphaBar,
}

/// An inline color picker that collapses to a swatch + hex string and expands
/// to reveal a full HSV editor with hue, saturation/value, and alpha controls.
///
/// The expanded panel is rendered via the overlay system (`paint_overlay`) so
/// it floats above other widgets in `FormLayout`, or can be drawn inline by
/// calling `paint_overlay` right after `paint` (as `AppearancePanel` does).
pub struct InlineColorPicker {
    pub value: Color,
    pub key: String,
    pub expanded: bool,

    hue: f32,
    sat: f32,
    val: f32,

    hex_input: String,
    hex_focused: bool,
    dragging: DragTarget,
}

impl InlineColorPicker {
    pub fn new(key: impl Into<String>, value: Color) -> Self {
        let (hue, sat, val) = value.to_hsv();
        let hex_input = value.to_hex();
        Self {
            value,
            key: key.into(),
            expanded: false,
            hue,
            sat,
            val,
            hex_input,
            hex_focused: false,
            dragging: DragTarget::None,
        }
    }

    /// Update the color from an external source (e.g. preset change).
    pub fn set_color(&mut self, color: Color) {
        self.value = color;
        let (h, s, v) = color.to_hsv();
        self.hue = h;
        self.sat = s;
        self.val = v;
        self.hex_input = color.to_hex();
    }

    fn sync_from_hsv(&mut self) {
        self.value = Color::from_hsv(self.hue, self.sat, self.val, self.value.a);
        self.hex_input = self.value.to_hex();
    }

    fn apply_hex(&mut self) -> bool {
        match Color::from_hex(&self.hex_input) {
            Ok(c) => {
                self.value = c;
                let (h, s, v) = c.to_hsv();
                self.hue = h;
                self.sat = s;
                self.val = v;
                true
            }
            Err(_) => false,
        }
    }

    // -----------------------------------------------------------------------
    // Sub-region rects — all computed relative to the HEADER bounds
    // (bounds.x, bounds.y, bounds.width, COLLAPSED_H).
    // The overlay content starts at bounds.y + COLLAPSED_H.
    // -----------------------------------------------------------------------

    fn sv_rect(&self, bounds: &Rect) -> Rect {
        Rect {
            x: bounds.x + H_PAD,
            y: bounds.y + COLLAPSED_H + GAP,
            width: SV_W,
            height: SV_H,
        }
    }

    fn hue_rect(&self, bounds: &Rect) -> Rect {
        Rect {
            x: bounds.x + H_PAD,
            y: bounds.y + COLLAPSED_H + GAP + SV_H + GAP,
            width: BAR_W,
            height: BAR_H,
        }
    }

    fn alpha_rect(&self, bounds: &Rect) -> Rect {
        Rect {
            x: bounds.x + H_PAD,
            y: bounds.y + COLLAPSED_H + GAP + SV_H + GAP + BAR_H + GAP,
            width: BAR_W,
            height: BAR_H,
        }
    }

    fn hex_row_rect(&self, bounds: &Rect) -> Rect {
        Rect {
            x: bounds.x + H_PAD,
            y: bounds.y + COLLAPSED_H + GAP + SV_H + GAP + BAR_H + GAP + BAR_H + GAP,
            width: bounds.width - H_PAD * 2.0,
            height: COLLAPSED_H,
        }
    }

    // -----------------------------------------------------------------------
    // Painting helpers
    // -----------------------------------------------------------------------

    fn paint_sv_square(&self, pixmap: &mut PixmapMut<'_>, r: &Rect) {
        let hue_color = Color::from_hsv(self.hue, 1.0, 1.0, 1.0).to_skia();
        let white =
            tiny_skia::Color::from_rgba(1.0, 1.0, 1.0, 1.0).unwrap_or(tiny_skia::Color::WHITE);
        let black_t =
            tiny_skia::Color::from_rgba(0.0, 0.0, 0.0, 0.0).unwrap_or(tiny_skia::Color::BLACK);
        let black_o =
            tiny_skia::Color::from_rgba(0.0, 0.0, 0.0, 1.0).unwrap_or(tiny_skia::Color::BLACK);

        let skia_rect = match tiny_skia::Rect::from_xywh(r.x, r.y, r.width, r.height) {
            Some(rect) => rect,
            None => return,
        };

        // Pass 1: horizontal gradient white → hue_color (saturation 0→1)
        if let Some(shader) = LinearGradient::new(
            tiny_skia::Point::from_xy(r.x, r.y),
            tiny_skia::Point::from_xy(r.x + r.width, r.y),
            vec![
                GradientStop::new(0.0, white),
                GradientStop::new(1.0, hue_color),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        ) {
            let paint = Paint {
                shader,
                ..Paint::default()
            };
            pixmap.fill_rect(skia_rect, &paint, Transform::identity(), None);
        }

        // Pass 2: vertical overlay transparent → black (value 1→0)
        if let Some(shader) = LinearGradient::new(
            tiny_skia::Point::from_xy(r.x, r.y),
            tiny_skia::Point::from_xy(r.x, r.y + r.height),
            vec![
                GradientStop::new(0.0, black_t),
                GradientStop::new(1.0, black_o),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        ) {
            let paint = Paint {
                shader,
                ..Paint::default()
            };
            pixmap.fill_rect(skia_rect, &paint, Transform::identity(), None);
        }

        // Crosshair at current sat/val position
        let cx = r.x + self.sat * r.width;
        let cy = r.y + (1.0 - self.val) * r.height;
        let cr = 5.0;
        draw_circle_outline(pixmap, cx, cy, cr + 1.0, Color::new(1.0, 1.0, 1.0, 0.9));
        draw_circle_outline(pixmap, cx, cy, cr, Color::new(0.0, 0.0, 0.0, 0.7));
    }

    fn paint_hue_bar(&self, pixmap: &mut PixmapMut<'_>, r: &Rect) {
        let skia_rect = match tiny_skia::Rect::from_xywh(r.x, r.y, r.width, r.height) {
            Some(rect) => rect,
            None => return,
        };

        let stops: Vec<GradientStop> = [0.0, 60.0, 120.0, 180.0, 240.0, 300.0, 360.0]
            .iter()
            .map(|&h| {
                let pos = h / 360.0;
                GradientStop::new(pos, Color::from_hsv(h, 1.0, 1.0, 1.0).to_skia())
            })
            .collect();

        if let Some(shader) = LinearGradient::new(
            tiny_skia::Point::from_xy(r.x, r.y),
            tiny_skia::Point::from_xy(r.x + r.width, r.y),
            stops,
            SpreadMode::Pad,
            Transform::identity(),
        ) {
            let paint = Paint {
                shader,
                ..Paint::default()
            };
            pixmap.fill_rect(skia_rect, &paint, Transform::identity(), None);
        }

        // Hue indicator: a thin vertical line
        let ix = r.x + (self.hue / 360.0) * r.width;
        fill_rounded_rect(
            pixmap,
            ix - 1.0,
            r.y - 2.0,
            2.0,
            r.height + 4.0,
            1.0,
            Color::new(1.0, 1.0, 1.0, 1.0),
        );
    }

    fn paint_alpha_bar(&self, pixmap: &mut PixmapMut<'_>, r: &Rect) {
        // Checkerboard background
        let sq = 4.0_f32;
        let cols = (r.width / sq).ceil() as u32;
        let rows = (r.height / sq).ceil() as u32;
        let light = Color::new(0.8, 0.8, 0.8, 1.0);
        let dark = Color::new(0.55, 0.55, 0.55, 1.0);

        for row in 0..rows {
            for col in 0..cols {
                let color = if (row + col) % 2 == 0 { light } else { dark };
                let cx = r.x + col as f32 * sq;
                let cy = r.y + row as f32 * sq;
                let cw = sq.min(r.x + r.width - cx);
                let ch = sq.min(r.y + r.height - cy);
                if cw > 0.0 && ch > 0.0 {
                    fill_rounded_rect(pixmap, cx, cy, cw, ch, 0.0, color);
                }
            }
        }

        // Alpha gradient: transparent → opaque
        let skia_rect = match tiny_skia::Rect::from_xywh(r.x, r.y, r.width, r.height) {
            Some(rect) => rect,
            None => return,
        };
        let color_t = tiny_skia::Color::from_rgba(self.value.r, self.value.g, self.value.b, 0.0)
            .unwrap_or(tiny_skia::Color::TRANSPARENT);
        let color_o = tiny_skia::Color::from_rgba(self.value.r, self.value.g, self.value.b, 1.0)
            .unwrap_or(tiny_skia::Color::BLACK);

        if let Some(shader) = LinearGradient::new(
            tiny_skia::Point::from_xy(r.x, r.y),
            tiny_skia::Point::from_xy(r.x + r.width, r.y),
            vec![
                GradientStop::new(0.0, color_t),
                GradientStop::new(1.0, color_o),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        ) {
            let paint = Paint {
                shader,
                ..Paint::default()
            };
            pixmap.fill_rect(skia_rect, &paint, Transform::identity(), None);
        }

        // Alpha indicator line
        let ix = r.x + self.value.a * r.width;
        fill_rounded_rect(
            pixmap,
            ix - 1.0,
            r.y - 2.0,
            2.0,
            r.height + 4.0,
            1.0,
            Color::new(1.0, 1.0, 1.0, 1.0),
        );
    }

    /// Paint the expanded overlay body starting at `bounds.y + COLLAPSED_H`.
    fn paint_expanded(
        &self,
        pixmap: &mut PixmapMut<'_>,
        bounds: &Rect,
        ctx: &mut RenderContext<'_>,
    ) {
        let surface = Color::from_hex(&ctx.palette.colors.surface)
            .unwrap_or(Color::new(0.192, 0.196, 0.267, 1.0));
        let fg = Color::from_hex(&ctx.palette.colors.foreground)
            .unwrap_or(Color::new(0.804, 0.839, 0.957, 1.0));
        let overlay_color = Color::from_hex(&ctx.palette.colors.overlay)
            .unwrap_or(Color::new(0.424, 0.439, 0.525, 1.0));
        let accent = Color::from_hex(&ctx.palette.colors.accent)
            .unwrap_or(Color::new(0.537, 0.706, 0.98, 1.0));
        let font_size = ctx.palette.fonts.size as f32;

        let body_y = bounds.y + COLLAPSED_H;
        let body_h = OVERLAY_H;
        let body_x = bounds.x;
        let body_w = bounds.width;

        // Drop shadow
        fill_rounded_rect(
            pixmap,
            body_x + 2.0,
            body_y + 2.0,
            body_w,
            body_h,
            6.0,
            Color::new(0.0, 0.0, 0.0, 0.3),
        );

        // Border (1px via overlay color)
        fill_rounded_rect(
            pixmap,
            body_x - 1.0,
            body_y - 1.0,
            body_w + 2.0,
            body_h + 2.0,
            7.0,
            overlay_color,
        );

        // Solid surface background
        fill_rounded_rect(pixmap, body_x, body_y, body_w, body_h, 6.0, surface);

        let sv = self.sv_rect(bounds);
        let hue_r = self.hue_rect(bounds);
        let alpha_r = self.alpha_rect(bounds);
        let hex_r = self.hex_row_rect(bounds);

        // SV square
        self.paint_sv_square(pixmap, &sv);

        // Hue bar
        fill_rounded_rect(
            pixmap,
            hue_r.x,
            hue_r.y,
            hue_r.width,
            hue_r.height,
            4.0,
            overlay_color,
        );
        self.paint_hue_bar(pixmap, &hue_r);

        // Alpha bar
        fill_rounded_rect(
            pixmap,
            alpha_r.x,
            alpha_r.y,
            alpha_r.width,
            alpha_r.height,
            4.0,
            overlay_color,
        );
        self.paint_alpha_bar(pixmap, &alpha_r);

        // Hex input
        let hex_input_w = hex_r.width - 64.0;
        let hex_border = if self.hex_focused {
            accent
        } else {
            overlay_color
        };

        fill_rounded_rect(
            pixmap,
            hex_r.x,
            hex_r.y,
            hex_input_w,
            hex_r.height,
            6.0,
            hex_border,
        );
        fill_rounded_rect(
            pixmap,
            hex_r.x + 1.0,
            hex_r.y + 1.0,
            hex_input_w - 2.0,
            hex_r.height - 2.0,
            5.0,
            surface,
        );
        let hex_text_y = hex_r.y + (hex_r.height - font_size * 1.3) / 2.0;
        ctx.text.draw_text(
            pixmap,
            &self.hex_input,
            hex_r.x + 8.0,
            hex_text_y,
            font_size,
            fg,
            hex_input_w - 16.0,
        );

        // Apply button
        let btn_x = hex_r.x + hex_input_w + 8.0;
        let btn_w = 54.0;
        fill_rounded_rect(pixmap, btn_x, hex_r.y, btn_w, hex_r.height, 6.0, accent);
        let btn_text_y = hex_r.y + (hex_r.height - font_size * 1.3) / 2.0;
        ctx.text.draw_text(
            pixmap,
            "Apply",
            btn_x + 6.0,
            btn_text_y,
            font_size,
            Color::new(1.0, 1.0, 1.0, 1.0),
            btn_w,
        );
    }
}

fn draw_circle_outline(pixmap: &mut PixmapMut<'_>, cx: f32, cy: f32, r: f32, color: Color) {
    let mut pb = PathBuilder::new();
    let k = 0.5522847_f32;
    pb.move_to(cx + r, cy);
    pb.cubic_to(cx + r, cy + k * r, cx + k * r, cy + r, cx, cy + r);
    pb.cubic_to(cx - k * r, cy + r, cx - r, cy + k * r, cx - r, cy);
    pb.cubic_to(cx - r, cy - k * r, cx - k * r, cy - r, cx, cy - r);
    pb.cubic_to(cx + k * r, cy - r, cx + r, cy - k * r, cx + r, cy);
    pb.close();

    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(color.to_skia());
        paint.anti_alias = true;
        let stroke = tiny_skia::Stroke {
            width: 1.5,
            ..Default::default()
        };
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

fn draw_chevron(pixmap: &mut PixmapMut<'_>, cx: f32, cy: f32, color: Color, flipped: bool) {
    let mut pb = PathBuilder::new();
    if flipped {
        pb.move_to(cx - 5.0, cy + 2.0);
        pb.line_to(cx + 5.0, cy + 2.0);
        pb.line_to(cx, cy - 3.0);
    } else {
        pb.move_to(cx - 5.0, cy - 2.0);
        pb.line_to(cx + 5.0, cy - 2.0);
        pb.line_to(cx, cy + 3.0);
    }
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

impl Widget for InlineColorPicker {
    fn measure(&self, constraints: Constraints) -> Size {
        let h = if self.expanded {
            EXPANDED_H
        } else {
            COLLAPSED_H
        };
        Size {
            width: constraints.max_width.max(constraints.min_width),
            height: h.clamp(constraints.min_height, constraints.max_height),
        }
    }

    /// Paint only the collapsed header row. The expanded panel is in `paint_overlay`.
    fn paint(&self, pixmap: &mut PixmapMut<'_>, bounds: Rect, ctx: &mut RenderContext<'_>) {
        let surface = Color::from_hex(&ctx.palette.colors.surface)
            .unwrap_or(Color::new(0.192, 0.196, 0.267, 1.0));
        let fg = Color::from_hex(&ctx.palette.colors.foreground)
            .unwrap_or(Color::new(0.804, 0.839, 0.957, 1.0));
        let overlay_color = Color::from_hex(&ctx.palette.colors.overlay)
            .unwrap_or(Color::new(0.424, 0.439, 0.525, 1.0));
        let font_size = ctx.palette.fonts.size as f32;

        let header = Rect {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: COLLAPSED_H,
        };

        fill_rounded_rect(
            pixmap,
            header.x,
            header.y,
            header.width,
            header.height,
            6.0,
            surface,
        );

        // Checkerboard behind swatch if alpha < 1
        let sw_size = 24.0;
        let sw_x = header.x + H_PAD + 4.0;
        let sw_y = header.y + (COLLAPSED_H - sw_size) / 2.0;
        if self.value.a < 1.0 {
            let sq = 4.0_f32;
            let cols = (sw_size / sq).ceil() as u32;
            let rows = (sw_size / sq).ceil() as u32;
            let light = Color::new(0.8, 0.8, 0.8, 1.0);
            let dark = Color::new(0.55, 0.55, 0.55, 1.0);
            for row in 0..rows {
                for col in 0..cols {
                    let c = if (row + col) % 2 == 0 { light } else { dark };
                    let cx = sw_x + col as f32 * sq;
                    let cy = sw_y + row as f32 * sq;
                    let cw = sq.min(sw_x + sw_size - cx);
                    let ch = sq.min(sw_y + sw_size - cy);
                    if cw > 0.0 && ch > 0.0 {
                        fill_rounded_rect(pixmap, cx, cy, cw, ch, 0.0, c);
                    }
                }
            }
        }
        fill_rounded_rect(pixmap, sw_x, sw_y, sw_size, sw_size, 4.0, self.value);

        // Hex text
        let hex = self.value.to_hex();
        let text_x = sw_x + sw_size + 8.0;
        let text_y = header.y + (COLLAPSED_H - font_size * 1.3) / 2.0;
        ctx.text.draw_text(
            pixmap,
            &hex,
            text_x,
            text_y,
            font_size,
            fg,
            header.width - 60.0,
        );

        // Expand/collapse chevron
        let chev_cx = header.x + header.width - 14.0;
        let chev_cy = header.y + COLLAPSED_H / 2.0;
        draw_chevron(pixmap, chev_cx, chev_cy, overlay_color, self.expanded);
    }

    fn has_overlay(&self) -> bool {
        self.expanded
    }

    fn overlay_height(&self) -> f32 {
        if self.expanded {
            OVERLAY_H
        } else {
            0.0
        }
    }

    fn paint_overlay(&self, pixmap: &mut PixmapMut<'_>, bounds: Rect, ctx: &mut RenderContext<'_>) {
        if !self.expanded {
            return;
        }
        self.paint_expanded(pixmap, &bounds, ctx);
    }

    fn dismiss_overlay(&mut self) {
        self.expanded = false;
        self.hex_focused = false;
        self.dragging = DragTarget::None;
    }

    fn event(&mut self, event: &Event, bounds: &Rect) -> EventResponse {
        match event {
            // Toggle expand/collapse when clicking the header (on PointerUp with no drag)
            Event::PointerUp { x, y } => {
                let header = Rect {
                    x: bounds.x,
                    y: bounds.y,
                    width: bounds.width,
                    height: COLLAPSED_H,
                };

                if self.dragging != DragTarget::None {
                    self.dragging = DragTarget::None;
                    return EventResponse {
                        consumed: true,
                        action: None,
                    };
                }

                if header.contains(*x, *y) {
                    self.expanded = !self.expanded;
                    self.hex_focused = false;
                    return EventResponse {
                        consumed: true,
                        action: None,
                    };
                }

                if self.expanded {
                    // Apply button
                    let hex_r = self.hex_row_rect(bounds);
                    let hex_input_w = hex_r.width - 64.0;
                    let btn_x = hex_r.x + hex_input_w + 8.0;
                    let btn_rect = Rect {
                        x: btn_x,
                        y: hex_r.y,
                        width: 54.0,
                        height: hex_r.height,
                    };
                    if btn_rect.contains(*x, *y) {
                        if self.apply_hex() {
                            return EventResponse {
                                consumed: true,
                                action: Some(WidgetAction::ValueChanged {
                                    key: self.key.clone(),
                                    value: self.value.to_hex(),
                                }),
                            };
                        }
                        return EventResponse {
                            consumed: true,
                            action: None,
                        };
                    }

                    // Hex input focus
                    let hex_input_rect = Rect {
                        x: hex_r.x,
                        y: hex_r.y,
                        width: hex_input_w,
                        height: hex_r.height,
                    };
                    if hex_input_rect.contains(*x, *y) {
                        self.hex_focused = true;
                        return EventResponse {
                            consumed: true,
                            action: None,
                        };
                    }
                    self.hex_focused = false;
                }

                EventResponse::ignored()
            }

            Event::PointerDown { x, y } => {
                if !self.expanded {
                    return EventResponse::ignored();
                }

                let sv = self.sv_rect(bounds);
                let hue_r = self.hue_rect(bounds);
                let alpha_r = self.alpha_rect(bounds);

                if sv.contains(*x, *y) {
                    self.dragging = DragTarget::SvSquare;
                    self.sat = ((x - sv.x) / sv.width).clamp(0.0, 1.0);
                    self.val = (1.0 - (y - sv.y) / sv.height).clamp(0.0, 1.0);
                    self.sync_from_hsv();
                    return EventResponse {
                        consumed: true,
                        action: Some(WidgetAction::ValueChanged {
                            key: self.key.clone(),
                            value: self.value.to_hex(),
                        }),
                    };
                }

                if hue_r.contains(*x, *y) {
                    self.dragging = DragTarget::HueBar;
                    self.hue = ((x - hue_r.x) / hue_r.width * 360.0).clamp(0.0, 359.9);
                    self.sync_from_hsv();
                    return EventResponse {
                        consumed: true,
                        action: Some(WidgetAction::ValueChanged {
                            key: self.key.clone(),
                            value: self.value.to_hex(),
                        }),
                    };
                }

                if alpha_r.contains(*x, *y) {
                    self.dragging = DragTarget::AlphaBar;
                    self.value.a = ((x - alpha_r.x) / alpha_r.width).clamp(0.0, 1.0);
                    self.hex_input = self.value.to_hex();
                    return EventResponse {
                        consumed: true,
                        action: Some(WidgetAction::ValueChanged {
                            key: self.key.clone(),
                            value: self.value.to_hex(),
                        }),
                    };
                }

                EventResponse::ignored()
            }

            // Drag updates — forwarded by FormLayout when dragging_widget is set
            Event::PointerMove { x, y } => {
                if self.dragging == DragTarget::None || !self.expanded {
                    return EventResponse::ignored();
                }

                let sv = self.sv_rect(bounds);
                let hue_r = self.hue_rect(bounds);
                let alpha_r = self.alpha_rect(bounds);

                match self.dragging {
                    DragTarget::SvSquare => {
                        self.sat = ((x - sv.x) / sv.width).clamp(0.0, 1.0);
                        self.val = (1.0 - (y - sv.y) / sv.height).clamp(0.0, 1.0);
                        self.sync_from_hsv();
                    }
                    DragTarget::HueBar => {
                        self.hue = ((x - hue_r.x) / hue_r.width * 360.0).clamp(0.0, 359.9);
                        self.sync_from_hsv();
                    }
                    DragTarget::AlphaBar => {
                        self.value.a = ((x - alpha_r.x) / alpha_r.width).clamp(0.0, 1.0);
                        self.hex_input = self.value.to_hex();
                    }
                    DragTarget::None => {}
                }

                EventResponse {
                    consumed: true,
                    action: Some(WidgetAction::ValueChanged {
                        key: self.key.clone(),
                        value: self.value.to_hex(),
                    }),
                }
            }

            Event::KeyPress { key, utf8 } if self.hex_focused => {
                match key.as_str() {
                    "BackSpace" => {
                        self.hex_input.pop();
                    }
                    "Return" | "KP_Enter" => {
                        if self.apply_hex() {
                            return EventResponse {
                                consumed: true,
                                action: Some(WidgetAction::ValueChanged {
                                    key: self.key.clone(),
                                    value: self.value.to_hex(),
                                }),
                            };
                        }
                    }
                    _ => {
                        if let Some(s) = utf8 {
                            for ch in s.chars() {
                                if !ch.is_control() && self.hex_input.len() < 9 {
                                    self.hex_input.push(ch);
                                }
                            }
                        }
                    }
                }
                EventResponse {
                    consumed: true,
                    action: None,
                }
            }

            _ => EventResponse::ignored(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapsed_height() {
        let cp = InlineColorPicker::new("bg", Color::new(1.0, 0.0, 0.0, 1.0));
        let size = cp.measure(Constraints::loose(200.0, 400.0));
        assert_eq!(size.height, COLLAPSED_H);
    }

    #[test]
    fn expanded_height() {
        let mut cp = InlineColorPicker::new("bg", Color::new(0.0, 1.0, 0.0, 1.0));
        cp.expanded = true;
        let size = cp.measure(Constraints::loose(200.0, 400.0));
        assert_eq!(size.height, EXPANDED_H);
    }

    #[test]
    fn set_color_syncs_hsv() {
        let mut cp = InlineColorPicker::new("acc", Color::new(0.0, 0.0, 0.0, 1.0));
        cp.set_color(Color::new(1.0, 0.0, 0.0, 1.0));
        assert!((cp.hue).abs() < 1.0);
        assert!((cp.sat - 1.0).abs() < 0.01);
        assert!((cp.val - 1.0).abs() < 0.01);
    }

    #[test]
    fn has_overlay_when_expanded() {
        let mut cp = InlineColorPicker::new("bg", Color::new(1.0, 0.0, 0.0, 1.0));
        assert!(!cp.has_overlay());
        cp.expanded = true;
        assert!(cp.has_overlay());
        assert!((cp.overlay_height() - (EXPANDED_H - COLLAPSED_H)).abs() < 0.1);
    }

    #[test]
    fn dismiss_overlay_collapses() {
        let mut cp = InlineColorPicker::new("bg", Color::new(1.0, 0.0, 0.0, 1.0));
        cp.expanded = true;
        cp.dismiss_overlay();
        assert!(!cp.expanded);
        assert!(!cp.has_overlay());
    }

    #[test]
    fn header_click_expands() {
        let mut cp = InlineColorPicker::new("c", Color::new(0.0, 0.5, 1.0, 1.0));
        let bounds = Rect {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 32.0,
        };
        let resp = cp.event(&Event::PointerUp { x: 100.0, y: 16.0 }, &bounds);
        assert!(resp.consumed);
        assert!(cp.expanded);
    }

    #[test]
    fn sv_drag_updates_color() {
        let mut cp = InlineColorPicker::new("c", Color::new(1.0, 0.0, 0.0, 1.0));
        cp.expanded = true;
        cp.hue = 0.0;
        let bounds = Rect {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 32.0,
        };
        // Simulate PointerDown in SV square
        let sv_y = COLLAPSED_H + GAP;
        let resp = cp.event(
            &Event::PointerDown {
                x: H_PAD + SV_W * 0.5,
                y: sv_y + SV_H * 0.5,
            },
            &bounds,
        );
        assert!(resp.consumed);
        assert!((cp.sat - 0.5).abs() < 0.01);
        assert!((cp.val - 0.5).abs() < 0.01);
    }
}
