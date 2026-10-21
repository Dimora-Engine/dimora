//! GPU batching for material-compatible sprites and meshes.
//!
//! Draw counts come from [`SceneRenderer::stats`]: each `draw` or `draw_indexed` in the scene pass.
//! `largest_draw_instances` is the instance range of the biggest of those calls. Placeholder and
//! diagnostic draws are not added. Pixel checks use the linear `Rgba8Unorm` target.

use engine_assets::{
    AlphaMode, AssetHandle, AssetManager, ColorSpace, MaterialAsset, MeshData, SamplerDesc, SpriteAsset, Texture,
    TextureFilter, Vertex,
};
use engine_core::Color;
use engine_math::{Quat, Rect, Transform, Vec2, Vec3};
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

fn scene(world: &mut World, assets: &AssetManager) -> RenderScene {
    propagate_transforms(world);
    extract_scene(world, assets, Vec2::new(W as f32, H as f32)).unwrap()
}

fn render(gpu: &GpuContext, renderer: &mut SceneRenderer, world: &mut World, assets: &AssetManager) -> Vec<u8> {
    let scene = scene(world, assets);
    renderer.render_to_rgba(gpu, &scene, assets, W, H).unwrap()
}

fn px(img: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * W + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

fn expect_px(img: &[u8], x: u32, y: u32, expected: [u8; 4], tolerance: i16, what: &str) {
    let pixel = px(img, x, y);
    let close = pixel.iter().zip(expected).all(|(got, want)| (*got as i16 - want as i16).abs() <= tolerance);
    assert!(close, "{what} at ({x},{y}) is {pixel:?}, expected {expected:?} ±{tolerance}");
}

fn linear(rgba: [u8; 4]) -> Texture {
    Texture::solid(1, 1, rgba).with_color_space(ColorSpace::Linear)
}

fn ortho(height: f32) -> World {
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.spawn((Camera::orthographic(height).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -4.0)));
    world
}

fn insert_sprite(assets: &mut AssetManager, name: &str, texture: Texture) -> AssetHandle<Texture> {
    let texture = assets.insert(&format!("{name}-tex"), texture).unwrap();
    assets.insert(name, SpriteAsset::whole_texture(texture, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    texture
}

fn sprite(name: &str, material: AssetHandle<MaterialAsset>) -> Sprite {
    Sprite::new(name).with_size(Vec2::splat(0.4)).with_anchor(Vec2::splat(0.5)).with_material(material)
}

fn card() -> MeshData {
    let n = Vec3::new(0.0, 0.0, -1.0);
    let verts = [
        Vertex::new(Vec3::new(-0.15, -0.15, 0.0), n, [0.0, 1.0]),
        Vertex::new(Vec3::new(0.15, -0.15, 0.0), n, [1.0, 1.0]),
        Vertex::new(Vec3::new(0.15, 0.15, 0.0), n, [1.0, 0.0]),
        Vertex::new(Vec3::new(-0.15, 0.15, 0.0), n, [0.0, 0.0]),
    ];
    MeshData::new(verts.to_vec(), vec![0, 1, 2, 0, 2, 3])
}

#[test]
fn one_thousand_compatible_opaque_sprites_are_one_draw() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    let image = Texture::from_fn(2, 1, |x, _| if x == 0 { [255, 0, 0, 255] } else { [0, 255, 0, 255] })
        .with_color_space(ColorSpace::Linear);
    insert_sprite(&mut assets, "atlas", image);
    let material = assets.insert("flat", MaterialAsset::unlit(Color::WHITE)).unwrap();
    let mut world = ortho(2.0);
    let left = Rect::new(Vec2::ZERO, Vec2::new(0.5, 1.0));
    let right = Rect::new(Vec2::new(0.5, 0.0), Vec2::ONE);
    for i in 0..1000 {
        let (x, y, region, tint, size) = if i == 0 {
            (0.0, 0.0, left, Color::WHITE, 0.25)
        } else if i == 1 {
            (0.55, 0.0, right, Color::WHITE, 0.2)
        } else if i == 2 {
            (-0.55, 0.0, left, Color::rgba(0.5, 1.0, 1.0, 1.0), 0.2)
        } else {
            (-0.85, -0.85, if i % 2 == 0 { left } else { right }, Color::WHITE, 0.02)
        };
        world.spawn((
            Transform::from_xyz(x, y, i as f32 * 0.0001),
            Sprite::new("atlas")
                .with_size(Vec2::splat(size))
                .with_anchor(Vec2::splat(0.5))
                .with_material(material)
                .with_region(region)
                .with_tint(tint),
        ));
    }

    let extracted = scene(&mut world, &assets);
    assert_eq!(extracted.stats.sprites_visible, 1000);
    assert_eq!(extracted.stats.meshes_visible, 0);
    assert_eq!(extracted.opaque_sprites.batches.len(), 1);
    assert_eq!(extracted.opaque_sprites.batches[0].range, 0..1000);
    assert_ne!(extracted.opaque_sprites.instances[0].uv, extracted.opaque_sprites.instances[1].uv);
    assert_ne!(extracted.opaque_sprites.instances[0].tint, extracted.opaque_sprites.instances[2].tint);

    let before = renderer.resource_stats();
    let pixels = renderer.render_to_rgba(&gpu, &extracted, &assets, W, H).unwrap();
    let stats = renderer.stats();
    let resources = renderer.resource_stats();
    assert_eq!(stats.sprite_instances, 1000);
    assert_eq!(stats.mesh_instances, 0);
    assert_eq!(stats.draw_calls, 1, "one compatible instanced draw, with no extra pass");
    assert_eq!(stats.largest_draw_instances, 1000);
    assert_eq!(stats.submitted_instances, 1000);
    assert_eq!(resources.texture_creates - before.texture_creates, 1, "one atlas image; placeholders already exist");
    assert_eq!(resources.texture_writes - before.texture_writes, 1);
    assert_eq!(resources.sampler_creates, before.sampler_creates, "the default sampler is reused");
    assert_eq!(resources.bind_group_creates - before.bind_group_creates, 1);
    assert_eq!(
        resources.instance_buffer_grows - before.instance_buffer_grows,
        1,
        "1000 instances grow the opaque buffer once"
    );
    expect_px(&pixels, 64, 64, [255, 0, 0, 255], 2, "sprite 0 samples the left texel");
    expect_px(&pixels, 99, 64, [0, 255, 0, 255], 2, "sprite 1 samples the right texel");
    expect_px(&pixels, 29, 64, [128, 0, 0, 255], 2, "sprite 2 keeps its own tint in the same draw");

    let grows = renderer.resource_stats().instance_buffer_grows;
    let creates = renderer.resource_stats().texture_creates;
    let binds = renderer.resource_stats().bind_group_creates;
    let _ = render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(renderer.stats().draw_calls, 1);
    assert_eq!(renderer.resource_stats().instance_buffer_grows, grows, "the second frame fits");
    assert_eq!(renderer.resource_stats().texture_creates, creates);
    assert_eq!(renderer.resource_stats().bind_group_creates, binds);
    eprintln!(
        "1000-sprite batch: cpu batches={} gpu draws={} instances={} texture_creates={} uploads={} samplers={} bind_groups={} buffer_grows={}",
        extracted.opaque_sprites.batches.len(),
        stats.draw_calls,
        stats.submitted_instances,
        resources.texture_creates - before.texture_creates,
        resources.texture_writes - before.texture_writes,
        resources.sampler_creates - before.sampler_creates,
        resources.bind_group_creates - before.bind_group_creates,
        resources.instance_buffer_grows - before.instance_buffer_grows
    );
}

#[test]
fn mask_sprites_batch_per_material_and_keep_their_cutoff() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "half", linear([0, 255, 0, 128]));
    let loose = assets
        .insert("loose", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::mask(0.4).unwrap()))
        .unwrap();
    let tight = assets
        .insert("tight", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::mask(0.75).unwrap()))
        .unwrap();
    let mut world = ortho(2.0);
    for x in [-0.5_f32, 0.0, 0.45] {
        world.spawn((Transform::from_xyz(x, 0.0, 0.0), sprite("half", loose)));
    }
    world.spawn((Transform::from_xyz(0.0, 0.55, 0.0), sprite("half", tight)));
    let extracted = scene(&mut world, &assets);
    assert_eq!(extracted.mask_sprites.batches.len(), 2);
    assert_eq!(extracted.mask_sprites.instances.len(), 4);
    let image = renderer.render_to_rgba(&gpu, &extracted, &assets, W, H).unwrap();
    assert_eq!(renderer.stats().draw_calls, 2);
    assert_eq!(renderer.stats().submitted_instances, 4);
    expect_px(&image, 64, 64, [0, 255, 0, 255], 2, "alpha 128/255 survives cutoff 0.4");
    expect_px(&image, 64, 29, [0, 0, 0, 255], 2, "the same texel is discarded at cutoff 0.75");
}

#[test]
fn one_blended_sprite_draw_matches_separate_ordered_draws() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "white", linear([255, 255, 255, 255]));
    let shared = assets.insert("shared", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::Blend)).unwrap();
    let red = assets
        .insert("red", MaterialAsset::unlit(Color::rgba(1.0, 0.0, 0.0, 0.5)).with_alpha_mode(AlphaMode::Blend))
        .unwrap();
    let green = assets
        .insert("green", MaterialAsset::unlit(Color::rgba(0.0, 1.0, 0.0, 0.5)).with_alpha_mode(AlphaMode::Blend))
        .unwrap();

    let mut separate = ortho(2.0);
    separate.spawn((Transform::from_xyz(0.0, 0.0, 1.0), sprite("white", red).with_size(Vec2::splat(1.2))));
    separate.spawn((Transform::IDENTITY, sprite("white", green).with_size(Vec2::splat(1.2))));
    let split = scene(&mut separate, &assets);
    assert_eq!(split.transparent.len(), 2);
    let split_image = renderer.render_to_rgba(&gpu, &split, &assets, W, H).unwrap();
    assert_eq!(renderer.stats().draw_calls, 2);

    let mut batched = ortho(2.0);
    batched.spawn((
        Transform::from_xyz(0.0, 0.0, 1.0),
        sprite("white", shared).with_tint(Color::rgba(1.0, 0.0, 0.0, 0.5)).with_size(Vec2::splat(1.2)),
    ));
    batched.spawn((
        Transform::IDENTITY,
        sprite("white", shared).with_tint(Color::rgba(0.0, 1.0, 0.0, 0.5)).with_size(Vec2::splat(1.2)),
    ));
    let together = scene(&mut batched, &assets);
    assert_eq!(together.transparent.len(), 1);
    assert_eq!(together.blended_sprites.batches[0].range, 0..2);
    let together_image = renderer.render_to_rgba(&gpu, &together, &assets, W, H).unwrap();
    assert_eq!(renderer.stats().draw_calls, 1);
    assert_eq!(renderer.stats().largest_draw_instances, 2);
    let split_px = px(&split_image, 64, 64);
    let together_px = px(&together_image, 64, 64);
    assert!(
        split_px.iter().zip(together_px).all(|(a, b)| (*a as i16 - b as i16).abs() <= 2),
        "one blended draw {together_px:?} must match ordered draws {split_px:?}"
    );
    expect_px(&together_image, 64, 64, [64, 128, 0, 255], 3, "back red 0.5 then front green 0.5 over black");
}

#[test]
fn blended_meshes_batch_only_inside_a_contiguous_run() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    assets.insert("card", card()).unwrap();
    insert_sprite(&mut assets, "white", linear([255, 255, 255, 255]));
    let glass = assets
        .insert("glass", MaterialAsset::unlit(Color::rgba(0.0, 0.0, 1.0, 0.5)).with_alpha_mode(AlphaMode::Blend))
        .unwrap();
    let other = assets
        .insert("other", MaterialAsset::unlit(Color::rgba(1.0, 0.0, 0.0, 0.5)).with_alpha_mode(AlphaMode::Blend))
        .unwrap();
    let sprite_mat = assets
        .insert("sprite", MaterialAsset::unlit(Color::rgba(0.0, 1.0, 0.0, 0.5)).with_alpha_mode(AlphaMode::Blend))
        .unwrap();

    let mut adjacent = ortho(2.0);
    for x in [-0.5_f32, 0.0, 0.5] {
        adjacent.spawn((Transform::from_xyz(x, 0.0, 1.0), Mesh::new("card").with_material_asset(glass)));
    }
    let extracted = scene(&mut adjacent, &assets);
    assert_eq!(extracted.transparent.len(), 1);
    let _ = renderer.render_to_rgba(&gpu, &extracted, &assets, W, H).unwrap();
    assert_eq!(renderer.stats().draw_calls, 1);
    assert_eq!(renderer.stats().largest_draw_instances, 3);

    let mut split = ortho(2.0);
    split.spawn((Transform::from_xyz(0.0, 0.0, 2.0), Mesh::new("card").with_material_asset(glass)));
    split.spawn((Transform::from_xyz(0.0, 0.0, 1.0), sprite("white", sprite_mat).with_size(Vec2::splat(0.3))));
    split.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(glass)));
    let extracted = scene(&mut split, &assets);
    assert_eq!(extracted.transparent.len(), 3);
    let image = renderer.render_to_rgba(&gpu, &extracted, &assets, W, H).unwrap();
    assert_eq!(renderer.stats().draw_calls, 3);
    // Blue 0.5, then green 0.5, then blue 0.5 over black.
    // After two blues and a green the centre is not the "all sprites last" green.
    // Blue 0.5, green 0.5, blue 0.5 over black → about (0, 64, 160).
    expect_px(&image, 64, 64, [0, 64, 160, 255], 8, "mesh, sprite, mesh stays that order");

    let mut interrupted = ortho(2.0);
    interrupted.spawn((Transform::from_xyz(-0.5, 0.0, 2.0), Mesh::new("card").with_material_asset(glass)));
    interrupted.spawn((Transform::from_xyz(0.0, 0.0, 1.0), Mesh::new("card").with_material_asset(other)));
    interrupted.spawn((Transform::from_xyz(0.5, 0.0, 0.0), Mesh::new("card").with_material_asset(glass)));
    let extracted = scene(&mut interrupted, &assets);
    assert_eq!(extracted.transparent.len(), 3, "a different material in between does not regroup the matching meshes");
    let _ = renderer.render_to_rgba(&gpu, &extracted, &assets, W, H).unwrap();
    assert_eq!(renderer.stats().draw_calls, 3);
}

#[test]
fn mixed_sprite_and_mesh_runs_merge_only_at_kind_boundaries() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    assets.insert("card", card()).unwrap();
    insert_sprite(&mut assets, "white", linear([255, 255, 255, 255]));
    let glass = assets.insert("glass", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::Blend)).unwrap();

    let sequences: [(f32, &str); 4] = [(3.0, "mesh"), (2.0, "mesh"), (1.0, "sprite"), (0.0, "sprite")];
    let mut world = ortho(2.0);
    for (z, kind) in sequences {
        let x = z * 0.2;
        if kind == "mesh" {
            world.spawn((Transform::from_xyz(x, 0.0, z), Mesh::new("card").with_material_asset(glass)));
        } else {
            world.spawn((Transform::from_xyz(x, 0.0, z), sprite("white", glass)));
        }
    }
    let extracted = scene(&mut world, &assets);
    assert_eq!(extracted.transparent.len(), 2);
    let _ = renderer.render_to_rgba(&gpu, &extracted, &assets, W, H).unwrap();
    assert_eq!(renderer.stats().draw_calls, 2);
    assert_eq!(renderer.stats().submitted_instances, 4);

    let mut alternate = ortho(2.0);
    for (i, kind) in ["mesh", "sprite", "mesh", "sprite"].into_iter().enumerate() {
        let z = 3.0 - i as f32;
        if kind == "mesh" {
            alternate
                .spawn((Transform::from_xyz(0.2 * i as f32, 0.0, z), Mesh::new("card").with_material_asset(glass)));
        } else {
            alternate.spawn((Transform::from_xyz(0.2 * i as f32, 0.0, z), sprite("white", glass)));
        }
    }
    let extracted = scene(&mut alternate, &assets);
    assert_eq!(extracted.transparent.len(), 4);
    let _ = renderer.render_to_rgba(&gpu, &extracted, &assets, W, H).unwrap();
    assert_eq!(renderer.stats().draw_calls, 4, "alternating kinds are not merged across the layout change");
}

#[test]
fn material_replacement_updates_class_colour_cutoff_light_and_culling() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "half", linear([255, 255, 255, 128]));
    insert_sprite(&mut assets, "blue", linear([0, 0, 255, 255]));
    let back = assets.insert("back", MaterialAsset::unlit(Color::rgb(0.0, 0.0, 1.0))).unwrap();
    let front = assets.insert("front", MaterialAsset::unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let mut world = ortho(2.0);
    world.spawn((Transform::from_xyz(0.0, 0.0, 1.0), sprite("blue", back).with_size(Vec2::splat(1.4))));
    for _ in 0..4 {
        world.spawn((Transform::IDENTITY, sprite("half", front).with_size(Vec2::splat(1.0))));
    }

    let image = render(&gpu, &mut renderer, &mut world, &assets);
    expect_px(&image, 64, 64, [255, 0, 0, 255], 2, "opaque front covers the blue sprite");
    assert_eq!(renderer.stats().draw_calls, 2);
    assert_eq!(renderer.stats().largest_draw_instances, 4);
    let creates = renderer.resource_stats().texture_creates;

    assets.insert("front", MaterialAsset::unlit(Color::rgb(0.0, 1.0, 0.0))).unwrap();
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    expect_px(&image, 64, 64, [0, 255, 0, 255], 2, "the replaced base colour is on the next frame");
    assert_eq!(renderer.stats().draw_calls, 2);
    assert_eq!(renderer.resource_stats().texture_creates, creates);

    let albedo = assets.texture("blue-tex");
    let mut pointed = MaterialAsset::unlit(Color::WHITE);
    pointed.albedo = Some(albedo);
    assets.insert("front", pointed).unwrap();
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    expect_px(&image, 64, 64, [0, 0, 255, 255], 2, "the replaced albedo samples the cached blue image");
    assert_eq!(renderer.resource_stats().texture_creates, creates, "pointing at an existing texture allocates nothing");

    assets.insert("front", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::mask(0.75).unwrap())).unwrap();
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    expect_px(&image, 64, 64, [0, 0, 255, 255], 2, "cutoff 0.75 discards alpha 0.5 and shows the sprite behind");
    assert_eq!(scene(&mut world, &assets).mask_sprites.batches.len(), 1);
    assert_eq!(renderer.resource_stats().texture_creates, creates);

    assets
        .insert("front", MaterialAsset::unlit(Color::rgba(1.0, 0.0, 0.0, 0.5)).with_alpha_mode(AlphaMode::Blend))
        .unwrap();
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    // Four stacked sprites. Each is texture alpha 0.5 times material alpha 0.5, so 0.25, over blue.
    expect_px(&image, 64, 64, [175, 0, 80, 255], 4, "four blended instances composite in order over the blue sprite");
    assert_eq!(scene(&mut world, &assets).transparent.len(), 1);
    assert_eq!(renderer.stats().largest_draw_instances, 4);

    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 0.0 });
    assets.insert("front", MaterialAsset::color(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    expect_px(&image, 64, 64, [0, 0, 0, 255], 2, "lit with zero ambient is black");
    assets.insert("front", MaterialAsset::unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    expect_px(&image, 64, 64, [255, 0, 0, 255], 2, "unlit ignores the zero ambient");
    assert_eq!(renderer.resource_stats().texture_creates, creates);

    let mut back = World::new();
    back.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    back.spawn((
        Camera::orthographic(2.0).with_clear_color(Color::BLACK),
        Transform::from_xyz(0.0, 0.0, 4.0).with_rotation(Quat::from_rotation_y(core::f32::consts::PI)),
    ));
    back.spawn((Transform::IDENTITY, sprite("half", front).with_size(Vec2::splat(1.0)).fixed()));
    let image = render(&gpu, &mut renderer, &mut back, &assets);
    expect_px(&image, 64, 64, [0, 0, 0, 255], 2, "double_sided false hides the back");
    let mut shown = MaterialAsset::unlit(Color::rgb(1.0, 0.0, 0.0));
    shown.double_sided = true;
    assets.insert("front", shown).unwrap();
    let image = render(&gpu, &mut renderer, &mut back, &assets);
    expect_px(&image, 64, 64, [255, 0, 0, 255], 2, "double_sided true shows the back on the next frame");
    assert_eq!(renderer.stats().draw_calls, 1);
    assert_eq!(renderer.resource_stats().texture_creates, creates);
}

#[test]
fn different_samplers_split_draws_and_share_the_sampler_cache() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    let nearest = linear([255, 0, 0, 255]);
    let mut filtered = linear([0, 0, 255, 255]);
    filtered.sampler =
        SamplerDesc { min_filter: TextureFilter::Linear, mag_filter: TextureFilter::Linear, ..filtered.sampler };
    insert_sprite(&mut assets, "near", nearest);
    insert_sprite(&mut assets, "far", filtered);
    let material = assets.insert("flat", MaterialAsset::unlit(Color::WHITE)).unwrap();
    let mut world = ortho(2.0);
    world.spawn((Transform::from_xyz(-0.4, 0.0, 0.0), sprite("near", material)));
    world.spawn((Transform::from_xyz(0.4, 0.0, 0.0), sprite("far", material)));
    let before = renderer.resource_stats();
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(renderer.stats().draw_calls, 2, "two textures are two views even when the material matches");
    assert_eq!(
        renderer.resource_stats().sampler_creates,
        before.sampler_creates + 1,
        "the linear sampler is cached once"
    );
    expect_px(&image, 38, 64, [255, 0, 0, 255], 2, "nearest red");
    expect_px(&image, 90, 64, [0, 0, 255, 255], 2, "linear blue");

    let mut again = ortho(2.0);
    again.spawn((Transform::from_xyz(-0.4, 0.0, 0.0), sprite("near", material)));
    again.spawn((Transform::from_xyz(0.4, 0.0, 0.0), sprite("near", material).with_tint(Color::rgb(0.0, 1.0, 0.0))));
    let _ = render(&gpu, &mut renderer, &mut again, &assets);
    assert_eq!(renderer.stats().draw_calls, 1, "tint does not split one texture");
    assert_eq!(renderer.stats().largest_draw_instances, 2);
}

#[test]
fn legacy_sprites_and_meshes_keep_their_batches() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "white", linear([255, 255, 255, 255]));
    assets.insert("box", MeshData::cuboid(Vec3::splat(0.15))).unwrap();
    assets.insert("card", card()).unwrap();
    let mut world = ortho(2.0);
    for x in [-0.5_f32, 0.0, 0.5] {
        world.spawn((
            Transform::from_xyz(x, 0.3, 0.0),
            Sprite::new("white").with_size(Vec2::splat(0.25)).with_anchor(Vec2::splat(0.5)).cutout().unlit(),
        ));
    }
    world.spawn((Transform::from_xyz(-0.3, -0.3, 0.0), Mesh::new("box")));
    world.spawn((Transform::from_xyz(0.0, -0.3, 0.0), Mesh::new("box")));
    world.spawn((Transform::from_xyz(0.35, -0.3, 0.0), Mesh::new("card")));
    let extracted = scene(&mut world, &assets);
    assert_eq!(extracted.cutout_sprites.batches.len(), 1);
    assert_eq!(extracted.cutout_sprites.batches[0].range, 0..3);
    assert_eq!(MeshInstance::run_end(&extracted.meshes, 0), 2);
    assert_eq!(extracted.meshes.len(), 3);
    let _ = renderer.render_to_rgba(&gpu, &extracted, &assets, W, H).unwrap();
    assert_eq!(renderer.stats().draw_calls, 3, "one sprite batch plus two mesh geometries");
    assert_eq!(renderer.stats().sprite_instances, 3);
    assert_eq!(renderer.stats().mesh_instances, 3);
    assert!(extracted.cutout_sprites.instances.iter().all(|i| i.material_id.is_none()));
}

#[test]
fn batching_and_depth_hold_under_three_cameras() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "white", linear([255, 255, 255, 255]));
    let material = assets.insert("flat", MaterialAsset::unlit(Color::WHITE)).unwrap();
    let pitch = (1.0_f32 / 2.0_f32.sqrt()).atan();
    let rotation = Quat::from_rotation_y(core::f32::consts::FRAC_PI_4) * Quat::from_rotation_x(pitch);
    let forward = Transform::from_rotation(rotation).forward();
    let views = [
        ("perspective", Camera::perspective(50.0), Transform::from_xyz(0.0, 0.0, -4.0)),
        ("orthographic", Camera::orthographic(2.0), Transform::from_xyz(0.0, 0.0, -4.0)),
        ("isometric", Camera::orthographic(4.0), Transform::from_position(-forward * 6.0).with_rotation(rotation)),
    ];
    for (name, camera, pose) in views {
        let mut world = World::new();
        world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
        world.spawn((camera.with_clear_color(Color::BLACK), pose));
        world.spawn((
            Transform::from_xyz(0.0, 0.0, 0.4),
            sprite("white", material).with_tint(Color::rgb(1.0, 0.0, 0.0)).with_size(Vec2::splat(0.7)),
        ));
        world.spawn((
            Transform::IDENTITY,
            sprite("white", material).with_tint(Color::rgb(0.0, 1.0, 0.0)).with_size(Vec2::splat(0.7)),
        ));
        let extracted = scene(&mut world, &assets);
        assert_eq!(extracted.opaque_sprites.batches.len(), 1, "{name}");
        assert!(extracted.transparent.is_empty(), "{name} does not reclassify opaque");
        let image = renderer.render_to_rgba(&gpu, &extracted, &assets, W, H).unwrap();
        assert_eq!(renderer.stats().draw_calls, 1, "{name}");
        assert_eq!(renderer.stats().largest_draw_instances, 2, "{name}");
        expect_px(&image, 64, 64, [0, 255, 0, 255], 4, name);
    }
}
