//! Named camera configurations for the test scene.
//!
//! Each one returns the same `Camera` and `Transform` a caller may edit afterwards. The placements
//! and the oblique matrix come from [`PlacedCamera`] and [`CameraProjection::oblique_orthographic`].
//! This file only supplies the scene's framing: where the camera sits, how large the view is, the
//! sky clear, and the clip range `1..4000`. Isometric is still an orthographic camera. `custom`
//! stores the public oblique matrix on [`CameraProjection::Custom`].

use engine::prelude::*;

pub const FOV_DEGREES: f32 = 50.0;
/// Vertical extent of the home orthographic view in world units. Fixed, so moving the camera in depth never
/// zooms an orthographic image (that is the point of the comparison with perspective).
pub const ORTHO_HEIGHT: f32 = 400.0;
pub const SKY: Color = Color::rgb(0.62, 0.74, 0.88);
/// The world point the framed configurations look at.
pub const FOCUS: Vec3 = Vec3::new(0.0, 60.0, 300.0);
/// Pitch of a true isometric camera: [`PlacedCamera::ISOMETRIC_PITCH`].
pub const ISOMETRIC_PITCH: f32 = PlacedCamera::ISOMETRIC_PITCH;

pub type CameraConfig = fn() -> (Camera, Transform);

/// Name and constructor of every configuration, in the order the example cycles through them.
/// `custom` is last. After it, cycling wraps to `perspective`.
pub const NAMED: [(&str, CameraConfig); 7] = [
    ("perspective", perspective),
    ("orthographic", orthographic),
    ("isometric", isometric),
    ("top-down", top_down),
    ("steep", steep),
    ("side", side),
    ("custom", custom),
];

pub fn by_name(name: &str) -> Option<usize> {
    NAMED.iter().position(|(n, _)| *n == name)
}

/// Index chosen by `--camera NAME` and `--ortho`, using the example's existing precedence:
/// an explicit name wins, `--ortho` selects orthographic, and neither selects index 0.
///
/// An unknown name is an error. It is not read as `custom`.
pub fn config_from_args(camera: Option<&str>, ortho: bool) -> Result<usize, String> {
    match (camera, ortho) {
        (Some(name), _) => by_name(name).ok_or_else(|| {
            let known: Vec<&str> = NAMED.iter().map(|(n, _)| *n).collect();
            format!("unknown camera `{name}`; known: {}", known.join(", "))
        }),
        (None, true) => Ok(by_name("orthographic").expect("listed")),
        (None, false) => Ok(0),
    }
}

fn finish(mut placed: PlacedCamera) -> (Camera, Transform) {
    placed.camera.projection = placed.camera.projection.with_clip_range(1.0, 4000.0);
    placed.camera = placed.camera.with_clear_color(SKY);
    (placed.camera, placed.transform)
}

fn home() -> (Vec3, f32, f32) {
    let rig = CameraRig::HOME;
    (rig.position, rig.yaw, rig.pitch)
}

pub fn home_transform() -> Transform {
    CameraRig::HOME.transform()
}

/// Perspective, slightly above the scene and pitched down. The default view.
pub fn perspective() -> (Camera, Transform) {
    let (eye, yaw, pitch) = home();
    finish(PlacedCamera::perspective(eye, yaw, pitch, FOV_DEGREES).expect("home perspective"))
}

/// The same placement as [`perspective`] with an orthographic projection.
pub fn orthographic() -> (Camera, Transform) {
    let (eye, yaw, pitch) = home();
    finish(PlacedCamera::orthographic(eye, yaw, pitch, ORTHO_HEIGHT).expect("home orthographic"))
}

/// Orthographic, yawed 45 degrees and pitched by the true isometric angle.
pub fn isometric() -> (Camera, Transform) {
    finish(PlacedCamera::isometric(FOCUS, 1500.0, 900.0).expect("isometric"))
}

/// Orthographic, looking straight down -Y with +Z at the top of the image.
pub fn top_down() -> (Camera, Transform) {
    finish(PlacedCamera::top_down(FOCUS, 1000.0, 1000.0).expect("top-down"))
}

/// Perspective, pitched by [`PlacedCamera::STEEP_PITCH`]: steeper than the default, short of straight down.
pub fn steep() -> (Camera, Transform) {
    finish(PlacedCamera::steep(FOCUS, 1000.0, FOV_DEGREES).expect("steep"))
}

/// Orthographic, level, looking along +Z: the side-scrolling placement.
pub fn side() -> (Camera, Transform) {
    finish(PlacedCamera::side(FOCUS, 1000.0, ORTHO_HEIGHT).expect("side"))
}

/// Aspect stored in [`custom`]. [`CameraProjection::Custom`] ignores later viewport aspect.
/// The example window (1280×720) and the configuration tests (320×180) are both 16:9.
pub const CUSTOM_ASPECT: f32 = 16.0 / 9.0;

/// View-space cabinet shear for [`custom`]: `x' = x + z * shear.x`, `y' = y + z * shear.y`.
///
/// The values move a mid-scene point by tens of pixels in the test viewport and leave the home
/// figures inside a frame of height [`ORTHO_HEIGHT`].
pub const OBLIQUE_SHEAR: Vec2 = Vec2::new(0.14, 0.04);

/// The same placement as [`orthographic`], with [`CameraProjection::oblique_orthographic`].
///
/// Height [`ORTHO_HEIGHT`], near 1, far 4000, and aspect [`CUSTOM_ASPECT`] are inside the matrix.
/// [`CameraProjection::with_clip_range`] does not edit it. The pose stays on the transform.
pub fn custom() -> (Camera, Transform) {
    let (eye, yaw, pitch) = home();
    let projection = CameraProjection::oblique_orthographic(ORTHO_HEIGHT, CUSTOM_ASPECT, 1.0, 4000.0, OBLIQUE_SHEAR)
        .expect("showcase oblique matrix is finite");
    finish(PlacedCamera::at(eye, yaw, pitch, projection).expect("home pose"))
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
            if name == "custom" {
                continue;
            }
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
        let (camera, transform) = custom();
        let matrix = camera.projection.matrix(CUSTOM_ASPECT);
        assert!(matches!(camera.projection, CameraProjection::Custom(_)));
        assert!(matrix.to_cols_array().iter().all(|v| v.is_finite()));
        let ordinary = CameraProjection::orthographic(ORTHO_HEIGHT).with_clip_range(1.0, 4000.0).matrix(CUSTOM_ASPECT);
        assert!(!matrix.approx_eq(&ordinary, 1e-5), "the oblique matrix is not the plain orthographic matrix");
        let perspective = CameraProjection::perspective(FOV_DEGREES).with_clip_range(1.0, 4000.0).matrix(CUSTOM_ASPECT);
        assert!(!matrix.approx_eq(&perspective, 1e-4));
        assert!(!camera.projection.is_perspective());
        assert!(
            custom().0.projection.matrix(1.0).approx_eq(&matrix, 0.0),
            "construction is deterministic and aspect-independent"
        );
        assert!(transform.position.approx_eq(home_transform().position, 1e-4));
        assert_eq!(config_from_args(Some("custom"), false).unwrap(), by_name("custom").unwrap());
    }

    #[test]
    fn cli_names_keep_their_indices_and_an_unknown_name_is_rejected() {
        for (i, (name, _)) in NAMED.iter().enumerate() {
            assert_eq!(config_from_args(Some(name), false).unwrap(), i, "{name}");
        }
        assert_eq!(config_from_args(None, true).unwrap(), by_name("orthographic").unwrap());
        assert_eq!(config_from_args(None, false).unwrap(), 0);
        assert_eq!(config_from_args(Some("orthographic"), true).unwrap(), by_name("orthographic").unwrap());
        let err = config_from_args(Some("nonsense"), false).unwrap_err();
        assert!(err.contains("unknown camera `nonsense`"), "{err}");
        assert_ne!(by_name("nonsense"), by_name("custom"));
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
