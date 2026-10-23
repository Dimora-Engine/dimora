//! Coplanar priority on the production reverse-Z path.
//!
//! Opted-in sprites change depth only. Meshes, blend order, and sprites without the component
//! keep their existing tests. These cases are not ignored.

use engine_assets::{AssetManager, ColorSpace, MaterialAsset, MeshData, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Mat4, Quat, Transform, Vec2, Vec3, Vec4};
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

fn px(img: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * W + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

fn expect(img: &[u8], x: u32, y: u32, expected: [u8; 4], tolerance: i16, what: &str) {
    let pixel = px(img, x, y);
    let close = pixel.iter().zip(expected).all(|(got, want)| (*got as i16 - want as i16).abs() <= tolerance);
    assert!(close, "{what} at ({x},{y}) is {pixel:?}, expected {expected:?} ±{tolerance}");
}

fn center(img: &[u8], expected: [u8; 4], what: &str) {
    expect(img, W / 2, W / 2, expected, 2, what);
}

fn assets() -> AssetManager {
    let mut assets = AssetManager::in_memory();
    let white =
        assets.insert("white-tex", Texture::solid(1, 1, [255; 4]).with_color_space(ColorSpace::Linear)).unwrap();
    assets.insert("quad", SpriteAsset::whole_texture(white, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    assets
        .insert("paint", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(engine_assets::AlphaMode::Opaque))
        .unwrap();
    assets.insert("slab", MeshData::cuboid(Vec3::new(3.0, 3.0, 0.002)).unwrap()).unwrap();
    assets.insert("wall", MeshData::cuboid(Vec3::new(4.0, 4.0, 0.2)).unwrap()).unwrap();
    assets
}

fn scene_world() -> World {
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
    let scene = extract_scene(world, assets, Vec2::new(W as f32, W as f32)).unwrap();
    let image = renderer.render_to_rgba(gpu, &scene, assets, W, W).unwrap();
    (image, scene)
}

fn sprite(color: Color, size: f32, orientation: SpriteOrientation) -> Sprite {
    Sprite::new("quad")
        .with_size(Vec2::splat(size))
        .with_anchor(Vec2::splat(0.5))
        .with_tint(color)
        .with_orientation(orientation)
        .cutout()
        .unlit()
}

fn thickness(world: &mut World, value: f32) {
    world.insert_resource(CoplanarSettings::new(value).unwrap());
}

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

fn coincident(
    gpu: &GpuContext,
    renderer: &mut SceneRenderer,
    assets: &AssetManager,
    camera: Camera,
    pose: Transform,
    orientation: SpriteOrientation,
    size: f32,
) -> [u8; 4] {
    let mut world = scene_world();
    thickness(&mut world, CANDIDATE_COPLANAR_THICKNESS);
    // A fixed quad lives in local XY. Laying it in XZ is what a top-down camera can see.
    let sprite_pose = if orientation == SpriteOrientation::Fixed {
        Transform::from_rotation(Quat::from_rotation_x(core::f32::consts::FRAC_PI_2))
    } else {
        Transform::IDENTITY
    };
    world.spawn((camera.with_clear_color(Color::BLACK), pose));
    // The lower priority is spawned second. A tie that followed submission order would show blue.
    world.spawn((sprite_pose, sprite(Color::rgb(1.0, 0.0, 0.0), size, orientation), CoplanarPriority::new(3).unwrap()));
    world.spawn((sprite_pose, sprite(Color::rgb(0.0, 0.0, 1.0), size, orientation), CoplanarPriority::new(0).unwrap()));
    let (image, _) = render(gpu, renderer, &mut world, assets);
    px(&image, W / 2, W / 2)
}

#[test]
fn coincident_cutouts_keep_the_higher_priority_in_every_visible_configuration() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let cases = [
        (
            "perspective",
            Camera::perspective(45.0),
            Transform::from_xyz(0.0, 0.0, -8.0),
            SpriteOrientation::Billboard,
            4.0,
        ),
        (
            "orthographic",
            Camera::orthographic(10.0),
            Transform::from_xyz(0.0, 0.0, -8.0),
            SpriteOrientation::Billboard,
            4.0,
        ),
        (
            "side",
            Camera::orthographic(10.0),
            Transform::looking_at(Vec3::new(-8.0, 0.0, 0.0), Vec3::ZERO, Vec3::Y),
            SpriteOrientation::Billboard,
            4.0,
        ),
        (
            "steep",
            Camera::perspective(45.0),
            Transform::looking_at(Vec3::new(0.0, 8.0, -4.0), Vec3::ZERO, Vec3::Y),
            SpriteOrientation::Billboard,
            4.0,
        ),
        (
            "isometric",
            Camera::orthographic(12.0),
            Transform::looking_at(Vec3::new(8.0, 8.0, -8.0), Vec3::ZERO, Vec3::Y),
            SpriteOrientation::Billboard,
            4.0,
        ),
        (
            "top-down",
            Camera::orthographic(10.0),
            Transform::looking_at(Vec3::new(0.0, 10.0, 0.0), Vec3::ZERO, Vec3::Z),
            SpriteOrientation::Fixed,
            4.0,
        ),
    ];
    for (name, camera, pose, orientation, size) in cases {
        let pixel = coincident(&gpu, &mut renderer, &assets, camera, pose, orientation, size);
        assert_eq!(pixel, RED, "{name} should show the higher priority, got {pixel:?}");
    }

    let ortho = Mat4::orthographic_lh(-5.0, 5.0, -5.0, 5.0, 0.1, 100.0);
    let shear = Mat4::from_cols(
        Vec4::new(1.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 1.0, 0.0, 0.0),
        Vec4::new(0.2, 0.05, 1.0, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    );
    let custom = Camera::new(CameraProjection::custom(ortho * shear).unwrap());
    let pixel = coincident(
        &gpu,
        &mut renderer,
        &assets,
        custom,
        Transform::from_xyz(0.0, 0.0, -8.0),
        SpriteOrientation::Billboard,
        4.0,
    );
    assert_eq!(pixel, RED, "custom oblique should show the higher priority, got {pixel:?}");
}

#[test]
fn priority_holds_across_distance_and_camera_movement() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    for distance in [5.0, 40.0, 200.0] {
        let pixel = coincident(
            &gpu,
            &mut renderer,
            &assets,
            Camera::perspective(45.0),
            Transform::from_xyz(0.0, 0.0, -distance),
            SpriteOrientation::Billboard,
            distance * 0.35,
        );
        assert_eq!(pixel, RED, "perspective at {distance} got {pixel:?}");
    }
    for z in [-6.0, -30.0, -80.0] {
        let pixel = coincident(
            &gpu,
            &mut renderer,
            &assets,
            Camera::orthographic(10.0),
            Transform::from_xyz(0.0, 0.0, z),
            SpriteOrientation::Billboard,
            4.0,
        );
        assert_eq!(pixel, RED, "orthographic camera at {z} got {pixel:?}");
    }
}

fn slab_at(z: f32, color: Color) -> (Transform, Mesh) {
    (Transform::from_xyz(0.0, 0.0, z), Mesh::new("slab").with_material(Material::unlit(color)))
}

fn override_at(
    gpu: &GpuContext,
    renderer: &mut SceneRenderer,
    assets: &AssetManager,
    gap: f32,
    step: f32,
    level: u8,
) -> [u8; 4] {
    let mut world = scene_world();
    thickness(&mut world, step);
    world.spawn((Camera::orthographic(6.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -8.0)));
    world.spawn((
        Transform::IDENTITY,
        sprite(Color::rgb(1.0, 0.0, 0.0), 2.0, SpriteOrientation::Billboard),
        CoplanarPriority::new(level).unwrap(),
    ));
    // The slab center is `gap` closer to the camera than the sprite. Half the slab is 0.001.
    world.spawn(slab_at(-gap, Color::rgb(0.0, 0.0, 1.0)));
    let (image, _) = render(gpu, renderer, &mut world, assets);
    px(&image, W / 2, W / 2)
}

#[test]
fn the_candidate_thickness_overrides_only_inside_its_documented_pull() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    // Level 7 pull is 8 * thickness. A nearer mesh beyond that pull stays visible.
    let pull = |step: f32| 8.0 * step;
    for step in [1.0 / 1024.0, CANDIDATE_COPLANAR_THICKNESS, 1.0 / 64.0] {
        let inside = pull(step) * 0.35;
        let outside = pull(step) * 2.0;
        let overridden = override_at(&gpu, &mut renderer, &assets, inside, step, 7);
        let kept = override_at(&gpu, &mut renderer, &assets, outside, step, 7);
        assert_eq!(overridden, RED, "step {step}: a mesh {inside} closer should be overridden, got {overridden:?}");
        assert_eq!(kept, BLUE, "step {step}: a mesh {outside} closer should stay visible, got {kept:?}");
    }
}

#[test]
fn a_cutout_with_priority_wins_against_a_coplanar_mesh_and_a_plain_sprite_does_not_move() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut opted = scene_world();
    thickness(&mut opted, CANDIDATE_COPLANAR_THICKNESS);
    opted.spawn((Camera::orthographic(6.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -8.0)));
    opted.spawn((
        Transform::IDENTITY,
        sprite(Color::rgb(1.0, 0.0, 0.0), 2.0, SpriteOrientation::Billboard),
        CoplanarPriority::new(1).unwrap(),
    ));
    opted.spawn(slab_at(0.0, Color::rgb(0.0, 0.0, 1.0)));
    let (after, _) = render(&gpu, &mut renderer, &mut opted, &assets);
    let after_px = px(&after, W / 2, W / 2);
    assert_eq!(after_px, RED, "an opted-in cutout is in front of the coplanar slab, got {after_px:?}");
    // The plain pair is a tie, so its colour is not a contract. A mesh one thickness closer is
    // beyond a plain sprite and inside a level-1 pull (two thicknesses).
    let gap = CANDIDATE_COPLANAR_THICKNESS;
    let mut nearer = scene_world();
    nearer.spawn((Camera::orthographic(6.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -8.0)));
    nearer.spawn((Transform::IDENTITY, sprite(Color::rgb(1.0, 0.0, 0.0), 2.0, SpriteOrientation::Billboard)));
    nearer.spawn(slab_at(-gap, Color::rgb(0.0, 0.0, 1.0)));
    let (plain_near, _) = render(&gpu, &mut renderer, &mut nearer, &assets);
    assert_eq!(px(&plain_near, W / 2, W / 2), BLUE, "a plain sprite does not cross a mesh one thickness closer");
    let pulled = override_at(&gpu, &mut renderer, &assets, gap, CANDIDATE_COPLANAR_THICKNESS, 1);
    assert_eq!(pulled, RED, "level 1 reaches across that same gap, got {pulled:?}");
}

#[test]
fn blend_order_ignores_priority_and_depth_still_hides_a_blend_behind_a_cutout() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut blends = scene_world();
    thickness(&mut blends, 1.0 / 64.0);
    blends.spawn((Camera::orthographic(6.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -8.0)));
    blends.spawn((
        Transform::IDENTITY,
        Sprite::new("quad")
            .with_size(Vec2::splat(2.0))
            .with_anchor(Vec2::splat(0.5))
            .with_tint(Color::rgb(0.0, 0.0, 1.0))
            .unlit(),
        RenderOrder::new(0, 0),
        CoplanarPriority::new(7).unwrap(),
    ));
    blends.spawn((
        Transform::IDENTITY,
        Sprite::new("quad")
            .with_size(Vec2::splat(2.0))
            .with_anchor(Vec2::splat(0.5))
            .with_tint(Color::rgb(1.0, 0.0, 0.0))
            .unlit(),
        RenderOrder::new(0, 1),
        CoplanarPriority::new(0).unwrap(),
    ));
    let (image, scene) = render(&gpu, &mut renderer, &mut blends, &assets);
    center(&image, RED, "the later RenderOrder stays on top of the higher priority blend");
    assert!(scene.sole_pass().blended_sprites.instances.iter().any(|i| i.coplanar_level == Some(7)));

    let mut behind = scene_world();
    thickness(&mut behind, CANDIDATE_COPLANAR_THICKNESS);
    behind.spawn((Camera::orthographic(6.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -8.0)));
    behind.spawn((Transform::IDENTITY, sprite(Color::rgb(1.0, 0.0, 0.0), 2.0, SpriteOrientation::Billboard)));
    behind.spawn((
        Transform::from_xyz(0.0, 0.0, 0.5),
        Sprite::new("quad")
            .with_size(Vec2::splat(2.0))
            .with_anchor(Vec2::splat(0.5))
            .with_tint(Color::rgb(0.0, 0.0, 1.0))
            .unlit(),
        CoplanarPriority::new(7).unwrap(),
    ));
    let (image, _) = render(&gpu, &mut renderer, &mut behind, &assets);
    center(&image, RED, "a blend 0.5 behind a cutout stays hidden at the candidate pull");

    let mut over = scene_world();
    thickness(&mut over, CANDIDATE_COPLANAR_THICKNESS);
    over.spawn((Camera::orthographic(6.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -8.0)));
    over.spawn((Transform::IDENTITY, sprite(Color::rgb(1.0, 0.0, 0.0), 2.0, SpriteOrientation::Billboard)));
    over.spawn((
        Transform::IDENTITY,
        Sprite::new("quad")
            .with_size(Vec2::splat(2.0))
            .with_anchor(Vec2::splat(0.5))
            .with_tint(Color { r: 0.0, g: 0.0, b: 1.0, a: 0.5 })
            .unlit(),
        CoplanarPriority::new(4).unwrap(),
    ));
    let (image, _) = render(&gpu, &mut renderer, &mut over, &assets);
    let pixel = px(&image, W / 2, W / 2);
    assert!(pixel[2] > 80 && pixel[0] > 80, "a coplanar blend with priority composites over the cutout, got {pixel:?}");
}

#[test]
fn same_material_priorities_share_one_draw_and_runtime_changes_swap_the_winner() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let paint = engine_assets::AssetHandle::from_name("paint");
    let mut world = scene_world();
    world.spawn((Camera::orthographic(6.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -8.0)));
    let low = world.spawn((
        Transform::IDENTITY,
        Sprite::new("quad")
            .with_size(Vec2::splat(2.0))
            .with_anchor(Vec2::splat(0.5))
            .with_material(paint)
            .with_tint(Color::rgb(0.0, 0.0, 1.0)),
        CoplanarPriority::new(0).unwrap(),
    ));
    world.spawn((
        Transform::IDENTITY,
        Sprite::new("quad")
            .with_size(Vec2::splat(2.0))
            .with_anchor(Vec2::splat(0.5))
            .with_material(paint)
            .with_tint(Color::rgb(1.0, 0.0, 0.0)),
        CoplanarPriority::new(2).unwrap(),
    ));
    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    center(&image, RED, "the higher priority of a shared material wins");
    assert_eq!(scene.sole_pass().opaque_sprites.batches.len(), 1);
    let stats = renderer.stats();
    assert_eq!(stats.world_draw_calls, 1);
    assert_eq!(stats.submitted_instances, 2);
    assert_eq!(stats.camera_passes, 1);

    *world.component_mut::<CoplanarPriority>(low).unwrap() = CoplanarPriority::new(7).unwrap();
    let (image, _) = render(&gpu, &mut renderer, &mut world, &assets);
    center(&image, BLUE, "raising the other sprite's priority at runtime puts it in front");
}

#[test]
fn an_unmodified_sprite_and_mesh_keep_spatial_order_and_one_batch() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut world = scene_world();
    world.spawn((Camera::perspective(45.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -8.0)));
    world.spawn((
        Transform::from_xyz(-0.4, 0.0, 0.0),
        sprite(Color::rgb(1.0, 0.0, 0.0), 1.2, SpriteOrientation::Billboard),
    ));
    world.spawn((
        Transform::from_xyz(0.4, 0.0, 0.0),
        sprite(Color::rgb(0.0, 1.0, 0.0), 1.2, SpriteOrientation::Billboard),
    ));
    world.spawn((
        Transform::from_xyz(0.0, 0.0, -1.0),
        Mesh::new("slab").with_material(Material::unlit(Color::rgb(0.0, 0.0, 1.0))),
    ));
    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    center(&image, BLUE, "a nearer mesh still occludes sprites that did not opt in");
    assert!(scene.sole_pass().cutout_sprites.instances.iter().all(|i| i.coplanar_level.is_none()));
    assert_eq!(scene.sole_pass().cutout_sprites.batches.len(), 1);
    assert_eq!(renderer.stats().camera_passes, 1);
    assert_eq!(scene.coplanar_thickness, CANDIDATE_COPLANAR_THICKNESS);
}

#[test]
fn near_plane_clamps_an_opted_in_sprite_and_still_clips_one_in_front_of_it() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut world = scene_world();
    thickness(&mut world, 1.0 / 64.0);
    world.spawn((
        Camera::new(CameraProjection::perspective(45.0).with_clip_range(0.1, 100.0)).with_clear_color(Color::BLACK),
        Transform::IDENTITY,
    ));
    // 0.02 past the near plane. Level 7 at 1/64 pulls 0.125, which would cross the near plane
    // if the depth write were not clamped.
    world.spawn((
        Transform::from_xyz(0.0, 0.0, 0.12),
        sprite(Color::rgb(1.0, 0.0, 0.0), 1.0, SpriteOrientation::Billboard),
        CoplanarPriority::new(7).unwrap(),
    ));
    let (image, _) = render(&gpu, &mut renderer, &mut world, &assets);
    center(&image, RED, "the clamped sprite stays inside the near plane");

    let mut clipped = scene_world();
    thickness(&mut clipped, 1.0 / 64.0);
    clipped.spawn((
        Camera::new(CameraProjection::perspective(45.0).with_clip_range(0.1, 100.0)).with_clear_color(Color::BLACK),
        Transform::IDENTITY,
    ));
    clipped.spawn((
        Transform::from_xyz(0.0, 0.0, 0.02),
        sprite(Color::rgb(1.0, 0.0, 0.0), 1.0, SpriteOrientation::Billboard),
        CoplanarPriority::new(7).unwrap(),
    ));
    let (image, _) = render(&gpu, &mut renderer, &mut clipped, &assets);
    center(&image, [0, 0, 0, 255], "a sprite in front of the near plane is still clipped");
}

#[test]
fn two_cameras_and_an_offscreen_target_keep_independent_depth() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut world = scene_world();
    thickness(&mut world, CANDIDATE_COPLANAR_THICKNESS);
    world.spawn((
        Camera::orthographic(6.0).with_clear_color(Color::BLACK).with_viewport(ViewportRect::new(0.0, 0.0, 0.5, 1.0)),
        Transform::from_xyz(0.0, 0.0, -8.0),
    ));
    world.spawn((
        Camera {
            order: 1,
            ..Camera::orthographic(6.0)
                .with_clear_color(Color::BLACK)
                .with_viewport(ViewportRect::new(0.5, 0.0, 0.5, 1.0))
        },
        Transform::from_xyz(0.0, 0.0, -8.0),
    ));
    world.spawn((
        Transform::IDENTITY,
        sprite(Color::rgb(1.0, 0.0, 0.0), 2.0, SpriteOrientation::Billboard),
        CoplanarPriority::new(2).unwrap(),
    ));
    world.spawn((
        Transform::IDENTITY,
        sprite(Color::rgb(0.0, 0.0, 1.0), 2.0, SpriteOrientation::Billboard),
        CoplanarPriority::new(0).unwrap(),
    ));
    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    expect(&image, W / 4, W / 2, RED, 2, "left camera");
    expect(&image, 3 * W / 4, W / 2, RED, 2, "right camera");
    assert_eq!(scene.passes.len(), 2);
    assert_eq!(renderer.stats().camera_passes, 2);
    assert!(renderer.stats().world_draw_calls >= 2);

    let mut offscreen = scene_world();
    thickness(&mut offscreen, CANDIDATE_COPLANAR_THICKNESS);
    offscreen.insert_resource(RenderTargetStore::new());
    let target = offscreen.resource_mut::<RenderTargetStore>().create(64, 64).unwrap();
    offscreen.spawn((
        Camera { output: CameraOutput::Target(target), ..Camera::orthographic(4.0).with_clear_color(Color::BLACK) },
        Transform::from_xyz(0.0, 0.0, -6.0),
    ));
    offscreen.spawn((
        Transform::IDENTITY,
        sprite(Color::rgb(1.0, 0.0, 0.0), 2.0, SpriteOrientation::Billboard),
        CoplanarPriority::new(1).unwrap(),
    ));
    offscreen.spawn((Transform::IDENTITY, sprite(Color::rgb(0.0, 0.0, 1.0), 2.0, SpriteOrientation::Billboard)));
    let (_, scene) = render(&gpu, &mut renderer, &mut offscreen, &assets);
    let image = renderer.read_render_target(&gpu, target).unwrap();
    let i = ((32u32 * 64 + 32) * 4) as usize;
    let pixel = [image[i], image[i + 1], image[i + 2], image[i + 3]];
    assert_eq!(pixel, RED, "offscreen target got {pixel:?}");
    assert_eq!(scene.passes[0].output, CameraOutput::Target(target));
    assert_eq!(renderer.stats().offscreen_passes, 1);
}

#[test]
fn an_intersection_keeps_its_silhouette_when_priority_changes_depth() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut wall = |priority: Option<u8>| {
        let mut world = scene_world();
        thickness(&mut world, CANDIDATE_COPLANAR_THICKNESS);
        world.spawn((Camera::orthographic(8.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -8.0)));
        let sprite = sprite(Color::rgb(1.0, 0.0, 0.0), 1.2, SpriteOrientation::Billboard);
        if let Some(level) = priority {
            world.spawn((Transform::IDENTITY, sprite, CoplanarPriority::new(level).unwrap()));
        } else {
            world.spawn((Transform::IDENTITY, sprite));
        }
        world.spawn((
            Transform::from_rotation(Quat::from_rotation_y(0.7)),
            Mesh::new("wall").with_material(Material::unlit(Color::rgb(0.0, 0.0, 1.0))),
        ));
        render(&gpu, &mut renderer, &mut world, &assets).0
    };
    let plain = wall(None);
    let opted = wall(Some(7));
    assert_eq!(px(&plain, 4, 4), px(&opted, 4, 4), "a pixel outside the sprite quad must not move");
    let red = |img: &[u8]| img.chunks(4).filter(|p| p[0] > 200 && p[2] < 40).count();
    let blue = |img: &[u8]| img.chunks(4).filter(|p| p[2] > 200 && p[0] < 40).count();
    assert!(
        red(&plain) > 10 && blue(&plain) > 10,
        "the crossing shows both surfaces: red {} blue {}",
        red(&plain),
        blue(&plain)
    );
    assert!(red(&opted) > 10 && blue(&opted) > 10, "priority does not erase the mesh outside the pull");
    assert!(red(&opted) >= red(&plain), "the pull should not hand pixels back to the mesh");
}

#[test]
fn many_compatible_sprites_stay_one_draw_when_their_priorities_differ() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut world = scene_world();
    world.spawn((Camera::orthographic(40.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -20.0)));
    for i in 0..64u8 {
        world.spawn((
            Transform::from_xyz((i % 8) as f32 * 2.0 - 7.0, (i / 8) as f32 * 2.0 - 7.0, 0.0),
            sprite(Color::rgb(1.0, 0.0, 0.0), 1.0, SpriteOrientation::Billboard),
            CoplanarPriority::new(i % 8).unwrap(),
        ));
    }
    let (_, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(scene.sole_pass().cutout_sprites.batches.len(), 1);
    assert_eq!(renderer.stats().world_draw_calls, 1);
    assert_eq!(renderer.stats().submitted_instances, 64);
    assert_eq!(renderer.stats().camera_passes, 1);
}
