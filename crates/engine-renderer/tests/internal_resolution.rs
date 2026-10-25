//! Integer-resolution presentation on the CPU: fit, picking, snap, and rejection.
//!
//! No GPU image is allocated here. A camera with no internal resolution stays on the direct path.

use engine_core::Color;
use engine_math::{Transform, Vec2, Vec3};
use engine_renderer::*;
use engine_world::{propagate_transforms, ComponentRegistry, GlobalTransform, World};

fn view(camera: Camera, eye: Vec3, target: Vec2) -> CameraView {
    let transform = Transform::from_xyz(eye.x, eye.y, eye.z);
    CameraView::new(&camera, &GlobalTransform { matrix: transform.to_matrix() }, target)
}

fn ortho(height: f32) -> Camera {
    Camera::orthographic(height)
}

#[test]
fn defaults_are_direct_and_black_bars() {
    let camera = Camera::orthographic(8.0);
    assert_eq!(camera.internal_resolution, None);
    assert_eq!(camera.letterbox_color, Color::BLACK);
    assert!(!camera.pixel_snap);
    let built = view(camera, Vec3::ZERO, Vec2::new(1280.0, 720.0));
    assert_eq!(built.fit, PresentationFit::Direct);
    assert_eq!(built.render_viewport, built.viewport);
    assert!(built.presented());
}

#[test]
fn builders_set_and_clear_the_resolution() {
    let camera = ortho(4.0).with_internal_resolution(320, 180).with_letterbox_color(Color::rgb(0.0, 1.0, 0.0));
    assert_eq!(camera.internal_resolution, Some((320, 180)));
    assert_eq!(camera.letterbox_color, Color::rgb(0.0, 1.0, 0.0));
    assert_eq!(camera.without_internal_resolution().internal_resolution, None);
}

#[test]
fn presets_do_not_enable_an_internal_resolution() {
    for placed in [
        PlacedCamera::perspective(Vec3::ZERO, 0.0, 0.2, 50.0).unwrap(),
        PlacedCamera::orthographic(Vec3::ZERO, 0.0, 0.2, 8.0).unwrap(),
        PlacedCamera::isometric(Vec3::ZERO, 12.0, 8.0).unwrap(),
        PlacedCamera::top_down(Vec3::ZERO, 12.0, 8.0).unwrap(),
        PlacedCamera::steep(Vec3::ZERO, 12.0, 50.0).unwrap(),
        PlacedCamera::side(Vec3::ZERO, 12.0, 8.0).unwrap(),
    ] {
        assert_eq!(placed.camera.internal_resolution, None, "{placed:?}");
        assert_eq!(placed.camera.letterbox_color, Color::BLACK);
    }
}

#[test]
fn the_registry_lists_the_resolution_and_the_letterbox_color() {
    let mut world = World::new();
    register_render_components(&mut world.resource_mut::<ComponentRegistry>()).unwrap();
    let entity = world.spawn((
        ortho(8.0).with_internal_resolution(320, 180).with_letterbox_color(Color::rgb(0.2, 0.0, 0.0)),
        Transform::IDENTITY,
    ));
    let dump = world.dump_entity(entity).unwrap();
    assert!(dump.contains("internal_resolution: Option<(u32, u32)>"), "{dump}");
    assert!(dump.contains("= 320x180"), "{dump}");
    assert!(dump.contains("letterbox_color: Color"), "{dump}");
    let off = world.spawn((Camera::perspective(40.0), Transform::IDENTITY));
    let off_dump = world.dump_entity(off).unwrap();
    assert!(
        off_dump.contains("internal_resolution: Option<(u32, u32)> script_visible=false script_writable=false = none"),
        "{off_dump}"
    );
}

#[test]
fn a_wide_output_scales_by_four_and_centers_the_bars() {
    let fit = fit_internal_resolution(320, 180, 1366, 768).unwrap();
    let PresentationFit::Scaled(image) = fit else { panic!("{fit:?}") };
    assert_eq!(image.scale, 4);
    assert_eq!(image.display_size, Vec2::new(1280.0, 720.0));
    assert_eq!(image.display_origin, Vec2::new(43.0, 24.0));
}

#[test]
fn an_exact_multiple_has_no_bars() {
    let fit = fit_internal_resolution(320, 180, 1280, 720).unwrap();
    let PresentationFit::Scaled(image) = fit else { panic!("{fit:?}") };
    assert_eq!(image.scale, 4);
    assert_eq!(image.display_origin, Vec2::ZERO);
    assert_eq!(image.display_size, Vec2::new(1280.0, 720.0));
}

#[test]
fn an_odd_remainder_sits_on_the_right_and_the_bottom() {
    let fit = fit_internal_resolution(4, 4, 11, 9).unwrap();
    let PresentationFit::Scaled(image) = fit else { panic!("{fit:?}") };
    assert_eq!(image.scale, 2);
    assert_eq!(image.display_origin, Vec2::new(1.0, 0.0));
    assert_eq!(image.display_size, Vec2::new(8.0, 8.0));
    let right = 11.0 - (image.display_origin.x + image.display_size.x);
    let bottom = 9.0 - (image.display_origin.y + image.display_size.y);
    assert_eq!(right, 2.0);
    assert_eq!(bottom, 1.0);
    assert!(right > image.display_origin.x);
    assert!(bottom > image.display_origin.y);
}

#[test]
fn a_viewport_smaller_than_the_image_is_letterbox_only() {
    let fit = fit_internal_resolution(8, 8, 4, 7).unwrap();
    assert_eq!(fit, PresentationFit::Letterbox { internal_width: 8, internal_height: 8 });
}

#[test]
fn zero_and_oversized_resolutions_are_rejected() {
    assert!(matches!(fit_internal_resolution(0, 10, 100, 100), Err(RenderTargetError::ZeroDimension { .. })));
    assert!(matches!(fit_internal_resolution(9000, 10, 100, 100), Err(RenderTargetError::Extent { .. })));
    let rejected = view(ortho(4.0).with_internal_resolution(0, 10), Vec3::ZERO, Vec2::new(100.0, 100.0));
    assert!(matches!(rejected.fit, PresentationFit::Rejected(PresentationReject::Size(_))));
    assert!(!rejected.presented());
    assert!(rejected.render_pixel(Vec2::new(10.0, 10.0)).is_none());
}

#[test]
fn load_with_an_internal_resolution_is_rejected_and_not_treated_as_color() {
    let camera = ortho(4.0).with_internal_resolution(16, 8).with_clear(CameraClear::Load);
    let built = view(camera, Vec3::ZERO, Vec2::new(64.0, 32.0));
    assert_eq!(built.fit, PresentationFit::Rejected(PresentationReject::Load { width: 16, height: 8 }));
    let too_small = view(
        ortho(4.0).with_internal_resolution(32, 32).with_clear(CameraClear::Load),
        Vec3::ZERO,
        Vec2::new(8.0, 8.0),
    );
    assert!(matches!(too_small.fit, PresentationFit::Rejected(PresentationReject::Load { .. })));
}

#[test]
fn a_split_viewport_letterboxes_inside_its_own_rectangle() {
    let camera = ortho(4.0).with_viewport(ViewportRect::new(0.5, 0.0, 0.5, 1.0)).with_internal_resolution(10, 10);
    let built = view(camera, Vec3::ZERO, Vec2::new(200.0, 100.0));
    let PresentationFit::Scaled(image) = built.fit else { panic!("{:?}", built.fit) };
    assert_eq!(built.viewport_origin, Vec2::new(100.0, 0.0));
    assert_eq!(built.viewport, Vec2::new(100.0, 100.0));
    assert_eq!(image.scale, 10);
    assert_eq!(image.display_origin, Vec2::new(100.0, 0.0));
    assert_eq!(built.render_viewport, Vec2::new(10.0, 10.0));
}

#[test]
fn resizing_the_destination_changes_the_scale_and_not_the_internal_size() {
    let camera = ortho(8.0).with_internal_resolution(320, 180);
    let large = view(camera, Vec3::ZERO, Vec2::new(1280.0, 720.0));
    let small = view(camera, Vec3::ZERO, Vec2::new(640.0, 360.0));
    let PresentationFit::Scaled(large_image) = large.fit else { panic!("{:?}", large.fit) };
    let PresentationFit::Scaled(small_image) = small.fit else { panic!("{:?}", small.fit) };
    assert_eq!(large_image.scale, 4);
    assert_eq!(small_image.scale, 2);
    assert_eq!(large.render_viewport, small.render_viewport);
}

#[test]
fn physical_pixels_are_the_destination_and_the_scale_factor_is_not_applied_again() {
    let camera = ortho(8.0).with_internal_resolution(320, 180);
    let hidpi = view(camera, Vec3::ZERO, Vec2::new(2560.0, 1440.0));
    let PresentationFit::Scaled(image) = hidpi.fit else { panic!("{:?}", hidpi.fit) };
    assert_eq!(image.scale, 8);
    assert_eq!(image.display_size, Vec2::new(2560.0, 1440.0));
}

#[test]
fn the_center_of_a_block_maps_to_the_center_of_the_internal_pixel() {
    let camera = ortho(8.0).with_internal_resolution(320, 180);
    let built = view(camera, Vec3::ZERO, Vec2::new(1366.0, 768.0));
    assert!(built.render_pixel(Vec2::new(0.0, 0.0)).is_none(), "left bar");
    assert!(built.render_pixel(Vec2::new(42.0, 24.0)).is_none(), "pixel before the image");
    assert_eq!(built.render_pixel(Vec2::new(45.0, 26.0)), Some(Vec2::new(0.5, 0.5)));
    assert!(built.ray_from_window_pixel(Vec2::new(10.0, 10.0)).is_none());
    assert!(built.ray_from_window_pixel(Vec2::new(45.0, 26.0)).is_some());
    let direct = view(ortho(8.0), Vec3::ZERO, Vec2::new(1366.0, 768.0));
    let pixel = Vec2::new(45.0, 26.0);
    assert_eq!(direct.render_pixel(pixel), Some(direct.window_to_viewport(pixel)));
    let (direct_ray, _) = direct.ray_from_pixels(pixel).unwrap();
    let (window_ray, _) = direct.ray_from_window_pixel(pixel).unwrap();
    assert_eq!(direct_ray, window_ray);
}

#[test]
fn bars_belong_to_the_scaled_camera_and_a_rejected_camera_does_not_own_them() {
    let mut world = World::new();
    world.spawn((ortho(4.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)), Transform::from_xyz(0.0, 0.0, -4.0)));
    world.spawn((
        Camera {
            order: 1,
            ..ortho(4.0).with_viewport(ViewportRect::new(0.25, 0.25, 0.5, 0.5)).with_internal_resolution(6, 4)
        },
        Transform::from_xyz(0.0, 0.0, -4.0),
    ));
    propagate_transforms(&mut world);
    let size = Vec2::new(40.0, 40.0);
    // Inset is (10, 10, 20, 20). Scale 3 leaves a top bar, so y=10 is inside the inset and not the image.
    let bar = top_surface_camera(&world, Vec2::new(20.0, 10.0), size).unwrap();
    assert_eq!(bar.order, 1);
    assert!(bar.render_pixel.is_none());
    let outside = top_surface_camera(&world, Vec2::new(2.0, 2.0), size).unwrap();
    assert_eq!(outside.order, 0);
    assert!(outside.render_pixel.is_some());
}

#[test]
fn orthographic_snap_uses_the_internal_height() {
    let eye = Vec3::new(0.6, 0.0, -4.0);
    let target = Vec2::new(1280.0, 720.0);
    let internal = view(ortho(180.0).with_pixel_snap(true).with_internal_resolution(320, 180), eye, target);
    let small = view(ortho(180.0).with_pixel_snap(true), eye, Vec2::new(320.0, 180.0));
    let large = view(ortho(180.0).with_pixel_snap(true), eye, target);
    assert_eq!(internal.position, small.position);
    assert_ne!(internal.position, large.position);
    assert_eq!(internal.render_viewport.y, 180.0);
    let perspective = Camera::perspective(50.0).with_pixel_snap(true).with_internal_resolution(320, 180);
    let perspective_off = perspective.with_pixel_snap(false);
    assert_eq!(view(perspective, eye, target).view, view(perspective_off, eye, target).view);
    let custom =
        Camera::new(CameraProjection::oblique_orthographic(8.0, 2.0, 0.1, 100.0, Vec2::new(0.5, 0.25)).unwrap())
            .with_pixel_snap(true)
            .with_internal_resolution(32, 16);
    let custom_off = custom.with_pixel_snap(false);
    let on = view(custom, eye, Vec2::new(64.0, 32.0));
    let off = view(custom_off, eye, Vec2::new(64.0, 32.0));
    assert_eq!(on.view, off.view);
    assert_eq!(on.proj, off.proj);
    assert_eq!(on.proj, custom.projection.matrix(1.0));
}

#[test]
fn extraction_does_not_write_the_transform_and_skips_rejected_cameras() {
    let mut world = World::new();
    world.insert_resource(engine_assets::AssetManager::in_memory());
    let eye = Transform::from_xyz(0.6, 0.2, -4.0);
    let kept = world.spawn((ortho(8.0).with_internal_resolution(32, 16).with_pixel_snap(true), eye));
    world.spawn((
        Camera { order: 1, ..ortho(8.0).with_internal_resolution(8, 8).with_clear(CameraClear::Load) },
        Transform::IDENTITY,
    ));
    propagate_transforms(&mut world);
    let before = world.component::<GlobalTransform>(kept).unwrap().matrix;
    let scene = extract_scene(&world, &world.resource::<engine_assets::AssetManager>(), Vec2::new(64.0, 32.0)).unwrap();
    let after = world.component::<GlobalTransform>(kept).unwrap().matrix;
    assert_eq!(before, after);
    assert_eq!(scene.passes.len(), 1);
    assert_eq!(scene.stats.presentation_skipped, 1);
    assert!(matches!(scene.passes[0].view.fit, PresentationFit::Scaled(_)));
}

#[test]
fn a_too_small_viewport_skips_the_scene_but_still_owns_the_pass() {
    let mut world = World::new();
    let mut assets = engine_assets::AssetManager::in_memory();
    let texture = assets.insert("t", engine_assets::Texture::solid(1, 1, [255, 255, 255, 255])).unwrap();
    assets.insert("s", engine_assets::SpriteAsset::whole_texture(texture, 1, 1)).unwrap();
    world.insert_resource(assets);
    world.spawn((ortho(4.0).with_internal_resolution(32, 32), Transform::from_xyz(0.0, 0.0, -4.0)));
    world.spawn((Sprite::new("s").with_size(Vec2::splat(1.0)), Transform::IDENTITY));
    propagate_transforms(&mut world);
    let scene = extract_scene(&world, &world.resource::<engine_assets::AssetManager>(), Vec2::new(8.0, 8.0)).unwrap();
    assert_eq!(scene.passes.len(), 1);
    assert!(matches!(scene.passes[0].view.fit, PresentationFit::Letterbox { .. }));
    assert_eq!(scene.passes[0].stats.sprites_visible, 0);
    assert_eq!(scene.stats.presentation_skipped, 0);
}

#[test]
fn two_cameras_keep_independent_internal_sizes() {
    let mut world = World::new();
    world.insert_resource(engine_assets::AssetManager::in_memory());
    world.spawn((ortho(4.0).with_internal_resolution(16, 8), Transform::from_xyz(0.0, 0.0, -4.0)));
    world.spawn((
        Camera { order: 1, ..ortho(4.0).with_internal_resolution(10, 10) },
        Transform::from_xyz(1.0, 0.0, -4.0),
    ));
    propagate_transforms(&mut world);
    let scene = extract_scene(&world, &world.resource::<engine_assets::AssetManager>(), Vec2::new(80.0, 40.0)).unwrap();
    assert_eq!(scene.passes[0].view.render_viewport, Vec2::new(16.0, 8.0));
    assert_eq!(scene.passes[1].view.render_viewport, Vec2::new(10.0, 10.0));
    assert_ne!(scene.passes[0].view.position, scene.passes[1].view.position);
}

#[test]
fn an_offscreen_camera_fits_against_the_target_and_not_the_surface() {
    let mut world = World::new();
    let mut store = RenderTargetStore::new();
    let target = store.create(64, 32).unwrap();
    world.insert_resource(store);
    let entity = world.spawn((
        ortho(4.0).with_output(CameraOutput::Target(target)).with_internal_resolution(8, 4),
        Transform::from_xyz(0.0, 0.0, -4.0),
    ));
    propagate_transforms(&mut world);
    let built = camera_view_of(&world, entity.id(), Vec2::new(1280.0, 720.0)).unwrap();
    let PresentationFit::Scaled(image) = built.fit else { panic!("{:?}", built.fit) };
    assert_eq!(image.scale, 8);
    assert_eq!(built.render_viewport, Vec2::new(8.0, 4.0));
    assert_eq!(built.viewport, Vec2::new(64.0, 32.0));
}
