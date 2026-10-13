//! Renders small scenes offscreen and inspects pixels.
//!
//! When the machine has no usable GPU adapter these tests skip with a message (and still report `ok`),
//! unless `DIMORA_REQUIRE_GPU=1` is set, in which case a missing adapter fails them. `scripts/check.sh`
//! sets the variable. The skip-or-fail decision is [`on_gpu_unavailable`], which is a pure function so it
//! can be tested without removing the GPU.

use engine_assets::{AssetHandle, AssetManager, MeshData, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Quat, Transform, Vec2, Vec3};
use engine_renderer::*;
use engine_renderer_wgpu::{GpuContext, RenderError, SceneRenderer};
use engine_world::{propagate_transforms, World};

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
            eprintln!("GPU test adapter: {} ({:?}, {:?})", info.name, info.backend, info.device_type);
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
    let white = a.insert("white", Texture::solid(4, 4, [255; 4]));
    a.insert("quad", SpriteAsset::whole_texture(white, 4, 4).with_pixels_per_unit(1.0));
    a.insert("blue", Texture::solid(4, 4, [0, 0, 255, 255]));
    a.insert("cube", MeshData::cuboid(Vec3::splat(2.0)));
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
        a.insert("white", Texture::solid(4, 4, colour));
        let p = px(&render(&gpu, &mut r, &mut w, &a), W / 2, H / 2);
        assert_eq!(p, colour, "replacement {i} is what is drawn");
        assert_eq!(r.cache_stats().textures, 1, "replacement {i} did not add a second copy");
    }

    // Remove and re-insert under the same name with no render in between: the new value must still win.
    a.remove(a.texture("white"));
    a.insert("white", Texture::solid(4, 4, [255, 255, 0, 255]));
    let p = px(&render(&gpu, &mut r, &mut w, &a), W / 2, H / 2);
    assert_eq!(p, [255, 255, 0, 255], "remove + insert between frames is seen as a change");
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
