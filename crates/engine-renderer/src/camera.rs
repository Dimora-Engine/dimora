use engine_core::Color;
use engine_math::{Mat4, Vec2, Vec3};
use engine_world::GlobalTransform;

use crate::projection::{CameraProjection, Projection, ScreenPosition};
use crate::render_target::RenderTargetId;

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

/// Where a camera's image is written.
///
/// [`CameraOutput::Surface`] is the window or the headless color target passed to the renderer.
/// [`CameraOutput::Target`] is an offscreen image owned by the renderer and described by
/// [`crate::RenderTargetStore`]. The default from [`Camera::new`] is [`CameraOutput::Surface`].
///
/// This is CPU data. It does not hold a GPU texture.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CameraOutput {
    /// The surface being presented or read back for this frame.
    #[default]
    Surface,
    /// An offscreen target. A missing id does not fall back to the surface.
    Target(RenderTargetId),
}

/// What a camera does to the color already in its viewport before drawing its scene.
///
/// [`CameraClear::Color`] replaces that rectangle with [`Camera::clear_color`].
/// [`CameraClear::Load`] keeps the color already there. The default is [`CameraClear::Color`].
///
/// A render-pass clear covers the whole attachment. A later camera's [`CameraClear::Color`] is
/// therefore a viewport-sized draw, not another attachment clear. The first camera that writes a
/// destination in a frame still clears that whole image to its `clear_color`, including when its
/// policy is `Load`, so the frame never loads an uninitialized image. `Load` and `Color` differ
/// from the second camera on that same destination. A surface camera and an offscreen target do
/// not share that first clear.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CameraClear {
    /// Replace this camera's viewport with [`Camera::clear_color`] before its scene.
    #[default]
    Color,
    /// Keep the color already in this viewport, then draw the scene over it.
    Load,
}

/// Camera component. Its *position and orientation* come from the entity's `Transform`; this
/// says how view space is projected ([`CameraProjection`]) and where the image lands on the render
/// target ([`ViewportRect`]). Moving the camera in 3D is what produces parallax.
///
/// Every [`Self::active`] camera is rendered, in ascending `(order, entity id)`. A lower order
/// draws first and sits underneath. A higher order draws later and can overlay it. There is no
/// separate priority field. `Visibility` does not activate or hide a camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub projection: CameraProjection,
    pub viewport: ViewportRect,
    pub clear_color: Color,
    /// Viewport color policy. The default from [`Camera::new`] is [`CameraClear::Color`].
    pub clear: CameraClear,
    /// Destination of this camera. [`CameraOutput::Surface`] unless [`Camera::with_output`] says otherwise.
    pub output: CameraOutput,
    pub active: bool,
    /// Draw order among active cameras. Lower draws first. Equal orders use the entity id.
    pub order: i32,
}

impl Camera {
    pub fn new(projection: CameraProjection) -> Self {
        Self {
            projection,
            viewport: ViewportRect::FULL,
            clear_color: Color::rgb(0.05, 0.06, 0.09),
            clear: CameraClear::Color,
            output: CameraOutput::Surface,
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
    pub fn with_clear(mut self, clear: CameraClear) -> Self {
        self.clear = clear;
        self
    }
    pub fn with_output(mut self, output: CameraOutput) -> Self {
        self.output = output;
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
    ///
    /// [`CameraProjection::Perspective`] and [`CameraProjection::Orthographic`] rebuild `proj`
    /// from this viewport's pixel aspect. [`CameraProjection::Custom`] keeps the stored view-to-clip
    /// matrix; a resize changes pixel mapping only.
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

    /// Whether `window_pixel` lies in this viewport. Coordinates are target pixels, origin
    /// top-left. The rectangle is half-open: the far edges belong to a neighbor that starts there.
    pub fn contains_window_pixel(&self, window_pixel: Vec2) -> bool {
        let (origin, size) = (self.viewport_origin, self.viewport);
        window_pixel.x >= origin.x
            && window_pixel.y >= origin.y
            && window_pixel.x < origin.x + size.x
            && window_pixel.y < origin.y + size.y
    }

    /// `window_pixel - viewport_origin`. [`Self::ray_from_pixels`] and [`Self::project`] use
    /// this viewport-local space, not window pixels.
    pub fn window_to_viewport(&self, window_pixel: Vec2) -> Vec2 {
        window_pixel - self.viewport_origin
    }

    /// World-space ray through a pixel (origin top-left of this viewport): `(origin, unit direction)`.
    ///
    /// The ray is the unprojection of NDC depth 0 and 1 through `view_proj`. `None` when that
    /// matrix has no reliable inverse, including a singular custom projection. Rendering does not
    /// require the inverse and does not substitute a ray.
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
