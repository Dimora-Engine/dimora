//! Custom view-to-clip matrices through the normal extract and GPU path.
//!
//! The matrix is stored on `CameraProjection::Custom`. These tests do not call a second renderer.
//! Pixel checks use the linear `Rgba8Unorm` target.

use engine_assets::{AlphaMode, AssetManager, ColorSpace, MaterialAsset, MeshData, SpriteAsset, Texture, Vertex};
use engine_core::Color;
use engine_math::{Mat4, Quat, Transform, Vec2, Vec3, Vec4};
use engine_renderer::*;
use engine_renderer_wgpu::{GpuContext, SceneRenderer};
use engine_world::{propagate_transforms, GlobalTransform, World};

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

fn custom_from(projection: CameraProjection, aspect: f32) -> Camera {
    let matrix = projection.matrix(aspect);
    Camera::new(CameraProjection::custom(matrix).expect("built-in matrices are finite")).with_clear_color(Color::BLACK)
}

fn world(camera: Camera, transform: Transform) -> World {
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.spawn((camera, transform));
    world
}

fn render_size(
    gpu: &GpuContext,
    renderer: &mut SceneRenderer,
    world: &mut World,
    assets: &AssetManager,
    width: u32,
    height: u32,
) -> (Vec<u8>, RenderScene) {
    propagate_transforms(world);
    let scene = extract_scene(world, assets, Vec2::new(width as f32, height as f32)).unwrap();
    let pixels = renderer.render_to_rgba(gpu, &scene, assets, width, height).unwrap();
    (pixels, scene)
}

fn render(
    gpu: &GpuContext,
    renderer: &mut SceneRenderer,
    world: &mut World,
    assets: &AssetManager,
) -> (Vec<u8>, RenderScene) {
    render_size(gpu, renderer, world, assets, W, H)
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

fn assert_same_image(left: &[u8], right: &[u8], what: &str) {
    assert_eq!(left.len(), right.len(), "{what}");
    if let Some(index) = left.iter().zip(right).position(|(a, b)| a != b) {
        let pixel = index / 4;
        let end = index + 4;
        panic!("{what}: pixel {pixel} is {:?} vs {:?}", &left[index..end], &right[index..end]);
    }
}

fn white_sprite(assets: &mut AssetManager, name: &str) {
    let texture = assets.insert(name, Texture::solid(1, 1, [255; 4]).with_color_space(ColorSpace::Linear)).unwrap();
    assets.insert(name, SpriteAsset::whole_texture(texture, 1, 1).with_pixels_per_unit(1.0)).unwrap();
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

fn unlit(color: Color) -> MaterialAsset {
    MaterialAsset::unlit(color)
}

fn blend(color: Color) -> MaterialAsset {
    MaterialAsset::unlit(color).with_alpha_mode(AlphaMode::Blend)
}

fn sprite(name: &str, material: engine_assets::AssetHandle<MaterialAsset>, size: f32) -> Sprite {
    Sprite::new(name).with_size(Vec2::splat(size)).with_anchor(Vec2::splat(0.5)).with_material(material).fixed()
}

fn sample_projected(img: &[u8], width: u32, view: &CameraView, point: Vec3, expected: [u8; 4], what: &str) {
    let screen = view.project(point).unwrap_or_else(|| panic!("{what}: point does not project"));
    let x = screen.pixels.x.floor() as u32;
    let y = screen.pixels.y.floor() as u32;
    expect_px(img, width, x, y, expected, 2, what);
}

#[test]
fn custom_perspective_matches_builtin_pixels() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    white_sprite(&mut assets, "quad");
    assets.insert("cube", MeshData::cube(0.6).unwrap()).unwrap();
    let red = assets.insert("red", unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let spawn = |camera: Camera| {
        let mut world = world(camera, Transform::from_xyz(0.0, 0.0, -4.0));
        world.spawn((
            Transform::from_xyz(0.0, 0.0, 1.2),
            sprite("quad", red, 1.4).with_orientation(SpriteOrientation::Billboard),
        ));
        world.spawn((
            Transform::IDENTITY,
            Mesh::new("cube").with_material(Material { lit: false, ..Material::color(Color::rgb(0.0, 0.0, 1.0)) }),
        ));
        world
    };
    let builtin = Camera::perspective(60.0).with_clear_color(Color::BLACK);
    let custom = custom_from(CameraProjection::perspective(60.0), 1.0);
    let (built_in_image, _) = render(&gpu, &mut renderer, &mut spawn(builtin), &assets);
    let (custom_image, scene) = render(&gpu, &mut renderer, &mut spawn(custom), &assets);
    assert_same_image(&built_in_image, &custom_image, "custom perspective");
    assert!(scene.sole_pass().view.proj.approx_eq(&custom.projection.matrix(1.0), 1e-5));
    expect_px(&custom_image, W, 64, 64, [0, 0, 255, 255], 2, "the cube, not the sprite, covers the centre");
    assert_ne!(px(&custom_image, W, 64, 64), [0, 0, 0, 255]);
}

#[test]
fn custom_orthographic_matches_builtin_pixels() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    white_sprite(&mut assets, "quad");
    let red = assets.insert("red", unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let spawn = |camera: Camera| {
        let mut world = world(camera, Transform::from_xyz(0.0, 0.0, -4.0));
        world.spawn((Transform::from_xyz(0.25, 0.0, 0.0), sprite("quad", red, 0.4)));
        world
    };
    let builtin = Camera::orthographic(2.0).with_clear_color(Color::BLACK);
    let custom = custom_from(CameraProjection::orthographic(2.0), 1.0);
    let (built_in_image, _) = render(&gpu, &mut renderer, &mut spawn(builtin), &assets);
    let (custom_image, _) = render(&gpu, &mut renderer, &mut spawn(custom), &assets);
    assert_same_image(&built_in_image, &custom_image, "custom orthographic");
    expect_px(&custom_image, W, 80, 64, [255, 0, 0, 255], 2, "the offset sprite");
    expect_px(&custom_image, W, 40, 64, [0, 0, 0, 255], 2, "clear colour beside the sprite");
}

#[test]
fn oblique_custom_projection_matches_project_and_not_plain_ortho() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    white_sprite(&mut assets, "quad");
    assets.insert("cube", MeshData::cube(0.25).unwrap()).unwrap();
    let red = assets.insert("red", unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let blue = Material { lit: false, ..Material::color(Color::rgb(0.0, 0.0, 1.0)) };
    let ortho = CameraProjection::orthographic(2.0).matrix(1.0);
    let shear = Mat4::from_cols(
        Vec4::new(1.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 1.0, 0.0, 0.0),
        Vec4::new(0.05, 0.0, 1.0, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    );
    let oblique = Camera::new(CameraProjection::custom(ortho * shear).unwrap()).with_clear_color(Color::BLACK);
    let mut sheared = world(oblique, Transform::from_xyz(0.0, 0.0, -4.0));
    sheared.spawn((Transform::from_xyz(0.0, 0.35, 0.0), sprite("quad", red, 0.2)));
    sheared.spawn((Transform::from_xyz(0.0, -0.35, 0.2), Mesh::new("cube").with_material(blue)));
    let (image, scene) = render(&gpu, &mut renderer, &mut sheared, &assets);
    sample_projected(&image, W, &scene.sole_pass().view, Vec3::new(0.0, 0.35, 0.0), [255, 0, 0, 255], "sheared sprite");
    sample_projected(&image, W, &scene.sole_pass().view, Vec3::new(0.0, -0.35, 0.2), [0, 0, 255, 255], "sheared cube");
    let plain = CameraView::new(
        &Camera::orthographic(2.0),
        &GlobalTransform { matrix: Transform::from_xyz(0.0, 0.0, -4.0).to_matrix() },
        Vec2::new(W as f32, H as f32),
    );
    let sprite_oblique = scene.sole_pass().view.project(Vec3::new(0.0, 0.35, 0.0)).unwrap();
    let sprite_plain = plain.project(Vec3::new(0.0, 0.35, 0.0)).unwrap();
    assert!((sprite_oblique.pixels.x - sprite_plain.pixels.x).abs() > 4.0, "oblique is not orthographic");
    let plain_x = sprite_plain.pixels.x.floor() as u32;
    let plain_y = sprite_plain.pixels.y.floor() as u32;
    let there = px(&image, W, plain_x, plain_y);
    assert_ne!(there, [255, 0, 0, 255], "the orthographic pixel is not the sheared sprite");
}

#[test]
fn custom_projection_depth_and_unified_blend_order() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    white_sprite(&mut assets, "white");
    assets.insert("card", card()).unwrap();
    let red = assets.insert("red", unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let blue = assets.insert("blue", unlit(Color::rgb(0.0, 0.0, 1.0))).unwrap();
    let keep =
        assets.insert("keep", unlit(Color::rgb(0.0, 1.0, 0.0)).with_alpha_mode(AlphaMode::mask(0.4).unwrap())).unwrap();
    let drop = assets
        .insert("drop", unlit(Color::rgba(0.0, 1.0, 0.0, 0.2)).with_alpha_mode(AlphaMode::mask(0.5).unwrap()))
        .unwrap();
    let glass = assets.insert("glass", blend(Color::rgba(1.0, 0.0, 0.0, 0.5))).unwrap();
    let far = assets.insert("far", blend(Color::rgb(0.0, 0.0, 1.0))).unwrap();
    let mid = assets.insert("mid", blend(Color::rgba(0.0, 1.0, 0.0, 0.5))).unwrap();
    let front = assets.insert("front", blend(Color::rgba(1.0, 0.0, 0.0, 0.5))).unwrap();
    let camera = custom_from(CameraProjection::orthographic(2.0), 1.0);
    let mut occlude = world(camera, Transform::from_xyz(0.0, 0.0, -4.0));
    occlude.spawn((Transform::from_xyz(0.0, 0.0, 1.0), sprite("white", blue, 2.0)));
    occlude.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(red)));
    let (image, _) = render(&gpu, &mut renderer, &mut occlude, &assets);
    expect_px(&image, W, 64, 64, [255, 0, 0, 255], 2, "a nearer opaque mesh occludes a farther sprite");

    let mut masked = world(camera, Transform::from_xyz(0.0, 0.0, -4.0));
    masked.spawn((Transform::from_xyz(0.0, 0.0, 1.0), Mesh::new("card").with_material_asset(blue)));
    masked.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(keep)));
    let (image, _) = render(&gpu, &mut renderer, &mut masked, &assets);
    expect_px(&image, W, 64, 64, [0, 255, 0, 255], 2, "a kept mask fragment writes depth");

    let mut hole = world(camera, Transform::from_xyz(0.0, 0.0, -4.0));
    hole.spawn((Transform::from_xyz(0.0, 0.0, 1.0), Mesh::new("card").with_material_asset(blue)));
    hole.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(drop)));
    let (image, _) = render(&gpu, &mut renderer, &mut hole, &assets);
    expect_px(&image, W, 64, 64, [0, 0, 255, 255], 2, "a discarded mask fragment does not write depth");

    let mut glass_world = world(camera, Transform::from_xyz(0.0, 0.0, -4.0));
    glass_world.spawn((Transform::from_xyz(0.0, 0.0, 1.0), Mesh::new("card").with_material_asset(blue)));
    glass_world.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(glass)));
    let (image, _) = render(&gpu, &mut renderer, &mut glass_world, &assets);
    expect_px(&image, W, 64, 64, [128, 0, 128, 255], 3, "blend tests depth and does not replace it");

    let mut layers = world(camera, Transform::from_xyz(0.0, 0.0, -4.0));
    layers.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(glass), RenderOrder::new(0, 0)));
    layers.spawn((
        Transform::from_xyz(0.0, 0.0, 2.0),
        Mesh::new("card").with_material_asset(far),
        RenderOrder::new(1, 0),
    ));
    let (image, _) = render(&gpu, &mut renderer, &mut layers, &assets);
    expect_px(&image, W, 64, 64, [0, 0, 255, 255], 2, "an earlier nearer blend did not write depth");

    let mut between = world(camera, Transform::from_xyz(0.0, 0.0, -4.0));
    between.spawn((Transform::from_xyz(0.0, 0.0, 2.0), Mesh::new("card").with_material_asset(far)));
    between.spawn((Transform::from_xyz(0.0, 0.0, 1.0), sprite("white", mid, 2.0)));
    between.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(front)));
    let (image, _) = render(&gpu, &mut renderer, &mut between, &assets);
    expect_px(&image, W, 64, 64, [128, 64, 64, 255], 3, "camera-space order: mesh, sprite, mesh");
    assert_eq!(renderer.stats().draw_calls, 3, "the sprite splits the two mesh draws");
}

#[test]
fn custom_projection_sprite_orientations_share_one_matrix() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    white_sprite(&mut assets, "quad");
    let red = assets.insert("red", unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let camera = custom_from(CameraProjection::perspective(60.0), 1.0);
    for orientation in [SpriteOrientation::Fixed, SpriteOrientation::Billboard, SpriteOrientation::Upright] {
        let mut world = world(camera, Transform::from_xyz(0.0, 0.0, -4.0));
        world.spawn((Transform::IDENTITY, sprite("quad", red, 1.0).with_orientation(orientation)));
        let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
        sample_projected(&image, W, &scene.sole_pass().view, Vec3::ZERO, [255, 0, 0, 255], "orientation");
        assert!(scene.sole_pass().view.proj.approx_eq(&camera.projection.matrix(99.0), 1e-6));
    }
}

#[test]
fn custom_projection_camera_translation_rotation_and_parent() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    white_sprite(&mut assets, "quad");
    let red = assets.insert("red", unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let matrix = CameraProjection::orthographic(2.0).matrix(1.0);
    let camera = Camera::new(CameraProjection::custom(matrix).unwrap()).with_clear_color(Color::BLACK);

    let mut moved = world(camera, Transform::from_xyz(0.35, 0.0, -4.0));
    moved.spawn((Transform::IDENTITY, sprite("quad", red, 0.3)));
    let (moved_image, moved_scene) = render(&gpu, &mut renderer, &mut moved, &assets);
    sample_projected(&moved_image, W, &moved_scene.sole_pass().view, Vec3::ZERO, [255, 0, 0, 255], "translated camera");
    expect_px(&moved_image, W, 64, 64, [0, 0, 0, 255], 2, "translation moves the sprite off the centre");
    assert!(moved_scene.sole_pass().view.proj.approx_eq(&matrix, 0.0));

    let turned = Camera::new(CameraProjection::custom(CameraProjection::perspective(60.0).matrix(1.0)).unwrap())
        .with_clear_color(Color::BLACK);
    let mut yawed =
        world(turned, Transform::IDENTITY.with_rotation(Quat::from_rotation_y(core::f32::consts::FRAC_PI_2)));
    yawed.spawn((
        Transform::from_xyz(6.0, 0.0, 0.0),
        sprite("quad", red, 1.2).with_orientation(SpriteOrientation::Billboard),
    ));
    let (yawed_image, yawed_scene) = render(&gpu, &mut renderer, &mut yawed, &assets);
    assert!(yawed_scene.sole_pass().view.forward.approx_eq(Vec3::X, 1e-4));
    sample_projected(
        &yawed_image,
        W,
        &yawed_scene.sole_pass().view,
        Vec3::new(6.0, 0.0, 0.0),
        [255, 0, 0, 255],
        "rotated camera",
    );

    let mut child = World::new();
    child.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    let parent = child.spawn((Transform::from_xyz(0.35, 0.0, 0.0),));
    child.spawn_child(parent, (camera, Transform::from_xyz(0.0, 0.0, -4.0)));
    child.spawn((Transform::IDENTITY, sprite("quad", red, 0.3)));
    let (child_image, child_scene) = render(&gpu, &mut renderer, &mut child, &assets);
    assert!(
        moved_scene.sole_pass().view.position.approx_eq(child_scene.sole_pass().view.position, 1e-3),
        "camera {:?} vs parented {:?}",
        moved_scene.sole_pass().view.position,
        child_scene.sole_pass().view.position
    );
    assert!(moved_scene.sole_pass().view.view_proj.approx_eq(&child_scene.sole_pass().view.view_proj, 1e-4));
    assert_same_image(&moved_image, &child_image, "parent transform matches a translated camera");
    assert!(child_scene.sole_pass().view.proj.approx_eq(&matrix, 0.0));
}

#[test]
fn custom_projection_viewport_resize_keeps_the_matrix() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    white_sprite(&mut assets, "quad");
    let red = assets.insert("red", unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let baked = CameraProjection::orthographic(2.0).matrix(1.0);
    let custom = Camera::new(CameraProjection::custom(baked).unwrap()).with_clear_color(Color::BLACK);
    let mut scene_world = world(custom, Transform::from_xyz(0.0, 0.0, -4.0));
    let point = Vec3::new(0.5, 0.0, 0.0);
    scene_world.spawn((Transform::from_position(point), sprite("quad", red, 0.08)));

    let (square, square_scene) = render_size(&gpu, &mut renderer, &mut scene_world, &assets, 128, 128);
    let (wide, wide_scene) = render_size(&gpu, &mut renderer, &mut scene_world, &assets, 256, 128);
    assert!(square_scene.sole_pass().view.proj.approx_eq(&baked, 0.0));
    assert!(wide_scene.sole_pass().view.proj.approx_eq(&square_scene.sole_pass().view.proj, 0.0));
    let square_ndc = square_scene.sole_pass().view.project(point).unwrap().ndc;
    let wide_ndc = wide_scene.sole_pass().view.project(point).unwrap().ndc;
    assert!(square_ndc.approx_eq(wide_ndc, 1e-5), "a resize does not rebuild the custom matrix");
    sample_projected(&square, 128, &square_scene.sole_pass().view, point, [255, 0, 0, 255], "square viewport");
    sample_projected(&wide, 256, &wide_scene.sole_pass().view, point, [255, 0, 0, 255], "wide viewport");

    let perspective = Camera::perspective(90.0).with_clear_color(Color::BLACK);
    let placed = GlobalTransform { matrix: Transform::from_xyz(0.0, 0.0, -4.0).to_matrix() };
    let narrow_proj = CameraView::new(&perspective, &placed, Vec2::new(128.0, 128.0)).proj;
    let wide_proj = CameraView::new(&perspective, &placed, Vec2::new(256.0, 128.0)).proj;
    assert!(!narrow_proj.approx_eq(&wide_proj, 1e-3), "perspective still tracks aspect");
    let mut perspective_world = world(perspective, Transform::from_xyz(0.0, 0.0, -4.0));
    perspective_world.spawn((Transform::from_position(point), sprite("quad", red, 0.08)));
    let (perspective_image, perspective_scene) =
        render_size(&gpu, &mut renderer, &mut perspective_world, &assets, 256, 128);
    let custom_x = wide_scene.sole_pass().view.project(point).unwrap().pixels.x;
    let perspective_x = perspective_scene.sole_pass().view.project(point).unwrap().pixels.x;
    assert!((custom_x - perspective_x).abs() > 8.0, "the wide custom frustum is not the wide perspective frustum");
    sample_projected(
        &perspective_image,
        256,
        &perspective_scene.sole_pass().view,
        point,
        [255, 0, 0, 255],
        "perspective on the wide viewport",
    );
}

#[test]
fn one_thousand_sprites_remain_one_draw_under_a_custom_projection() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    let image = Texture::from_fn(2, 1, |x, _| if x == 0 { [255, 0, 0, 255] } else { [0, 255, 0, 255] })
        .with_color_space(ColorSpace::Linear);
    let texture = assets.insert("atlas-tex", image).unwrap();
    assets.insert("atlas", SpriteAsset::whole_texture(texture, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    let material = assets.insert("flat", MaterialAsset::unlit(Color::WHITE)).unwrap();
    let mut world = world(custom_from(CameraProjection::orthographic(2.0), 1.0), Transform::from_xyz(0.0, 0.0, -4.0));
    let left = engine_math::Rect::new(Vec2::ZERO, Vec2::new(0.5, 1.0));
    let right = engine_math::Rect::new(Vec2::new(0.5, 0.0), Vec2::ONE);
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
    let (pixels, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(scene.stats.sprites_visible, 1000);
    assert_eq!(scene.sole_pass().opaque_sprites.batches.len(), 1);
    assert_eq!(scene.sole_pass().opaque_sprites.batches[0].range, 0..1000);
    let stats = renderer.stats();
    assert_eq!(stats.draw_calls, 1);
    assert_eq!(stats.sprite_instances, 1000);
    assert_eq!(stats.largest_draw_instances, 1000);
    expect_px(&pixels, W, 64, 64, [255, 0, 0, 255], 2, "sprite 0");
    expect_px(&pixels, W, 99, 64, [0, 255, 0, 255], 2, "sprite 1");
    expect_px(&pixels, W, 29, 64, [128, 0, 0, 255], 2, "sprite 2 tint");
}

#[test]
fn custom_frustum_culls_on_the_gpu_and_a_degenerate_plane_does_not() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    white_sprite(&mut assets, "quad");
    assets.insert("cube", MeshData::cube(0.4).unwrap()).unwrap();
    let red = assets.insert("red", unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let blue = Material { lit: false, ..Material::color(Color::rgb(0.0, 0.0, 1.0)) };

    let mut inside = world(custom_from(CameraProjection::perspective(60.0), 1.0), Transform::from_xyz(0.0, 0.0, -4.0));
    inside.spawn((Transform::IDENTITY, sprite("quad", red, 0.8)));
    inside.spawn((Transform::from_xyz(40.0, 0.0, 0.0), Mesh::new("cube").with_material(blue)));
    let (image, scene) = render(&gpu, &mut renderer, &mut inside, &assets);
    assert_eq!(scene.stats.sprites_visible, 1);
    assert_eq!(scene.stats.meshes_culled, 1);
    expect_px(&image, W, 64, 64, [255, 0, 0, 255], 2, "the inside sprite is drawn");

    let offcentre = Mat4::orthographic_lh(-0.2, 1.8, -1.0, 1.0, 0.1, 80.0);
    let mut shifted = world(
        Camera::new(CameraProjection::custom(offcentre).unwrap()).with_clear_color(Color::BLACK),
        Transform::IDENTITY,
    );
    shifted.spawn((Transform::from_xyz(0.4, 0.0, 5.0), sprite("quad", red, 0.3)));
    shifted.spawn((Transform::from_xyz(-1.2, 0.0, 5.0), sprite("quad", red, 0.15)));
    let (image, scene) = render(&gpu, &mut renderer, &mut shifted, &assets);
    assert_eq!((scene.stats.sprites_visible, scene.stats.sprites_culled), (1, 1));
    sample_projected(
        &image,
        W,
        &scene.sole_pass().view,
        Vec3::new(0.4, 0.0, 5.0),
        [255, 0, 0, 255],
        "inside the off-centre box",
    );

    let ortho = CameraProjection::orthographic(10.0).matrix(1.0);
    let shear = Mat4::from_cols(
        Vec4::new(1.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 1.0, 0.0, 0.0),
        Vec4::new(0.5, 0.0, 1.0, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    );
    let mut oblique = world(
        Camera::new(CameraProjection::custom(ortho * shear).unwrap()).with_clear_color(Color::BLACK),
        Transform::IDENTITY,
    );
    oblique.spawn((Transform::from_xyz(0.0, 0.0, 4.0), sprite("quad", red, 1.0)));
    oblique.spawn((Transform::from_xyz(4.0, 0.0, 4.0), sprite("quad", red, 0.4)));
    let (image, scene) = render(&gpu, &mut renderer, &mut oblique, &assets);
    assert_eq!((scene.stats.sprites_visible, scene.stats.sprites_culled), (1, 1));
    sample_projected(
        &image,
        W,
        &scene.sole_pass().view,
        Vec3::new(0.0, 0.0, 4.0),
        [255, 0, 0, 255],
        "inside the oblique frustum",
    );

    let mut finite_far = CameraProjection::orthographic(2.0).with_clip_range(0.1, 20.0).matrix(1.0);
    // Drop NDC z = 0 (the far plane under reverse-Z) and keep the near plane in r3 - r2.
    let r2 = [finite_far.cols[0].z, finite_far.cols[1].z, finite_far.cols[2].z, finite_far.cols[3].z];
    let r3 = [finite_far.cols[0].w, finite_far.cols[1].w, finite_far.cols[2].w, finite_far.cols[3].w];
    let near = [r3[0] - r2[0], r3[1] - r2[1], r3[2] - r2[2], r3[3] - r2[3]];
    finite_far.cols[0].z = 0.0;
    finite_far.cols[1].z = 0.0;
    finite_far.cols[2].z = 0.0;
    finite_far.cols[3].z = 1.0;
    finite_far.cols[0].w = near[0];
    finite_far.cols[1].w = near[1];
    finite_far.cols[2].w = near[2];
    finite_far.cols[3].w = near[3] + 1.0;
    let mut open = world(
        Camera::new(CameraProjection::custom(finite_far).unwrap()).with_clear_color(Color::BLACK),
        Transform::from_xyz(0.0, 0.0, -4.0),
    );
    open.spawn((Transform::from_xyz(0.0, 0.0, 40.0), sprite("quad", red, 1.5)));
    open.spawn((Transform::from_xyz(5.0, 0.0, 0.0), sprite("quad", red, 0.2)));
    let (image, scene) = render(&gpu, &mut renderer, &mut open, &assets);
    assert_eq!((scene.stats.sprites_visible, scene.stats.sprites_culled), (1, 1));
    expect_px(&image, W, 64, 64, [255, 0, 0, 255], 2, "a dropped far plane does not hide the far sprite");

    let mut closed = world(
        Camera::new(CameraProjection::orthographic(2.0).with_clip_range(0.1, 20.0)).with_clear_color(Color::BLACK),
        Transform::from_xyz(0.0, 0.0, -4.0),
    );
    closed.spawn((Transform::from_xyz(0.0, 0.0, 40.0), sprite("quad", red, 1.5)));
    let (image, scene) = render(&gpu, &mut renderer, &mut closed, &assets);
    assert_eq!(scene.stats.sprites_culled, 1);
    expect_px(&image, W, 64, 64, [0, 0, 0, 255], 2, "a real far plane still culls");
}
