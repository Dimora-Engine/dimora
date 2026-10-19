//! GPU acceptance for atlases packed by [`AtlasBuilder`].
//!
//! These tests upload the packed [`Texture`], build [`SpriteAsset`] values from the returned
//! [`AtlasRegion`] rectangles, and read pixels back from the existing scene renderer. They do not
//! add a second atlas renderer. Magnified samples use the region's UV as the packer stored it.
//!
//! Mip ranges below are measurements for the layouts in this file. They are not a formula that
//! makes an arbitrary atlas safe.

use engine_assets::{
    AddressMode, AssetHandle, AssetManager, Atlas, AtlasBuilder, AtlasConfig, ColorSpace, MipFilter, Mipmaps,
    SamplerDesc, SpriteAsset, TexelRect, Texture, TextureDimensionLimits, TextureFilter, TexturePixelEdits,
};
use engine_core::Color;
use engine_math::{Quat, Transform, Vec2, Vec3};
use engine_renderer::*;
use engine_renderer_wgpu::{GpuContext, SceneRenderer};
use engine_world::{propagate_transforms, GlobalTransform, World};

mod capture {
    include!("../../log_capture.rs");
}

const W: u32 = 128;
const H: u32 = 128;

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const YELLOW: [u8; 4] = [255, 255, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];
const CLEAR: [u8; 4] = [0, 0, 0, 0];

const REQUIRE_GPU_VAR: &str = "DIMORA_REQUIRE_GPU";

fn gpu() -> Option<GpuContext> {
    match GpuContext::headless() {
        Ok(gpu) => {
            let info = gpu.adapter.get_info();
            let anisotropic =
                gpu.adapter.get_downlevel_capabilities().flags.contains(wgpu::DownlevelFlags::ANISOTROPIC_FILTERING);
            eprintln!(
                "GPU test adapter: {} ({:?}, {:?}); DownlevelFlags::ANISOTROPIC_FILTERING={anisotropic}",
                info.name, info.backend, info.device_type
            );
            Some(gpu)
        }
        Err(error) => {
            let require = std::env::var(REQUIRE_GPU_VAR).ok();
            if require.as_deref() == Some("1") {
                panic!("{REQUIRE_GPU_VAR}=1 but the GPU test adapter is unavailable: {error}");
            }
            eprintln!("skipping GPU test: {error}");
            None
        }
    }
}

fn srgb_renderer(gpu: &GpuContext) -> SceneRenderer {
    SceneRenderer::new(gpu, wgpu::TextureFormat::Rgba8UnormSrgb)
}

fn linear_renderer(gpu: &GpuContext) -> SceneRenderer {
    SceneRenderer::new(gpu, wgpu::TextureFormat::Rgba8Unorm)
}

fn render(gpu: &GpuContext, renderer: &mut SceneRenderer, world: &mut World, assets: &AssetManager) -> Vec<u8> {
    propagate_transforms(world);
    let scene = extract_scene(world, assets, Vec2::new(W as f32, H as f32)).unwrap();
    renderer.render_to_rgba(gpu, &scene, assets, W, H).unwrap()
}

fn px(img: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * W + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

fn texel(image: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * width + x) * 4) as usize;
    [image[i], image[i + 1], image[i + 2], image[i + 3]]
}

fn near(pixel: [u8; 4], expected: [u8; 4], tolerance: i16) -> bool {
    pixel.iter().zip(expected).all(|(got, want)| (*got as i16 - want as i16).abs() <= tolerance)
}

fn expect_px(img: &[u8], x: u32, y: u32, expected: [u8; 4], tolerance: i16, what: &str) {
    let pixel = px(img, x, y);
    assert!(near(pixel, expected, tolerance), "{what} at ({x},{y}) is {pixel:?}, expected {expected:?} ±{tolerance}");
}

fn expect_texel(image: &[u8], width: u32, x: u32, y: u32, expected: [u8; 4], tolerance: i16, what: &str) {
    let pixel = texel(image, width, x, y);
    assert!(
        near(pixel, expected, tolerance),
        "{what} mip texel ({x},{y}) is {pixel:?}, expected {expected:?} ±{tolerance}"
    );
}

fn is_red(pixel: [u8; 4]) -> bool {
    pixel[0] > 200 && pixel[1] < 40 && pixel[2] < 40 && pixel[3] > 240
}

fn is_blue(pixel: [u8; 4]) -> bool {
    pixel[2] > 200 && pixel[0] < 40 && pixel[1] < 40 && pixel[3] > 240
}

fn count_red(img: &[u8]) -> usize {
    (0..H).map(|y| (0..W).filter(|&x| is_red(px(img, x, y))).count()).sum()
}

/// Screen pixel at the centre of texel `index` in a region of `count` texels filling the view.
fn texel_center(index: u32, count: u32) -> u32 {
    let x = (index as f32 + 0.5) / count as f32 * W as f32 - 0.5;
    x.round() as u32
}

fn solid(width: u32, height: u32, pixel: [u8; 4]) -> Vec<u8> {
    pixel.repeat((width * height) as usize)
}

fn bytes(pixels: &[[u8; 4]]) -> Vec<u8> {
    pixels.iter().copied().flatten().collect()
}

fn pack(config: AtlasConfig, images: &[(&str, u32, u32, &[u8])]) -> Atlas {
    let mut builder = AtlasBuilder::new(config);
    for (name, width, height, rgba) in images {
        builder.add(name, *width, *height, rgba).unwrap();
    }
    builder.build().unwrap()
}

fn linear_sampler(mip: Option<MipFilter>) -> SamplerDesc {
    SamplerDesc {
        min_filter: TextureFilter::Linear,
        mag_filter: TextureFilter::Linear,
        mip_filter: mip,
        address_u: AddressMode::Clamp,
        address_v: AddressMode::Clamp,
        anisotropy: 1,
    }
}

fn nearest_sampler(mip: Option<MipFilter>) -> SamplerDesc {
    SamplerDesc { mip_filter: mip, ..SamplerDesc::default() }
}

fn register(assets: &mut AssetManager, texture_name: &str, atlas: &Atlas) -> AssetHandle<Texture> {
    let texture = assets.insert(texture_name, atlas.texture().clone()).unwrap();
    for region in atlas.regions() {
        let sprite = atlas.sprite(&region.name, texture).expect("region was packed");
        assert_eq!(sprite.uv, region.uv, "{} uses the packer's UV", region.name);
        assert_eq!(sprite.size_px, Vec2::new(region.source_width as f32, region.source_height as f32));
        assert_eq!(sprite.pixels_per_unit, SpriteAsset::DEFAULT_PIXELS_PER_UNIT);
        assert_eq!(sprite.pivot, Vec2::new(0.5, 0.0));
        assets.insert(&region.name, sprite).unwrap();
    }
    texture
}

fn publish(assets: &mut AssetManager, name: &str, edit: impl FnOnce(&mut Texture)) {
    let handle = assets.texture(name);
    let mut texture = assets.get(handle).expect(name).clone();
    edit(&mut texture);
    assets.insert(name, texture).unwrap();
}

fn camera_at(camera: Camera, transform: Transform) -> World {
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.spawn((camera, transform));
    world
}

fn spawn_sprite(world: &mut World, name: &str, at: Transform, size: Vec2, blend: SpriteBlend) {
    world.spawn((at, Sprite::new(name).with_size(size).with_anchor(Vec2::splat(0.5)).unlit().with_blend(blend)));
}

fn draw_fullscreen(
    gpu: &GpuContext,
    renderer: &mut SceneRenderer,
    assets: &AssetManager,
    name: &str,
    blend: SpriteBlend,
) -> Vec<u8> {
    let mut world =
        camera_at(Camera::orthographic(2.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -4.0));
    spawn_sprite(&mut world, name, Transform::IDENTITY, Vec2::splat(2.0), blend);
    render(gpu, renderer, &mut world, assets)
}

fn projected(world: &mut World, point: Vec3) -> (u32, u32) {
    propagate_transforms(world);
    let (camera, global) = {
        let mut query = world.query::<(&Camera, &GlobalTransform)>();
        let (_, (camera, global)) = query.iter().next().expect("camera");
        (*camera, *global)
    };
    let screen = CameraView::new(&camera, &global, Vec2::new(W as f32, H as f32))
        .project(point)
        .expect("in front of the camera");
    let x = screen.pixels.x.round().clamp(0.0, (W - 1) as f32) as u32;
    let y = screen.pixels.y.round().clamp(0.0, (H - 1) as f32) as u32;
    (x, y)
}

fn mip_image(
    renderer: &SceneRenderer,
    gpu: &GpuContext,
    id: engine_assets::AssetId,
    level: u32,
) -> (Vec<u8>, u32, u32) {
    let info = renderer.cached_texture(id).expect("cached atlas");
    let width = (info.width >> level).max(1);
    let height = (info.height >> level).max(1);
    let image = renderer.read_cached_mip(gpu, id, level).expect("mip level");
    assert_eq!(image.len(), (width * height * 4) as usize);
    (image, width, height)
}

#[test]
fn atlas_nearest_sprites_show_their_own_texels_and_share_one_image() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = srgb_renderer(&gpu);
    let mut assets = AssetManager::in_memory();

    // 8×4, no padding. Insertion order puts mark, band and edge on the first shelf and low on the next.
    let mark = bytes(&[RED, GREEN, BLUE, WHITE]);
    let band = bytes(&[RED, GREEN, BLUE, YELLOW]);
    let edge = solid(2, 2, WHITE);
    let low = bytes(&[BLACK, BLACK, BLACK, RED, RED, RED]);
    let atlas = pack(
        AtlasConfig::new(8, 4),
        &[
            ("mark", 2, 2, mark.as_slice()),
            ("band", 4, 1, band.as_slice()),
            ("edge", 2, 2, edge.as_slice()),
            ("low", 3, 2, low.as_slice()),
        ],
    );
    assert_eq!((atlas.texture().width, atlas.texture().height), (8, 4));
    let mark_r = atlas.region("mark").unwrap();
    let band_r = atlas.region("band").unwrap();
    let edge_r = atlas.region("edge").unwrap();
    let low_r = atlas.region("low").unwrap();
    assert_eq!((mark_r.x, mark_r.y, mark_r.width, mark_r.height), (0, 0, 2, 2));
    assert_eq!((band_r.x, band_r.y, band_r.width, band_r.height), (2, 0, 4, 1), "adjacent to mark");
    assert_eq!((edge_r.x, edge_r.y), (6, 0));
    assert_eq!(edge_r.x + edge_r.width, 8, "edge touches the texture boundary");
    assert_eq!((low_r.x, low_r.y, low_r.width, low_r.height), (0, 2, 3, 2), "next shelf");
    assert_eq!(low_r.y + low_r.height, 4, "low touches the bottom boundary");

    let handle = register(&mut assets, "atlas", &atlas);
    let before = renderer.resource_stats();
    let mut together =
        camera_at(Camera::orthographic(2.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -4.0));
    for (index, name) in ["mark", "band", "edge", "low"].iter().enumerate() {
        let x = -0.6 + index as f32 * 0.4;
        spawn_sprite(&mut together, name, Transform::from_xyz(x, 0.0, 0.0), Vec2::splat(0.3), SpriteBlend::Cutout);
    }
    render(&gpu, &mut renderer, &mut together, &assets);
    let uploaded = renderer.resource_stats();
    assert_eq!(uploaded.texture_creates, before.texture_creates + 1);
    assert_eq!(uploaded.full_base_uploads, before.full_base_uploads + 1);
    assert_eq!(uploaded.partial_base_uploads, before.partial_base_uploads);
    assert_eq!(uploaded.base_upload_bytes, before.base_upload_bytes + 8 * 4 * 4);
    assert_eq!(uploaded.sampler_creates, before.sampler_creates, "the default nearest clamp sampler already exists");
    assert_eq!(uploaded.bind_group_creates, before.bind_group_creates + 1);
    assert_eq!(uploaded.mip_generations, before.mip_generations);
    assert_eq!(renderer.cache_stats().textures, 1);
    let info = renderer.cached_texture(handle.id()).unwrap();
    assert_eq!(info.color_space, ColorSpace::Srgb);
    assert_eq!((info.min_filter, info.mag_filter), (TextureFilter::Nearest, TextureFilter::Nearest));
    assert_eq!((info.address_u, info.address_v), (AddressMode::Clamp, AddressMode::Clamp));
    assert!(!info.mip_filter_linear);
    assert_eq!(info.mip_level_count, 1);
    let base = renderer.read_cached_mip(&gpu, handle.id(), 0).unwrap();
    assert_eq!(base, atlas.texture().rgba, "the upload keeps the packed bytes");

    let mark_img = draw_fullscreen(&gpu, &mut renderer, &assets, "mark", SpriteBlend::Cutout);
    expect_px(&mark_img, texel_center(0, 2), texel_center(0, 2), RED, 0, "mark top-left, not flipped");
    expect_px(&mark_img, texel_center(1, 2), texel_center(0, 2), GREEN, 0, "mark top-right");
    expect_px(&mark_img, texel_center(0, 2), texel_center(1, 2), BLUE, 0, "mark bottom-left");
    expect_px(&mark_img, texel_center(1, 2), texel_center(1, 2), WHITE, 0, "mark bottom-right");

    let band_img = draw_fullscreen(&gpu, &mut renderer, &assets, "band", SpriteBlend::Cutout);
    let band_colors = [RED, GREEN, BLUE, YELLOW];
    for (index, color) in band_colors.iter().enumerate() {
        expect_px(&band_img, texel_center(index as u32, 4), texel_center(0, 1), *color, 0, "band texel");
    }

    let edge_img = draw_fullscreen(&gpu, &mut renderer, &assets, "edge", SpriteBlend::Cutout);
    expect_px(&edge_img, texel_center(0, 2), texel_center(0, 2), WHITE, 0, "boundary region is its own image");
    expect_px(&edge_img, texel_center(1, 2), texel_center(1, 2), WHITE, 0, "boundary region is its own image");
    assert!(!is_red(px(&edge_img, texel_center(0, 2), texel_center(0, 2))));

    let low_img = draw_fullscreen(&gpu, &mut renderer, &assets, "low", SpriteBlend::Cutout);
    expect_px(&low_img, texel_center(1, 3), texel_center(0, 2), BLACK, 0, "lower shelf top row");
    expect_px(&low_img, texel_center(1, 3), texel_center(1, 2), RED, 0, "lower shelf bottom row");

    render(&gpu, &mut renderer, &mut together, &assets);
    let again = renderer.resource_stats();
    assert_eq!(again.full_base_uploads, uploaded.full_base_uploads);
    assert_eq!(again.partial_base_uploads, uploaded.partial_base_uploads);
    assert_eq!(again.texture_creates, uploaded.texture_creates);
    assert_eq!(again.sampler_creates, uploaded.sampler_creates);
    assert_eq!(again.bind_group_creates, uploaded.bind_group_creates);
    assert_eq!(renderer.cache_stats().textures, 1);

    assets.remove(handle);
    let mut empty =
        camera_at(Camera::orthographic(2.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -4.0));
    render(&gpu, &mut renderer, &mut empty, &assets);
    assert_eq!(renderer.cache_stats().textures, 0, "removing the atlas texture follows the existing eviction rule");
    assert!(renderer.cached_texture(handle.id()).is_none());
}

#[test]
fn padding_stays_out_of_sprite_size_and_the_pivot_is_unchanged() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = srgb_renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    let atlas =
        pack(AtlasConfig::new(4, 4).with_padding(1).with_extrusion(1), &[("red", 2, 2, solid(2, 2, RED).as_slice())]);
    let region = atlas.region("red").unwrap();
    assert_eq!((region.width, region.height), (2, 2));
    assert_eq!((region.source_width, region.source_height), (2, 2));
    assert_eq!((region.x, region.y), (1, 1), "the visible origin is inset by the padding");
    register(&mut assets, "atlas", &atlas);
    let sprite = assets.get(assets.sprite("red")).unwrap();
    assert_eq!(sprite.world_size(), Vec2::splat(2.0 / 16.0));

    let mut centered =
        camera_at(Camera::orthographic(0.25).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -4.0));
    centered.spawn((Transform::IDENTITY, Sprite::new("red").with_anchor(Vec2::splat(0.5)).unlit().cutout()));
    let image = render(&gpu, &mut renderer, &mut centered, &assets);
    let (inside_x, inside_y) = projected(&mut centered, Vec3::ZERO);
    let (outside_x, outside_y) = projected(&mut centered, Vec3::new(0.0, 0.1, 0.0));
    assert!(is_red(px(&image, inside_x, inside_y)), "natural centre {:?}", px(&image, inside_x, inside_y));
    assert_eq!(
        px(&image, outside_x, outside_y),
        [0, 0, 0, 255],
        "y=0.1 is outside a 2px sprite and inside a sprite that included the padding gutter"
    );

    let mut pivoted =
        camera_at(Camera::orthographic(0.5).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -4.0));
    pivoted.spawn((Transform::IDENTITY, Sprite::new("red").unlit().cutout()));
    let image = render(&gpu, &mut renderer, &mut pivoted, &assets);
    let (body_x, body_y) = projected(&mut pivoted, Vec3::new(0.0, 0.0625, 0.0));
    let (below_x, below_y) = projected(&mut pivoted, Vec3::new(0.0, -0.02, 0.0));
    let (above_x, above_y) = projected(&mut pivoted, Vec3::new(0.0, 0.14, 0.0));
    assert!(is_red(px(&image, body_x, body_y)), "default bottom-centre pivot {:?}", px(&image, body_x, body_y));
    assert_eq!(px(&image, below_x, below_y), [0, 0, 0, 255], "below the pivot");
    assert_eq!(px(&image, above_x, above_y), [0, 0, 0, 255], "above the 2px sprite");
}

#[test]
fn linear_magnification_reaches_about_half_a_texel_past_the_region() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = linear_renderer(&gpu);

    // Padding 0. The right edge of each 1×1 region blends with the next image. This is a limitation.
    let tight = pack(
        AtlasConfig::new(4, 1).with_color_space(ColorSpace::Linear).with_sampler(linear_sampler(None)).unwrap(),
        &[("red", 1, 1, &RED), ("blue", 1, 1, &BLUE), ("yellow", 1, 1, &YELLOW), ("clear", 1, 1, &CLEAR)],
    );
    let mut tight_assets = AssetManager::in_memory();
    let tight_handle = register(&mut tight_assets, "tight", &tight);
    let red = draw_fullscreen(&gpu, &mut renderer, &tight_assets, "red", SpriteBlend::Cutout);
    expect_px(&red, 0, 64, RED, 2, "clamp at the atlas edge stays on the region");
    expect_px(&red, 64, 64, RED, 4, "the centre of a magnified texel stays on that texel");
    let edge = px(&red, 127, 64);
    assert!(
        edge[0] > 80 && edge[2] > 80 && edge[1] < 20,
        "no padding: the region edge mixes the blue neighbour {edge:?}"
    );
    let blue = draw_fullscreen(&gpu, &mut renderer, &tight_assets, "blue", SpriteBlend::Cutout);
    let blue_left = px(&blue, 0, 64);
    let blue_right = px(&blue, 127, 64);
    assert!(blue_left[0] > 80 && blue_left[2] > 80, "blue's left edge mixes red {blue_left:?}");
    assert!(
        blue_right[0] > 80 && blue_right[1] > 80 && blue_right[2] > 80,
        "blue's right edge mixes yellow {blue_right:?}"
    );
    let uploaded = renderer.read_cached_mip(&gpu, tight_handle.id(), 0).unwrap();
    assert_eq!(uploaded, tight.texture().rgba);

    // Padding without extrusion. The same edge mixes transparent black, not the neighbour.
    let gutter = pack(
        AtlasConfig::new(12, 3)
            .with_padding(1)
            .with_color_space(ColorSpace::Linear)
            .with_sampler(linear_sampler(None))
            .unwrap(),
        &[("red", 1, 1, &RED), ("blue", 1, 1, &BLUE), ("yellow", 1, 1, &YELLOW), ("clear", 1, 1, &CLEAR)],
    );
    let mut gutter_assets = AssetManager::in_memory();
    let gutter_handle = register(&mut gutter_assets, "gutter", &gutter);
    let red = draw_fullscreen(&gpu, &mut renderer, &gutter_assets, "red", SpriteBlend::Alpha);
    let fringe = px(&red, 127, 64);
    assert!(
        (40..=100).contains(&fringe[0]) && fringe[1] < 12 && fringe[2] < 12 && fringe[3] > 240,
        "padding without extrusion darkens the edge over black and does not pick up blue {fringe:?}"
    );
    expect_px(&red, 64, 64, RED, 8, "the centre is still the source texel");
    let red_region = gutter.region("red").unwrap();
    let base = renderer.read_cached_mip(&gpu, gutter_handle.id(), 0).unwrap();
    expect_texel(&base, 12, red_region.x + 1, red_region.y, CLEAR, 0, "the unextruded gutter stays clear");
    expect_texel(&base, 12, gutter.region("blue").unwrap().x, red_region.y, BLUE, 0, "the neighbour is untouched");

    // Extrusion 1 copies the edge, so this magnification no longer sees the neighbour or the clear gutter.
    let extruded = pack(
        AtlasConfig::new(12, 3)
            .with_padding(1)
            .with_extrusion(1)
            .with_color_space(ColorSpace::Linear)
            .with_sampler(linear_sampler(None))
            .unwrap(),
        &[("red", 1, 1, &RED), ("blue", 1, 1, &BLUE), ("yellow", 1, 1, &YELLOW), ("clear", 1, 1, &CLEAR)],
    );
    let mut extruded_assets = AssetManager::in_memory();
    let extruded_handle = register(&mut extruded_assets, "extruded", &extruded);
    let red = draw_fullscreen(&gpu, &mut renderer, &extruded_assets, "red", SpriteBlend::Cutout);
    expect_px(&red, 96, 64, RED, 2, "inside the region");
    expect_px(&red, 127, 64, RED, 2, "extrusion 1 covers the half-texel linear footprint");
    let red_region = extruded.region("red").unwrap();
    let base = renderer.read_cached_mip(&gpu, extruded_handle.id(), 0).unwrap();
    expect_texel(&base, 12, red_region.x + 1, red_region.y, RED, 0, "the right gutter is the red edge");
    expect_texel(&base, 12, red_region.x - 1, red_region.y - 1, RED, 0, "the corner gutter is the red corner");
    expect_texel(
        &base,
        12,
        extruded.region("blue").unwrap().x,
        red_region.y,
        BLUE,
        0,
        "extrusion stops at the allocated edge",
    );

    // Extrusion 2 fills two texels. Magnification still only reaches about half a texel, so the
    // framebuffer edge matches extrusion 1. The second gutter texel is checked on the base mip.
    let wide = pack(
        AtlasConfig::new(10, 5)
            .with_padding(2)
            .with_extrusion(2)
            .with_color_space(ColorSpace::Linear)
            .with_sampler(linear_sampler(None))
            .unwrap(),
        &[("red", 1, 1, &RED), ("blue", 1, 1, &BLUE)],
    );
    let mut wide_assets = AssetManager::in_memory();
    let wide_handle = register(&mut wide_assets, "wide", &wide);
    let red = draw_fullscreen(&gpu, &mut renderer, &wide_assets, "red", SpriteBlend::Cutout);
    expect_px(&red, 127, 64, RED, 2, "a wider extrusion still covers this magnification");
    let red_region = wide.region("red").unwrap();
    let base = renderer.read_cached_mip(&gpu, wide_handle.id(), 0).unwrap();
    expect_texel(&base, 10, red_region.x - 2, red_region.y, RED, 0, "two texels of extrusion");
    expect_texel(&base, 10, red_region.x + 2, red_region.y + 2, RED, 0, "the far corner is extruded");

    let partial = pack(
        AtlasConfig::new(10, 5)
            .with_padding(2)
            .with_extrusion(1)
            .with_color_space(ColorSpace::Linear)
            .with_sampler(linear_sampler(None))
            .unwrap(),
        &[("red", 1, 1, &RED), ("blue", 1, 1, &BLUE)],
    );
    let mut partial_assets = AssetManager::in_memory();
    let partial_handle = register(&mut partial_assets, "partial", &partial);
    let region = partial.region("red").unwrap();
    let _ = draw_fullscreen(&gpu, &mut renderer, &partial_assets, "red", SpriteBlend::Cutout);
    let base = renderer.read_cached_mip(&gpu, partial_handle.id(), 0).unwrap();
    expect_texel(&base, 10, region.x - 1, region.y, RED, 0, "the inner gutter copies the edge");
    expect_texel(&base, 10, region.x - 2, region.y, CLEAR, 0, "padding outside the extrusion stays clear");
}

#[test]
fn generated_mips_isolate_regions_only_for_specific_layouts() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = linear_renderer(&gpu);

    // Padding 0, 1×2 images side by side in a 4×2 atlas. The first downsample's 2×2 footprint
    // crosses the shared edge, so level 1 already mixes. floor(log2(0)) is not the condition.
    let crossing =
        mip_atlas(4, 2, 0, 0, &[("red", 1, 2, RED), ("blue", 1, 2, BLUE)], nearest_sampler(Some(MipFilter::Nearest)));
    let mut assets = AssetManager::in_memory();
    let handle = register(&mut assets, "crossing", &crossing);
    draw_fullscreen(&gpu, &mut renderer, &assets, "red", SpriteBlend::Cutout);
    let info = renderer.cached_texture(handle.id()).unwrap();
    assert_eq!(info.mip_level_count, 3);
    assert!(!info.mip_filter_linear);
    let (level1, width, _) = mip_image(&renderer, &gpu, handle.id(), 1);
    expect_texel(&level1, width, 0, 0, [128, 0, 128, 255], 3, "padding 0 crosses on level 1");
    expect_texel(&level1, width, 1, 0, CLEAR, 2, "the empty half of the atlas stays clear");
    let magnified = draw_fullscreen(&gpu, &mut renderer, &assets, "red", SpriteBlend::Cutout);
    expect_px(&magnified, 64, 64, RED, 2, "magnification still shows the base level");

    // Padding 0, but the colour boundary sits between 2×2 groups. Level 1 stays pure. Level 2 mixes.
    let aligned =
        mip_atlas(16, 8, 0, 0, &[("red", 6, 4, RED), ("blue", 6, 4, BLUE)], nearest_sampler(Some(MipFilter::Linear)));
    let mut aligned_assets = AssetManager::in_memory();
    let aligned_handle = register(&mut aligned_assets, "aligned", &aligned);
    draw_fullscreen(&gpu, &mut renderer, &aligned_assets, "red", SpriteBlend::Cutout);
    assert!(renderer.cached_texture(aligned_handle.id()).unwrap().mip_filter_linear);
    assert_eq!(renderer.cached_texture(aligned_handle.id()).unwrap().mip_level_count, 5);
    let (level1, width, _) = mip_image(&renderer, &gpu, aligned_handle.id(), 1);
    expect_texel(&level1, width, 2, 0, RED, 1, "level 1 red group");
    expect_texel(&level1, width, 3, 0, BLUE, 1, "level 1 blue group");
    let (level2, width, _) = mip_image(&renderer, &gpu, aligned_handle.id(), 2);
    expect_texel(&level2, width, 0, 0, RED, 1, "level 2 still has a pure red texel");
    expect_texel(&level2, width, 1, 0, [128, 0, 128, 255], 3, "level 2 is the first mixed level");
    expect_texel(&level2, width, 2, 0, BLUE, 1, "level 2 still has a pure blue texel");

    // Padding 1 and extrusion 0. Neighbours stay apart through level 2. Every coarser texel of the
    // region is already mixed with the clear gutter, so the colour is not preserved.
    let clear_gutter =
        mip_atlas(8, 4, 1, 0, &[("red", 2, 2, RED), ("blue", 2, 2, BLUE)], linear_sampler(Some(MipFilter::Nearest)));
    let mut clear_assets = AssetManager::in_memory();
    let clear_handle = register(&mut clear_assets, "clear-gutter", &clear_gutter);
    draw_fullscreen(&gpu, &mut renderer, &clear_assets, "red", SpriteBlend::Cutout);
    for level in 1..=2 {
        let (image, width, height) = mip_image(&renderer, &gpu, clear_handle.id(), level);
        for y in 0..height {
            for x in 0..width {
                let pixel = texel(&image, width, x, y);
                let mixed_neighbours = pixel[0] > 20 && pixel[2] > 20;
                assert!(!mixed_neighbours, "level {level} texel ({x},{y}) mixes red and blue: {pixel:?}");
                if pixel[0] > 20 {
                    assert!(pixel[3] < 80, "level {level} keeps the red side clear-mixed, not opaque: {pixel:?}");
                }
            }
        }
    }
    let (level3, width, _) = mip_image(&renderer, &gpu, clear_handle.id(), 3);
    let coarse = texel(&level3, width, 0, 0);
    assert!(coarse[0] > 10 && coarse[2] > 10, "level 3 of the clear gutter mixes the neighbours {coarse:?}");

    // Padding 1 and extrusion 1 fill each 4×4 allocation. Levels 1 and 2 stay opaque and apart.
    // Level 3 mixes. That is wider than floor(log2(1)).
    let filled =
        mip_atlas(8, 4, 1, 1, &[("red", 2, 2, RED), ("blue", 2, 2, BLUE)], nearest_sampler(Some(MipFilter::Nearest)));
    let mut filled_assets = AssetManager::in_memory();
    let filled_handle = register(&mut filled_assets, "filled", &filled);
    draw_fullscreen(&gpu, &mut renderer, &filled_assets, "red", SpriteBlend::Cutout);
    let (level1, width, _) = mip_image(&renderer, &gpu, filled_handle.id(), 1);
    expect_texel(&level1, width, 0, 0, RED, 1, "extruded level 1");
    expect_texel(&level1, width, 3, 1, BLUE, 1, "extruded level 1");
    let (level2, width, _) = mip_image(&renderer, &gpu, filled_handle.id(), 2);
    expect_texel(&level2, width, 0, 0, RED, 1, "extruded level 2");
    expect_texel(&level2, width, 1, 0, BLUE, 1, "extruded level 2");
    let (level3, width, _) = mip_image(&renderer, &gpu, filled_handle.id(), 3);
    expect_texel(&level3, width, 0, 0, [128, 0, 128, 255], 3, "level 3 mixes the two filled allocations");

    // Padding 2 and extrusion 2, 4×4 images in a 16×16 atlas. Isolation holds through level 3.
    // Level 4 is one texel over the red allocation, the blue allocation, and the empty half.
    let safe =
        mip_atlas(16, 16, 2, 2, &[("red", 4, 4, RED), ("blue", 4, 4, BLUE)], linear_sampler(Some(MipFilter::Linear)));
    let mut safe_assets = AssetManager::in_memory();
    let safe_handle = register(&mut safe_assets, "safe", &safe);
    draw_fullscreen(&gpu, &mut renderer, &safe_assets, "red", SpriteBlend::Cutout);
    assert_eq!(renderer.cached_texture(safe_handle.id()).unwrap().mip_level_count, 5);
    let (level3, width, _) = mip_image(&renderer, &gpu, safe_handle.id(), 3);
    expect_texel(&level3, width, 0, 0, RED, 1, "aligned extrusion stays red through level 3");
    expect_texel(&level3, width, 1, 0, BLUE, 1, "aligned extrusion stays blue through level 3");
    expect_texel(&level3, width, 0, 1, CLEAR, 2, "the empty half is still clear at level 3");
    let (level4, width, _) = mip_image(&renderer, &gpu, safe_handle.id(), 4);
    expect_texel(&level4, width, 0, 0, [64, 0, 64, 128], 4, "level 4 blends red, blue, and the empty half");

    // A 4-texel region drawn across 2 screen pixels selects about mip 1, which this layout keeps red.
    for (label, sampler) in [
        ("nearest mip", nearest_sampler(Some(MipFilter::Nearest))),
        ("linear mip", linear_sampler(Some(MipFilter::Linear))),
    ] {
        let atlas = mip_atlas(16, 16, 2, 2, &[("red", 4, 4, RED), ("blue", 4, 4, BLUE)], sampler);
        let mut lod_assets = AssetManager::in_memory();
        register(&mut lod_assets, "lod", &atlas);
        let mut world =
            camera_at(Camera::orthographic(64.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -4.0));
        spawn_sprite(&mut world, "red", Transform::IDENTITY, Vec2::splat(1.0), SpriteBlend::Cutout);
        let image = render(&gpu, &mut renderer, &mut world, &lod_assets);
        assert!(
            is_red(px(&image, 64, 64)),
            "{label} of the safe atlas at this zoom stays red {:?}",
            px(&image, 64, 64)
        );
    }
}

fn mip_atlas(
    width: u32,
    height: u32,
    padding: u32,
    extrusion: u32,
    images: &[(&str, u32, u32, [u8; 4])],
    sampler: SamplerDesc,
) -> Atlas {
    let config = AtlasConfig::new(width, height)
        .with_padding(padding)
        .with_extrusion(extrusion)
        .with_color_space(ColorSpace::Linear)
        .with_mipmaps(Mipmaps::Generate)
        .with_sampler(sampler)
        .unwrap();
    let owned: Vec<(&str, u32, u32, Vec<u8>)> =
        images.iter().map(|(name, w, h, pixel)| (*name, *w, *h, solid(*w, *h, *pixel))).collect();
    let views: Vec<(&str, u32, u32, &[u8])> =
        owned.iter().map(|(name, w, h, rgba)| (*name, *w, *h, rgba.as_slice())).collect();
    pack(config, &views)
}

#[test]
fn srgb_and_linear_atlases_preserve_bytes_and_decode_on_sample() {
    let Some(gpu) = gpu() else { return };
    let byte = 180u8;
    let decoded = (Color::from_srgb8(byte, byte, byte).r * 255.0).round() as u8;
    assert!(decoded + 20 < byte, "decoded {decoded}");

    for space in [ColorSpace::Linear, ColorSpace::Srgb] {
        let mut renderer = linear_renderer(&gpu);
        let gray = [byte, byte, byte, 255];
        let other = [10, 20, 30, 255];
        let atlas = pack(
            AtlasConfig::new(2, 1)
                .with_color_space(space)
                .with_mipmaps(Mipmaps::Generate)
                .with_sampler(nearest_sampler(Some(MipFilter::Nearest)))
                .unwrap(),
            &[("gray", 1, 1, &gray), ("other", 1, 1, &other)],
        );
        assert_eq!(&atlas.texture().rgba[..4], &gray, "{space:?} construction keeps the source bytes");
        assert_eq!(&atlas.texture().rgba[4..], &other);
        let mut assets = AssetManager::in_memory();
        let handle = register(&mut assets, "colors", &atlas);
        let image = draw_fullscreen(&gpu, &mut renderer, &assets, "gray", SpriteBlend::Cutout);
        let info = renderer.cached_texture(handle.id()).unwrap();
        assert_eq!(info.color_space, space);
        assert_eq!(info.mip_level_count, 2, "a 2×1 chain follows the existing mip allocation");
        let base = renderer.read_cached_mip(&gpu, handle.id(), 0).unwrap();
        assert_eq!(base, atlas.texture().rgba, "{space:?} upload stores the packed bytes");
        let expected = if space == ColorSpace::Linear { byte } else { decoded };
        expect_px(&image, 64, 64, [expected, expected, expected, 255], 2, "sampled atlas texel");
        let (level1, width, _) = mip_image(&renderer, &gpu, handle.id(), 1);
        let averaged = texel(&level1, width, 0, 0);
        if space == ColorSpace::Linear {
            expect_texel(&level1, width, 0, 0, [95, 100, 105, 255], 3, "linear mip averages stored bytes");
        } else {
            let stored = i16::from(averaged[0]);
            assert!(
                stored > i16::from(decoded) - 5 && stored + 30 < i16::from(byte),
                "sRGB mip decodes before averaging {averaged:?}"
            );
            assert!(!near(averaged, [95, 100, 105, 255], 3), "sRGB mip is not the byte average {averaged:?}");
        }
    }
}

#[test]
fn transparent_edges_separate_packing_bugs_from_straight_alpha() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = linear_renderer(&gpu);

    // Padding 0. The soft image's right texel is clear, and the next texel is blue.
    // Linear magnification at the sprite edge reaches that neighbour. That is a missing gutter.
    let soft = bytes(&[CLEAR, [255, 0, 0, 128], CLEAR]);
    let tight = pack(
        AtlasConfig::new(4, 1).with_color_space(ColorSpace::Linear).with_sampler(linear_sampler(None)).unwrap(),
        &[("soft", 3, 1, soft.as_slice()), ("blue", 1, 1, &BLUE)],
    );
    let mut tight_assets = AssetManager::in_memory();
    register(&mut tight_assets, "tight", &tight);
    let nearest = pack(
        AtlasConfig::new(4, 1).with_color_space(ColorSpace::Linear),
        &[("soft", 3, 1, soft.as_slice()), ("blue", 1, 1, &BLUE)],
    );
    let mut nearest_assets = AssetManager::in_memory();
    let nearest_handle = register(&mut nearest_assets, "nearest", &nearest);
    let image = draw_fullscreen(&gpu, &mut renderer, &nearest_assets, "soft", SpriteBlend::Alpha);
    let center = px(&image, texel_center(1, 3), texel_center(0, 1));
    assert!(
        near(center, [128, 0, 0, 255], 4),
        "nearest partial alpha composites over black and does not borrow blue {center:?}"
    );
    let stored = renderer.read_cached_mip(&gpu, nearest_handle.id(), 0).unwrap();
    expect_texel(&stored, 4, 1, 0, [255, 0, 0, 128], 0, "nearest storage keeps the partial pixel");
    expect_texel(&stored, 4, 0, 0, CLEAR, 0, "the clear edge is stored");

    let leaked = draw_fullscreen(&gpu, &mut renderer, &tight_assets, "soft", SpriteBlend::Alpha);
    let edge = px(&leaked, 127, 64);
    assert!(edge[2] > 30 && edge[0] < 20, "no gutter: the clear edge mixes the blue neighbour {edge:?}");

    // Extruding the clear edge keeps that same sample on clear texels.
    let extruded = pack(
        AtlasConfig::new(8, 3)
            .with_padding(1)
            .with_extrusion(1)
            .with_color_space(ColorSpace::Linear)
            .with_sampler(linear_sampler(None))
            .unwrap(),
        &[("soft", 3, 1, soft.as_slice()), ("blue", 1, 1, &BLUE)],
    );
    let mut extruded_assets = AssetManager::in_memory();
    let extruded_handle = register(&mut extruded_assets, "extruded", &extruded);
    let image = draw_fullscreen(&gpu, &mut renderer, &extruded_assets, "soft", SpriteBlend::Alpha);
    let edge = px(&image, 127, 64);
    assert!(edge[2] < 12 && edge[0] < 20, "extruded clear edge does not pick up blue {edge:?}");
    let region = extruded.region("soft").unwrap();
    let base = renderer.read_cached_mip(&gpu, extruded_handle.id(), 0).unwrap();
    expect_texel(&base, 8, region.x + region.width, region.y, CLEAR, 0, "the extruded edge copies the clear texel");
    expect_texel(
        &base,
        8,
        extruded.region("blue").unwrap().x,
        extruded.region("blue").unwrap().y,
        BLUE,
        0,
        "blue stays in its region",
    );

    // A whole-atlas mip averages straight alpha. Green 128 is not premultiplied down to about 64.
    let straight = pack(
        AtlasConfig::new(2, 1)
            .with_color_space(ColorSpace::Linear)
            .with_mipmaps(Mipmaps::Generate)
            .with_sampler(nearest_sampler(Some(MipFilter::Nearest)))
            .unwrap(),
        &[("red", 1, 1, &RED), ("green", 1, 1, &[0, 255, 0, 128])],
    );
    let mut straight_assets = AssetManager::in_memory();
    let straight_handle = register(&mut straight_assets, "straight", &straight);
    draw_fullscreen(&gpu, &mut renderer, &straight_assets, "red", SpriteBlend::Cutout);
    let (level1, width, _) = mip_image(&renderer, &gpu, straight_handle.id(), 1);
    expect_texel(&level1, width, 0, 0, [128, 128, 0, 192], 4, "mip alpha is a straight average");
}

#[test]
fn perspective_orthographic_and_isometric_sample_the_same_regions() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = srgb_renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    let atlas = pack(
        AtlasConfig::new(12, 6).with_padding(1).with_extrusion(1),
        &[("red", 4, 4, solid(4, 4, RED).as_slice()), ("blue", 4, 4, solid(4, 4, BLUE).as_slice())],
    );
    register(&mut assets, "atlas", &atlas);
    let red = Sprite::new("red").with_size(Vec2::splat(1.0)).with_anchor(Vec2::splat(0.5)).unlit().cutout();
    let blue = Sprite::new("blue").with_size(Vec2::splat(1.0)).with_anchor(Vec2::splat(0.5)).unlit().cutout();

    let mut pair = |camera: Camera, transform: Transform, label: &str| {
        let mut world = camera_at(camera, transform);
        world.spawn((Transform::from_xyz(-0.7, 0.0, 0.0), red));
        world.spawn((Transform::from_xyz(0.7, 0.0, 0.0), blue));
        let image = render(&gpu, &mut renderer, &mut world, &assets);
        let (red_x, red_y) = projected(&mut world, Vec3::new(-0.7, 0.0, 0.0));
        let (blue_x, blue_y) = projected(&mut world, Vec3::new(0.7, 0.0, 0.0));
        assert!(is_red(px(&image, red_x, red_y)), "{label} red {:?}", px(&image, red_x, red_y));
        assert!(is_blue(px(&image, blue_x, blue_y)), "{label} blue {:?}", px(&image, blue_x, blue_y));
        assert!((red_x as i32 - blue_x as i32).abs() > 8, "{label} separates the sprites ({red_x},{blue_x})");
    };
    pair(Camera::orthographic(4.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -4.0), "orthographic");
    pair(Camera::perspective(60.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -6.0), "perspective");

    let pitch = 0.615_479_7;
    let rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_4) * Quat::from_rotation_x(pitch);
    let forward = Transform::from_rotation(rotation).forward();
    let isometric = Transform::from_position(-forward * 6.0).with_rotation(rotation);
    pair(Camera::orthographic(4.0).with_clear_color(Color::BLACK), isometric, "isometric");

    let mut near =
        camera_at(Camera::perspective(60.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -4.0));
    near.spawn((Transform::IDENTITY, red));
    let near_img = render(&gpu, &mut renderer, &mut near, &assets);
    let mut far =
        camera_at(Camera::perspective(60.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -18.0));
    far.spawn((Transform::IDENTITY, red));
    let far_img = render(&gpu, &mut renderer, &mut far, &assets);
    let near_count = count_red(&near_img);
    let far_count = count_red(&far_img);
    assert!(near_count > far_count * 4 && far_count > 0, "perspective depth minifies ({near_count} vs {far_count})");
    let (far_x, far_y) = projected(&mut far, Vec3::ZERO);
    assert!(is_red(px(&far_img, far_x, far_y)), "the minified region stays red {:?}", px(&far_img, far_x, far_y));

    let mut wide =
        camera_at(Camera::orthographic(2.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -4.0));
    wide.spawn((Transform::IDENTITY, red));
    let wide_img = render(&gpu, &mut renderer, &mut wide, &assets);
    let mut zoomed =
        camera_at(Camera::orthographic(10.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -4.0));
    zoomed.spawn((Transform::IDENTITY, red));
    let zoomed_img = render(&gpu, &mut renderer, &mut zoomed, &assets);
    let wide_count = count_red(&wide_img);
    let zoomed_count = count_red(&zoomed_img);
    assert!(
        wide_count > zoomed_count * 4 && zoomed_count > 0,
        "orthographic zoom minifies ({wide_count} vs {zoomed_count})"
    );
    assert!(is_red(px(&zoomed_img, 64, 64)), "zoomed region {:?}", px(&zoomed_img, 64, 64));
}

#[test]
fn replace_source_clears_a_stale_gutter_with_one_partial_upload() {
    let Some(gpu) = gpu() else { return };
    let green = [0, 255, 0, 255];
    for mips in [Mipmaps::None, Mipmaps::Generate] {
        let mut renderer = linear_renderer(&gpu);
        let mut assets = AssetManager::in_memory();
        let atlas = pack(
            AtlasConfig::new(8, 4)
                .with_padding(1)
                .with_extrusion(1)
                .with_color_space(ColorSpace::Linear)
                .with_mipmaps(mips)
                .with_sampler(linear_sampler(None))
                .unwrap(),
            &[("red", 1, 1, &RED), ("blue", 1, 1, &BLUE)],
        );
        let region = atlas.region("red").unwrap().clone();
        let blue = atlas.region("blue").unwrap().clone();
        let handle = register(&mut assets, "atlas", &atlas);
        let image = draw_fullscreen(&gpu, &mut renderer, &assets, "red", SpriteBlend::Cutout);
        expect_px(&image, 64, 64, RED, 4, "before the edit");
        expect_px(&image, 127, 64, RED, 4, "the extruded edge before the edit");
        let uploaded = renderer.resource_stats();
        let levels = renderer.cached_texture(handle.id()).unwrap().mip_level_count;
        if mips == Mipmaps::Generate {
            assert_eq!(levels, 4);
        } else {
            assert_eq!(levels, 1);
        }

        publish(&mut assets, "atlas", |texture| {
            texture.update_region(region.x, region.y, region.width, region.height, &green).unwrap();
        });
        let stale = draw_fullscreen(&gpu, &mut renderer, &assets, "red", SpriteBlend::Cutout);
        let after_visible = renderer.resource_stats();
        assert_eq!(after_visible.partial_base_uploads, uploaded.partial_base_uploads + 1);
        assert_eq!(after_visible.full_base_uploads, uploaded.full_base_uploads);
        assert_eq!(after_visible.base_upload_bytes, uploaded.base_upload_bytes + 4);
        assert_eq!(after_visible.texture_creates, uploaded.texture_creates);
        assert_eq!(after_visible.sampler_creates, uploaded.sampler_creates);
        assert_eq!(after_visible.bind_group_creates, uploaded.bind_group_creates);
        expect_px(&stale, 64, 64, green, 8, "the visible texel updates");
        let stale_edge = px(&stale, 127, 64);
        assert!(
            stale_edge[0] > 80 && stale_edge[1] > 80 && stale_edge[2] < 20,
            "a visible-only write leaves the red gutter in the linear edge {stale_edge:?}"
        );
        let base = renderer.read_cached_mip(&gpu, handle.id(), 0).unwrap();
        expect_texel(&base, 8, region.x, region.y, green, 0, "GPU visible texel");
        expect_texel(&base, 8, region.x + 1, region.y, RED, 0, "GPU gutter is stale");
        expect_texel(&base, 8, blue.x, blue.y, BLUE, 0, "the other region is unchanged");
        if levels > 1 {
            assert_eq!(
                after_visible.mip_generations,
                uploaded.mip_generations + 1,
                "a partial base upload rebuilds the chain"
            );
        } else {
            assert_eq!(after_visible.mip_generations, uploaded.mip_generations);
        }

        publish(&mut assets, "atlas", |texture| {
            atlas.replace_source(texture, "red", &green).unwrap();
        });
        let fresh = draw_fullscreen(&gpu, &mut renderer, &assets, "red", SpriteBlend::Cutout);
        let after_replace = renderer.resource_stats();
        assert_eq!(after_replace.partial_base_uploads, after_visible.partial_base_uploads + 1);
        assert_eq!(after_replace.full_base_uploads, uploaded.full_base_uploads);
        assert_eq!(after_replace.base_upload_bytes, after_visible.base_upload_bytes + 3 * 3 * 4);
        assert_eq!(after_replace.texture_creates, uploaded.texture_creates);
        assert_eq!(after_replace.sampler_creates, uploaded.sampler_creates);
        assert_eq!(after_replace.bind_group_creates, uploaded.bind_group_creates);
        expect_px(&fresh, 64, 64, green, 4, "centre after the gutter refresh");
        expect_px(&fresh, 127, 64, green, 4, "the refreshed gutter no longer tints the edge");
        let base = renderer.read_cached_mip(&gpu, handle.id(), 0).unwrap();
        for y in region.y - 1..=region.y + 1 {
            for x in region.x - 1..=region.x + 1 {
                expect_texel(&base, 8, x, y, green, 0, "refreshed gutter");
            }
        }
        expect_texel(&base, 8, blue.x, blue.y, BLUE, 0, "blue survives the refresh");
        expect_texel(&base, 8, blue.x - 1, blue.y, BLUE, 0, "blue's gutter survives the refresh");
        let TexturePixelEdits::Journal(edits) = assets.get(handle).unwrap().pixel_edits() else {
            panic!("expected a journal");
        };
        assert_eq!(edits.last().unwrap().rect, TexelRect { x: region.x - 1, y: region.y - 1, width: 3, height: 3 });
        assert_eq!(atlas.region("red").unwrap().x, region.x);
        if levels > 1 {
            assert_eq!(after_replace.mip_generations, after_visible.mip_generations + 1);
            let (level1, width, _) = mip_image(&renderer, &gpu, handle.id(), 1);
            expect_texel(&level1, width, 0, 0, green, 2, "the rebuilt chain sees the new gutter");
        }
    }
}

#[test]
fn an_oversized_atlas_is_refused_and_a_missing_texture_stays_magenta() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = srgb_renderer(&gpu);
    capture::begin();
    renderer.set_texture_dimension_limits(Some(TextureDimensionLimits { max_width: 4, max_height: 4 }));
    let mut assets = AssetManager::in_memory();
    let atlas = pack(AtlasConfig::new(8, 4), &[("swatch", 2, 2, solid(2, 2, RED).as_slice())]);
    let creates = renderer.resource_stats().texture_creates;
    let handle = register(&mut assets, "wide-atlas", &atlas);
    let mut world =
        camera_at(Camera::orthographic(2.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -4.0));
    spawn_sprite(&mut world, "swatch", Transform::IDENTITY, Vec2::splat(2.0), SpriteBlend::Cutout);
    let image = render(&gpu, &mut renderer, &mut world, &assets);
    render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(px(&image, 64, 64), [255, 0, 255, 255]);
    assert_eq!(renderer.resource_stats().texture_creates, creates, "a rejected atlas allocates nothing");
    assert!(renderer.cached_texture(handle.id()).is_none());
    let warnings = capture::messages().into_iter().filter(|message| message.contains("wide-atlas")).collect::<Vec<_>>();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("was not uploaded") && warnings[0].contains("exceeds"), "{warnings:?}");

    capture::begin();
    let mut missing_assets = AssetManager::in_memory();
    let missing = AssetHandle::<Texture>::from_name("not-a-registered-atlas");
    let sprite = atlas.sprite("swatch", missing).unwrap();
    missing_assets.insert("packed", sprite).unwrap();
    let mut missing_world =
        camera_at(Camera::orthographic(2.0).with_clear_color(Color::BLACK), Transform::from_xyz(0.0, 0.0, -4.0));
    missing_world.insert_resource(MissingAssetWarnings::default());
    spawn_sprite(&mut missing_world, "packed", Transform::IDENTITY, Vec2::splat(2.0), SpriteBlend::Cutout);
    let image = render(&gpu, &mut renderer, &mut missing_world, &missing_assets);
    render(&gpu, &mut renderer, &mut missing_world, &missing_assets);
    assert_eq!(px(&image, 64, 64), [255, 0, 255, 255]);
    let records = capture::records();
    let warnings =
        records.iter().filter(|record| record.message.contains("missing sprite texture")).collect::<Vec<_>>();
    assert_eq!(warnings.len(), 1, "{:?}", records.iter().map(|record| &record.message).collect::<Vec<_>>());
    assert_eq!(warnings[0].level, log::Level::Warn);
    assert!(warnings[0].message.contains(&format!("{:016x}", missing.id().0)));
    assert!(warnings[0].message.contains("referenced by sprite 'packed'"));
    assert!(!warnings[0].message.contains("not-a-registered-atlas"));
}
