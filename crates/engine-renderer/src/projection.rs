//! Projection: how real 3D world positions become screen positions.
//!
//! A [`Projection`] only provides the view-space -> clip-space matrix; the camera's `Transform`
//! provides the view. Because the engine always projects true 3D positions, parallax is a
//! consequence rather than a feature: under [`Perspective`] a camera move shifts near objects more
//! than far ones, under [`Orthographic`] it shifts them all equally, and no object carries a
//! "parallax factor".
//!
//! [`CameraProjection::Custom`] stores a caller-supplied view-to-clip matrix. It is not rebuilt
//! when the viewport changes. [`Perspective`] and [`Orthographic`] still are.

use engine_math::{Mat4, Vec2, Vec3, Vec4};

use crate::camera::CameraView;

/// Result of projecting a world position onto the viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenPosition {
    /// Pixels, origin at the top-left of the viewport, +Y down.
    pub pixels: Vec2,
    /// Normalized device coordinates: `[-1, 1]` on screen, +Y up.
    pub ndc: Vec2,
    /// Depth-buffer value, `1` at the near plane and `0` at the far plane.
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
    /// View-space -> clip-space matrix (left-handed, NDC depth `[0, 1]`, 1 at near and 0 at far).
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

/// The projection a [`crate::Camera`] uses.
///
/// Isometric, top-down and side view are placements of [`Orthographic`] or [`Perspective`], not
/// extra variants. [`Custom`](Self::Custom) is a view-to-clip matrix supplied by game code.
/// [`Self::oblique_orthographic`] and [`Self::oblique_perspective`] build one shear in front of
/// the reverse-Z helpers. They do not reinterpret a matrix that was already stored.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum CameraProjection {
    Orthographic(Orthographic),
    Perspective(Perspective),
    /// A view-to-clip matrix in Dimora's clip convention. The camera's position and orientation
    /// stay on the entity `Transform`; this value is only `proj` in `view_proj = proj * view`.
    ///
    /// The matrix is fixed until game code replaces it. [`Projection::matrix`] ignores `aspect`
    /// and returns this value unchanged, including when the window or [`crate::ViewportRect`]
    /// changes. Pixel mapping (`project`, `ray_from_pixels`) still uses the current viewport
    /// rectangle. [`Perspective`] and [`Orthographic`] keep recomputing from that aspect.
    /// [`Self::with_clip_range`] does not edit this matrix: near and far are already inside it.
    /// Replace the matrix to change clipping.
    ///
    /// Convention, matching [`Mat4::perspective_lh`](engine_math::Mat4::perspective_lh) and
    /// [`Mat4::orthographic_lh`](engine_math::Mat4::orthographic_lh):
    ///
    /// * Left-handed view space, +Y up, +Z forward (into the scene).
    /// * Column vectors, `M * v`, projection applied after the view matrix.
    /// * NDC x and y in `[-1, 1]`, NDC +Y up, viewport origin at the top-left.
    /// * NDC depth in `[0, 1]`: 1 at the near plane, 0 at the far plane.
    /// * The depth buffer compares with greater-than and clears to 0. Opaque and mask writes use that value.
    ///
    /// A matrix is stored as given. OpenGL depth (`[-1, 1]`) and the previous 0-near mapping are not
    /// converted. Rebuild a 0-near matrix with [`Mat4::perspective_lh`](engine_math::Mat4::perspective_lh)
    /// or [`Mat4::orthographic_lh`](engine_math::Mat4::orthographic_lh). A stored 0-near matrix still
    /// draws, and under greater-than its farther fragments win. A finite matrix is accepted even when it
    /// is singular, oblique, off-centre, or unlike either built-in projection. Rendering does not
    /// need an inverse. [`crate::CameraView::ray_from_pixels`] returns `None` when the inverse
    /// cannot be computed. There is no panic on that path.
    ///
    /// Multiplying the matrix by a negative scalar leaves divided NDC unchanged and can make the
    /// CPU frustum agree after its `w` sign correction. The GPU still clips before that divide.
    /// A reversed clip `w` is not treated as the same picture as the positive matrix.
    ///
    /// Transparent ordering stays camera-space depth from the view matrix (larger Z is farther).
    /// It does not use this matrix's clip Z or window Z, so a projection that scrambles view-space
    /// depth relative to occlusion will not get a different sort. `Upright` sprites follow the
    /// camera basis, not the shear in this matrix (D-26). A coplanar tie stays a tie unless a sprite
    /// opts into [`crate::CoplanarPriority`] (ADR-0020).
    Custom(Mat4),
}

impl CameraProjection {
    pub fn orthographic(height: f32) -> Self {
        Self::Orthographic(Orthographic { height, ..Default::default() })
    }
    pub fn perspective(fov_y_degrees: f32) -> Self {
        Self::Perspective(Perspective { fov_y: fov_y_degrees.to_radians(), ..Default::default() })
    }
    /// View-space shear in front of a reverse-Z orthographic projection, stored as [`Custom`](Self::Custom).
    ///
    /// `(x, y, z)` becomes `(x + z * shear.x, y + z * shear.y, z)` before [`Mat4::orthographic_lh`].
    /// View `z` is unchanged, so a smaller view `z` still writes the larger depth. `height` is the
    /// vertical world size, `aspect` is width / height, and `near` / `far` are view-space distances.
    /// Those values are baked into the matrix. A later viewport resize does not rebuild it, and
    /// [`Self::with_clip_range`] does not edit it. `near` may be 0. A negative aspect or height is
    /// refused; pass a finished matrix to [`Self::custom`] when that flip is intentional.
    ///
    /// This does not convert a matrix already stored with [`Self::custom`].
    pub fn oblique_orthographic(
        height: f32,
        aspect: f32,
        near: f32,
        far: f32,
        shear: Vec2,
    ) -> Result<Self, ProjectionError> {
        let half_height = check_window(height, aspect)?;
        check_clip(near, far, true)?;
        check_shear(shear)?;
        let half_width = half_height * aspect;
        let ortho = Mat4::orthographic_lh(-half_width, half_width, -half_height, half_height, near, far);
        Self::custom(ortho * view_shear(shear))
    }

    /// View-space shear in front of a reverse-Z perspective projection, stored as [`Custom`](Self::Custom).
    ///
    /// The shear is the same as [`Self::oblique_orthographic`]. `fov_y_degrees` is the vertical
    /// field of view. `near` must be greater than 0. Aspect, field of view, and clip distances are
    /// baked into the matrix and are not rebuilt on resize.
    pub fn oblique_perspective(
        fov_y_degrees: f32,
        aspect: f32,
        near: f32,
        far: f32,
        shear: Vec2,
    ) -> Result<Self, ProjectionError> {
        let fov_y = check_fov_degrees(fov_y_degrees)?;
        check_aspect(aspect)?;
        check_clip(near, far, false)?;
        check_shear(shear)?;
        let perspective = Mat4::perspective_lh(fov_y, aspect, near, far);
        Self::custom(perspective * view_shear(shear))
    }

    /// Stores `matrix` as a view-to-clip projection.
    ///
    /// Refuses NaN and infinity. A singular or oblique matrix is accepted; see
    /// [`CameraProjection::Custom`].
    pub fn custom(matrix: Mat4) -> Result<Self, ProjectionError> {
        if matrix.cols.iter().any(|column| !column_is_finite(*column)) {
            return Err(ProjectionError::NonFinite);
        }
        Ok(Self::Custom(matrix))
    }

    /// Sets the near/far clip distances on [`Orthographic`](Self::Orthographic) and
    /// [`Perspective`](Self::Perspective), in view-space units in front of the camera.
    ///
    /// [`Custom`](Self::Custom) is returned unchanged. Those distances live in the stored matrix.
    pub fn with_clip_range(mut self, near: f32, far: f32) -> Self {
        match &mut self {
            Self::Orthographic(p) => (p.near, p.far) = (near, far),
            Self::Perspective(p) => (p.near, p.far) = (near, far),
            Self::Custom(_) => {}
        }
        self
    }

    /// `true` only for the built-in [`Perspective`](Self::Perspective) variant.
    ///
    /// [`Custom`](Self::Custom) is not perspective and not orthographic. An arbitrary matrix does
    /// not have a reliable class, so this method does not guess. Picking, culling, sprite facing
    /// and extraction use the resolved view and projection matrices, not this flag.
    pub fn is_perspective(&self) -> bool {
        matches!(self, Self::Perspective(_))
    }
}

fn column_is_finite(column: Vec4) -> bool {
    column.x.is_finite() && column.y.is_finite() && column.z.is_finite() && column.w.is_finite()
}

fn view_shear(shear: Vec2) -> Mat4 {
    Mat4::from_cols(
        Vec4::new(1.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 1.0, 0.0, 0.0),
        Vec4::new(shear.x, shear.y, 1.0, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    )
}

pub(crate) fn check_finite(value: f32) -> Result<(), ProjectionError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(ProjectionError::NonFinite)
    }
}

pub(crate) fn check_vec3(value: Vec3) -> Result<(), ProjectionError> {
    if value.x.is_finite() && value.y.is_finite() && value.z.is_finite() {
        Ok(())
    } else {
        Err(ProjectionError::NonFinite)
    }
}

fn check_aspect(aspect: f32) -> Result<(), ProjectionError> {
    check_finite(aspect)?;
    if aspect > 0.0 {
        Ok(())
    } else {
        Err(ProjectionError::InvalidRange)
    }
}

fn check_window(height: f32, aspect: f32) -> Result<f32, ProjectionError> {
    check_finite(height)?;
    check_aspect(aspect)?;
    if height > 0.0 {
        Ok(height * 0.5)
    } else {
        Err(ProjectionError::InvalidRange)
    }
}

pub(crate) fn check_height(height: f32) -> Result<(), ProjectionError> {
    check_window(height, 1.0).map(|_| ())
}

pub(crate) fn check_fov_degrees(fov_y_degrees: f32) -> Result<f32, ProjectionError> {
    check_finite(fov_y_degrees)?;
    if fov_y_degrees > 0.0 && fov_y_degrees < 180.0 {
        Ok(fov_y_degrees.to_radians())
    } else {
        Err(ProjectionError::InvalidRange)
    }
}

/// `allow_zero_near` is the orthographic rule. Perspective requires `near > 0`.
fn check_clip(near: f32, far: f32, allow_zero_near: bool) -> Result<(), ProjectionError> {
    check_finite(near)?;
    check_finite(far)?;
    let near_ok = if allow_zero_near { near >= 0.0 } else { near > 0.0 };
    if near_ok && far > near {
        Ok(())
    } else {
        Err(ProjectionError::InvalidRange)
    }
}

fn check_shear(shear: Vec2) -> Result<(), ProjectionError> {
    check_finite(shear.x)?;
    check_finite(shear.y)
}

pub(crate) fn check_distance(distance: f32) -> Result<(), ProjectionError> {
    check_finite(distance)?;
    if distance >= 0.0 {
        Ok(())
    } else {
        Err(ProjectionError::InvalidRange)
    }
}

/// Why a projection constructor refused its arguments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectionError {
    /// A matrix element or constructor argument was NaN or infinite.
    NonFinite,
    /// A size, field of view, aspect, distance, or clip range cannot form this projection.
    ///
    /// Height and aspect must be greater than 0. Vertical field of view must be greater than 0
    /// and less than 180 degrees. `far` must be greater than `near`. Orthographic `near` may be 0.
    /// Perspective `near` must be greater than 0. A placed camera's distance back from its target
    /// must be at least 0.
    InvalidRange,
}

impl core::fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ProjectionError::NonFinite => write!(f, "a projection argument is NaN or infinity"),
            ProjectionError::InvalidRange => {
                write!(f, "a projection size, field of view, aspect, distance, or clip range is not usable")
            }
        }
    }
}

impl std::error::Error for ProjectionError {}

impl Projection for CameraProjection {
    fn matrix(&self, aspect: f32) -> Mat4 {
        match self {
            Self::Orthographic(p) => p.matrix(aspect),
            Self::Perspective(p) => p.matrix(aspect),
            Self::Custom(matrix) => *matrix,
        }
    }
}
