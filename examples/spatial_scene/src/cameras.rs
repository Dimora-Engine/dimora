//! Named camera configurations for the test scene.
//!
//! Each one is a plain function returning the same `Camera` component and a `Transform` that the caller may
//! edit afterwards. They live in the example on purpose: the engine does not have camera presets yet, and
//! their signatures and defaults are a deferred decision (ADR-0004, D-25). Isometric is an orthographic
//! camera that is yawed and pitched, not a projection of its own.

use engine::prelude::*;

pub const FOV_DEGREES: f32 = 50.0;
/// Vertical extent of the home orthographic view in world units. Fixed, so moving the camera in depth never
/// zooms an orthographic image (that is the point of the comparison with perspective).
pub const ORTHO_HEIGHT: f32 = 400.0;
pub const SKY: Color = Color::rgb(0.62, 0.74, 0.88);
/// The world point the framed configurations look at.
pub const FOCUS: Vec3 = Vec3::new(0.0, 60.0, 300.0);
/// Pitch of a true isometric camera: `atan(1 / sqrt(2))`.
pub const ISOMETRIC_PITCH: f32 = 0.615_479_7;

pub type CameraConfig = fn() -> (Camera, Transform);

/// Name and constructor of every configuration, in the order the example cycles through them.
pub const NAMED: [(&str, CameraConfig); 6] = [
    ("perspective", perspective),
    ("orthographic", orthographic),
    ("isometric", isometric),
    ("top-down", top_down),
    ("steep", steep),
    ("side", side),
];

pub fn by_name(name: &str) -> Option<usize> {
    NAMED.iter().position(|(n, _)| *n == name)
}

fn camera(projection: CameraProjection) -> Camera {
    Camera::new(projection.with_clip_range(1.0, 4000.0)).with_clear_color(SKY)
}

/// Camera `distance` back from [`FOCUS`], turned by `yaw` about +Y and then `pitch` about the camera's X.
fn framed(yaw: f32, pitch: f32, distance: f32) -> Transform {
    let rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch);
    let forward = Transform::from_position(Vec3::ZERO).with_rotation(rotation).forward();
    Transform::from_position(FOCUS - forward * distance).with_rotation(rotation)
}

pub fn home_transform() -> Transform {
    CameraRig::HOME.transform()
}

/// Perspective, slightly above the scene and pitched down. The default view.
pub fn perspective() -> (Camera, Transform) {
    (camera(CameraProjection::perspective(FOV_DEGREES)), home_transform())
}

/// The same placement as [`perspective`] with an orthographic projection.
pub fn orthographic() -> (Camera, Transform) {
    (camera(CameraProjection::orthographic(ORTHO_HEIGHT)), home_transform())
}

/// Orthographic, yawed 45 degrees and pitched by the true isometric angle.
pub fn isometric() -> (Camera, Transform) {
    (camera(CameraProjection::orthographic(900.0)), framed(core::f32::consts::FRAC_PI_4, ISOMETRIC_PITCH, 1500.0))
}

/// Orthographic, looking straight down -Y with +Z at the top of the image.
pub fn top_down() -> (Camera, Transform) {
    (camera(CameraProjection::orthographic(1000.0)), framed(0.0, core::f32::consts::FRAC_PI_2, 1000.0))
}

/// Perspective, pitched 60 degrees: steeper than the default, short of straight down.
pub fn steep() -> (Camera, Transform) {
    (camera(CameraProjection::perspective(FOV_DEGREES)), framed(0.0, 1.05, 1000.0))
}

/// Orthographic, level, looking along +Z: the side-scrolling placement.
pub fn side() -> (Camera, Transform) {
    (camera(CameraProjection::orthographic(ORTHO_HEIGHT)), framed(0.0, 0.0, 1000.0))
}

/// Free-fly pose used by the example: a position and two angles. Every configuration above is expressible as
/// one, which lets the controls start from any of them.
#[derive(Clone, Copy, Debug)]
pub struct CameraRig {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
}

impl CameraRig {
    pub const HOME: CameraRig = CameraRig { position: Vec3::new(0.0, 215.0, -420.0), yaw: 0.0, pitch: 0.3 };

    pub fn rotation(&self) -> Quat {
        Quat::from_rotation_y(self.yaw) * Quat::from_rotation_x(self.pitch)
    }

    pub fn transform(&self) -> Transform {
        Transform::from_position(self.position).with_rotation(self.rotation())
    }

    /// The rig whose pose equals `transform` (exact for transforms made of one yaw and one pitch, which is
    /// what every configuration here is). Straight down has no defined yaw, so it reads as 0.
    pub fn from_transform(transform: &Transform) -> Self {
        let f = transform.forward();
        let pitch = (-f.y).atan2(f.x.hypot(f.z));
        let yaw = if f.x.abs() + f.z.abs() < 1e-5 { 0.0 } else { f.x.atan2(f.z) };
        Self { position: transform.position, yaw, pitch }
    }

    /// Unit vectors on the ground plane: where the camera is heading and its right.
    pub fn heading(&self) -> (Vec3, Vec3) {
        let (s, c) = self.yaw.sin_cos();
        (Vec3::new(s, 0.0, c), Vec3::new(c, 0.0, -s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_configuration_is_a_camera_and_a_transform_that_a_rig_reproduces() {
        for (name, make) in NAMED {
            let (_, t) = make();
            let again = CameraRig::from_transform(&t).transform();
            assert!(again.forward().approx_eq(t.forward(), 1e-4), "{name}: forward");
            assert!(again.up().approx_eq(t.up(), 1e-4), "{name}: up");
            assert!(again.position.approx_eq(t.position, 1e-3), "{name}: position");
        }
    }

    #[test]
    fn the_framed_configurations_look_at_the_focus_point() {
        for name in ["isometric", "top-down", "steep", "side"] {
            let (_, t) = NAMED[by_name(name).unwrap()].1();
            let to_focus = (FOCUS - t.position).normalized();
            assert!(t.forward().approx_eq(to_focus, 1e-4), "{name}: {:?} vs {:?}", t.forward(), to_focus);
        }
    }

    #[test]
    fn existing_camera_presets_remain_compatible() {
        for (name, make) in NAMED {
            let (mut camera, mut transform) = make();
            let before = camera.projection;
            assert!(!matches!(before, CameraProjection::Custom(_)), "{name} is still a built-in projection");
            camera.viewport = ViewportRect::new(0.1, 0.2, 0.4, 0.5);
            camera.clear_color = Color::BLACK;
            transform.position += Vec3::X;
            assert_eq!(camera.projection, before, "{name}: editing the camera does not rewrite the projection");
            let matrix = before.matrix(16.0 / 9.0);
            camera.projection = CameraProjection::custom(matrix).unwrap();
            assert!(camera.projection.matrix(1.0).approx_eq(&matrix, 0.0), "{name}");
            assert!(!camera.projection.is_perspective());
        }
    }

    #[test]
    fn isometric_is_an_orthographic_camera_and_not_a_projection_of_its_own() {
        let (camera, t) = isometric();
        assert!(!camera.projection.is_perspective());
        assert!(t.forward().y < -0.5 && t.forward().x > 0.0 && t.forward().z > 0.0, "yawed and pitched");
    }

    #[test]
    fn names_are_unique_and_found() {
        for (i, (name, _)) in NAMED.iter().enumerate() {
            assert_eq!(by_name(name), Some(i));
        }
        assert_eq!(by_name("nonsense"), None);
    }
}
