//! Production reverse-Z on the normal extract and GPU path.
//!
//! Near is NDC 1, far is NDC 0, the depth test is greater-than, and each pass clears depth to 0.
//! These tests do not add a second renderer. The historical 0-near matrix is built only inside
//! `a_stored_standard_z_matrix_is_not_reinterpreted` to show that `Custom` is stored unchanged.

use engine_assets::{AlphaMode, AssetManager, ColorSpace, MaterialAsset, MeshData, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Mat4, Transform, Vec2, Vec3, Vec4};
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

fn center(img: &[u8], width: u32, expected: [u8; 4], tolerance: i16, what: &str) {
    expect_px(img, width, width / 2, width / 2, expected, tolerance, what);
}

fn assets() -> AssetManager {
    let mut assets = AssetManager::in_memory();
    let white =
        assets.insert("white-tex", Texture::solid(1, 1, [255; 4]).with_color_space(ColorSpace::Linear)).unwrap();
    assets.insert("quad", SpriteAsset::whole_texture(white, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    assets.insert("cube", MeshData::cuboid(Vec3::splat(2.0)).unwrap()).unwrap();
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
) -> (Vec<u8>, RenderScene) {
    propagate_transforms(world);
    let scene = extract_scene(world, assets, Vec2::new(W as f32, H as f32)).unwrap();
    let image = renderer.render_to_rgba(gpu, &scene, assets, W, H).unwrap();
    (image, scene)
}

fn tinted(color: Color, size: f32) -> Sprite {
    Sprite::new("quad").with_size(Vec2::splat(size)).with_anchor(Vec2::splat(0.5)).with_tint(color).cutout().unlit()
}

fn pair(camera: Camera, pose: Transform, near: (Vec3, Color, f32), far: (Vec3, Color, f32)) -> World {
    let mut world = world();
    world.spawn((camera, pose));
    world.spawn((Transform::from_position(near.0), tinted(near.1, near.2)));
    world.spawn((Transform::from_position(far.0), tinted(far.1, far.2)));
    world
}

#[test]
fn nearer_sprites_win_for_perspective_orthographic_and_side_views() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let red = Color::rgb(1.0, 0.0, 0.0);
    let blue = Color::rgb(0.0, 0.0, 1.0);
    let cases = [
        (
            "perspective",
            Camera::perspective(60.0).with_clear_color(Color::BLACK),
            Transform::from_xyz(0.0, 0.0, -10.0),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 6.0),
            8.0,
        ),
        (
            "orthographic",
            Camera::orthographic(20.0).with_clear_color(Color::BLACK),
            Transform::from_xyz(0.0, 0.0, -10.0),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 6.0),
            8.0,
        ),
        (
            "side",
            Camera::orthographic(20.0).with_clear_color(Color::BLACK),
            Transform::looking_at(Vec3::ZERO, Vec3::X, Vec3::Y),
            Vec3::new(4.0, 0.0, 0.0),
            Vec3::new(12.0, 0.0, 0.0),
            8.0,
        ),
    ];
    for (name, camera, pose, near, far, size) in cases {
        let mut world = pair(camera, pose, (near, red, size), (far, blue, size));
        let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
        assert_eq!(scene.passes.len(), 1, "{name}");
        center(&image, W, [255, 0, 0, 255], 2, name);
        let stats = renderer.stats();
        assert_eq!((stats.camera_passes, stats.clear_draw_calls, stats.world_draw_calls), (1, 0, 1), "{name}");
        assert_eq!(stats.submitted_instances, 2, "{name} keeps both sprites in one batch");
    }
}

#[test]
fn sprites_and_meshes_occlude_each_other() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    for camera in [Camera::perspective(60.0), Camera::orthographic(20.0)] {
        let mut mesh_in_front = world();
        mesh_in_front.spawn((camera.with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -10.0)));
        mesh_in_front
            .spawn((Transform::from_xyz(0.0, 0.0, 4.0), tinted(Color::rgb(1.0, 0.0, 0.0), 10.0).cutout().unlit()));
        mesh_in_front.spawn((
            Transform::from_xyz(0.0, 0.0, 0.0),
            Mesh::new("cube").with_material(Material { lit: false, ..Material::color(Color::rgb(0.0, 0.0, 1.0)) }),
        ));
        let (image, _) = render(&gpu, &mut renderer, &mut mesh_in_front, &assets);
        center(&image, W, [0, 0, 255, 255], 2, "a nearer mesh occludes a sprite");

        let mut sprite_in_front = world();
        sprite_in_front.spawn((camera.with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -10.0)));
        sprite_in_front
            .spawn((Transform::from_xyz(0.0, 0.0, 0.0), tinted(Color::rgb(1.0, 0.0, 0.0), 6.0).cutout().unlit()));
        sprite_in_front.spawn((
            Transform::from_xyz(0.0, 0.0, 5.0),
            Mesh::new("cube").with_material(Material { lit: false, ..Material::color(Color::rgb(0.0, 0.0, 1.0)) }),
        ));
        let (image, scene) = render(&gpu, &mut renderer, &mut sprite_in_front, &assets);
        center(&image, W, [255, 0, 0, 255], 2, "a nearer sprite occludes a mesh");
        assert_eq!(renderer.stats().camera_passes, 1);
        assert_eq!(scene.sole_pass().meshes.len(), 1);
    }
}

#[test]
fn opaque_cutout_mask_and_blend_follow_the_same_depth() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = assets();
    let keep = assets
        .insert(
            "keep",
            MaterialAsset::unlit(Color::rgba(0.0, 1.0, 0.0, 0.5)).with_alpha_mode(AlphaMode::mask(0.5).unwrap()),
        )
        .unwrap();
    let drop = assets
        .insert(
            "drop",
            MaterialAsset::unlit(Color::rgba(0.0, 1.0, 0.0, 0.5)).with_alpha_mode(AlphaMode::mask(0.75).unwrap()),
        )
        .unwrap();
    let glass = assets
        .insert("glass", MaterialAsset::unlit(Color::rgba(1.0, 1.0, 1.0, 0.5)).with_alpha_mode(AlphaMode::Blend))
        .unwrap();
    let solid = assets.insert("solid", MaterialAsset::unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();

    let mut kept = world();
    kept.spawn((Camera::orthographic(8.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -4.0)));
    kept.spawn((Transform::from_xyz(0.0, 0.0, 2.0), tinted(Color::rgb(1.0, 0.0, 0.0), 3.0).cutout().unlit()));
    kept.spawn((
        Transform::IDENTITY,
        Sprite::new("quad").with_size(Vec2::splat(3.0)).with_anchor(Vec2::splat(0.5)).with_material(keep),
    ));
    let (image, _) = render(&gpu, &mut renderer, &mut kept, &assets);
    center(&image, W, [0, 255, 0, 255], 2, "a kept mask occludes the farther cutout");

    let mut dropped = world();
    dropped.spawn((Camera::orthographic(8.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -4.0)));
    dropped.spawn((Transform::from_xyz(0.0, 0.0, 2.0), tinted(Color::rgb(1.0, 0.0, 0.0), 3.0).cutout().unlit()));
    dropped.spawn((
        Transform::IDENTITY,
        Sprite::new("quad").with_size(Vec2::splat(3.0)).with_anchor(Vec2::splat(0.5)).with_material(drop),
    ));
    let (image, _) = render(&gpu, &mut renderer, &mut dropped, &assets);
    center(&image, W, [255, 0, 0, 255], 2, "a discarded mask does not occlude");

    let mut blended = world();
    blended.spawn((Camera::orthographic(8.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -4.0)));
    blended.spawn((Transform::from_xyz(0.0, 0.0, 2.0), tinted(Color::rgb(1.0, 0.0, 0.0), 3.0).cutout().unlit()));
    blended.spawn((
        Transform::IDENTITY,
        Sprite::new("quad").with_size(Vec2::splat(3.0)).with_anchor(Vec2::splat(0.5)).with_material(glass),
    ));
    let (image, _) = render(&gpu, &mut renderer, &mut blended, &assets);
    center(&image, W, [255, 127, 127, 255], 4, "a nearer blend composites over the cutout");

    let mut covered = world();
    covered.spawn((Camera::orthographic(8.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -4.0)));
    covered.spawn((
        Transform::from_xyz(0.0, 0.0, 2.0),
        Sprite::new("quad").with_size(Vec2::splat(3.0)).with_anchor(Vec2::splat(0.5)).with_material(glass),
    ));
    covered.spawn((
        Transform::IDENTITY,
        Sprite::new("quad").with_size(Vec2::splat(3.0)).with_anchor(Vec2::splat(0.5)).with_material(solid),
    ));
    let (image, _) = render(&gpu, &mut renderer, &mut covered, &assets);
    center(&image, W, [255, 0, 0, 255], 2, "a nearer opaque hides a farther blend");
    assert_eq!(renderer.stats().camera_passes, 1);
}

#[test]
fn perspective_separates_a_game_scale_gap_at_distance_200() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let gap = 1.0 / 1024.0;
    let camera = Camera::perspective(45.0).with_clear_color(Color::BLACK);
    let mut world = pair(
        camera,
        Transform::IDENTITY,
        (Vec3::new(0.0, 0.0, 200.0), Color::rgb(1.0, 0.0, 0.0), 40.0),
        (Vec3::new(0.0, 0.0, 200.0 + gap), Color::rgb(0.0, 0.0, 1.0), 40.0),
    );
    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    center(&image, W, [255, 0, 0, 255], 2, "perspective reverse-Z keeps 1/1024 at z=200");
    let stats = renderer.stats();
    assert_eq!((stats.camera_passes, stats.world_draw_calls, stats.submitted_instances), (1, 1, 2));
    assert_eq!(scene.stats.sprites_visible, 2);

    let ortho = Camera {
        projection: CameraProjection::orthographic(10.0).with_clip_range(0.1, 1000.0),
        ..Camera::orthographic(10.0).with_clear_color(Color::BLACK)
    };
    let mut world = pair(
        ortho,
        Transform::IDENTITY,
        (Vec3::new(0.0, 0.0, 200.0), Color::rgb(1.0, 0.0, 0.0), 4.0),
        (Vec3::new(0.0, 0.0, 200.0 + gap), Color::rgb(0.0, 0.0, 1.0), 4.0),
    );
    let (image, _) = render(&gpu, &mut renderer, &mut world, &assets);
    center(&image, W, [255, 0, 0, 255], 2, "orthographic reverse-Z keeps 1/1024 at z=200");
    assert_eq!(renderer.stats().world_draw_calls, 1);
}

#[test]
fn near_and_far_planes_clip() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let camera = Camera {
        projection: CameraProjection::perspective(60.0).with_clip_range(0.1, 20.0),
        ..Camera::perspective(60.0).with_clear_color(Color::BLACK)
    };
    let mut world = world();
    world.spawn((camera, Transform::IDENTITY));
    world.spawn((Transform::from_xyz(0.0, 0.0, 0.02), tinted(Color::rgb(0.0, 1.0, 0.0), 0.01)));
    world.spawn((Transform::from_xyz(0.0, 0.0, 5.0), tinted(Color::rgb(1.0, 0.0, 0.0), 2.0)));
    world.spawn((Transform::from_xyz(0.0, 0.0, 40.0), tinted(Color::rgb(0.0, 0.0, 1.0), 0.4)));
    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!((scene.stats.sprites_visible, scene.stats.sprites_culled), (1, 2));
    center(&image, W, [255, 0, 0, 255], 2, "only the sprite inside the clip range is drawn");
}

#[test]
fn a_stored_standard_z_matrix_is_not_reinterpreted() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let (left, right, bottom, top, near, far) = (-5.0, 5.0, -5.0, 5.0, 0.1, 100.0);
    let d = far - near;
    let standard = Mat4::from_cols(
        Vec4::new(2.0 / (right - left), 0.0, 0.0, 0.0),
        Vec4::new(0.0, 2.0 / (top - bottom), 0.0, 0.0),
        Vec4::new(0.0, 0.0, 1.0 / d, 0.0),
        Vec4::new(0.0, 0.0, -near / d, 1.0),
    );
    let red = (Vec3::new(0.0, 0.0, 4.0), Color::rgb(1.0, 0.0, 0.0), 2.0);
    let blue = (Vec3::new(0.0, 0.0, 8.0), Color::rgb(0.0, 0.0, 1.0), 2.0);
    let mut stored = pair(
        Camera::new(CameraProjection::custom(standard).unwrap()).with_clear_color(Color::BLACK),
        Transform::IDENTITY,
        red,
        blue,
    );
    let (image, _) = render(&gpu, &mut renderer, &mut stored, &assets);
    center(&image, W, [0, 0, 255, 255], 2, "a stored 0-near matrix still lets the farther sprite win");

    let reverse = Mat4::orthographic_lh(left, right, bottom, top, near, far);
    let mut rebuilt = pair(
        Camera::new(CameraProjection::custom(reverse).unwrap()).with_clear_color(Color::BLACK),
        Transform::IDENTITY,
        red,
        blue,
    );
    let (image, _) = render(&gpu, &mut renderer, &mut rebuilt, &assets);
    center(&image, W, [255, 0, 0, 255], 2, "rebuilding with orthographic_lh follows reverse-Z");
}

#[test]
fn an_oblique_custom_matrix_keeps_view_depth_order() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let ortho = CameraProjection::orthographic(10.0).matrix(1.0);
    let shear = Mat4::from_cols(
        Vec4::new(1.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 1.0, 0.0, 0.0),
        Vec4::new(0.25, 0.0, 1.0, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    );
    let mut world = pair(
        Camera::new(CameraProjection::custom(ortho * shear).unwrap()).with_clear_color(Color::BLACK),
        Transform::IDENTITY,
        (Vec3::new(0.0, 0.0, 4.0), Color::rgb(1.0, 0.0, 0.0), 2.0),
        (Vec3::new(0.0, 0.0, 8.0), Color::rgb(0.0, 0.0, 1.0), 2.0),
    );
    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    let nearer = scene.sole_pass().view.project(Vec3::new(0.0, 0.0, 4.0)).unwrap();
    let farther = scene.sole_pass().view.project(Vec3::new(0.0, 0.0, 8.0)).unwrap();
    assert!(nearer.depth > farther.depth);
    center(&image, W, [255, 0, 0, 255], 2, "shear does not reverse occlusion");
    assert_eq!(renderer.stats().camera_passes, 1);
}

#[test]
fn a_later_camera_clears_depth_before_its_own_draws() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut world = world();
    world.spawn((
        Camera { order: 0, ..Camera::orthographic(4.0).with_clear_color(Color::BLACK) },
        Transform::from_xyz(0.0, 0.0, -4.0),
    ));
    world.spawn((
        Camera { order: 1, clear: CameraClear::Load, ..Camera::orthographic(4.0).with_clear_color(Color::BLACK) },
        Transform::from_xyz(100.0, 0.0, -4.0),
    ));
    world.spawn((Transform::from_xyz(0.0, 0.0, 0.0), tinted(Color::rgb(1.0, 0.0, 0.0), 2.0)));
    world.spawn((Transform::from_xyz(100.0, 0.0, 2.0), tinted(Color::rgb(0.0, 0.0, 1.0), 2.0)));
    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(scene.passes.len(), 2);
    center(&image, W, [0, 0, 255, 255], 2, "camera B is not rejected by camera A's depth");
    let stats = renderer.stats();
    assert_eq!((stats.camera_passes, stats.clear_draw_calls, stats.world_draw_calls), (2, 0, 2));
    assert_eq!(stats.submitted_instances, 2);
}

#[test]
fn an_offscreen_camera_uses_the_same_depth_convention() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut world = world();
    world.insert_resource(RenderTargetStore::new());
    let target = world.resource_mut::<RenderTargetStore>().create(64, 64).unwrap();
    world.spawn((
        Camera { output: CameraOutput::Target(target), ..Camera::orthographic(4.0).with_clear_color(Color::BLACK) },
        Transform::from_xyz(0.0, 0.0, -4.0),
    ));
    world.spawn((Transform::from_xyz(0.0, 0.0, 0.0), tinted(Color::rgb(1.0, 0.0, 0.0), 2.0)));
    world.spawn((Transform::from_xyz(0.0, 0.0, 2.0), tinted(Color::rgb(0.0, 0.0, 1.0), 2.0)));
    let (_, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    let image = renderer.read_render_target(&gpu, target).expect("offscreen image");
    center(&image, 64, [255, 0, 0, 255], 2, "the offscreen nearer sprite wins");
    let stats = renderer.stats();
    assert_eq!((stats.camera_passes, stats.surface_passes, stats.offscreen_passes), (1, 0, 1));
    assert_eq!((stats.world_draw_calls, stats.submitted_instances), (1, 2));
    assert_eq!(scene.passes[0].output, CameraOutput::Target(target));
}
