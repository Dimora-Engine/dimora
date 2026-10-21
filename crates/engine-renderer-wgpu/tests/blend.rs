//! GPU readback for blended meshes in the same transparent sequence as blended sprites.
//!
//! The renderer is linear (`Rgba8Unorm`), so a stored byte is the shader value times 255.
//! Straight-alpha blends use a tolerance of 3 to absorb that rounding. A result that would also
//! pass if every sprite were drawn after every mesh is not accepted as proof of the global order.

use engine_assets::{
    AlphaMode, AssetManager, ColorSpace, MaterialAsset, MeshData, SamplerDesc, SpriteAsset, Texture, TextureFilter,
    Vertex,
};
use engine_core::Color;
use engine_math::{Quat, Transform, Vec2, Vec3};
use engine_renderer::*;
use engine_renderer_wgpu::{GpuContext, SceneRenderer};
use engine_world::{propagate_transforms, World};

const W: u32 = 128;
const H: u32 = 128;
const REQUIRE_GPU_VAR: &str = "DIMORA_REQUIRE_GPU";

fn gpu() -> Option<GpuContext> {
    match GpuContext::headless() {
        Ok(gpu) => {
            let info = gpu.adapter.get_info();
            eprintln!("GPU test adapter: {} ({:?}, {:?})", info.name, info.backend, info.device_type);
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

fn expect_px(img: &[u8], expected: [u8; 4], tolerance: i16, what: &str) {
    let i = ((64 * W + 64) * 4) as usize;
    let pixel = [img[i], img[i + 1], img[i + 2], img[i + 3]];
    let close = pixel.iter().zip(expected).all(|(got, want)| (*got as i16 - want as i16).abs() <= tolerance);
    assert!(close, "{what}: centre is {pixel:?}, expected {expected:?} ±{tolerance}");
}

fn linear(rgba: [u8; 4]) -> Texture {
    Texture::solid(1, 1, rgba).with_color_space(ColorSpace::Linear)
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

fn card() -> MeshData {
    let n = Vec3::new(0.0, 0.0, -1.0);
    MeshData::new(
        vec![
            Vertex::new(Vec3::new(-1.0, -1.0, 0.0), n, [0.0, 1.0]),
            Vertex::new(Vec3::new(1.0, -1.0, 0.0), n, [1.0, 1.0]),
            Vertex::new(Vec3::new(1.0, 1.0, 0.0), n, [1.0, 0.0]),
            Vertex::new(Vec3::new(-1.0, 1.0, 0.0), n, [0.0, 0.0]),
        ],
        vec![0, 1, 2, 0, 2, 3],
    )
}

fn blend(color: Color) -> MaterialAsset {
    MaterialAsset::unlit(color).with_alpha_mode(AlphaMode::Blend)
}

fn sprite(name: &str, material: engine_assets::AssetHandle<MaterialAsset>) -> Sprite {
    Sprite::new(name).with_size(Vec2::splat(2.0)).with_anchor(Vec2::splat(0.5)).with_material(material).fixed()
}

fn insert_sprite(assets: &mut AssetManager, name: &str, texture: Texture) {
    let texture = assets.insert(&format!("{name}-tex"), texture).unwrap();
    assets.insert(name, SpriteAsset::whole_texture(texture, 1, 1).with_pixels_per_unit(1.0)).unwrap();
}

trait Place {
    fn place(self, world: &mut World, z: f32);
}

impl Place for Mesh {
    fn place(self, world: &mut World, z: f32) {
        world.spawn((Transform::from_xyz(0.0, 0.0, z), self));
    }
}

impl Place for Sprite {
    fn place(self, world: &mut World, z: f32) {
        world.spawn((Transform::from_xyz(0.0, 0.0, z), self));
    }
}

fn at(world: &mut World, z: f32, item: impl Place) {
    item.place(world, z);
}

#[test]
fn blended_mesh_sprite_mesh_order_is_the_submitted_order() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    assets.insert("card", card()).unwrap();
    insert_sprite(&mut assets, "white", linear([255, 255, 255, 255]));
    let far = assets.insert("far", blend(Color::rgba(0.0, 0.0, 1.0, 1.0))).unwrap();
    let mid = assets.insert("mid", blend(Color::rgba(0.0, 1.0, 0.0, 0.5))).unwrap();
    let front = assets.insert("front", blend(Color::rgba(1.0, 0.0, 0.0, 0.5))).unwrap();

    // Far blue mesh, middle green sprite, near red mesh.
    // Correct over black: green 0.5 over blue, then red 0.5 → about (128, 64, 64).
    // Every sprite after every mesh → about (64, 128, 64).
    // Every mesh after every sprite → about (128, 0, 128).
    let mut between = ortho();
    at(&mut between, 2.0, Mesh::new("card").with_material_asset(far));
    at(&mut between, 1.0, sprite("white", mid));
    at(&mut between, 0.0, Mesh::new("card").with_material_asset(front));
    let image = render(&gpu, &mut renderer, &mut between, &assets);
    expect_px(&image, [128, 64, 64, 255], 3, "back mesh, middle sprite, front mesh");
    assert_eq!(renderer.stats().draw_calls, 3, "the sprite splits the two mesh draws");

    // Sprite in front of both meshes. All-sprites-last matches this pose because the sprite
    // really is last. All-meshes-after-sprites does not: that composite is about (128, 0, 128).
    let mut ahead = ortho();
    at(&mut ahead, 2.0, Mesh::new("card").with_material_asset(far));
    at(&mut ahead, 1.0, Mesh::new("card").with_material_asset(front));
    at(&mut ahead, 0.0, sprite("white", mid));
    let image = render(&gpu, &mut renderer, &mut ahead, &assets);
    expect_px(&image, [64, 128, 64, 255], 3, "sprite in front of both meshes");
}

#[test]
fn blended_meshes_depth_test_without_writing_depth() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    assets.insert("card", card()).unwrap();
    insert_sprite(&mut assets, "white", linear([255, 255, 255, 255]));
    let red = assets.insert("red", blend(Color::rgba(1.0, 0.0, 0.0, 0.5))).unwrap();
    let blue = assets.insert("blue", blend(Color::rgb(0.0, 0.0, 1.0))).unwrap();
    let opaque = assets.insert("opaque", MaterialAsset::unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let keep = assets
        .insert("keep", MaterialAsset::unlit(Color::rgb(0.0, 1.0, 0.0)).with_alpha_mode(AlphaMode::mask(0.4).unwrap()))
        .unwrap();
    let drop = assets
        .insert(
            "drop",
            MaterialAsset::unlit(Color::rgba(0.0, 1.0, 0.0, 0.2)).with_alpha_mode(AlphaMode::mask(0.5).unwrap()),
        )
        .unwrap();

    let mut hidden = ortho();
    at(&mut hidden, 1.0, Mesh::new("card").with_material_asset(blue));
    at(&mut hidden, 0.0, Mesh::new("card").with_material_asset(opaque));
    let image = render(&gpu, &mut renderer, &mut hidden, &assets);
    expect_px(&image, [255, 0, 0, 255], 2, "a nearer opaque mesh occludes a blended mesh");

    let mut masked = ortho();
    at(&mut masked, 1.0, Mesh::new("card").with_material_asset(blue));
    at(&mut masked, 0.0, Mesh::new("card").with_material_asset(keep));
    let image = render(&gpu, &mut renderer, &mut masked, &assets);
    expect_px(&image, [0, 255, 0, 255], 2, "a kept mask fragment occludes a blended mesh");

    let mut hole = ortho();
    at(&mut hole, 1.0, Mesh::new("card").with_material_asset(blue));
    at(&mut hole, 0.0, Mesh::new("card").with_material_asset(drop));
    let image = render(&gpu, &mut renderer, &mut hole, &assets);
    expect_px(&image, [0, 0, 255, 255], 2, "a discarded mask fragment does not occlude");

    let mut glass = ortho();
    at(&mut glass, 1.0, Mesh::new("card").with_material_asset(blue));
    at(&mut glass, 0.0, Mesh::new("card").with_material_asset(red));
    let image = render(&gpu, &mut renderer, &mut glass, &assets);
    expect_px(&image, [128, 0, 128, 255], 3, "the farther blend stays visible through the nearer one");

    // The nearer blend is layer 0, so it is drawn first. The farther blend is layer 1 and is
    // drawn afterwards. It still replaces the pixel, which is only possible if the first blend
    // did not write depth.
    let mut layers = ortho();
    layers.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(red), RenderOrder::new(0, 0)));
    layers.spawn((
        Transform::from_xyz(0.0, 0.0, 2.0),
        Mesh::new("card").with_material_asset(blue),
        RenderOrder::new(1, 0),
    ));
    let image = render(&gpu, &mut renderer, &mut layers, &assets);
    expect_px(&image, [0, 0, 255, 255], 2, "a later farther blend is not hidden by an earlier nearer blend");
}

#[test]
fn blended_mesh_materials_honour_colour_alpha_lighting_and_culling() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    assets.insert("card", card()).unwrap();
    let green = assets.insert("green-tex", linear([0, 255, 0, 128])).unwrap();
    let solid = assets.insert("solid", blend(Color::rgba(1.0, 0.0, 0.0, 0.5))).unwrap();
    let mut textured_asset = blend(Color::rgba(1.0, 1.0, 1.0, 1.0));
    textured_asset.albedo = Some(green);
    let textured = assets.insert("textured", textured_asset).unwrap();
    let mut combined_asset = blend(Color::rgba(1.0, 1.0, 1.0, 0.5));
    combined_asset.albedo = Some(green);
    let combined = assets.insert("combined", combined_asset).unwrap();
    let flat = assets.insert("flat", blend(Color::WHITE)).unwrap();
    let lit = assets.insert("lit", MaterialAsset::color(Color::WHITE).with_alpha_mode(AlphaMode::Blend)).unwrap();
    let both = assets.insert("both", blend(Color::rgb(0.0, 0.0, 1.0)).double_sided()).unwrap();
    let mut missing_asset = blend(Color::rgb(1.0, 0.0, 0.0));
    missing_asset.albedo = Some(engine_assets::AssetHandle::from_name("absent"));
    let missing = assets.insert("missing", missing_asset).unwrap();

    let mut base = ortho();
    at(&mut base, 1.0, Mesh::new("card").with_material(Material::unlit(Color::rgb(0.0, 0.0, 1.0))));
    at(&mut base, 0.0, Mesh::new("card").with_material_asset(solid));
    let image = render(&gpu, &mut renderer, &mut base, &assets);
    expect_px(&image, [128, 0, 128, 255], 3, "untextured base colour alpha 0.5 over blue");

    let mut albedo = ortho();
    at(&mut albedo, 1.0, Mesh::new("card").with_material(Material::unlit(Color::rgb(1.0, 0.0, 0.0))));
    at(&mut albedo, 0.0, Mesh::new("card").with_material_asset(textured));
    let image = render(&gpu, &mut renderer, &mut albedo, &assets);
    expect_px(&image, [127, 128, 0, 255], 3, "texture alpha 128/255 over red");

    let mut both_alpha = ortho();
    at(&mut both_alpha, 1.0, Mesh::new("card").with_material(Material::unlit(Color::rgb(1.0, 0.0, 0.0))));
    at(&mut both_alpha, 0.0, Mesh::new("card").with_material_asset(combined));
    let image = render(&gpu, &mut renderer, &mut both_alpha, &assets);
    expect_px(&image, [191, 64, 0, 255], 3, "texture alpha times material alpha 0.5");

    let mut dark = ortho();
    dark.insert_resource(AmbientLight { color: Color::WHITE, intensity: 0.0 });
    at(&mut dark, 0.0, Mesh::new("card").with_material_asset(lit));
    let image = render(&gpu, &mut renderer, &mut dark, &assets);
    expect_px(&image, [0, 0, 0, 255], 2, "a lit blend mesh is black in zero ambient");

    let mut unlit = ortho();
    unlit.insert_resource(AmbientLight { color: Color::WHITE, intensity: 0.0 });
    at(&mut unlit, 0.0, Mesh::new("card").with_material_asset(flat));
    let image = render(&gpu, &mut renderer, &mut unlit, &assets);
    expect_px(&image, [255, 255, 255, 255], 2, "an unlit blend mesh ignores zero ambient");

    let mut shaded = ortho();
    shaded.insert_resource(AmbientLight { color: Color::WHITE, intensity: 0.5 });
    at(&mut shaded, 1.0, Mesh::new("card").with_material(Material::unlit(Color::rgb(1.0, 0.0, 0.0))));
    at(
        &mut shaded,
        0.0,
        Mesh::new("card").with_material_asset(
            assets
                .insert(
                    "half-lit",
                    MaterialAsset::color(Color::rgba(1.0, 1.0, 1.0, 0.5)).with_alpha_mode(AlphaMode::Blend),
                )
                .unwrap(),
        ),
    );
    let image = render(&gpu, &mut renderer, &mut shaded, &assets);
    expect_px(&image, [191, 64, 64, 255], 3, "lighting scales rgb and leaves alpha at 0.5");

    let mut front = ortho();
    at(&mut front, 0.0, Mesh::new("card").with_material_asset(flat));
    let image = render(&gpu, &mut renderer, &mut front, &assets);
    expect_px(&image, [255, 255, 255, 255], 2, "the front of a single-sided blend card is visible");

    let mut back = back_view();
    at(&mut back, 0.0, Mesh::new("card").with_material_asset(flat));
    let image = render(&gpu, &mut renderer, &mut back, &assets);
    expect_px(&image, [0, 0, 0, 255], 2, "double_sided false culls the back of a blend card");

    let mut back_both = back_view();
    at(&mut back_both, 0.0, Mesh::new("card").with_material_asset(both));
    let image = render(&gpu, &mut renderer, &mut back_both, &assets);
    expect_px(&image, [0, 0, 255, 255], 2, "double_sided true draws the back of a blend card");

    let mut gone = ortho();
    at(&mut gone, 0.0, Mesh::new("card").with_material_asset(missing));
    let image = render(&gpu, &mut renderer, &mut gone, &assets);
    expect_px(&image, [255, 0, 0, 255], 2, "a missing mesh albedo samples the white placeholder, not magenta");

    assets.insert("cube", MeshData::cuboid(Vec3::splat(1.0)).unwrap()).unwrap();
    let mut cube = world(Camera::perspective(50.0), Transform::from_xyz(0.0, 0.0, -4.0));
    cube.spawn((Transform::IDENTITY, Mesh::new("cube").with_material(Material::unlit(Color::rgb(0.0, 0.0, 1.0)))));
    let image = render(&gpu, &mut renderer, &mut cube, &assets);
    expect_px(&image, [0, 0, 255, 255], 2, "an unassigned mesh stays opaque");
}

#[test]
fn render_order_and_stable_ties_are_visible_on_the_gpu() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    assets.insert("card", card()).unwrap();
    insert_sprite(&mut assets, "white", linear([255, 255, 255, 255]));
    let red = assets.insert("red", blend(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let green = assets.insert("green", blend(Color::rgb(0.0, 1.0, 0.0))).unwrap();
    let blue = assets.insert("blue", blend(Color::rgb(0.0, 0.0, 1.0))).unwrap();

    let mut layered = ortho();
    layered.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(blue), RenderOrder::new(0, 0)));
    layered.spawn((
        Transform::from_xyz(0.0, 0.0, 2.0),
        Mesh::new("card").with_material_asset(red),
        RenderOrder::new(1, 0),
    ));
    let image = render(&gpu, &mut renderer, &mut layered, &assets);
    expect_px(&image, [255, 0, 0, 255], 2, "layer 1 is drawn after layer 0 even though it is farther");

    let mut depth = ortho();
    at(&mut depth, 2.0, Mesh::new("card").with_material_asset(blue));
    at(&mut depth, 0.0, Mesh::new("card").with_material_asset(red));
    let image = render(&gpu, &mut renderer, &mut depth, &assets);
    expect_px(&image, [255, 0, 0, 255], 2, "within one layer the nearer blend is drawn last");

    let mut ties = ortho();
    ties.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(red), RenderOrder::new(0, 5)));
    ties.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(blue), RenderOrder::new(0, 1)));
    let image = render(&gpu, &mut renderer, &mut ties, &assets);
    expect_px(&image, [255, 0, 0, 255], 2, "the higher order is drawn last when depth and layer match");

    let mut stable = ortho();
    at(&mut stable, 0.0, Mesh::new("card").with_material_asset(blue));
    at(&mut stable, 0.0, Mesh::new("card").with_material_asset(red));
    let image = render(&gpu, &mut renderer, &mut stable, &assets);
    expect_px(&image, [255, 0, 0, 255], 2, "the later extracted mesh wins an equal key");

    let mut pair = ortho();
    at(&mut pair, 0.0, Mesh::new("card").with_material_asset(red));
    at(&mut pair, 0.0, sprite("white", green));
    let image = render(&gpu, &mut renderer, &mut pair, &assets);
    expect_px(&image, [255, 0, 0, 255], 2, "a mesh tied with a sprite is drawn after that sprite");

    let mut mixed = ortho();
    at(
        &mut mixed,
        2.0,
        Sprite::new("white")
            .with_size(Vec2::splat(2.0))
            .with_anchor(Vec2::splat(0.5))
            .with_blend(SpriteBlend::Alpha)
            .with_tint(Color::rgb(0.0, 0.0, 1.0))
            .fixed(),
    );
    at(&mut mixed, 1.0, sprite("white", green).with_tint(Color::rgba(0.0, 1.0, 0.0, 0.5)));
    let red_glass = assets.insert("red-glass", blend(Color::rgba(1.0, 0.0, 0.0, 0.5))).unwrap();
    at(&mut mixed, 0.0, Mesh::new("card").with_material_asset(red_glass));
    let image = render(&gpu, &mut renderer, &mut mixed, &assets);
    // Blue, then green 0.5 → (0, 0.5, 0.5), then red 0.5 → (0.5, 0.25, 0.25).
    // Sprites after the mesh would be about (0, 128, 128). Green after red would be about (64, 128, 64).
    expect_px(&image, [128, 64, 64, 255], 3, "legacy sprite, material sprite, then blend mesh");
}

#[test]
fn projection_does_not_change_blend_class_and_a_turn_reverses_pixels() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    assets.insert("card", card()).unwrap();
    let glass = assets.insert("glass", blend(Color::rgba(1.0, 0.0, 0.0, 0.5)).double_sided()).unwrap();
    let pitch = (1.0_f32 / 2.0_f32.sqrt()).atan();
    let rotation = Quat::from_rotation_y(core::f32::consts::FRAC_PI_4) * Quat::from_rotation_x(pitch);
    let forward = Transform::from_rotation(rotation).forward();
    let views = [
        ("perspective", Camera::perspective(60.0), Transform::from_xyz(0.0, 0.0, -4.0)),
        ("orthographic", Camera::orthographic(4.0), Transform::from_xyz(0.0, 0.0, -4.0)),
        ("isometric", Camera::orthographic(4.0), Transform::from_position(-forward * 6.0).with_rotation(rotation)),
    ];
    for (name, camera, pose) in views {
        let mut world = world(camera, pose);
        world.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(glass)));
        let image = render(&gpu, &mut renderer, &mut world, &assets);
        expect_px(&image, [128, 0, 0, 255], 3, name);
    }

    let red = assets.insert("red", blend(Color::rgb(1.0, 0.0, 0.0)).double_sided()).unwrap();
    let blue = assets.insert("blue", blend(Color::rgb(0.0, 0.0, 1.0)).double_sided()).unwrap();
    let mut looking_ahead = world(Camera::orthographic(4.0), Transform::from_xyz(0.0, 0.0, -4.0));
    at(&mut looking_ahead, 0.0, Mesh::new("card").with_material_asset(red));
    at(&mut looking_ahead, 2.0, Mesh::new("card").with_material_asset(blue));
    let image = render(&gpu, &mut renderer, &mut looking_ahead, &assets);
    expect_px(&image, [255, 0, 0, 255], 2, "looking along +Z, the z=0 card is nearer");

    let mut looking_back = world(
        Camera::orthographic(4.0),
        Transform::from_xyz(0.0, 0.0, 6.0).with_rotation(Quat::from_rotation_y(core::f32::consts::PI)),
    );
    at(&mut looking_back, 0.0, Mesh::new("card").with_material_asset(red));
    at(&mut looking_back, 2.0, Mesh::new("card").with_material_asset(blue));
    let image = render(&gpu, &mut renderer, &mut looking_back, &assets);
    expect_px(&image, [0, 0, 255, 255], 2, "looking along -Z, the z=2 card is nearer");
}

#[test]
fn material_revisions_change_the_pipeline_without_a_new_image() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    assets.insert("card", card()).unwrap();
    let image_tex = assets.insert("albedo", linear([0, 255, 0, 255])).unwrap();
    let mut shared_asset = blend(Color::WHITE);
    shared_asset.albedo = Some(image_tex);
    let shared = assets.insert("shared", shared_asset).unwrap();
    let mut world = ortho();
    at(&mut world, 0.0, Mesh::new("card").with_material_asset(shared));
    at(&mut world, 0.2, Mesh::new("card").with_material_asset(shared));
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    expect_px(&image, [0, 255, 0, 255], 2, "two blend meshes share one albedo");
    let creates = renderer.resource_stats().texture_creates;
    let uploads = renderer.resource_stats().full_base_uploads;
    let samplers = renderer.resource_stats().sampler_creates;
    assert_eq!(renderer.cache_stats().textures, 1);
    assert_eq!(creates, 3, "two placeholders plus the shared albedo");
    assert_eq!(samplers, 1);

    assets.insert("shared", blend(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    expect_px(&image, [255, 0, 0, 255], 2, "a colour-only revision drops the albedo and changes the pixel");
    assert_eq!(renderer.resource_stats().texture_creates, creates);
    assert_eq!(renderer.resource_stats().full_base_uploads, uploads);

    // Near subject, far probe on a higher layer. Blend lets the probe through. Opaque and mask hide it.
    let probe = assets.insert("probe", blend(Color::rgb(0.0, 0.0, 1.0))).unwrap();
    let mut posed = ortho();
    posed.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(shared), RenderOrder::new(0, 0)));
    posed.spawn((
        Transform::from_xyz(0.0, 0.0, 2.0),
        Mesh::new("card").with_material_asset(probe),
        RenderOrder::new(1, 0),
    ));
    assets.insert("shared", blend(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let image = render(&gpu, &mut renderer, &mut posed, &assets);
    expect_px(&image, [0, 0, 255, 255], 2, "blend does not write depth, so the higher layer still appears");

    assets.insert("shared", MaterialAsset::unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let image = render(&gpu, &mut renderer, &mut posed, &assets);
    expect_px(&image, [255, 0, 0, 255], 2, "opaque writes depth and hides the later blend");

    assets.insert("shared", blend(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let image = render(&gpu, &mut renderer, &mut posed, &assets);
    expect_px(&image, [0, 0, 255, 255], 2, "returning to blend stops writing depth");

    assets
        .insert(
            "shared",
            MaterialAsset::unlit(Color::rgb(0.0, 1.0, 0.0)).with_alpha_mode(AlphaMode::mask(0.4).unwrap()),
        )
        .unwrap();
    let image = render(&gpu, &mut renderer, &mut posed, &assets);
    expect_px(&image, [0, 255, 0, 255], 2, "mask writes depth and hides the later blend");

    assets.insert("shared", blend(Color::rgb(0.0, 0.0, 1.0))).unwrap();
    let _ = render(&gpu, &mut renderer, &mut posed, &assets);
    assets.insert("shared", blend(Color::rgb(0.0, 0.0, 1.0)).double_sided()).unwrap();
    let mut back = back_view();
    back.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(shared)));
    let image = render(&gpu, &mut renderer, &mut back, &assets);
    expect_px(&image, [0, 0, 255, 255], 2, "a double_sided revision shows the back face");
    assert_eq!(renderer.resource_stats().texture_creates, creates, "class and cull changes allocate no image");

    let mut sampling = assets.get(image_tex).unwrap().clone();
    sampling.sampler =
        SamplerDesc { min_filter: TextureFilter::Linear, mag_filter: TextureFilter::Linear, ..sampling.sampler };
    assets.insert("albedo", sampling).unwrap();
    let mut sampled = ortho();
    let mut again_asset = blend(Color::WHITE);
    again_asset.albedo = Some(image_tex);
    let again = assets.insert("again", again_asset).unwrap();
    at(&mut sampled, 0.0, Mesh::new("card").with_material_asset(again));
    let _ = render(&gpu, &mut renderer, &mut sampled, &assets);
    assert!(renderer.resource_stats().sampler_creates > samplers, "a new sampler descriptor is cached");
    assert_eq!(renderer.resource_stats().texture_creates, creates, "the sampler change does not allocate an image");

    assets.remove(image_tex);
    let mut empty = ortho();
    let _ = render(&gpu, &mut renderer, &mut empty, &assets);
    assert_eq!(renderer.cache_stats().textures, 0, "removing the albedo evicts the GPU image");
}

#[test]
fn transparent_draw_counts_follow_the_sequence() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    assets.insert("card", card()).unwrap();
    insert_sprite(&mut assets, "white", linear([255, 255, 255, 255]));
    let glass = assets.insert("glass", blend(Color::rgba(1.0, 0.0, 0.0, 0.5))).unwrap();
    let flat = assets.insert("flat", MaterialAsset::unlit(Color::WHITE)).unwrap();

    let mut one = ortho();
    at(&mut one, 0.0, Mesh::new("card").with_material_asset(glass));
    let _ = render(&gpu, &mut renderer, &mut one, &assets);
    assert_eq!(renderer.stats().draw_calls, 1);
    assert_eq!(renderer.stats().mesh_instances, 1);
    eprintln!("draw calls, one blended mesh: {}", renderer.stats().draw_calls);

    let mut several = ortho();
    for z in [0.0, 0.5, 1.0, 1.5] {
        at(&mut several, z, Mesh::new("card").with_material_asset(glass));
    }
    let _ = render(&gpu, &mut renderer, &mut several, &assets);
    assert_eq!(renderer.stats().draw_calls, 1, "four compatible blend meshes are one run");
    assert_eq!(renderer.stats().mesh_instances, 4);
    eprintln!("draw calls, four compatible blended meshes: {}", renderer.stats().draw_calls);

    let mut split = ortho();
    at(&mut split, 2.0, Mesh::new("card").with_material_asset(glass));
    at(&mut split, 1.0, sprite("white", glass));
    at(&mut split, 0.0, Mesh::new("card").with_material_asset(glass));
    let _ = render(&gpu, &mut renderer, &mut split, &assets);
    assert_eq!(renderer.stats().draw_calls, 3, "a sprite between two meshes is three draws");
    eprintln!("draw calls, sprite between two blended meshes: {}", renderer.stats().draw_calls);

    let mut alternate = ortho();
    at(&mut alternate, 3.0, Mesh::new("card").with_material_asset(glass));
    at(&mut alternate, 2.0, sprite("white", glass));
    at(&mut alternate, 1.0, Mesh::new("card").with_material_asset(glass));
    at(&mut alternate, 0.0, sprite("white", glass));
    let _ = render(&gpu, &mut renderer, &mut alternate, &assets);
    assert_eq!(renderer.stats().draw_calls, 4, "alternating kinds are not reordered into fewer draws");
    eprintln!("draw calls, alternating sprite and mesh: {}", renderer.stats().draw_calls);

    let mut baseline = ortho();
    for i in 0..32 {
        baseline.spawn((Transform::from_xyz(0.0, 0.0, i as f32 * 0.01), sprite("white", flat)));
    }
    let _ = render(&gpu, &mut renderer, &mut baseline, &assets);
    assert_eq!(renderer.stats().draw_calls, 1, "the 32-sprite opaque material baseline is still one draw");
    assert_eq!(renderer.stats().sprite_instances, 32);
    eprintln!("draw calls, 32 compatible opaque material sprites: {}", renderer.stats().draw_calls);
}
