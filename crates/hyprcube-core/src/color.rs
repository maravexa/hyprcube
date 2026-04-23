use thiserror::Error;

#[derive(Debug, Error)]
pub enum ColorError {
    #[error("invalid hex color string: {0}")]
    InvalidHex(String),
}

/// RGBA color with f32 components in the range 0.0–1.0.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Parse a hex color string: `#rrggbb` or `#rrggbbaa`.
    pub fn from_hex(hex: &str) -> Result<Self, ColorError> {
        let hex = hex.strip_prefix('#').unwrap_or(hex);

        match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16)
                    .map_err(|_| ColorError::InvalidHex(format!("#{hex}")))?;
                let g = u8::from_str_radix(&hex[2..4], 16)
                    .map_err(|_| ColorError::InvalidHex(format!("#{hex}")))?;
                let b = u8::from_str_radix(&hex[4..6], 16)
                    .map_err(|_| ColorError::InvalidHex(format!("#{hex}")))?;
                Ok(Self {
                    r: r as f32 / 255.0,
                    g: g as f32 / 255.0,
                    b: b as f32 / 255.0,
                    a: 1.0,
                })
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16)
                    .map_err(|_| ColorError::InvalidHex(format!("#{hex}")))?;
                let g = u8::from_str_radix(&hex[2..4], 16)
                    .map_err(|_| ColorError::InvalidHex(format!("#{hex}")))?;
                let b = u8::from_str_radix(&hex[4..6], 16)
                    .map_err(|_| ColorError::InvalidHex(format!("#{hex}")))?;
                let a = u8::from_str_radix(&hex[6..8], 16)
                    .map_err(|_| ColorError::InvalidHex(format!("#{hex}")))?;
                Ok(Self {
                    r: r as f32 / 255.0,
                    g: g as f32 / 255.0,
                    b: b as f32 / 255.0,
                    a: a as f32 / 255.0,
                })
            }
            _ => Err(ColorError::InvalidHex(format!("#{hex}"))),
        }
    }

    /// Convert to `#rrggbb` (opaque) or `#rrggbbaa` hex string.
    pub fn to_hex(&self) -> String {
        let r = (self.r * 255.0).round() as u8;
        let g = (self.g * 255.0).round() as u8;
        let b = (self.b * 255.0).round() as u8;
        let a = (self.a * 255.0).round() as u8;

        if a == 255 {
            format!("#{r:02x}{g:02x}{b:02x}")
        } else {
            format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
        }
    }

    /// Convert to a `tiny_skia::Color`.
    pub fn to_skia(&self) -> tiny_skia::Color {
        tiny_skia::Color::from_rgba(self.r, self.g, self.b, self.a).unwrap_or(tiny_skia::Color::BLACK)
    }

    /// Convert to HSV: `(hue 0..360, saturation 0..1, value 0..1)`.
    pub fn to_hsv(&self) -> (f32, f32, f32) {
        let r = self.r;
        let g = self.g;
        let b = self.b;

        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;

        let v = max;
        let s = if max > 1e-6 { delta / max } else { 0.0 };
        let h = if delta < 1e-6 {
            0.0
        } else if max == r {
            60.0 * ((g - b) / delta).rem_euclid(6.0)
        } else if max == g {
            60.0 * ((b - r) / delta + 2.0)
        } else {
            60.0 * ((r - g) / delta + 4.0)
        };

        (h, s, v)
    }

    /// Create a color from HSV components plus alpha.
    /// `h` is in 0..360, `s` and `v` are in 0..1, `a` is in 0..1.
    pub fn from_hsv(h: f32, s: f32, v: f32, a: f32) -> Self {
        let c = v * s;
        let h_norm = h / 60.0;
        let x = c * (1.0 - (h_norm % 2.0 - 1.0).abs());
        let m = v - c;

        let (r1, g1, b1) = match h_norm as u32 {
            0 => (c, x, 0.0_f32),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };

        Self::new(
            (r1 + m).clamp(0.0, 1.0),
            (g1 + m).clamp(0.0, 1.0),
            (b1 + m).clamp(0.0, 1.0),
            a.clamp(0.0, 1.0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_rgb() {
        let hex = "#89b4fa";
        let color = Color::from_hex(hex).unwrap();
        assert_eq!(color.to_hex(), hex);
    }

    #[test]
    fn roundtrip_rgba() {
        let hex = "#1e1e2ed9";
        let color = Color::from_hex(hex).unwrap();
        assert_eq!(color.to_hex(), hex);
    }

    #[test]
    fn invalid_hex() {
        assert!(Color::from_hex("#zz0000").is_err());
        assert!(Color::from_hex("#123").is_err());
    }

    #[test]
    fn hsv_roundtrip_red() {
        let c = Color::new(1.0, 0.0, 0.0, 1.0);
        let (h, s, v) = c.to_hsv();
        assert!((h - 0.0).abs() < 0.1);
        assert!((s - 1.0).abs() < 0.01);
        assert!((v - 1.0).abs() < 0.01);
        let back = Color::from_hsv(h, s, v, 1.0);
        assert!((back.r - c.r).abs() < 0.01);
    }

    #[test]
    fn hsv_from_produces_correct_rgb() {
        // Pure green at full saturation and value
        let g = Color::from_hsv(120.0, 1.0, 1.0, 1.0);
        assert!((g.r).abs() < 0.01);
        assert!((g.g - 1.0).abs() < 0.01);
        assert!((g.b).abs() < 0.01);
    }

    #[test]
    fn hsv_white_and_black() {
        let white = Color::from_hsv(0.0, 0.0, 1.0, 1.0);
        assert!((white.r - 1.0).abs() < 0.01);
        assert!((white.g - 1.0).abs() < 0.01);
        assert!((white.b - 1.0).abs() < 0.01);

        let black = Color::from_hsv(0.0, 0.0, 0.0, 1.0);
        assert!(black.r.abs() < 0.01);
        assert!(black.g.abs() < 0.01);
        assert!(black.b.abs() < 0.01);
    }
}
