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
}
