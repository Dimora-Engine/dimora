/// Linear-space RGBA color, `f32` per channel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Default for Color {
    fn default() -> Self {
        Self::WHITE
    }
}

impl Color {
    pub const WHITE: Color = Color::rgb(1.0, 1.0, 1.0);
    pub const BLACK: Color = Color::rgb(0.0, 0.0, 0.0);
    pub const TRANSPARENT: Color = Color::rgba(0.0, 0.0, 0.0, 0.0);

    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Builds a color from 8-bit sRGB channels (the usual authoring space), converting to linear.
    pub fn from_srgb8(r: u8, g: u8, b: u8) -> Self {
        Self::rgb(srgb_to_linear(r as f32 / 255.0), srgb_to_linear(g as f32 / 255.0), srgb_to_linear(b as f32 / 255.0))
    }

    pub fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    pub fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// Multiplies RGB (not alpha) by `k`; handy for light intensity.
    pub fn scaled(self, k: f32) -> Self {
        Self { r: self.r * k, g: self.g * k, b: self.b * k, a: self.a }
    }
}

pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_endpoints() {
        assert_eq!(Color::from_srgb8(0, 0, 0), Color::BLACK);
        assert!((Color::from_srgb8(255, 255, 255).g - 1.0).abs() < 1e-6);
        assert!(Color::from_srgb8(128, 128, 128).r < 0.25);
    }
}
