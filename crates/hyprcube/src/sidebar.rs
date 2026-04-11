use cosmic_text::{
    Attrs, Buffer as CosmicBuffer, Family, FontSystem, Metrics, Shaping, SwashCache,
};
use hyprcube_core::color::Color;
use hyprcube_core::render::fill_rounded_rect;
use tiny_skia::PixmapMut;

pub const SIDEBAR_WIDTH: f32 = 220.0;
pub const ITEM_HEIGHT: f32 = 44.0;
const ICON_SIZE: f32 = 16.0;
const ICON_LEFT: f32 = 16.0;
const TEXT_LEFT: f32 = 44.0;

/// Paint the sidebar onto the pixmap.
pub fn paint(
    pixmap: &mut PixmapMut<'_>,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
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
        fill_rounded_rect(pixmap, ICON_LEFT, icon_y, ICON_SIZE, ICON_SIZE, 3.0, icon_color);

        // Title text
        let text_color = if i == active_index {
            Color::new(1.0, 1.0, 1.0, 1.0)
        } else {
            foreground_color
        };
        let text_y = y + (ITEM_HEIGHT - font_size * 1.2) / 2.0;
        draw_text(
            pixmap,
            font_system,
            swash_cache,
            title,
            TEXT_LEFT,
            text_y,
            SIDEBAR_WIDTH - TEXT_LEFT - 8.0,
            font_size,
            text_color,
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

/// Draw text onto the pixmap using cosmic-text.
fn draw_text(
    pixmap: &mut PixmapMut<'_>,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    text: &str,
    x: f32,
    y: f32,
    max_width: f32,
    font_size: f32,
    color: Color,
) {
    let metrics = Metrics::new(font_size, font_size * 1.3);
    let mut buffer = CosmicBuffer::new(font_system, metrics);
    buffer.set_size(font_system, Some(max_width), Some(font_size * 2.0));
    buffer.set_text(
        font_system,
        text,
        Attrs::new().family(Family::SansSerif),
        Shaping::Advanced,
    );
    buffer.shape_until_scroll(font_system, false);

    let offset_x = x as i32;
    let offset_y = y as i32;
    let img_w = pixmap.width() as i32;
    let img_h = pixmap.height() as i32;

    let cosmic_color = cosmic_text::Color::rgba(
        (color.r * 255.0) as u8,
        (color.g * 255.0) as u8,
        (color.b * 255.0) as u8,
        (color.a * 255.0) as u8,
    );

    buffer.draw(
        font_system,
        swash_cache,
        cosmic_color,
        |gx, gy, gw, gh, drawn_color| {
            let alpha = drawn_color.a();
            if alpha == 0 {
                return;
            }
            let a = alpha as f32 / 255.0;
            let sr = drawn_color.r() as f32;
            let sg = drawn_color.g() as f32;
            let sb = drawn_color.b() as f32;

            for dy in 0..gh as i32 {
                for dx in 0..gw as i32 {
                    let px = offset_x + gx + dx;
                    let py = offset_y + gy + dy;
                    if px < 0 || px >= img_w || py < 0 || py >= img_h {
                        continue;
                    }
                    let idx = (py as usize * img_w as usize + px as usize) * 4;
                    let data = pixmap.data_mut();
                    // Alpha-blend text over background (background is always opaque)
                    data[idx] = (sr * a + data[idx] as f32 * (1.0 - a)) as u8;
                    data[idx + 1] = (sg * a + data[idx + 1] as f32 * (1.0 - a)) as u8;
                    data[idx + 2] = (sb * a + data[idx + 2] as f32 * (1.0 - a)) as u8;
                    data[idx + 3] = 255;
                }
            }
        },
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
