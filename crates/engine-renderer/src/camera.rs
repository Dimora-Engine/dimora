use engine_core::Color;
use engine_math::{Mat4, Vec2, Vec3};
use engine_world::GlobalTransform;

use crate::projection::{CameraProjection, Projection, ScreenPosition};

/// The part of the render target a camera draws into, in normalized target coordinates with the
/// origin at the **top-left** (`x`, `y`, `width`, `height` all in `[0, 1]`). The default fills the
/// whole target. The projection's aspect ratio comes from this rectangle's pixel size, so a camera
/// in half the window is not stretched.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewportRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl ViewportRect {
    pub const FULL: ViewportRect = ViewportRect { x: 0.0, y: 0.0, width: 1.0, height: 1.0 };

    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    /// `(origin, size)` in pixels for a target of `target` pixels, clamped to the target and at
    /// least one pixel in each dimension.
    pub fn to_pixels(&self, target: Vec2) -> (Vec2, Vec2) {
        let x = (self.x.clamp(0.0, 1.0) * target.x).floor();
        let y = (self.y.clamp(0.0, 1.0) * target.y).floor();
        let w = (self.width.clamp(0.0, 1.0) * target.x).round().min(target.x - x).max(1.0);
        let h = (self.height.clamp(0.0, 1.0) * target.y).round().min(target.y - y).max(1.0);
        (Vec2::new(x, y), Vec2::new(w, h))
    }
}

impl Default for ViewportRect {
    fn default() -> Self {
        Self::FULL
    }
}

/// Camera component. Its *position and orientation* come from the entity's `Transform`; this
/// says how view space is projected ([`CameraProjection`]) and where the image lands on the render
/// target ([`ViewportRect`]). Moving the camera in 3D is what produces parallax.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub projection: CameraProjection,
    pub viewport: ViewportRect,
    pub clear_color: Color,
    pub active: bool,
    /// Among active cameras the lowest `order` is rendered.
    pub order: i32,
}

impl Camera {
    pub fn new(projection: CameraProjection) -> Self {
        Self {
            projection,
            viewport: ViewportRect::FULL,
            clear_color: Color::rgb(0.05, 0.06, 0.09),
            active: true,
            order: 0,
        }
    }
    pub fn perspective(fov_y_degrees: f32) -> Self {
        Self::new(CameraProjection::perspective(fov_y_degrees))
    }
    pub fn orthographic(height: f32) -> Self {
        Self::new(CameraProjection::orthographic(height))
    }
    pub fn with_viewport(mut self, viewport: ViewportRect) -> Self {
        self.viewport = viewport;
        self
    }
    pub fn with_clear_color(mut self, c: Color) -> Self {
        self.clear_color = c;
        self
    }
}

/// A camera resolved against a viewport: everything needed to project, cull and render.
#[derive(Clone, Copy, Debug)]
pub struct CameraView {
    /// World -> view.
    pub view: Mat4,
    /// View -> clip.
    pub proj: Mat4,
    /// World -> clip.
    pub view_proj: Mat4,
    pub position: Vec3,
    pub forward: Vec3,
    pub up: Vec3,
    pub right: Vec3,
    /// Size in pixels of the camera's viewport rectangle (what the aspect ratio is computed from).
    pub viewport: Vec2,
    /// Top-left corner of the viewport rectangle on the render target, in pixels.
    pub viewport_origin: Vec2,
}

impl CameraView {
    /// Resolves `camera` placed by `transform` against a render target of `target_size` pixels.
    /// Scale on the camera entity is ignored.
    ///
    /// The chain is world -> view (inverse of the camera's transform) -> clip (the projection) ->
    /// NDC (perspective divide) -> viewport pixels.
    pub fn new(camera: &Camera, transform: &GlobalTransform, target_size: Vec2) -> Self {
        let t = transform.to_transform();
        let (position, forward, up) = (t.position, t.forward(), t.up());
        let (viewport_origin, viewport) = camera.viewport.to_pixels(target_size);
        let view = Mat4::look_to_lh(position, forward, up);
        let proj = camera.projection.matrix(viewport.x / viewport.y.max(1.0));
        Self { view, proj, view_proj: proj * view, position, forward, up, right: t.right(), viewport, viewport_origin }
    }

    pub fn aspect(&self) -> f32 {
        self.viewport.x / self.viewport.y.max(1.0)
    }

    /// Projects with this view's own matrices (identical to `Projection::project` for the camera's
    /// projection).
    pub fn project(&self, world_position: Vec3) -> Option<ScreenPosition> {
        let clip = self.view_proj * world_position.extend(1.0);
        ScreenPosition::from_clip(clip, self.view.transform_point3(world_position).z, self.viewport)
    }

    /// World-space ray through a pixel (origin top-left): `(origin, unit direction)`. Used for
    /// picking; works for any projection because it unprojects through the inverse matrix.
    pub fn ray_from_pixels(&self, pixels: Vec2) -> Option<(Vec3, Vec3)> {
        let inv = self.view_proj.inverse()?;
        let ndc = Vec2::new(pixels.x / self.viewport.x * 2.0 - 1.0, 1.0 - pixels.y / self.viewport.y * 2.0);
        let unproject = |z: f32| {
            let p = inv * engine_math::Vec4::new(ndc.x, ndc.y, z, 1.0);
            p.xyz() / p.w
        };
        let (near, far) = (unproject(0.0), unproject(1.0));
        Some((near, (far - near).normalized()))
    }
}
