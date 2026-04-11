/// A 2D point.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// An axis-aligned rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    /// Returns `true` if the point `(px, py)` lies inside this rectangle.
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }
}

/// Layout direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Vertical,
    Horizontal,
}

/// Alignment along the cross axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
}

/// A simple flex-layout container descriptor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flex {
    pub direction: Direction,
    pub gap: f32,
    pub padding: f32,
    pub align: Align,
}

impl Default for Flex {
    fn default() -> Self {
        Self {
            direction: Direction::Vertical,
            gap: 0.0,
            padding: 0.0,
            align: Align::Start,
        }
    }
}
