//! GPU acceptance for the three sprite orientation policies.
//!
//! Exact top-down area is already measured in `spatial_scene` `configurations.rs`: a billboard has
//! pixels and an upright quad does not. These tests cover the gaps: opposite near-overhead
//! headings, one batch across the three policies, and independent views including an offscreen camera.

use engine_assets::{AssetManager, ColorSpace, MaterialAsset, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Transform, Vec2, Vec3};
use engine_renderer::*;
use engine_renderer_wgpu::{GpuContext, SceneRenderer};
use engine_world::{propagate_transforms, World};

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

fn assets() -> AssetManager {
    let mut assets = AssetManager::in_memory();
    let white =
        assets.insert("white-tex", Texture::solid(1, 1, [255; 4]).with_color_space(ColorSpace::Linear)).unwrap();
    assets.insert("quad", SpriteAsset::whole_texture(white, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    assets.insert("flat", MaterialAsset::unlit(Color::WHITE)).unwrap();
    assets
}

fn world() -> World {
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world
}

fn render(
    gpu: &GpuContext,
    renderer: &mut SceneRenderer,
    world: &mut World,
    assets: &AssetManager,
    width: u32,
    height: u32,
) -> (Vec<u8>, RenderScene) {
    propagate_transforms(world);
    let scene = extract_scene(world, assets, Vec2::new(width as f32, height as f32)).unwrap();
    let image = renderer.render_to_rgba(gpu, &scene, assets, width, height).unwrap();
    (image, scene)
}

fn px(img: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * width + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

fn expect(img: &[u8], width: u32, x: u32, y: u32, expected: [u8; 4], what: &str) {
    let pixel = px(img, width, x, y);
    let close = pixel.iter().zip(expected).all(|(got, want)| (*got as i16 - want as i16).abs() <= 2);
    assert!(close, "{what} at ({x},{y}) is {pixel:?}, expected {expected:?}");
}

fn project(view: &CameraView, point: Vec3) -> (u32, u32) {
    let screen = view.project(point).unwrap_or_else(|| panic!("off screen: {point:?}"));
    let x = (view.viewport_origin.x + screen.pixels.x).floor() as u32;
    let y = (view.viewport_origin.y + screen.pixels.y).floor() as u32;
    (x, y)
}

fn sprite(color: Color, size: Vec2, anchor: Vec2, orientation: SpriteOrientation) -> Sprite {
    Sprite::new("quad")
        .with_size(size)
        .with_anchor(anchor)
        .with_tint(color)
        .with_orientation(orientation)
        .cutout()
        .unlit()
}

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];

#[test]
fn opposite_near_overhead_headings_paint_opposite_sides() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    for z in [2.5, -2.5] {
        let pose = Transform::looking_at(Vec3::new(0.0, 8.0, z), Vec3::ZERO, Vec3::Z);
        let mut scene_world = world();
        scene_world.spawn((Camera::orthographic(6.0).with_clear_color(Color::BLACK), pose));
        scene_world.spawn((
            Transform::IDENTITY,
            sprite(Color::rgb(1.0, 0.0, 0.0), Vec2::new(2.0, 2.0), Vec2::new(0.0, 0.5), SpriteOrientation::Upright),
        ));
        let (image, scene) = render(&gpu, &mut renderer, &mut scene_world, &assets, W, W);
        let forward = pose.forward();
        let right = Vec3::Y.cross(forward).normalized();
        let view = &scene.sole_pass().view;
        let (on_x, on_y) = project(view, right);
        let (off_x, off_y) = project(view, -right * 0.5);
        expect(&image, W, on_x, on_y, RED, "the upright quad extends along Y × forward");
        expect(&image, W, off_x, off_y, BLACK, "the opposite side of that heading is empty");
        let placed = scene.sole_pass().cutout_sprites.instances[0].model.cols[0].xyz().normalized();
        assert!(placed.dot(right) > 0.99, "z={z}: placed X {placed:?}, heading {right:?}");
    }
}

#[test]
fn three_policies_share_one_batch_and_stay_visible() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut scene_world = world();
    scene_world.spawn((Camera::orthographic(8.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -6.0)));
    let material = engine_assets::AssetHandle::<MaterialAsset>::from_name("flat");
    let cases = [
        (-2.2, SpriteOrientation::Fixed, Color::rgb(1.0, 0.0, 0.0), RED),
        (0.0, SpriteOrientation::Upright, Color::rgb(0.0, 1.0, 0.0), GREEN),
        (2.2, SpriteOrientation::Billboard, Color::rgb(0.0, 0.0, 1.0), BLUE),
    ];
    for (x, orientation, color, _) in cases {
        scene_world.spawn((
            Transform::from_xyz(x, 0.0, 0.0),
            sprite(color, Vec2::splat(1.2), Vec2::splat(0.5), orientation).with_material(material),
        ));
    }
    let (image, scene) = render(&gpu, &mut renderer, &mut scene_world, &assets, W, W);
    let layer = &scene.sole_pass().opaque_sprites;
    assert_eq!(layer.batches.len(), 1);
    assert_eq!(layer.batches[0].range, 0..3);
    let view = &scene.sole_pass().view;
    for (x, _, _, expected) in cases {
        let (px_x, px_y) = project(view, Vec3::new(x, 0.0, 0.0));
        expect(&image, W, px_x, px_y, expected, "each policy draws inside the shared batch");
    }
}

#[test]
fn surface_cameras_and_an_offscreen_camera_orient_independently() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut scene_world = world();
    scene_world.insert_resource(RenderTargetStore::new());
    let target = scene_world.resource_mut::<RenderTargetStore>().create(64, 64).unwrap();
    scene_world.spawn((
        Camera::orthographic(6.0).with_clear_color(Color::BLACK).with_viewport(ViewportRect::new(0.0, 0.0, 0.5, 1.0)),
        Transform::from_xyz(0.0, 0.0, -6.0),
    ));
    scene_world.spawn((
        Camera {
            order: 1,
            viewport: ViewportRect::new(0.5, 0.0, 0.5, 1.0),
            ..Camera::orthographic(6.0).with_clear_color(Color::BLACK)
        },
        Transform::looking_at(Vec3::new(0.0, 8.0, 0.0), Vec3::ZERO, Vec3::Z),
    ));
    scene_world.spawn((
        Camera {
            order: 2,
            output: CameraOutput::Target(target),
            ..Camera::orthographic(6.0).with_clear_color(Color::BLACK)
        },
        Transform::looking_at(Vec3::new(6.0, 0.0, 0.0), Vec3::ZERO, Vec3::Y),
    ));
    let upright = scene_world.spawn((
        Transform::IDENTITY,
        sprite(Color::rgb(1.0, 0.0, 0.0), Vec2::splat(1.6), Vec2::splat(0.5), SpriteOrientation::Upright),
    ));
    let billboard = scene_world.spawn((
        Transform::from_xyz(0.0, 0.0, 1.8),
        sprite(Color::rgb(0.0, 0.0, 1.0), Vec2::splat(1.0), Vec2::splat(0.5), SpriteOrientation::Billboard),
    ));
    let upright_transform = *scene_world.component::<Transform>(upright).unwrap();
    let billboard_transform = *scene_world.component::<Transform>(billboard).unwrap();
    let (image, scene) = render(&gpu, &mut renderer, &mut scene_world, &assets, W, W);
    assert_eq!(*scene_world.component::<Transform>(upright).unwrap(), upright_transform);
    assert_eq!(*scene_world.component::<Transform>(billboard).unwrap(), billboard_transform);

    let (side, overhead, from_x) = (&scene.passes[0], &scene.passes[1], &scene.passes[2]);
    let (side_x, side_y) = project(&side.view, Vec3::ZERO);
    expect(&image, W, side_x, side_y, RED, "the side camera sees the upright sprite");
    let (over_x, over_y) = project(&overhead.view, Vec3::ZERO);
    expect(&image, W, over_x, over_y, BLACK, "the overhead camera sees the upright sprite edge-on");
    let (mark_x, mark_y) = project(&overhead.view, Vec3::new(0.0, 0.0, 1.8));
    expect(&image, W, mark_x, mark_y, BLUE, "the same overhead camera sees the billboard");

    let offscreen = renderer.read_render_target(&gpu, target).unwrap();
    let (off_x, off_y) = project(&from_x.view, Vec3::ZERO);
    expect(&offscreen, 64, off_x, off_y, RED, "the offscreen camera faces the upright sprite from +X");
    assert_eq!(renderer.stats().offscreen_passes, 1);
    assert_eq!(renderer.stats().surface_passes, 2);
    // Both horizontal views yaw the quad. The overhead view has no heading, so the quad stays
    // vertical and the picture is the edge. The +X camera's heading is not the side camera's.
    assert!(upright_model(side).cols[1].xyz().normalized().approx_eq(Vec3::Y, 1e-3));
    assert!(upright_model(overhead).cols[1].xyz().normalized().approx_eq(Vec3::Y, 1e-3));
    let side_right = upright_model(side).cols[0].xyz().normalized();
    let from_x_right = upright_model(from_x).cols[0].xyz().normalized();
    assert!(from_x_right.dot(side_right).abs() < 0.2, "side {side_right:?}, +X {from_x_right:?}");
}

fn upright_model(pass: &CameraPass) -> engine_math::Mat4 {
    pass.cutout_sprites.instances.iter().find(|instance| instance.tint[0] > 0.5).expect("red upright sprite").model
}

#[test]
fn coplanar_priority_does_not_give_an_edge_on_upright_sprite_a_face() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut scene_world = world();
    scene_world.insert_resource(CoplanarSettings::new(CANDIDATE_COPLANAR_THICKNESS).unwrap());
    scene_world.spawn((
        Camera::orthographic(6.0).with_clear_color(Color::BLACK),
        Transform::looking_at(Vec3::new(0.0, 8.0, 0.0), Vec3::ZERO, Vec3::Z),
    ));
    scene_world.spawn((
        Transform::IDENTITY,
        sprite(Color::rgb(1.0, 0.0, 0.0), Vec2::splat(2.0), Vec2::splat(0.5), SpriteOrientation::Upright),
        CoplanarPriority::new(7).unwrap(),
    ));
    let (image, _) = render(&gpu, &mut renderer, &mut scene_world, &assets, W, W);
    expect(&image, W, W / 2, W / 2, BLACK, "priority does not thicken an edge-on upright quad");
}
