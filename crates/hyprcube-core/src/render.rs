use tiny_skia::{Paint, PathBuilder, PixmapMut, Transform};

use crate::color::Color;

/// Draw a filled rounded rectangle onto `pixmap`.
pub fn fill_rounded_rect(
    pixmap: &mut PixmapMut<'_>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
    color: Color,
) {
    let r = radius.min(width / 2.0).min(height / 2.0);

    let mut pb = PathBuilder::new();

    // Start at top-left, after the corner radius.
    pb.move_to(x + r, y);

    // Top edge → top-right corner
    pb.line_to(x + width - r, y);
    pb.quad_to(x + width, y, x + width, y + r);

    // Right edge → bottom-right corner
    pb.line_to(x + width, y + height - r);
    pb.quad_to(x + width, y + height, x + width - r, y + height);

    // Bottom edge → bottom-left corner
    pb.line_to(x + r, y + height);
    pb.quad_to(x, y + height, x, y + height - r);

    // Left edge → top-left corner
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);

    pb.close();

    let path = match pb.finish() {
        Some(p) => p,
        None => return,
    };

    let mut paint = Paint::default();
    let skia_color = color.to_skia();
    paint.set_color(skia_color);
    paint.anti_alias = true;

    pixmap.fill_path(
        &path,
        &paint,
        tiny_skia::FillRule::Winding,
        Transform::identity(),
        None,
    );
}
