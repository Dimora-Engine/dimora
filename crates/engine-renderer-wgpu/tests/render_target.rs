//! Camera-to-texture on the real sprite pipeline.
//!
//! Pixel checks read the surface from `render_to_rgba` and, where a test names a target, the
//! offscreen image from `read_render_target`. Nothing here copies those pixels back into a
//! texture asset. The linear `Rgba8Unorm` format matches the other GPU tests.

use engine_assets::{AlphaMode, AssetHandle, AssetManager, ColorSpace, MaterialAsset, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Mat4, Transform, Vec2, Vec3};
use engine_renderer::*;
use engine_renderer_wgpu::{GpuContext, SceneRenderer};
use engine_world::{propagate_transforms, World};

const W: u32 = 128;
const H: u32 = 128;

fn gpu() -> Option<GpuContext> {
    match GpuContext::headless() {
        Ok(gpu) => {
            let info = gpu.adapter.get_info();
            eprintln!("GPU test adapter: {} ({:?}, {:?})", info.name, info.backend, info.device_type);
            Some(gpu)
        }
        Err(error) => {
            if std::env::var("DIMORA_REQUIRE_GPU").ok().as_deref() == Some("1") {
                panic!("DIMORA_REQUIRE_GPU=1 but the GPU test adapter is unavailable: {error}");
            }
            eprintln!("skipping GPU test: {error}");
            None
        }
    }
}

fn renderer(gpu: &GpuContext) -> SceneRenderer {
    SceneRenderer::new(gpu, wgpu::TextureFormat::Rgba8Unorm)
}

fn px(img: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * width + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

fn expect_px(img: &[u8], width: u32, x: u32, y: u32, expected: [u8; 4], tolerance: i16, what: &str) {
    let pixel = px(img, width, x, y);
    let close = pixel.iter().zip(expected).all(|(got, want)| (*got as i16 - want as i16).abs() <= tolerance);
    assert!(close, "{what} at ({x},{y}) is {pixel:?}, expected {expected:?} ±{tolerance}");
}

fn expect_world(img: &[u8], width: u32, view: &CameraView, point: Vec3, expected: [u8; 4], what: &str) {
    let screen = view.project(point).unwrap_or_else(|| panic!("{what}: does not project"));
    assert!(screen.is_visible(), "{what}: outside the view");
    let x = (view.viewport_origin.x + screen.pixels.x).floor() as u32;
    let y = (view.viewport_origin.y + screen.pixels.y).floor() as u32;
    expect_px(img, width, x, y, expected, 3, what);
}

fn linear(rgba: [u8; 4]) -> Texture {
    Texture::solid(1, 1, rgba).with_color_space(ColorSpace::Linear)
}

fn insert_sprite(assets: &mut AssetManager, name: &str, rgba: [u8; 4]) {
    let texture = assets.insert(&format!("{name}-tex"), linear(rgba)).unwrap();
    assets.insert(name, SpriteAsset::whole_texture(texture, 1, 1).with_pixels_per_unit(1.0)).unwrap();
}

fn world() -> World {
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.insert_resource(RenderTargetStore::new());
    world
}

fn create(world: &World, width: u32, height: u32) -> RenderTargetId {
    world.resource_mut::<RenderTargetStore>().create(width, height).unwrap()
}

fn ortho(order: i32, output: CameraOutput, clear: Color) -> Camera {
    Camera { order, output, ..Camera::orthographic(4.0).with_clear_color(clear) }
}

fn look(x: f32) -> Transform {
    Transform::from_xyz(x, 0.0, -4.0)
}

fn at(x: f32) -> Transform {
    Transform::from_xyz(x, 0.0, 0.0)
}

fn quad(name: &str, material: AssetHandle<MaterialAsset>, size: f32) -> Sprite {
    Sprite::new(name).with_size(Vec2::splat(size)).with_anchor(Vec2::splat(0.5)).with_material(material).fixed()
}

fn render(
    gpu: &GpuContext,
    renderer: &mut SceneRenderer,
    world: &mut World,
    assets: &AssetManager,
) -> (Vec<u8>, RenderScene) {
    propagate_transforms(world);
    let scene = extract_scene(world, assets, Vec2::new(W as f32, H as f32)).unwrap();
    let image = renderer.render_to_rgba(gpu, &scene, assets, W, H).unwrap();
    (image, scene)
}

fn target_pixels(gpu: &GpuContext, renderer: &SceneRenderer, id: RenderTargetId, width: u32, height: u32) -> Vec<u8> {
    let image = renderer.read_render_target(gpu, id).unwrap_or_else(|| panic!("missing GPU image for {id}"));
    assert_eq!(image.len(), (width * height * 4) as usize, "{id} is {width}x{height}");
    image
}

#[test]
fn camera_to_texture_displays_the_offscreen_image_on_the_surface() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    let texture = assets.insert("white-tex", linear([255, 255, 255, 255])).unwrap();
    assets
        .insert("atlas", SpriteAsset::from_atlas_rect(texture, Vec2::splat(4.0), 0, 0, 1, 1).with_pixels_per_unit(1.0))
        .unwrap();
    let red = assets.insert("red", MaterialAsset::unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let white = assets.insert("white", MaterialAsset::unlit(Color::WHITE)).unwrap();

    let mut world = world();
    let target = create(&world, 64, 64);
    world.spawn((ortho(0, CameraOutput::Target(target), Color::rgb(0.0, 0.0, 1.0)), look(0.0)));
    world.spawn((ortho(1, CameraOutput::Surface, Color::rgb(0.0, 1.0, 0.0)), look(40.0)));
    world.spawn((at(0.0), quad("atlas", red, 2.0)));
    world.spawn((at(40.0), quad("atlas", white, 4.0).with_render_target(target)));

    let before = renderer.resource_stats();
    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    let offscreen = target_pixels(&gpu, &renderer, target, 64, 64);
    assert_eq!(scene.passes.len(), 2);
    assert_eq!(scene.passes[0].output, CameraOutput::Target(target));
    assert!((scene.passes[0].view.aspect() - 1.0).abs() < 1e-4);
    assert_eq!(scene.passes[0].opaque_sprites.instances[0].render_target, None);
    let consumer = &scene.passes[1].opaque_sprites.instances[0];
    assert_eq!(consumer.render_target, Some(target));
    assert!(!consumer.unavailable_target);
    assert_eq!(consumer.uv, [0.0, 0.0, 1.0, 1.0], "the atlas corner is not the sample rect");

    expect_px(&offscreen, 64, 32, 32, [255, 0, 0, 255], 2, "target center is camera A's sprite");
    expect_px(&offscreen, 64, 4, 4, [0, 0, 255, 255], 2, "target corner keeps camera A's clear");
    expect_px(&image, W, 64, 64, [255, 0, 0, 255], 2, "surface center samples the target, not the white atlas texel");
    expect_px(
        &image,
        W,
        8,
        8,
        [0, 0, 255, 255],
        2,
        "surface corner samples the target clear, not the green surface clear",
    );

    let stats = renderer.stats();
    assert_eq!((stats.camera_passes, stats.surface_passes, stats.offscreen_passes), (2, 1, 1));
    assert_eq!(stats.world_draw_calls, 2);
    assert_eq!(stats.clear_draw_calls, 0, "each destination's first camera uses the attachment clear");
    assert_eq!(stats.submitted_instances, 2);
    let gpu_stats = renderer.resource_stats();
    assert_eq!(gpu_stats.render_target_creates - before.render_target_creates, 1);
    assert_eq!(gpu_stats.render_target_reallocs, 0);
    assert_eq!(gpu_stats.texture_creates - before.texture_creates, 1, "the target image is not a texture upload");
    assert_eq!(gpu_stats.texture_writes - before.texture_writes, 1);
    assert_eq!(gpu_stats.sampler_creates, 1, "the target reuses the default sampler");
    assert_eq!(
        gpu_stats.bind_group_creates - before.bind_group_creates,
        2,
        "one asset bind group and one target bind group"
    );

    let again = renderer.resource_stats();
    let _ = render(&gpu, &mut renderer, &mut world, &assets);
    let after = renderer.resource_stats();
    assert_eq!(after.render_target_creates, again.render_target_creates);
    assert_eq!(after.render_target_reallocs, again.render_target_reallocs);
    assert_eq!(after.texture_creates, again.texture_creates);
    assert_eq!(after.texture_writes, again.texture_writes);
    assert_eq!(after.bind_group_creates, again.bind_group_creates);
    expect_px(
        &renderer.read_render_target(&gpu, target).unwrap(),
        64,
        32,
        32,
        [255, 0, 0, 255],
        2,
        "the retained image",
    );
}

#[test]
fn resize_replaces_the_sampled_view_and_leaves_the_other_target() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "sprite", [255, 255, 255, 255]);
    let red = assets.insert("red", MaterialAsset::unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let white = assets.insert("white-mat", MaterialAsset::unlit(Color::WHITE)).unwrap();

    let mut world = world();
    let resized = create(&world, 64, 64);
    let steady = create(&world, 32, 32);
    world.spawn((ortho(0, CameraOutput::Target(resized), Color::rgb(0.0, 0.0, 1.0)), look(0.0)));
    world.spawn((ortho(1, CameraOutput::Target(steady), Color::rgb(1.0, 1.0, 0.0)), look(40.0)));
    world.spawn((ortho(2, CameraOutput::Surface, Color::rgb(0.0, 1.0, 0.0)), look(80.0)));
    world.spawn((at(0.0), quad("sprite", red, 2.0)));
    world.spawn((at(80.0), quad("sprite", white, 4.0).with_render_target(resized)));

    let (image, _) = render(&gpu, &mut renderer, &mut world, &assets);
    expect_px(&image, W, 51, 64, [255, 0, 0, 255], 2, "UV 0.4 of the square target is inside the red sprite");
    expect_px(&target_pixels(&gpu, &renderer, steady, 32, 32), 32, 8, 8, [255, 255, 0, 255], 2, "the other target");
    let created = renderer.resource_stats();
    assert_eq!(created.render_target_creates, 2);
    assert_eq!(created.render_target_reallocs, 0);

    world.resource_mut::<RenderTargetStore>().resize(resized, 128, 32).unwrap();
    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    let wide = scene.passes.iter().find(|pass| pass.output == CameraOutput::Target(resized)).unwrap();
    assert!((wide.view.aspect() - 4.0).abs() < 1e-4);
    assert_eq!(scene.targets.iter().find(|desc| desc.id == resized).unwrap().generation, 2);
    assert_eq!(scene.targets.iter().find(|desc| desc.id == steady).unwrap().generation, 1);
    let wide_image = target_pixels(&gpu, &renderer, resized, 128, 32);
    expect_px(&wide_image, 128, 64, 16, [255, 0, 0, 255], 2, "the new image center is still the sprite");
    expect_px(&image, W, 51, 64, [0, 0, 255, 255], 2, "UV 0.4 of the wide target is outside the sprite");
    expect_px(&image, W, 64, 64, [255, 0, 0, 255], 2, "the new view is the one sampled");
    expect_px(&target_pixels(&gpu, &renderer, steady, 32, 32), 32, 8, 8, [255, 255, 0, 255], 2, "unchanged target");
    let after = renderer.resource_stats();
    assert_eq!(after.render_target_creates, 2);
    assert_eq!(after.render_target_reallocs, 1);
    assert_eq!(after.bind_group_creates, created.bind_group_creates + 1);
    assert_eq!(after.texture_writes, created.texture_writes);
}

#[test]
fn perspective_orthographic_and_custom_projections_use_the_target() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "sprite", [255, 255, 255, 255]);
    let red = assets.insert("red", MaterialAsset::unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();

    let matrix = Mat4::orthographic_lh(-2.0, 2.0, -2.0, 2.0, 0.1, 40.0);
    let mut world = world();
    let perspective_id = create(&world, 64, 32);
    let ortho_id = create(&world, 64, 32);
    let custom_id = create(&world, 48, 48);
    world.spawn((
        Camera {
            order: 0,
            output: CameraOutput::Target(perspective_id),
            ..Camera::perspective(60.0).with_clear_color(Color::rgb(0.0, 0.0, 1.0))
        },
        look(0.0),
    ));
    world.spawn((
        Camera {
            order: 1,
            output: CameraOutput::Target(ortho_id),
            ..Camera::orthographic(4.0).with_clear_color(Color::rgb(0.0, 0.0, 1.0))
        },
        look(0.0),
    ));
    world.spawn((
        Camera {
            order: 2,
            output: CameraOutput::Target(custom_id),
            ..Camera::new(CameraProjection::custom(matrix).unwrap()).with_clear_color(Color::rgb(0.0, 0.0, 1.0))
        },
        look(0.0),
    ));
    world.spawn((at(0.0), quad("sprite", red, 1.0)));
    let (_, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    let (perspective, orthographic, custom) = (&scene.passes[0], &scene.passes[1], &scene.passes[2]);
    assert!((perspective.view.aspect() - 2.0).abs() < 1e-4);
    assert!((orthographic.view.aspect() - 2.0).abs() < 1e-4);
    assert!(custom.view.proj.approx_eq(&matrix, 0.0));
    assert_ne!(perspective.view.proj, orthographic.view.proj);
    let perspective_image = target_pixels(&gpu, &renderer, perspective_id, 64, 32);
    let ortho_image = target_pixels(&gpu, &renderer, ortho_id, 64, 32);
    let custom_image = target_pixels(&gpu, &renderer, custom_id, 48, 48);
    expect_world(&perspective_image, 64, &perspective.view, Vec3::ZERO, [255, 0, 0, 255], "perspective sprite");
    expect_world(
        &perspective_image,
        64,
        &perspective.view,
        Vec3::new(1.2, 0.0, 0.0),
        [0, 0, 255, 255],
        "perspective clear beside the sprite",
    );
    expect_world(&ortho_image, 64, &orthographic.view, Vec3::ZERO, [255, 0, 0, 255], "orthographic sprite");
    expect_world(
        &ortho_image,
        64,
        &orthographic.view,
        Vec3::new(1.2, 0.0, 0.0),
        [0, 0, 255, 255],
        "orthographic clear beside the sprite",
    );
    expect_world(&custom_image, 48, &custom.view, Vec3::ZERO, [255, 0, 0, 255], "custom sprite");

    world.resource_mut::<RenderTargetStore>().resize(perspective_id, 32, 64).unwrap();
    world.resource_mut::<RenderTargetStore>().resize(custom_id, 80, 20).unwrap();
    let (_, resized) = render(&gpu, &mut renderer, &mut world, &assets);
    assert!(!resized.passes[0].view.proj.approx_eq(&perspective.view.proj, 1e-4));
    assert!((resized.passes[0].view.aspect() - 0.5).abs() < 1e-4);
    assert!(resized.passes[2].view.proj.approx_eq(&matrix, 0.0));
    let next = target_pixels(&gpu, &renderer, perspective_id, 32, 64);
    let custom_next = target_pixels(&gpu, &renderer, custom_id, 80, 20);
    expect_world(&next, 32, &resized.passes[0].view, Vec3::ZERO, [255, 0, 0, 255], "resized perspective sprite");
    expect_world(&custom_next, 80, &resized.passes[2].view, Vec3::ZERO, [255, 0, 0, 255], "resized custom sprite");
    assert_eq!(renderer.resource_stats().render_target_reallocs, 2);
}

#[test]
fn materials_apply_tint_alpha_class_and_depth_to_a_sampled_target() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "sprite", [255, 255, 255, 255]);
    insert_sprite(&mut assets, "hole", [255, 255, 255, 0]);
    let tinted = assets.insert("green", MaterialAsset::unlit(Color::rgb(0.0, 1.0, 0.0))).unwrap();
    let mask = assets
        .insert("mask", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::mask(0.5).unwrap()))
        .unwrap();
    let glass = assets
        .insert("glass", MaterialAsset::unlit(Color::rgba(1.0, 1.0, 0.0, 0.5)).with_alpha_mode(AlphaMode::Blend))
        .unwrap();
    let blue = assets
        .insert("blue", MaterialAsset::unlit(Color::rgb(0.0, 0.0, 1.0)).with_alpha_mode(AlphaMode::Blend))
        .unwrap();

    let mut legacy = world();
    let target = create(&legacy, 64, 64);
    legacy.spawn((ortho(0, CameraOutput::Target(target), Color::WHITE), look(40.0)));
    legacy.spawn((ortho(1, CameraOutput::Surface, Color::rgb(0.0, 0.0, 1.0)), look(0.0)));
    legacy.spawn((
        at(0.0),
        Sprite::new("sprite")
            .with_size(Vec2::splat(4.0))
            .with_anchor(Vec2::splat(0.5))
            .with_tint(Color::rgb(1.0, 0.0, 0.0))
            .unlit()
            .fixed()
            .with_render_target(target),
    ));
    let (image, scene) = render(&gpu, &mut renderer, &mut legacy, &assets);
    assert!(matches!(scene.passes[1].blended_sprites.instances[0].shading, SpriteShading::LegacyBlend));
    expect_px(&image, W, 64, 64, [255, 0, 0, 255], 2, "unassigned tint multiplies the white target");

    let mut opaque = world();
    let target = create(&opaque, 64, 64);
    opaque.spawn((ortho(0, CameraOutput::Target(target), Color::WHITE), look(40.0)));
    opaque.spawn((ortho(1, CameraOutput::Surface, Color::rgb(1.0, 0.0, 0.0)), look(0.0)));
    opaque
        .spawn((at(0.0), quad("sprite", tinted, 1.0).with_tint(Color::rgb(1.0, 0.5, 1.0)).with_render_target(target)));
    opaque.spawn((Transform::from_xyz(0.0, 0.0, 2.0), quad("sprite", glass, 4.0)));
    let (image, scene) = render(&gpu, &mut renderer, &mut opaque, &assets);
    assert!(matches!(scene.passes[1].opaque_sprites.instances[0].shading, SpriteShading::Opaque { .. }));
    expect_px(&image, W, 64, 64, [0, 128, 0, 255], 3, "base color times tint, and the farther blend is rejected");
    expect_px(&image, W, 8, 8, [255, 128, 0, 255], 3, "the blend still covers pixels the opaque sprite missed");

    let mut masked = world();
    let target = create(&masked, 64, 64);
    masked.spawn((ortho(0, CameraOutput::Target(target), Color::WHITE), look(40.0)));
    masked.spawn((ortho(1, CameraOutput::Surface, Color::rgb(1.0, 0.0, 0.0)), look(0.0)));
    masked.spawn((at(0.0), quad("sprite", mask, 1.0).with_render_target(target)));
    masked.spawn((Transform::from_xyz(1.2, 0.0, 0.0), quad("hole", mask, 0.6)));
    masked.spawn((Transform::from_xyz(0.0, 0.0, 2.0), quad("sprite", glass, 4.0)));
    let (image, scene) = render(&gpu, &mut renderer, &mut masked, &assets);
    let view = &scene.passes[1].view;
    expect_world(
        &image,
        W,
        view,
        Vec3::ZERO,
        [255, 255, 255, 255],
        "alpha 1 from the target passes the mask and writes depth",
    );
    expect_world(
        &image,
        W,
        view,
        Vec3::new(1.2, 0.0, 0.0),
        [255, 128, 0, 255],
        "alpha 0 is discarded and does not block the blend",
    );

    let mut blended = world();
    let target = create(&blended, 64, 64);
    blended.spawn((ortho(0, CameraOutput::Target(target), Color::WHITE), look(40.0)));
    blended.spawn((ortho(1, CameraOutput::Surface, Color::rgb(1.0, 0.0, 0.0)), look(0.0)));
    blended.spawn((at(0.0), quad("sprite", glass, 4.0).with_render_target(target), RenderOrder::new(0, 0)));
    blended.spawn((Transform::from_xyz(0.0, 0.0, 2.0), quad("sprite", blue, 1.0), RenderOrder::new(1, 0)));
    let (image, _) = render(&gpu, &mut renderer, &mut blended, &assets);
    expect_px(
        &image,
        W,
        64,
        64,
        [0, 0, 255, 255],
        2,
        "the blend sample does not write depth, so the later layer draws",
    );
    expect_px(&image, W, 8, 8, [255, 128, 0, 255], 3, "yellow 0.5 from the white target over the red clear");
}

#[test]
fn invalid_dependencies_sample_magenta_and_a_cycle_is_ordered() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "sprite", [255, 255, 255, 255]);
    let white = assets.insert("white-mat", MaterialAsset::unlit(Color::WHITE)).unwrap();

    let mut forward = world();
    let target = create(&forward, 64, 64);
    forward.spawn((ortho(1, CameraOutput::Target(target), Color::rgb(1.0, 0.0, 0.0)), look(40.0)));
    forward.spawn((ortho(0, CameraOutput::Surface, Color::rgb(0.0, 0.0, 1.0)), look(0.0)));
    forward.spawn((at(0.0), quad("sprite", white, 4.0).with_render_target(target)));
    let (image, scene) = render(&gpu, &mut renderer, &mut forward, &assets);
    assert!(scene.passes[0].opaque_sprites.instances[0].unavailable_target);
    assert_eq!(scene.passes[0].output, CameraOutput::Surface);
    expect_px(&image, W, 64, 64, [255, 0, 255, 255], 2, "a forward reference is the placeholder");
    expect_px(
        &target_pixels(&gpu, &renderer, target, 64, 64),
        64,
        16,
        16,
        [255, 0, 0, 255],
        2,
        "the later writer still fills its target",
    );

    let mut itself = world();
    let target = create(&itself, 64, 64);
    itself.spawn((ortho(0, CameraOutput::Target(target), Color::rgb(0.0, 0.0, 1.0)), look(0.0)));
    itself.spawn((at(0.0), quad("sprite", white, 2.0).with_render_target(target)));
    let (_, scene) = render(&gpu, &mut renderer, &mut itself, &assets);
    assert!(scene.sole_pass().opaque_sprites.instances[0].unavailable_target);
    let image = target_pixels(&gpu, &renderer, target, 64, 64);
    expect_px(&image, 64, 32, 32, [255, 0, 255, 255], 2, "a camera does not sample the target it is writing");
    expect_px(&image, 64, 4, 4, [0, 0, 255, 255], 2, "the rest of that target is its own clear");

    let mut missing = world();
    let foreign = RenderTargetStore::new().create(16, 16).unwrap();
    missing.spawn((ortho(0, CameraOutput::Surface, Color::rgb(0.0, 0.0, 1.0)), look(0.0)));
    missing.spawn((at(0.0), quad("sprite", white, 4.0).with_render_target(foreign)));
    let (image, _) = render(&gpu, &mut renderer, &mut missing, &assets);
    expect_px(&image, W, 64, 64, [255, 0, 255, 255], 2, "a missing target is the placeholder");

    let mut removed = world();
    let target = create(&removed, 64, 64);
    let producer = removed.spawn((ortho(0, CameraOutput::Target(target), Color::rgb(1.0, 0.0, 0.0)), look(40.0)));
    removed.spawn((ortho(1, CameraOutput::Surface, Color::rgb(0.0, 0.0, 1.0)), look(0.0)));
    removed.spawn((at(0.0), quad("sprite", white, 4.0).with_render_target(target)));
    let (image, _) = render(&gpu, &mut renderer, &mut removed, &assets);
    expect_px(&image, W, 64, 64, [255, 0, 0, 255], 2, "the target is visible before removal");
    removed.despawn(producer).unwrap();
    removed.resource_mut::<RenderTargetStore>().remove(target);
    let (image, scene) = render(&gpu, &mut renderer, &mut removed, &assets);
    assert!(scene.targets.iter().all(|desc| desc.id != target));
    assert!(scene.sole_pass().opaque_sprites.instances[0].unavailable_target);
    expect_px(&image, W, 64, 64, [255, 0, 255, 255], 2, "a removed target does not keep sampling");
    assert!(renderer.read_render_target(&gpu, target).is_none());

    let mut cycle = world();
    let left = create(&cycle, 64, 64);
    let right = create(&cycle, 64, 64);
    cycle.spawn((ortho(0, CameraOutput::Target(left), Color::rgb(0.0, 0.0, 1.0)), look(0.0)));
    cycle.spawn((ortho(1, CameraOutput::Target(right), Color::rgb(0.0, 1.0, 0.0)), look(40.0)));
    cycle.spawn((at(0.0), quad("sprite", white, 2.0).with_render_target(right)));
    cycle.spawn((at(40.0), quad("sprite", white, 4.0).with_render_target(left)));
    let (_, scene) = render(&gpu, &mut renderer, &mut cycle, &assets);
    assert!(scene.passes[0].opaque_sprites.instances[0].unavailable_target);
    assert!(!scene.passes[1].opaque_sprites.instances[0].unavailable_target);
    let left_image = target_pixels(&gpu, &renderer, left, 64, 64);
    let right_image = target_pixels(&gpu, &renderer, right, 64, 64);
    expect_px(&left_image, 64, 32, 32, [255, 0, 255, 255], 2, "the earlier edge of the cycle is the placeholder");
    expect_px(&left_image, 64, 4, 4, [0, 0, 255, 255], 2, "the earlier target keeps its clear");
    expect_px(&right_image, 64, 32, 32, [255, 0, 255, 255], 2, "the later camera samples the earlier target");
    expect_px(&right_image, 64, 4, 4, [0, 0, 255, 255], 2, "that sample includes the earlier clear, not the later one");
}

#[test]
fn a_frame_without_a_producer_does_not_sample_the_previous_image() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "sprite", [255, 255, 255, 255]);
    let white = assets.insert("white-mat", MaterialAsset::unlit(Color::WHITE)).unwrap();
    let mut world = world();
    let target = create(&world, 64, 64);
    let producer = world.spawn((ortho(0, CameraOutput::Target(target), Color::rgb(1.0, 0.0, 0.0)), look(40.0)));
    world.spawn((ortho(1, CameraOutput::Surface, Color::rgb(0.0, 0.0, 1.0)), look(0.0)));
    world.spawn((at(0.0), quad("sprite", white, 4.0).with_render_target(target)));
    let (image, _) = render(&gpu, &mut renderer, &mut world, &assets);
    expect_px(&image, W, 64, 64, [255, 0, 0, 255], 2, "the first frame samples the write");

    world.despawn(producer).unwrap();
    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    assert!(scene.sole_pass().opaque_sprites.instances[0].unavailable_target);
    expect_px(&image, W, 64, 64, [255, 0, 255, 255], 2, "the next frame does not show the stale target");
    expect_px(
        &target_pixels(&gpu, &renderer, target, 64, 64),
        64,
        16,
        16,
        [255, 0, 0, 255],
        2,
        "the GPU image is still the previous write",
    );
}

#[test]
fn two_writers_compose_on_one_target_and_consumers_see_completed_writes() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "sprite", [255, 255, 255, 255]);
    let white = assets.insert("white-mat", MaterialAsset::unlit(Color::WHITE)).unwrap();
    let green = assets.insert("green", MaterialAsset::unlit(Color::rgb(0.0, 1.0, 0.0))).unwrap();
    let red = assets.insert("red", MaterialAsset::unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();

    let mut colors = world();
    let target = create(&colors, 128, 128);
    colors.spawn((ortho(0, CameraOutput::Target(target), Color::rgb(1.0, 0.0, 0.0)), look(0.0)));
    colors.spawn((
        Camera {
            order: 1,
            viewport: ViewportRect::new(0.5, 0.0, 0.5, 1.0),
            output: CameraOutput::Target(target),
            ..Camera::orthographic(4.0).with_clear_color(Color::rgb(0.0, 0.0, 1.0))
        },
        look(0.0),
    ));
    let (_, _) = render(&gpu, &mut renderer, &mut colors, &assets);
    let image = target_pixels(&gpu, &renderer, target, 128, 128);
    expect_px(&image, 128, 16, 64, [255, 0, 0, 255], 2, "left half keeps the first clear");
    expect_px(&image, 128, 63, 64, [255, 0, 0, 255], 2, "pixel before the second viewport");
    expect_px(&image, 128, 64, 64, [0, 0, 255, 255], 2, "Color fills the second viewport");
    expect_px(&image, 128, 100, 16, [0, 0, 255, 255], 2, "the second viewport's corner");
    assert_eq!(renderer.stats().clear_draw_calls, 1);
    assert_eq!(renderer.stats().offscreen_passes, 2);

    let mut overlap = world();
    let target = create(&overlap, 128, 128);
    overlap.spawn((ortho(0, CameraOutput::Target(target), Color::rgb(1.0, 0.0, 0.0)), look(0.0)));
    overlap.spawn((
        Camera {
            order: 1,
            viewport: ViewportRect::new(0.25, 0.25, 0.5, 0.5),
            output: CameraOutput::Target(target),
            ..Camera::orthographic(4.0).with_clear_color(Color::rgb(0.0, 0.0, 1.0))
        },
        look(0.0),
    ));
    let _ = render(&gpu, &mut renderer, &mut overlap, &assets);
    let image = target_pixels(&gpu, &renderer, target, 128, 128);
    expect_px(&image, 128, 8, 8, [255, 0, 0, 255], 2, "outside the overlap");
    expect_px(&image, 128, 64, 64, [0, 0, 255, 255], 2, "the overlapping Color viewport");
    expect_px(&image, 128, 31, 64, [255, 0, 0, 255], 2, "pixel before the overlap");
    expect_px(&image, 128, 32, 64, [0, 0, 255, 255], 2, "first pixel of the overlap");

    let mut loaded = world();
    let target = create(&loaded, 128, 128);
    loaded.spawn((ortho(0, CameraOutput::Target(target), Color::rgb(1.0, 0.0, 0.0)), look(0.0)));
    loaded.spawn((
        Camera {
            order: 1,
            viewport: ViewportRect::new(0.5, 0.0, 0.5, 1.0),
            output: CameraOutput::Target(target),
            ..Camera::orthographic(4.0).with_clear_color(Color::rgb(0.0, 0.0, 1.0)).with_clear(CameraClear::Load)
        },
        look(30.0),
    ));
    loaded.spawn((at(30.0), quad("sprite", green, 1.0)));
    loaded.spawn((ortho(2, CameraOutput::Surface, Color::rgb(1.0, 1.0, 0.0)), look(-40.0)));
    loaded.spawn((at(-40.0), quad("sprite", white, 4.0).with_render_target(target)));
    let (surface, _) = render(&gpu, &mut renderer, &mut loaded, &assets);
    let image = target_pixels(&gpu, &renderer, target, 128, 128);
    expect_px(&image, 128, 16, 64, [255, 0, 0, 255], 2, "Load leaves the left half");
    expect_px(&image, 128, 96, 16, [255, 0, 0, 255], 2, "Load leaves the right corner");
    expect_px(&image, 128, 96, 64, [0, 255, 0, 255], 2, "Load draws the second camera's sprite");
    expect_px(&surface, W, 16, 64, [255, 0, 0, 255], 2, "the consumer after both writers sees the left clear");
    expect_px(&surface, W, 96, 64, [0, 255, 0, 255], 2, "the consumer sees the second sprite");

    let mut depth = world();
    let target = create(&depth, 128, 128);
    depth.spawn((ortho(0, CameraOutput::Target(target), Color::rgb(1.0, 0.0, 0.0)), look(0.0)));
    depth.spawn((at(0.0), quad("sprite", red, 8.0)));
    depth.spawn((
        ortho(1, CameraOutput::Target(target), Color::rgb(0.0, 0.0, 1.0)).with_clear(CameraClear::Load),
        look(30.0),
    ));
    depth.spawn((at(30.0), quad("sprite", green, 2.0)));
    let _ = render(&gpu, &mut renderer, &mut depth, &assets);
    let image = target_pixels(&gpu, &renderer, target, 128, 128);
    expect_px(&image, 128, 64, 64, [0, 255, 0, 255], 2, "the second camera's depth starts clear, so its sprite draws");
    expect_px(&image, 128, 8, 8, [255, 0, 0, 255], 2, "Load keeps the first sprite outside the second one");

    let mut between = world();
    let target = create(&between, 64, 64);
    between.spawn((ortho(0, CameraOutput::Target(target), Color::rgb(1.0, 0.0, 0.0)), look(40.0)));
    between.spawn((ortho(1, CameraOutput::Surface, Color::rgb(0.0, 0.0, 1.0)), look(0.0)));
    between.spawn((ortho(2, CameraOutput::Target(target), Color::rgb(0.0, 1.0, 0.0)), look(40.0)));
    between.spawn((at(0.0), quad("sprite", white, 4.0).with_render_target(target)));
    let (surface, scene) = render(&gpu, &mut renderer, &mut between, &assets);
    assert!(!scene.passes[1].opaque_sprites.instances[0].unavailable_target);
    expect_px(&surface, W, 64, 64, [255, 0, 0, 255], 2, "a consumer between writers sees only the first");
    expect_px(
        &target_pixels(&gpu, &renderer, target, 64, 64),
        64,
        16,
        16,
        [0, 255, 0, 255],
        2,
        "the second writer then replaces the target",
    );
}

#[test]
fn sprites_that_sample_one_target_share_a_draw() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "sprite", [255, 255, 255, 255]);
    let white = assets.insert("white-mat", MaterialAsset::unlit(Color::WHITE)).unwrap();

    let mut same = world();
    let target = create(&same, 32, 32);
    same.spawn((ortho(0, CameraOutput::Target(target), Color::rgb(1.0, 0.0, 0.0)), look(40.0)));
    same.spawn((ortho(1, CameraOutput::Surface, Color::WHITE), look(0.0)));
    same.spawn((at(-0.4), quad("sprite", white, 0.5).with_render_target(target)));
    same.spawn((at(0.4), quad("sprite", white, 0.5).with_render_target(target)));
    let (_, scene) = render(&gpu, &mut renderer, &mut same, &assets);
    let surface = &scene.passes[1];
    assert_eq!(surface.opaque_sprites.batches.len(), 1);
    assert_eq!(surface.opaque_sprites.batches[0].range, 0..2);
    assert_eq!(renderer.stats().world_draw_calls, 1, "the producer only clears; the two samples are one draw");
    assert_eq!(renderer.stats().submitted_instances, 2);
    assert_eq!(renderer.pass_stats()[1].world_draw_calls, 1);

    let mut split = world();
    let left = create(&split, 32, 32);
    let right = create(&split, 32, 32);
    split.spawn((ortho(0, CameraOutput::Target(left), Color::rgb(1.0, 0.0, 0.0)), look(-40.0)));
    split.spawn((ortho(1, CameraOutput::Target(right), Color::rgb(0.0, 0.0, 1.0)), look(40.0)));
    split.spawn((ortho(2, CameraOutput::Surface, Color::WHITE), look(0.0)));
    split.spawn((at(-0.4), quad("sprite", white, 0.5).with_render_target(left)));
    split.spawn((at(0.4), quad("sprite", white, 0.5).with_render_target(right)));
    let (_, scene) = render(&gpu, &mut renderer, &mut split, &assets);
    assert_eq!(scene.passes[2].opaque_sprites.batches.len(), 2);
    assert_eq!(renderer.stats().world_draw_calls, 2);
    assert_eq!(renderer.pass_stats()[2].world_draw_calls, 2);
}
