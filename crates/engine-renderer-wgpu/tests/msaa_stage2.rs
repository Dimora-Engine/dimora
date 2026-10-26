//! Stage-2 4× coverage: Load expansion, nearest-sample depth, offscreen targets, and internal resolution.
//!
//! The depth diagnostic shows the nearest reverse-Z sample (the maximum stored sample), not an
//! average and not a coverage fraction. A public target and an internal image stay one sample.
//! The present blit samples that resolved image.

use engine_assets::{
    AlphaMode, AssetManager, ColorSpace, MaterialAsset, MeshData, SamplerDesc, SpriteAsset, Texture,
    TextureDimensionLimits, TextureFilter, Vertex,
};
use engine_core::Color;
use engine_math::{Mat4, Transform, Vec2, Vec3, Vec4};
use engine_renderer::*;
use engine_renderer_wgpu::{GpuContext, MsaaImageUsage, SceneRenderer};
use engine_world::{propagate_transforms, World};

mod capture {
    #![allow(dead_code)]
    include!("../../log_capture.rs");
}

fn gpu() -> Option<GpuContext> {
    match GpuContext::headless() {
        Ok(gpu) => Some(gpu),
        Err(error) => {
            if std::env::var("DIMORA_REQUIRE_GPU").ok().as_deref() == Some("1") {
                panic!("DIMORA_REQUIRE_GPU=1 but the GPU test adapter is unavailable: {error}");
            }
            eprintln!("skipping GPU test: {error}");
            None
        }
    }
}

fn linear_renderer(gpu: &GpuContext) -> SceneRenderer {
    SceneRenderer::new(gpu, wgpu::TextureFormat::Rgba8Unorm)
}

fn srgb_renderer(gpu: &GpuContext) -> SceneRenderer {
    SceneRenderer::new(gpu, wgpu::TextureFormat::Rgba8UnormSrgb)
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

fn color_bytes(color: [f32; 4]) -> [u8; 4] {
    color.map(|channel| (channel * 255.0).round() as u8)
}

fn draw(
    gpu: &GpuContext,
    renderer: &mut SceneRenderer,
    world: &mut World,
    assets: &AssetManager,
    width: u32,
    height: u32,
) -> Vec<u8> {
    propagate_transforms(world);
    let scene = extract_scene(world, assets, Vec2::new(width as f32, height as f32)).unwrap();
    renderer.render_to_rgba(gpu, &scene, assets, width, height).unwrap()
}

fn scene_of(
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

fn world_with(camera: Camera) -> World {
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.insert_resource(DebugDraw::default());
    world.spawn((camera, Transform::from_xyz(0.0, 0.0, -4.0)));
    world
}

fn pose() -> Transform {
    Transform::from_xyz(0.0, 0.0, -4.0)
}

fn solid_sprite(assets: &mut AssetManager, name: &str, rgba: [u8; 4]) -> Sprite {
    let texture = assets.insert(name, Texture::solid(1, 1, rgba).with_color_space(ColorSpace::Linear)).unwrap();
    let sprite_name = format!("{name}-sprite");
    assets.insert(&sprite_name, SpriteAsset::whole_texture(texture, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    Sprite::new(&sprite_name).with_anchor(Vec2::splat(0.5)).unlit()
}

fn diagonal_mesh() -> MeshData {
    let normal = Vec3::new(0.0, 0.0, -1.0);
    MeshData::new(
        vec![
            Vertex::new(Vec3::new(-1.0, -1.0, 0.0), normal, [0.0, 1.0]),
            Vertex::new(Vec3::new(0.2, -1.0, 0.0), normal, [1.0, 1.0]),
            Vertex::new(Vec3::new(-1.0, 1.0, 0.0), normal, [0.0, 0.0]),
        ],
        vec![0, 1, 2],
    )
}

fn card(z: f32) -> MeshData {
    let normal = Vec3::new(0.0, 0.0, -1.0);
    MeshData::new(
        vec![
            Vertex::new(Vec3::new(-0.7, -0.7, z), normal, [0.0, 1.0]),
            Vertex::new(Vec3::new(0.7, -0.7, z), normal, [1.0, 1.0]),
            Vertex::new(Vec3::new(0.7, 0.7, z), normal, [1.0, 0.0]),
            Vertex::new(Vec3::new(-0.7, 0.7, z), normal, [0.0, 0.0]),
        ],
        vec![0, 1, 2, 0, 2, 3],
    )
}

fn four(gpu: &GpuContext) -> SceneRenderer {
    let mut renderer = linear_renderer(gpu);
    renderer.set_msaa(gpu, MsaaSamples::Four).unwrap();
    renderer
}

#[test]
fn a_later_load_keeps_uncovered_pixels_and_replaces_covered_ones() {
    let Some(gpu) = gpu() else { return };
    let mut assets = AssetManager::in_memory();
    let blue = solid_sprite(&mut assets, "blue", [0, 0, 255, 255]).with_size(Vec2::splat(1.0));
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.spawn((Camera::orthographic(2.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)), pose()));
    world.spawn((
        Camera {
            order: 1,
            ..Camera::orthographic(2.0).with_clear(CameraClear::Load).with_clear_color(Color::rgb(0.0, 1.0, 0.0))
        },
        pose(),
    ));
    world.spawn((Transform::IDENTITY, blue));
    let mut renderer = four(&gpu);
    let image = draw(&gpu, &mut renderer, &mut world, &assets, 32, 32);
    expect_px(&image, 32, 1, 1, [255, 0, 0, 255], 0, "corner outside the blue sprite stays red");
    expect_px(&image, 32, 16, 16, [0, 0, 255, 255], 0, "opaque blue replaces red");
    expect_px(&image, 32, 1, 1, [255, 0, 0, 255], 0, "destination alpha stays opaque");
    assert_eq!(renderer.stats().expand_draws, 1);
    assert_eq!(renderer.stats().resolve_draws, 2);
    assert_eq!(renderer.stats().msaa_skipped, 0);
}

#[test]
fn the_first_load_camera_still_clears_the_destination() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = four(&gpu);
    let assets = AssetManager::in_memory();
    let mut world =
        world_with(Camera::orthographic(2.0).with_clear(CameraClear::Load).with_clear_color(Color::rgb(1.0, 0.0, 0.0)));
    let image = draw(&gpu, &mut renderer, &mut world, &assets, 8, 8);
    assert!(image.chunks(4).all(|pixel| pixel == [255, 0, 0, 255]));
    assert_eq!(renderer.stats().expand_draws, 0);
}

#[test]
fn load_blend_matches_one_sample_where_the_sprite_covers() {
    let Some(gpu) = gpu() else { return };
    let mut assets = AssetManager::in_memory();
    let texture =
        assets.insert("tex", Texture::solid(1, 1, [0, 0, 255, 128]).with_color_space(ColorSpace::Linear)).unwrap();
    assets.insert("sprite", SpriteAsset::whole_texture(texture, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    let blend = assets.insert("blend", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::Blend)).unwrap();
    let sprite = Sprite::new("sprite").with_size(Vec2::splat(1.2)).with_anchor(Vec2::splat(0.5)).with_material(blend);
    let build = |samples| {
        let mut renderer = linear_renderer(&gpu);
        renderer.set_msaa(&gpu, samples).unwrap();
        let mut world = World::new();
        world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
        world.spawn((Camera::orthographic(2.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)), pose()));
        world.spawn((Camera { order: 1, ..Camera::orthographic(2.0).with_clear(CameraClear::Load) }, pose()));
        world.spawn((Transform::IDENTITY, sprite));
        let image = draw(&gpu, &mut renderer, &mut world, &assets, 24, 24);
        (renderer, image)
    };
    let (mut low_renderer, low) = build(MsaaSamples::One);
    let _ = &mut low_renderer;
    let (high_renderer, high) = build(MsaaSamples::Four);
    expect_px(&high, 24, 0, 0, [255, 0, 0, 255], 0, "uncovered red and alpha");
    expect_px(&high, 24, 12, 12, px(&low, 24, 12, 12), 1, "covered blend matches 1×");
    assert_eq!(px(&high, 24, 0, 0)[3], 255);
    assert_eq!(high_renderer.stats().expand_draws, 1);
    assert_eq!(low_renderer.stats().expand_draws, 0);
}

#[test]
fn load_on_srgb_matches_one_sample_for_a_fully_covered_pixel() {
    let Some(gpu) = gpu() else { return };
    let mut assets = AssetManager::in_memory();
    let sprite = solid_sprite(&mut assets, "blue", [0, 0, 255, 255]).with_size(Vec2::splat(4.0));
    let build = |samples| {
        let mut renderer = srgb_renderer(&gpu);
        renderer.set_msaa(&gpu, samples).unwrap();
        let mut world = World::new();
        world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
        world.spawn((Camera::orthographic(2.0).with_clear_color(Color::rgb(0.4, 0.05, 0.02)), pose()));
        world.spawn((Camera { order: 1, ..Camera::orthographic(2.0).with_clear(CameraClear::Load) }, pose()));
        world.spawn((Transform::IDENTITY, sprite));
        draw(&gpu, &mut renderer, &mut world, &assets, 16, 16)
    };
    let low = build(MsaaSamples::One);
    let high = build(MsaaSamples::Four);
    expect_px(&high, 16, 8, 8, px(&low, 16, 8, 8), 1, "sRGB covered pixel");
    expect_px(&high, 16, 0, 0, px(&low, 16, 0, 0), 1, "sRGB expanded clear");
}

#[test]
fn a_small_load_viewport_and_a_third_camera_leave_the_rest() {
    let Some(gpu) = gpu() else { return };
    let mut assets = AssetManager::in_memory();
    let blue = solid_sprite(&mut assets, "blue", [0, 0, 255, 255]).with_size(Vec2::splat(1.0));
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.spawn((Camera::orthographic(2.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)), pose()));
    world.spawn((
        Camera {
            order: 1,
            ..Camera::orthographic(2.0)
                .with_clear(CameraClear::Load)
                .with_viewport(ViewportRect::new(0.25, 0.25, 0.25, 0.25))
        },
        pose(),
    ));
    world.spawn((
        Camera {
            order: 2,
            ..Camera::orthographic(2.0)
                .with_clear_color(Color::rgb(0.0, 1.0, 0.0))
                .with_viewport(ViewportRect::new(0.0, 0.0, 0.15, 0.15))
        },
        Transform::from_xyz(30.0, 0.0, -4.0),
    ));
    world.spawn((Transform::IDENTITY, blue));
    let mut renderer = four(&gpu);
    let image = draw(&gpu, &mut renderer, &mut world, &assets, 32, 32);
    expect_px(&image, 32, 31, 31, [255, 0, 0, 255], 0, "outside both later viewports");
    expect_px(&image, 32, 12, 12, [0, 0, 255, 255], 1, "small load viewport");
    expect_px(&image, 32, 1, 1, [0, 255, 0, 255], 0, "third camera after load");
    assert_eq!(renderer.stats().expand_draws, 1);
}

#[test]
fn overlapping_load_viewports_keep_pixels_outside_the_later_one() {
    let Some(gpu) = gpu() else { return };
    let mut assets = AssetManager::in_memory();
    let blue = solid_sprite(&mut assets, "blue", [0, 0, 255, 255]).with_size(Vec2::splat(0.6));
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.spawn((
        Camera::orthographic(2.0)
            .with_clear_color(Color::rgb(1.0, 0.0, 0.0))
            .with_viewport(ViewportRect::new(0.0, 0.0, 0.75, 1.0)),
        pose(),
    ));
    world.spawn((
        Camera {
            order: 1,
            ..Camera::orthographic(2.0)
                .with_clear(CameraClear::Load)
                .with_viewport(ViewportRect::new(0.25, 0.0, 0.75, 1.0))
        },
        Transform::from_xyz(0.8, 0.0, -4.0),
    ));
    world.spawn((Transform::from_xyz(0.8, 0.0, 0.0), blue));
    let mut renderer = four(&gpu);
    let image = draw(&gpu, &mut renderer, &mut world, &assets, 32, 16);
    expect_px(&image, 32, 1, 8, [255, 0, 0, 255], 0, "left of the overlap stays red");
    expect_px(&image, 32, 20, 8, [0, 0, 255, 255], 1, "overlap covered by the load sprite");
    assert_eq!(renderer.stats().msaa_skipped, 0);
}

#[test]
fn depth_shows_the_nearest_sample_including_partial_coverage() {
    let Some(gpu) = gpu() else { return };
    let mut assets = AssetManager::in_memory();
    assets.insert("tri", diagonal_mesh()).unwrap();
    assets.insert("far", card(1.2)).unwrap();
    let mut world = world_with(
        Camera::orthographic(2.0).with_clear_color(Color::BLACK).with_presentation(CameraPresentation::Depth),
    );
    world.spawn((Transform::IDENTITY, Mesh::new("far").with_material(Material::unlit(Color::WHITE))));
    world.spawn((Transform::IDENTITY, Mesh::new("tri").with_material(Material::unlit(Color::WHITE))));
    let mut shaded = four(&gpu);
    let camera = world.query::<&Camera>().iter().next().unwrap().0;
    world.component_mut::<Camera>(camera).unwrap().presentation = CameraPresentation::Shaded;
    let color = draw(&gpu, &mut shaded, &mut world, &assets, 48, 48);
    let partial = color
        .chunks(4)
        .enumerate()
        .find(|(_, pixel)| pixel[0] > 8 && pixel[0] < 247 && pixel[1] == pixel[0] && pixel[3] == 255)
        .map(|(index, _)| index);
    let Some(partial) = partial else {
        panic!("expected a partial-coverage edge");
    };
    let (px_x, px_y) = ((partial as u32) % 48, (partial as u32) / 48);
    world.component_mut::<Camera>(camera).unwrap().presentation = CameraPresentation::Depth;
    let mut renderer = four(&gpu);
    let depth = draw(&gpu, &mut renderer, &mut world, &assets, 48, 48);
    let edge = px(&depth, 48, px_x, px_y);
    let empty = color_bytes(DEPTH_VIEW_EMPTY);
    assert!(edge != empty, "partial coverage must show the covered sample, got {edge:?}");
    assert!(edge[0].abs_diff(edge[1]) <= 2 && edge[3] == 255, "nearest sample is gray, got {edge:?}");
    let covered = color.chunks(4).enumerate().find(|(_, pixel)| *pixel == [255, 255, 255, 255]).map(|(index, _)| {
        let index = index as u32;
        (index % 48, index / 48)
    });
    let Some((covered_x, covered_y)) = covered else {
        panic!("expected a fully covered pixel");
    };
    let interior = px(&depth, 48, covered_x, covered_y);
    assert!(
        edge[0].abs_diff(interior[0]) <= 20,
        "edge {edge:?} should match a covered sample {interior:?}, not an average toward clear"
    );
    assert_eq!(renderer.stats().depth_view_draws, 1);
    assert_eq!(renderer.stats().resolve_draws, 0);
}

#[test]
fn depth_empty_near_far_and_reverse_z_order() {
    let Some(gpu) = gpu() else { return };
    let mut assets = AssetManager::in_memory();
    assets.insert("near", card(0.0)).unwrap();
    assets.insert("far", card(0.0)).unwrap();
    let mut empty_world = world_with(
        Camera::orthographic(2.0).with_clear_color(Color::BLACK).with_presentation(CameraPresentation::Depth),
    );
    let mut renderer = four(&gpu);
    let image = draw(&gpu, &mut renderer, &mut empty_world, &assets, 16, 16);
    expect_px(&image, 16, 8, 8, color_bytes(DEPTH_VIEW_EMPTY), 3, "empty background");

    let mut depth_camera =
        Camera::orthographic(2.0).with_clear_color(Color::BLACK).with_presentation(CameraPresentation::Depth);
    depth_camera.projection = depth_camera.projection.with_clip_range(0.5, 20.0);
    let mut world = world_with(depth_camera);
    world.spawn((Transform::from_xyz(-0.8, 0.0, 0.0), Mesh::new("near").with_material(Material::unlit(Color::WHITE))));
    world.spawn((Transform::from_xyz(0.8, 0.0, 14.0), Mesh::new("far").with_material(Material::unlit(Color::WHITE))));
    let image = draw(&gpu, &mut renderer, &mut world, &assets, 32, 16);
    let near = px(&image, 32, 8, 8);
    let far = px(&image, 32, 24, 8);
    assert!(near[0] > far[0].saturating_add(20), "near {near:?} is brighter than far {far:?}");
    assert!(near[0].abs_diff(near[1]) <= 2 && far[0].abs_diff(far[1]) <= 2);

    let mut perspective = world_with(
        Camera::perspective(50.0).with_clear_color(Color::BLACK).with_presentation(CameraPresentation::Depth),
    );
    perspective
        .spawn((Transform::from_xyz(0.0, 0.0, 0.0), Mesh::new("near").with_material(Material::unlit(Color::WHITE))));
    let image = draw(&gpu, &mut renderer, &mut perspective, &assets, 16, 16);
    let center = px(&image, 16, 8, 8);
    assert!(center != color_bytes(DEPTH_VIEW_EMPTY) && center[0].abs_diff(center[1]) <= 2, "{center:?}");
}

#[test]
fn depth_sprite_in_front_of_a_mesh_is_nearer() {
    let Some(gpu) = gpu() else { return };
    let mut assets = AssetManager::in_memory();
    assets.insert("card", card(0.0)).unwrap();
    let sprite = solid_sprite(&mut assets, "white", [255, 255, 255, 255]).with_size(Vec2::splat(0.6));
    let mut world = world_with(
        Camera::orthographic(2.0).with_clear_color(Color::BLACK).with_presentation(CameraPresentation::Depth),
    );
    world.spawn((Transform::from_xyz(0.0, 0.0, 1.2), Mesh::new("card").with_material(Material::unlit(Color::WHITE))));
    world.spawn((Transform::IDENTITY, sprite));
    let mut renderer = four(&gpu);
    let image = draw(&gpu, &mut renderer, &mut world, &assets, 32, 32);
    let sprite_px = px(&image, 32, 16, 16);
    let mesh_px = px(&image, 32, 4, 4);
    assert!(sprite_px[0] > mesh_px[0].saturating_add(10), "sprite {sprite_px:?} nearer than mesh {mesh_px:?}");
}

#[test]
fn a_later_depth_viewport_clears_its_own_depth() {
    let Some(gpu) = gpu() else { return };
    let mut assets = AssetManager::in_memory();
    assets.insert("card", card(0.0)).unwrap();
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.spawn((
        Camera::orthographic(2.0).with_clear_color(Color::BLACK).with_presentation(CameraPresentation::Depth),
        pose(),
    ));
    world.spawn((
        Camera {
            order: 1,
            ..Camera::orthographic(2.0)
                .with_presentation(CameraPresentation::Depth)
                .with_clear_color(Color::BLACK)
                .with_viewport(ViewportRect::new(0.5, 0.0, 0.5, 1.0))
        },
        Transform::from_xyz(30.0, 0.0, -4.0),
    ));
    world.spawn((Transform::IDENTITY, Mesh::new("card").with_material(Material::unlit(Color::WHITE))));
    let mut renderer = four(&gpu);
    let image = draw(&gpu, &mut renderer, &mut world, &assets, 32, 16);
    let left = px(&image, 32, 14, 8);
    expect_px(&image, 32, 24, 8, color_bytes(DEPTH_VIEW_EMPTY), 3, "second camera cleared depth");
    assert!(left != color_bytes(DEPTH_VIEW_EMPTY) && left[0] > 40, "first viewport kept its surface, {left:?}");
    assert_eq!(renderer.stats().depth_view_draws, 2);
    assert_eq!(renderer.stats().resolve_draws, 0);
}

#[test]
fn an_unsupported_custom_projection_keeps_its_diagnostic_color() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = four(&gpu);
    let assets = AssetManager::in_memory();
    let mut world = world_with(
        Camera::new(CameraProjection::custom(Mat4::from_cols(Vec4::ZERO, Vec4::ZERO, Vec4::ZERO, Vec4::ZERO)).unwrap())
            .with_presentation(CameraPresentation::Depth)
            .with_clear_color(Color::BLACK),
    );
    let image = draw(&gpu, &mut renderer, &mut world, &assets, 8, 8);
    expect_px(&image, 8, 4, 4, color_bytes(DEPTH_VIEW_UNSUPPORTED), 3, "singular projection");
}

#[test]
fn an_offscreen_target_resolves_and_a_later_camera_samples_it() {
    let Some(gpu) = gpu() else { return };
    let mut assets = AssetManager::in_memory();
    assets.insert("tri", diagonal_mesh()).unwrap();
    let sprite = solid_sprite(&mut assets, "red", [255, 0, 0, 255]).with_size(Vec2::splat(0.5));
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.insert_resource(RenderTargetStore::new());
    let target = world.resource_mut::<RenderTargetStore>().create(32, 32).unwrap();
    world.spawn((
        Camera::orthographic(2.0).with_output(CameraOutput::Target(target)).with_clear_color(Color::BLACK),
        pose(),
    ));
    world.spawn((Transform::IDENTITY, Mesh::new("tri").with_material(Material::unlit(Color::WHITE))));
    world.spawn((Transform::from_xyz(0.0, 0.0, -0.2), sprite));
    let mut renderer = four(&gpu);
    let display_tex = assets.insert("display-tex", Texture::solid(1, 1, [255, 255, 255, 255])).unwrap();
    assets.insert("display", SpriteAsset::whole_texture(display_tex, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    world.spawn((
        Camera { order: 1, ..Camera::orthographic(2.0).with_clear_color(Color::rgb(0.0, 0.0, 1.0)) },
        Transform::from_xyz(40.0, 0.0, -4.0),
    ));
    world.spawn((
        Transform::from_xyz(40.0, 0.0, 0.0),
        Sprite::new("display")
            .with_size(Vec2::splat(2.0))
            .with_anchor(Vec2::splat(0.5))
            .unlit()
            .with_render_target(target),
    ));
    let image = draw(&gpu, &mut renderer, &mut world, &assets, 32, 32);
    let offscreen = renderer.read_render_target(&gpu, target).unwrap();
    let partial = offscreen.chunks(4).filter(|pixel| pixel[0] > 8 && pixel[0] < 247 && pixel[0] == pixel[1]).count();
    assert!(partial > 4, "resolved target keeps geometric coverage, counted {partial}");
    expect_px(&offscreen, 32, 16, 16, [255, 0, 0, 255], 1, "sprite on the target");
    expect_px(&image, 32, 16, 16, px(&offscreen, 32, 16, 16), 1, "later camera samples the resolved target");
    let shown = image.chunks(4).filter(|pixel| pixel[0] > 8 && pixel[0] < 247 && pixel[0] == pixel[1]).count();
    assert!(shown > 0, "sampled edges keep the resolved coverage");
    assert!(renderer.msaa_images().image_count >= 2);
    assert_eq!(renderer.stats().resolve_draws, 2);
}

#[test]
fn two_targets_and_a_later_load_do_not_alias() {
    let Some(gpu) = gpu() else { return };
    let mut assets = AssetManager::in_memory();
    let blue = solid_sprite(&mut assets, "blue", [0, 0, 255, 255]).with_size(Vec2::splat(0.8));
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.insert_resource(RenderTargetStore::new());
    let left = world.resource_mut::<RenderTargetStore>().create(16, 16).unwrap();
    let right = world.resource_mut::<RenderTargetStore>().create(16, 16).unwrap();
    world.spawn((
        Camera::orthographic(2.0).with_output(CameraOutput::Target(left)).with_clear_color(Color::rgb(1.0, 0.0, 0.0)),
        pose(),
    ));
    world.spawn((
        Camera {
            order: 1,
            ..Camera::orthographic(2.0)
                .with_output(CameraOutput::Target(right))
                .with_clear_color(Color::rgb(0.0, 1.0, 0.0))
        },
        pose(),
    ));
    world.spawn((
        Camera {
            order: 2,
            ..Camera::orthographic(2.0).with_output(CameraOutput::Target(left)).with_clear(CameraClear::Load)
        },
        pose(),
    ));
    world.spawn((Transform::IDENTITY, blue));
    let mut renderer = four(&gpu);
    let _ = draw(&gpu, &mut renderer, &mut world, &assets, 8, 8);
    let left_image = renderer.read_render_target(&gpu, left).unwrap();
    let right_image = renderer.read_render_target(&gpu, right).unwrap();
    expect_px(&left_image, 16, 8, 8, [0, 0, 255, 255], 0, "load on the first target");
    expect_px(&left_image, 16, 0, 0, [255, 0, 0, 255], 0, "uncovered red on the first target");
    expect_px(&right_image, 16, 0, 0, [0, 255, 0, 255], 0, "second target clear is not the first target's red");
    expect_px(&right_image, 16, 8, 8, [0, 0, 255, 255], 0, "second target draws the sprite on its own image");
    assert_eq!(renderer.stats().expand_draws, 1);
    let before = renderer.resource_stats().msaa_image_creates;
    let _ = draw(&gpu, &mut renderer, &mut world, &assets, 8, 8);
    assert_eq!(renderer.resource_stats().msaa_image_creates, before, "steady frames reuse target images");

    world.resource_mut::<RenderTargetStore>().resize(right, 24, 16).unwrap();
    let _ = draw(&gpu, &mut renderer, &mut world, &assets, 8, 8);
    let resized = renderer.read_render_target(&gpu, right).unwrap();
    assert_eq!(resized.len(), 24 * 16 * 4);
    expect_px(&resized, 24, 20, 8, [0, 255, 0, 255], 0, "resized target is the new clear, not stale red");

    let drops = renderer.resource_stats().msaa_image_drops;
    world.resource_mut::<RenderTargetStore>().remove(right);
    let _ = draw(&gpu, &mut renderer, &mut world, &assets, 8, 8);
    assert!(renderer.resource_stats().msaa_image_drops > drops);
    assert!(renderer.read_render_target(&gpu, right).is_none());
    expect_px(&renderer.read_render_target(&gpu, left).unwrap(), 16, 0, 0, [255, 0, 0, 255], 0, "remaining target");
}

#[test]
fn switching_sample_counts_releases_images_and_restores_one_sample_pixels() {
    let Some(gpu) = gpu() else { return };
    let mut assets = AssetManager::in_memory();
    assets.insert("tri", diagonal_mesh()).unwrap();
    let mut world = world_with(Camera::orthographic(2.0).with_clear_color(Color::BLACK));
    world.spawn((Transform::IDENTITY, Mesh::new("tri").with_material(Material::unlit(Color::WHITE))));
    let mut one = linear_renderer(&gpu);
    let low = draw(&gpu, &mut one, &mut world, &assets, 32, 32);
    let mut renderer = four(&gpu);
    let _high = draw(&gpu, &mut renderer, &mut world, &assets, 32, 32);
    assert!(renderer.msaa_images().image_count >= 1);
    renderer.set_msaa(&gpu, MsaaSamples::One).unwrap();
    assert_eq!(renderer.msaa_images(), MsaaImageUsage::default());
    let again = draw(&gpu, &mut renderer, &mut world, &assets, 32, 32);
    assert_eq!(again, low, "1× after 4× matches a renderer that never left 1×");
    renderer.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    let high = draw(&gpu, &mut renderer, &mut world, &assets, 32, 32);
    assert_ne!(high, low);
    assert_eq!(renderer.resource_stats().msaa_pipeline_creates, 1);
}

#[test]
fn reordering_cameras_does_not_keep_a_stale_clear() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = four(&gpu);
    let assets = AssetManager::in_memory();
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    let left = world.spawn((
        Camera::orthographic(2.0)
            .with_clear_color(Color::rgb(1.0, 0.0, 0.0))
            .with_viewport(ViewportRect::new(0.0, 0.0, 0.5, 1.0)),
        pose(),
    ));
    let right = world.spawn((
        Camera {
            order: 1,
            ..Camera::orthographic(2.0)
                .with_clear_color(Color::rgb(0.0, 0.0, 1.0))
                .with_viewport(ViewportRect::new(0.5, 0.0, 0.5, 1.0))
        },
        pose(),
    ));
    let image = draw(&gpu, &mut renderer, &mut world, &assets, 16, 8);
    expect_px(&image, 16, 2, 4, [255, 0, 0, 255], 0, "left");
    expect_px(&image, 16, 12, 4, [0, 0, 255, 255], 0, "right");
    world.component_mut::<Camera>(left).unwrap().order = 2;
    let _ = right;
    let image = draw(&gpu, &mut renderer, &mut world, &assets, 16, 8);
    expect_px(&image, 16, 2, 4, [255, 0, 0, 255], 0, "later left camera");
    expect_px(&image, 16, 12, 4, [0, 0, 255, 255], 0, "earlier right clear remains outside the left viewport");
}

#[test]
fn internal_resolution_resolves_before_the_integer_blit() {
    let Some(gpu) = gpu() else { return };
    let mut assets = AssetManager::in_memory();
    assets.insert("tri", diagonal_mesh()).unwrap();
    let mut world =
        world_with(Camera::orthographic(2.0).with_clear_color(Color::BLACK).with_internal_resolution(320, 180));
    world.spawn((Transform::IDENTITY, Mesh::new("tri").with_material(Material::unlit(Color::WHITE))));
    let mut one = linear_renderer(&gpu);
    let mut four = four(&gpu);
    let (low, low_scene) = scene_of(&gpu, &mut one, &mut world, &assets, 1280, 720);
    let (high, high_scene) = scene_of(&gpu, &mut four, &mut world, &assets, 1280, 720);
    let low_internal = one.read_internal(&gpu, low_scene.passes[0].camera_id).unwrap();
    let high_internal = four.read_internal(&gpu, high_scene.passes[0].camera_id).unwrap();
    assert_eq!(high_internal.len(), 320 * 180 * 4);
    let low_partial = low_internal.chunks(4).filter(|pixel| pixel[0] > 8 && pixel[0] < 247).count();
    let high_partial = high_internal.chunks(4).filter(|pixel| pixel[0] > 8 && pixel[0] < 247).count();
    assert_eq!(low_partial, 0, "1× internal edges stay binary");
    assert!(high_partial > 8, "4× coverage is in the resolved internal image, counted {high_partial}");
    let sample = high_internal
        .chunks(4)
        .enumerate()
        .find(|(_, pixel)| pixel[0] > 8 && pixel[0] < 247)
        .map(|(index, pixel)| (index, [pixel[0], pixel[1], pixel[2], pixel[3]]))
        .unwrap();
    let (index, color) = sample;
    let (ix, iy) = ((index as u32) % 320, (index as u32) / 320);
    for dy in 0..4 {
        for dx in 0..4 {
            expect_px(&high, 1280, ix * 4 + dx, iy * 4 + dy, color, 0, "integer block stays flat");
        }
    }
    assert_eq!(four.msaa_images().width, 0, "multisample storage is the internal size, not the window");
    assert_eq!(four.msaa_images().image_count, 1);
    assert_eq!(four.msaa_images().color_bytes, 320 * 180 * 16);
    assert_eq!(four.stats().presentation_draws, 1);
    assert_eq!(four.stats().resolve_draws, 1);
    assert_ne!(low, high);
}

#[test]
fn thirteen_sixty_six_keeps_letterbox_bars_at_four_samples() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = four(&gpu);
    let assets = AssetManager::in_memory();
    let mut world = world_with(
        Camera::orthographic(180.0)
            .with_clear_color(Color::rgb(1.0, 0.0, 0.0))
            .with_letterbox_color(Color::rgb(0.0, 1.0, 0.0))
            .with_internal_resolution(320, 180),
    );
    let image = draw(&gpu, &mut renderer, &mut world, &assets, 1366, 768);
    expect_px(&image, 1366, 0, 0, [0, 255, 0, 255], 2, "top-left bar");
    expect_px(&image, 1366, 43, 24, [255, 0, 0, 255], 2, "image origin");
    expect_px(&image, 1366, 46, 27, [255, 0, 0, 255], 0, "flat block inside the image");
    expect_px(&image, 1366, 1322, 743, [255, 0, 0, 255], 2, "image far corner");
    expect_px(&image, 1366, 1323, 100, [0, 255, 0, 255], 2, "right bar");
    assert_eq!(renderer.stats().presentation_draws, 1);
    assert_eq!(renderer.msaa_images().color_bytes, 320 * 180 * 16);
}

#[test]
fn internal_depth_lighting_and_blend_survive_presentation() {
    let Some(gpu) = gpu() else { return };
    let mut assets = AssetManager::in_memory();
    assets.insert("card", card(0.0)).unwrap();
    let texture =
        assets.insert("fade", Texture::solid(1, 1, [0, 255, 0, 128]).with_color_space(ColorSpace::Linear)).unwrap();
    assets.insert("fade-sprite", SpriteAsset::whole_texture(texture, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    let blend = assets.insert("blend", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::Blend)).unwrap();
    let mut depth_world = world_with(
        Camera::orthographic(2.0)
            .with_clear_color(Color::BLACK)
            .with_presentation(CameraPresentation::Depth)
            .with_internal_resolution(16, 16),
    );
    depth_world.spawn((Transform::IDENTITY, Mesh::new("card").with_material(Material::unlit(Color::WHITE))));
    let mut renderer = four(&gpu);
    let (image, scene) = scene_of(&gpu, &mut renderer, &mut depth_world, &assets, 64, 64);
    let internal = renderer.read_internal(&gpu, scene.passes[0].camera_id).unwrap();
    let center = px(&internal, 16, 8, 8);
    assert!(center != color_bytes(DEPTH_VIEW_EMPTY) && center[0].abs_diff(center[1]) <= 2, "{center:?}");
    expect_px(&image, 64, 32, 32, center, 0, "depth block matches the resolved internal pixel");
    assert_eq!(renderer.stats().resolve_draws, 0);
    assert_eq!(renderer.stats().depth_view_draws, 1);

    let mut lit = world_with(Camera::orthographic(2.0).with_clear_color(Color::BLACK).with_internal_resolution(16, 16));
    lit.spawn((Transform::from_xyz(0.0, 2.0, -2.0), Light::directional(Color::rgb(1.0, 0.2, 0.2), 3.0)));
    lit.spawn((Transform::IDENTITY, Mesh::new("card").with_material(Material::color(Color::WHITE))));
    let mut one = linear_renderer(&gpu);
    let low = draw(&gpu, &mut one, &mut lit, &assets, 64, 64);
    let high = draw(&gpu, &mut renderer, &mut lit, &assets, 64, 64);
    expect_px(&high, 64, 32, 32, px(&low, 64, 32, 32), 2, "lit internal pixel matches 1×");

    let mut blended = world_with(
        Camera::orthographic(2.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)).with_internal_resolution(16, 16),
    );
    blended.spawn((
        Transform::IDENTITY,
        Sprite::new("fade-sprite").with_size(Vec2::splat(4.0)).with_anchor(Vec2::splat(0.5)).with_material(blend),
    ));
    let low = draw(&gpu, &mut one, &mut blended, &assets, 64, 64);
    let high = draw(&gpu, &mut renderer, &mut blended, &assets, 64, 64);
    expect_px(&high, 64, 32, 32, px(&low, 64, 32, 32), 1, "blended internal pixel matches 1×");
}

#[test]
fn scale_zero_allocates_no_multisample_image() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = four(&gpu);
    let mut assets = AssetManager::in_memory();
    let sprite = solid_sprite(&mut assets, "white", [255, 255, 255, 255]).with_size(Vec2::splat(2.0));
    let mut world = world_with(
        Camera::orthographic(4.0)
            .with_clear_color(Color::rgb(1.0, 0.0, 0.0))
            .with_letterbox_color(Color::rgb(0.0, 0.0, 1.0))
            .with_internal_resolution(16, 16),
    );
    world.spawn((Transform::IDENTITY, sprite));
    let image = draw(&gpu, &mut renderer, &mut world, &assets, 8, 8);
    assert!(image.chunks(4).all(|pixel| pixel == [0, 0, 255, 255]));
    assert_eq!(renderer.stats().world_draw_calls, 0);
    assert_eq!(renderer.stats().resolve_draws, 0);
    assert_eq!(renderer.internal_targets().cameras, 0);
    assert_eq!(renderer.msaa_images(), MsaaImageUsage::default());
}

#[test]
fn resizing_the_internal_image_drops_the_old_multisample_pair() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = four(&gpu);
    let assets = AssetManager::in_memory();
    let mut world = world_with(
        Camera::orthographic(2.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)).with_internal_resolution(8, 8),
    );
    let _ = draw(&gpu, &mut renderer, &mut world, &assets, 32, 32);
    assert_eq!(renderer.msaa_images().color_bytes, 8 * 8 * 16);
    let camera = world.query::<&Camera>().iter().next().unwrap().0;
    world.component_mut::<Camera>(camera).unwrap().internal_resolution = Some((12, 8));
    world.component_mut::<Camera>(camera).unwrap().clear_color = Color::rgb(0.0, 0.0, 1.0);
    let (image, scene) = scene_of(&gpu, &mut renderer, &mut world, &assets, 48, 32);
    let internal = renderer.read_internal(&gpu, scene.passes[0].camera_id).unwrap();
    assert_eq!(internal.len(), 12 * 8 * 4);
    expect_px(&internal, 12, 1, 1, [0, 0, 255, 255], 0, "new internal clear");
    expect_px(&image, 48, 4, 4, [0, 0, 255, 255], 1, "presented block");
    assert_eq!(renderer.msaa_images().color_bytes, 12 * 8 * 16);
    assert!(renderer.resource_stats().msaa_image_drops >= 1);
}

#[test]
fn a_low_resolution_offscreen_target_can_be_internal() {
    let Some(gpu) = gpu() else { return };
    let mut assets = AssetManager::in_memory();
    assets.insert("tri", diagonal_mesh()).unwrap();
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.insert_resource(RenderTargetStore::new());
    let target = world.resource_mut::<RenderTargetStore>().create(64, 64).unwrap();
    world.spawn((
        Camera::orthographic(2.0)
            .with_output(CameraOutput::Target(target))
            .with_clear_color(Color::BLACK)
            .with_internal_resolution(16, 16),
        pose(),
    ));
    world.spawn((Transform::IDENTITY, Mesh::new("tri").with_material(Material::unlit(Color::WHITE))));
    let mut renderer = four(&gpu);
    let _ = draw(&gpu, &mut renderer, &mut world, &assets, 8, 8);
    let image = renderer.read_render_target(&gpu, target).unwrap();
    assert_eq!(image.len(), 64 * 64 * 4);
    assert!(image.chunks(4).any(|pixel| pixel[0] == 255), "the triangle survived the blit");
    assert_eq!(renderer.msaa_images().color_bytes, 16 * 16 * 16);
    assert_eq!(renderer.stats().presentation_draws, 1);
}

#[test]
fn combinations_of_surface_offscreen_scaled_and_load_stay_ordered() {
    let Some(gpu) = gpu() else { return };
    let mut assets = AssetManager::in_memory();
    let blue = solid_sprite(&mut assets, "blue", [0, 0, 255, 255]).with_size(Vec2::splat(0.7));
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.insert_resource(RenderTargetStore::new());
    let target = world.resource_mut::<RenderTargetStore>().create(16, 16).unwrap();
    world.spawn((
        Camera::orthographic(2.0)
            .with_clear_color(Color::rgb(1.0, 0.0, 0.0))
            .with_viewport(ViewportRect::new(0.0, 0.0, 1.0, 1.0))
            .with_internal_resolution(16, 8),
        pose(),
    ));
    world.spawn((
        Camera {
            order: 1,
            ..Camera::orthographic(2.0)
                .with_output(CameraOutput::Target(target))
                .with_clear_color(Color::rgb(0.0, 1.0, 0.0))
        },
        pose(),
    ));
    world.spawn((
        Camera {
            order: 2,
            ..Camera::orthographic(2.0)
                .with_clear(CameraClear::Load)
                .with_viewport(ViewportRect::new(0.5, 0.0, 0.5, 1.0))
        },
        pose(),
    ));
    world.spawn((
        Camera {
            order: 3,
            ..Camera::orthographic(2.0)
                .with_clear(CameraClear::Load)
                .with_internal_resolution(4, 4)
                .with_clear_color(Color::rgb(1.0, 1.0, 0.0))
        },
        pose(),
    ));
    world.spawn((Transform::from_xyz(2.0, 0.0, 0.0), blue.with_size(Vec2::splat(2.0))));
    let mut renderer = four(&gpu);
    let (image, scene) = scene_of(&gpu, &mut renderer, &mut world, &assets, 32, 16);
    assert!(scene.stats.presentation_skipped >= 1, "Load plus internal resolution stays rejected");
    expect_px(&image, 32, 8, 8, [255, 0, 0, 255], 2, "scaled camera before the direct load");
    expect_px(&image, 32, 24, 8, [0, 0, 255, 255], 1, "direct load after the scaled camera");
    expect_px(&image, 32, 20, 8, [255, 0, 0, 255], 2, "load leaves the scaled clear outside the sprite");
    let offscreen = renderer.read_render_target(&gpu, target).unwrap();
    expect_px(&offscreen, 16, 8, 8, [0, 255, 0, 255], 0, "offscreen camera in the same frame");
    assert_eq!(renderer.stats().msaa_skipped, 0);
    assert!(renderer.stats().expand_draws >= 1);
    assert!(renderer.stats().presentation_draws >= 1);
}

#[test]
fn a_skipped_load_camera_does_not_black_clear_a_failed_color_camera() {
    let Some(gpu) = gpu() else { return };
    capture::begin();
    let mut renderer = four(&gpu);
    renderer.set_texture_dimension_limits(Some(TextureDimensionLimits { max_width: 1, max_height: 1 }));
    let assets = AssetManager::in_memory();
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.spawn((Camera::orthographic(2.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)), pose()));
    world.spawn((Camera { order: 1, ..Camera::orthographic(2.0).with_clear(CameraClear::Load) }, pose()));
    let _ = draw(&gpu, &mut renderer, &mut world, &assets, 16, 16);
    assert_eq!(renderer.msaa(), MsaaSamples::Four);
    assert_eq!(renderer.stats().msaa_skipped, 2);
    assert_eq!(renderer.stats().world_draw_calls, 0);
    let messages = capture::messages();
    assert!(messages.iter().any(|message| message.contains("not replaced with 1×")));
    assert!(!messages.iter().any(|message| message.contains("opaque black")), "{messages:?}");
}

#[test]
fn materials_orientations_tiles_coplanar_and_filters_draw_at_four_samples() {
    let Some(gpu) = gpu() else { return };
    let mut assets = AssetManager::in_memory();
    let white = assets
        .insert("white", Texture::solid(2, 2, [255, 255, 255, 255]).with_color_space(ColorSpace::Linear))
        .unwrap();
    assets.insert("sprite", SpriteAsset::whole_texture(white, 2, 2).with_pixels_per_unit(1.0)).unwrap();
    let linear =
        SamplerDesc { min_filter: TextureFilter::Linear, mag_filter: TextureFilter::Linear, ..Default::default() };
    let filtered = assets
        .insert(
            "linear-tex",
            Texture::solid(2, 2, [0, 255, 0, 255]).with_color_space(ColorSpace::Linear).with_sampler(linear).unwrap(),
        )
        .unwrap();
    assets.insert("linear", SpriteAsset::whole_texture(filtered, 2, 2).with_pixels_per_unit(1.0)).unwrap();
    assets.insert("tiles", Texture::solid(1, 1, [0, 0, 255, 255]).with_color_space(ColorSpace::Linear)).unwrap();
    let mask = assets
        .insert("mask", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::mask(0.5).unwrap()))
        .unwrap();
    let mut renderer = four(&gpu);
    let mut one = linear_renderer(&gpu);

    for orientation in [SpriteOrientation::Billboard, SpriteOrientation::Upright, SpriteOrientation::Fixed] {
        let mut world = world_with(Camera::orthographic(2.0).with_clear_color(Color::BLACK));
        world.spawn((
            Transform::IDENTITY,
            Sprite::new("sprite")
                .with_size(Vec2::splat(1.5))
                .with_anchor(Vec2::splat(0.5))
                .with_orientation(orientation)
                .cutout()
                .unlit(),
        ));
        let image = draw(&gpu, &mut renderer, &mut world, &assets, 16, 16);
        assert!(image.chunks(4).any(|pixel| pixel[0] > 200), "orientation {orientation:?} is visible at 4×");
    }

    let mut masked = world_with(Camera::orthographic(2.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)));
    let clear_tex =
        assets.insert("clear-tex", Texture::solid(1, 1, [0, 255, 0, 0]).with_color_space(ColorSpace::Linear)).unwrap();
    assets.insert("clear-sprite", SpriteAsset::whole_texture(clear_tex, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    masked.spawn((
        Transform::IDENTITY,
        Sprite::new("clear-sprite").with_size(Vec2::splat(4.0)).with_anchor(Vec2::splat(0.5)).with_material(mask),
    ));
    let image = draw(&gpu, &mut renderer, &mut masked, &assets, 16, 16);
    expect_px(&image, 16, 8, 8, [255, 0, 0, 255], 1, "mask discard");

    let mut tiles = world_with(Camera::orthographic(4.0).with_clear_color(Color::BLACK));
    let mut map = TileMap::new("tiles", 1, 1, Vec2::splat(2.0), 2, 2).filled_with(|_, _| 0);
    map.lit = false;
    tiles.spawn((Transform::IDENTITY, map));
    let low = draw(&gpu, &mut one, &mut tiles, &assets, 16, 16);
    let high = draw(&gpu, &mut renderer, &mut tiles, &assets, 16, 16);
    expect_px(&high, 16, 8, 8, px(&low, 16, 8, 8), 1, "tile center matches 1×");

    let mut stacked = world_with(Camera::orthographic(2.0).with_clear_color(Color::BLACK));
    stacked.insert_resource(CoplanarSettings::new(0.4).unwrap());
    stacked.spawn((
        Transform::IDENTITY,
        Sprite::new("sprite")
            .with_size(Vec2::splat(1.5))
            .with_anchor(Vec2::splat(0.5))
            .cutout()
            .unlit()
            .with_tint(Color::rgb(1.0, 0.0, 0.0)),
        CoplanarPriority::new(3).unwrap(),
    ));
    stacked.spawn((
        Transform::IDENTITY,
        Sprite::new("sprite")
            .with_size(Vec2::splat(1.5))
            .with_anchor(Vec2::splat(0.5))
            .cutout()
            .unlit()
            .with_tint(Color::rgb(0.0, 0.0, 1.0)),
        CoplanarPriority::new(0).unwrap(),
    ));
    let image = draw(&gpu, &mut renderer, &mut stacked, &assets, 16, 16);
    expect_px(&image, 16, 8, 8, [255, 0, 0, 255], 1, "higher coplanar priority");

    let mut filtered_world = world_with(Camera::orthographic(2.0).with_clear_color(Color::BLACK));
    filtered_world.spawn((
        Transform::IDENTITY,
        Sprite::new("linear").with_size(Vec2::splat(1.5)).with_anchor(Vec2::splat(0.5)).cutout().unlit(),
    ));
    let low = draw(&gpu, &mut one, &mut filtered_world, &assets, 16, 16);
    let high = draw(&gpu, &mut renderer, &mut filtered_world, &assets, 16, 16);
    expect_px(&high, 16, 8, 8, px(&low, 16, 8, 8), 1, "linear filter matches 1×");

    let mut nearest = world_with(Camera::orthographic(2.0).with_clear_color(Color::BLACK));
    nearest.spawn((
        Transform::IDENTITY,
        Sprite::new("sprite").with_size(Vec2::splat(1.5)).with_anchor(Vec2::splat(0.5)).cutout().unlit(),
    ));
    let low = draw(&gpu, &mut one, &mut nearest, &assets, 16, 16);
    let high = draw(&gpu, &mut renderer, &mut nearest, &assets, 16, 16);
    expect_px(&high, 16, 8, 8, px(&low, 16, 8, 8), 1, "nearest filter matches 1×");
    assert_eq!(renderer.resource_stats().msaa_pipeline_creates, 1);
}

#[test]
fn point_and_directional_lights_match_one_sample() {
    let Some(gpu) = gpu() else { return };
    let mut assets = AssetManager::in_memory();
    assets.insert("card", card(0.0)).unwrap();
    let mut world = world_with(Camera::orthographic(2.0).with_clear_color(Color::BLACK));
    world.spawn((Transform::from_xyz(-1.0, 1.0, -2.0), Light::point(Color::rgb(0.2, 0.6, 1.0), 4.0, 12.0)));
    world.spawn((Transform::from_xyz(0.0, 0.0, -1.0), Light::directional(Color::WHITE, 0.4)));
    world.spawn((Transform::IDENTITY, Mesh::new("card").with_material(Material::color(Color::WHITE))));
    let mut one = linear_renderer(&gpu);
    let mut four = four(&gpu);
    let low = draw(&gpu, &mut one, &mut world, &assets, 16, 16);
    let high = draw(&gpu, &mut four, &mut world, &assets, 16, 16);
    expect_px(&high, 16, 8, 8, px(&low, 16, 8, 8), 2, "lit center");
    assert!(px(&high, 16, 8, 8)[2] > 20, "the point light contributes");
}
