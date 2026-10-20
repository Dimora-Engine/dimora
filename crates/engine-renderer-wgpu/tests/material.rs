//! GPU readback for material-driven sprites and meshes.
//!
//! Unassigned sprites and meshes stay on the legacy pipelines. An assigned material selects the
//! texture, the linear colour, the alpha class, lighting, and culling. These tests read pixels.

use engine_assets::{
    AlphaMode, AssetHandle, AssetManager, AtlasBuilder, AtlasConfig, ColorSpace, MaterialAsset, MeshData, SamplerDesc,
    SpriteAsset, Texture, TextureFilter, Vertex,
};
use engine_core::Color;
use engine_math::{Quat, Transform, Vec2, Vec3};
use engine_renderer::*;
use engine_renderer_wgpu::{GpuContext, SceneRenderer};
use engine_world::{propagate_transforms, World};

mod capture {
    include!("../../log_capture.rs");
}

const W: u32 = 128;
const H: u32 = 128;
const REQUIRE_GPU_VAR: &str = "DIMORA_REQUIRE_GPU";

fn gpu() -> Option<GpuContext> {
    match GpuContext::headless() {
        Ok(gpu) => {
            let info = gpu.adapter.get_info();
            let anisotropic =
                gpu.adapter.get_downlevel_capabilities().flags.contains(wgpu::DownlevelFlags::ANISOTROPIC_FILTERING);
            eprintln!(
                "GPU test adapter: {} ({:?}, {:?}); DownlevelFlags::ANISOTROPIC_FILTERING={anisotropic}",
                info.name, info.backend, info.device_type
            );
            Some(gpu)
        }
        Err(error) => {
            if std::env::var(REQUIRE_GPU_VAR).ok().as_deref() == Some("1") {
                panic!("{REQUIRE_GPU_VAR}=1 but the GPU test adapter is unavailable: {error}");
            }
            eprintln!("skipping GPU test: {error}");
            None
        }
    }
}

fn renderer(gpu: &GpuContext) -> SceneRenderer {
    SceneRenderer::new(gpu, wgpu::TextureFormat::Rgba8Unorm)
}

fn render(gpu: &GpuContext, renderer: &mut SceneRenderer, world: &mut World, assets: &AssetManager) -> Vec<u8> {
    propagate_transforms(world);
    let scene = extract_scene(world, assets, Vec2::new(W as f32, H as f32)).unwrap();
    renderer.render_to_rgba(gpu, &scene, assets, W, H).unwrap()
}

fn px(img: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * W + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

fn near(pixel: [u8; 4], expected: [u8; 4], tolerance: i16) -> bool {
    pixel.iter().zip(expected).all(|(got, want)| (*got as i16 - want as i16).abs() <= tolerance)
}

fn expect_px(img: &[u8], x: u32, y: u32, expected: [u8; 4], tolerance: i16, what: &str) {
    let pixel = px(img, x, y);
    assert!(near(pixel, expected, tolerance), "{what} at ({x},{y}) is {pixel:?}, expected {expected:?} ±{tolerance}");
}

fn linear(rgba: [u8; 4]) -> Texture {
    Texture::solid(1, 1, rgba).with_color_space(ColorSpace::Linear)
}

fn row(left: [u8; 4], right: [u8; 4]) -> Texture {
    Texture::from_fn(2, 1, move |x, _| if x == 0 { left } else { right }).with_color_space(ColorSpace::Linear)
}

fn world(camera: Camera, transform: Transform) -> World {
    let mut world = World::new();
    world.insert_resource(MissingAssetWarnings::default());
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.spawn((camera.with_clear_color(Color::BLACK), transform));
    world
}

fn ortho() -> World {
    world(Camera::orthographic(2.0), Transform::from_xyz(0.0, 0.0, -4.0))
}

fn back_view() -> World {
    world(
        Camera::orthographic(2.0),
        Transform::from_xyz(0.0, 0.0, 4.0).with_rotation(Quat::from_rotation_y(core::f32::consts::PI)),
    )
}

fn sprite(name: &str, material: AssetHandle<MaterialAsset>) -> Sprite {
    Sprite::new(name).with_size(Vec2::splat(2.0)).with_anchor(Vec2::splat(0.5)).with_material(material).unlit()
}

fn card() -> MeshData {
    let n = Vec3::new(0.0, 0.0, -1.0);
    let verts = [
        Vertex::new(Vec3::new(-1.0, -1.0, 0.0), n, [0.0, 1.0]),
        Vertex::new(Vec3::new(1.0, -1.0, 0.0), n, [1.0, 1.0]),
        Vertex::new(Vec3::new(1.0, 1.0, 0.0), n, [1.0, 0.0]),
        Vertex::new(Vec3::new(-1.0, 1.0, 0.0), n, [0.0, 0.0]),
    ];
    // Counter-clockwise when the camera at -Z looks along +Z, so back-face culling keeps this side.
    MeshData::new(verts.to_vec(), vec![0, 1, 2, 0, 2, 3])
}

fn insert_sprite(assets: &mut AssetManager, name: &str, texture: Texture) -> AssetHandle<Texture> {
    let texture = assets.insert(&format!("{name}-tex"), texture).unwrap();
    assets.insert(name, SpriteAsset::whole_texture(texture, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    texture
}

#[test]
fn opaque_sprites_write_depth_and_do_not_discard_transparent_texels() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "clear", linear([0, 0, 0, 0]));
    insert_sprite(&mut assets, "red", linear([255, 0, 0, 255]));
    let material = assets.insert("solid", MaterialAsset::unlit(Color::WHITE)).unwrap();
    let mut world = ortho();
    world.spawn((
        Transform::from_xyz(0.0, 0.0, 1.0),
        Sprite::new("red").with_size(Vec2::splat(2.0)).with_anchor(Vec2::splat(0.5)).cutout().unlit(),
    ));
    world.spawn((Transform::IDENTITY, sprite("clear", material)));
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    expect_px(&image, 64, 64, [0, 0, 0, 255], 1, "opaque keeps rgb and hides the cutout behind it");
}

#[test]
fn masked_sprites_use_the_material_cutoff_and_write_depth() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "half", linear([0, 255, 0, 255]));
    insert_sprite(&mut assets, "red", linear([255, 0, 0, 255]));
    let keep = assets
        .insert(
            "keep",
            MaterialAsset::unlit(Color::rgba(1.0, 1.0, 1.0, 0.5)).with_alpha_mode(AlphaMode::mask(0.5).unwrap()),
        )
        .unwrap();
    let drop = assets
        .insert(
            "drop",
            MaterialAsset::unlit(Color::rgba(1.0, 1.0, 1.0, 0.5)).with_alpha_mode(AlphaMode::mask(0.75).unwrap()),
        )
        .unwrap();

    let mut kept = ortho();
    kept.spawn((
        Transform::from_xyz(0.0, 0.0, 1.0),
        Sprite::new("red").with_size(Vec2::splat(2.0)).with_anchor(Vec2::splat(0.5)).cutout().unlit(),
    ));
    kept.spawn((Transform::IDENTITY, sprite("half", keep)));
    let image = render(&gpu, &mut renderer, &mut kept, &assets);
    expect_px(&image, 64, 64, [0, 255, 0, 255], 2, "effective alpha 0.5 is kept at cutoff 0.5 and occludes the cutout");

    let mut dropped = ortho();
    dropped.spawn((
        Transform::from_xyz(0.0, 0.0, 1.0),
        Sprite::new("red").with_size(Vec2::splat(2.0)).with_anchor(Vec2::splat(0.5)).cutout().unlit(),
    ));
    dropped.spawn((Transform::IDENTITY, sprite("half", drop)));
    let image = render(&gpu, &mut renderer, &mut dropped, &assets);
    expect_px(&image, 64, 64, [255, 0, 0, 255], 2, "effective alpha 0.5 is discarded at cutoff 0.75");
}

#[test]
fn blended_material_sprites_composite_and_sort_with_legacy_blends() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "white", linear([255, 255, 255, 255]));
    insert_sprite(&mut assets, "red", linear([255, 0, 0, 255]));
    insert_sprite(&mut assets, "blue", linear([0, 0, 255, 255]));
    let glass = assets
        .insert("glass", MaterialAsset::unlit(Color::rgba(1.0, 1.0, 1.0, 0.5)).with_alpha_mode(AlphaMode::Blend))
        .unwrap();
    let mut world = ortho();
    world.spawn((
        Transform::from_xyz(0.0, 0.0, 1.0),
        Sprite::new("red").with_size(Vec2::splat(2.0)).with_anchor(Vec2::splat(0.5)).cutout().unlit(),
    ));
    world.spawn((Transform::IDENTITY, sprite("white", glass)));
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    expect_px(&image, 64, 64, [255, 127, 127, 255], 3, "blend alpha 0.5 over red");

    let cover = assets
        .insert("cover", MaterialAsset::unlit(Color::rgb(1.0, 0.0, 0.0)).with_alpha_mode(AlphaMode::Blend))
        .unwrap();
    let mut sorted = ortho();
    sorted.spawn((
        Transform::from_xyz(0.0, 0.0, 2.0),
        Sprite::new("blue").with_size(Vec2::splat(2.0)).with_anchor(Vec2::splat(0.5)).unlit(),
    ));
    sorted.spawn((Transform::IDENTITY, sprite("white", cover)));
    let image = render(&gpu, &mut renderer, &mut sorted, &assets);
    expect_px(&image, 64, 64, [255, 0, 0, 255], 2, "the nearer material blend draws after the legacy blend");
}

#[test]
fn material_color_and_sprite_tint_multiply_in_linear_space() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "white", linear([255, 255, 255, 255]));
    let material = assets.insert("paint", MaterialAsset::unlit(Color::rgb(1.0, 0.5, 0.0))).unwrap();
    let mut world = ortho();
    world.spawn((Transform::IDENTITY, sprite("white", material).with_tint(Color::rgba(0.5, 1.0, 0.0, 1.0))));
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    expect_px(&image, 64, 64, [127, 127, 0, 255], 2, "white * (1, 0.5, 0) * (0.5, 1, 0)");
}

#[test]
fn an_assigned_material_replaces_the_sprite_lit_flag() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "white", linear([255, 255, 255, 255]));
    let lit = assets.insert("lit", MaterialAsset::color(Color::WHITE)).unwrap();
    let unlit = assets.insert("unlit", MaterialAsset::unlit(Color::WHITE)).unwrap();
    let mut dark = ortho();
    dark.insert_resource(AmbientLight { color: Color::WHITE, intensity: 0.0 });
    dark.spawn((Transform::IDENTITY, sprite("white", lit)));
    let image = render(&gpu, &mut renderer, &mut dark, &assets);
    expect_px(&image, 64, 64, [0, 0, 0, 255], 2, "material lit with zero ambient is black");

    let mut flat = ortho();
    flat.insert_resource(AmbientLight { color: Color::WHITE, intensity: 0.0 });
    flat.spawn((
        Transform::IDENTITY,
        Sprite::new("white").with_size(Vec2::splat(2.0)).with_anchor(Vec2::splat(0.5)).with_material(unlit),
    ));
    let image = render(&gpu, &mut renderer, &mut flat, &assets);
    expect_px(
        &image,
        64,
        64,
        [255, 255, 255, 255],
        2,
        "material unlit ignores a zero ambient and the sprite lit default",
    );
}

#[test]
fn double_sided_sprites_are_visible_from_the_back_and_single_sided_are_not() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "red", linear([255, 0, 0, 255]));
    let single = assets.insert("single", MaterialAsset::unlit(Color::WHITE)).unwrap();
    let both = assets.insert("both", MaterialAsset::unlit(Color::WHITE).double_sided()).unwrap();

    let mut front = ortho();
    front.spawn((Transform::IDENTITY, sprite("red", single).fixed()));
    let image = render(&gpu, &mut renderer, &mut front, &assets);
    expect_px(&image, 64, 64, [255, 0, 0, 255], 2, "the front of a culled sprite is visible");

    let mut back = back_view();
    back.spawn((Transform::IDENTITY, sprite("red", single).fixed()));
    let image = render(&gpu, &mut renderer, &mut back, &assets);
    expect_px(&image, 64, 64, [0, 0, 0, 255], 2, "double_sided false hides the back face");

    let mut back_both = back_view();
    back_both.spawn((Transform::IDENTITY, sprite("red", both).fixed()));
    let image = render(&gpu, &mut renderer, &mut back_both, &assets);
    expect_px(&image, 64, 64, [255, 0, 0, 255], 2, "double_sided true shows the back face");
}

#[test]
fn an_albedo_override_keeps_sprite_uvs_and_none_uses_the_sprite_texture() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    let source = assets.insert("source", row([255, 0, 0, 255], [0, 0, 255, 255])).unwrap();
    let sprite_asset = SpriteAsset::from_atlas_rect(source, Vec2::new(2.0, 1.0), 1, 0, 1, 1).with_pixels_per_unit(1.0);
    assets.insert("sign", sprite_asset).unwrap();
    let paint = assets.insert("paint", row([0, 255, 0, 255], [255, 255, 0, 255])).unwrap();
    let mut material = MaterialAsset::unlit(Color::WHITE);
    material.albedo = Some(paint);
    let material = assets.insert("painted", material).unwrap();

    let mut world = ortho();
    world.spawn((Transform::IDENTITY, sprite("sign", material).fixed()));
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    expect_px(&image, 32, 64, [255, 255, 0, 255], 2, "right-half UVs sample the override's right texel");
    expect_px(&image, 96, 64, [255, 255, 0, 255], 2, "the override is not stretched across 0..1");

    let plain = assets.insert("plain", MaterialAsset::unlit(Color::WHITE)).unwrap();
    let mut fallback = ortho();
    fallback.spawn((Transform::IDENTITY, sprite("sign", plain).fixed()));
    let image = render(&gpu, &mut renderer, &mut fallback, &assets);
    expect_px(&image, 64, 64, [0, 0, 255, 255], 2, "albedo None samples the sprite asset's right half");
}

#[test]
fn a_missing_material_is_not_drawn_and_a_missing_albedo_uses_the_placeholder() {
    capture::begin();
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "white", linear([255, 255, 255, 255]));
    assets.insert("card", card()).unwrap();
    let missing = AssetHandle::<MaterialAsset>::from_name("absent");
    let mut world = ortho();
    world.spawn((Transform::IDENTITY, sprite("white", missing)));
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    expect_px(&image, 64, 64, [0, 0, 0, 255], 1, "an unresolved material draws nothing");
    let messages = capture::messages();
    assert!(messages.iter().any(|m| m.contains("skipping material")), "{messages:?}");
    assert!(
        capture::records().iter().any(|r| r.level == log::Level::Warn && r.message.contains("skipping material")),
        "{:?}",
        capture::messages()
    );

    let mut broken = MaterialAsset::unlit(Color::WHITE);
    broken.albedo = Some(AssetHandle::from_name("no-such-texture"));
    let broken = assets.insert("broken", broken).unwrap();
    let mut sprite_world = ortho();
    sprite_world.spawn((Transform::IDENTITY, sprite("white", broken)));
    let image = render(&gpu, &mut renderer, &mut sprite_world, &assets);
    expect_px(&image, 64, 64, [255, 0, 255, 255], 2, "a missing sprite albedo is the magenta placeholder");

    let mut mesh_world = ortho();
    mesh_world.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(broken)));
    let image = render(&gpu, &mut renderer, &mut mesh_world, &assets);
    expect_px(&image, 64, 64, [255, 255, 255, 255], 2, "a missing mesh albedo is the white placeholder");
    let messages = capture::messages();
    assert!(messages.iter().any(|m| m.contains("material texture")), "{messages:?}");
}

#[test]
fn assigned_meshes_follow_opaque_and_mask_materials() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    assets.insert("card", card()).unwrap();
    assets.insert("green", linear([0, 255, 0, 255])).unwrap();
    assets.insert("half", linear([0, 255, 0, 128])).unwrap();
    let red = assets.insert("red-mat", MaterialAsset::unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let mut tinted = MaterialAsset::unlit(Color::WHITE);
    tinted.albedo = Some(assets.texture("green"));
    let tinted = assets.insert("tinted", tinted).unwrap();
    let keep = assets
        .insert("keep", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::mask(0.4).unwrap()))
        .unwrap();
    let mut keep = *assets.get(keep).unwrap();
    keep.albedo = Some(assets.texture("half"));
    assets.insert("keep", keep).unwrap();
    let drop = assets
        .insert("drop", {
            let mut material = MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::mask(0.6).unwrap());
            material.albedo = Some(assets.texture("half"));
            material
        })
        .unwrap();

    let mut opaque = ortho();
    opaque.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(red)));
    let image = render(&gpu, &mut renderer, &mut opaque, &assets);
    expect_px(&image, 64, 64, [255, 0, 0, 255], 2, "untextured opaque mesh uses the base colour");

    let mut textured = ortho();
    textured.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(tinted)));
    let image = render(&gpu, &mut renderer, &mut textured, &assets);
    expect_px(&image, 64, 64, [0, 255, 0, 255], 2, "the albedo texture replaces the untextured white");

    let mut masked = ortho();
    masked.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(assets.material("keep"))));
    let image = render(&gpu, &mut renderer, &mut masked, &assets);
    expect_px(&image, 64, 64, [0, 255, 0, 255], 2, "mesh alpha 128/255 is kept at cutoff 0.4");

    let mut discarded = ortho();
    discarded.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(drop)));
    let image = render(&gpu, &mut renderer, &mut discarded, &assets);
    expect_px(&image, 64, 64, [0, 0, 0, 255], 2, "mesh alpha 128/255 is discarded at cutoff 0.6");
}

#[test]
fn assigned_mesh_lighting_and_culling_follow_the_material() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    assets.insert("card", card()).unwrap();
    let lit = assets.insert("lit", MaterialAsset::color(Color::WHITE)).unwrap();
    let unlit = assets.insert("unlit", MaterialAsset::unlit(Color::WHITE)).unwrap();
    let both = assets.insert("both", MaterialAsset::unlit(Color::WHITE).double_sided()).unwrap();

    let mut dark = ortho();
    dark.insert_resource(AmbientLight { color: Color::WHITE, intensity: 0.0 });
    dark.spawn((
        Transform::IDENTITY,
        Mesh::new("card").with_material(Material::unlit(Color::rgb(0.0, 0.0, 1.0))).with_material_asset(lit),
    ));
    let image = render(&gpu, &mut renderer, &mut dark, &assets);
    expect_px(&image, 64, 64, [0, 0, 0, 255], 2, "material lit replaces the inline unlit flag");

    let mut flat = ortho();
    flat.insert_resource(AmbientLight { color: Color::WHITE, intensity: 0.0 });
    flat.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(unlit)));
    let image = render(&gpu, &mut renderer, &mut flat, &assets);
    expect_px(&image, 64, 64, [255, 255, 255, 255], 2, "an unlit mesh material stays white in zero ambient");

    let mut front = ortho();
    front.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(unlit)));
    let image = render(&gpu, &mut renderer, &mut front, &assets);
    expect_px(&image, 64, 64, [255, 255, 255, 255], 2, "the front of a single-sided mesh is visible");

    let mut back = back_view();
    back.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(unlit)));
    let image = render(&gpu, &mut renderer, &mut back, &assets);
    expect_px(&image, 64, 64, [0, 0, 0, 255], 2, "double_sided false culls the mesh back face");

    let mut back_both = back_view();
    back_both.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(both)));
    let image = render(&gpu, &mut renderer, &mut back_both, &assets);
    expect_px(&image, 64, 64, [255, 255, 255, 255], 2, "double_sided true shows the mesh back face");
}

#[test]
fn a_blended_mesh_is_skipped_and_an_unassigned_mesh_still_draws() {
    capture::begin();
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    assets.insert("card", card()).unwrap();
    assets.insert("cube", MeshData::cuboid(Vec3::splat(1.0))).unwrap();
    let blend = assets
        .insert("glass", MaterialAsset::unlit(Color::rgb(1.0, 0.0, 0.0)).with_alpha_mode(AlphaMode::Blend))
        .unwrap();
    let mut skipped = ortho();
    skipped.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(blend)));
    let image = render(&gpu, &mut renderer, &mut skipped, &assets);
    expect_px(&image, 64, 64, [0, 0, 0, 255], 1, "a blended mesh is not drawn as opaque");
    assert_eq!(renderer.stats().draw_calls, 0);
    let messages = capture::messages();
    assert!(messages.iter().any(|m| m.contains("glass") && m.contains("Blend is not drawn")), "{messages:?}");
    assert!(
        capture::records().iter().any(|r| r.level == log::Level::Warn && r.message.contains("Blend is not drawn")),
        "{messages:?}"
    );

    let mut legacy = world(Camera::perspective(60.0), Transform::from_xyz(0.0, 0.0, -4.0));
    legacy.spawn((Transform::IDENTITY, Mesh::new("cube").with_material(Material::unlit(Color::rgb(0.0, 0.0, 1.0)))));
    let image = render(&gpu, &mut renderer, &mut legacy, &assets);
    expect_px(&image, 64, 64, [0, 0, 255, 255], 2, "an unassigned mesh still uses its inline material");
}

#[test]
fn material_class_is_stable_across_perspective_orthographic_and_isometric_views() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "red", linear([255, 0, 0, 255]));
    insert_sprite(&mut assets, "green", linear([0, 255, 0, 255]));
    let material = assets.insert("flat", MaterialAsset::unlit(Color::WHITE).double_sided()).unwrap();
    let pitch = (1.0_f32 / 2.0_f32.sqrt()).atan();
    let rotation = Quat::from_rotation_y(core::f32::consts::FRAC_PI_4) * Quat::from_rotation_x(pitch);
    let forward = Transform::from_rotation(rotation).forward();
    let views = [
        ("perspective", Camera::perspective(60.0), Transform::from_xyz(0.0, 0.0, -4.0)),
        ("orthographic", Camera::orthographic(4.0), Transform::from_xyz(0.0, 0.0, -4.0)),
        ("isometric", Camera::orthographic(4.0), Transform::from_position(-forward * 6.0).with_rotation(rotation)),
    ];
    for (name, camera, transform) in views {
        let mut world = world(camera, transform);
        world.spawn((
            Transform::IDENTITY,
            Sprite::new("red")
                .with_size(Vec2::splat(8.0))
                .with_anchor(Vec2::splat(0.5))
                .with_material(material)
                .fixed(),
        ));
        let image = render(&gpu, &mut renderer, &mut world, &assets);
        expect_px(&image, 64, 64, [255, 0, 0, 255], 2, name);
    }

    let mask = assets
        .insert("mask", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::mask(0.5).unwrap()))
        .unwrap();
    let mut world = world(Camera::perspective(60.0), Transform::from_xyz(0.0, 0.0, -4.0));
    world.spawn((
        Transform::from_xyz(0.0, 0.0, 1.0),
        Sprite::new("red").with_size(Vec2::splat(4.0)).with_anchor(Vec2::splat(0.5)).cutout().unlit(),
    ));
    world.spawn((
        Transform::IDENTITY,
        Sprite::new("green").with_size(Vec2::splat(4.0)).with_anchor(Vec2::splat(0.5)).with_material(mask).unlit(),
    ));
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    expect_px(&image, 64, 64, [0, 255, 0, 255], 2, "a mask still occludes under perspective");
}

#[test]
fn a_shared_material_revision_does_not_allocate_a_new_texture_image() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    let texture = insert_sprite(&mut assets, "white", linear([255, 255, 255, 255]));
    let other = assets.insert("other", linear([0, 0, 255, 255])).unwrap();
    let material = assets.insert("shared", MaterialAsset::unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let mut world = ortho();
    for i in 0..4 {
        world.spawn((Transform::from_xyz(i as f32 * 0.01, 0.0, 0.0), sprite("white", material)));
    }
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    expect_px(&image, 64, 64, [255, 0, 0, 255], 2, "shared red material");
    let creates = renderer.resource_stats().texture_creates;
    assert_eq!(renderer.cache_stats().textures, 1);
    assert_eq!(creates, 3, "two placeholders plus one sprite texture");

    assets.insert("shared", MaterialAsset::unlit(Color::rgb(0.0, 1.0, 0.0))).unwrap();
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    expect_px(&image, 64, 64, [0, 255, 0, 255], 2, "the next frame uses the replaced colour");
    assert_eq!(renderer.resource_stats().texture_creates, creates, "a colour change does not allocate an image");

    let mut pointed = MaterialAsset::unlit(Color::WHITE);
    pointed.albedo = Some(other);
    assets.insert("shared", pointed).unwrap();
    // Upload the other image once, then show a later material edit does not upload it again.
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    expect_px(&image, 64, 64, [0, 0, 255, 255], 2, "the replaced albedo is sampled");
    let creates = renderer.resource_stats().texture_creates;
    assert_eq!(creates, 4);
    assets.insert("shared", MaterialAsset::unlit(Color::rgb(0.0, 0.0, 1.0))).unwrap();
    let mut restored = *assets.get(material).unwrap();
    restored.albedo = Some(texture);
    assets.insert("shared", restored).unwrap();
    let _ = render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(
        renderer.resource_stats().texture_creates,
        creates,
        "switching back to a cached albedo allocates nothing"
    );

    let handle = assets.texture("white-tex");
    let mut sampling = assets.get(handle).unwrap().clone();
    sampling.sampler =
        SamplerDesc { min_filter: TextureFilter::Linear, mag_filter: TextureFilter::Linear, ..sampling.sampler };
    assets.insert("white-tex", sampling).unwrap();
    let _ = render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(renderer.resource_stats().texture_creates, creates, "a sampler change does not allocate an image");
}

#[test]
fn a_material_can_reference_an_atlas_without_repacking_it() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut builder = AtlasBuilder::new(AtlasConfig::new(2, 1).with_color_space(ColorSpace::Linear));
    builder.add("left", 1, 1, &[255, 0, 0, 255]).unwrap();
    builder.add("right", 1, 1, &[0, 0, 255, 255]).unwrap();
    let atlas = builder.build().unwrap();
    let mut assets = AssetManager::in_memory();
    let texture = assets.insert("atlas", atlas.texture().clone()).unwrap();
    let region = atlas.regions().iter().find(|region| region.name == "right").unwrap();
    assets.insert("right", atlas.sprite("right", texture).unwrap()).unwrap();
    let mut material = MaterialAsset::unlit(Color::WHITE);
    material.albedo = Some(texture);
    let material = assets.insert("atlas-mat", material).unwrap();
    let mut world = ortho();
    world.spawn((Transform::IDENTITY, sprite("right", material).fixed()));
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    expect_px(&image, 64, 64, [0, 0, 255, 255], 2, "the material samples the packed right region");
    assert_eq!(region.uv.min.x, 0.5);
    assert_eq!(renderer.cache_stats().textures, 1, "the atlas image is not duplicated");
}

#[test]
fn compatible_material_sprites_share_a_draw_and_blends_are_not_reordered_to_batch() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "white", linear([255, 255, 255, 255]));
    insert_sprite(&mut assets, "red", linear([255, 0, 0, 255]));
    let material = assets.insert("flat", MaterialAsset::unlit(Color::WHITE)).unwrap();
    let mut world = ortho();
    for i in 0..32 {
        let z = (i as f32) * 0.01;
        world.spawn((Transform::from_xyz(0.0, 0.0, z), sprite("white", material)));
    }
    let _ = render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(renderer.stats().sprite_instances, 32);
    assert_eq!(renderer.stats().draw_calls, 1, "M5-5D baseline: 32 compatible opaque sprites are one draw");

    let glass = assets.insert("glass", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::Blend)).unwrap();
    let mut blended = ortho();
    blended.spawn((Transform::from_xyz(0.0, 0.0, 2.0), sprite("white", glass)));
    blended.spawn((
        Transform::from_xyz(0.0, 0.0, 1.0),
        Sprite::new("red").with_size(Vec2::splat(2.0)).with_anchor(Vec2::splat(0.5)).unlit(),
    ));
    blended.spawn((Transform::IDENTITY, sprite("white", glass)));
    let _ = render(&gpu, &mut renderer, &mut blended, &assets);
    assert_eq!(renderer.stats().sprite_instances, 3);
    assert_eq!(renderer.stats().draw_calls, 3, "alternating blended textures stay in depth order");
}

#[test]
fn a_nearer_opaque_sprite_wins_even_when_its_instance_is_drawn_first() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "white", linear([255, 255, 255, 255]));
    let material = assets.insert("flat", MaterialAsset::unlit(Color::WHITE)).unwrap();
    let mut world = ortho();
    world.spawn((Transform::IDENTITY, sprite("white", material).with_tint(Color::rgb(0.0, 1.0, 0.0))));
    world.spawn((Transform::from_xyz(0.0, 0.0, 1.0), sprite("white", material).with_tint(Color::rgb(1.0, 0.0, 0.0))));
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    expect_px(&image, 64, 64, [0, 255, 0, 255], 2, "the farther red instance does not cover the nearer green one");
    assert_eq!(renderer.stats().draw_calls, 1);
}
