use hyprcube_core::color::Color;
use hyprcube_core::render::fill_rounded_rect;
use hyprcube_core::text::TextRenderer;
use tiny_skia::PixmapMut;

pub const SIDEBAR_WIDTH: f32 = 220.0;
pub const ITEM_HEIGHT: f32 = 44.0;
const ICON_SIZE: f32 = 16.0;
const ICON_LEFT: f32 = 16.0;
const TEXT_LEFT: f32 = 44.0;

/// Paint the sidebar onto the pixmap.
#[allow(clippy::too_many_arguments)] // The sidebar renderer consumes the complete frame palette and geometry.
pub fn paint(
    pixmap: &mut PixmapMut<'_>,
    text_renderer: &mut TextRenderer,
    titles: &[&str],
    active_index: usize,
    hover_index: Option<usize>,
    surface_color: Color,
    accent_color: Color,
    foreground_color: Color,
    overlay_color: Color,
    height: f32,
    font_size: f32,
) {
    // Sidebar background
    fill_rounded_rect(pixmap, 0.0, 0.0, SIDEBAR_WIDTH, height, 0.0, surface_color);

    for (i, title) in titles.iter().enumerate() {
        let y = i as f32 * ITEM_HEIGHT;

        // Active / hover highlight
        if i == active_index {
            fill_rounded_rect(
                pixmap,
                0.0,
                y,
                SIDEBAR_WIDTH - 1.0,
                ITEM_HEIGHT,
                0.0,
                accent_color,
            );
        } else if hover_index == Some(i) {
            let hover = Color::new(
                (surface_color.r + 0.04).min(1.0),
                (surface_color.g + 0.04).min(1.0),
                (surface_color.b + 0.04).min(1.0),
                surface_color.a,
            );
            fill_rounded_rect(pixmap, 0.0, y, SIDEBAR_WIDTH - 1.0, ITEM_HEIGHT, 0.0, hover);
        }

        // Icon placeholder (colored square)
        let icon_y = y + (ITEM_HEIGHT - ICON_SIZE) / 2.0;
        let icon_color = if i == active_index {
            Color::new(1.0, 1.0, 1.0, 0.9)
        } else {
            accent_color
        };
        fill_rounded_rect(
            pixmap, ICON_LEFT, icon_y, ICON_SIZE, ICON_SIZE, 3.0, icon_color,
        );

        // Title text
        let text_color = if i == active_index {
            Color::new(1.0, 1.0, 1.0, 1.0)
        } else {
            foreground_color
        };
        let text_y = y + (ITEM_HEIGHT - font_size * 1.3) / 2.0;
        text_renderer.draw_text(
            pixmap,
            title,
            TEXT_LEFT,
            text_y,
            font_size,
            text_color,
            SIDEBAR_WIDTH - TEXT_LEFT - 8.0,
        );
    }

    // Separator line between sidebar and content
    fill_rounded_rect(
        pixmap,
        SIDEBAR_WIDTH - 1.0,
        0.0,
        1.0,
        height,
        0.0,
        overlay_color,
    );
}

/// Given a y coordinate within the sidebar, return which panel index was hit.
pub fn hit_test(y: f32, panel_count: usize) -> Option<usize> {
    if y < 0.0 {
        return None;
    }
    let index = (y / ITEM_HEIGHT) as usize;
    if index < panel_count {
        Some(index)
    } else {
        None
    }
}
