//! Window size in physical pixels and the display scale that relates it to logical pixels.
//!
//! Physical pixels are the framebuffer. Logical pixels are that size divided by the scale factor,
//! in the same top-left, +X right, +Y down space as [`engine_core::Input::mouse_position`]. World
//! space is a different system: Y is up, and a DPI change does not move sprites or meshes.
//! Rendering and camera extraction use the physical size. UI that wants window coordinates uses
//! the logical size, or converts a physical pointer through [`Viewport::physical_to_logical`].

use engine_math::Vec2;

/// Why [`Viewport::new`] rejected a scale factor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewportError {
    /// The scale was zero, negative, or not finite.
    InvalidScale,
}

impl std::fmt::Display for ViewportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViewportError::InvalidScale => write!(f, "window scale factor must be finite and greater than zero"),
        }
    }
}

impl std::error::Error for ViewportError {}

/// Physical framebuffer size and the display scale factor.
///
/// Logical size is always `physical / scale_factor`. Changing the scale does not multiply the
/// physical size; a later resize supplies the new framebuffer size. A zero physical size passed
/// to [`Viewport::apply_physical_size`] is ignored, so a minimized window keeps the last
/// positive framebuffer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    physical_width: u32,
    physical_height: u32,
    scale_factor: f64,
}

impl Viewport {
    /// Physical pixel size and a display scale. The scale must be finite and greater than zero.
    /// A zero physical size is stored as given; the window loop uses [`Viewport::apply_physical_size`]
    /// instead, which keeps the previous positive size.
    pub fn new(physical_width: u32, physical_height: u32, scale_factor: f64) -> Result<Self, ViewportError> {
        if !valid_scale(scale_factor) {
            return Err(ViewportError::InvalidScale);
        }
        Ok(Self { physical_width, physical_height, scale_factor })
    }

    /// A headless or not-yet-shown window. The logical size is the given pixel size and the scale is 1.
    pub fn from_logical_pixels(width: u32, height: u32) -> Self {
        Self { physical_width: width, physical_height: height, scale_factor: 1.0 }
    }

    pub fn physical_width(&self) -> u32 {
        self.physical_width
    }

    pub fn physical_height(&self) -> u32 {
        self.physical_height
    }

    /// Framebuffer size in physical pixels, `(width, height)`.
    pub fn physical_size(&self) -> (u32, u32) {
        (self.physical_width, self.physical_height)
    }

    /// Framebuffer size as the `Vec2` camera extraction takes. Units are physical pixels.
    pub fn physical_size_pixels(&self) -> Vec2 {
        Vec2::new(self.physical_width as f32, self.physical_height as f32)
    }

    /// Window size in logical pixels, `physical / scale_factor`.
    pub fn logical_size(&self) -> Vec2 {
        self.physical_to_logical(self.physical_size_pixels())
    }

    /// Display scale. Physical pixels per logical pixel.
    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    /// Physical pixels to logical pixels. Does not clamp to the window, and does not flip Y.
    pub fn physical_to_logical(&self, physical_px: Vec2) -> Vec2 {
        let scale = self.scale_factor;
        Vec2::new((physical_px.x as f64 / scale) as f32, (physical_px.y as f64 / scale) as f32)
    }

    /// Logical pixels to physical pixels. Does not clamp to the window, and does not flip Y.
    pub fn logical_to_physical(&self, logical_px: Vec2) -> Vec2 {
        let scale = self.scale_factor;
        Vec2::new((logical_px.x as f64 * scale) as f32, (logical_px.y as f64 * scale) as f32)
    }

    /// Stores a new framebuffer size. Width or height of zero leaves this viewport unchanged
    /// and returns `false`.
    pub fn apply_physical_size(&mut self, width: u32, height: u32) -> bool {
        if width == 0 || height == 0 {
            return false;
        }
        let changed = self.physical_width != width || self.physical_height != height;
        self.physical_width = width;
        self.physical_height = height;
        changed
    }

    /// Stores a new scale without changing the physical size. A non-finite or non-positive scale
    /// leaves this viewport unchanged and returns `false`.
    pub fn apply_scale_factor(&mut self, scale_factor: f64) -> bool {
        if !valid_scale(scale_factor) {
            return false;
        }
        let changed = self.scale_factor != scale_factor;
        self.scale_factor = scale_factor;
        changed
    }
}

fn valid_scale(scale: f64) -> bool {
    scale.is_finite() && scale > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: Vec2, b: Vec2) {
        assert!((a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3, "{a:?} vs {b:?}");
    }

    #[test]
    fn logical_size_is_physical_divided_by_scale() {
        let viewport = Viewport::new(2560, 1440, 2.0).unwrap();
        assert_eq!(viewport.physical_size(), (2560, 1440));
        assert_eq!(viewport.physical_size_pixels(), Vec2::new(2560.0, 1440.0));
        assert_eq!(viewport.scale_factor(), 2.0);
        assert_eq!(viewport.logical_size(), Vec2::new(1280.0, 720.0));
        assert_eq!(Viewport::from_logical_pixels(1280, 720).physical_size(), (1280, 720));
        assert_eq!(Viewport::from_logical_pixels(1280, 720).scale_factor(), 1.0);
    }

    #[test]
    fn a_non_integer_scale_round_trips_including_points_outside_the_window() {
        let viewport = Viewport::new(1500, 750, 1.5).unwrap();
        assert_eq!(viewport.logical_size(), Vec2::new(1000.0, 500.0));
        let samples = [
            Vec2::new(0.0, 0.0),
            Vec2::new(3.0, 4.5),
            Vec2::new(1500.0, 750.0),
            Vec2::new(-12.0, 20.0),
            Vec2::new(1800.0, -6.0),
        ];
        for physical in samples {
            let logical = viewport.physical_to_logical(physical);
            approx(viewport.logical_to_physical(logical), physical);
            approx(logical, Vec2::new(physical.x / 1.5, physical.y / 1.5));
        }
        let awkward = Viewport::new(1111, 777, 1.1).unwrap();
        let point = Vec2::new(333.25, -40.5);
        approx(awkward.logical_to_physical(awkward.physical_to_logical(point)), point);
        assert_eq!(awkward.physical_to_logical(Vec2::new(0.0, 11.0)).x, 0.0);
        assert!(awkward.physical_to_logical(Vec2::new(0.0, 11.0)).y > 0.0);
    }

    #[test]
    fn invalid_scales_are_rejected_and_zero_resizes_keep_the_last_framebuffer() {
        assert_eq!(Viewport::new(10, 10, 0.0), Err(ViewportError::InvalidScale));
        assert_eq!(Viewport::new(10, 10, -2.0), Err(ViewportError::InvalidScale));
        assert_eq!(Viewport::new(10, 10, f64::NAN), Err(ViewportError::InvalidScale));
        assert_eq!(Viewport::new(10, 10, f64::INFINITY), Err(ViewportError::InvalidScale));
        let explicit_zero = Viewport::new(0, 0, 1.0).unwrap();
        assert_eq!(explicit_zero.physical_size(), (0, 0));
        assert_eq!(explicit_zero.logical_size(), Vec2::ZERO);

        let mut viewport = Viewport::new(800, 600, 2.0).unwrap();
        assert!(!viewport.apply_physical_size(0, 600));
        assert!(!viewport.apply_physical_size(800, 0));
        assert_eq!(viewport.physical_size(), (800, 600));
        assert_eq!(viewport.logical_size(), Vec2::new(400.0, 300.0));
        assert!(!viewport.apply_scale_factor(0.0));
        assert!(!viewport.apply_scale_factor(f64::NAN));
        assert_eq!(viewport.scale_factor(), 2.0);
    }

    #[test]
    fn resize_and_scale_events_do_not_apply_the_scale_twice() {
        let mut viewport = Viewport::new(2000, 1000, 2.0).unwrap();
        assert_eq!(viewport.logical_size(), Vec2::new(1000.0, 500.0));
        assert!(viewport.apply_physical_size(1920, 1080));
        assert_eq!(viewport.scale_factor(), 2.0);
        assert_eq!(viewport.logical_size(), Vec2::new(960.0, 540.0));

        assert!(viewport.apply_scale_factor(1.5));
        assert_eq!(viewport.physical_size(), (1920, 1080));
        assert!(viewport.apply_physical_size(1440, 810));
        assert_eq!(viewport.scale_factor(), 1.5);
        assert_eq!(viewport.logical_size(), Vec2::new(960.0, 540.0));
        assert!(!viewport.apply_scale_factor(1.5));
        assert!(!viewport.apply_physical_size(1440, 810));
    }
}
