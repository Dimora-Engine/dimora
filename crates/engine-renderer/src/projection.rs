//! Projection: how real 3D world positions become screen positions.
//!
//! A [`Projection`] only provides the view-space -> clip-space matrix; the camera's `Transform`
//! provides the view. Because the engine always projects true 3D positions, parallax is a
//! consequence rather than a feature: under [`Perspective`] a camera move shifts near objects more
//! than far ones, under [`Orthographic`] it shifts them all equally, and no object carries a
//! "parallax factor".

use engine_math::{Mat4, Vec2, Vec3, Vec4};

use crate::camera::CameraView;

/// Result of projecting a world position onto the viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenPosition {
    /// Pixels, origin at the top-left of the viewport, +Y down.
    pub pixels: Vec2,
    /// Normalized device coordinates: `[-1, 1]` on screen, +Y up.
    pub ndc: Vec2,
    /// Depth-buffer value, `0` at the near plane and `1` at the far plane.
    pub depth: f32,
    /// Distance in front of the camera along its forward axis, in world units.
    pub view_depth: f32,
}

impl ScreenPosition {
    pub(crate) fn from_clip(clip: Vec4, view_depth: f32, viewport: Vec2) -> Option<Self> {
        if clip.w <= 1e-6 {
            return None;
        }
        let ndc = Vec2::new(clip.x / clip.w, clip.y / clip.w);
        Some(Self {
            pixels: Vec2::new((ndc.x * 0.5 + 0.5) * viewport.x, (0.5 - ndc.y * 0.5) * viewport.y),
            ndc,
            depth: clip.z / clip.w,
            view_depth,
        })
    }

    /// True if the point lies inside the visible volume.
    pub fn is_visible(&self) -> bool {
        self.ndc.x.abs() <= 1.0 && self.ndc.y.abs() <= 1.0 && (0.0..=1.0).contains(&self.depth)
    }
}

/// Maps view space to clip space. Implement this to add specialized 2.5D projections
/// (oblique/cabinet, isometric with custom axes, ...): anything expressible as a 4x4 matrix works
/// with the GPU backend, culling and picking unchanged.
pub trait Projection {
    /// View-space -> clip-space matrix (left-handed, NDC depth `[0, 1]`).
    fn matrix(&self, aspect: f32) -> Mat4;

    /// Projects a world-space position for the given camera view. `None` if the point is behind
    /// the camera.
    fn project(&self, world_position: Vec3, camera: &CameraView) -> Option<ScreenPosition> {
        let view = camera.view.transform_point3(world_position);
        let clip = self.matrix(camera.aspect()) * camera.view * world_position.extend(1.0);
        ScreenPosition::from_clip(clip, view.z, camera.viewport)
    }
}

/// Parallel projection: size on screen does not depend on depth.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Orthographic {
    /// World units visible vertically; width follows from the aspect ratio.
    pub height: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for Orthographic {
    fn default() -> Self {
        Self { height: 10.0, near: 0.1, far: 1000.0 }
    }
}

impl Projection for Orthographic {
    fn matrix(&self, aspect: f32) -> Mat4 {
        let h = self.height * 0.5;
        Mat4::orthographic_lh(-h * aspect, h * aspect, -h, h, self.near, self.far)
    }
}

/// Pinhole projection: objects shrink and converge with distance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Perspective {
    /// Vertical field of view in radians.
    pub fov_y: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for Perspective {
    fn default() -> Self {
        Self { fov_y: 45f32.to_radians(), near: 0.1, far: 1000.0 }
    }
}

impl Projection for Perspective {
    fn matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_lh(self.fov_y, aspect, self.near, self.far)
    }
}

/// The projection a [`crate::Camera`] uses. Non-exhaustive: isometric, oblique, side-scroller,
/// top-down and custom variants are expected to be added without breaking callers.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum CameraProjection {
    Orthographic(Orthographic),
    Perspective(Perspective),
}

impl CameraProjection {
    pub fn orthographic(height: f32) -> Self {
        Self::Orthographic(Orthographic { height, ..Default::default() })
    }
    pub fn perspective(fov_y_degrees: f32) -> Self {
        Self::Perspective(Perspective { fov_y: fov_y_degrees.to_radians(), ..Default::default() })
    }
    pub fn is_perspective(&self) -> bool {
        matches!(self, Self::Perspective(_))
    }
}

impl Projection for CameraProjection {
    fn matrix(&self, aspect: f32) -> Mat4 {
        match self {
            Self::Orthographic(p) => p.matrix(aspect),
            Self::Perspective(p) => p.matrix(aspect),
        }
    }
}
