//! CPU contract for the renderer-wide sample count. No GPU.

use engine_renderer::*;
use engine_world::World;

fn target_id() -> RenderTargetId {
    let mut world = World::new();
    world.insert_resource(RenderTargetStore::new());
    let id = world.resource_mut::<RenderTargetStore>().create(8, 8).unwrap();
    id
}

#[test]
fn one_is_the_default_and_four_is_the_only_other_count() {
    assert_eq!(MsaaSamples::default(), MsaaSamples::One);
    assert_eq!(MsaaSamples::One.count(), 1);
    assert_eq!(MsaaSamples::Four.count(), 4);
}

#[test]
fn a_rejected_four_request_does_not_become_one() {
    let error = select_msaa(MsaaSamples::One, MsaaSamples::Four, false, true).unwrap_err();
    assert_eq!(error, MsaaError::Unavailable { color: false, depth: true });
    let error = select_msaa(MsaaSamples::Four, MsaaSamples::Four, true, false).unwrap_err();
    assert_eq!(error, MsaaError::Unavailable { color: true, depth: false });
    assert!(select_msaa(MsaaSamples::One, MsaaSamples::Four, false, false).is_err());
    assert_eq!(select_msaa(MsaaSamples::Four, MsaaSamples::One, false, false).unwrap(), MsaaSamples::One);
    assert_eq!(select_msaa(MsaaSamples::One, MsaaSamples::Four, true, true).unwrap(), MsaaSamples::Four);
}

#[test]
fn stage1_four_accepts_only_a_direct_shaded_surface_clear() {
    let direct = msaa4_camera_skip(
        MsaaSamples::Four,
        PresentationFit::Direct,
        CameraOutput::Surface,
        CameraClear::Color,
        CameraPresentation::Shaded,
    );
    assert_eq!(direct, None);
    assert_eq!(
        msaa4_camera_skip(
            MsaaSamples::Four,
            PresentationFit::Direct,
            CameraOutput::Surface,
            CameraClear::Load,
            CameraPresentation::Shaded,
        ),
        Some(MsaaSkip::Load)
    );
    assert_eq!(
        msaa4_camera_skip(
            MsaaSamples::Four,
            PresentationFit::Direct,
            CameraOutput::Surface,
            CameraClear::Color,
            CameraPresentation::Depth,
        ),
        Some(MsaaSkip::DepthPresentation)
    );
    assert_eq!(
        msaa4_camera_skip(
            MsaaSamples::Four,
            PresentationFit::Direct,
            CameraOutput::Target(target_id()),
            CameraClear::Color,
            CameraPresentation::Shaded,
        ),
        Some(MsaaSkip::Offscreen)
    );
    assert_eq!(
        msaa4_camera_skip(
            MsaaSamples::Four,
            PresentationFit::Letterbox { internal_width: 8, internal_height: 8 },
            CameraOutput::Surface,
            CameraClear::Color,
            CameraPresentation::Shaded,
        ),
        Some(MsaaSkip::InternalResolution)
    );
}

#[test]
fn one_sample_does_not_skip_combinations_stage1_rejects() {
    for (fit, output, clear, presentation) in [
        (PresentationFit::Direct, CameraOutput::Surface, CameraClear::Load, CameraPresentation::Shaded),
        (PresentationFit::Direct, CameraOutput::Surface, CameraClear::Color, CameraPresentation::Depth),
        (PresentationFit::Direct, CameraOutput::Target(target_id()), CameraClear::Color, CameraPresentation::Shaded),
        (
            PresentationFit::Letterbox { internal_width: 2, internal_height: 2 },
            CameraOutput::Surface,
            CameraClear::Color,
            CameraPresentation::Shaded,
        ),
    ] {
        assert_eq!(msaa4_camera_skip(MsaaSamples::One, fit, output, clear, presentation), None);
    }
}

#[test]
fn skip_messages_name_the_camera_and_the_reason() {
    let load = MsaaSkip::Load.message(9);
    assert!(load.contains("camera 9"));
    assert!(load.contains("CameraClear::Load"));
    assert!(MsaaSkip::ImagesUnavailable.message(4).contains("not replaced with 1×"));
    assert_ne!(MsaaSkip::Load.message(1), MsaaSkip::Offscreen.message(1));
}
