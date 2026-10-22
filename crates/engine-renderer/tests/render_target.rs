//! Offscreen target descriptions, camera output, and frame-local sampling.
//!
//! These tests call the production store and `extract_scene`. They do not simulate a scheduler.
//! A sprite samples a target only after an earlier camera in this extract has written it.

use engine_assets::{AlphaMode, AssetHandle, AssetManager, MaterialAsset, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Mat4, Rect, Transform, Vec2, Vec3};
use engine_renderer::{
    active_surface_cameras, camera_view_of, check_render_target_size, extract_scene, top_surface_camera, Camera,
    CameraClear, CameraOutput, CameraPass, CameraProjection, RenderScene, RenderTargetError, RenderTargetId,
    RenderTargetStore, Sprite, SpriteInstance, SpriteShading, ViewportRect, RENDER_TARGET_MAX_EXTENT,
};
use engine_world::{propagate_transforms, World};

const SURFACE: Vec2 = Vec2::new(200.0, 50.0);

fn assert_copy<T: Copy>() {}
fn assert_hash_eq<T: Eq + std::hash::Hash>() {}

fn prepare(world: &mut World) {
    propagate_transforms(world);
}

fn extract(world: &mut World, assets: &AssetManager) -> RenderScene {
    prepare(world);
    extract_scene(world, assets, SURFACE).expect("drawable cameras")
}

fn assets() -> AssetManager {
    let mut assets = AssetManager::in_memory();
    let texture = assets.insert("tex", Texture::solid(4, 4, [0, 255, 0, 255])).unwrap();
    assets
        .insert(
            "atlas",
            SpriteAsset::from_atlas_rect(texture, Vec2::splat(4.0), 0, 0, 1, 1)
                .with_pixels_per_unit(1.0)
                .with_pivot(Vec2::splat(0.25)),
        )
        .unwrap();
    assets.insert("sprite", SpriteAsset::whole_texture(texture, 4, 4).with_pixels_per_unit(1.0)).unwrap();
    let albedo = assets.insert("albedo", Texture::solid(1, 1, [0, 0, 255, 255])).unwrap();
    let mut material = MaterialAsset::unlit(Color::rgb(0.5, 1.0, 0.25));
    material.albedo = Some(albedo);
    assets.insert("flat", material).unwrap();
    assets.insert("mask", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::mask(0.5).unwrap())).unwrap();
    assets.insert("glass", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::Blend)).unwrap();
    assets
}

fn insert_store(world: &mut World) {
    world.insert_resource(RenderTargetStore::new());
}

fn create(world: &World, width: u32, height: u32) -> RenderTargetId {
    world.resource_mut::<RenderTargetStore>().create(width, height).unwrap()
}

fn pose(x: f32) -> Transform {
    Transform::from_xyz(x, 0.0, -6.0)
}

fn ortho(order: i32, output: CameraOutput) -> Camera {
    Camera { order, output, ..Camera::orthographic(4.0).with_clear_color(Color::BLACK) }
}

fn sprite_at(world: &mut World, x: f32, sprite: Sprite) {
    world.spawn((Transform::from_xyz(x, 0.0, 0.0), sprite));
}

fn flat_sprite(target: Option<RenderTargetId>) -> Sprite {
    let sprite = Sprite::new("atlas")
        .with_region(Rect::new(Vec2::ZERO, Vec2::splat(0.5)))
        .with_size(Vec2::splat(1.0))
        .with_anchor(Vec2::splat(0.5))
        .with_tint(Color::rgb(1.0, 0.5, 1.0))
        .with_material(AssetHandle::from_name("flat"));
    match target {
        Some(id) => sprite.with_render_target(id),
        None => sprite,
    }
}

fn instances(pass: &CameraPass) -> Vec<&SpriteInstance> {
    pass.opaque_sprites
        .instances
        .iter()
        .chain(pass.mask_sprites.instances.iter())
        .chain(pass.cutout_sprites.instances.iter())
        .chain(pass.blended_sprites.instances.iter())
        .collect()
}

fn on_ray(point: Vec3, origin: Vec3, direction: Vec3) -> bool {
    let along = (point - origin).dot(direction);
    let closest = origin + direction * along;
    along > 0.0 && point.approx_eq(closest, 2e-3)
}

#[test]
fn render_target_ids_are_copy_eq_and_not_reused() {
    assert_copy::<RenderTargetId>();
    assert_copy::<Camera>();
    assert_copy::<CameraOutput>();
    assert_hash_eq::<RenderTargetId>();
    assert_hash_eq::<CameraOutput>();

    let mut store = RenderTargetStore::new();
    assert!(store.is_empty());
    let first = store.create(8, 4).unwrap();
    let second = store.create(8, 4).unwrap();
    assert_ne!(first, second);
    assert_eq!(first.raw(), 1);
    assert_eq!(second.raw(), 2);
    assert_eq!(first.to_string(), "render-target-1");
    assert!(store.remove(first));
    assert!(!store.remove(first));
    assert!(store.get(first).is_none());
    let third = store.create(8, 4).unwrap();
    assert_ne!(third, first, "a removed id is not issued again");
    assert!(store.get(second).is_some());
    assert_eq!(store.len(), 2);
}

#[test]
fn sizes_are_validated_without_panicking() {
    assert_eq!(check_render_target_size(2, 3).unwrap(), 24);
    assert_eq!(check_render_target_size(0, 4), Err(RenderTargetError::ZeroDimension { width: 0, height: 4 }));
    assert_eq!(
        check_render_target_size(RENDER_TARGET_MAX_EXTENT + 1, 1),
        Err(RenderTargetError::Extent {
            width: RENDER_TARGET_MAX_EXTENT + 1,
            height: 1,
            limit: RENDER_TARGET_MAX_EXTENT
        })
    );
    assert_eq!(
        check_render_target_size(u32::MAX, u32::MAX),
        Err(RenderTargetError::Allocation { width: u32::MAX, height: u32::MAX })
    );

    let mut store = RenderTargetStore::new();
    assert!(store.create(0, 8).is_err());
    assert!(store.create(RENDER_TARGET_MAX_EXTENT + 1, 2).is_err());
    assert!(store.is_empty());
    let id = store.create(4, 4).unwrap();
    assert_eq!(store.resize(id, 0, 4), Err(RenderTargetError::ZeroDimension { width: 0, height: 4 }));
    assert_eq!(store.get(id).unwrap().width, 4, "a rejected resize leaves the description");
    let missing = store.create(2, 2).unwrap();
    store.remove(missing);
    assert_eq!(store.resize(missing, 3, 3), Err(RenderTargetError::Missing(missing)));
}

#[test]
fn resize_bumps_generation_only_when_the_size_changes() {
    let mut store = RenderTargetStore::new();
    let id = store.create(16, 8).unwrap();
    let created = *store.get(id).unwrap();
    assert_eq!(created.generation, 1);
    store.resize(id, 16, 8).unwrap();
    assert_eq!(store.get(id).unwrap().generation, 1, "an equal size allocates nothing");
    store.resize(id, 32, 8).unwrap();
    let resized = *store.get(id).unwrap();
    assert_eq!(resized.id, id);
    assert_eq!((resized.width, resized.height, resized.generation), (32, 8, 2));
    let other = store.create(4, 4).unwrap();
    let listed = store.descriptions();
    assert_eq!(listed.iter().map(|desc| desc.id).collect::<Vec<_>>(), vec![id, other]);
    assert_eq!(listed[0], resized);
    assert_eq!(store.get(other).unwrap().generation, 1, "another target is left untouched");
}

#[test]
fn a_target_outlives_the_cameras_that_use_it() {
    let assets = assets();
    let mut world = World::new();
    insert_store(&mut world);
    let id = create(&world, 32, 16);
    let camera = world.spawn((ortho(0, CameraOutput::Target(id)), pose(0.0)));
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.targets.len(), 1);
    assert_eq!(scene.sole_pass().output, CameraOutput::Target(id));
    world.despawn(camera).unwrap();
    prepare(&mut world);
    assert!(extract_scene(&world, &assets, SURFACE).is_none());
    assert_eq!(world.resource::<RenderTargetStore>().get(id).unwrap().id, id);
    assert!(world.resource_mut::<RenderTargetStore>().remove(id));
    assert!(world.resource::<RenderTargetStore>().is_empty());
}

#[test]
fn constructors_default_to_the_surface_and_stay_copyable() {
    let perspective = Camera::perspective(60.0);
    let orthographic = Camera::orthographic(4.0);
    let custom = Camera::new(CameraProjection::custom(Mat4::IDENTITY).unwrap());
    assert_eq!(perspective.output, CameraOutput::Surface);
    assert_eq!(orthographic.output, CameraOutput::Surface);
    assert_eq!(custom.output, CameraOutput::Surface);
    let copied = perspective;
    assert_eq!(copied.output, perspective.output);
    let updated = Camera { order: 3, ..orthographic };
    assert_eq!(updated.output, CameraOutput::Surface);
    assert_eq!(updated.order, 3);
    let id = RenderTargetStore::new().create(8, 8).unwrap();
    let targeted = perspective.with_output(CameraOutput::Target(id));
    assert_eq!(targeted.output, CameraOutput::Target(id));
    assert_eq!(perspective.output, CameraOutput::Surface, "with_output consumes a copy");
}

#[test]
fn output_dimensions_drive_aspect_and_viewport_except_for_custom() {
    let assets = assets();
    let mut world = World::new();
    insert_store(&mut world);
    let id = create(&world, 64, 32);
    let matrix = Mat4::orthographic_lh(-3.0, 3.0, -1.0, 1.0, 0.1, 40.0);
    world.spawn((
        Camera {
            order: 0,
            viewport: ViewportRect::new(0.0, 0.0, 0.5, 1.0),
            output: CameraOutput::Target(id),
            ..Camera::perspective(60.0)
        },
        pose(0.0),
    ));
    world.spawn((Camera { order: 1, output: CameraOutput::Target(id), ..Camera::orthographic(8.0) }, pose(0.0)));
    world.spawn((
        Camera { order: 2, output: CameraOutput::Target(id), ..Camera::new(CameraProjection::custom(matrix).unwrap()) },
        pose(0.0),
    ));
    let scene = extract(&mut world, &assets);
    let (perspective, orthographic, custom) = (&scene.passes[0], &scene.passes[1], &scene.passes[2]);
    assert!((perspective.view.aspect() - 1.0).abs() < 1e-4, "half of 64x32 is square");
    assert_eq!(perspective.viewport_px, [0.0, 0.0, 32.0, 32.0]);
    assert!((orthographic.view.aspect() - 2.0).abs() < 1e-4);
    assert_eq!(orthographic.viewport_px, [0.0, 0.0, 64.0, 32.0]);
    assert!(custom.view.proj.approx_eq(&matrix, 0.0));
    let center = orthographic.view.project(Vec3::ZERO).unwrap();
    assert!(center.pixels.x < 64.0 && center.pixels.y < 32.0);
    assert!(center.pixels.x > 16.0, "the target center is not the 200px surface center");

    world.resource_mut::<RenderTargetStore>().resize(id, 32, 64).unwrap();
    let resized = extract(&mut world, &assets);
    assert!(!resized.passes[0].view.proj.approx_eq(&perspective.view.proj, 1e-4));
    assert!(!resized.passes[1].view.proj.approx_eq(&orthographic.view.proj, 1e-4));
    assert!(resized.passes[2].view.proj.approx_eq(&matrix, 0.0));
    assert!((resized.passes[1].view.aspect() - 0.5).abs() < 1e-4);
    assert_eq!(resized.targets[0].generation, 2);
    assert_eq!(resized.passes[0].output, CameraOutput::Target(id));
}

#[test]
fn surface_and_target_cameras_share_one_order() {
    let assets = assets();
    let mut world = World::new();
    insert_store(&mut world);
    let id = create(&world, 40, 20);
    let surface = world.spawn((ortho(2, CameraOutput::Surface), pose(0.0)));
    let target = world.spawn((ortho(-1, CameraOutput::Target(id)), pose(30.0)));
    let tied = world.spawn((ortho(2, CameraOutput::Surface), pose(-30.0)));
    let scene = extract(&mut world, &assets);
    let ids: Vec<u32> = scene.passes.iter().map(|pass| pass.camera_id).collect();
    let (first_tie, second_tie) =
        if surface.id() < tied.id() { (surface.id(), tied.id()) } else { (tied.id(), surface.id()) };
    assert_eq!(ids, vec![target.id(), first_tie, second_tie]);
    assert_eq!(scene.passes[0].output, CameraOutput::Target(id));
    assert!(scene.passes[1..].iter().all(|pass| pass.output == CameraOutput::Surface));
    assert_eq!(scene.passes[0].view.viewport, Vec2::new(40.0, 20.0));
    assert_eq!(scene.passes[1].view.viewport, Vec2::new(200.0, 50.0));
}

#[test]
fn a_missing_output_is_skipped_and_the_surface_camera_still_renders() {
    let assets = assets();
    let mut world = World::new();
    insert_store(&mut world);
    let foreign = RenderTargetStore::new().create(8, 8).unwrap();
    let missing = world.spawn((ortho(0, CameraOutput::Target(foreign)), pose(0.0)));
    let surface = world.spawn((ortho(1, CameraOutput::Surface), pose(0.0)));
    sprite_at(&mut world, 0.0, flat_sprite(None));
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.passes.len(), 1);
    assert_eq!(scene.sole_pass().camera_id, surface.id());
    assert_ne!(scene.sole_pass().camera_id, missing.id());
    assert_eq!(scene.sole_pass().output, CameraOutput::Surface);
    assert_eq!(scene.stats.missing_targets, 1);
    assert_eq!(scene.sole_pass().stats.sprites_visible, 1);

    let mut only = World::new();
    insert_store(&mut only);
    only.spawn((ortho(0, CameraOutput::Target(foreign)), pose(0.0)));
    prepare(&mut only);
    assert!(extract_scene(&only, &assets, SURFACE).is_none());
}

#[test]
fn each_camera_still_extracts_its_own_view() {
    let assets = assets();
    let mut world = World::new();
    insert_store(&mut world);
    let id = create(&world, 64, 64);
    world.spawn((ortho(0, CameraOutput::Target(id)), pose(0.0)));
    world.spawn((ortho(1, CameraOutput::Surface), pose(40.0)));
    sprite_at(&mut world, 0.0, flat_sprite(None));
    sprite_at(&mut world, 40.0, flat_sprite(Some(id)));
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.passes[0].stats.sprites_visible, 1, "the consumer sits outside the offscreen frustum");
    assert_eq!(scene.passes[1].stats.sprites_visible, 1, "the source sprite sits outside the surface frustum");
    assert!(scene.passes[0].opaque_sprites.instances[0].render_target.is_none());
    assert_eq!(scene.passes[1].opaque_sprites.instances[0].render_target, Some(id));
    assert!(!scene.passes[1].opaque_sprites.instances[0].unavailable_target);
}

#[test]
fn sampling_follows_camera_order_and_resets_every_frame() {
    let assets = assets();
    let mut world = World::new();
    insert_store(&mut world);
    let target = create(&world, 32, 32);
    let producer = world.spawn((ortho(0, CameraOutput::Target(target)), pose(0.0)));
    world.spawn((ortho(1, CameraOutput::Surface), pose(40.0)));
    sprite_at(&mut world, 40.0, flat_sprite(Some(target)));
    let ready = extract(&mut world, &assets);
    let consumer = &ready.passes[1].opaque_sprites.instances[0];
    assert_eq!(consumer.render_target, Some(target));
    assert!(!consumer.unavailable_target);
    assert_eq!(ready.stats.missing_targets, 0);

    let mut early = World::new();
    insert_store(&mut early);
    let early_target = create(&early, 32, 32);
    early.spawn((ortho(1, CameraOutput::Target(early_target)), pose(0.0)));
    early.spawn((ortho(0, CameraOutput::Surface), pose(40.0)));
    sprite_at(&mut early, 40.0, flat_sprite(Some(early_target)));
    let early_scene = extract(&mut early, &assets);
    assert_eq!(early_scene.passes[0].output, CameraOutput::Surface, "order is not rearranged to satisfy the sample");
    assert!(early_scene.passes[0].opaque_sprites.instances[0].unavailable_target);
    assert_eq!(early_scene.stats.missing_targets, 1);

    world.despawn(producer).unwrap();
    let stale = extract(&mut world, &assets);
    assert_eq!(stale.passes.len(), 1);
    assert!(stale.sole_pass().opaque_sprites.instances[0].unavailable_target);
    assert_eq!(stale.stats.missing_targets, 1, "the next frame does not keep the previous write");
    assert!(stale.targets.iter().any(|desc| desc.id == target));
}

#[test]
fn self_sampling_is_rejected_even_after_an_earlier_writer() {
    let assets = assets();
    let mut world = World::new();
    insert_store(&mut world);
    let target = create(&world, 32, 32);
    world.spawn((ortho(0, CameraOutput::Target(target)), pose(0.0)));
    world.spawn((ortho(1, CameraOutput::Target(target)), pose(40.0)));
    sprite_at(&mut world, 40.0, flat_sprite(Some(target)));
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.passes[0].output, CameraOutput::Target(target));
    assert_eq!(scene.passes[1].output, CameraOutput::Target(target));
    assert_eq!(scene.passes[0].stats.sprites_visible, 0);
    assert!(scene.passes[1].opaque_sprites.instances[0].unavailable_target);
    assert_eq!(scene.passes[1].opaque_sprites.instances[0].render_target, Some(target));
    assert_eq!(scene.stats.missing_targets, 1);
}

#[test]
fn missing_and_removed_targets_use_the_placeholder_path() {
    let assets = assets();
    let mut world = World::new();
    insert_store(&mut world);
    let removed = create(&world, 16, 16);
    world.resource_mut::<RenderTargetStore>().remove(removed);
    let foreign = RenderTargetStore::new().create(16, 16).unwrap();
    world.spawn((ortho(0, CameraOutput::Surface), pose(0.0)));
    sprite_at(&mut world, 0.0, flat_sprite(Some(removed)).with_tint(Color::rgb(1.0, 0.0, 0.0)));
    sprite_at(&mut world, 0.0, flat_sprite(Some(foreign)).with_tint(Color::rgb(0.0, 0.0, 1.0)));
    let scene = extract(&mut world, &assets);
    let drawn = &scene.sole_pass().opaque_sprites.instances;
    assert_eq!(drawn.len(), 2);
    assert!(drawn.iter().all(|instance| instance.unavailable_target));
    assert_eq!(scene.stats.missing_targets, 2);
    assert!(drawn.iter().all(|instance| instance.uv == [0.0, 0.0, 1.0, 1.0]));
    assert_eq!(scene.sole_pass().opaque_sprites.batches.len(), 1, "placeholder samples share one bind");
}

#[test]
fn an_ordered_chain_succeeds_and_a_cycle_fails_the_forward_edge() {
    let assets = assets();
    let mut chain = World::new();
    insert_store(&mut chain);
    let a = create(&chain, 16, 16);
    let b = create(&chain, 16, 16);
    let c = create(&chain, 16, 16);
    chain.spawn((ortho(0, CameraOutput::Target(a)), pose(0.0)));
    chain.spawn((ortho(1, CameraOutput::Target(b)), pose(40.0)));
    chain.spawn((ortho(2, CameraOutput::Target(c)), pose(80.0)));
    sprite_at(&mut chain, 40.0, flat_sprite(Some(a)));
    sprite_at(&mut chain, 80.0, flat_sprite(Some(b)));
    let chained = extract(&mut chain, &assets);
    assert_eq!(chained.passes.len(), 3);
    assert!(!chained.passes[1].opaque_sprites.instances[0].unavailable_target);
    assert_eq!(chained.passes[1].opaque_sprites.instances[0].render_target, Some(a));
    assert!(!chained.passes[2].opaque_sprites.instances[0].unavailable_target);
    assert_eq!(chained.passes[2].opaque_sprites.instances[0].render_target, Some(b));
    assert_eq!(chained.stats.missing_targets, 0);

    let mut cycle = World::new();
    insert_store(&mut cycle);
    let left = create(&cycle, 16, 16);
    let right = create(&cycle, 16, 16);
    cycle.spawn((ortho(0, CameraOutput::Target(left)), pose(0.0)));
    cycle.spawn((ortho(1, CameraOutput::Target(right)), pose(40.0)));
    sprite_at(&mut cycle, 0.0, flat_sprite(Some(right)).with_tint(Color::rgb(1.0, 0.0, 0.0)));
    sprite_at(&mut cycle, 40.0, flat_sprite(Some(left)).with_tint(Color::rgb(0.0, 0.0, 1.0)));
    let cycled = extract(&mut cycle, &assets);
    assert_eq!((cycled.passes[0].order, cycled.passes[1].order), (0, 1));
    assert!(
        cycled.passes[0].opaque_sprites.instances[0].unavailable_target,
        "the earlier camera cannot see the later target"
    );
    assert!(
        !cycled.passes[1].opaque_sprites.instances[0].unavailable_target,
        "the later camera can see the earlier target"
    );
    assert_eq!(cycled.stats.missing_targets, 1);
}

#[test]
fn several_writers_publish_only_the_writes_that_have_already_run() {
    let assets = assets();
    let mut world = World::new();
    insert_store(&mut world);
    let target = create(&world, 32, 32);
    world.spawn((ortho(0, CameraOutput::Target(target)), pose(0.0)));
    world.spawn((ortho(1, CameraOutput::Surface), pose(40.0)));
    world.spawn((ortho(2, CameraOutput::Target(target)).with_clear(CameraClear::Load), pose(80.0)));
    world.spawn((ortho(3, CameraOutput::Surface), pose(120.0)));
    sprite_at(&mut world, 40.0, flat_sprite(Some(target)).with_tint(Color::rgb(1.0, 0.0, 0.0)));
    sprite_at(&mut world, 120.0, flat_sprite(Some(target)).with_tint(Color::rgb(0.0, 0.0, 1.0)));
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.passes.len(), 4);
    assert_eq!(scene.passes[0].output, CameraOutput::Target(target));
    assert_eq!(scene.passes[2].output, CameraOutput::Target(target));
    assert_eq!(scene.passes[2].clear, CameraClear::Load);
    assert!(!scene.passes[1].opaque_sprites.instances[0].unavailable_target, "between the writers");
    assert!(!scene.passes[3].opaque_sprites.instances[0].unavailable_target, "after both writers");
    assert_eq!(scene.stats.missing_targets, 0);
    assert_eq!(scene.passes.iter().filter(|pass| pass.output == CameraOutput::Target(target)).count(), 2);
}

#[test]
fn a_render_target_source_replaces_the_texture_and_uses_the_full_image() {
    let mut assets = assets();
    let mut world = World::new();
    insert_store(&mut world);
    let target = create(&world, 32, 32);
    world.spawn((ortho(0, CameraOutput::Target(target)), pose(0.0)));
    world.spawn((ortho(1, CameraOutput::Surface), pose(0.0)));
    sprite_at(&mut world, 0.0, flat_sprite(None));
    sprite_at(&mut world, 0.0, flat_sprite(Some(target)).with_flip_x(true));
    let scene = extract(&mut world, &assets);
    let drawn = &scene.passes[1].opaque_sprites.instances;
    let plain = drawn.iter().find(|instance| instance.render_target.is_none()).unwrap();
    let sampled = drawn.iter().find(|instance| instance.render_target == Some(target)).unwrap();
    let albedo = AssetHandle::<Texture>::from_name("albedo");
    assert_eq!(plain.texture, albedo, "without an override, material albedo is still the sample");
    assert_ne!(plain.uv, [0.0, 0.0, 1.0, 1.0], "the atlas region and sprite region still select texels");
    assert_eq!(sampled.texture, albedo, "the asset handle is retained and is not a fabricated target id");
    assert!(!sampled.unavailable_target);
    assert_eq!(sampled.uv, [1.0, 0.0, 0.0, 1.0], "flip applies to the full target, not the atlas rect");
    assert!(sampled.model.approx_eq(&plain.model, 1e-5), "size and pivot stay on the sprite");
    assert_eq!(sampled.tint, [0.5, 0.5, 0.25, 1.0], "base color times sprite tint");
    assert_eq!(sampled.tint, plain.tint);
    assert_eq!(sampled.shading, SpriteShading::Opaque { double_sided: false });
    assert_eq!(sampled.shading, plain.shading);
    let material = AssetHandle::<MaterialAsset>::from_name("flat");
    assert_eq!(sampled.material_revision, assets.revision(material));
    assert_eq!(sampled.material_revision, plain.material_revision);
    assert!(!sampled.lit);
    assert!(!plain.shares_batch(sampled));
    let mut replaced = *assets.get(material).unwrap();
    replaced.base_color = Color::WHITE;
    assets.insert("flat", replaced).unwrap();
    assert_ne!(
        assets.revision(material),
        sampled.material_revision,
        "sampling a target does not own the material revision"
    );
}

#[test]
fn alpha_class_stays_on_the_material_and_matching_targets_batch() {
    let assets = assets();
    let mut world = World::new();
    insert_store(&mut world);
    let red = create(&world, 8, 8);
    let blue = create(&world, 8, 8);
    world.spawn((ortho(0, CameraOutput::Target(red)), pose(-40.0)));
    world.spawn((ortho(1, CameraOutput::Target(blue)), pose(40.0)));
    world.spawn((ortho(2, CameraOutput::Surface), pose(0.0)));
    let mask = Sprite::new("sprite")
        .with_size(Vec2::splat(0.4))
        .with_anchor(Vec2::splat(0.5))
        .with_material(AssetHandle::from_name("mask"))
        .with_render_target(red);
    let glass = Sprite::new("sprite")
        .with_size(Vec2::splat(0.4))
        .with_anchor(Vec2::splat(0.5))
        .with_material(AssetHandle::from_name("glass"))
        .with_render_target(red);
    sprite_at(&mut world, 0.0, mask);
    sprite_at(&mut world, 0.2, mask);
    sprite_at(&mut world, -0.2, glass);
    sprite_at(&mut world, 0.4, glass.with_render_target(blue));
    let scene = extract(&mut world, &assets);
    let surface = scene.passes.iter().find(|pass| pass.output == CameraOutput::Surface).unwrap();
    assert_eq!(surface.mask_sprites.instances.len(), 2);
    assert!(surface.mask_sprites.instances.iter().all(|instance| !instance.unavailable_target));
    assert_eq!(surface.mask_sprites.batches.len(), 1);
    assert_eq!(surface.mask_sprites.batches[0].range, 0..2);
    assert_eq!(surface.mask_sprites.batches[0].render_target, Some(red));
    assert_eq!(surface.blended_sprites.instances.len(), 2);
    assert_eq!(surface.blended_sprites.batches.len(), 2, "different targets do not share a bind");
    assert!(matches!(surface.mask_sprites.instances[0].shading, SpriteShading::Mask { .. }));
    assert!(matches!(surface.blended_sprites.instances[0].shading, SpriteShading::Blend { .. }));
}

#[test]
fn an_unassigned_sprite_keeps_its_asset_texture_until_a_target_is_set() {
    let assets = assets();
    let mut world = World::new();
    insert_store(&mut world);
    let target = create(&world, 8, 8);
    world.spawn((ortho(0, CameraOutput::Target(target)), pose(40.0)));
    world.spawn((ortho(1, CameraOutput::Surface), pose(0.0)));
    sprite_at(
        &mut world,
        0.0,
        Sprite::new("atlas")
            .with_size(Vec2::splat(1.0))
            .with_anchor(Vec2::splat(0.25))
            .with_tint(Color::rgb(0.0, 1.0, 0.0)),
    );
    sprite_at(
        &mut world,
        0.0,
        Sprite::new("atlas")
            .with_size(Vec2::splat(1.0))
            .with_anchor(Vec2::splat(0.25))
            .with_tint(Color::rgb(0.0, 1.0, 0.0))
            .with_render_target(target),
    );
    let scene = extract(&mut world, &assets);
    let drawn = instances(&scene.passes[1]);
    let plain = drawn.iter().find(|instance| instance.render_target.is_none()).unwrap();
    let sampled = drawn.iter().find(|instance| instance.render_target.is_some()).unwrap();
    assert_eq!(plain.texture, AssetHandle::<Texture>::from_name("tex"));
    assert_eq!(plain.shading, SpriteShading::LegacyBlend);
    assert_eq!(plain.tint, [0.0, 1.0, 0.0, 1.0]);
    assert_ne!(plain.uv, [0.0, 0.0, 1.0, 1.0]);
    assert_eq!(sampled.uv, [0.0, 0.0, 1.0, 1.0]);
    assert_eq!(sampled.tint, plain.tint);
    assert_eq!(sampled.shading, plain.shading);
    assert!(sampled.model.approx_eq(&plain.model, 1e-5));
    assert!(!sampled.unavailable_target);
}

#[test]
fn picking_stays_on_the_surface_and_an_offscreen_view_is_target_local() {
    let mut world = World::new();
    insert_store(&mut world);
    let id = create(&world, 80, 40);
    let surface = world.spawn((ortho(0, CameraOutput::Surface), pose(0.0)));
    let offscreen =
        world.spawn((Camera { order: 5, output: CameraOutput::Target(id), ..Camera::orthographic(4.0) }, pose(0.0)));
    prepare(&mut world);
    let hit = top_surface_camera(&world, Vec2::new(20.0, 10.0), SURFACE).unwrap();
    assert_eq!(hit.camera_id, surface.id());
    assert_ne!(hit.camera_id, offscreen.id());
    let ids: Vec<u32> = active_surface_cameras(&world, SURFACE).iter().map(|camera| camera.camera_id).collect();
    assert_eq!(ids, vec![surface.id()]);

    let mut only_offscreen = World::new();
    insert_store(&mut only_offscreen);
    let offscreen_id = create(&only_offscreen, 80, 40);
    let entity = only_offscreen.spawn((ortho(0, CameraOutput::Target(offscreen_id)), pose(0.0)));
    prepare(&mut only_offscreen);
    assert!(top_surface_camera(&only_offscreen, Vec2::new(10.0, 10.0), SURFACE).is_none());
    let view = camera_view_of(&only_offscreen, entity.id(), SURFACE).unwrap();
    assert_eq!(view.viewport, Vec2::new(80.0, 40.0));
    assert!((view.aspect() - 2.0).abs() < 1e-4);
    assert!((SURFACE.x / SURFACE.y - view.aspect()).abs() > 1.0);
    let point = Vec3::new(0.3, -0.2, 0.0);
    let pixels = view.project(point).unwrap().pixels;
    assert!(pixels.x >= 0.0 && pixels.x <= 80.0 && pixels.y >= 0.0 && pixels.y <= 40.0);
    let (origin, direction) = view.ray_from_pixels(pixels).unwrap();
    assert!(on_ray(point, origin, direction));

    world.resource_mut::<RenderTargetStore>().remove(id);
    prepare(&mut world);
    assert!(camera_view_of(&world, offscreen.id(), SURFACE).is_none());
}
