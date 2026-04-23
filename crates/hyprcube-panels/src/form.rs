use tiny_skia::PixmapMut;

use hyprcube_core::color::Color;
use hyprcube_core::color_picker::InlineColorPicker;
use hyprcube_core::layout::Rect;
use hyprcube_core::text::RenderContext;
use hyprcube_core::widget::{
    Dropdown, Event, EventResponse, SearchableDropdown, Slider, ScrollArea,
    TextInput, Toggle, Widget, WidgetAction,
};

use crate::{FieldType, PanelField};

const ROW_MAIN_H: f32 = 40.0;
const ROW_DESC_H: f32 = 18.0;
const ROW_GAP: f32 = 8.0;
const H_PAD: f32 = 16.0;

/// A single row in a settings form — one field + its widget.
pub struct FormRow {
    pub field: PanelField,
    pub widget: Box<dyn Widget>,
    /// Y position in content space (top of the row, including description).
    pub y_offset: f32,
    /// Total height of this row including gap.
    pub height: f32,
}

/// A scrollable settings form built from a `Vec<PanelField>`.
pub struct FormLayout {
    pub rows: Vec<FormRow>,
    pub scroll: ScrollArea,
    focused_row: Option<usize>,
    /// Index of the row whose widget is currently being dragged (mouse held down).
    /// PointerMove events are forwarded directly to this widget until PointerUp.
    dragging_widget: Option<usize>,
}

impl FormLayout {
    /// Build a form from a list of panel fields.
    pub fn from_fields(fields: Vec<PanelField>, _content_width: f32) -> Self {
        let mut rows = Vec::new();
        let mut y = H_PAD;

        for field in fields {
            let has_desc = !field.description.is_empty();
            let row_h = ROW_MAIN_H + if has_desc { ROW_DESC_H } else { 0.0 } + ROW_GAP;
            let widget = make_widget(&field);
            rows.push(FormRow { field, widget, y_offset: y, height: row_h });
            y += row_h;
        }

        let content_height = y + H_PAD;
        let mut scroll = ScrollArea::new(0.0);
        scroll.content_height = content_height;

        Self { rows, scroll, focused_row: None, dragging_widget: None }
    }

    /// Update the visible viewport height (call when window resizes).
    pub fn set_viewport_height(&mut self, h: f32) {
        self.scroll.viewport_height = h;
    }

    /// Paint the form into `pixmap` within `bounds`.
    ///
    /// Two-pass so open overlay content always renders above every other row.
    pub fn paint(&self, bounds: Rect, ctx: &mut RenderContext<'_>, pixmap: &mut PixmapMut<'_>) {
        let font_size = ctx.palette.fonts.size as f32;
        let fg = Color::from_hex(&ctx.palette.colors.foreground)
            .unwrap_or(Color::new(0.804, 0.839, 0.957, 1.0));
        let overlay = Color::from_hex(&ctx.palette.colors.overlay)
            .unwrap_or(Color::new(0.424, 0.439, 0.525, 1.0));

        let (scroll_top, scroll_bottom) = self.scroll.visible_range();
        let widget_w = bounds.width * 0.45;
        let label_max_w = bounds.width * 0.5 - 16.0;
        let desc_font_size = (font_size - 2.0).max(8.0);

        // Pass 1 — base layer: labels, descriptions, collapsed widget states.
        for row in &self.rows {
            if row.y_offset + row.height < scroll_top || row.y_offset > scroll_bottom {
                continue;
            }

            let row_y = bounds.y + row.y_offset - self.scroll.scroll_offset;

            let label_y = row_y + (ROW_MAIN_H - font_size * 1.3) / 2.0;
            ctx.text.draw_text(
                pixmap,
                &row.field.label,
                bounds.x + H_PAD,
                label_y,
                font_size,
                fg,
                label_max_w,
            );

            if !row.field.description.is_empty() {
                ctx.text.draw_text(
                    pixmap,
                    &row.field.description,
                    bounds.x + H_PAD,
                    row_y + ROW_MAIN_H,
                    desc_font_size,
                    overlay,
                    label_max_w,
                );
            }

            let widget_x = bounds.x + bounds.width - widget_w - H_PAD;
            let widget_y = row_y + (ROW_MAIN_H - 32.0) / 2.0;
            let wb = Rect { x: widget_x, y: widget_y, width: widget_w, height: 32.0 };
            row.widget.paint(pixmap, wb, ctx);
        }

        self.scroll.paint_scrollbar(bounds, pixmap, ctx.palette);

        // Pass 2 — overlay layer: expanded dropdown/picker panels on top of everything.
        for row in &self.rows {
            if !row.widget.has_overlay() {
                continue;
            }
            if row.y_offset + row.height < scroll_top || row.y_offset > scroll_bottom {
                continue;
            }
            let row_y = bounds.y + row.y_offset - self.scroll.scroll_offset;
            let widget_x = bounds.x + bounds.width - widget_w - H_PAD;
            let widget_y = row_y + (ROW_MAIN_H - 32.0) / 2.0;
            let wb = Rect { x: widget_x, y: widget_y, width: widget_w, height: 32.0 };
            row.widget.paint_overlay(pixmap, wb, ctx);
        }
    }

    /// Deliver an input event to the form.
    pub fn event(&mut self, event: &Event, bounds: Rect) -> EventResponse {
        // Escape closes any open overlays.
        if let Event::KeyPress { key, .. } = event {
            if key == "Escape" {
                let mut any_open = false;
                for row in &mut self.rows {
                    if row.widget.has_overlay() {
                        row.widget.dismiss_overlay();
                        any_open = true;
                    }
                }
                if any_open {
                    self.dragging_widget = None;
                    return EventResponse { consumed: true, action: None };
                }
            }
        }

        let widget_w = bounds.width * 0.45;

        // PointerMove — forward to the widget currently being dragged.
        if let Event::PointerMove { .. } = event {
            if let Some(idx) = self.dragging_widget {
                if let Some(row) = self.rows.get_mut(idx) {
                    let row_y = bounds.y + row.y_offset - self.scroll.scroll_offset;
                    let widget_x = bounds.x + bounds.width - widget_w - H_PAD;
                    let widget_y = row_y + (ROW_MAIN_H - 32.0) / 2.0;
                    let wb = Rect { x: widget_x, y: widget_y, width: widget_w, height: 32.0 };
                    let resp = row.widget.event(event, &wb);
                    if let Some(WidgetAction::ValueChanged { ref value, .. }) = resp.action {
                        row.field.current_value = value.clone();
                    }
                    return resp;
                }
            }
            return EventResponse::ignored();
        }

        match event {
            Event::Scroll { dy, .. } => {
                self.scroll.scroll_by(*dy * 30.0);
                return EventResponse { consumed: true, action: None };
            }

            Event::PointerUp { x, y } => {
                // Clear drag state on release.
                let prev_dragging = self.dragging_widget.take();

                if !bounds.contains(*x, *y) {
                    return EventResponse::ignored();
                }

                // If we were dragging a widget, send PointerUp to it.
                if let Some(idx) = prev_dragging {
                    if let Some(row) = self.rows.get_mut(idx) {
                        let row_y = bounds.y + row.y_offset - self.scroll.scroll_offset;
                        let widget_x = bounds.x + bounds.width - widget_w - H_PAD;
                        let widget_y = row_y + (ROW_MAIN_H - 32.0) / 2.0;
                        let wb = Rect { x: widget_x, y: widget_y, width: widget_w, height: 32.0 };
                        let resp = row.widget.event(event, &wb);
                        if let Some(WidgetAction::ValueChanged { ref value, .. }) = resp.action {
                            row.field.current_value = value.clone();
                        }
                        return resp;
                    }
                }

                // Normal PointerUp routing (e.g. dropdown option selection via overlay).
                for i in 0..self.rows.len() {
                    if !self.rows[i].widget.has_overlay() {
                        continue;
                    }
                    let row_y = bounds.y + self.rows[i].y_offset - self.scroll.scroll_offset;
                    let widget_x = bounds.x + bounds.width - widget_w - H_PAD;
                    let widget_y = row_y + (ROW_MAIN_H - 32.0) / 2.0;
                    let overlay_h = self.rows[i].widget.overlay_height();
                    let overlay_rect = Rect {
                        x: widget_x,
                        y: widget_y + 32.0,
                        width: widget_w,
                        height: overlay_h,
                    };
                    if overlay_rect.contains(*x, *y) {
                        self.focused_row = Some(i);
                        let wb = Rect { x: widget_x, y: widget_y, width: widget_w, height: 32.0 };
                        let resp = self.rows[i].widget.event(event, &wb);
                        if let Some(WidgetAction::ValueChanged { ref value, .. }) = resp.action {
                            self.rows[i].field.current_value = value.clone();
                        }
                        return resp;
                    }
                }

                // Normal row hit-test.
                let content_y = *y - bounds.y + self.scroll.scroll_offset;
                if let Some(i) = self.rows.iter().position(|r| {
                    content_y >= r.y_offset && content_y < r.y_offset + r.height
                }) {
                    self.focused_row = Some(i);
                    let row = &mut self.rows[i];
                    let widget_x = bounds.x + bounds.width - widget_w - H_PAD;
                    let row_y = bounds.y + row.y_offset - self.scroll.scroll_offset;
                    let widget_y = row_y + (ROW_MAIN_H - 32.0) / 2.0;
                    let wb = Rect { x: widget_x, y: widget_y, width: widget_w, height: 32.0 };
                    let resp = row.widget.event(event, &wb);
                    if let Some(WidgetAction::ValueChanged { ref value, .. }) = resp.action {
                        row.field.current_value = value.clone();
                    }
                    return resp;
                }
            }

            Event::PointerDown { x, y } => {
                if !bounds.contains(*x, *y) {
                    return EventResponse::ignored();
                }

                // Click-outside-to-close: dismiss overlays not hit by this click.
                for row in &mut self.rows {
                    if !row.widget.has_overlay() {
                        continue;
                    }
                    let row_y = bounds.y + row.y_offset - self.scroll.scroll_offset;
                    let widget_x = bounds.x + bounds.width - widget_w - H_PAD;
                    let widget_y = row_y + (ROW_MAIN_H - 32.0) / 2.0;
                    let full_h = 32.0 + row.widget.overlay_height();
                    let full_wb = Rect {
                        x: widget_x,
                        y: widget_y,
                        width: widget_w,
                        height: full_h,
                    };
                    if !full_wb.contains(*x, *y) {
                        row.widget.dismiss_overlay();
                    }
                }

                // Check if click lands in an open overlay (e.g. expanded color picker).
                for i in 0..self.rows.len() {
                    if !self.rows[i].widget.has_overlay() {
                        continue;
                    }
                    let row_y = bounds.y + self.rows[i].y_offset - self.scroll.scroll_offset;
                    let widget_x = bounds.x + bounds.width - widget_w - H_PAD;
                    let widget_y = row_y + (ROW_MAIN_H - 32.0) / 2.0;
                    let overlay_h = self.rows[i].widget.overlay_height();
                    let overlay_rect = Rect {
                        x: widget_x,
                        y: widget_y + 32.0,
                        width: widget_w,
                        height: overlay_h,
                    };
                    if overlay_rect.contains(*x, *y) {
                        self.focused_row = Some(i);
                        let wb = Rect { x: widget_x, y: widget_y, width: widget_w, height: 32.0 };
                        let resp = self.rows[i].widget.event(event, &wb);
                        if resp.consumed {
                            self.dragging_widget = Some(i);
                        }
                        if let Some(WidgetAction::ValueChanged { ref value, .. }) = resp.action {
                            self.rows[i].field.current_value = value.clone();
                        }
                        return resp;
                    }
                }

                // Normal row hit-test.
                let content_y = *y - bounds.y + self.scroll.scroll_offset;
                if let Some(i) = self.rows.iter().position(|r| {
                    content_y >= r.y_offset && content_y < r.y_offset + r.height
                }) {
                    self.focused_row = Some(i);
                    let row = &mut self.rows[i];
                    let widget_x = bounds.x + bounds.width - widget_w - H_PAD;
                    let row_y = bounds.y + row.y_offset - self.scroll.scroll_offset;
                    let widget_y = row_y + (ROW_MAIN_H - 32.0) / 2.0;
                    let wb = Rect { x: widget_x, y: widget_y, width: widget_w, height: 32.0 };
                    let resp = row.widget.event(event, &wb);
                    if resp.consumed {
                        self.dragging_widget = Some(i);
                    }
                    if let Some(WidgetAction::ValueChanged { ref value, .. }) = resp.action {
                        row.field.current_value = value.clone();
                    }
                    return resp;
                }
            }

            Event::KeyPress { .. } => {
                if let Some(idx) = self.focused_row {
                    if let Some(row) = self.rows.get_mut(idx) {
                        let dummy = Rect::default();
                        let resp = row.widget.event(event, &dummy);
                        if let Some(WidgetAction::ValueChanged { ref value, .. }) = resp.action {
                            row.field.current_value = value.clone();
                        }
                        return resp;
                    }
                }
            }

            _ => {}
        }
        EventResponse::ignored()
    }

    /// Iterate over (key, current_value) for all rows.
    pub fn values(&self) -> impl Iterator<Item = (&str, &str)> {
        self.rows.iter().map(|r| (r.field.key.as_str(), r.field.current_value.as_str()))
    }
}

fn make_widget(field: &PanelField) -> Box<dyn Widget> {
    match &field.field_type {
        FieldType::Text | FieldType::KeyBind => {
            Box::new(TextInput::new(&field.key, &field.current_value))
        }
        FieldType::Boolean => {
            let on =
                matches!(field.current_value.to_lowercase().as_str(), "true" | "yes" | "1");
            Box::new(Toggle::new(&field.key, on))
        }
        FieldType::Color => {
            // Try hex first, then Hyprland rgba() format.
            let color = hyprcube_core::color::Color::from_hex(&field.current_value)
                .or_else(|_| {
                    hyprcube_core::color::Color::from_hypr_rgba(&field.current_value)
                })
                .unwrap_or(hyprcube_core::color::Color::new(0.5, 0.5, 0.5, 1.0));
            Box::new(InlineColorPicker::new(&field.key, color))
        }
        FieldType::Choice { options } => {
            let sel = options.iter().position(|o| o == &field.current_value).unwrap_or(0);
            Box::new(Dropdown::new(&field.key, options.clone(), sel))
        }
        FieldType::SearchableChoice { options } => {
            Box::new(SearchableDropdown::new(&field.key, options.clone(), &field.current_value))
        }
        FieldType::Integer { min, max } => {
            let lo = min.unwrap_or(0) as f64;
            let hi = max.unwrap_or(100) as f64;
            let val: f64 = field.current_value.parse().unwrap_or(lo);
            let range = (hi - lo) as i64;
            if range > 0 && range <= 10 {
                let opts: Vec<String> = (min.unwrap_or(0)..=max.unwrap_or(10))
                    .map(|i| i.to_string())
                    .collect();
                let sel = opts
                    .iter()
                    .position(|o| o.parse::<f64>().ok() == Some(val))
                    .unwrap_or(0);
                Box::new(Dropdown::new(&field.key, opts, sel))
            } else {
                Box::new(Slider::new(&field.key, val, lo, hi, 1.0))
            }
        }
        FieldType::Float { min, max } => {
            let lo = min.unwrap_or(0.0);
            let hi = max.unwrap_or(1.0);
            let val: f64 = field.current_value.parse().unwrap_or(lo);
            Box::new(Slider::new(&field.key, val, lo, hi, 0.1))
        }
    }
}
