use cosmic_text::{
    Attrs, Buffer as CosmicBuffer, Family, FontSystem, Metrics, Shaping, SwashCache,
};
use tiny_skia::PixmapMut;

use crate::color::Color;
use crate::palette::Palette;

pub struct TextRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
}

impl TextRenderer {
    pub fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
        }
    }

    /// Measure text width and height for a given font size and max width.
    pub fn measure_text(&mut self, text: &str, font_size: f32, max_width: f32) -> (f32, f32) {
        let metrics = Metrics::new(font_size, font_size * 1.3);
        let mut buffer = CosmicBuffer::new(&mut self.font_system, metrics);
        buffer.set_size(&mut self.font_system, Some(max_width), None);
        buffer.set_text(
            &mut self.font_system,
            text,
            Attrs::new().family(Family::SansSerif),
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(&mut self.font_system, false);

        let mut w: f32 = 0.0;
        for run in buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                let end = glyph.x + glyph.w;
                if end > w {
                    w = end;
                }
            }
        }
        (w.min(max_width), font_size * 1.3)
    }

    /// Render text into pixmap at (x, y) with the given font size and color.
    #[allow(clippy::too_many_arguments)] // Drawing coordinates and bounds form one rendering operation.
    pub fn draw_text(
        &mut self,
        pixmap: &mut PixmapMut<'_>,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        color: Color,
        max_width: f32,
    ) {
        let metrics = Metrics::new(font_size, font_size * 1.3);
        let mut buffer = CosmicBuffer::new(&mut self.font_system, metrics);
        buffer.set_size(
            &mut self.font_system,
            Some(max_width),
            Some(font_size * 2.0),
        );
        buffer.set_text(
            &mut self.font_system,
            text,
            Attrs::new().family(Family::SansSerif),
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(&mut self.font_system, false);

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
            &mut self.font_system,
            &mut self.swash_cache,
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
                        data[idx] = (sr * a + data[idx] as f32 * (1.0 - a)) as u8;
                        data[idx + 1] = (sg * a + data[idx + 1] as f32 * (1.0 - a)) as u8;
                        data[idx + 2] = (sb * a + data[idx + 2] as f32 * (1.0 - a)) as u8;
                        data[idx + 3] = 255;
                    }
                }
            },
        );
    }
}

impl Default for TextRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Rendering context passed to widget paint methods.
pub struct RenderContext<'a> {
    pub text: &'a mut TextRenderer,
    pub palette: &'a Palette,
}
