//! Orthographic camera-view translation snapping.
//!
//! `f32::round` takes the view-space X and Y translation to the nearest viewport pixel and rounds
//! a tie away from zero. A camera move smaller than one pixel still changes the view when it
//! crosses that halfway value. Perspective and custom projections keep the flag and do not snap.

use core::f32::consts::{FRAC_PI_2, FRAC_PI_4};

use engine_assets::{AssetManager, MeshData, SpriteAsset, Texture};
use engine_core::{Color, FixedTimestep};
use engine_math::{Mat4, Quat, Transform, Vec2, Vec3};
use engine_renderer::*;
use engine_world::{propagate_transforms, ComponentRegistry, GlobalTransform, Interpolate, World};

const SQUARE: Vec2 = Vec2::new(32.0, 32.0);

fn view_of(camera: Camera, transform: Transform, target: Vec2) -> CameraView {
    CameraView::new(&camera, &GlobalTransform { matrix: transform.to_matrix() }, target)
}

fn ortho(height: f32, snap: bool) -> Camera {
    Camera::orthographic(height).with_pixel_snap(snap)
}

fn at(x: f32, y: f32) -> Transform {
    Transform::from_xyz(x, y, -10.0)
}

fn translation_px(view: &CameraView, height: f32) -> Vec2 {
    let scale = view.viewport.y / height;
    Vec2::new(view.view.cols[3].x * scale, view.view.cols[3].y * scale)
}

fn assert_axis_pixels(px: Vec2) {
    assert!((px.x - px.x.round()).abs() < 1e-4 && (px.y - px.y.round()).abs() < 1e-4, "{px:?}");
}

fn rotation_eq(a: &Mat4, b: &Mat4) {
    for column in 0..3 {
        assert!(
            a.cols[column].xyz().approx_eq(b.cols[column].xyz(), 1e-5),
            "column {column}: {:?} vs {:?}",
            a.cols[column],
            b.cols[column]
        );
    }
}

fn assets() -> AssetManager {
    let mut assets = AssetManager::in_memory();
    let texture = assets.insert("white", Texture::solid(4, 4, [255, 255, 255, 255])).unwrap();
    assets.insert("sprite", SpriteAsset::whole_texture(texture, 4, 4)).unwrap();
    assets.insert("cube", MeshData::cuboid(Vec3::ONE).unwrap()).unwrap();
    assets
}

fn ray_miss(origin: Vec3, direction: Vec3, point: Vec3) -> f32 {
    let offset = point - origin;
    (offset - direction * offset.dot(direction)).length()
}

#[test]
fn snapping_is_off_unless_the_builder_says_so() {
    assert!(!Camera::orthographic(8.0).pixel_snap);
    assert!(!Camera::perspective(50.0).pixel_snap);
    assert!(!Camera::new(CameraProjection::orthographic(4.0)).pixel_snap);
    let on = Camera::orthographic(8.0).with_pixel_snap(true);
    assert!(on.pixel_snap);
    assert!(!on.with_pixel_snap(false).pixel_snap);
    for placed in [
        PlacedCamera::perspective(Vec3::ZERO, 0.0, 0.2, 50.0).unwrap(),
        PlacedCamera::orthographic(Vec3::ZERO, 0.0, 0.2, 8.0).unwrap(),
        PlacedCamera::isometric(Vec3::ZERO, 12.0, 8.0).unwrap(),
        PlacedCamera::top_down(Vec3::ZERO, 12.0, 8.0).unwrap(),
        PlacedCamera::steep(Vec3::ZERO, 12.0, 50.0).unwrap(),
        PlacedCamera::side(Vec3::ZERO, 12.0, 8.0).unwrap(),
    ] {
        assert!(!placed.camera.pixel_snap, "{placed:?}");
    }
}

#[test]
fn the_registry_lists_pixel_snap_with_the_camera_value() {
    let mut world = World::new();
    register_render_components(&mut world.resource_mut::<ComponentRegistry>()).unwrap();
    let entity = world.spawn((ortho(8.0, true), at(0.0, 0.0)));
    let dump = world.dump_entity(entity).unwrap();
    assert!(dump.contains("pixel_snap: bool script_visible=false script_writable=false = true"), "{dump}");
    let off = world.spawn((Camera::perspective(40.0), Transform::IDENTITY));
    let off_dump = world.dump_entity(off).unwrap();
    assert!(off_dump.contains("pixel_snap: bool script_visible=false script_writable=false = false"), "{off_dump}");
}

#[test]
fn a_disabled_orthographic_camera_matches_the_unsnapped_view() {
    let transform = at(0.4, -0.25);
    let view = view_of(ortho(32.0, false), transform, SQUARE);
    let expected = Mat4::look_to_lh(transform.position, transform.forward(), transform.up());
    assert_eq!(view.view, expected);
    assert_eq!(view.position, transform.position);
}

#[test]
fn fractional_translations_round_away_from_zero_on_both_axes() {
    let snapped = view_of(ortho(32.0, true), at(1.6, -2.4), SQUARE);
    assert!(snapped.position.approx_eq(Vec3::new(2.0, -2.0, -10.0), 1e-5));
    assert_axis_pixels(translation_px(&snapped, 32.0));
    let negative = view_of(ortho(32.0, true), at(-1.6, 2.4), SQUARE);
    assert!(negative.position.approx_eq(Vec3::new(-2.0, 2.0, -10.0), 1e-5));
    let positive_small = view_of(ortho(32.0, true), at(0.25, 0.25), SQUARE);
    assert!(positive_small.position.approx_eq(Vec3::new(0.0, 0.0, -10.0), 1e-5));
}

#[test]
fn half_pixels_round_away_from_zero_and_the_next_value_does_not() {
    let halfway = view_of(ortho(32.0, true), at(0.5, -0.5), SQUARE);
    assert!(halfway.position.approx_eq(Vec3::new(1.0, -1.0, -10.0), 1e-5), "{:?}", halfway.position);
    let just_inside = view_of(ortho(32.0, true), at(0.5f32.next_down(), (-0.5f32).next_up()), SQUARE);
    assert!(just_inside.position.approx_eq(Vec3::new(0.0, 0.0, -10.0), 1e-4), "{:?}", just_inside.position);
    let point = Vec3::new(0.0, 0.0, 4.0);
    let jumped = halfway.project(point).unwrap().pixels.x - just_inside.project(point).unwrap().pixels.x;
    assert!((jumped + 1.0).abs() < 1e-3, "one ulp across the boundary moves one pixel, got {jumped}");
}

#[test]
fn movement_inside_one_bin_is_stable_and_crossing_the_boundary_jumps() {
    let low = view_of(ortho(32.0, true), at(0.125, 0.0), SQUARE);
    let high = view_of(ortho(32.0, true), at(0.375, 0.0), SQUARE);
    assert!(low.view.approx_eq(&high.view, 1e-5), "0.25 world units is under one pixel and stays in the bin");
    let across = view_of(ortho(32.0, true), at(0.625, 0.0), SQUARE);
    let point = Vec3::new(1.0, 0.0, 4.0);
    let delta = high.project(point).unwrap().pixels.x - across.project(point).unwrap().pixels.x;
    assert!((delta - 1.0).abs() < 1e-3, "a 0.25 move that crosses 0.5 jumps one pixel, got {delta}");
}

#[test]
fn one_camera_pixel_moves_the_picture_by_one_pixel() {
    let origin = view_of(ortho(32.0, true), at(0.0, 0.0), SQUARE);
    let shifted = view_of(ortho(32.0, true), at(1.0, 0.0), SQUARE);
    let point = Vec3::new(2.0, -3.0, 4.0);
    let (a, b) = (origin.project(point).unwrap().pixels, shifted.project(point).unwrap().pixels);
    assert!((a.x - b.x - 1.0).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3, "{a:?} vs {b:?}");
    let neighbor = origin.project(point + Vec3::X).unwrap().pixels;
    assert!((neighbor.x - a.x - 1.0).abs() < 1e-3, "one world unit is one pixel");
}

#[test]
fn zoom_and_viewport_height_change_the_pixel_size() {
    let wide_pixels = view_of(ortho(16.0, true), at(0.375, 0.0), SQUARE);
    assert!(
        wide_pixels.position.x > 0.4,
        "two pixels per unit rounds 0.375 to the next pixel: {:?}",
        wide_pixels.position
    );
    let coarse = view_of(ortho(32.0, true), at(0.375, 0.0), SQUARE);
    assert!(coarse.position.x.abs() < 1e-4, "{:?}", coarse.position);
    let tall = view_of(ortho(32.0, true), at(0.375, 0.0), Vec2::new(64.0, 64.0));
    assert!((tall.position.x - 0.5).abs() < 1e-4, "a 64px viewport is two pixels per unit: {:?}", tall.position);
}

#[test]
fn a_split_viewport_uses_its_own_pixel_height_and_origin() {
    let full = view_of(ortho(32.0, true), at(0.375, 0.0), Vec2::new(64.0, 64.0));
    let lower = Camera::orthographic(32.0).with_pixel_snap(true).with_viewport(ViewportRect::new(0.0, 0.5, 1.0, 0.5));
    let split = view_of(lower, at(0.375, 0.0), Vec2::new(64.0, 64.0));
    assert!(split.viewport_origin.approx_eq(Vec2::new(0.0, 32.0), 1e-4));
    assert!(split.viewport.approx_eq(Vec2::new(64.0, 32.0), 1e-4));
    assert!(split.position.x.abs() < 1e-4, "half height is one pixel per unit: {:?}", split.position);
    assert!((full.position.x - 0.5).abs() < 1e-4);
    let side = Camera::orthographic(32.0).with_pixel_snap(true).with_viewport(ViewportRect::new(0.5, 0.0, 0.5, 1.0));
    let right = view_of(side, at(0.375, 0.0), Vec2::new(64.0, 64.0));
    assert!(right.viewport_origin.approx_eq(Vec2::new(32.0, 0.0), 1e-4));
    assert!((right.position.x - full.position.x).abs() < 1e-4, "same pixel height, same snap");
    let point = Vec3::new(0.0, 0.0, 4.0);
    let local = right.project(point).unwrap().pixels;
    let full_pixels = view_of(ortho(32.0, true), at(0.375, 0.0), Vec2::new(32.0, 64.0)).project(point).unwrap().pixels;
    assert!(local.approx_eq(full_pixels, 1e-3), "project is viewport-local: {local:?} vs {full_pixels:?}");
}

#[test]
fn odd_viewports_put_the_axis_on_a_pixel_center_without_an_extra_bias() {
    let odd = view_of(ortho(33.0, true), at(0.4, 0.0), Vec2::new(33.0, 33.0));
    let center = odd.project(Vec3::new(0.0, 0.0, 4.0)).unwrap().pixels;
    assert!(center.approx_eq(Vec2::splat(16.5), 1e-3), "odd viewport center is a pixel center: {center:?}");
    let even = view_of(ortho(32.0, true), at(0.0, 0.0), SQUARE);
    let edge = even.project(Vec3::new(0.0, 0.0, 4.0)).unwrap().pixels;
    assert!(edge.approx_eq(Vec2::splat(16.0), 1e-3), "even viewport center is a pixel edge: {edge:?}");
    let already = view_of(ortho(32.0, true), at(2.0, -3.0), SQUARE);
    assert!(
        already.position.approx_eq(Vec3::new(2.0, -3.0, -10.0), 1e-5),
        "an integer pose does not gain a half pixel"
    );
}

#[test]
fn side_top_down_and_isometric_placements_snap_along_the_view_axes() {
    let target = Vec2::new(32.0, 32.0);
    for (name, placed) in [
        ("side", PlacedCamera::side(Vec3::ZERO, 12.0, 32.0).unwrap()),
        ("top-down", PlacedCamera::top_down(Vec3::ZERO, 12.0, 32.0).unwrap()),
        ("isometric", PlacedCamera::isometric(Vec3::ZERO, 12.0, 32.0).unwrap()),
    ] {
        let shifted = Transform::from_position(placed.transform.position + placed.transform.right() * 0.125)
            .with_rotation(placed.transform.rotation);
        let camera = placed.camera.with_pixel_snap(true);
        let snapped = view_of(camera, shifted, target);
        let home = view_of(camera, placed.transform, target);
        assert!(snapped.view.approx_eq(&home.view, 1e-4), "{name} drops a quarter pixel along right");
        assert!(snapped.forward.approx_eq(placed.transform.forward(), 1e-4), "{name}");
        let raw = view_of(ortho(32.0, false), shifted, target);
        assert!((snapped.view.cols[3].z - raw.view.cols[3].z).abs() < 1e-4, "{name} keeps view z");
        assert_axis_pixels(translation_px(&snapped, 32.0));
        assert!(!placed.camera.pixel_snap);
    }
}

#[test]
fn roll_quantizes_view_axes_and_leaves_the_rotation_alone() {
    let rolled = at(0.6, 0.0).with_rotation(Quat::from_rotation_z(FRAC_PI_4));
    let snapped = view_of(ortho(32.0, true), rolled, SQUARE);
    let raw = view_of(ortho(32.0, false), rolled, SQUARE);
    rotation_eq(&snapped.view, &raw.view);
    assert!((snapped.view.cols[3].z - raw.view.cols[3].z).abs() < 1e-4);
    assert_axis_pixels(translation_px(&snapped, 32.0));
    let planar = snapped.position - rolled.position;
    assert!(planar.dot(rolled.forward()).abs() < 1e-4, "{planar:?}");
    assert!(snapped.position.x.abs() < 0.05, "45 degrees puts 0.6 inside both view bins: {:?}", snapped.position);
    let flat = view_of(ortho(32.0, true), at(0.6, 0.0), SQUARE);
    assert!(flat.position.x > 0.9, "the unrolled camera rounds the same world offset the other way");
}

#[test]
fn perspective_and_custom_matrices_ignore_the_flag() {
    let transform = at(0.4, -0.7).with_rotation(Quat::from_rotation_y(0.3) * Quat::from_rotation_x(0.2));
    let perspective = Camera::perspective(50.0).with_pixel_snap(true);
    let perspective_off = perspective.with_pixel_snap(false);
    let on = view_of(perspective, transform, SQUARE);
    let off = view_of(perspective_off, transform, SQUARE);
    assert_eq!(on.view, off.view);
    assert_eq!(on.view_proj, off.view_proj);
    assert_eq!(on.position, transform.position);
    assert!(perspective.pixel_snap);

    let oblique = CameraProjection::oblique_orthographic(32.0, 1.0, 0.1, 100.0, Vec2::new(0.2, -0.05)).unwrap();
    let custom = Camera::new(oblique).with_pixel_snap(true);
    let custom_on = view_of(custom, transform, Vec2::new(40.0, 20.0));
    let custom_off = view_of(custom.with_pixel_snap(false), transform, Vec2::new(40.0, 20.0));
    assert_eq!(custom_on.view, custom_off.view);
    assert_eq!(custom_on.proj, custom_off.proj);
    assert_eq!(custom_on.view_proj, custom_off.view_proj);
    let point = Vec3::new(1.0, 2.0, 6.0);
    assert!(custom_on.project(point).unwrap().pixels.approx_eq(custom_off.project(point).unwrap().pixels, 0.0));

    let negative = Camera::orthographic(-8.0).with_pixel_snap(true);
    let negative_view = view_of(negative, transform, SQUARE);
    let negative_off = view_of(negative.with_pixel_snap(false), transform, SQUARE);
    assert_eq!(negative_view.view, negative_off.view);
}

#[test]
fn extraction_does_not_write_transforms_and_culls_with_the_snapped_frustum() {
    let mut world = World::new();
    let assets = assets();
    let camera = world.spawn((ortho(8.0, true), at(0.4, 0.0)));
    let sprite_at = Vec3::new(4.25, 0.0, 0.0);
    let mesh_at = Vec3::new(0.35, 0.2, 1.0);
    world.spawn((
        Transform::from_position(sprite_at),
        Sprite::new("sprite").with_size(Vec2::splat(0.2)).with_anchor(Vec2::splat(0.5)).cutout().unlit(),
    ));
    world.spawn((Transform::from_position(mesh_at), Mesh::new("cube").with_material(Material::unlit(Color::WHITE))));
    let mut lines = DebugDraw::default();
    lines.line(Vec3::ZERO, Vec3::X, Color::WHITE);
    world.insert_resource(lines);
    propagate_transforms(&mut world);
    let scene = extract_scene(&world, &assets, Vec2::splat(8.0)).unwrap();
    assert_eq!(world.component::<Transform>(camera).unwrap().position, Vec3::new(0.4, 0.0, -10.0));
    assert!(scene.sole_pass().view.position.x.abs() < 1e-4);
    assert_eq!(scene.stats.sprites_visible, 0);
    assert_eq!(scene.stats.sprites_culled, 1);
    let mesh = &scene.sole_pass().meshes[0];
    assert!(mesh.model.translation().approx_eq(mesh_at, 1e-4));
    assert_eq!(scene.debug_lines[0].start, Vec3::ZERO);
    assert_eq!(scene.debug_lines[0].end, Vec3::X);

    let raw = Frustum::from_view_proj(&view_of(ortho(8.0, false), at(0.4, 0.0), Vec2::splat(8.0)).view_proj);
    let snapped = Frustum::from_view_proj(&scene.sole_pass().view.view_proj);
    assert!(raw.intersects_sphere(sprite_at, 0.0));
    assert!(!snapped.intersects_sphere(sprite_at, 0.0));
}

#[test]
fn interpolation_snaps_the_blended_pose_and_leaves_the_components() {
    let mut world = World::new();
    let assets = assets();
    // Half a 60 Hz step is just under 0.5. 0 and 1.25 blend to about 0.625, which snaps to 1.
    // Lerping the already-snapped eyes (0 and 1) would still be about 0.5.
    let previous = at(0.0, 0.0);
    let current = at(1.25, 0.0);
    let camera = world.spawn((current, ortho(32.0, true), Interpolate { previous: Some(previous) }));
    let mut time = FixedTimestep::new(60);
    assert_eq!(time.accumulate(time.step() / 2), 0);
    world.insert_resource(time);
    propagate_transforms(&mut world);
    let scene = extract_scene(&world, &assets, SQUARE).unwrap();
    assert!(scene.sole_pass().view.position.approx_eq(Vec3::new(1.0, 0.0, -10.0), 1e-4));
    assert_eq!(world.component::<Transform>(camera).unwrap().position, current.position);
    assert_eq!(world.component::<Interpolate>(camera).unwrap().previous, Some(previous));
}

#[test]
fn two_cameras_keep_independent_snap_policies() {
    let mut world = World::new();
    let assets = assets();
    let pose = at(0.4, -0.2);
    world.spawn((ortho(32.0, false), pose));
    world.spawn((Camera { order: 1, ..ortho(32.0, true) }, pose));
    world.spawn((
        Transform::from_xyz(1.0, 0.0, 0.0),
        Sprite::new("sprite").with_size(Vec2::splat(1.0)).with_anchor(Vec2::ZERO).cutout().unlit(),
    ));
    propagate_transforms(&mut world);
    let scene = extract_scene(&world, &assets, SQUARE).unwrap();
    assert_eq!(scene.passes.len(), 2);
    let (raw, snapped) = (&scene.passes[0].view, &scene.passes[1].view);
    assert_eq!(raw.position, pose.position);
    assert!(snapped.position.x.abs() < 1e-4);
    let point = Vec3::new(1.0, 0.0, 0.0);
    let delta = raw.project(point).unwrap().pixels.x - snapped.project(point).unwrap().pixels.x;
    assert!(delta.abs() > 0.2, "{delta}");
    let models: Vec<_> = scene.passes.iter().map(|pass| pass.cutout_sprites.instances[0].model.translation()).collect();
    assert!(models[0].approx_eq(models[1], 1e-5));
    assert!(models[0].approx_eq(point, 1e-4));
}

#[test]
fn inverse_matrices_and_picking_rays_use_the_snapped_view() {
    let mut world = World::new();
    let pose = at(0.4, 0.2);
    let entity = world.spawn((ortho(32.0, true), pose));
    propagate_transforms(&mut world);
    let resolved = camera_view_of(&world, entity.id(), SQUARE).unwrap();
    let direct = view_of(ortho(32.0, true), pose, SQUARE);
    assert!(resolved.view.approx_eq(&direct.view, 1e-5));
    let inverse = direct.view.inverse().unwrap();
    assert!((inverse * direct.view).approx_eq(&Mat4::IDENTITY, 1e-4));
    let inverse_clip = direct.view_proj.inverse().unwrap();
    assert!((inverse_clip * direct.view_proj).approx_eq(&Mat4::IDENTITY, 1e-4));

    let point = Vec3::new(1.25, -0.5, 4.0);
    let pixels = direct.project(point).unwrap().pixels;
    let (origin, direction) = direct.ray_from_pixels(pixels).unwrap();
    assert!(ray_miss(origin, direction, point) < 1e-3, "snapped ray");
    let raw = view_of(ortho(32.0, false), pose, SQUARE);
    let (raw_origin, raw_direction) = raw.ray_from_pixels(pixels).unwrap();
    let miss = ray_miss(raw_origin, raw_direction, point);
    assert!(miss > 0.2, "the unsnapped camera misses the snapped pixel by {miss}");
}

#[test]
fn reverse_z_near_and_far_are_unchanged_by_the_translation_snap() {
    let pose = at(0.45, -0.3);
    let snapped = view_of(ortho(32.0, true), pose, SQUARE);
    let raw = view_of(ortho(32.0, false), pose, SQUARE);
    let near = Vec3::new(0.2, -0.2, -10.0 + 0.1);
    let far = Vec3::new(0.2, -0.2, -10.0 + 1000.0);
    let mid = Vec3::new(0.2, -0.2, -10.0 + 500.0);
    for point in [near, far, mid] {
        let a = snapped.project(point).unwrap();
        let b = raw.project(point).unwrap();
        assert!((a.depth - b.depth).abs() < 1e-5 && (a.view_depth - b.view_depth).abs() < 1e-4, "{a:?} vs {b:?}");
    }
    let near_depth = snapped.project(near).unwrap().depth;
    let far_depth = snapped.project(far).unwrap().depth;
    assert!((near_depth - 1.0).abs() < 1e-4, "{near_depth}");
    assert!(far_depth.abs() < 1e-4, "{far_depth}");
    assert!(snapped.project(mid).unwrap().depth > far_depth && snapped.project(mid).unwrap().depth < near_depth);
    assert_eq!(snapped.proj, raw.proj);
}

#[test]
fn a_top_down_pitch_still_has_a_defined_snap_basis() {
    let pose = Transform::from_xyz(0.4, 12.0, -0.3).with_rotation(Quat::from_rotation_x(FRAC_PI_2));
    let snapped = view_of(ortho(32.0, true), pose, SQUARE);
    assert!(snapped.position.x.is_finite() && snapped.position.z.is_finite());
    assert_axis_pixels(translation_px(&snapped, 32.0));
    assert!(snapped.forward.approx_eq(pose.forward(), 1e-4));
}
