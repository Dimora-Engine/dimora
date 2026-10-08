//! Renders small scenes offscreen and inspects pixels. Skips (with a message) when the machine has
//! no usable adapter, e.g. CI without a software Vulkan driver.

use engine_assets::{AssetManager, MeshData, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Quat, Transform, Vec2, Vec3};
use engine_renderer::*;
use engine_renderer_wgpu::{GpuContext, SceneRenderer};
use engine_world::{propagate_transforms, World};

const W: u32 = 128;
const H: u32 = 128;

fn setup() -> Option<(GpuContext, SceneRenderer)> {
    match GpuContext::headless() {
        Ok(gpu) => {
            let r = SceneRenderer::new(&gpu, wgpu::TextureFormat::Rgba8UnormSrgb);
            Some((gpu, r))
        }
        Err(e) => {
            eprintln!("skipping GPU test: {e}");
            None
        }
    }
}

fn assets() -> AssetManager {
    let mut a = AssetManager::in_memory();
    let white = a.insert("white", Texture::solid(4, 4, [255; 4]));
    a.insert("quad", SpriteAsset::whole_texture(white, 4, 4).with_pixels_per_unit(1.0));
    a.insert("cube", MeshData::cuboid(Vec3::splat(2.0)));
    a
}

fn world_with_camera() -> World {
    let mut w = World::new();
    w.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    w.spawn((Camera::perspective(60.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -10.0)));
    w
}

fn render(gpu: &GpuContext, r: &mut SceneRenderer, w: &mut World, a: &AssetManager) -> Vec<u8> {
    propagate_transforms(w);
    let scene = extract_scene(w, a, Vec2::new(W as f32, H as f32)).unwrap();
    r.render_to_rgba(gpu, &scene, a, W, H).unwrap()
}

fn px(img: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * W + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

fn is_red(p: [u8; 4]) -> bool {
    p[0] > 200 && p[1] < 40 && p[2] < 40
}
fn is_blue(p: [u8; 4]) -> bool {
    p[2] > 200 && p[0] < 40 && p[1] < 40
}

fn spawn_scene(w: &mut World, cube_z: f32) {
    // Big red cutout sprite at z=10, blue cube in the middle at cube_z.
    w.spawn((
        Transform::from_xyz(0.0, 0.0, 10.0),
        Sprite::new("quad")
            .with_size(Vec2::splat(12.0))
            .with_anchor(Vec2::splat(0.5))
            .with_tint(Color::rgb(1.0, 0.0, 0.0))
            .unlit()
            .cutout(),
    ));
    w.spawn((
        Transform::from_xyz(0.0, 0.0, cube_z),
        Mesh::new("cube").with_material(Material { lit: false, ..Material::color(Color::rgb(0.0, 0.0, 1.0)) }),
    ));
}

#[test]
fn nearer_mesh_occludes_sprite_and_farther_mesh_does_not() {
    let Some((gpu, mut r)) = setup() else { return };
    let a = assets();

    let mut w = world_with_camera();
    spawn_scene(&mut w, 5.0);
    let img = render(&gpu, &mut r, &mut w, &a);
    assert!(is_blue(px(&img, W / 2, H / 2)), "cube in front of the sprite: {:?}", px(&img, W / 2, H / 2));
    assert!(is_red(px(&img, W / 2 + 25, H / 2)), "sprite visible beside the cube: {:?}", px(&img, W / 2 + 25, H / 2));
    assert_eq!(px(&img, 2, 2), [0, 0, 0, 255], "clear color outside everything");

    let mut w = world_with_camera();
    spawn_scene(&mut w, 15.0);
    let img = render(&gpu, &mut r, &mut w, &a);
    assert!(is_red(px(&img, W / 2, H / 2)), "cube behind the sprite is hidden: {:?}", px(&img, W / 2, H / 2));
}

#[test]
fn blended_sprites_interleave_with_meshes_by_depth() {
    let Some((gpu, mut r)) = setup() else { return };
    let a = assets();
    let mut w = world_with_camera();
    // Blended (default) sprite *behind* a cube must not draw over it.
    w.spawn((
        Transform::from_xyz(0.0, 0.0, 10.0),
        Sprite::new("quad")
            .with_size(Vec2::splat(12.0))
            .with_anchor(Vec2::splat(0.5))
            .with_tint(Color::rgb(1.0, 0.0, 0.0))
            .unlit(),
    ));
    w.spawn((
        Transform::from_xyz(0.0, 0.0, 5.0),
        Mesh::new("cube").with_material(Material { lit: false, ..Material::color(Color::rgb(0.0, 0.0, 1.0)) }),
    ));
    let img = render(&gpu, &mut r, &mut w, &a);
    assert!(is_blue(px(&img, W / 2, H / 2)));
    assert!(is_red(px(&img, W / 2 + 25, H / 2)));
}

#[test]
fn lighting_shades_sprites_from_world_space_lights() {
    let Some((gpu, mut r)) = setup() else { return };
    let a = assets();
    let mut brightness = |light: Option<Light>, light_pos: Vec3| {
        let mut w = world_with_camera();
        w.insert_resource(AmbientLight { color: Color::WHITE, intensity: 0.0 });
        w.spawn((
            Transform::from_xyz(0.0, 0.0, 10.0),
            Sprite::new("quad").with_size(Vec2::splat(12.0)).with_anchor(Vec2::splat(0.5)).cutout(),
        ));
        if let Some(l) = light {
            // Aim the light at the sprite (+Z), from `light_pos`.
            w.spawn((Transform::from_position(light_pos), l));
        }
        let p = px(&render(&gpu, &mut r, &mut w, &a), W / 2, H / 2);
        p[0] as u32 + p[1] as u32 + p[2] as u32
    };
    let dark = brightness(None, Vec3::ZERO);
    let sun = brightness(Some(Light::directional(Color::WHITE, 1.0)), Vec3::ZERO);
    let lamp_near = brightness(Some(Light::point(Color::WHITE, 1.0, 20.0)), Vec3::new(0.0, 0.0, 6.0));
    let lamp_far = brightness(Some(Light::point(Color::WHITE, 1.0, 20.0)), Vec3::new(0.0, 0.0, -9.0));
    assert_eq!(dark, 0, "no ambient and no lights is black");
    assert!(sun > 600, "directional light facing the sprite lights it: {sun}");
    assert!(lamp_near > lamp_far, "point light falloff depends on depth distance: {lamp_near} vs {lamp_far}");
    assert!(lamp_far > 0, "inside range but dim: {lamp_far}");
}

#[test]
fn thousands_of_sprites_cost_one_draw_call() {
    let Some((gpu, mut r)) = setup() else { return };
    let a = assets();
    let mut w = world_with_camera();
    for i in 0..5000 {
        let (x, y) = ((i % 100) as f32 * 0.2 - 10.0, (i / 100) as f32 * 0.2 - 5.0);
        w.spawn((Transform::from_xyz(x, y, 12.0), Sprite::new("quad").with_size(Vec2::splat(0.15)).unlit().cutout()));
    }
    // And a rotated 3D sprite, to show sprites are tiltable spatial objects.
    w.spawn((
        Transform::from_xyz(0.0, 0.0, 8.0).with_rotation(Quat::from_rotation_y(1.0)),
        Sprite::new("quad").unlit(),
    ));
    let img = render(&gpu, &mut r, &mut w, &a);
    let stats = r.stats();
    assert_eq!(stats.sprite_instances, 5001);
    assert_eq!(stats.draw_calls, 2, "one cutout batch + one blended batch");
    assert!(img.chunks(4).any(|p| p[0] > 200), "something was drawn");
}

#[test]
fn meshes_cull_back_faces_so_their_interior_is_hidden() {
    let Some((gpu, mut r)) = setup() else { return };
    let mut a = assets();
    a.insert("big_cube", MeshData::cuboid(Vec3::splat(4.0)));
    let mut w = world_with_camera();
    // A red sprite *inside* the cube (between its front face at z=8 and back face at z=12). With
    // correct culling the cube's front face hides it; with inverted culling only the back faces
    // draw and the sprite would show through.
    w.spawn((
        Transform::from_xyz(0.0, 0.0, 10.0),
        Sprite::new("quad")
            .with_size(Vec2::splat(1.0))
            .with_anchor(Vec2::splat(0.5))
            .with_tint(Color::rgb(1.0, 0.0, 0.0))
            .unlit()
            .cutout(),
    ));
    w.spawn((
        Transform::from_xyz(0.0, 0.0, 10.0),
        Mesh::new("big_cube").with_material(Material { lit: false, ..Material::color(Color::rgb(0.0, 0.0, 1.0)) }),
    ));
    let img = render(&gpu, &mut r, &mut w, &a);
    assert!(is_blue(px(&img, W / 2, H / 2)), "{:?}", px(&img, W / 2, H / 2));
}
