//! GPU checks for world-space debug lines.
//!
//! Lines are one pixel wide, depth-tested with reverse-Z, and drawn after transparent geometry
//! without writing depth. Pixel searches use a small window because a one-pixel line can land on
//! a neighboring pixel.

use engine_assets::{AlphaMode, AssetManager, ColorSpace, MaterialAsset, MeshData, SpriteAsset, Texture};
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

fn world() -> World {
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.insert_resource(RenderTargetStore::new());
    world.insert_resource(DebugDraw::default());
    world
}

fn assets() -> AssetManager {
    let mut assets = AssetManager::in_memory();
    let white =
        assets.insert("white-tex", Texture::solid(1, 1, [255; 4]).with_color_space(ColorSpace::Linear)).unwrap();
    assets.insert("sprite", SpriteAsset::whole_texture(white, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    assets.insert("cube", MeshData::cuboid(Vec3::splat(1.0)).unwrap()).unwrap();
    assets
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

fn px(img: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * width + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

fn close(pixel: [u8; 4], expected: [u8; 4], tolerance: i16) -> bool {
    pixel.iter().zip(expected).all(|(got, want)| (*got as i16 - want as i16).abs() <= tolerance)
}

fn project(view: &CameraView, point: Vec3) -> (u32, u32) {
    let screen = view.project(point).unwrap_or_else(|| panic!("off screen: {point:?}"));
    assert!(screen.is_visible(), "outside the view: {point:?}");
    let x = (view.viewport_origin.x + screen.pixels.x).round().clamp(0.0, (W - 1) as f32) as u32;
    let y = (view.viewport_origin.y + screen.pixels.y).round().clamp(0.0, (W - 1) as f32) as u32;
    (x, y)
}

fn window_has(img: &[u8], x: u32, y: u32, radius: i32, expected: [u8; 4]) -> bool {
    let span = W as i32;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let px_x = x as i32 + dx;
            let px_y = y as i32 + dy;
            if px_x < 0 || px_y < 0 || px_x >= span || px_y >= span {
                continue;
            }
            if close(px(img, W, px_x as u32, px_y as u32), expected, 12) {
                return true;
            }
        }
    }
    false
}

fn window_all(img: &[u8], x: u32, y: u32, radius: i32, expected: [u8; 4]) -> bool {
    let span = W as i32;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let px_x = x as i32 + dx;
            let px_y = y as i32 + dy;
            if px_x < 0 || px_y < 0 || px_x >= span || px_y >= span {
                continue;
            }
            if !close(px(img, W, px_x as u32, px_y as u32), expected, 12) {
                return false;
            }
        }
    }
    true
}

fn count_near(img: &[u8], expected: [u8; 4]) -> usize {
    img.chunks(4).filter(|pixel| close([pixel[0], pixel[1], pixel[2], pixel[3]], expected, 12)).count()
}

fn side_camera(height: f32) -> (Camera, Transform) {
    (Camera::orthographic(height).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -8.0))
}

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];

#[test]
fn world_line_is_visible() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut world = world();
    world.spawn(side_camera(4.0));
    world.resource_mut::<DebugDraw>().line(
        Vec3::new(-1.5, 0.0, 0.0),
        Vec3::new(1.5, 0.0, 0.0),
        Color::rgb(1.0, 0.0, 0.0),
    );

    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    let (x, y) = project(&scene.sole_pass().view, Vec3::ZERO);
    assert!(window_has(&image, x, y, 3, RED), "line at ({x},{y}) pixel {:?}", px(&image, W, x, y));
    assert!(close(px(&image, W, 4, 4), BLACK, 2), "clear stays black");
    assert_eq!(renderer.stats().world_draw_calls, 1);
    assert_eq!(renderer.stats().submitted_instances, 2);
}

#[test]
fn wire_box_and_sphere_are_visible() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut world = world();
    world.spawn(side_camera(6.0));
    let mut debug = world.resource_mut::<DebugDraw>();
    debug.wire_box(Vec3::new(0.0, -1.3, 0.0), Vec3::splat(0.5), Color::rgb(0.0, 0.0, 1.0));
    debug.wire_sphere(Vec3::new(0.0, 1.4, 0.0), 0.45, Color::rgb(0.0, 1.0, 0.0));
    drop(debug);

    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    assert!(scene.debug_lines.len() > 12);
    assert_eq!(renderer.stats().world_draw_calls, 1, "box and sphere share one line draw");
    assert_eq!(renderer.stats().submitted_instances, scene.debug_lines.len() as u32 * 2);
    assert!(count_near(&image, BLUE) > 8, "box edges");
    assert!(count_near(&image, GREEN) > 8, "sphere circles");
    let (box_x, box_y) = project(&scene.sole_pass().view, Vec3::new(0.0, -1.3, 0.0));
    assert!(close(px(&image, W, box_x, box_y), BLACK, 8), "box center is empty, got {:?}", px(&image, W, box_x, box_y));
    let (sphere_x, sphere_y) = project(&scene.sole_pass().view, Vec3::new(0.12, 1.52, 0.0));
    assert!(
        close(px(&image, W, sphere_x, sphere_y), BLACK, 8),
        "sphere interior is empty, got {:?}",
        px(&image, W, sphere_x, sphere_y)
    );
}

#[test]
fn line_behind_a_mesh_is_hidden() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut world = world();
    world.spawn(side_camera(4.0));
    world.spawn((Transform::IDENTITY, Mesh::new("cube").with_material(Material::unlit(Color::WHITE))));
    world.resource_mut::<DebugDraw>().line(
        Vec3::new(-2.0, 0.0, 1.0),
        Vec3::new(2.0, 0.0, 1.0),
        Color::rgb(1.0, 0.0, 0.0),
    );

    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    let view = &scene.sole_pass().view;
    let (cx, cy) = project(view, Vec3::ZERO);
    assert!(window_all(&image, cx, cy, 2, WHITE), "cube center stays white, got {:?}", px(&image, W, cx, cy));
    let (ox, oy) = project(view, Vec3::new(1.2, 0.0, 1.0));
    assert!(window_has(&image, ox, oy, 3, RED), "the line is visible beside the cube");
}

#[test]
fn line_in_front_of_a_mesh_is_visible() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut world = world();
    world.spawn(side_camera(4.0));
    world.spawn((Transform::IDENTITY, Mesh::new("cube").with_material(Material::unlit(Color::WHITE))));
    world.resource_mut::<DebugDraw>().line(
        Vec3::new(-2.0, 0.0, -1.0),
        Vec3::new(2.0, 0.0, -1.0),
        Color::rgb(1.0, 0.0, 0.0),
    );

    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    let view = &scene.sole_pass().view;
    let (cx, cy) = project(view, Vec3::ZERO);
    assert!(window_has(&image, cx, cy, 3, RED), "nearer line covers the cube, got {:?}", px(&image, W, cx, cy));
    let (mx, my) = project(view, Vec3::new(0.0, 0.35, 0.0));
    assert!(close(px(&image, W, mx, my), WHITE, 8), "the rest of the cube stays white");
}

#[test]
fn primitives_share_one_draw_with_a_mesh() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut world = world();
    world.spawn(side_camera(4.0));
    let mut debug = world.resource_mut::<DebugDraw>();
    debug.line(Vec3::new(-1.0, 1.2, 0.0), Vec3::new(1.0, 1.2, 0.0), Color::rgb(1.0, 0.0, 0.0));
    debug.wire_box(Vec3::new(0.0, -1.2, 0.0), Vec3::splat(0.3), Color::rgb(0.0, 0.0, 1.0));
    debug.ray(Vec3::new(-1.5, -0.2, 0.0), Vec3::X, 3.0, Color::rgb(0.0, 1.0, 0.0));
    drop(debug);

    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    let segments = scene.debug_lines.len() as u32;
    assert!(segments > 3);
    assert_eq!(renderer.stats().world_draw_calls, 1);
    assert_eq!(renderer.stats().submitted_instances, segments * 2);
    assert!(count_near(&image, RED) > 0 && count_near(&image, BLUE) > 0 && count_near(&image, GREEN) > 0);

    world.spawn((Transform::from_xyz(0.0, 0.0, 0.0), Mesh::new("cube").with_material(Material::unlit(Color::WHITE))));
    let (_image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(renderer.stats().world_draw_calls, 2, "the mesh draw plus one debug batch");
    assert_eq!(renderer.stats().submitted_instances, 1 + scene.debug_lines.len() as u32 * 2);
}

#[test]
fn perspective_orthographic_and_custom_projections_show_the_line() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();

    let ortho = CameraProjection::orthographic(4.0);
    let cases = [
        Camera::new(ortho).with_clear_color(Color::BLACK),
        Camera::perspective(50.0).with_clear_color(Color::BLACK),
        Camera::new(CameraProjection::custom(ortho.matrix(1.0)).unwrap()).with_clear_color(Color::BLACK),
    ];
    for camera in cases {
        let mut world = world();
        world.spawn((camera, Transform::from_xyz(0.0, 0.0, -6.0)));
        world.resource_mut::<DebugDraw>().line(
            Vec3::new(-0.6, 0.0, 0.0),
            Vec3::new(0.6, 0.0, 0.0),
            Color::rgb(1.0, 0.0, 0.0),
        );
        let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
        let (x, y) = project(&scene.sole_pass().view, Vec3::ZERO);
        assert!(
            window_has(&image, x, y, 4, RED),
            "{:?} missed the line at ({x},{y}) pixel {:?}",
            scene.sole_pass().view.proj,
            px(&image, W, x, y)
        );
        assert_eq!(renderer.stats().world_draw_calls, 1);
    }
}

#[test]
fn oblique_camera_sees_the_line() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut world = world();
    let pose = Transform::looking_at(Vec3::new(4.0, 3.0, -5.0), Vec3::ZERO, Vec3::Y);
    world.spawn((Camera::orthographic(10.0).with_clear_color(Color::BLACK), pose));
    world.resource_mut::<DebugDraw>().line(
        Vec3::new(-1.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        Color::rgb(1.0, 0.0, 0.0),
    );

    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    let (x, y) = project(&scene.sole_pass().view, Vec3::ZERO);
    assert!(window_has(&image, x, y, 4, RED), "oblique view missed the line, pixel {:?}", px(&image, W, x, y));
}

#[test]
fn two_cameras_draw_the_same_segments() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut world = world();
    world.spawn((
        Camera::orthographic(4.0).with_clear_color(Color::BLACK).with_viewport(ViewportRect::new(0.0, 0.0, 0.5, 1.0)),
        Transform::from_xyz(0.0, 0.0, -8.0),
    ));
    world.spawn((
        Camera {
            order: 1,
            ..Camera::orthographic(4.0)
                .with_clear_color(Color::rgb(0.0, 0.0, 0.35))
                .with_viewport(ViewportRect::new(0.5, 0.0, 0.5, 1.0))
        },
        Transform::from_xyz(0.0, 0.0, -8.0),
    ));
    world.resource_mut::<DebugDraw>().line(
        Vec3::new(-0.4, 0.0, 0.0),
        Vec3::new(0.4, 0.0, 0.0),
        Color::rgb(1.0, 0.0, 0.0),
    );

    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(scene.passes.len(), 2);
    assert_eq!(scene.stats.debug_segments, 1);
    assert_eq!(renderer.stats().world_draw_calls, 2);
    assert_eq!(renderer.stats().submitted_instances, 4, "two vertices, drawn by each camera");
    for pass in &scene.passes {
        let (x, y) = project(&pass.view, Vec3::ZERO);
        assert!(window_has(&image, x, y, 3, RED), "camera {} missed the line", pass.camera_id);
    }
    assert!(close(px(&image, W, 4, 4), BLACK, 8));
    assert!(close(px(&image, W, W - 4, 4), [0, 0, 89, 255], 12));
}

#[test]
fn offscreen_camera_draws_the_line() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut world = world();
    let target = world.resource_mut::<RenderTargetStore>().create(W, W).unwrap();
    world.spawn((
        Camera::orthographic(4.0).with_clear_color(Color::BLACK).with_output(CameraOutput::Target(target)),
        Transform::from_xyz(0.0, 0.0, -8.0),
    ));
    world.resource_mut::<DebugDraw>().line(
        Vec3::new(-1.0, 0.25, 0.0),
        Vec3::new(1.0, 0.25, 0.0),
        Color::rgb(1.0, 0.0, 0.0),
    );

    let (_surface, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    let image = renderer.read_render_target(&gpu, target).expect("offscreen image");
    assert_eq!(image.len(), (W * W * 4) as usize);
    assert_eq!(scene.passes[0].output, CameraOutput::Target(target));
    assert_eq!(renderer.stats().offscreen_passes, 1);
    assert_eq!(renderer.stats().world_draw_calls, 1);
    let (x, y) = project(&scene.sole_pass().view, Vec3::new(0.0, 0.25, 0.0));
    assert!(window_has(&image, x, y, 3, RED), "offscreen line missing at ({x},{y})");
}

#[test]
fn empty_debug_does_not_change_sprite_or_mesh_draws() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = assets();
    let red = assets.insert("red", MaterialAsset::unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let glass = assets
        .insert("glass", MaterialAsset::unlit(Color::rgba(0.0, 1.0, 0.0, 0.5)).with_alpha_mode(AlphaMode::Blend))
        .unwrap();

    let mut world = world();
    world.remove_resource::<DebugDraw>();
    world.spawn(side_camera(6.0));
    world.spawn((
        Transform::from_xyz(-1.5, 0.0, 0.0),
        Sprite::new("sprite").with_size(Vec2::splat(1.0)).with_anchor(Vec2::splat(0.5)).with_material(red).fixed(),
    ));
    world.spawn((
        Transform::from_xyz(1.5, 0.0, 0.0),
        Sprite::new("sprite").with_size(Vec2::splat(1.0)).with_anchor(Vec2::splat(0.5)).with_material(glass).fixed(),
    ));
    world.spawn((Transform::from_xyz(0.0, 1.6, 0.0), Mesh::new("cube").with_material(Material::unlit(Color::WHITE))));

    let (before, _) = render(&gpu, &mut renderer, &mut world, &assets);
    let before_stats = renderer.stats();
    world.insert_resource(DebugDraw::default());
    let (after, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    assert!(scene.debug_lines.is_empty());
    assert_eq!(before, after);
    assert_eq!(renderer.stats().draw_calls, before_stats.draw_calls);
    assert_eq!(renderer.stats().world_draw_calls, before_stats.world_draw_calls);
    assert_eq!(renderer.stats().submitted_instances, before_stats.submitted_instances);
    assert!(before_stats.world_draw_calls > 0);
}

#[test]
fn transparency_and_camera_target_stay_compatible() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = assets();
    let white = assets.insert("sample", MaterialAsset::unlit(Color::WHITE)).unwrap();
    let red = assets.insert("red-cutout", MaterialAsset::unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let glass = assets
        .insert("glass", MaterialAsset::unlit(Color::rgba(0.0, 1.0, 0.0, 0.5)).with_alpha_mode(AlphaMode::Blend))
        .unwrap();

    let mut world = world();
    world.remove_resource::<DebugDraw>();
    let target = world.resource_mut::<RenderTargetStore>().create(W, W).unwrap();
    world.spawn((
        Camera::orthographic(4.0).with_clear_color(Color::rgb(0.0, 0.0, 1.0)).with_output(CameraOutput::Target(target)),
        Transform::from_xyz(0.0, 0.0, -8.0),
    ));
    world.spawn((
        Camera { order: 1, ..Camera::orthographic(4.0).with_clear_color(Color::rgb(0.0, 0.4, 0.0)) },
        Transform::from_xyz(20.0, 0.0, -8.0),
    ));
    world.spawn((
        Transform::from_xyz(0.0, 0.0, 0.0),
        Sprite::new("sprite")
            .with_size(Vec2::splat(1.2))
            .with_anchor(Vec2::splat(0.5))
            .with_material(red)
            .fixed()
            .cutout(),
    ));
    world.spawn((
        Transform::from_xyz(0.0, 1.35, 0.0),
        Sprite::new("sprite").with_size(Vec2::splat(0.7)).with_anchor(Vec2::splat(0.5)).with_material(glass).fixed(),
    ));
    world.spawn((
        Transform::from_xyz(20.0, 0.0, 0.0),
        Sprite::new("sprite")
            .with_size(Vec2::splat(4.0))
            .with_anchor(Vec2::splat(0.5))
            .with_material(white)
            .with_render_target(target)
            .fixed(),
    ));

    let (surface_before, _) = render(&gpu, &mut renderer, &mut world, &assets);
    let target_before = renderer.read_render_target(&gpu, target).unwrap();
    let draws_before = renderer.stats().world_draw_calls;

    world.insert_resource(DebugDraw::default());
    let (surface_empty, _) = render(&gpu, &mut renderer, &mut world, &assets);
    let target_empty = renderer.read_render_target(&gpu, target).unwrap();
    assert_eq!(surface_before, surface_empty);
    assert_eq!(target_before, target_empty);
    assert_eq!(renderer.stats().world_draw_calls, draws_before);

    world.resource_mut::<DebugDraw>().line(
        Vec3::new(-1.6, 1.35, 1.0),
        Vec3::new(1.6, 1.35, 1.0),
        Color::rgb(1.0, 1.0, 0.0),
    );
    world.resource_mut::<DebugDraw>().line(
        Vec3::new(-1.6, 0.0, 1.0),
        Vec3::new(1.6, 0.0, 1.0),
        Color::rgb(1.0, 1.0, 0.0),
    );
    let (surface, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    let target_image = renderer.read_render_target(&gpu, target).unwrap();
    let offscreen = scene.passes.iter().find(|pass| pass.output == CameraOutput::Target(target)).unwrap();
    let (cutout_x, cutout_y) = project(&offscreen.view, Vec3::ZERO);
    assert!(
        window_all(&target_image, cutout_x, cutout_y, 1, RED),
        "cutout depth hides the line behind it, got {:?}",
        px(&target_image, W, cutout_x, cutout_y)
    );
    let (blend_x, blend_y) = project(&offscreen.view, Vec3::new(0.0, 1.35, 0.0));
    let yellow = [255, 255, 0, 255];
    assert!(
        window_has(&target_image, blend_x, blend_y, 3, yellow),
        "a line paints over blended pixels that did not write depth, got {:?}",
        px(&target_image, W, blend_x, blend_y)
    );
    let (sample_x, sample_y) = project(&scene.passes[1].view, Vec3::new(20.0, 0.0, 0.0));
    assert!(
        close(px(&surface, W, sample_x, sample_y), RED, 8),
        "the surface still samples the target center, got {:?}",
        px(&surface, W, sample_x, sample_y)
    );
    assert_eq!(renderer.stats().world_draw_calls, draws_before + 2);
}
