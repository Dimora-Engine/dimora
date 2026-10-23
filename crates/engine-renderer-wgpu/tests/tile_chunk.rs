//! GPU checks that chunked tile extraction matches the previous picture.
//!
//! Tiles stay legacy cutout. They are not blended, and they do not read `CoplanarPriority`.

use core::f32::consts::{FRAC_PI_2, FRAC_PI_4};

use engine_assets::{AssetManager, ColorSpace, MeshData, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Quat, Transform, Vec2, Vec3};
use engine_renderer::*;
use engine_renderer_wgpu::{GpuContext, SceneRenderer};
use engine_world::{propagate_transforms, GlobalTransform, World};

const W: u32 = 128;

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

fn linear(rgba: [u8; 4]) -> Texture {
    Texture::solid(4, 4, rgba).with_color_space(ColorSpace::Linear)
}

fn assets() -> AssetManager {
    let mut assets = AssetManager::in_memory();
    assets.insert("tiles", linear([255, 0, 0, 255])).unwrap();
    assets.insert("white-tiles", linear([255, 255, 255, 255])).unwrap();
    let white =
        assets.insert("sprite-tex", Texture::solid(1, 1, [255; 4]).with_color_space(ColorSpace::Linear)).unwrap();
    assets.insert("sprite", SpriteAsset::whole_texture(white, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    assets.insert("cube", MeshData::cuboid(Vec3::splat(0.5)).unwrap()).unwrap();
    assets
}

fn world() -> World {
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.insert_resource(RenderTargetStore::new());
    world
}

fn render(
    gpu: &GpuContext,
    renderer: &mut SceneRenderer,
    world: &mut World,
    assets: &AssetManager,
) -> (Vec<u8>, RenderScene) {
    propagate_transforms(world);
    let scene = extract_scene(world, assets, Vec2::new(W as f32, W as f32)).unwrap();
    let image = renderer.render_to_rgba(gpu, &scene, assets, W, W).unwrap();
    (image, scene)
}

fn px(img: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * W + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

fn expect(img: &[u8], x: u32, y: u32, expected: [u8; 4], what: &str) {
    let pixel = px(img, x, y);
    let close = pixel.iter().zip(expected).all(|(got, want)| (*got as i16 - want as i16).abs() <= 8);
    assert!(close, "{what} at ({x},{y}) is {pixel:?}, expected {expected:?}");
}

fn project(view: &CameraView, point: Vec3) -> (u32, u32) {
    let screen = view.project(point).unwrap_or_else(|| panic!("off screen: {point:?}"));
    assert!(screen.is_visible(), "outside the view: {point:?}");
    let x = (view.viewport_origin.x + screen.pixels.x).floor().clamp(0.0, (W - 1) as f32) as u32;
    let y = (view.viewport_origin.y + screen.pixels.y).floor().clamp(0.0, (W - 1) as f32) as u32;
    (x, y)
}

fn unlit_map(name: &str, width: u32, height: u32) -> TileMap {
    let mut map = TileMap::new(name, 1, 1, Vec2::ONE, width, height).filled_with(|_, _| 0);
    map.lit = false;
    map
}

fn ground(width: f32, height: f32) -> Transform {
    let rotation = Quat::from_rotation_x(FRAC_PI_2);
    let center = rotation.rotate(Vec3::new(width * 0.5, height * 0.5, 0.0));
    Transform::from_position(-center).with_rotation(rotation)
}

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];

#[test]
fn chunk_boundary_has_no_gap() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let edge = TileMap::CHUNK_EDGE;
    let mut world = world();
    let boundary = edge as f32;
    world.spawn((Camera::orthographic(4.0).with_clear_color(Color::BLACK), Transform::from_xyz(boundary, 0.5, -8.0)));
    world.spawn((Transform::IDENTITY, unlit_map("white-tiles", edge + 1, 1)));
    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    let view = &scene.sole_pass().view;
    let (left_x, left_y) = project(view, Vec3::new(boundary - 0.5, 0.5, 0.0));
    let (right_x, right_y) = project(view, Vec3::new(boundary + 0.5, 0.5, 0.0));
    let (start, end) = (left_x.min(right_x), left_x.max(right_x));
    assert!(end - start > 4, "the two tile centers should be several pixels apart");
    for x in start..=end {
        expect(&image, x, left_y.min(right_y), WHITE, "pixels between chunk tiles");
    }
}

#[test]
fn orthographic_perspective_and_isometric_views_show_the_ground_tile() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let cases = [
        (
            "orthographic",
            Camera::orthographic(12.0),
            Transform::looking_at(Vec3::new(0.0, 20.0, 0.1), Vec3::ZERO, Vec3::Z),
        ),
        (
            "perspective",
            Camera::perspective(50.0),
            Transform::looking_at(Vec3::new(0.0, 12.0, -8.0), Vec3::ZERO, Vec3::Y),
        ),
        (
            "isometric",
            Camera::orthographic(16.0),
            Transform::looking_at(
                Quat::from_rotation_y(FRAC_PI_4).rotate(Vec3::new(0.0, 0.0, -12.0))
                    + Vec3::Y * (12.0 * 0.615_479_7_f32.sin()),
                Vec3::ZERO,
                Vec3::Y,
            ),
        ),
    ];
    for (name, camera, pose) in cases {
        let mut world = world();
        world.spawn((camera.with_clear_color(Color::BLACK), pose));
        let entity = world.spawn((ground(4.0, 4.0), unlit_map("tiles", 4, 4)));
        let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
        let global = *world.component::<GlobalTransform>(entity).unwrap();
        let center = global.matrix.transform_point3(Vec3::new(2.5, 2.5, 0.0));
        let (x, y) = project(&scene.sole_pass().view, center);
        expect(&image, x, y, RED, name);
        assert_eq!(scene.sole_pass().cutout_sprites.batches.len(), 1, "{name} stays one batch");
    }
}

#[test]
fn a_yawed_ground_map_is_still_the_tile_color() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut world = world();
    world.spawn((
        Camera::perspective(45.0).with_clear_color(Color::BLACK),
        Transform::looking_at(Vec3::new(6.0, 8.0, 6.0), Vec3::ZERO, Vec3::Y),
    ));
    let pose = ground(4.0, 4.0).with_rotation(Quat::from_rotation_y(FRAC_PI_4) * Quat::from_rotation_x(FRAC_PI_2));
    let entity = world.spawn((pose, unlit_map("tiles", 4, 4)));
    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    let global = *world.component::<GlobalTransform>(entity).unwrap();
    let center = global.matrix.transform_point3(Vec3::new(2.5, 2.5, 0.0));
    let (x, y) = project(&scene.sole_pass().view, center);
    expect(&image, x, y, RED, "yawed ground tile");
}

#[test]
fn nearer_mesh_occludes_tiles_and_tiles_occlude_a_farther_mesh() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let camera = Camera::orthographic(4.0).with_clear_color(Color::BLACK);
    let pose = Transform::from_xyz(0.5, 0.5, -6.0);

    let mut front = world();
    front.spawn((camera, pose));
    front.spawn((Transform::IDENTITY, unlit_map("tiles", 2, 2)));
    front.spawn((
        Transform::from_xyz(0.5, 0.5, -0.4),
        Mesh::new("cube").with_material(Material::unlit(Color::rgb(0.0, 0.0, 1.0))),
    ));
    let (image, scene) = render(&gpu, &mut renderer, &mut front, &assets);
    let (x, y) = project(&scene.sole_pass().view, Vec3::new(0.5, 0.5, -0.4));
    expect(&image, x, y, BLUE, "nearer mesh");

    let mut back = world();
    back.spawn((camera, pose));
    back.spawn((Transform::IDENTITY, unlit_map("tiles", 2, 2)));
    back.spawn((
        Transform::from_xyz(0.5, 0.5, 0.8),
        Mesh::new("cube").with_material(Material::unlit(Color::rgb(0.0, 0.0, 1.0))),
    ));
    let (image, scene) = render(&gpu, &mut renderer, &mut back, &assets);
    let (x, y) = project(&scene.sole_pass().view, Vec3::new(0.5, 0.5, 0.0));
    expect(&image, x, y, RED, "tile in front of the farther mesh");
}

#[test]
fn a_blended_sprite_in_front_of_tiles_shows_and_one_behind_does_not() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let camera = Camera::orthographic(4.0).with_clear_color(Color::BLACK);
    let pose = Transform::from_xyz(0.5, 0.5, -6.0);
    let sprite = Sprite::new("sprite")
        .with_size(Vec2::splat(0.4))
        .with_anchor(Vec2::splat(0.5))
        .with_tint(Color::rgb(0.0, 1.0, 0.0))
        .unlit()
        .fixed();

    let mut front = world();
    front.spawn((camera, pose));
    front.spawn((Transform::IDENTITY, unlit_map("tiles", 2, 2)));
    front.spawn((Transform::from_xyz(0.5, 0.5, -0.3), sprite));
    let (image, scene) = render(&gpu, &mut renderer, &mut front, &assets);
    let (x, y) = project(&scene.sole_pass().view, Vec3::new(0.5, 0.5, -0.3));
    expect(&image, x, y, GREEN, "blended sprite in front of the tiles");

    let mut behind = world();
    behind.spawn((camera, pose));
    behind.spawn((Transform::IDENTITY, unlit_map("tiles", 2, 2)));
    behind.spawn((Transform::from_xyz(0.5, 0.5, 0.6), sprite));
    let (image, scene) = render(&gpu, &mut renderer, &mut behind, &assets);
    let (x, y) = project(&scene.sole_pass().view, Vec3::new(0.5, 0.5, 0.0));
    expect(&image, x, y, RED, "cutout tiles hide the blended sprite behind them");
}

#[test]
fn two_surface_cameras_and_an_offscreen_camera_see_one_map() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut world = world();
    let target = world.resource_mut::<RenderTargetStore>().create(W, W).unwrap();
    world.spawn((
        Camera::orthographic(4.0).with_output(CameraOutput::Target(target)).with_clear_color(Color::BLACK),
        Transform::from_xyz(0.5, 0.5, -6.0),
    ));
    world.spawn((
        Camera {
            order: 1,
            viewport: ViewportRect::new(0.0, 0.0, 0.5, 1.0),
            ..Camera::orthographic(4.0).with_clear_color(Color::BLACK)
        },
        Transform::from_xyz(0.5, 0.5, -6.0),
    ));
    world.spawn((
        Camera {
            order: 2,
            viewport: ViewportRect::new(0.5, 0.0, 0.5, 1.0),
            ..Camera::orthographic(4.0).with_clear_color(Color::BLACK)
        },
        Transform::from_xyz(10_000.0, 0.0, -6.0),
    ));
    world.spawn((Transform::IDENTITY, unlit_map("tiles", 2, 2)));
    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(scene.passes.len(), 3);
    assert_eq!(scene.passes[1].cutout_sprites.batches.len(), 1);
    let (x, y) = project(&scene.passes[1].view, Vec3::new(0.5, 0.5, 0.0));
    expect(&image, x, y, RED, "left surface camera");
    let right = project(&scene.passes[2].view, Vec3::new(10_000.0, 0.0, 0.0));
    expect(&image, right.0, right.1, BLACK, "right camera is looking at empty space");
    let offscreen = renderer.read_render_target(&gpu, target).expect("offscreen image");
    let (ox, oy) = project(&scene.passes[0].view, Vec3::new(0.5, 0.5, 0.0));
    expect(&offscreen, ox, oy, RED, "offscreen camera");
}
