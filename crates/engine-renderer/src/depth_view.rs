//! Linearized view-depth classification for one stored depth-buffer value.
//!
//! The GPU presentation pass uses the same rules. This module does not read the world and does
//! not draw. A stored value is the reverse-Z depth the renderer wrote: 1 at the near plane, 0 at
//! the far plane, greater-than test, clear 0. A fragment at exactly 0 does not pass that test, so
//! a stored 0 is unwritten, not the far plane.
//!
//! Classification uses the inverse of the camera's projection at one NDC sample. It does not
//! assume every matrix is [`Mat4::perspective_lh`](engine_math::Mat4::perspective_lh). A singular
//! matrix, a non-finite value, or a matrix whose NDC 1 is not in front of NDC 0 along view +Z is
//! [`StoredDepthClass::Unsupported`]. Those matrices are not reported as a flipped gradient.

use engine_math::{Mat4, Vec4};

/// Gray is not used for unwritten pixels. This purple stays distinct from a far fragment, which
/// is a dark gray. Linear RGBA, matching an `Rgba8Unorm` target before encoding.
pub const DEPTH_VIEW_EMPTY: [f32; 4] = [0.35, 0.02, 0.45, 1.0];

/// Used when the projection cannot be turned into view depth. Distinct from [`DEPTH_VIEW_EMPTY`]
/// and from any grayscale nearness.
pub const DEPTH_VIEW_UNSUPPORTED: [f32; 4] = [0.62, 0.22, 0.08, 1.0];

/// Minimum `z_far - z_near` accepted as a reverse-Z view range.
///
/// NDC 1 is the near sample and NDC 0 is the far sample. A span at or below this, including a
/// negative span from a 0-near matrix, is not linearized. The GPU uniform carries this value.
pub const DEPTH_VIEW_MIN_SPAN: f32 = 1.0e-5;

/// What one stored depth value means for the diagnostic image.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StoredDepthClass {
    /// The projection is usable and this sample was not written. Stored depth is `<= 0`.
    Empty,
    /// The value or the projection cannot be linearized. Not a darker shade of a valid range.
    Unsupported,
    /// View-space Z along +Z, and nearness in `0..=1` (1 at the near plane, 0 at the far plane).
    Linear { view_depth: f32, nearness: f32 },
}

/// Classifies `stored` depth at one NDC position.
///
/// `proj` is the camera's view-to-clip matrix. `ndc_x` and `ndc_y` are the sample in NDC, +Y up.
/// `stored` is the depth-buffer value at that pixel.
///
/// Unwritten samples are [`StoredDepthClass::Empty`] only after the projection is accepted. A
/// singular matrix or a span that is not reverse-Z view depth is [`StoredDepthClass::Unsupported`]
/// for every stored value, including the clear value, so it is not drawn as an empty scene.
pub fn classify_stored_depth(proj: Mat4, ndc_x: f32, ndc_y: f32, stored: f32) -> StoredDepthClass {
    if !stored.is_finite() || !ndc_x.is_finite() || !ndc_y.is_finite() {
        return StoredDepthClass::Unsupported;
    }
    let Some(inv) = proj.inverse() else {
        return StoredDepthClass::Unsupported;
    };
    let Some(z_near) = unproject_view_z(inv, ndc_x, ndc_y, 1.0) else {
        return StoredDepthClass::Unsupported;
    };
    let Some(z_far) = unproject_view_z(inv, ndc_x, ndc_y, 0.0) else {
        return StoredDepthClass::Unsupported;
    };
    let span = z_far - z_near;
    if !span.is_finite() || span <= DEPTH_VIEW_MIN_SPAN {
        return StoredDepthClass::Unsupported;
    }
    if stored <= 0.0 {
        return StoredDepthClass::Empty;
    }
    let Some(view_depth) = unproject_view_z(inv, ndc_x, ndc_y, stored) else {
        return StoredDepthClass::Unsupported;
    };
    let nearness = ((z_far - view_depth) / span).clamp(0.0, 1.0);
    if !nearness.is_finite() {
        return StoredDepthClass::Unsupported;
    }
    StoredDepthClass::Linear { view_depth, nearness }
}

fn unproject_view_z(inv: Mat4, ndc_x: f32, ndc_y: f32, ndc_z: f32) -> Option<f32> {
    let h = inv * Vec4::new(ndc_x, ndc_y, ndc_z, 1.0);
    if !h.x.is_finite() || !h.y.is_finite() || !h.z.is_finite() || !h.w.is_finite() || h.w.abs() <= 1.0e-8 {
        return None;
    }
    let z = h.z / h.w;
    z.is_finite().then_some(z)
}

/// Nearness of a view-space Z once the projection has already been accepted.
pub fn linear_nearness(view_depth: f32, z_near: f32, z_far: f32) -> Option<f32> {
    let span = z_far - z_near;
    if !view_depth.is_finite() || !span.is_finite() || span <= DEPTH_VIEW_MIN_SPAN {
        return None;
    }
    Some(((z_far - view_depth) / span).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use engine_assets::AssetManager;
    use engine_math::{Mat4, Transform, Vec2, Vec3, Vec4};
    use engine_world::{propagate_transforms, World};

    use super::*;
    use crate::{extract_scene, Camera, CameraPresentation};

    fn perspective(near: f32, far: f32) -> Mat4 {
        Mat4::perspective_lh(60f32.to_radians(), 1.0, near, far)
    }

    fn ortho(near: f32, far: f32) -> Mat4 {
        Mat4::orthographic_lh(-4.0, 4.0, -4.0, 4.0, near, far)
    }

    fn ndc_of(proj: Mat4, view: Vec3) -> f32 {
        let clip = proj * view.extend(1.0);
        clip.z / clip.w
    }

    fn linear(class: StoredDepthClass) -> (f32, f32) {
        match class {
            StoredDepthClass::Linear { view_depth, nearness } => (view_depth, nearness),
            other => panic!("expected a linear sample, got {other:?}"),
        }
    }

    #[test]
    fn reverse_z_perspective_and_ortho_round_trip_view_depth() {
        let cases: [(Mat4, f32); 3] = [(perspective(0.5, 20.0), 0.5), (ortho(0.5, 20.0), 0.5), (ortho(0.0, 20.0), 0.0)];
        for (proj, near) in cases {
            for z in [near.max(0.5_f32), 2.0, 6.0, 19.0] {
                let stored = ndc_of(proj, Vec3::new(0.2, -0.4, z));
                let (view_depth, nearness) = linear(classify_stored_depth(proj, 0.15, -0.2, stored));
                assert!((view_depth - z).abs() < 1.0e-3, "view z {view_depth} stored {stored} from {z}");
                let expected = linear_nearness(z, near, 20.0).unwrap();
                assert!((nearness - expected).abs() < 1.0e-3, "nearness {nearness} expected {expected}");
            }
            let (near_z, nearness) = linear(classify_stored_depth(proj, 0.0, 0.0, 1.0));
            assert!((near_z - near).abs() < 1.0e-3, "stored 1 is the near plane, got {near_z}");
            assert!((nearness - 1.0).abs() < 1.0e-4);
        }
    }

    #[test]
    fn nearer_samples_are_brighter_for_perspective_and_ortho() {
        for proj in [perspective(1.0, 20.0), ortho(1.0, 20.0)] {
            let near = linear(classify_stored_depth(proj, 0.0, 0.0, ndc_of(proj, Vec3::new(0.0, 0.0, 3.0))));
            let far = linear(classify_stored_depth(proj, 0.0, 0.0, ndc_of(proj, Vec3::new(0.0, 0.0, 15.0))));
            assert!(near.1 > far.1 + 0.2, "near {:?} far {:?}", near.1, far.1);
            assert!(near.0 < far.0);
        }
    }

    #[test]
    fn perspective_midpoint_is_linear_in_view_z_not_raw_ndc() {
        let proj = perspective(1.0, 9.0);
        let z = 5.0;
        let stored = ndc_of(proj, Vec3::new(0.0, 0.0, z));
        let (_, nearness) = linear(classify_stored_depth(proj, 0.0, 0.0, stored));
        assert!((nearness - 0.5).abs() < 1.0e-3, "linear midpoint");
        assert!(stored < 0.2, "the stored reverse-Z value at the midpoint is not 0.5, got {stored}");
    }

    #[test]
    fn clear_value_is_empty_and_limits_stay_ordered() {
        let proj = ortho(0.5, 20.0);
        assert_eq!(classify_stored_depth(proj, 0.0, 0.0, 0.0), StoredDepthClass::Empty);
        assert_eq!(classify_stored_depth(proj, 0.0, 0.0, -0.25), StoredDepthClass::Empty);
        let (near_z, nearness) = linear(classify_stored_depth(proj, 0.0, 0.0, 1.0));
        assert!((near_z - 0.5).abs() < 1.0e-3);
        assert!((nearness - 1.0).abs() < 1.0e-4);
        let (far_z, farness) = linear(classify_stored_depth(proj, 0.3, -0.4, 1.0e-5));
        assert!(far_z > 19.0, "{far_z}");
        assert!(farness < 0.01, "{farness}");
    }

    #[test]
    fn singular_standard_z_and_non_finite_inputs_are_unsupported() {
        let zero = Mat4::from_cols(Vec4::ZERO, Vec4::ZERO, Vec4::ZERO, Vec4::ZERO);
        assert_eq!(classify_stored_depth(zero, 0.0, 0.0, 0.0), StoredDepthClass::Unsupported);
        assert_eq!(classify_stored_depth(zero, 0.0, 0.0, 0.4), StoredDepthClass::Unsupported);

        let mut standard = ortho(0.5, 20.0);
        let span = 20.0 - 0.5;
        standard.cols[2].z = 1.0 / span;
        standard.cols[3].z = -0.5 / span;
        assert_eq!(classify_stored_depth(standard, 0.0, 0.0, 0.0), StoredDepthClass::Unsupported);
        assert_eq!(classify_stored_depth(standard, 0.0, 0.0, 0.8), StoredDepthClass::Unsupported);
        assert!(standard.inverse().is_some(), "a 0-near matrix can still be invertible");

        let proj = perspective(0.5, 20.0);
        assert_eq!(classify_stored_depth(proj, 0.0, 0.0, f32::NAN), StoredDepthClass::Unsupported);
        assert_eq!(classify_stored_depth(proj, f32::INFINITY, 0.0, 0.5), StoredDepthClass::Unsupported);
    }

    #[test]
    fn a_sheared_reverse_z_matrix_keeps_nearer_brighter() {
        let mut sheared = ortho(1.0, 20.0);
        sheared.cols[2].x = 0.25;
        let near = linear(classify_stored_depth(sheared, 0.1, 0.0, ndc_of(sheared, Vec3::new(0.0, 0.0, 4.0))));
        let far = linear(classify_stored_depth(sheared, 0.1, 0.0, ndc_of(sheared, Vec3::new(0.0, 0.0, 14.0))));
        assert!(near.1 > far.1);
        assert!((near.0 - 4.0).abs() < 2.0e-3, "{}", near.0);
        assert!((far.0 - 14.0).abs() < 2.0e-3, "{}", far.0);
    }

    #[test]
    fn classification_is_deterministic_and_presentation_defaults_to_shaded() {
        let proj = perspective(0.5, 20.0);
        let stored = ndc_of(proj, Vec3::new(0.0, 0.0, 6.0));
        assert_eq!(classify_stored_depth(proj, 0.2, -0.2, stored), classify_stored_depth(proj, 0.2, -0.2, stored));
        let camera = Camera::perspective(50.0);
        assert_eq!(camera.presentation, CameraPresentation::Shaded);
        assert_eq!(camera.with_presentation(CameraPresentation::Depth).presentation, CameraPresentation::Depth);
        assert_eq!(camera.presentation, CameraPresentation::Shaded);
        assert_eq!(
            camera
                .with_presentation(CameraPresentation::Depth)
                .with_presentation(CameraPresentation::Shaded)
                .presentation,
            CameraPresentation::Shaded
        );
    }

    #[test]
    fn extraction_copies_presentation_and_switching_does_not_rebuild_the_world() {
        let mut world = World::new();
        let camera = world.spawn((Camera::orthographic(8.0), Transform::from_xyz(0.0, 0.0, -4.0)));
        propagate_transforms(&mut world);
        let assets = AssetManager::in_memory();
        let size = Vec2::new(64.0, 64.0);
        let shaded = extract_scene(&world, &assets, size).unwrap();
        assert_eq!(shaded.sole_pass().presentation, CameraPresentation::Shaded);
        world.component_mut::<Camera>(camera).unwrap().presentation = CameraPresentation::Depth;
        let depth = extract_scene(&world, &assets, size).unwrap();
        assert_eq!(depth.sole_pass().presentation, CameraPresentation::Depth);
        assert_eq!(depth.sole_pass().camera_id, shaded.sole_pass().camera_id);
        world.component_mut::<Camera>(camera).unwrap().presentation = CameraPresentation::Shaded;
        let again = extract_scene(&world, &assets, size).unwrap();
        assert_eq!(again.sole_pass().presentation, CameraPresentation::Shaded);
    }
}
