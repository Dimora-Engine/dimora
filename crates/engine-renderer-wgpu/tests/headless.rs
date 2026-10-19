//! Renders small scenes offscreen and inspects pixels.
//!
//! When the machine has no usable GPU adapter these tests skip with a message (and still report `ok`),
//! unless `DIMORA_REQUIRE_GPU=1` is set, in which case a missing adapter fails them. `scripts/check.sh`
//! sets the variable. The skip-or-fail decision is [`on_gpu_unavailable`], which is a pure function so it
//! can be tested without removing the GPU.

use engine_assets::{
    AddressMode, AssetHandle, AssetManager, ColorSpace, MeshData, MipFilter, Mipmaps, SamplerDesc, SpriteAsset,
    Texture, TextureDimensionLimits, TextureFilter, TextureRegionError, MAX_TRACKED_REGIONS,
};
use engine_core::Color;
use engine_math::{Quat, Rect, Transform, Vec2, Vec3};
use engine_renderer::*;
use engine_renderer_wgpu::{GpuContext, RenderError, SceneRenderer};
use engine_world::{propagate_transforms, Visibility, World};

mod capture {
    include!("../../log_capture.rs");
}

const W: u32 = 128;
const H: u32 = 128;

/// Name of the environment variable that turns "no GPU" from a skip into a failure.
const REQUIRE_GPU_VAR: &str = "DIMORA_REQUIRE_GPU";

/// What a GPU test does when it cannot get a GPU.
#[derive(Debug, PartialEq, Eq)]
enum GpuUnavailable {
    /// Print the message and return. The test still reports `ok` without having rendered anything.
    Skip(String),
    /// Panic with the message.
    Fail(String),
}

/// Decides between skipping and failing when `GpuContext::headless()` returned `error`.
///
/// `require_gpu` is the value of `DIMORA_REQUIRE_GPU` (`None` when unset); only the exact value `1` makes
/// the GPU mandatory. `os` is `std::env::consts::OS` and only selects the hint in the failure message.
fn on_gpu_unavailable(error: &RenderError, require_gpu: Option<&str>, os: &str) -> GpuUnavailable {
    if require_gpu != Some("1") {
        return GpuUnavailable::Skip(format!("skipping GPU test: {error}"));
    }
    let what = match error {
        RenderError::NoAdapter(_) => "no GPU adapter was found",
        _ => "a GPU adapter was found but it could not be used",
    };
    let hint = match os {
        "linux" => {
            "Install a Vulkan driver, for example the lavapipe software driver: \
             `sudo apt install mesa-vulkan-drivers`."
        }
        "macos" => {
            "wgpu uses Metal on macOS, so this usually means the process has no GPU access \
             (a virtual machine or a restricted sandbox) or WGPU_BACKEND excludes Metal."
        }
        _ => "Install or enable a GPU driver that wgpu supports (Vulkan, Metal or DX12).",
    };
    GpuUnavailable::Fail(format!("{REQUIRE_GPU_VAR}=1 but {what}: {error}. {hint} Unset {REQUIRE_GPU_VAR} to skip."))
}

fn setup() -> Option<(GpuContext, SceneRenderer)> {
    match GpuContext::headless() {
        Ok(gpu) => {
            let info = gpu.adapter.get_info();
            let anisotropic =
                gpu.adapter.get_downlevel_capabilities().flags.contains(wgpu::DownlevelFlags::ANISOTROPIC_FILTERING);
            eprintln!(
                "GPU test adapter: {} ({:?}, {:?}); DownlevelFlags::ANISOTROPIC_FILTERING={anisotropic}",
                info.name, info.backend, info.device_type
            );
            let r = SceneRenderer::new(&gpu, wgpu::TextureFormat::Rgba8UnormSrgb);
            Some((gpu, r))
        }
        Err(e) => {
            let require = std::env::var(REQUIRE_GPU_VAR).ok();
            match on_gpu_unavailable(&e, require.as_deref(), std::env::consts::OS) {
                GpuUnavailable::Skip(message) => {
                    eprintln!("{message}");
                    None
                }
                GpuUnavailable::Fail(message) => panic!("{message}"),
            }
        }
    }
}

#[cfg(test)]
mod gpu_requirement {
    use super::*;

    fn no_adapter() -> RenderError {
        RenderError::NoAdapter("no backend produced an adapter".into())
    }

    #[test]
    fn unset_skips_with_the_existing_message() {
        let out = on_gpu_unavailable(&no_adapter(), None, "linux");
        let GpuUnavailable::Skip(message) = out else { panic!("expected Skip, got {out:?}") };
        assert!(message.starts_with("skipping GPU test"), "{message}");
    }

    #[test]
    fn only_the_exact_value_one_requires_a_gpu() {
        for value in ["0", "", "true", "yes", "2", " 1"] {
            let out = on_gpu_unavailable(&no_adapter(), Some(value), "linux");
            assert!(matches!(out, GpuUnavailable::Skip(_)), "value {value:?} gave {out:?}");
        }
    }

    #[test]
    fn required_and_missing_fails_and_says_no_adapter_was_found() {
        for os in ["linux", "macos", "windows"] {
            let out = on_gpu_unavailable(&no_adapter(), Some("1"), os);
            let GpuUnavailable::Fail(message) = out else { panic!("expected Fail on {os}, got {out:?}") };
            assert!(message.contains("no GPU adapter was found"), "{os}: {message}");
            assert!(message.contains("DIMORA_REQUIRE_GPU=1"), "{os}: {message}");
        }
    }

    #[test]
    fn linux_failure_suggests_a_vulkan_driver() {
        let GpuUnavailable::Fail(message) = on_gpu_unavailable(&no_adapter(), Some("1"), "linux") else {
            panic!("expected Fail")
        };
        assert!(message.contains("mesa-vulkan-drivers"), "{message}");
        let GpuUnavailable::Fail(mac) = on_gpu_unavailable(&no_adapter(), Some("1"), "macos") else {
            panic!("expected Fail")
        };
        assert!(!mac.contains("mesa-vulkan-drivers"), "{mac}");
    }

    #[test]
    fn required_with_an_unusable_adapter_also_fails() {
        let error = RenderError::Device("out of memory".into());
        let out = on_gpu_unavailable(&error, Some("1"), "linux");
        let GpuUnavailable::Fail(message) = out else { panic!("expected Fail, got {out:?}") };
        assert!(message.contains("could not be used") && message.contains("out of memory"), "{message}");
    }
}

fn assets() -> AssetManager {
    let mut a = AssetManager::in_memory();
    let white = a.insert("white", Texture::solid(4, 4, [255; 4])).unwrap();
    a.insert("quad", SpriteAsset::whole_texture(white, 4, 4).with_pixels_per_unit(1.0)).unwrap();
    a.insert("cube", MeshData::cuboid(Vec3::splat(2.0))).unwrap();
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
fn a_hidden_parent_keeps_its_sprite_off_the_framebuffer() {
    let Some((gpu, mut renderer)) = setup() else { return };
    let assets = assets();
    let mut world = world_with_camera();
    let parent = world.spawn((Transform::IDENTITY, Visibility::Hidden));
    world.spawn_child(
        parent,
        (
            Transform::from_xyz(0.0, 0.0, 10.0),
            Sprite::new("quad")
                .with_size(Vec2::splat(12.0))
                .with_anchor(Vec2::splat(0.5))
                .with_tint(Color::rgb(1.0, 0.0, 0.0))
                .unlit()
                .cutout(),
        ),
    );
    let hidden = render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(px(&hidden, W / 2, H / 2), [0, 0, 0, 255], "hidden subtree is the clear color");

    world.remove::<Visibility>(parent).unwrap();
    let shown = render(&gpu, &mut renderer, &mut world, &assets);
    assert!(is_red(px(&shown, W / 2, H / 2)), "removing Hidden draws the child: {:?}", px(&shown, W / 2, H / 2));
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
    a.insert("big_cube", MeshData::cuboid(Vec3::splat(4.0))).unwrap();
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

fn red_sprite(size: f32) -> Sprite {
    Sprite::new("quad")
        .with_size(Vec2::splat(size))
        .with_anchor(Vec2::splat(0.5))
        .with_tint(Color::rgb(1.0, 0.0, 0.0))
        .unlit()
        .cutout()
}

#[test]
fn camera_viewport_confines_the_image_to_its_rectangle() {
    let Some((gpu, mut r)) = setup() else { return };
    let a = assets();
    let mut w = World::new();
    w.spawn((
        Camera::perspective(60.0).with_clear_color(Color::BLACK).with_viewport(ViewportRect::new(0.0, 0.0, 0.5, 1.0)),
        Transform::from_xyz(0.0, 0.0, -10.0),
    ));
    w.spawn((Transform::IDENTITY, red_sprite(3.0)));
    let img = render(&gpu, &mut r, &mut w, &a);
    // The world origin is the centre of the camera's rectangle (the left half), not of the target.
    assert!(is_red(px(&img, W / 4, H / 2)), "{:?}", px(&img, W / 4, H / 2));
    assert!(!is_red(px(&img, 3 * W / 4, H / 2)), "nothing is drawn outside the viewport");
    assert!(!is_red(px(&img, W / 2 + 1, H / 2)));
}

#[test]
fn billboards_face_the_camera_but_fixed_quads_go_edge_on() {
    let Some((gpu, mut r)) = setup() else { return };
    let a = assets();
    let render_with = |gpu: &GpuContext, r: &mut SceneRenderer, sprite: Sprite| {
        let mut w = World::new();
        // Camera on the -X side, looking along +X: a quad in the world XY plane is seen edge-on.
        w.spawn((
            Camera::perspective(60.0).with_clear_color(Color::BLACK),
            Transform::from_xyz(-10.0, 0.0, 0.0).with_rotation(Quat::from_rotation_y(core::f32::consts::FRAC_PI_2)),
        ));
        w.spawn((Transform::IDENTITY, sprite));
        render(gpu, r, &mut w, &a)
    };
    let fixed = render_with(&gpu, &mut r, red_sprite(4.0).fixed());
    let billboard = render_with(&gpu, &mut r, red_sprite(4.0));
    assert!(!is_red(px(&fixed, W / 2, H / 2)), "a fixed quad seen edge-on has no area");
    assert!(is_red(px(&billboard, W / 2, H / 2)), "a billboard always faces the view plane");
}

// ---- GPU cache lifetime (D-04) ------------------------------------------------------------------

use engine_renderer_wgpu::CacheStats;

const EMPTY_CACHE: CacheStats = CacheStats { textures: 0, meshes: 0 };

/// Builds the assets used by the cache tests: a white texture behind the `quad` sprite asset, a
/// blue texture for the cube's material, and the `cube` mesh. Two textures and one mesh.
fn insert_cache_test_assets(a: &mut AssetManager) {
    let white = a.insert("white", Texture::solid(4, 4, [255; 4])).unwrap();
    a.insert("quad", SpriteAsset::whole_texture(white, 4, 4).with_pixels_per_unit(1.0)).unwrap();
    a.insert("blue", Texture::solid(4, 4, [0, 0, 255, 255])).unwrap();
    a.insert("cube", MeshData::cuboid(Vec3::splat(2.0))).unwrap();
}

fn remove_cache_test_assets(a: &mut AssetManager) {
    assert!(a.remove(a.texture("white")));
    assert!(a.remove(a.sprite("quad")));
    assert!(a.remove(a.texture("blue")));
    assert!(a.remove(a.mesh("cube")));
}

/// A red sprite beside a cube textured blue; the sprite covers pixel (W/2 + 25, H/2), the cube covers the centre.
fn spawn_cache_test_scene(w: &mut World) {
    w.spawn((
        Transform::from_xyz(3.6, 0.0, 6.0),
        Sprite::new("quad")
            .with_size(Vec2::splat(4.0))
            .with_anchor(Vec2::splat(0.5))
            .with_tint(Color::rgb(1.0, 0.0, 0.0))
            .unlit()
            .cutout(),
    ));
    w.spawn((
        Transform::from_xyz(0.0, 0.0, 10.0),
        Mesh::new("cube").with_material(Material {
            lit: false,
            base_texture: Some(AssetHandle::from_name("blue")),
            ..Material::color(Color::WHITE)
        }),
    ));
}

fn cache_test_world() -> World {
    let mut w = world_with_camera();
    spawn_cache_test_scene(&mut w);
    w
}

#[test]
fn removing_assets_returns_the_gpu_caches_to_the_empty_baseline() {
    let Some((gpu, mut r)) = setup() else { return };
    let mut a = AssetManager::in_memory();
    insert_cache_test_assets(&mut a);
    assert_eq!(r.cache_stats(), EMPTY_CACHE, "a fresh renderer caches nothing");

    let mut w = cache_test_world();
    let img = render(&gpu, &mut r, &mut w, &a);
    let (sprite_px, cube_px) = (px(&img, W / 2 + 25, H / 2), px(&img, W / 2, H / 2));
    assert!(is_red(sprite_px), "red sprite beside the cube: {sprite_px:?}");
    assert!(is_blue(cube_px), "cube textured with the blue texture: {cube_px:?}");
    assert_eq!(r.cache_stats(), CacheStats { textures: 2, meshes: 1 });

    remove_cache_test_assets(&mut a);
    let img = render(&gpu, &mut r, &mut w, &a);
    assert_eq!(r.cache_stats(), EMPTY_CACHE, "every cached copy of a removed asset is evicted");
    assert_eq!(px(&img, W / 2, H / 2), [0, 0, 0, 255], "nothing is drawn from a removed asset");

    insert_cache_test_assets(&mut a);
    let img = render(&gpu, &mut r, &mut w, &a);
    assert_eq!(r.cache_stats(), CacheStats { textures: 2, meshes: 1 }, "re-inserted assets are uploaded again");
    assert_eq!((px(&img, W / 2 + 25, H / 2), px(&img, W / 2, H / 2)), (sprite_px, cube_px), "same picture as before");
}

#[test]
fn unloading_a_scene_and_removing_its_assets_returns_the_gpu_cache_to_baseline() {
    let Some((gpu, mut r)) = setup() else { return };
    let mut assets = AssetManager::in_memory();
    let mut world = world_with_camera();
    let baseline_entities = world.entities.len();
    render(&gpu, &mut r, &mut world, &assets);
    let baseline_cache = r.cache_stats();
    assert_eq!(baseline_cache, EMPTY_CACHE, "a camera with no renderables caches nothing");

    insert_cache_test_assets(&mut assets);
    let instance = world
        .load_scene(|builder| {
            builder.spawn((
                Transform::from_xyz(3.6, 0.0, 6.0),
                Sprite::new("quad")
                    .with_size(Vec2::splat(4.0))
                    .with_anchor(Vec2::splat(0.5))
                    .with_tint(Color::rgb(1.0, 0.0, 0.0))
                    .unlit()
                    .cutout(),
            ));
            builder.spawn((
                Transform::from_xyz(0.0, 0.0, 10.0),
                Mesh::new("cube").with_material(Material {
                    lit: false,
                    base_texture: Some(AssetHandle::from_name("blue")),
                    ..Material::color(Color::WHITE)
                }),
            ));
            Ok(())
        })
        .unwrap();
    let img = render(&gpu, &mut r, &mut world, &assets);
    assert!(is_red(px(&img, W / 2 + 25, H / 2)), "scene sprite is drawn");
    assert!(is_blue(px(&img, W / 2, H / 2)), "scene mesh is drawn");
    assert_eq!(r.cache_stats(), CacheStats { textures: 2, meshes: 1 });
    assert!(world.entities.len() > baseline_entities);

    world.unload_scene(instance).unwrap();
    assert_eq!(world.entities.len(), baseline_entities, "only the camera entity remains");
    assert!(world.scene_instances().unwrap().is_empty());
    remove_cache_test_assets(&mut assets);
    let img = render(&gpu, &mut r, &mut world, &assets);
    assert_eq!(r.cache_stats(), baseline_cache, "test-owned gpu copies are gone after remove and another frame");
    assert_eq!(world.entities.len(), baseline_entities);
    assert_eq!(px(&img, W / 2, H / 2), [0, 0, 0, 255], "the unloaded scene no longer draws");
}

#[test]
fn a_removed_texture_is_drawn_as_the_placeholder_never_from_a_stale_copy() {
    let Some((gpu, mut r)) = setup() else { return };
    let mut a = assets();
    let mut w = world_with_camera();
    w.spawn((
        Transform::from_xyz(0.0, 0.0, 10.0),
        Sprite::new("quad").with_size(Vec2::splat(12.0)).with_anchor(Vec2::splat(0.5)).unlit().cutout(),
    ));
    let before = px(&render(&gpu, &mut r, &mut w, &a), W / 2, H / 2);
    assert_eq!(before, [255, 255, 255, 255], "white texture");
    assert_eq!(r.cache_stats().textures, 1);

    assert!(a.remove(a.texture("white")));
    let after = px(&render(&gpu, &mut r, &mut w, &a), W / 2, H / 2);
    assert_eq!(after, [255, 0, 255, 255], "the sprite asset remains, its texture is gone: magenta placeholder");
    assert_eq!(r.cache_stats().textures, 0);
}

#[test]
fn a_replaced_asset_is_reuploaded_without_keeping_the_old_copy() {
    let Some((gpu, mut r)) = setup() else { return };
    let mut a = assets();
    let mut w = world_with_camera();
    w.spawn((
        Transform::from_xyz(0.0, 0.0, 10.0),
        Sprite::new("quad").with_size(Vec2::splat(12.0)).with_anchor(Vec2::splat(0.5)).unlit().cutout(),
    ));
    assert_eq!(px(&render(&gpu, &mut r, &mut w, &a), W / 2, H / 2), [255, 255, 255, 255]);
    assert_eq!(r.cache_stats().textures, 1);

    // Replace in place: the revision changes, the single cache entry is rewritten.
    for (i, colour) in [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]].into_iter().enumerate() {
        a.insert("white", Texture::solid(4, 4, colour)).unwrap();
        let p = px(&render(&gpu, &mut r, &mut w, &a), W / 2, H / 2);
        assert_eq!(p, colour, "replacement {i} is what is drawn");
        assert_eq!(r.cache_stats().textures, 1, "replacement {i} did not add a second copy");
    }
}

#[test]
fn remove_and_reinsert_between_frames_is_seen_as_a_change() {
    let Some((gpu, mut r)) = setup() else { return };
    let mut a = assets();
    let mut w = world_with_camera();
    w.spawn((
        Transform::from_xyz(0.0, 0.0, 10.0),
        Sprite::new("quad").with_size(Vec2::splat(12.0)).with_anchor(Vec2::splat(0.5)).unlit().cutout(),
    ));
    // The cache now holds the texture's very first revision. A manager that restarted revisions after a
    // removal would hand the replacement the same one, and the renderer would keep drawing the old pixels.
    assert_eq!(px(&render(&gpu, &mut r, &mut w, &a), W / 2, H / 2), [255, 255, 255, 255]);

    a.remove(a.texture("white"));
    a.insert("white", Texture::solid(4, 4, [255, 255, 0, 255])).unwrap();
    let p = px(&render(&gpu, &mut r, &mut w, &a), W / 2, H / 2);
    assert_eq!(p, [255, 255, 0, 255], "the replacement is drawn, not the stale first upload");
    assert_eq!(r.cache_stats().textures, 1);
}

#[test]
fn assets_unused_for_the_idle_limit_are_evicted_and_come_back_when_needed() {
    let Some((gpu, mut r)) = setup() else { return };
    let mut a = AssetManager::in_memory();
    insert_cache_test_assets(&mut a);
    r.set_cache_max_idle_frames(3);
    assert_eq!(r.cache_max_idle_frames(), 3);

    let mut with_scene = cache_test_world();
    let mut empty = world_with_camera();
    let drawn = render(&gpu, &mut r, &mut with_scene, &a);
    let expected = (px(&drawn, W / 2 + 25, H / 2), px(&drawn, W / 2, H / 2));
    let full = CacheStats { textures: 2, meshes: 1 };
    assert_eq!(r.cache_stats(), full);

    // Used again within the limit: never evicted, however long this goes on.
    for _ in 0..10 {
        render(&gpu, &mut r, &mut with_scene, &a);
        assert_eq!(r.cache_stats(), full);
    }

    // Unused for 1 and 2 frames: still cached. The third unused frame reaches the limit.
    for idle in 1..=2 {
        render(&gpu, &mut r, &mut empty, &a);
        assert_eq!(r.cache_stats(), full, "idle for {idle} frame(s), limit is 3");
    }
    render(&gpu, &mut r, &mut empty, &a);
    assert_eq!(r.cache_stats(), EMPTY_CACHE, "idle for 3 frames: evicted");
    assert!(a.get(a.mesh("cube")).is_some(), "eviction only drops the GPU copy; the asset itself stays");

    let img = render(&gpu, &mut r, &mut with_scene, &a);
    assert_eq!(r.cache_stats(), full, "needed again: uploaded again");
    assert_eq!((px(&img, W / 2 + 25, H / 2), px(&img, W / 2, H / 2)), expected, "and it draws the same picture");
}

#[test]
fn idle_limit_is_clamped_and_can_be_disabled() {
    let Some((gpu, mut r)) = setup() else { return };
    assert_eq!(r.cache_max_idle_frames(), engine_renderer_wgpu::DEFAULT_CACHE_MAX_IDLE_FRAMES);
    r.set_cache_max_idle_frames(0);
    assert_eq!(r.cache_max_idle_frames(), 1, "0 would evict assets in the frame that draws them");

    let mut a = AssetManager::in_memory();
    insert_cache_test_assets(&mut a);
    let mut with_scene = cache_test_world();
    let mut empty = world_with_camera();
    render(&gpu, &mut r, &mut with_scene, &a);
    assert_eq!(r.cache_stats(), CacheStats { textures: 2, meshes: 1 }, "a limit of 1 keeps what the frame uses");
    render(&gpu, &mut r, &mut empty, &a);
    assert_eq!(r.cache_stats(), EMPTY_CACHE, "one unused frame reaches a limit of 1");

    r.set_cache_max_idle_frames(u32::MAX);
    render(&gpu, &mut r, &mut with_scene, &a);
    for _ in 0..50 {
        render(&gpu, &mut r, &mut empty, &a);
    }
    assert_eq!(r.cache_stats(), CacheStats { textures: 2, meshes: 1 }, "u32::MAX disables idle eviction");
    remove_cache_test_assets(&mut a);
    render(&gpu, &mut r, &mut empty, &a);
    assert_eq!(r.cache_stats(), EMPTY_CACHE, "removal still evicts when idle eviction is off");
}

fn sampling_world(size: Vec2, region: Option<Rect>, at: Transform) -> World {
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.spawn((Camera::orthographic(2.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -4.0)));
    let mut sprite = Sprite::new("sprite").with_size(size).with_anchor(Vec2::splat(0.5)).unlit().cutout();
    if let Some(region) = region {
        sprite = sprite.with_region(region);
    }
    world.spawn((at, sprite));
    world
}

fn insert_sampling_texture(assets: &mut AssetManager, texture: Texture) -> AssetHandle<Texture> {
    let (width, height) = (texture.width, texture.height);
    let handle = assets.insert("tex", texture).unwrap();
    assets.insert("sprite", SpriteAsset::whole_texture(handle, width, height).with_pixels_per_unit(1.0)).unwrap();
    handle
}

fn full_sprite() -> (Vec2, Transform) {
    (Vec2::splat(2.0), Transform::IDENTITY)
}

fn assert_pixel(img: &[u8], x: u32, y: u32, expected: [u8; 4], tolerance: i16) {
    let pixel = px(img, x, y);
    assert_rgba(pixel, expected, tolerance, &format!("pixel ({x},{y})"));
}

fn assert_rgba(pixel: [u8; 4], expected: [u8; 4], tolerance: i16, what: &str) {
    let close = pixel.iter().zip(expected).all(|(got, want)| (*got as i16 - want as i16).abs() <= tolerance);
    assert!(close, "{what} is {pixel:?}, expected {expected:?} ±{tolerance}");
}

fn mixed_red_blue(pixel: [u8; 4]) -> bool {
    pixel[0] > 60 && pixel[2] > 60 && pixel[1] < 20 && pixel[3] > 250
}

/// `uv` across a full-screen sprite whose asset region is `(0, 0)-(2, 1)`.
fn pixel_for_u(uv: f32) -> u32 {
    // uv = 2 * (x + 0.5) / W, so x = uv * W / 2 - 0.5.
    (uv * W as f32 / 2.0 - 0.5).round() as u32
}

fn linear_aniso(anisotropy: u8) -> SamplerDesc {
    SamplerDesc {
        min_filter: TextureFilter::Linear,
        mag_filter: TextureFilter::Linear,
        mip_filter: Some(MipFilter::Linear),
        anisotropy,
        ..SamplerDesc::default()
    }
}

#[test]
fn srgb_and_linear_textures_select_different_gpu_formats() {
    let Some((gpu, mut srgb_renderer)) = setup() else { return };
    let mut linear_renderer = SceneRenderer::new(&gpu, wgpu::TextureFormat::Rgba8Unorm);
    let byte = 180u8;
    let decoded = (Color::from_srgb8(byte, byte, byte).r * 255.0).round() as u8;
    assert!(decoded + 20 < byte, "the fixture must make an sRGB decode visible, decoded {decoded}");

    let mut linear_assets = AssetManager::in_memory();
    let mut srgb_assets = AssetManager::in_memory();
    let sample = |renderer: &mut SceneRenderer, assets: &mut AssetManager, texture: Texture| {
        let handle = insert_sampling_texture(assets, texture);
        let (size, at) = full_sprite();
        let mut world = sampling_world(size, None, at);
        let image = render(&gpu, renderer, &mut world, assets);
        let info = renderer.cached_texture(handle.id()).expect("texture cached");
        (px(&image, W / 2, H / 2), info)
    };

    let linear_tex = Texture::solid(1, 1, [byte, byte, byte, 255]).with_color_space(ColorSpace::Linear);
    let (linear_pixel, linear_info) = sample(&mut linear_renderer, &mut linear_assets, linear_tex);
    assert_eq!(linear_info.color_space, ColorSpace::Linear);
    assert_rgba(linear_pixel, [byte, byte, byte, 255], 1, "linear sample");

    let srgb_tex = Texture::solid(1, 1, [byte, byte, byte, 255]).with_color_space(ColorSpace::Srgb);
    let (srgb_pixel, srgb_info) = sample(&mut linear_renderer, &mut linear_assets, srgb_tex);
    assert_eq!(srgb_info.color_space, ColorSpace::Srgb);
    assert_rgba(srgb_pixel, [decoded, decoded, decoded, 255], 2, "sRGB sample on a linear target");

    let default_tex = Texture::solid(1, 1, [byte, byte, byte, 255]);
    let (default_pixel, default_info) = sample(&mut linear_renderer, &mut linear_assets, default_tex);
    assert_eq!(default_info.color_space, ColorSpace::Srgb);
    assert_rgba(default_pixel, [decoded, decoded, decoded, 255], 2, "default colour space");

    let (round_trip, round_info) =
        sample(&mut srgb_renderer, &mut srgb_assets, Texture::solid(1, 1, [byte, byte, byte, 255]));
    assert_eq!(round_info.color_space, ColorSpace::Srgb);
    assert_rgba(round_trip, [byte, byte, byte, 255], 2, "sRGB round trip");
}

#[test]
fn filtering_and_addressing_follow_the_sampler_descriptor() {
    let Some((gpu, mut renderer)) = setup() else { return };
    let red_blue = Texture::from_rgba8(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255]);
    let (size, at) = full_sprite();
    let mut assets = AssetManager::in_memory();

    let draw = |renderer: &mut SceneRenderer, assets: &mut AssetManager, texture: Texture, region: Option<Rect>| {
        let handle = insert_sampling_texture(assets, texture);
        let mut world = sampling_world(size, region, at);
        let image = render(&gpu, renderer, &mut world, assets);
        (image, renderer.cached_texture(handle.id()).expect("cached"))
    };

    let (nearest_img, nearest_info) =
        draw(&mut renderer, &mut assets, red_blue.clone().with_filter(TextureFilter::Nearest), None);
    assert_eq!(nearest_info.min_filter, TextureFilter::Nearest);
    assert_eq!(nearest_info.mag_filter, TextureFilter::Nearest);
    assert!(is_red(px(&nearest_img, 32, 64)), "nearest texel centre {:?}", px(&nearest_img, 32, 64));
    assert!(is_blue(px(&nearest_img, 96, 64)), "nearest texel centre {:?}", px(&nearest_img, 96, 64));
    assert!(is_red(px(&nearest_img, 63, 64)), "nearest keeps the texel edge {:?}", px(&nearest_img, 63, 64));

    let (linear_img, linear_info) =
        draw(&mut renderer, &mut assets, red_blue.clone().with_filter(TextureFilter::Linear), None);
    assert_eq!((linear_info.min_filter, linear_info.mag_filter), (TextureFilter::Linear, TextureFilter::Linear));
    assert!(is_red(px(&linear_img, 32, 64)) && is_blue(px(&linear_img, 96, 64)));
    assert!(mixed_red_blue(px(&linear_img, 63, 64)), "linear blends the edge {:?}", px(&linear_img, 63, 64));

    let split = red_blue
        .clone()
        .with_sampler(SamplerDesc {
            min_filter: TextureFilter::Nearest,
            mag_filter: TextureFilter::Linear,
            ..SamplerDesc::default()
        })
        .unwrap();
    let (split_img, split_info) = draw(&mut renderer, &mut assets, split, None);
    assert_eq!(split_info.min_filter, TextureFilter::Nearest);
    assert_eq!(split_info.mag_filter, TextureFilter::Linear);
    assert!(mixed_red_blue(px(&split_img, 63, 64)), "magnification uses the mag filter {:?}", px(&split_img, 63, 64));

    // The full-screen sprite magnifies two texels across the view, so this edge uses mag nearest.
    let min_only = red_blue
        .with_sampler(SamplerDesc {
            min_filter: TextureFilter::Linear,
            mag_filter: TextureFilter::Nearest,
            ..SamplerDesc::default()
        })
        .unwrap();
    let (min_only_img, min_only_info) = draw(&mut renderer, &mut assets, min_only, None);
    assert_eq!(min_only_info.min_filter, TextureFilter::Linear);
    assert_eq!(min_only_info.mag_filter, TextureFilter::Nearest);
    assert_eq!((min_only_info.lod_min, min_only_info.lod_max), (0.0, 32.0));
    assert_eq!(min_only_info.mip_level_count, 1);
    assert!(
        is_red(px(&min_only_img, 63, 64)),
        "magnified sampling uses mag nearest, not the linear min filter {:?}",
        px(&min_only_img, 63, 64)
    );

    // Eight screen pixels across 32 texels of a one-level texture. The centre sample sits on the
    // red/blue edge and the footprint is several texels wide, so the GPU selects minification.
    let minified = Texture::from_fn(32, 1, |x, _| if x < 16 { [255, 0, 0, 255] } else { [0, 0, 255, 255] });
    let center = -1.0 + (64.5 / W as f32) * 2.0;
    let sprite = Vec2::new(8.0 * 2.0 / W as f32, 2.0);
    let at_pixel = Transform::from_xyz(center, 0.0, 0.0);
    let min_linear = minified
        .clone()
        .with_sampler(SamplerDesc {
            min_filter: TextureFilter::Linear,
            mag_filter: TextureFilter::Nearest,
            ..SamplerDesc::default()
        })
        .unwrap();
    let handle = insert_sampling_texture(&mut assets, min_linear);
    let mut world = sampling_world(sprite, None, at_pixel);
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    let info = renderer.cached_texture(handle.id()).expect("cached");
    assert_eq!(info.mip_level_count, 1);
    assert_eq!((info.min_filter, info.mag_filter), (TextureFilter::Linear, TextureFilter::Nearest));
    assert!(
        mixed_red_blue(px(&image, 64, 64)),
        "one-level minification uses the linear min filter {:?}",
        px(&image, 64, 64)
    );
    let min_nearest = minified
        .with_sampler(SamplerDesc {
            min_filter: TextureFilter::Nearest,
            mag_filter: TextureFilter::Linear,
            ..SamplerDesc::default()
        })
        .unwrap();
    insert_sampling_texture(&mut assets, min_nearest);
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    let pixel = px(&image, 64, 64);
    assert!(
        is_red(pixel) || is_blue(pixel),
        "one-level minification keeps nearest min crisp at the texel edge {pixel:?}"
    );

    let wide = Rect::new(Vec2::ZERO, Vec2::new(2.0, 1.0));
    let repeat_uv = pixel_for_u(1.25);
    let mirror_uv = pixel_for_u(1.75);
    let modes = [
        (AddressMode::Repeat, AddressMode::Clamp, repeat_uv, true),
        (AddressMode::Clamp, AddressMode::Clamp, repeat_uv, false),
        (AddressMode::Mirror, AddressMode::Clamp, mirror_uv, true),
    ];
    for (address_u, address_v, x, expect_red) in modes {
        let texture = Texture::from_rgba8(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255])
            .with_sampler(SamplerDesc { address_u, address_v, ..SamplerDesc::default() })
            .unwrap();
        let (image, info) = draw(&mut renderer, &mut assets, texture, Some(wide));
        assert_eq!((info.address_u, info.address_v), (address_u, address_v));
        let pixel = px(&image, x, 64);
        assert_eq!(is_red(pixel), expect_red, "{address_u:?}/{address_v:?} at x={x} is {pixel:?}, red={expect_red}");
    }

    let grid = Texture::from_rgba8(2, 2, vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255]);
    let region = Rect::new(Vec2::ZERO, Vec2::splat(2.0));
    let axes = [
        (AddressMode::Repeat, AddressMode::Repeat, [255, 0, 0, 255]),
        (AddressMode::Clamp, AddressMode::Clamp, [255, 255, 0, 255]),
        (AddressMode::Repeat, AddressMode::Clamp, [0, 0, 255, 255]),
        (AddressMode::Clamp, AddressMode::Repeat, [0, 255, 0, 255]),
    ];
    for (address_u, address_v, expected) in axes {
        let texture = grid
            .clone()
            .with_sampler(SamplerDesc {
                address_u,
                address_v,
                min_filter: TextureFilter::Nearest,
                mag_filter: TextureFilter::Nearest,
                ..SamplerDesc::default()
            })
            .unwrap();
        let (image, info) = draw(&mut renderer, &mut assets, texture, Some(region));
        assert_eq!((info.address_u, info.address_v), (address_u, address_v));
        assert_pixel(&image, pixel_for_u(1.25), pixel_for_u(1.25), expected, 2);
    }
}

#[test]
fn sampler_cache_reuses_descriptors_without_copying_pixels() {
    let Some((gpu, mut renderer)) = setup() else { return };
    let before = renderer.resource_stats();
    assert_eq!(before.sampler_creates, 1, "placeholders share the default sampler");
    assert_eq!(renderer.sampler_cache_len(), 1);

    let mut assets = AssetManager::in_memory();
    let linear =
        SamplerDesc { min_filter: TextureFilter::Linear, mag_filter: TextureFilter::Linear, ..SamplerDesc::default() };
    let first = assets.insert("a", Texture::solid(1, 1, [10, 20, 30, 255]).with_sampler(linear).unwrap()).unwrap();
    let second = assets.insert("b", Texture::solid(1, 1, [40, 50, 60, 255]).with_sampler(linear).unwrap()).unwrap();
    assets.insert("sprite-a", SpriteAsset::whole_texture(first, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    assets.insert("sprite-b", SpriteAsset::whole_texture(second, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.spawn((Camera::orthographic(2.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -4.0)));
    for name in ["sprite-a", "sprite-a", "sprite-b"] {
        world.spawn((
            Transform::IDENTITY,
            Sprite::new(name).with_size(Vec2::splat(1.0)).with_anchor(Vec2::splat(0.5)).unlit().cutout(),
        ));
    }
    render(&gpu, &mut renderer, &mut world, &assets);
    let uploaded = renderer.resource_stats();
    assert_eq!(uploaded.texture_creates, before.texture_creates + 2, "two images, three entities");
    assert_eq!(uploaded.texture_writes, before.texture_writes + 2);
    assert_eq!(uploaded.sampler_creates, before.sampler_creates + 1, "one sampler for both images");
    assert_eq!(renderer.sampler_cache_len(), 2);
    let fingerprint = renderer.cached_texture(first.id()).unwrap().pixel_fingerprint;
    assert_eq!(renderer.cached_texture(second.id()).unwrap().min_filter, TextureFilter::Linear);

    let again = renderer.resource_stats();
    render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(renderer.resource_stats(), again, "an unchanged revision does not hash or upload");

    assets
        .insert(
            "a",
            Texture::solid(1, 1, [10, 20, 30, 255])
                .with_sampler(SamplerDesc { address_u: AddressMode::Repeat, ..linear })
                .unwrap(),
        )
        .unwrap();
    render(&gpu, &mut renderer, &mut world, &assets);
    let changed = renderer.resource_stats();
    assert_eq!(changed.texture_creates, uploaded.texture_creates);
    assert_eq!(changed.texture_writes, uploaded.texture_writes);
    assert!(changed.bind_group_creates > uploaded.bind_group_creates);
    assert!(changed.sampler_creates > uploaded.sampler_creates);
    assert_eq!(changed.pixel_fingerprints, uploaded.pixel_fingerprints + 1);
    assert_eq!(renderer.cached_texture(first.id()).unwrap().pixel_fingerprint, fingerprint);
    assert_eq!(renderer.cached_texture(first.id()).unwrap().address_u, AddressMode::Repeat);
    assert_ne!(
        renderer.cached_texture(first.id()).unwrap().address_u,
        renderer.cached_texture(second.id()).unwrap().address_u
    );
}

#[test]
fn anisotropy_follows_device_support_and_keeps_nearest() {
    let Some((gpu, mut renderer)) = setup() else { return };
    let supported = renderer.anisotropic_filtering_supported();
    eprintln!("anisotropy test queried DownlevelFlags::ANISOTROPIC_FILTERING={supported}");

    let mut assets = AssetManager::in_memory();
    let handle = insert_sampling_texture(
        &mut assets,
        Texture::solid(2, 2, [8, 16, 32, 255]).with_sampler(linear_aniso(8)).unwrap(),
    );
    let (size, at) = full_sprite();
    let mut world = sampling_world(size, None, at);
    render(&gpu, &mut renderer, &mut world, &assets);
    let info = renderer.cached_texture(handle.id()).unwrap();
    assert_eq!(info.min_filter, TextureFilter::Linear);
    assert_eq!(info.mag_filter, TextureFilter::Linear);
    assert!(info.mip_filter_linear);
    if supported {
        assert_eq!(info.anisotropy, 8);
        assert_eq!(renderer.resource_stats().anisotropy_capability_fallbacks, 0);
    } else {
        assert_eq!(info.anisotropy, 1);
        assert_eq!(renderer.resource_stats().anisotropy_capability_fallbacks, 1);
    }

    let mut nearest = Texture::solid(2, 2, [8, 16, 32, 255]);
    nearest.sampler.anisotropy = 8;
    nearest.sampler.min_filter = TextureFilter::Nearest;
    assets.insert("tex", nearest).unwrap();
    render(&gpu, &mut renderer, &mut world, &assets);
    let nearest_info = renderer.cached_texture(handle.id()).unwrap();
    assert_eq!(nearest_info.anisotropy, 1);
    assert_eq!(nearest_info.min_filter, TextureFilter::Nearest);
    assert_eq!(nearest_info.mag_filter, TextureFilter::Nearest);

    assets.insert("tex", Texture::solid(2, 2, [8, 16, 32, 255]).with_sampler(linear_aniso(3)).unwrap()).unwrap();
    render(&gpu, &mut renderer, &mut world, &assets);
    let odd = renderer.cached_texture(handle.id()).unwrap();
    assert_eq!(odd.anisotropy, if supported { 3 } else { 1 });
    assert_eq!(odd.min_filter, TextureFilter::Linear);

    let mut invalid = Texture::solid(2, 2, [8, 16, 32, 255]).with_filter(TextureFilter::Linear);
    invalid.sampler.anisotropy = 0;
    assets.insert("tex", invalid).unwrap();
    render(&gpu, &mut renderer, &mut world, &assets);
    let corrected = renderer.cached_texture(handle.id()).unwrap();
    assert_eq!(corrected.anisotropy, 1);
    assert_eq!(corrected.min_filter, TextureFilter::Linear);
    assert_eq!(renderer.resource_stats().invalid_anisotropy_fallbacks, 1);
    render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(renderer.resource_stats().invalid_anisotropy_fallbacks, 1, "the invalid clamp is logged once");
}

#[test]
fn one_allocated_mip_level_clamps_sampler_lod() {
    let Some((gpu, mut renderer)) = setup() else { return };
    let before = renderer.resource_stats();
    let mut assets = AssetManager::in_memory();
    let generated = Texture::solid(4, 4, [1, 2, 3, 255]).with_mipmaps(Mipmaps::Generate);
    assert_eq!(generated.mips, Mipmaps::Generate);
    let handle = insert_sampling_texture(&mut assets, generated);
    let (size, at) = full_sprite();
    let mut world = sampling_world(size, None, at);
    render(&gpu, &mut renderer, &mut world, &assets);
    let info = renderer.cached_texture(handle.id()).unwrap();
    assert_eq!(assets.get(handle).unwrap().mips, Mipmaps::Generate);
    assert_eq!(info.mip_level_count, 3, "4x4 generates floor(log2(4))+1 levels");
    assert_eq!((info.lod_min, info.lod_max), (0.0, 32.0), "no mip filter keeps the base-level LOD range");
    assert!(!info.mip_filter_linear);
    assert_eq!(renderer.sampler_cache_len(), 1, "base-level sampling reuses the placeholder sampler");
    assert_eq!(
        renderer.resource_stats().sampler_creates,
        before.sampler_creates + 1,
        "the shared blit sampler is created once"
    );
    assert_eq!(renderer.resource_stats().mip_generations, before.mip_generations + 1);

    assets
        .insert(
            "tex",
            Texture::solid(4, 4, [1, 2, 3, 255])
                .with_mipmaps(Mipmaps::Generate)
                .with_sampler(SamplerDesc { mip_filter: Some(MipFilter::Nearest), ..SamplerDesc::default() })
                .unwrap(),
        )
        .unwrap();
    render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(renderer.resource_stats().sampler_creates, before.sampler_creates + 2);
    assert_eq!(
        renderer.resource_stats().mip_generations,
        before.mip_generations + 1,
        "sampler-only does not rebuild mips"
    );
    assert_eq!(renderer.cached_texture(handle.id()).unwrap().lod_max, 2.0);
    assert_eq!(renderer.cached_texture(handle.id()).unwrap().mip_level_count, 3);

    assets
        .insert(
            "tex",
            Texture::solid(4, 4, [1, 2, 3, 255])
                .with_sampler(SamplerDesc { mip_filter: Some(MipFilter::Linear), ..SamplerDesc::default() })
                .unwrap(),
        )
        .unwrap();
    render(&gpu, &mut renderer, &mut world, &assets);
    let linear = renderer.cached_texture(handle.id()).unwrap();
    assert!(linear.mip_filter_linear);
    assert_eq!((linear.lod_min, linear.lod_max), (0.0, 32.0));
    assert_eq!(linear.mip_level_count, 1);
    assert_eq!(renderer.resource_stats().sampler_creates, before.sampler_creates + 3);
    assert_eq!(
        renderer.resource_stats().mip_generations,
        before.mip_generations + 1,
        "dropping to one level does not blit"
    );
    assert_eq!(assets.get(handle).unwrap().mips, Mipmaps::None);
}

#[test]
fn asset_revision_reuploads_storage_only_when_it_changes() {
    let Some((gpu, mut renderer)) = setup() else { return };
    let mut assets = AssetManager::in_memory();
    let handle = insert_sampling_texture(&mut assets, Texture::solid(1, 1, [9, 8, 7, 255]));
    let (size, at) = full_sprite();
    let mut world = sampling_world(size, None, at);
    render(&gpu, &mut renderer, &mut world, &assets);
    let baseline = renderer.resource_stats();
    let fingerprint = renderer.cached_texture(handle.id()).unwrap().pixel_fingerprint;

    assets.insert("tex", Texture::solid(1, 1, [9, 8, 7, 255])).unwrap();
    render(&gpu, &mut renderer, &mut world, &assets);
    let same_pixels = renderer.resource_stats();
    assert_eq!(same_pixels.texture_writes, baseline.texture_writes);
    assert_eq!(same_pixels.texture_creates, baseline.texture_creates);
    assert_eq!(same_pixels.pixel_fingerprints, baseline.pixel_fingerprints + 1);
    assert_eq!(renderer.cached_texture(handle.id()).unwrap().pixel_fingerprint, fingerprint);

    assets.insert("tex", Texture::solid(1, 1, [1, 2, 3, 255])).unwrap();
    render(&gpu, &mut renderer, &mut world, &assets);
    let new_pixels = renderer.resource_stats();
    assert_eq!(new_pixels.texture_writes, baseline.texture_writes + 1);
    assert_eq!(new_pixels.texture_creates, baseline.texture_creates, "same dimensions and format keep the image");
    assert_ne!(renderer.cached_texture(handle.id()).unwrap().pixel_fingerprint, fingerprint);

    assets.insert("tex", Texture::solid(1, 1, [1, 2, 3, 255]).with_color_space(ColorSpace::Linear)).unwrap();
    render(&gpu, &mut renderer, &mut world, &assets);
    let recolored = renderer.resource_stats();
    assert_eq!(recolored.texture_creates, new_pixels.texture_creates + 1);
    assert_eq!(recolored.texture_writes, new_pixels.texture_writes + 1);
    assert_eq!(renderer.cached_texture(handle.id()).unwrap().color_space, ColorSpace::Linear);

    assets.insert("tex", Texture::solid(2, 2, [1, 2, 3, 255]).with_color_space(ColorSpace::Linear)).unwrap();
    render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(renderer.cached_texture(handle.id()).unwrap().width, 2);
    assert_eq!(renderer.resource_stats().texture_creates, recolored.texture_creates + 1);

    let writes = renderer.resource_stats().texture_writes;
    let creates = renderer.resource_stats().texture_creates;
    assets
        .insert(
            "tex",
            Texture::solid(2, 2, [1, 2, 3, 255])
                .with_color_space(ColorSpace::Linear)
                .with_sampler(SamplerDesc { address_u: AddressMode::Mirror, ..SamplerDesc::default() })
                .unwrap(),
        )
        .unwrap();
    render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(renderer.resource_stats().texture_writes, writes);
    assert_eq!(renderer.resource_stats().texture_creates, creates);
    assert_eq!(renderer.cached_texture(handle.id()).unwrap().address_u, AddressMode::Mirror);
    assert_eq!(renderer.sampler_cache_len(), 2);

    assets.remove(handle);
    let mut empty = World::new();
    empty.spawn((Camera::orthographic(2.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -4.0)));
    render(&gpu, &mut renderer, &mut empty, &assets);
    assert_eq!(renderer.cache_stats().textures, 0);
    assert_eq!(renderer.sampler_cache_len(), 1, "the unused sampler goes; the placeholder sampler stays");
    assert!(renderer.cached_texture(handle.id()).is_none());
}

fn linear_renderer(gpu: &GpuContext) -> SceneRenderer {
    SceneRenderer::new(gpu, wgpu::TextureFormat::Rgba8Unorm)
}

fn checker(size: u32, mips: Mipmaps, sampler: SamplerDesc) -> Texture {
    Texture::from_fn(size, size, |x, y| if (x + y) % 2 == 0 { [0, 0, 0, 255] } else { [255, 255, 255, 255] })
        .with_color_space(ColorSpace::Linear)
        .with_mipmaps(mips)
        .with_sampler(sampler)
        .unwrap()
}

fn nearest_sampler(mip: Option<MipFilter>) -> SamplerDesc {
    SamplerDesc {
        min_filter: TextureFilter::Nearest,
        mag_filter: TextureFilter::Nearest,
        mip_filter: mip,
        ..SamplerDesc::default()
    }
}

fn is_near_gray(pixel: [u8; 4]) -> bool {
    pixel.iter().take(3).all(|c| (110..=145).contains(c)) && pixel[3] > 250
}

fn is_extreme(pixel: [u8; 4]) -> bool {
    (pixel[0] < 30 || pixel[0] > 225) && (pixel[0] as i16 - pixel[1] as i16).abs() < 8
}

fn camera_world(camera: Camera, size: Vec2) -> World {
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.spawn((camera, Transform::from_xyz(0.0, 0.0, -4.0)));
    world.spawn((
        Transform::IDENTITY,
        Sprite::new("sprite").with_size(size).with_anchor(Vec2::splat(0.5)).unlit().cutout(),
    ));
    world
}

fn render_checker(
    gpu: &GpuContext,
    renderer: &mut SceneRenderer,
    assets: &mut AssetManager,
    texture: Texture,
    camera: Camera,
    size: Vec2,
) -> ([u8; 4], engine_renderer_wgpu::CachedTextureInfo) {
    let handle = insert_sampling_texture(assets, texture);
    let mut world = camera_world(camera, size);
    let image = render(gpu, renderer, &mut world, assets);
    (px(&image, 64, 64), renderer.cached_texture(handle.id()).expect("cached"))
}

#[test]
fn generated_mips_average_a_minified_checker_and_alias_without_them() {
    let Some((gpu, _)) = setup() else { return };
    let mut renderer = linear_renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    let far = Camera::orthographic(64.0).with_clear_color(Color::BLACK);
    let (aliased, alias_info) = render_checker(
        &gpu,
        &mut renderer,
        &mut assets,
        checker(32, Mipmaps::None, nearest_sampler(None)),
        far,
        Vec2::splat(1.0),
    );
    assert_eq!(alias_info.mip_level_count, 1);
    assert!(is_extreme(aliased), "one-level nearest minification aliases the checker {aliased:?}");

    let (averaged, mip_info) = render_checker(
        &gpu,
        &mut renderer,
        &mut assets,
        checker(32, Mipmaps::Generate, nearest_sampler(Some(MipFilter::Nearest))),
        far,
        Vec2::splat(1.0),
    );
    assert_eq!(mip_info.mip_level_count, 6);
    assert_eq!(mip_info.lod_max, 5.0);
    assert!(is_near_gray(averaged), "nearest mip of the minified checker approaches the mean {averaged:?}");
    assert!(!is_near_gray(aliased) && is_near_gray(averaged));

    let (linear_mip, linear_info) = render_checker(
        &gpu,
        &mut renderer,
        &mut assets,
        checker(32, Mipmaps::Generate, nearest_sampler(Some(MipFilter::Linear))),
        far,
        Vec2::splat(1.0),
    );
    assert!(linear_info.mip_filter_linear);
    assert_eq!(linear_info.lod_max, 5.0);
    assert!(is_near_gray(linear_mip), "linear mip of the minified checker approaches the mean {linear_mip:?}");

    let close = Camera::orthographic(2.0).with_clear_color(Color::BLACK);
    let handle =
        insert_sampling_texture(&mut assets, checker(32, Mipmaps::Generate, nearest_sampler(Some(MipFilter::Linear))));
    let mut world = camera_world(close, Vec2::splat(2.0));
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(renderer.cached_texture(handle.id()).unwrap().mip_level_count, 6);
    let black = px(&image, 1, 1);
    let white = px(&image, 6, 1);
    assert!(is_extreme(black) && black[0] < 30, "magnified ortho zoom stays on the base checker {black:?}");
    assert!(is_extreme(white) && white[0] > 225, "the neighbouring magnified texel stays white {white:?}");
}

#[test]
fn perspective_and_orthographic_zoom_both_select_mip_levels() {
    let Some((gpu, _)) = setup() else { return };
    let mut renderer = linear_renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    let generated = checker(32, Mipmaps::Generate, nearest_sampler(Some(MipFilter::Linear)));
    let plain = checker(32, Mipmaps::None, nearest_sampler(None));

    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.spawn((Camera::perspective(60.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -40.0)));
    let handle = insert_sampling_texture(&mut assets, generated);
    world.spawn((
        Transform::IDENTITY,
        Sprite::new("sprite").with_size(Vec2::splat(1.0)).with_anchor(Vec2::splat(0.5)).unlit().cutout(),
    ));
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    let with_mips = px(&image, 64, 64);
    assert!(renderer.cached_texture(handle.id()).unwrap().mip_level_count > 1);
    assert!(is_near_gray(with_mips), "perspective minification approaches the mean {with_mips:?}");

    insert_sampling_texture(&mut assets, plain);
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    let without = px(&image, 64, 64);
    assert!(is_extreme(without), "perspective without mips aliases {without:?}");

    let (ortho_far, _) = render_checker(
        &gpu,
        &mut renderer,
        &mut assets,
        checker(32, Mipmaps::Generate, nearest_sampler(Some(MipFilter::Nearest))),
        Camera::orthographic(48.0).with_clear_color(Color::BLACK),
        Vec2::splat(1.0),
    );
    assert!(is_near_gray(ortho_far), "orthographic zoom-out selects a mip {ortho_far:?}");
}

#[test]
fn generated_mips_keep_base_level_sampling_until_a_mip_filter_is_set() {
    let Some((gpu, mut renderer)) = setup() else { return };
    let mut assets = AssetManager::in_memory();
    let minified = Texture::from_fn(32, 1, |x, _| if x < 16 { [255, 0, 0, 255] } else { [0, 0, 255, 255] })
        .with_mipmaps(Mipmaps::Generate)
        .with_sampler(SamplerDesc {
            min_filter: TextureFilter::Linear,
            mag_filter: TextureFilter::Nearest,
            mip_filter: None,
            ..SamplerDesc::default()
        })
        .unwrap();
    let center = -1.0 + (64.5 / W as f32) * 2.0;
    let sprite = Vec2::new(8.0 * 2.0 / W as f32, 2.0);
    let handle = insert_sampling_texture(&mut assets, minified);
    let mut world = sampling_world(sprite, None, Transform::from_xyz(center, 0.0, 0.0));
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    let info = renderer.cached_texture(handle.id()).unwrap();
    assert!(info.mip_level_count > 1);
    assert_eq!(info.lod_max, 32.0);
    assert!(
        mixed_red_blue(px(&image, 64, 64)),
        "generated mips without a mip filter still min-filter the base level {:?}",
        px(&image, 64, 64)
    );

    let magnified = Texture::from_rgba8(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255])
        .with_mipmaps(Mipmaps::Generate)
        .with_sampler(SamplerDesc {
            min_filter: TextureFilter::Linear,
            mag_filter: TextureFilter::Nearest,
            mip_filter: Some(MipFilter::Linear),
            ..SamplerDesc::default()
        })
        .unwrap();
    insert_sampling_texture(&mut assets, magnified);
    let mut world = sampling_world(Vec2::splat(2.0), None, Transform::IDENTITY);
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    assert!(
        is_red(px(&image, 63, 64)),
        "magnification uses mag nearest while the chain exists {:?}",
        px(&image, 63, 64)
    );
}

#[test]
fn srgb_and_linear_mip_samples_are_about_half_linear_intensity() {
    let Some((gpu, _)) = setup() else { return };
    let mut renderer = linear_renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    let camera = Camera::orthographic(64.0).with_clear_color(Color::BLACK);
    for space in [ColorSpace::Srgb, ColorSpace::Linear] {
        let texture =
            Texture::from_fn(32, 32, |x, y| if (x + y) % 2 == 0 { [0, 0, 0, 255] } else { [255, 255, 255, 255] })
                .with_color_space(space)
                .with_mipmaps(Mipmaps::Generate)
                .with_sampler(nearest_sampler(Some(MipFilter::Linear)))
                .unwrap();
        let (pixel, info) = render_checker(&gpu, &mut renderer, &mut assets, texture, camera, Vec2::splat(1.0));
        assert_eq!(info.mip_level_count, 6);
        assert!(is_near_gray(pixel), "{space:?} black/white mip samples near 0.5 linear on a linear target {pixel:?}");
    }
}

#[test]
fn mip_generation_follows_pixels_allocation_and_eviction() {
    let Some((gpu, mut renderer)) = setup() else { return };
    let mut assets = AssetManager::in_memory();
    let before = renderer.resource_stats();
    let handle =
        insert_sampling_texture(&mut assets, Texture::solid(8, 8, [20, 30, 40, 255]).with_mipmaps(Mipmaps::Generate));
    let (size, at) = full_sprite();
    let mut world = sampling_world(size, None, at);
    render(&gpu, &mut renderer, &mut world, &assets);
    let uploaded = renderer.resource_stats();
    assert_eq!(uploaded.texture_creates, before.texture_creates + 1);
    assert_eq!(uploaded.texture_writes, before.texture_writes + 1);
    assert_eq!(uploaded.mip_generations, before.mip_generations + 1);
    assert_eq!(renderer.cached_texture(handle.id()).unwrap().mip_level_count, 4);
    let fingerprint = renderer.cached_texture(handle.id()).unwrap().pixel_fingerprint;

    assets
        .insert(
            "tex",
            Texture::solid(8, 8, [20, 30, 40, 255])
                .with_mipmaps(Mipmaps::Generate)
                .with_sampler(SamplerDesc { address_u: AddressMode::Repeat, ..SamplerDesc::default() })
                .unwrap(),
        )
        .unwrap();
    render(&gpu, &mut renderer, &mut world, &assets);
    let sampled = renderer.resource_stats();
    assert_eq!(sampled.mip_generations, uploaded.mip_generations);
    assert_eq!(sampled.texture_creates, uploaded.texture_creates);
    assert_eq!(sampled.texture_writes, uploaded.texture_writes);
    assert!(sampled.bind_group_creates > uploaded.bind_group_creates);
    assert_eq!(renderer.cached_texture(handle.id()).unwrap().pixel_fingerprint, fingerprint);

    assets
        .insert(
            "tex",
            Texture::solid(8, 8, [21, 30, 40, 255])
                .with_mipmaps(Mipmaps::Generate)
                .with_sampler(SamplerDesc { address_u: AddressMode::Repeat, ..SamplerDesc::default() })
                .unwrap(),
        )
        .unwrap();
    render(&gpu, &mut renderer, &mut world, &assets);
    let pixels = renderer.resource_stats();
    assert_eq!(pixels.texture_creates, sampled.texture_creates);
    assert_eq!(pixels.texture_writes, sampled.texture_writes + 1);
    assert_eq!(pixels.mip_generations, sampled.mip_generations + 1);
    assert_ne!(renderer.cached_texture(handle.id()).unwrap().pixel_fingerprint, fingerprint);

    assets
        .insert(
            "tex",
            Texture::solid(8, 8, [21, 30, 40, 255])
                .with_sampler(SamplerDesc { address_u: AddressMode::Repeat, ..SamplerDesc::default() })
                .unwrap(),
        )
        .unwrap();
    render(&gpu, &mut renderer, &mut world, &assets);
    let one_level = renderer.resource_stats();
    assert_eq!(renderer.cached_texture(handle.id()).unwrap().mip_level_count, 1);
    assert_eq!(one_level.texture_creates, pixels.texture_creates + 1);
    assert_eq!(one_level.mip_generations, pixels.mip_generations);

    assets
        .insert(
            "tex",
            Texture::solid(8, 8, [21, 30, 40, 255])
                .with_color_space(ColorSpace::Linear)
                .with_mipmaps(Mipmaps::Generate)
                .with_sampler(SamplerDesc { address_u: AddressMode::Repeat, ..SamplerDesc::default() })
                .unwrap(),
        )
        .unwrap();
    render(&gpu, &mut renderer, &mut world, &assets);
    let recolored = renderer.resource_stats();
    assert_eq!(renderer.cached_texture(handle.id()).unwrap().color_space, ColorSpace::Linear);
    assert_eq!(renderer.cached_texture(handle.id()).unwrap().mip_level_count, 4);
    assert_eq!(recolored.texture_creates, one_level.texture_creates + 1);
    assert_eq!(recolored.mip_generations, one_level.mip_generations + 1);

    assets.remove(handle);
    let mut empty = World::new();
    empty.spawn((Camera::orthographic(2.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -4.0)));
    render(&gpu, &mut renderer, &mut empty, &assets);
    assert!(renderer.cached_texture(handle.id()).is_none());
    assert_eq!(renderer.cache_stats().textures, 0);
}

#[test]
fn npot_mip_allocation_follows_the_device_flag() {
    let Some((gpu, mut renderer)) = setup() else { return };
    let supported = renderer.npot_mipmapped_textures_supported();
    let flags = gpu.adapter.get_downlevel_capabilities().flags;
    eprintln!(
        "NPOT mip test: NON_POWER_OF_TWO_MIPMAPPED_TEXTURES={supported}; adapter {:?}",
        gpu.adapter.get_info().name
    );
    assert_eq!(supported, flags.contains(wgpu::DownlevelFlags::NON_POWER_OF_TWO_MIPMAPPED_TEXTURES));
    let mut assets = AssetManager::in_memory();
    let before = renderer.resource_stats();
    let handle = insert_sampling_texture(
        &mut assets,
        Texture::solid(6, 4, [8, 8, 8, 255])
            .with_mipmaps(Mipmaps::Generate)
            .with_sampler(nearest_sampler(Some(MipFilter::Nearest)))
            .unwrap(),
    );
    let (size, at) = full_sprite();
    let mut world = sampling_world(size, None, at);
    render(&gpu, &mut renderer, &mut world, &assets);
    let info = renderer.cached_texture(handle.id()).unwrap();
    if supported {
        assert_eq!(info.mip_level_count, 3);
        assert_eq!(info.lod_max, 2.0);
        assert_eq!(renderer.resource_stats().npot_mip_fallbacks, before.npot_mip_fallbacks);
        assert!(renderer.resource_stats().mip_generations > before.mip_generations);
        eprintln!("NPOT hardware fallback was not exercised; the adapter supports the flag");
    } else {
        assert_eq!(info.mip_level_count, 1);
        assert_eq!(renderer.resource_stats().npot_mip_fallbacks, before.npot_mip_fallbacks + 1);
        render(&gpu, &mut renderer, &mut world, &assets);
        assert_eq!(renderer.resource_stats().npot_mip_fallbacks, before.npot_mip_fallbacks + 1);
    }

    let narrow = insert_sampling_texture(
        &mut assets,
        Texture::solid(1, 16, [4, 5, 6, 255])
            .with_mipmaps(Mipmaps::Generate)
            .with_sampler(nearest_sampler(None))
            .unwrap(),
    );
    render(&gpu, &mut renderer, &mut world, &assets);
    let levels = renderer.cached_texture(narrow.id()).unwrap().mip_level_count;
    assert_eq!(levels, 5, "1x16 is a power of two, so the NPOT fallback does not apply");
    let point =
        insert_sampling_texture(&mut assets, Texture::solid(1, 1, [4, 5, 6, 255]).with_mipmaps(Mipmaps::Generate));
    let generations = renderer.resource_stats().mip_generations;
    render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(renderer.cached_texture(point.id()).unwrap().mip_level_count, 1);
    assert_eq!(renderer.resource_stats().mip_generations, generations);
}

fn gpu_texel(image: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * width + x) * 4) as usize;
    [image[i], image[i + 1], image[i + 2], image[i + 3]]
}

fn publish_tex(assets: &mut AssetManager, edit: impl FnOnce(&mut Texture)) {
    let handle = assets.texture("tex");
    let mut texture = assets.get(handle).expect("tex").clone();
    edit(&mut texture);
    assets.insert("tex", texture).unwrap();
}

#[test]
fn a_region_update_uploads_only_that_rectangle() {
    let Some((gpu, mut renderer)) = setup() else { return };
    let mut assets = AssetManager::in_memory();
    let handle = insert_sampling_texture(&mut assets, Texture::solid(8, 8, [10, 20, 30, 255]));
    let (size, at) = full_sprite();
    let mut world = sampling_world(size, None, at);
    render(&gpu, &mut renderer, &mut world, &assets);
    let before = renderer.resource_stats();

    publish_tex(&mut assets, |texture| {
        texture.update_region(2, 3, 2, 1, &[255, 0, 0, 255, 0, 255, 0, 255]).unwrap();
    });
    render(&gpu, &mut renderer, &mut world, &assets);
    let after = renderer.resource_stats();
    assert_eq!(after.partial_base_uploads, before.partial_base_uploads + 1);
    assert_eq!(after.full_base_uploads, before.full_base_uploads);
    assert_eq!(after.texture_writes, before.texture_writes + 1);
    assert_eq!(after.base_upload_bytes, before.base_upload_bytes + 8);
    assert_eq!(after.texture_creates, before.texture_creates);
    assert_eq!(after.sampler_creates, before.sampler_creates);
    assert_eq!(after.bind_group_creates, before.bind_group_creates);
    assert_eq!(after.mip_generations, before.mip_generations);
    assert_eq!(after.pixel_fingerprints, before.pixel_fingerprints + 1);

    let uploaded = renderer.read_cached_mip(&gpu, handle.id(), 0).expect("base level");
    assert_eq!(uploaded, assets.get(handle).unwrap().rgba);
    assert_eq!(gpu_texel(&uploaded, 8, 1, 3), [10, 20, 30, 255]);
    assert_eq!(gpu_texel(&uploaded, 8, 2, 3), [255, 0, 0, 255]);
    assert_eq!(gpu_texel(&uploaded, 8, 3, 3), [0, 255, 0, 255]);
    assert_eq!(gpu_texel(&uploaded, 8, 4, 3), [10, 20, 30, 255]);

    render(&gpu, &mut renderer, &mut world, &assets);
    let again = renderer.resource_stats();
    assert_eq!(again.partial_base_uploads, after.partial_base_uploads);
    assert_eq!(again.full_base_uploads, after.full_base_uploads);
    assert_eq!(again.pixel_fingerprints, after.pixel_fingerprints);
}

#[test]
fn successive_frames_upload_each_new_region_and_keep_the_earlier_ones() {
    let Some((gpu, mut renderer)) = setup() else { return };
    let mut assets = AssetManager::in_memory();
    let handle = insert_sampling_texture(&mut assets, Texture::solid(8, 8, [0, 0, 0, 255]));
    let (size, at) = full_sprite();
    let mut world = sampling_world(size, None, at);
    render(&gpu, &mut renderer, &mut world, &assets);
    let uploaded = renderer.resource_stats();

    publish_tex(&mut assets, |texture| {
        texture.update_region(0, 0, 2, 2, &[255, 0, 0, 255].repeat(4)).unwrap();
    });
    render(&gpu, &mut renderer, &mut world, &assets);
    let frame_a = renderer.resource_stats();
    assert_eq!(frame_a.partial_base_uploads, uploaded.partial_base_uploads + 1);
    assert_eq!(frame_a.full_base_uploads, uploaded.full_base_uploads);
    assert_eq!(frame_a.texture_creates, uploaded.texture_creates);

    publish_tex(&mut assets, |texture| {
        texture.update_region(6, 6, 2, 2, &[0, 255, 0, 255].repeat(4)).unwrap();
    });
    render(&gpu, &mut renderer, &mut world, &assets);
    let frame_b = renderer.resource_stats();
    assert_eq!(frame_b.partial_base_uploads, frame_a.partial_base_uploads + 1);
    assert_eq!(frame_b.full_base_uploads, uploaded.full_base_uploads);

    publish_tex(&mut assets, |texture| {
        texture.update_region(1, 1, 2, 2, &[0, 0, 255, 255].repeat(4)).unwrap();
    });
    render(&gpu, &mut renderer, &mut world, &assets);
    let frame_c = renderer.resource_stats();
    assert_eq!(frame_c.partial_base_uploads, frame_b.partial_base_uploads + 1);
    assert_eq!(frame_c.full_base_uploads, uploaded.full_base_uploads);
    assert_eq!(frame_c.base_upload_bytes, uploaded.base_upload_bytes + 16 + 16 + 16);

    let pixels = renderer.read_cached_mip(&gpu, handle.id(), 0).unwrap();
    assert_eq!(pixels, assets.get(handle).unwrap().rgba);
    assert_eq!(gpu_texel(&pixels, 8, 0, 0), [255, 0, 0, 255], "the first region stays");
    assert_eq!(gpu_texel(&pixels, 8, 1, 1), [0, 0, 255, 255], "the overlap uses the latest bytes");
    assert_eq!(gpu_texel(&pixels, 8, 2, 2), [0, 0, 255, 255]);
    assert_eq!(gpu_texel(&pixels, 8, 6, 6), [0, 255, 0, 255], "the disjoint region stays");
    assert_eq!(gpu_texel(&pixels, 8, 4, 4), [0, 0, 0, 255]);
}

#[test]
fn skipped_revisions_upload_the_retained_suffix_and_old_history_falls_back_to_full() {
    let Some((gpu, mut renderer)) = setup() else { return };
    let mut assets = AssetManager::in_memory();
    let handle = insert_sampling_texture(&mut assets, Texture::solid(4, 4, [1, 1, 1, 255]));
    let (size, at) = full_sprite();
    let mut world = sampling_world(size, None, at);
    render(&gpu, &mut renderer, &mut world, &assets);
    let baseline = renderer.resource_stats();

    publish_tex(&mut assets, |texture| {
        texture.update_region(0, 0, 1, 1, &[255, 0, 0, 255]).unwrap();
        texture.update_region(3, 3, 1, 1, &[0, 0, 255, 255]).unwrap();
    });
    render(&gpu, &mut renderer, &mut world, &assets);
    let skipped = renderer.resource_stats();
    assert_eq!(skipped.partial_base_uploads, baseline.partial_base_uploads + 2);
    assert_eq!(skipped.full_base_uploads, baseline.full_base_uploads);
    assert_eq!(skipped.base_upload_bytes, baseline.base_upload_bytes + 8);
    assert_eq!(skipped.texture_creates, baseline.texture_creates);
    let pixels = renderer.read_cached_mip(&gpu, handle.id(), 0).unwrap();
    assert_eq!(gpu_texel(&pixels, 4, 0, 0), [255, 0, 0, 255]);
    assert_eq!(gpu_texel(&pixels, 4, 3, 3), [0, 0, 255, 255]);
    assert_eq!(gpu_texel(&pixels, 4, 1, 1), [1, 1, 1, 255]);

    publish_tex(&mut assets, |texture| {
        for step in 0..MAX_TRACKED_REGIONS + 1 {
            let byte = u8::try_from(step + 2).unwrap();
            texture.update_region(0, 0, 1, 1, &[byte, 0, 0, 255]).unwrap();
        }
    });
    render(&gpu, &mut renderer, &mut world, &assets);
    let overflow = renderer.resource_stats();
    assert_eq!(overflow.full_base_uploads, skipped.full_base_uploads + 1);
    assert_eq!(overflow.partial_base_uploads, skipped.partial_base_uploads);
    assert_eq!(overflow.texture_creates, skipped.texture_creates);
    let after_full = renderer.read_cached_mip(&gpu, handle.id(), 0).unwrap();
    assert_eq!(after_full, assets.get(handle).unwrap().rgba);

    publish_tex(&mut assets, |texture| {
        texture.update_region(2, 2, 1, 1, &[0, 255, 0, 255]).unwrap();
    });
    render(&gpu, &mut renderer, &mut world, &assets);
    let resumed = renderer.resource_stats();
    assert_eq!(resumed.partial_base_uploads, overflow.partial_base_uploads + 1);
    assert_eq!(resumed.full_base_uploads, overflow.full_base_uploads);
    let resumed_pixels = renderer.read_cached_mip(&gpu, handle.id(), 0).unwrap();
    assert_eq!(gpu_texel(&resumed_pixels, 4, 2, 2), [0, 255, 0, 255]);
    assert_eq!(gpu_texel(&resumed_pixels, 4, 3, 3), [0, 0, 255, 255]);
}

#[test]
fn a_partial_base_upload_regenerates_the_mip_chain_from_the_new_base() {
    let Some((gpu, _)) = setup() else { return };
    let npot = gpu
        .adapter
        .get_downlevel_capabilities()
        .flags
        .contains(wgpu::DownlevelFlags::NON_POWER_OF_TWO_MIPMAPPED_TEXTURES);
    for (space, size) in [(ColorSpace::Srgb, 8u32), (ColorSpace::Linear, 8), (ColorSpace::Linear, 5)] {
        let mut renderer = SceneRenderer::new(&gpu, wgpu::TextureFormat::Rgba8Unorm);
        let mut assets = AssetManager::in_memory();
        let source = Texture::from_fn(size, size, |x, y| [x as u8 * 20, y as u8 * 20, 40, 255])
            .with_color_space(space)
            .with_mipmaps(Mipmaps::Generate);
        let handle = insert_sampling_texture(&mut assets, source);
        let (sprite, at) = full_sprite();
        let mut world = sampling_world(sprite, None, at);
        render(&gpu, &mut renderer, &mut world, &assets);
        let before = renderer.resource_stats();
        let levels = renderer.cached_texture(handle.id()).unwrap().mip_level_count;
        if size == 5 && !npot {
            assert_eq!(levels, 1);
        } else {
            assert!(levels > 1, "{space:?} {size} allocated {levels}");
        }

        publish_tex(&mut assets, |texture| {
            texture.update_region(1, 1, 1, 1, &[255, 255, 255, 128]).unwrap();
        });
        render(&gpu, &mut renderer, &mut world, &assets);
        let after = renderer.resource_stats();
        assert_eq!(after.partial_base_uploads, before.partial_base_uploads + 1);
        assert_eq!(after.full_base_uploads, before.full_base_uploads);
        assert_eq!(after.base_upload_bytes, before.base_upload_bytes + 4);
        assert_eq!(after.texture_creates, before.texture_creates);
        assert_eq!(after.sampler_creates, before.sampler_creates);
        if levels > 1 {
            assert_eq!(after.mip_generations, before.mip_generations + 1);
        } else {
            assert_eq!(after.mip_generations, before.mip_generations);
        }

        let final_cpu = assets.get(handle).unwrap().clone();
        let mut fresh_assets = AssetManager::in_memory();
        let mut fresh = SceneRenderer::new(&gpu, wgpu::TextureFormat::Rgba8Unorm);
        let fresh_handle = insert_sampling_texture(&mut fresh_assets, final_cpu.clone());
        let mut fresh_world = sampling_world(sprite, None, at);
        render(&gpu, &mut fresh, &mut fresh_world, &fresh_assets);
        assert_eq!(fresh.resource_stats().partial_base_uploads, 0);
        assert!(fresh.resource_stats().full_base_uploads >= 1);
        let fresh_levels = fresh.cached_texture(fresh_handle.id()).unwrap().mip_level_count;
        assert_eq!(fresh_levels, levels);
        for level in 0..levels {
            let updated = renderer.read_cached_mip(&gpu, handle.id(), level).unwrap();
            let rebuilt = fresh.read_cached_mip(&gpu, fresh_handle.id(), level).unwrap();
            assert_eq!(updated.len(), rebuilt.len(), "{space:?} {size} level {level}");
            for (index, (got, want)) in updated.iter().zip(rebuilt.iter()).enumerate() {
                let delta = (*got as i16 - *want as i16).abs();
                assert!(delta <= 1, "{space:?} {size} level {level} byte {index}: {got} vs {want}");
            }
        }
    }
}

#[test]
fn cache_policy_selects_full_partial_or_no_upload_and_rejects_oversized_textures() {
    let Some((gpu, mut renderer)) = setup() else { return };
    capture::begin();
    let mut assets = AssetManager::in_memory();
    let handle =
        insert_sampling_texture(&mut assets, Texture::solid(4, 4, [8, 8, 8, 255]).with_mipmaps(Mipmaps::Generate));
    let (size, at) = full_sprite();
    let mut world = sampling_world(size, None, at);
    render(&gpu, &mut renderer, &mut world, &assets);
    let baseline = renderer.resource_stats();
    assert!(baseline.mip_generations >= 1);
    let groups = baseline.bind_group_creates;

    publish_tex(&mut assets, |texture| {
        *texture =
            texture.clone().with_sampler(SamplerDesc { address_u: AddressMode::Repeat, ..texture.sampler }).unwrap();
    });
    render(&gpu, &mut renderer, &mut world, &assets);
    let sampled = renderer.resource_stats();
    assert_eq!(sampled.texture_writes, baseline.texture_writes);
    assert_eq!(sampled.full_base_uploads, baseline.full_base_uploads);
    assert_eq!(sampled.partial_base_uploads, baseline.partial_base_uploads);
    assert_eq!(sampled.mip_generations, baseline.mip_generations);
    assert_eq!(sampled.texture_creates, baseline.texture_creates);
    assert!(sampled.bind_group_creates > groups);
    assert_eq!(sampled.pixel_fingerprints, baseline.pixel_fingerprints + 1);

    assets.insert("tex", Texture::solid(4, 4, [8, 8, 8, 255]).with_mipmaps(Mipmaps::Generate)).unwrap();
    render(&gpu, &mut renderer, &mut world, &assets);
    let same = renderer.resource_stats();
    assert_eq!(same.texture_writes, sampled.texture_writes);
    assert_eq!(same.full_base_uploads, sampled.full_base_uploads);
    assert_eq!(same.partial_base_uploads, sampled.partial_base_uploads);

    assets.insert("tex", Texture::solid(4, 4, [9, 8, 8, 255]).with_mipmaps(Mipmaps::Generate)).unwrap();
    render(&gpu, &mut renderer, &mut world, &assets);
    let replaced = renderer.resource_stats();
    assert_eq!(replaced.full_base_uploads, same.full_base_uploads + 1);
    assert_eq!(replaced.partial_base_uploads, same.partial_base_uploads);
    assert_eq!(replaced.texture_creates, same.texture_creates);
    assert_eq!(replaced.mip_generations, same.mip_generations + 1);
    assert_eq!(replaced.base_upload_bytes, same.base_upload_bytes + 4 * 4 * 4);

    assets.insert("tex", Texture::solid(2, 2, [9, 8, 8, 255]).with_mipmaps(Mipmaps::Generate)).unwrap();
    render(&gpu, &mut renderer, &mut world, &assets);
    let resized = renderer.resource_stats();
    assert_eq!(renderer.cached_texture(handle.id()).unwrap().width, 2);
    assert_eq!(resized.texture_creates, replaced.texture_creates + 1);
    assert_eq!(resized.full_base_uploads, replaced.full_base_uploads + 1);

    assets
        .insert(
            "tex",
            Texture::solid(2, 2, [9, 8, 8, 255]).with_color_space(ColorSpace::Linear).with_mipmaps(Mipmaps::Generate),
        )
        .unwrap();
    render(&gpu, &mut renderer, &mut world, &assets);
    let recolored = renderer.resource_stats();
    assert_eq!(renderer.cached_texture(handle.id()).unwrap().color_space, ColorSpace::Linear);
    assert_eq!(recolored.texture_creates, resized.texture_creates + 1);

    assets.insert("tex", Texture::solid(2, 2, [9, 8, 8, 255]).with_color_space(ColorSpace::Linear)).unwrap();
    render(&gpu, &mut renderer, &mut world, &assets);
    let one_level = renderer.resource_stats();
    assert_eq!(renderer.cached_texture(handle.id()).unwrap().mip_level_count, 1);
    assert_eq!(one_level.texture_creates, recolored.texture_creates + 1);
    assert_eq!(one_level.mip_generations, recolored.mip_generations);

    publish_tex(&mut assets, |texture| {
        texture.rgba[0] = 3;
    });
    render(&gpu, &mut renderer, &mut world, &assets);
    let mutated = renderer.resource_stats();
    assert_eq!(mutated.full_base_uploads, one_level.full_base_uploads + 1);
    assert_eq!(mutated.partial_base_uploads, one_level.partial_base_uploads);

    let mut invalid = assets.get(handle).unwrap().clone();
    let error = invalid.update_region(0, 0, 0, 1, &[]).unwrap_err();
    assert_eq!(error, TextureRegionError::ZeroWidth);
    let held = renderer.resource_stats();
    render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(renderer.resource_stats(), held);

    publish_tex(&mut assets, |texture| {
        texture.rgba.pop();
    });
    render(&gpu, &mut renderer, &mut world, &assets);
    let refused = renderer.resource_stats();
    assert_eq!(refused.texture_writes, mutated.texture_writes);
    assert_eq!(refused.texture_creates, mutated.texture_creates);
    assert_eq!(refused.full_base_uploads, mutated.full_base_uploads);
    render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(renderer.resource_stats().pixel_fingerprints, refused.pixel_fingerprints);

    assets.remove(handle);
    let mut empty = World::new();
    empty.spawn((Camera::orthographic(2.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -4.0)));
    render(&gpu, &mut renderer, &mut empty, &assets);
    assert!(renderer.cached_texture(handle.id()).is_none());
    assert_eq!(renderer.cache_stats().textures, 0);

    renderer.set_texture_dimension_limits(Some(TextureDimensionLimits { max_width: 4, max_height: 4 }));
    let creates = renderer.resource_stats().texture_creates;
    let wide = assets.insert("wide", Texture::solid(8, 4, [4, 5, 6, 255])).unwrap();
    assets.insert("sprite", SpriteAsset::whole_texture(wide, 8, 4).with_pixels_per_unit(1.0)).unwrap();
    render(&gpu, &mut renderer, &mut world, &assets);
    render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(renderer.resource_stats().texture_creates, creates);
    assert!(renderer.cached_texture(wide.id()).is_none());
    assert_eq!(px(&render(&gpu, &mut renderer, &mut world, &assets), W / 2, H / 2), [255, 0, 255, 255]);
    let warnings = capture::messages().into_iter().filter(|message| message.contains("wide")).collect::<Vec<_>>();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("was not uploaded"), "{warnings:?}");
    assert!(warnings[0].contains("exceeds"), "{warnings:?}");
}

#[test]
fn a_missing_sprite_texture_stays_magenta_and_warns_once() {
    let Some((gpu, mut renderer)) = setup() else { return };
    capture::begin();
    let mut assets = AssetManager::in_memory();
    let missing = AssetHandle::<Texture>::from_name("no-such-texture");
    assets.insert("hero", SpriteAsset::whole_texture(missing, 4, 4).with_pixels_per_unit(1.0)).unwrap();
    let mut world = world_with_camera();
    world.insert_resource(MissingAssetWarnings::default());
    let sprite = Sprite::new("hero").with_size(Vec2::splat(12.0)).with_anchor(Vec2::splat(0.5)).unlit().cutout();
    world.spawn((Transform::from_xyz(0.0, 0.0, 10.0), sprite));
    world.spawn((Transform::from_xyz(0.2, 0.0, 10.0), sprite));
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(px(&image, W / 2, H / 2), [255, 0, 255, 255]);
    render(&gpu, &mut renderer, &mut world, &assets);
    let records = capture::records();
    let warnings =
        records.iter().filter(|record| record.message.contains("missing sprite texture")).collect::<Vec<_>>();
    let text = records.iter().map(|record| record.message.as_str()).collect::<Vec<_>>();
    assert_eq!(warnings.len(), 1, "{text:?}");
    assert_eq!(warnings[0].level, log::Level::Warn);
    assert!(warnings[0].message.contains(&format!("{:016x}", missing.id().0)), "{text:?}");
    assert!(warnings[0].message.contains("referenced by sprite 'hero'"), "{text:?}");
    assert!(!warnings[0].message.contains("no-such-texture"), "{text:?}");
}

#[test]
fn a_missing_mesh_texture_stays_white_and_warns_once() {
    let Some((gpu, mut renderer)) = setup() else { return };
    capture::begin();
    let mut assets = AssetManager::in_memory();
    assets.insert("cube", MeshData::cuboid(Vec3::splat(2.0))).unwrap();
    let missing = AssetHandle::<Texture>::from_name("no-mesh-texture");
    let mut world = world_with_camera();
    world.insert_resource(MissingAssetWarnings::default());
    world.spawn((
        Transform::IDENTITY,
        Mesh::new("cube").with_material(Material { base_color: Color::WHITE, base_texture: Some(missing), lit: false }),
    ));
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(px(&image, W / 2, H / 2), [255, 255, 255, 255]);
    render(&gpu, &mut renderer, &mut world, &assets);
    let messages = capture::messages();
    let warnings = messages.iter().filter(|message| message.contains("missing mesh texture")).collect::<Vec<_>>();
    assert_eq!(warnings.len(), 1, "{messages:?}");
    assert!(warnings[0].contains(&format!("{:016x}", missing.id().0)), "{warnings:?}");
    assert!(warnings[0].contains("referenced by mesh 'cube'"), "{warnings:?}");
    assert!(!warnings[0].contains("no-mesh-texture"), "{warnings:?}");
}
