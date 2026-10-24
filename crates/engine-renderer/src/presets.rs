//! Constructors for the common camera placements.
//!
//! Each one returns a [`PlacedCamera`]: an ordinary [`Camera`] and an ordinary [`Transform`].
//! There is no isometric projection, no top-down mode, and no preset tag stored on the camera.
//! After construction, projection, pose, viewport, output, and [`CameraPresentation`] are edited
//! on those two values. A preset does not set gameplay orientation or a visual style.
//!
//! World axes are +Y up and the ground is XZ. That ground is the provisional engine convention.
//! The camera's local axes are +X right, +Y up, and +Z forward. Yaw is a rotation about +Y, then
//! pitch about the camera's X: `Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch)`.
//! Positive pitch looks down toward −Y. `right.cross(up)` equals `forward`.
//!
//! [`CameraProjection::Perspective`] and [`CameraProjection::Orthographic`] take their aspect
//! ratio from the viewport when the view is built. Near and far start at 0.1 and 1000, the same
//! defaults as those projections, and [`CameraProjection::with_clip_range`] changes them.
//! [`CameraProjection::Custom`], including an oblique matrix, keeps the aspect and clip range
//! that were baked into it.

use engine_math::{Quat, Transform, Vec3};

use crate::camera::{Camera, CameraPresentation};
use crate::projection::{
    check_distance, check_finite, check_fov_degrees, check_height, check_vec3, CameraProjection, ProjectionError,
};

/// A [`Camera`] and the [`Transform`] that places it.
///
/// Spawn both on one entity. Changing either field afterwards is the supported way to edit the
/// view. The value is not a component and not a camera mode. An editor can read the projection
/// parameters and the transform. It cannot recover which constructor was used, because that name
/// is not stored. Straight down has no separate yaw: pitch of π/2 and yaw 0 is the top-down
/// placement, and a later decomposition of that orientation can fold yaw into the gimbal lock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacedCamera {
    pub camera: Camera,
    pub transform: Transform,
}

impl PlacedCamera {
    /// Yaw of [`Self::isometric`], 45 degrees about +Y.
    pub const ISOMETRIC_YAW: f32 = core::f32::consts::FRAC_PI_4;

    /// Pitch of [`Self::isometric`]: `atan(1 / sqrt(2))`, rounded to the value the demonstrated
    /// isometric camera uses (about 35.264 degrees).
    pub const ISOMETRIC_PITCH: f32 = 0.615_479_7;

    /// Pitch of [`Self::steep`], 1.05 radians (about 60.16 degrees). This is the demonstrated
    /// steep view. It is not exactly 60 degrees, and it is not a projection of its own.
    pub const STEEP_PITCH: f32 = 1.05;

    /// Places `projection` at `eye` with `yaw` then `pitch`.
    ///
    /// The projection is stored as given. A [`CameraProjection::Custom`] matrix is not rebuilt
    /// from `eye`. Non-finite `eye`, `yaw`, or `pitch` is [`ProjectionError::NonFinite`].
    pub fn at(eye: Vec3, yaw: f32, pitch: f32, projection: CameraProjection) -> Result<Self, ProjectionError> {
        check_vec3(eye)?;
        check_finite(yaw)?;
        check_finite(pitch)?;
        Ok(Self { camera: Camera::new(projection), transform: pose(eye, yaw, pitch) })
    }

    /// Perspective projection at `eye`. `fov_y_degrees` is the vertical field of view.
    ///
    /// Aspect follows the viewport. The field of view must be greater than 0 and less than 180.
    pub fn perspective(eye: Vec3, yaw: f32, pitch: f32, fov_y_degrees: f32) -> Result<Self, ProjectionError> {
        check_fov_degrees(fov_y_degrees)?;
        Self::at(eye, yaw, pitch, CameraProjection::perspective(fov_y_degrees))
    }

    /// Orthographic projection at `eye`. `height` is the vertical world size.
    ///
    /// Width follows the viewport aspect. `height` must be greater than 0.
    pub fn orthographic(eye: Vec3, yaw: f32, pitch: f32, height: f32) -> Result<Self, ProjectionError> {
        check_height(height)?;
        Self::at(eye, yaw, pitch, CameraProjection::orthographic(height))
    }

    /// Orthographic camera, yaw [`Self::ISOMETRIC_YAW`] and pitch [`Self::ISOMETRIC_PITCH`],
    /// sitting `distance` back from `target` along its forward axis.
    ///
    /// `distance` of 0 puts the camera on `target` and keeps the orientation. A negative distance
    /// is refused.
    pub fn isometric(target: Vec3, distance: f32, height: f32) -> Result<Self, ProjectionError> {
        let eye = eye_from_target(target, Self::ISOMETRIC_YAW, Self::ISOMETRIC_PITCH, distance)?;
        Self::orthographic(eye, Self::ISOMETRIC_YAW, Self::ISOMETRIC_PITCH, height)
    }

    /// Orthographic camera above `target`, looking along −Y, with +Z toward the top of the image.
    ///
    /// Pitch is π/2 and yaw is 0. `distance` is the height above `target`. The projection, the
    /// distance, and the yaw can be edited afterwards; none of those edits is a mode switch.
    pub fn top_down(target: Vec3, distance: f32, height: f32) -> Result<Self, ProjectionError> {
        let eye = eye_from_target(target, 0.0, core::f32::consts::FRAC_PI_2, distance)?;
        Self::orthographic(eye, 0.0, core::f32::consts::FRAC_PI_2, height)
    }

    /// Perspective camera pitched by [`Self::STEEP_PITCH`], yaw 0, `distance` back from `target`.
    pub fn steep(target: Vec3, distance: f32, fov_y_degrees: f32) -> Result<Self, ProjectionError> {
        let eye = eye_from_target(target, 0.0, Self::STEEP_PITCH, distance)?;
        Self::perspective(eye, 0.0, Self::STEEP_PITCH, fov_y_degrees)
    }

    /// Orthographic camera, level, looking along +Z, `distance` back from `target`.
    ///
    /// This is the side-scrolling placement. It does not set a gameplay movement plane.
    pub fn side(target: Vec3, distance: f32, height: f32) -> Result<Self, ProjectionError> {
        let eye = eye_from_target(target, 0.0, 0.0, distance)?;
        Self::orthographic(eye, 0.0, 0.0, height)
    }

    /// Sets the clip range on a perspective or orthographic projection.
    ///
    /// A custom projection, including an oblique matrix, is unchanged. Near and far already live
    /// in that matrix.
    pub fn with_clip_range(mut self, near: f32, far: f32) -> Self {
        self.camera.projection = self.camera.projection.with_clip_range(near, far);
        self
    }

    /// Sets [`CameraPresentation`] without changing the projection or the transform.
    pub fn with_presentation(mut self, presentation: CameraPresentation) -> Self {
        self.camera = self.camera.with_presentation(presentation);
        self
    }
}

fn pose(eye: Vec3, yaw: f32, pitch: f32) -> Transform {
    Transform::from_position(eye).with_rotation(Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch))
}

fn eye_from_target(target: Vec3, yaw: f32, pitch: f32, distance: f32) -> Result<Vec3, ProjectionError> {
    check_vec3(target)?;
    check_finite(yaw)?;
    check_finite(pitch)?;
    check_distance(distance)?;
    Ok(target - pose(Vec3::ZERO, yaw, pitch).forward() * distance)
}

#[cfg(test)]
mod tests {
    use engine_math::{Mat4, Quat, Vec2, Vec3, Vec4};
    use engine_world::GlobalTransform;

    use super::*;
    use crate::camera::{CameraOutput, CameraView, ViewportRect};
    use crate::frustum::Frustum;
    use crate::projection::{CameraProjection, Orthographic, Perspective, Projection};
    use crate::render_target::RenderTargetStore;

    const ASPECT: f32 = 16.0 / 9.0;

    fn view_of(placed: &PlacedCamera, aspect: f32) -> CameraView {
        let height = 180.0;
        CameraView::new(
            &placed.camera,
            &GlobalTransform { matrix: placed.transform.to_matrix() },
            Vec2::new(height * aspect, height),
        )
    }

    fn ndc_z(proj: Mat4, view_z: f32) -> f32 {
        let clip = proj * Vec4::new(0.0, 0.0, view_z, 1.0);
        clip.z / clip.w
    }

    #[test]
    fn perspective_and_orthographic_match_the_builtin_matrices_and_reverse_z() {
        let eye = Vec3::new(1.0, 2.0, -3.0);
        let perspective = PlacedCamera::perspective(eye, 0.2, 0.4, 50.0).unwrap().with_clip_range(1.0, 40.0);
        let orthographic = PlacedCamera::orthographic(eye, 0.2, 0.4, 12.0).unwrap().with_clip_range(0.0, 40.0);
        assert!(perspective.camera.projection.is_perspective());
        assert!(!orthographic.camera.projection.is_perspective());
        assert!(perspective.transform.position.approx_eq(eye, 0.0));
        assert!(perspective.transform.forward().y < 0.0, "positive pitch looks down");

        let persp = perspective.camera.projection.matrix(ASPECT);
        let expected = Perspective { fov_y: 50.0_f32.to_radians(), near: 1.0, far: 40.0 }.matrix(ASPECT);
        assert!(persp.approx_eq(&expected, 1.0e-6));
        assert!((ndc_z(persp, 1.0) - 1.0).abs() < 1.0e-4);
        assert!(ndc_z(persp, 40.0).abs() < 1.0e-4);
        assert!(ndc_z(persp, 8.0) > ndc_z(persp, 20.0), "nearer view z stores the larger depth");

        let ortho = orthographic.camera.projection.matrix(ASPECT);
        let expected = Orthographic { height: 12.0, near: 0.0, far: 40.0 }.matrix(ASPECT);
        assert!(ortho.approx_eq(&expected, 1.0e-6));
        assert!((ndc_z(ortho, 0.0) - 1.0).abs() < 1.0e-4);
        assert!(ndc_z(ortho, 40.0).abs() < 1.0e-4);
        let mid = ndc_z(ortho, 20.0);
        assert!((mid - 0.5).abs() < 1.0e-4, "orthographic midpoint is linear, got {mid}");
    }

    #[test]
    fn isometric_top_down_steep_and_side_use_the_demonstrated_orientations() {
        let iso = PlacedCamera::isometric(Vec3::new(0.0, 1.0, 2.0), 15.0, 9.0).unwrap();
        assert!(
            matches!(iso.camera.projection, CameraProjection::Orthographic(Orthographic { height, .. }) if height == 9.0)
        );
        let forward = iso.transform.forward();
        assert!(forward.x > 0.4 && forward.z > 0.4 && forward.y < -0.5, "yawed 45 and pitched down: {forward:?}");
        let to_target = (Vec3::new(0.0, 1.0, 2.0) - iso.transform.position).normalized();
        assert!(forward.approx_eq(to_target, 1.0e-5));
        assert!((PlacedCamera::ISOMETRIC_PITCH - (1.0 / 2.0_f32.sqrt()).atan()).abs() < 1.0e-7);

        let down = PlacedCamera::top_down(Vec3::ZERO, 10.0, 8.0).unwrap();
        assert!(down.transform.forward().approx_eq(-Vec3::Y, 1.0e-5), "{:?}", down.transform.forward());
        assert!(down.transform.up().z > 0.99, "+Z is the top of the image, up {:?}", down.transform.up());
        assert!(down.transform.position.approx_eq(Vec3::Y * 10.0, 1.0e-4));

        let steep = PlacedCamera::steep(Vec3::ZERO, 12.0, 50.0).unwrap();
        assert!(steep.camera.projection.is_perspective());
        assert!(steep.transform.forward().y < -0.8 && steep.transform.forward().z > 0.0);
        assert!((steep.transform.position.length() - 12.0).abs() < 1.0e-4);

        let side = PlacedCamera::side(Vec3::new(2.0, 3.0, 4.0), 6.0, 10.0).unwrap();
        assert!(side.transform.forward().approx_eq(Vec3::Z, 1.0e-5));
        assert!(side.transform.up().approx_eq(Vec3::Y, 1.0e-5));
        assert!(side.transform.right().cross(side.transform.up()).approx_eq(side.transform.forward(), 1.0e-5));
        assert!(side.transform.position.approx_eq(Vec3::new(2.0, 3.0, -2.0), 1.0e-4));
    }

    #[test]
    fn a_preset_stays_editable_and_movement_does_not_rewrite_the_projection() {
        let mut placed = PlacedCamera::perspective(Vec3::new(0.0, 2.0, -6.0), 0.0, 0.3, 50.0).unwrap();
        let projection = placed.camera.projection;
        assert!(matches!(
            projection,
            CameraProjection::Perspective(Perspective { fov_y, near, far })
                if (fov_y - 50.0_f32.to_radians()).abs() < 1.0e-6 && near == 0.1 && far == 1000.0
        ));
        placed.transform.position += Vec3::X;
        placed.transform.rotation = Quat::from_rotation_y(0.5) * placed.transform.rotation;
        placed.camera.viewport = ViewportRect::new(0.0, 0.0, 0.5, 1.0);
        let id = RenderTargetStore::new().create(8, 8).unwrap();
        placed.camera = placed.camera.with_output(CameraOutput::Target(id));
        placed = placed.with_presentation(CameraPresentation::Depth).with_clip_range(0.2, 80.0);
        assert_eq!(placed.camera.presentation, CameraPresentation::Depth);
        assert!(matches!(placed.camera.output, CameraOutput::Target(_)));
        assert!(
            matches!(placed.camera.projection, CameraProjection::Perspective(Perspective { near, far, .. }) if near == 0.2 && far == 80.0)
        );
        assert!(!matches!(placed.camera.projection, CameraProjection::Custom(_)));
        let moved = view_of(&placed, ASPECT);
        let parked = view_of(&PlacedCamera { transform: Transform::from_xyz(0.0, 2.0, -6.0), ..placed }, ASPECT);
        assert!(moved.proj.approx_eq(&parked.proj, 0.0), "moving the transform does not change the projection");
        assert!(!moved.view.approx_eq(&parked.view, 1.0e-4));
    }

    #[test]
    fn oblique_shear_keeps_reverse_z_and_a_zero_shear_matches_the_helper() {
        let plain = Orthographic { height: 10.0, near: 1.0, far: 40.0 }.matrix(ASPECT);
        let zero = CameraProjection::oblique_orthographic(10.0, ASPECT, 1.0, 40.0, Vec2::ZERO).unwrap();
        assert!(zero.matrix(1.0).approx_eq(&plain, 1.0e-5));
        assert!(zero.matrix(2.0).approx_eq(&plain, 1.0e-5), "a custom matrix ignores a later aspect");

        let sheared = CameraProjection::oblique_orthographic(10.0, ASPECT, 1.0, 40.0, Vec2::new(0.14, 0.04)).unwrap();
        assert!(matches!(sheared, CameraProjection::Custom(_)));
        assert!(!sheared.is_perspective());
        let matrix = sheared.matrix(ASPECT);
        assert!((ndc_z(matrix, 1.0) - 1.0).abs() < 1.0e-4);
        assert!(ndc_z(matrix, 40.0).abs() < 1.0e-4);
        assert!(ndc_z(matrix, 4.0) > ndc_z(matrix, 20.0));
        let clip = matrix * Vec4::new(0.0, 0.0, 8.0, 1.0);
        let unsheared = plain * Vec4::new(0.0, 0.0, 8.0, 1.0);
        assert!((clip.z - unsheared.z).abs() < 1.0e-4, "shear does not change view z's depth");
        assert!((clip.x - unsheared.x).abs() > 1.0e-3, "shear moves view x");
        assert!(sheared.with_clip_range(0.2, 10.0).matrix(1.0).approx_eq(&matrix, 0.0));

        let perspective = Perspective { fov_y: 50.0_f32.to_radians(), near: 0.5, far: 30.0 }.matrix(1.0);
        let oblique = CameraProjection::oblique_perspective(50.0, 1.0, 0.5, 30.0, Vec2::ZERO).unwrap();
        assert!(oblique.matrix(4.0).approx_eq(&perspective, 1.0e-5));
        let pulled =
            CameraProjection::oblique_perspective(50.0, 1.0, 0.5, 30.0, Vec2::new(0.2, 0.0)).unwrap().matrix(1.0);
        assert!(ndc_z(pulled, 2.0) > ndc_z(pulled, 10.0));
    }

    #[test]
    fn oblique_and_isometric_frusta_and_rays_agree_with_the_projection() {
        let placed = PlacedCamera::isometric(Vec3::ZERO, 20.0, 16.0).unwrap().with_clip_range(0.5, 80.0);
        let view = view_of(&placed, 1.0);
        let frustum = Frustum::from_view_proj(&view.view_proj);
        let ahead = placed.transform.position + placed.transform.forward() * 10.0;
        let behind = placed.transform.position - placed.transform.forward() * 5.0;
        assert!(frustum.intersects_sphere(ahead, 0.25));
        assert!(!frustum.intersects_sphere(behind, 0.25));

        let screen = view.project(ahead).unwrap();
        let (origin, direction) = view.ray_from_pixels(screen.pixels).unwrap();
        let closest = origin + direction * direction.dot(ahead - origin);
        assert!(closest.approx_eq(ahead, 0.05), "picked {closest:?} vs {ahead:?}");

        let oblique = CameraProjection::oblique_orthographic(16.0, 1.0, 0.5, 80.0, Vec2::new(0.25, 0.1)).unwrap();
        let placed = PlacedCamera::at(placed.transform.position, 0.4, 0.2, oblique).unwrap();
        let view = view_of(&placed, 1.0);
        let point = placed.transform.position + placed.transform.forward() * 8.0 + placed.transform.right();
        let screen = view.project(point).unwrap();
        assert!(screen.is_visible());
        let (origin, direction) = view.ray_from_pixels(screen.pixels).unwrap();
        let closest = origin + direction * direction.dot(point - origin);
        assert!(closest.approx_eq(point, 0.08), "oblique pick {closest:?} vs {point:?}");
        let frustum = Frustum::from_view_proj(&view.view_proj);
        assert!(frustum.intersects_sphere(point, 0.2));
        assert!(!frustum.intersects_sphere(placed.transform.position - placed.transform.forward() * 4.0, 0.2));
    }

    #[test]
    fn invalid_preset_and_oblique_arguments_are_errors() {
        assert_eq!(PlacedCamera::perspective(Vec3::ZERO, 0.0, 0.0, f32::NAN).unwrap_err(), ProjectionError::NonFinite);
        assert_eq!(
            PlacedCamera::perspective(Vec3::ZERO, f32::INFINITY, 0.0, 50.0).unwrap_err(),
            ProjectionError::NonFinite
        );
        assert_eq!(
            PlacedCamera::orthographic(Vec3::new(f32::NAN, 0.0, 0.0), 0.0, 0.0, 10.0).unwrap_err(),
            ProjectionError::NonFinite
        );
        assert_eq!(PlacedCamera::perspective(Vec3::ZERO, 0.0, 0.0, 0.0).unwrap_err(), ProjectionError::InvalidRange);
        assert_eq!(PlacedCamera::perspective(Vec3::ZERO, 0.0, 0.0, 180.0).unwrap_err(), ProjectionError::InvalidRange);
        assert_eq!(PlacedCamera::orthographic(Vec3::ZERO, 0.0, 0.0, 0.0).unwrap_err(), ProjectionError::InvalidRange);
        assert_eq!(PlacedCamera::orthographic(Vec3::ZERO, 0.0, 0.0, -2.0).unwrap_err(), ProjectionError::InvalidRange);
        assert_eq!(PlacedCamera::isometric(Vec3::ZERO, -1.0, 10.0).unwrap_err(), ProjectionError::InvalidRange);
        assert_eq!(
            PlacedCamera::side(Vec3::new(0.0, f32::NAN, 0.0), 1.0, 10.0).unwrap_err(),
            ProjectionError::NonFinite
        );
        assert!(PlacedCamera::top_down(Vec3::ZERO, 0.0, 4.0).is_ok(), "a distance of 0 sits on the target");

        let shear = Vec2::new(0.1, 0.0);
        assert_eq!(
            CameraProjection::oblique_orthographic(0.0, 1.0, 0.1, 10.0, shear).unwrap_err(),
            ProjectionError::InvalidRange
        );
        assert_eq!(
            CameraProjection::oblique_orthographic(10.0, 0.0, 0.1, 10.0, shear).unwrap_err(),
            ProjectionError::InvalidRange
        );
        assert_eq!(
            CameraProjection::oblique_orthographic(10.0, -1.0, 0.1, 10.0, shear).unwrap_err(),
            ProjectionError::InvalidRange
        );
        assert_eq!(
            CameraProjection::oblique_orthographic(10.0, 1.0, -0.1, 10.0, shear).unwrap_err(),
            ProjectionError::InvalidRange
        );
        assert_eq!(
            CameraProjection::oblique_orthographic(10.0, 1.0, 5.0, 5.0, shear).unwrap_err(),
            ProjectionError::InvalidRange
        );
        assert!(CameraProjection::oblique_orthographic(10.0, 1.0, 0.0, 10.0, shear).is_ok());
        assert_eq!(
            CameraProjection::oblique_perspective(50.0, 1.0, 0.0, 10.0, shear).unwrap_err(),
            ProjectionError::InvalidRange
        );
        assert_eq!(
            CameraProjection::oblique_perspective(0.0, 1.0, 0.1, 10.0, shear).unwrap_err(),
            ProjectionError::InvalidRange
        );
        assert_eq!(
            CameraProjection::oblique_perspective(50.0, 1.0, 0.1, 10.0, Vec2::new(f32::NAN, 0.0)).unwrap_err(),
            ProjectionError::NonFinite
        );
        assert_eq!(
            CameraProjection::custom(Mat4::from_cols(
                Vec4::ZERO,
                Vec4::ZERO,
                Vec4::ZERO,
                Vec4::new(f32::NAN, 0.0, 0.0, 1.0)
            )),
            Err(ProjectionError::NonFinite)
        );
    }

    #[test]
    fn top_down_and_steep_presets_keep_billboard_upright_and_fixed_distinct() {
        use engine_assets::{AssetManager, SpriteAsset, Texture};
        use engine_world::{propagate_transforms, World};

        use crate::components::{Sprite, SpriteOrientation};
        use crate::extract_scene;

        let mut assets = AssetManager::in_memory();
        let texture = assets.insert("quad", Texture::solid(1, 1, [255, 255, 255, 255])).unwrap();
        assets.insert("sheet", SpriteAsset::whole_texture(texture, 1, 1).with_pixels_per_unit(1.0)).unwrap();
        let sprite = |orientation: SpriteOrientation| {
            Sprite::new("sheet")
                .with_size(Vec2::new(2.0, 4.0))
                .with_anchor(Vec2::new(0.5, 0.0))
                .with_orientation(orientation)
                .cutout()
                .unlit()
        };
        let height = |placed: PlacedCamera, orientation: SpriteOrientation, rotation: Quat| {
            let mut world = World::new();
            world.spawn((placed.camera, placed.transform));
            world.spawn((Transform::from_rotation(rotation), sprite(orientation)));
            propagate_transforms(&mut world);
            let scene = extract_scene(&world, &assets, Vec2::splat(128.0)).unwrap();
            let model = scene.sole_pass().cutout_sprites.instances[0].model;
            let view = &scene.sole_pass().view;
            let bottom = view.project(model.transform_point3(Vec3::new(0.5, 0.0, 0.0))).unwrap();
            let top = view.project(model.transform_point3(Vec3::new(0.5, 1.0, 0.0))).unwrap();
            (bottom.pixels.y - top.pixels.y).abs()
        };
        let down = PlacedCamera::top_down(Vec3::ZERO, 12.0, 16.0).unwrap();
        let upright = height(down, SpriteOrientation::Upright, Quat::IDENTITY);
        let billboard = height(down, SpriteOrientation::Billboard, Quat::IDENTITY);
        let ground = height(down, SpriteOrientation::Fixed, Quat::from_rotation_x(core::f32::consts::FRAC_PI_2));
        assert!(upright < 0.5, "top-down upright is edge-on: {upright}");
        assert!(billboard > 8.0, "top-down billboard faces the camera: {billboard}");
        assert!(ground > 8.0, "a fixed quad rolled onto the ground stays visible: {ground}");

        let steep = PlacedCamera::steep(Vec3::ZERO, 16.0, 50.0).unwrap();
        let ratio = height(steep, SpriteOrientation::Upright, Quat::IDENTITY)
            / height(steep, SpriteOrientation::Billboard, Quat::IDENTITY);
        assert!((0.35..0.7).contains(&ratio), "steep upright is foreshortened, not a second renderer: {ratio}");
    }
}
