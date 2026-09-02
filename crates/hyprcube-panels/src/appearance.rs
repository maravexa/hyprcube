use std::sync::{Arc, Mutex};

use hyprcube_core::color::Color;
use hyprcube_core::color_picker::{InlineColorPicker, EXPANDED_H};
use hyprcube_core::layout::Rect;
use hyprcube_core::palette::Palette;
use hyprcube_core::render::fill_rounded_rect;
use hyprcube_core::widget::{
    Constraints, Dropdown, Event, EventResponse, ScrollArea, Size, Slider, TextInput, Widget,
    WidgetAction,
};

use crate::{FieldType, PanelError, PanelField, SettingsPanel};

// ── Layout constants ────────────────────────────────────────────────────────

const H_PAD: f32 = 16.0;
const SECTION_H: f32 = 36.0;
const ROW_H: f32 = 40.0;
const ROW_GAP: f32 = 8.0;
const LABEL_H: f32 = 20.0;
const PICKER_ROW_H: f32 = 32.0; // collapsed picker height

const COLOR_FIELDS: [(&str, &str); 6] = [
    ("colors.background", "Background"),
    ("colors.foreground", "Foreground"),
    ("colors.accent", "Accent"),
    ("colors.urgent", "Urgent"),
    ("colors.surface", "Surface"),
    ("colors.overlay", "Overlay"),
];

// ── Layout computation ──────────────────────────────────────────────────────

struct PanelLayout {
    palette_section_y: f32,
    preset_label_y: f32,
    preset_drop_y: f32,
    preset_drop_h: f32,
    color_label_y: [f32; 6],
    color_pick_y: [f32; 6],
    color_pick_h: [f32; 6],
    typography_y: f32,
    font_family_row_y: f32,
    mono_family_row_y: f32,
    font_size_row_y: f32,
    style_y: f32,
    border_row_y: f32,
    opacity_row_y: f32,
    content_height: f32,
}

// ── AppearancePanel ─────────────────────────────────────────────────────────

pub struct AppearancePanel {
    palette: Arc<Mutex<Palette>>,
    preset_dropdown: Dropdown,
    color_pickers: Vec<InlineColorPicker>,
    font_family: TextInput,
    mono_family: TextInput,
    font_size: Slider,
    border_radius: Slider,
    opacity: Slider,
    scroll: ScrollArea,
    /// Index of the color picker currently being dragged (if any).
    dragging_picker: Option<usize>,
}

impl AppearancePanel {
    pub fn new() -> Self {
        let palette = Palette::load(None).unwrap_or_else(|e| {
            tracing::warn!("failed to load palette, using defaults: {e}");
            Palette::default()
        });
        Self::with_arc(Arc::new(Mutex::new(palette)))
    }

    pub fn with_arc(palette: Arc<Mutex<Palette>>) -> Self {
        let pal = palette.lock().unwrap().clone();
        let presets = Palette::builtin_presets();
        let preset_names: Vec<String> = presets
            .iter()
            .map(|(n, _)| n.to_string())
            .chain(std::iter::once("Custom".to_string()))
            .collect();

        let preset_dropdown = Dropdown::new("palette.preset", preset_names, 0);

        let color_pickers = COLOR_FIELDS
            .iter()
            .map(|(key, _)| {
                let hex = color_hex_from_palette(&pal, key);
                let color = Color::from_hex(&hex).unwrap_or(Color::new(0.5, 0.5, 0.5, 1.0));
                InlineColorPicker::new(*key, color)
            })
            .collect();

        Self {
            palette,
            preset_dropdown,
            color_pickers,
            font_family: TextInput::new("fonts.family", &pal.fonts.family),
            mono_family: TextInput::new("fonts.mono_family", &pal.fonts.mono_family),
            font_size: Slider::new("fonts.size", pal.fonts.size, 8.0, 24.0, 0.5),
            border_radius: Slider::new(
                "style.border_radius",
                pal.style.border_radius,
                0.0,
                24.0,
                1.0,
            ),
            opacity: Slider::new("style.opacity", pal.style.opacity, 0.0, 1.0, 0.05),
            scroll: ScrollArea::new(600.0),
            dragging_picker: None,
        }
    }

    // ── Internal helpers ────────────────────────────────────────────────────

    fn sync_widgets_from_palette(&mut self) {
        let pal = self.palette.lock().unwrap().clone();
        let hexes = [
            &pal.colors.background,
            &pal.colors.foreground,
            &pal.colors.accent,
            &pal.colors.urgent,
            &pal.colors.surface,
            &pal.colors.overlay,
        ];
        for (picker, hex) in self.color_pickers.iter_mut().zip(hexes.iter()) {
            let color = Color::from_hex(hex).unwrap_or(Color::new(0.5, 0.5, 0.5, 1.0));
            picker.set_color(color);
        }
        self.font_family.value = pal.fonts.family.clone();
        self.mono_family.value = pal.fonts.mono_family.clone();
        self.font_size.set_value(pal.fonts.size);
        self.border_radius.set_value(pal.style.border_radius);
        self.opacity.set_value(pal.style.opacity);
    }

    fn apply_preset(&mut self, idx: usize) {
        let presets = Palette::builtin_presets();
        if let Some((_, preset)) = presets.into_iter().nth(idx) {
            *self.palette.lock().unwrap() = preset;
            self.sync_widgets_from_palette();
        }
    }

    fn mark_custom(&mut self) {
        self.preset_dropdown.selected = self.preset_dropdown.options.len() - 1;
    }

    fn compute_layout(&self) -> PanelLayout {
        let mut y = 0.0_f32;

        let palette_section_y = y;
        y += SECTION_H;

        let preset_label_y = y;
        y += LABEL_H;

        let preset_drop_y = y;
        let preset_drop_h = if self.preset_dropdown.open {
            32.0 + self.preset_dropdown.options.len() as f32 * 28.0
        } else {
            32.0
        };
        y += preset_drop_h + ROW_GAP;

        let mut color_label_y = [0.0f32; 6];
        let mut color_pick_y = [0.0f32; 6];
        let mut color_pick_h = [0.0f32; 6];
        for i in 0..6 {
            color_label_y[i] = y;
            y += LABEL_H;
            color_pick_y[i] = y;
            let ph = if self.color_pickers[i].expanded {
                EXPANDED_H
            } else {
                PICKER_ROW_H
            };
            color_pick_h[i] = ph;
            y += ph + ROW_GAP;
        }

        let typography_y = y;
        y += SECTION_H;

        let font_family_row_y = y;
        y += ROW_H + ROW_GAP;

        let mono_family_row_y = y;
        y += ROW_H + ROW_GAP;

        let font_size_row_y = y;
        y += ROW_H + ROW_GAP;

        let style_y = y;
        y += SECTION_H;

        let border_row_y = y;
        y += ROW_H + ROW_GAP;

        let opacity_row_y = y;
        y += ROW_H + 16.0;

        PanelLayout {
            palette_section_y,
            preset_label_y,
            preset_drop_y,
            preset_drop_h,
            color_label_y,
            color_pick_y,
            color_pick_h,
            typography_y,
            font_family_row_y,
            mono_family_row_y,
            font_size_row_y,
            style_y,
            border_row_y,
            opacity_row_y,
            content_height: y,
        }
    }

    /// Compute the header-only bounds for picker `i` (32px tall, same x/width as the full picker).
    fn picker_header_wb(&self, bounds: &Rect, screen_pick_y: f32) -> Rect {
        Rect {
            x: bounds.x + H_PAD,
            y: screen_pick_y,
            width: bounds.width - H_PAD * 2.0,
            height: PICKER_ROW_H,
        }
    }
}

impl Default for AppearancePanel {
    fn default() -> Self {
        Self::new()
    }
}

// ── SettingsPanel impl ──────────────────────────────────────────────────────

impl SettingsPanel for AppearancePanel {
    fn title(&self) -> &str {
        "Appearance"
    }

    fn icon(&self) -> &str {
        "preferences-desktop-theme"
    }

    fn uses_custom_layout(&self) -> bool {
        true
    }

    fn fields(&self) -> Vec<PanelField> {
        let pal = self.palette.lock().unwrap();

        let pf = |key: &'static str,
                  label: &'static str,
                  desc: &'static str,
                  ft: FieldType,
                  val: String| {
            PanelField {
                key: key.into(),
                section: None,
                label: label.into(),
                description: desc.into(),
                field_type: ft,
                original_value: Some(val.clone()),
                current_value: val,
                dirty: false,
            }
        };

        vec![
            pf(
                "colors.background",
                "Background",
                "Primary background color.",
                FieldType::Color,
                pal.colors.background.clone(),
            ),
            pf(
                "colors.foreground",
                "Foreground",
                "Primary text color.",
                FieldType::Color,
                pal.colors.foreground.clone(),
            ),
            pf(
                "colors.accent",
                "Accent",
                "Accent / highlight color.",
                FieldType::Color,
                pal.colors.accent.clone(),
            ),
            pf(
                "colors.urgent",
                "Urgent",
                "Color for urgent / error states.",
                FieldType::Color,
                pal.colors.urgent.clone(),
            ),
            pf(
                "colors.surface",
                "Surface",
                "Surface / card background color.",
                FieldType::Color,
                pal.colors.surface.clone(),
            ),
            pf(
                "colors.overlay",
                "Overlay",
                "Overlay / muted text color.",
                FieldType::Color,
                pal.colors.overlay.clone(),
            ),
            pf(
                "fonts.family",
                "Font Family",
                "Primary UI font family.",
                FieldType::Text,
                pal.fonts.family.clone(),
            ),
            pf(
                "fonts.mono_family",
                "Monospace Font",
                "Monospace font for code and config values.",
                FieldType::Text,
                pal.fonts.mono_family.clone(),
            ),
            pf(
                "fonts.size",
                "Font Size",
                "Base font size (pt).",
                FieldType::Float {
                    min: Some(6.0),
                    max: Some(48.0),
                },
                pal.fonts.size.to_string(),
            ),
            pf(
                "style.border_radius",
                "Border Radius",
                "Corner rounding for UI elements (px).",
                FieldType::Float {
                    min: Some(0.0),
                    max: Some(32.0),
                },
                pal.style.border_radius.to_string(),
            ),
            pf(
                "style.opacity",
                "Opacity",
                "Window opacity (0.0 – 1.0).",
                FieldType::Float {
                    min: Some(0.0),
                    max: Some(1.0),
                },
                pal.style.opacity.to_string(),
            ),
        ]
    }

    fn set_value(&mut self, key: &str, value: &str) -> Result<String, PanelError> {
        let mut pal = self.palette.lock().unwrap();
        match key {
            "colors.background" => {
                let old = pal.colors.background.clone();
                pal.colors.background = value.to_string();
                Ok(old)
            }
            "colors.foreground" => {
                let old = pal.colors.foreground.clone();
                pal.colors.foreground = value.to_string();
                Ok(old)
            }
            "colors.accent" => {
                let old = pal.colors.accent.clone();
                pal.colors.accent = value.to_string();
                Ok(old)
            }
            "colors.urgent" => {
                let old = pal.colors.urgent.clone();
                pal.colors.urgent = value.to_string();
                Ok(old)
            }
            "colors.surface" => {
                let old = pal.colors.surface.clone();
                pal.colors.surface = value.to_string();
                Ok(old)
            }
            "colors.overlay" => {
                let old = pal.colors.overlay.clone();
                pal.colors.overlay = value.to_string();
                Ok(old)
            }
            "fonts.family" => {
                let old = pal.fonts.family.clone();
                pal.fonts.family = value.to_string();
                Ok(old)
            }
            "fonts.mono_family" => {
                let old = pal.fonts.mono_family.clone();
                pal.fonts.mono_family = value.to_string();
                Ok(old)
            }
            "fonts.size" => {
                let new: f64 = value.parse().map_err(|_| PanelError::InvalidValue {
                    key: key.to_string(),
                    reason: "expected a number".into(),
                })?;
                let old = pal.fonts.size.to_string();
                pal.fonts.size = new;
                Ok(old)
            }
            "style.border_radius" => {
                let new: f64 = value.parse().map_err(|_| PanelError::InvalidValue {
                    key: key.to_string(),
                    reason: "expected a number".into(),
                })?;
                let old = pal.style.border_radius.to_string();
                pal.style.border_radius = new;
                Ok(old)
            }
            "style.opacity" => {
                let new: f64 = value.parse().map_err(|_| PanelError::InvalidValue {
                    key: key.to_string(),
                    reason: "expected a number".into(),
                })?;
                let old = pal.style.opacity.to_string();
                pal.style.opacity = new;
                Ok(old)
            }
            _ => Err(PanelError::UnknownKey(key.to_string())),
        }
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
        ctx: &mut hyprcube_core::text::RenderContext<'_>,
    ) {
        let fg = Color::from_hex(&ctx.palette.colors.foreground)
            .unwrap_or(Color::new(0.804, 0.839, 0.957, 1.0));
        let overlay = Color::from_hex(&ctx.palette.colors.overlay)
            .unwrap_or(Color::new(0.424, 0.439, 0.525, 1.0));
        let font_size = ctx.palette.fonts.size as f32;

        let layout = self.compute_layout();
        let (scroll_top, scroll_bot) = self.scroll.visible_range();

        let sy = |cy: f32| bounds.y + cy - self.scroll.scroll_offset;
        let visible = |cy: f32, h: f32| cy + h >= scroll_top && cy < scroll_bot;

        // ── Pass 1: base layer ───────────────────────────────────────────────

        if visible(layout.palette_section_y, SECTION_H) {
            paint_section(
                pixmap,
                ctx,
                bounds.x,
                sy(layout.palette_section_y),
                bounds.width,
                "Palette",
                font_size,
                fg,
                overlay,
            );
        }

        if visible(layout.preset_label_y, LABEL_H) {
            ctx.text.draw_text(
                pixmap,
                "Theme Preset",
                bounds.x + H_PAD,
                sy(layout.preset_label_y),
                font_size,
                overlay,
                bounds.width - H_PAD * 2.0,
            );
        }

        if visible(layout.preset_drop_y, layout.preset_drop_h) {
            let wb = Rect {
                x: bounds.x + H_PAD,
                y: sy(layout.preset_drop_y),
                width: bounds.width - H_PAD * 2.0,
                height: layout.preset_drop_h,
            };
            self.preset_dropdown.paint(pixmap, wb, ctx);
        }

        for (i, (_, label)) in COLOR_FIELDS.iter().enumerate() {
            if visible(layout.color_label_y[i], LABEL_H) {
                ctx.text.draw_text(
                    pixmap,
                    label,
                    bounds.x + H_PAD,
                    sy(layout.color_label_y[i]),
                    font_size,
                    fg,
                    bounds.width - H_PAD * 2.0,
                );
            }

            if visible(layout.color_pick_y[i], layout.color_pick_h[i]) {
                let wb = self.picker_header_wb(&bounds, sy(layout.color_pick_y[i]));
                self.color_pickers[i].paint(pixmap, wb, ctx);
            }
        }

        if visible(layout.typography_y, SECTION_H) {
            paint_section(
                pixmap,
                ctx,
                bounds.x,
                sy(layout.typography_y),
                bounds.width,
                "Typography",
                font_size,
                fg,
                overlay,
            );
        }

        if visible(layout.font_family_row_y, ROW_H) {
            paint_label_widget_row(
                pixmap,
                ctx,
                &bounds,
                sy(layout.font_family_row_y),
                "Font Family",
                &self.font_family,
                font_size,
                fg,
            );
        }

        if visible(layout.mono_family_row_y, ROW_H) {
            paint_label_widget_row(
                pixmap,
                ctx,
                &bounds,
                sy(layout.mono_family_row_y),
                "Mono Font",
                &self.mono_family,
                font_size,
                fg,
            );
        }

        if visible(layout.font_size_row_y, ROW_H) {
            paint_label_widget_row(
                pixmap,
                ctx,
                &bounds,
                sy(layout.font_size_row_y),
                "Font Size",
                &self.font_size,
                font_size,
                fg,
            );
        }

        if visible(layout.style_y, SECTION_H) {
            paint_section(
                pixmap,
                ctx,
                bounds.x,
                sy(layout.style_y),
                bounds.width,
                "Style",
                font_size,
                fg,
                overlay,
            );
        }

        if visible(layout.border_row_y, ROW_H) {
            paint_label_widget_row(
                pixmap,
                ctx,
                &bounds,
                sy(layout.border_row_y),
                "Border Radius",
                &self.border_radius,
                font_size,
                fg,
            );
        }

        if visible(layout.opacity_row_y, ROW_H) {
            paint_label_widget_row(
                pixmap,
                ctx,
                &bounds,
                sy(layout.opacity_row_y),
                "Opacity",
                &self.opacity,
                font_size,
                fg,
            );
        }

        self.scroll.paint_scrollbar(bounds, pixmap, ctx.palette);

        // ── Pass 2: overlay layer — expanded color pickers on top ────────────
        for i in 0..6 {
            if !self.color_pickers[i].expanded {
                continue;
            }
            if !visible(layout.color_pick_y[i], layout.color_pick_h[i]) {
                continue;
            }
            let wb = self.picker_header_wb(&bounds, sy(layout.color_pick_y[i]));
            self.color_pickers[i].paint_overlay(pixmap, wb, ctx);
        }
    }

    fn event(&mut self, event: &Event, bounds: Rect) -> EventResponse {
        // ── Scroll ──────────────────────────────────────────────────────────
        if let Event::Scroll { dy, .. } = event {
            self.scroll.scroll_by(*dy * 30.0);
            return EventResponse {
                consumed: true,
                action: None,
            };
        }

        let layout = self.compute_layout();
        self.scroll.content_height = layout.content_height;
        let scroll_offset = self.scroll.scroll_offset;
        let sy = |cy: f32| bounds.y + cy - scroll_offset;

        // ── PointerMove — forward to whichever continuous control is active ─
        if let Event::PointerMove { .. } = event {
            if let Some(i) = self.dragging_picker {
                let wb = self.picker_header_wb(&bounds, sy(layout.color_pick_y[i]));
                let resp = self.color_pickers[i].event(event, &wb);
                if let Some(WidgetAction::ValueChanged { ref value, .. }) = resp.action {
                    let key = COLOR_FIELDS[i].0;
                    let _ = self.set_value(key, value);
                    self.mark_custom();
                }
                return resp;
            }
            return self.route_slider_event(event, &bounds, &layout, sy);
        }

        match event {
            Event::PointerUp { x, y } => {
                let prev = self.dragging_picker.take();

                // Forward PointerUp to the picker that was being dragged.
                if let Some(i) = prev {
                    let wb = self.picker_header_wb(&bounds, sy(layout.color_pick_y[i]));
                    let resp = self.color_pickers[i].event(event, &wb);
                    if let Some(WidgetAction::ValueChanged { ref value, .. }) = resp.action {
                        let key = COLOR_FIELDS[i].0;
                        let _ = self.set_value(key, value);
                        self.mark_custom();
                    }
                    return resp;
                }

                // A slider may finish outside its row. Route the release
                // before applying coordinate-based widget dispatch so its
                // internal dragging state cannot get stuck.
                let slider_resp = self.route_slider_event(event, &bounds, &layout, sy);
                if slider_resp.consumed {
                    return slider_resp;
                }

                if !bounds.contains(*x, *y) {
                    return EventResponse::ignored();
                }

                // Normal PointerUp routing for non-drag interactions.
                self.route_pointer_event(event, &bounds, &layout, sy)
            }

            Event::PointerDown { x, y } => {
                if !bounds.contains(*x, *y) {
                    return EventResponse::ignored();
                }

                let font_family_bounds = label_widget_wb(&bounds, sy(layout.font_family_row_y));
                let mono_family_bounds = label_widget_wb(&bounds, sy(layout.mono_family_row_y));
                if !font_family_bounds.contains(*x, *y) {
                    self.font_family.focused = false;
                }
                if !mono_family_bounds.contains(*x, *y) {
                    self.mono_family.focused = false;
                }

                // Click-outside: dismiss expanded pickers not containing this click.
                for (i, _) in COLOR_FIELDS.iter().enumerate() {
                    if self.color_pickers[i].expanded {
                        let full_wb = Rect {
                            x: bounds.x + H_PAD,
                            y: sy(layout.color_pick_y[i]),
                            width: bounds.width - H_PAD * 2.0,
                            height: layout.color_pick_h[i],
                        };
                        if !full_wb.contains(*x, *y) {
                            self.color_pickers[i].dismiss_overlay();
                        }
                    }
                }

                let resp = self.route_pointer_event(event, &bounds, &layout, sy);
                // Record which picker started a drag (if any consumed the event).
                if resp.consumed && self.dragging_picker.is_none() {
                    for i in 0..6 {
                        let full_wb = Rect {
                            x: bounds.x + H_PAD,
                            y: sy(layout.color_pick_y[i]),
                            width: bounds.width - H_PAD * 2.0,
                            height: layout.color_pick_h[i],
                        };
                        if full_wb.contains(*x, *y) {
                            self.dragging_picker = Some(i);
                            break;
                        }
                    }
                }
                resp
            }

            Event::KeyPress { .. } => {
                if self.font_family.focused {
                    let resp = self.font_family.event(event, &Rect::default());
                    if let Some(WidgetAction::ValueChanged { ref value, .. }) = resp.action {
                        let _ = self.set_value("fonts.family", value);
                    }
                    return resp;
                }
                if self.mono_family.focused {
                    let resp = self.mono_family.event(event, &Rect::default());
                    if let Some(WidgetAction::ValueChanged { ref value, .. }) = resp.action {
                        let _ = self.set_value("fonts.mono_family", value);
                    }
                    return resp;
                }
                for (i, (key, _)) in COLOR_FIELDS.iter().enumerate() {
                    if self.color_pickers[i].expanded {
                        let dummy = Rect::default();
                        let resp = self.color_pickers[i].event(event, &dummy);
                        if let Some(WidgetAction::ValueChanged { ref value, .. }) = resp.action {
                            let _ = self.set_value(key, value);
                            self.mark_custom();
                        }
                        if resp.consumed {
                            return resp;
                        }
                    }
                }
                self.route_slider_event(event, &bounds, &layout, sy)
            }

            _ => EventResponse::ignored(),
        }
    }
}

impl AppearancePanel {
    fn route_slider_event(
        &mut self,
        event: &Event,
        bounds: &Rect,
        layout: &PanelLayout,
        sy: impl Fn(f32) -> f32,
    ) -> EventResponse {
        let controls = [
            (
                "fonts.size",
                label_widget_wb(bounds, sy(layout.font_size_row_y)),
            ),
            (
                "style.border_radius",
                label_widget_wb(bounds, sy(layout.border_row_y)),
            ),
            (
                "style.opacity",
                label_widget_wb(bounds, sy(layout.opacity_row_y)),
            ),
        ];

        for (index, (key, widget_bounds)) in controls.into_iter().enumerate() {
            let response = match index {
                0 => self.font_size.event(event, &widget_bounds),
                1 => self.border_radius.event(event, &widget_bounds),
                _ => self.opacity.event(event, &widget_bounds),
            };
            if response.consumed {
                if let Some(WidgetAction::ValueChanged { ref value, .. }) = response.action {
                    let _ = self.set_value(key, value);
                }
                return response;
            }
        }

        EventResponse::ignored()
    }

    /// Shared pointer-event routing for both PointerDown and PointerUp.
    fn route_pointer_event(
        &mut self,
        event: &Event,
        bounds: &Rect,
        layout: &PanelLayout,
        sy: impl Fn(f32) -> f32,
    ) -> EventResponse {
        let (x, y) = match event {
            Event::PointerDown { x, y } | Event::PointerUp { x, y } => (*x, *y),
            _ => return EventResponse::ignored(),
        };

        // ── Preset dropdown ─────────────────────────────────────────────────
        let drop_wb = Rect {
            x: bounds.x + H_PAD,
            y: sy(layout.preset_drop_y),
            width: bounds.width - H_PAD * 2.0,
            height: layout.preset_drop_h,
        };
        if drop_wb.contains(x, y) {
            let resp = self.preset_dropdown.event(event, &drop_wb);
            if let Some(WidgetAction::ValueChanged { ref value, .. }) = resp.action {
                let presets = Palette::builtin_presets();
                if let Some(idx) = presets.iter().position(|(name, _)| name == value) {
                    self.apply_preset(idx);
                    self.preset_dropdown.selected = idx;
                }
            }
            return resp;
        }

        // ── Color pickers ───────────────────────────────────────────────────
        for (i, (key, _)) in COLOR_FIELDS.iter().enumerate() {
            let full_wb = Rect {
                x: bounds.x + H_PAD,
                y: sy(layout.color_pick_y[i]),
                width: bounds.width - H_PAD * 2.0,
                height: layout.color_pick_h[i],
            };
            if full_wb.contains(x, y) {
                let hdr_wb = self.picker_header_wb(bounds, sy(layout.color_pick_y[i]));
                let resp = self.color_pickers[i].event(event, &hdr_wb);
                if let Some(WidgetAction::ValueChanged { ref value, .. }) = resp.action {
                    let _ = self.set_value(key, value);
                    self.mark_custom();
                    tracing::debug!("palette color {key} → {value}");
                }
                // When a picker expands, close all others.
                if self.color_pickers[i].expanded {
                    for j in 0..6 {
                        if j != i {
                            self.color_pickers[j].dismiss_overlay();
                        }
                    }
                }
                return resp;
            }
        }

        // ── Font family ─────────────────────────────────────────────────────
        let ff_wb = label_widget_wb(bounds, sy(layout.font_family_row_y));
        if ff_wb.contains(x, y) {
            let resp = self.font_family.event(event, &ff_wb);
            if let Some(WidgetAction::ValueChanged { ref value, .. }) = resp.action {
                let _ = self.set_value("fonts.family", value);
            }
            return resp;
        }

        // ── Mono family ─────────────────────────────────────────────────────
        let mf_wb = label_widget_wb(bounds, sy(layout.mono_family_row_y));
        if mf_wb.contains(x, y) {
            let resp = self.mono_family.event(event, &mf_wb);
            if let Some(WidgetAction::ValueChanged { ref value, .. }) = resp.action {
                let _ = self.set_value("fonts.mono_family", value);
            }
            return resp;
        }

        // ── Font size slider ─────────────────────────────────────────────────
        let fs_wb = label_widget_wb(bounds, sy(layout.font_size_row_y));
        if fs_wb.contains(x, y) {
            let resp = self.font_size.event(event, &fs_wb);
            if let Some(WidgetAction::ValueChanged { ref value, .. }) = resp.action {
                let _ = self.set_value("fonts.size", value);
            }
            return resp;
        }

        // ── Border radius slider ─────────────────────────────────────────────
        let br_wb = label_widget_wb(bounds, sy(layout.border_row_y));
        if br_wb.contains(x, y) {
            let resp = self.border_radius.event(event, &br_wb);
            if let Some(WidgetAction::ValueChanged { ref value, .. }) = resp.action {
                let _ = self.set_value("style.border_radius", value);
            }
            return resp;
        }

        // ── Opacity slider ───────────────────────────────────────────────────
        let op_wb = label_widget_wb(bounds, sy(layout.opacity_row_y));
        if op_wb.contains(x, y) {
            let resp = self.opacity.event(event, &op_wb);
            if let Some(WidgetAction::ValueChanged { ref value, .. }) = resp.action {
                let _ = self.set_value("style.opacity", value);
            }
            return resp;
        }

        EventResponse::ignored()
    }
}

// ── Painting helpers ────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)] // Rendering primitives map directly to the panel layout.
fn paint_section(
    pixmap: &mut tiny_skia::PixmapMut<'_>,
    ctx: &mut hyprcube_core::text::RenderContext<'_>,
    bx: f32,
    screen_y: f32,
    width: f32,
    title: &str,
    font_size: f32,
    fg: Color,
    overlay: Color,
) {
    let text_y = screen_y + (SECTION_H - 1.0 - (font_size + 2.0) * 1.3) / 2.0;
    ctx.text.draw_text(
        pixmap,
        title,
        bx + H_PAD,
        text_y,
        font_size + 2.0,
        fg,
        width,
    );
    fill_rounded_rect(
        pixmap,
        bx + H_PAD,
        screen_y + SECTION_H - 1.0,
        width - H_PAD * 2.0,
        1.0,
        0.0,
        overlay,
    );
}

#[allow(clippy::too_many_arguments)] // Rendering primitives map directly to the panel layout.
fn paint_label_widget_row(
    pixmap: &mut tiny_skia::PixmapMut<'_>,
    ctx: &mut hyprcube_core::text::RenderContext<'_>,
    bounds: &Rect,
    screen_y: f32,
    label: &str,
    widget: &dyn Widget,
    font_size: f32,
    fg: Color,
) {
    let label_max_w = bounds.width * 0.5 - H_PAD;
    let label_y = screen_y + (ROW_H - font_size * 1.3) / 2.0;
    ctx.text.draw_text(
        pixmap,
        label,
        bounds.x + H_PAD,
        label_y,
        font_size,
        fg,
        label_max_w,
    );

    let wb = label_widget_wb(bounds, screen_y);
    widget.paint(pixmap, wb, ctx);
}

fn label_widget_wb(bounds: &Rect, screen_y: f32) -> Rect {
    let widget_w = bounds.width * 0.5 - H_PAD;
    Rect {
        x: bounds.x + bounds.width * 0.5,
        y: screen_y + (ROW_H - 32.0) / 2.0,
        width: widget_w,
        height: 32.0,
    }
}

fn color_hex_from_palette(pal: &Palette, key: &str) -> String {
    match key {
        "colors.background" => pal.colors.background.clone(),
        "colors.foreground" => pal.colors.foreground.clone(),
        "colors.accent" => pal.colors.accent.clone(),
        "colors.urgent" => pal.colors.urgent.clone(),
        "colors.surface" => pal.colors.surface.clone(),
        "colors.overlay" => pal.colors.overlay.clone(),
        _ => "#000000".into(),
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_palette_fields() {
        let panel = AppearancePanel::with_arc(Arc::new(Mutex::new(Palette::default())));
        let fields = panel.fields();
        assert_eq!(fields.len(), 11);
        let bg = fields
            .iter()
            .find(|f| f.key == "colors.background")
            .unwrap();
        assert_eq!(bg.current_value, "#1e1e2e");
    }

    #[test]
    fn set_color_value() {
        let mut panel = AppearancePanel::with_arc(Arc::new(Mutex::new(Palette::default())));
        let old = panel.set_value("colors.accent", "#ff0000").unwrap();
        assert_eq!(old, "#89b4fa");
        assert_eq!(panel.palette.lock().unwrap().colors.accent, "#ff0000");
    }

    #[test]
    fn set_float_value() {
        let mut panel = AppearancePanel::with_arc(Arc::new(Mutex::new(Palette::default())));
        let old = panel.set_value("style.opacity", "0.95").unwrap();
        assert_eq!(old, "0.85");
        assert!((panel.palette.lock().unwrap().style.opacity - 0.95).abs() < f64::EPSILON);
    }

    #[test]
    fn set_invalid_float() {
        let mut panel = AppearancePanel::with_arc(Arc::new(Mutex::new(Palette::default())));
        let err = panel.set_value("fonts.size", "not_a_number").unwrap_err();
        assert!(matches!(err, PanelError::InvalidValue { .. }));
    }

    #[test]
    fn preset_dropdown_has_seven_options() {
        let panel = AppearancePanel::with_arc(Arc::new(Mutex::new(Palette::default())));
        assert_eq!(panel.preset_dropdown.options.len(), 7);
        assert_eq!(panel.preset_dropdown.options.last().unwrap(), "Custom");
    }

    #[test]
    fn apply_nord_preset_changes_background() {
        let mut panel = AppearancePanel::with_arc(Arc::new(Mutex::new(Palette::default())));
        panel.apply_preset(2);
        let pal = panel.palette.lock().unwrap();
        assert_eq!(pal.colors.background, "#2e3440");
    }

    #[test]
    fn six_color_pickers_created() {
        let panel = AppearancePanel::with_arc(Arc::new(Mutex::new(Palette::default())));
        assert_eq!(panel.color_pickers.len(), 6);
    }

    #[test]
    fn dragging_picker_starts_none() {
        let panel = AppearancePanel::with_arc(Arc::new(Mutex::new(Palette::default())));
        assert!(panel.dragging_picker.is_none());
    }
}
