use engine_core::Color;
use engine_math::{Mat4, Vec2, Vec3};
use engine_world::GlobalTransform;

use crate::projection::{CameraProjection, Projection, ScreenPosition};

/// Camera component. Its *position and orientation* come from the entity's `Transform`; this only
/// says how view space is projected. Moving the camera in 3D is what produces parallax.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub projection: CameraProjection,
    pub clear_color: Color,
    pub active: bool,
    /// Among active cameras the lowest `order` is rendered.
    pub order: i32,
}

impl Camera {
    pub fn new(projection: CameraProjection) -> Self {
        Self { projection, clear_color: Color::rgb(0.05, 0.06, 0.09), active: true, order: 0 }
    }
    pub fn perspective(fov_y_degrees: f32) -> Self {
        Self::new(CameraProjection::perspective(fov_y_degrees))
    }
    pub fn orthographic(height: f32) -> Self {
        Self::new(CameraProjection::orthographic(height))
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
    /// Viewport size in pixels.
    pub viewport: Vec2,
}

impl CameraView {
    /// Builds the view from the camera entity's world transform. Scale on the camera entity is
    /// ignored.
    pub fn new(camera: &Camera, transform: &GlobalTransform, viewport: Vec2) -> Self {
        let t = transform.to_transform();
        let (position, forward, up) = (t.position, t.forward(), t.up());
        let view = Mat4::look_to_lh(position, forward, up);
        let proj = camera.projection.matrix(viewport.x / viewport.y.max(1.0));
        Self { view, proj, view_proj: proj * view, position, forward, viewport }
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
