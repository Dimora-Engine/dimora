//! GPU readback for procedural meshes.
//!
//! The generators are ordinary mesh assets. These tests use the existing mesh pipelines, materials,
//! and batch key. Draw counts come from `FrameStats`, which increments at each draw submission.

use engine_assets::{AlphaMode, AssetManager, ColorSpace, MaterialAsset, MeshData, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Quat, Transform, Vec2, Vec3};
use engine_renderer::{AmbientLight, Camera, Light, Material, Mesh, Sprite};
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
    let scene = engine_renderer::extract_scene(world, assets, Vec2::new(W as f32, H as f32)).unwrap();
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

fn world(camera: Camera, transform: Transform) -> World {
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.spawn((camera.with_clear_color(Color::BLACK), transform));
    world
}

fn ortho_at(position: Vec3, target: Vec3) -> World {
    world(Camera::orthographic(2.0), Transform::looking_at(position, target, Vec3::Y))
}

fn red_material(assets: &mut AssetManager) -> engine_assets::AssetHandle<MaterialAsset> {
    assets.insert("red", MaterialAsset::unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap()
}

fn spawn_mesh(
    world: &mut World,
    name: &str,
    material: engine_assets::AssetHandle<MaterialAsset>,
    transform: Transform,
) {
    world.spawn((transform, Mesh::new(name).with_material_asset(material)));
}

#[test]
fn a_plane_is_visible_from_above_and_culled_from_below() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    assets.insert("plane", MeshData::plane(1.5, 1.5).unwrap()).unwrap();
    let material = red_material(&mut assets);
    let mut above =
        world(Camera::orthographic(2.0), Transform::looking_at(Vec3::new(0.0, 4.0, 0.0), Vec3::ZERO, Vec3::Z));
    spawn_mesh(&mut above, "plane", material, Transform::IDENTITY);
    let image = render(&gpu, &mut renderer, &mut above, &assets);
    expect_px(&image, 64, 64, [255, 0, 0, 255], 2, "the +Y side of the plane");

    let mut below =
        world(Camera::orthographic(2.0), Transform::looking_at(Vec3::new(0.0, -4.0, 0.0), Vec3::ZERO, Vec3::Z));
    spawn_mesh(&mut below, "plane", material, Transform::IDENTITY);
    let image = render(&gpu, &mut renderer, &mut below, &assets);
    expect_px(&image, 64, 64, [0, 0, 0, 255], 2, "back-face culling hides the plane from below");

    let both = assets.insert("both", MaterialAsset::unlit(Color::rgb(0.0, 1.0, 0.0)).double_sided()).unwrap();
    let mut open =
        world(Camera::orthographic(2.0), Transform::looking_at(Vec3::new(0.0, -4.0, 0.0), Vec3::ZERO, Vec3::Z));
    spawn_mesh(&mut open, "plane", both, Transform::IDENTITY);
    let image = render(&gpu, &mut renderer, &mut open, &assets);
    expect_px(&image, 64, 64, [0, 255, 0, 255], 2, "double_sided shows the plane from below");
}

#[test]
fn a_cube_shows_its_exterior_from_four_sides() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    assets.insert("cube", MeshData::cube(1.2).unwrap()).unwrap();
    let material = red_material(&mut assets);
    let views =
        [Vec3::new(0.0, 0.0, -4.0), Vec3::new(0.0, 0.0, 4.0), Vec3::new(4.0, 0.0, 0.0), Vec3::new(0.0, 4.0, 0.2)];
    for position in views {
        let up =
            if position.y.abs() > position.x.abs() && position.y.abs() > position.z.abs() { Vec3::Z } else { Vec3::Y };
        let mut world = world(Camera::orthographic(2.0), Transform::looking_at(position, Vec3::ZERO, up));
        spawn_mesh(&mut world, "cube", material, Transform::IDENTITY);
        let image = render(&gpu, &mut renderer, &mut world, &assets);
        expect_px(&image, 64, 64, [255, 0, 0, 255], 2, "cube exterior");
    }
}

#[test]
fn cuboid_shapes_transforms_and_face_uvs_render() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    let stripe = Texture::from_fn(2, 1, |x, _| if x == 0 { [255, 0, 0, 255] } else { [0, 0, 255, 255] })
        .with_color_space(ColorSpace::Linear);
    let stripe = assets.insert("stripe", stripe).unwrap();
    let mut textured = MaterialAsset::unlit(Color::WHITE);
    textured.albedo = Some(stripe);
    let textured = assets.insert("striped", textured).unwrap();
    assets.insert("box", MeshData::cube(2.0).unwrap()).unwrap();
    let views = [
        (Vec3::new(0.0, 0.0, -4.0), Vec3::Y),
        (Vec3::new(0.0, 0.0, 4.0), Vec3::Y),
        (Vec3::new(4.0, 0.0, 0.0), Vec3::Y),
        (Vec3::new(-4.0, 0.0, 0.0), Vec3::Y),
        (Vec3::new(0.0, 4.0, 0.0), Vec3::Z),
        (Vec3::new(0.0, -4.0, 0.0), Vec3::Z),
    ];
    for (position, up) in views {
        let mut world = world(Camera::orthographic(2.0), Transform::looking_at(position, Vec3::ZERO, up));
        spawn_mesh(&mut world, "box", textured, Transform::IDENTITY);
        let image = render(&gpu, &mut renderer, &mut world, &assets);
        let left = px(&image, 32, 64);
        let right = px(&image, 96, 64);
        assert_ne!(left, right, "face UVs vary across the view from {position:?}: {left:?} vs {right:?}");
        for pixel in [left, right] {
            assert!(pixel == [255, 0, 0, 255] || pixel == [0, 0, 255, 255], "face sample {pixel:?} from {position:?}");
        }
    }

    let flat = assets.insert("flat", MaterialAsset::unlit(Color::rgb(0.0, 1.0, 0.0))).unwrap();
    assets.insert("wall", MeshData::cuboid(Vec3::new(0.15, 1.6, 1.6)).unwrap()).unwrap();
    assets.insert("slab", MeshData::cuboid(Vec3::new(1.6, 0.1, 1.6)).unwrap()).unwrap();
    assets.insert("beam", MeshData::cuboid(Vec3::new(1.8, 0.3, 0.2)).unwrap()).unwrap();
    let mut wall = ortho_at(Vec3::new(0.0, 0.0, -4.0), Vec3::ZERO);
    spawn_mesh(&mut wall, "wall", flat, Transform::IDENTITY);
    let image = render(&gpu, &mut renderer, &mut wall, &assets);
    expect_px(&image, 64, 64, [0, 255, 0, 255], 2, "thin wall");

    let mut slab =
        world(Camera::orthographic(2.0), Transform::looking_at(Vec3::new(0.0, 4.0, 0.0), Vec3::ZERO, Vec3::Z));
    spawn_mesh(&mut slab, "slab", flat, Transform::IDENTITY);
    let image = render(&gpu, &mut renderer, &mut slab, &assets);
    expect_px(&image, 64, 64, [0, 255, 0, 255], 2, "flat slab from above");

    let mut beam = ortho_at(Vec3::new(0.0, 0.0, -4.0), Vec3::ZERO);
    spawn_mesh(&mut beam, "beam", flat, Transform::from_rotation(Quat::from_rotation_y(0.4)));
    let image = render(&gpu, &mut renderer, &mut beam, &assets);
    expect_px(&image, 64, 64, [0, 255, 0, 255], 2, "rotated beam");

    let mut scaled = ortho_at(Vec3::new(0.0, 0.0, -4.0), Vec3::ZERO);
    spawn_mesh(&mut scaled, "box", flat, Transform::from_scale(Vec3::new(0.4, 0.7, 0.2)));
    let image = render(&gpu, &mut renderer, &mut scaled, &assets);
    expect_px(&image, 64, 64, [0, 255, 0, 255], 2, "nonuniform scale");

    let mut culled = ortho_at(Vec3::new(0.0, 0.0, -4.0), Vec3::ZERO);
    spawn_mesh(&mut culled, "box", flat, Transform::from_xyz(30.0, 0.0, 0.0));
    let image = render(&gpu, &mut renderer, &mut culled, &assets);
    expect_px(&image, 64, 64, [0, 0, 0, 255], 2, "a box outside the frustum is culled");
    assert_eq!(renderer.stats().mesh_instances, 0);
}

#[test]
fn sphere_and_cylinder_faces_point_outward() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    assets.insert("ball", MeshData::sphere(0.7, 16, 8).unwrap()).unwrap();
    assets.insert("tube", MeshData::cylinder(0.55, 1.4, 16).unwrap()).unwrap();
    let material = red_material(&mut assets);
    let mut ball = ortho_at(Vec3::new(0.0, 0.0, -4.0), Vec3::ZERO);
    spawn_mesh(&mut ball, "ball", material, Transform::IDENTITY);
    let image = render(&gpu, &mut renderer, &mut ball, &assets);
    expect_px(&image, 64, 64, [255, 0, 0, 255], 2, "sphere exterior");

    let mut side = ortho_at(Vec3::new(0.0, 0.0, -4.0), Vec3::ZERO);
    spawn_mesh(&mut side, "tube", material, Transform::IDENTITY);
    let image = render(&gpu, &mut renderer, &mut side, &assets);
    expect_px(&image, 64, 64, [255, 0, 0, 255], 2, "cylinder side");

    let mut top =
        world(Camera::orthographic(2.0), Transform::looking_at(Vec3::new(0.0, 4.0, 0.0), Vec3::ZERO, Vec3::Z));
    spawn_mesh(&mut top, "tube", material, Transform::IDENTITY);
    let image = render(&gpu, &mut renderer, &mut top, &assets);
    expect_px(&image, 64, 64, [255, 0, 0, 255], 2, "cylinder top cap");

    let mut bottom =
        world(Camera::orthographic(2.0), Transform::looking_at(Vec3::new(0.0, -4.0, 0.0), Vec3::ZERO, Vec3::Z));
    spawn_mesh(&mut bottom, "tube", material, Transform::IDENTITY);
    let image = render(&gpu, &mut renderer, &mut bottom, &assets);
    expect_px(&image, 64, 64, [255, 0, 0, 255], 2, "cylinder bottom cap");

    assets.insert("open", MeshData::cylinder_with_caps(0.55, 1.4, 16, false).unwrap()).unwrap();
    let mut open =
        world(Camera::orthographic(2.0), Transform::looking_at(Vec3::new(0.0, 4.0, 0.0), Vec3::ZERO, Vec3::Z));
    spawn_mesh(&mut open, "open", material, Transform::IDENTITY);
    let image = render(&gpu, &mut renderer, &mut open, &assets);
    expect_px(&image, 64, 64, [0, 0, 0, 255], 2, "an open cylinder has no top cap");
}

#[test]
fn flat_cube_faces_take_different_light() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    assets.insert("cube", MeshData::cube(1.2).unwrap()).unwrap();
    let lit = assets.insert("lit", MaterialAsset::color(Color::WHITE)).unwrap();
    let unlit = assets.insert("unlit", MaterialAsset::unlit(Color::WHITE)).unwrap();
    let light = Transform::from_rotation(Quat::look_rotation(-Vec3::X, Vec3::Y));

    let mut bright = ortho_at(Vec3::new(4.0, 0.0, 0.0), Vec3::ZERO);
    bright.insert_resource(AmbientLight { color: Color::WHITE, intensity: 0.0 });
    bright.spawn((light, Light::directional(Color::WHITE, 1.0)));
    spawn_mesh(&mut bright, "cube", lit, Transform::IDENTITY);
    let image = render(&gpu, &mut renderer, &mut bright, &assets);
    expect_px(&image, 64, 64, [255, 255, 255, 255], 8, "+X face toward the light");

    let mut dark = ortho_at(Vec3::new(0.0, 0.0, 4.0), Vec3::ZERO);
    dark.insert_resource(AmbientLight { color: Color::WHITE, intensity: 0.0 });
    dark.spawn((light, Light::directional(Color::WHITE, 1.0)));
    spawn_mesh(&mut dark, "cube", lit, Transform::IDENTITY);
    let image = render(&gpu, &mut renderer, &mut dark, &assets);
    expect_px(&image, 64, 64, [0, 0, 0, 255], 8, "+Z face is edge-on to the light");

    let mut flat = ortho_at(Vec3::new(0.0, 0.0, 4.0), Vec3::ZERO);
    flat.insert_resource(AmbientLight { color: Color::WHITE, intensity: 0.0 });
    flat.spawn((light, Light::directional(Color::WHITE, 1.0)));
    spawn_mesh(&mut flat, "cube", unlit, Transform::IDENTITY);
    let image = render(&gpu, &mut renderer, &mut flat, &assets);
    expect_px(&image, 64, 64, [255, 255, 255, 255], 2, "unlit ignores the face normal");
}

#[test]
fn generated_meshes_follow_opaque_mask_and_blend_materials() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    assets.insert("box", MeshData::cube(1.2).unwrap()).unwrap();
    let half =
        assets.insert("half", Texture::solid(1, 1, [0, 255, 0, 128]).with_color_space(ColorSpace::Linear)).unwrap();
    let keep = assets
        .insert("keep", {
            let mut material = MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::mask(0.4).unwrap());
            material.albedo = Some(half);
            material
        })
        .unwrap();
    let drop = assets
        .insert("drop", {
            let mut material = MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::mask(0.75).unwrap());
            material.albedo = Some(half);
            material
        })
        .unwrap();
    let glass = assets
        .insert("glass", MaterialAsset::unlit(Color::rgba(1.0, 0.0, 0.0, 0.5)).with_alpha_mode(AlphaMode::Blend))
        .unwrap();
    let blue = assets
        .insert("blue", MaterialAsset::unlit(Color::rgb(0.0, 0.0, 1.0)).with_alpha_mode(AlphaMode::Blend))
        .unwrap();
    let tex = assets.insert("sprite-tex", Texture::solid(1, 1, [255, 255, 255, 255])).unwrap();
    assets.insert("sprite", SpriteAsset::whole_texture(tex, 1, 1).with_pixels_per_unit(1.0)).unwrap();

    let mut masked = ortho_at(Vec3::new(0.0, 0.0, -4.0), Vec3::ZERO);
    spawn_mesh(&mut masked, "box", keep, Transform::IDENTITY);
    let image = render(&gpu, &mut renderer, &mut masked, &assets);
    expect_px(&image, 64, 64, [0, 255, 0, 255], 2, "cutoff 0.4 keeps alpha 0.5");
    assert_eq!(renderer.stats().draw_calls, 1);

    let mut discarded = ortho_at(Vec3::new(0.0, 0.0, -4.0), Vec3::ZERO);
    spawn_mesh(&mut discarded, "box", drop, Transform::IDENTITY);
    let image = render(&gpu, &mut renderer, &mut discarded, &assets);
    expect_px(&image, 64, 64, [0, 0, 0, 255], 2, "cutoff 0.75 discards alpha 0.5");

    let mut blended = ortho_at(Vec3::new(0.0, 0.0, -4.0), Vec3::ZERO);
    blended.spawn((
        Transform::from_xyz(0.0, 0.0, 1.0),
        Sprite::new("sprite").with_size(Vec2::splat(2.0)).with_anchor(Vec2::splat(0.5)).with_material(blue),
    ));
    spawn_mesh(&mut blended, "box", glass, Transform::IDENTITY);
    let image = render(&gpu, &mut renderer, &mut blended, &assets);
    expect_px(&image, 64, 64, [128, 0, 128, 255], 3, "the nearer cuboid blends over the sprite");
    assert_eq!(renderer.stats().draw_calls, 2);

    let mut inline = ortho_at(Vec3::new(0.0, 0.0, -4.0), Vec3::ZERO);
    inline.spawn((Transform::IDENTITY, Mesh::new("box").with_material(Material::unlit(Color::rgb(0.0, 0.0, 1.0)))));
    let image = render(&gpu, &mut renderer, &mut inline, &assets);
    expect_px(&image, 64, 64, [0, 0, 255, 255], 2, "an unassigned cube uses the inline material");

    let mut revised = ortho_at(Vec3::new(0.0, 0.0, -4.0), Vec3::ZERO);
    let paint = assets.insert("paint", MaterialAsset::unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    spawn_mesh(&mut revised, "box", paint, Transform::from_xyz(-0.35, 0.0, 0.0));
    spawn_mesh(&mut revised, "box", paint, Transform::from_xyz(0.35, 0.0, 0.0));
    let _ = render(&gpu, &mut renderer, &mut revised, &assets);
    assert_eq!(renderer.stats().draw_calls, 1);
    assert_eq!(renderer.stats().submitted_instances, 2);
    let textures = renderer.resource_stats().texture_creates;
    assets.insert("paint", MaterialAsset::unlit(Color::rgb(0.0, 1.0, 0.0))).unwrap();
    let image = render(&gpu, &mut renderer, &mut revised, &assets);
    expect_px(&image, 40, 64, [0, 255, 0, 255], 2, "the replacement colour");
    expect_px(&image, 88, 64, [0, 255, 0, 255], 2, "both instances use the new revision");
    assert_eq!(renderer.stats().draw_calls, 1);
    assert_eq!(renderer.resource_stats().texture_creates, textures);
}

#[test]
fn the_same_cube_is_visible_under_three_cameras() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    assets.insert("cube", MeshData::cube(1.5).unwrap()).unwrap();
    assets.insert("ball", MeshData::sphere(0.6, 12, 6).unwrap()).unwrap();
    assets.insert("tube", MeshData::cylinder(0.4, 1.2, 12).unwrap()).unwrap();
    let material = red_material(&mut assets);
    let pitch = (1.0_f32 / 2.0_f32.sqrt()).atan();
    let rotation = Quat::from_rotation_y(core::f32::consts::FRAC_PI_4) * Quat::from_rotation_x(pitch);
    let forward = Transform::from_rotation(rotation).forward();
    let views = [
        (Camera::perspective(50.0), Transform::from_xyz(0.0, 0.0, -4.0)),
        (Camera::orthographic(3.0), Transform::from_xyz(0.0, 0.0, -4.0)),
        (Camera::orthographic(4.0), Transform::from_position(-forward * 6.0).with_rotation(rotation)),
    ];
    for (camera, transform) in views {
        for name in ["cube", "ball", "tube"] {
            let mut world = world(camera, transform);
            world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
            spawn_mesh(&mut world, name, material, Transform::from_rotation(Quat::from_rotation_y(0.5)));
            let image = render(&gpu, &mut renderer, &mut world, &assets);
            expect_px(&image, 64, 64, [255, 0, 0, 255], 2, name);
        }
    }
}

#[test]
fn two_cuboids_share_one_draw_and_a_sphere_does_not() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    assets.insert("box", MeshData::cuboid(Vec3::splat(0.45)).unwrap()).unwrap();
    assets.insert("ball", MeshData::sphere(0.25, 8, 4).unwrap()).unwrap();
    let material = red_material(&mut assets);
    let mut world = world(Camera::orthographic(3.0), Transform::from_xyz(0.0, 0.0, -4.0));
    spawn_mesh(&mut world, "box", material, Transform::from_xyz(-0.6, 0.0, 0.0));
    spawn_mesh(&mut world, "box", material, Transform::from_xyz(0.6, 0.0, 0.0));
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(renderer.stats().draw_calls, 1);
    assert_eq!(renderer.stats().mesh_instances, 2);
    assert_eq!(renderer.stats().submitted_instances, 2);
    expect_px(&image, 40, 64, [255, 0, 0, 255], 2, "left cuboid");
    expect_px(&image, 88, 64, [255, 0, 0, 255], 2, "right cuboid");

    spawn_mesh(&mut world, "ball", material, Transform::from_xyz(0.0, 0.7, 0.0));
    let _ = render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(renderer.stats().draw_calls, 2, "a different mesh asset is a different draw");
    assert_eq!(renderer.stats().submitted_instances, 3);
}
