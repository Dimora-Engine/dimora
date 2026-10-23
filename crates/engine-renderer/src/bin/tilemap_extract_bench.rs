//! Release timings for tilemap extraction. No GPU, no extra crates.
//!
//! The timed region is `extract_scene` plus dropping the `RenderScene` it returns.
//! `propagate_transforms` is timed separately and is not inside that region.
//! Allocation figures are one extra extract after the timed samples. They count
//! allocator requests, including a reallocation at its new size, and they do not
//! subtract frees.
//!
//! `DIMORA_BENCH_SIZES` restricts the grid, for example `64,256`. The default is
//! `64,256,512,1000`. Refuses a debug build so a profile mix cannot be recorded
//! as a baseline.
//!
//! Cells scanned are `width * height * cameras`. That is the current extractor:
//! every cell is visited, and an empty cell does not increment `sprites_culled`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use engine_assets::{AssetManager, Texture};
use engine_math::{Quat, Transform, Vec2, Vec3};
use engine_renderer::{extract_scene, Camera, CameraProjection, TileMap};
use engine_world::{propagate_transforms, World};

const VIEWPORT: Vec2 = Vec2::new(1280.0, 720.0);
const ISOMETRIC_PITCH: f32 = 0.615_479_7;
const PERSPECTIVE_FOV: f32 = 50.0;

// The map is rotated 90 degrees about X onto the ground and centered on the origin.
// Mostly-visible cameras sit outside the map's bounding sphere and use an orthographic
// height of 1.6 map-widths, or a 50 degree perspective from 2.2 widths away.
// Partial cameras use height 0.28 widths, or a perspective distance of 0.38 widths,
// and look at a point on a ring 0.18 widths from the origin. Extra cameras orbit.

static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
static ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);

struct CountingAlloc;

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            note(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !ptr.is_null() {
            note(new_size);
        }
        ptr
    }
}

fn note(bytes: usize) {
    ALLOC_BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
    ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

#[derive(Clone, Copy)]
enum View {
    Perspective,
    Orthographic,
    Isometric,
}

#[derive(Clone, Copy)]
enum Coverage {
    Mostly,
    Partial,
}

#[derive(Clone, Copy)]
enum Density {
    Dense,
    Sparse,
}

impl View {
    fn name(self) -> &'static str {
        match self {
            Self::Perspective => "perspective",
            Self::Orthographic => "orthographic",
            Self::Isometric => "isometric",
        }
    }

    fn pitch(self) -> f32 {
        match self {
            Self::Perspective => 0.65,
            Self::Orthographic => 0.55,
            Self::Isometric => ISOMETRIC_PITCH,
        }
    }

    fn yaw(self, index: u32, count: u32) -> f32 {
        let orbit = if count <= 1 { 0.0 } else { index as f32 * std::f32::consts::TAU / count as f32 };
        let base = if matches!(self, Self::Isometric) { std::f32::consts::FRAC_PI_4 } else { 0.0 };
        base + orbit
    }
}

impl Coverage {
    fn name(self) -> &'static str {
        match self {
            Self::Mostly => "mostly",
            Self::Partial => "partial",
        }
    }
}

impl Density {
    fn name(self) -> &'static str {
        match self {
            Self::Dense => "dense",
            Self::Sparse => "sparse",
        }
    }
}

fn main() {
    if cfg!(debug_assertions) {
        eprintln!("tilemap_extract_bench: refusing a debug build; use cargo run --release");
        std::process::exit(2);
    }

    let sizes = selected_sizes();
    println!(
        "size,cameras,view,coverage,density,populated,samples,median_us,p95_us,min_us,max_us,\
visible,culled,instances,batches,cells_scanned,sim_median_us,alloc_count,alloc_bytes,visible_fraction"
    );
    eprintln!("tilemap extract bench: release, viewport {}x{}, sizes {:?}", VIEWPORT.x, VIEWPORT.y, sizes);

    let assets = assets();
    for size in sizes {
        for cameras in [1u32, 2, 7] {
            for view in [View::Perspective, View::Orthographic, View::Isometric] {
                for coverage in [Coverage::Mostly, Coverage::Partial] {
                    for density in [Density::Dense, Density::Sparse] {
                        run_case(&assets, size, cameras, view, coverage, density);
                    }
                }
            }
        }
    }
}

fn selected_sizes() -> Vec<u32> {
    match std::env::var("DIMORA_BENCH_SIZES") {
        Ok(raw) => {
            let sizes: Vec<u32> = raw.split(',').filter_map(|part| part.trim().parse().ok()).collect();
            if sizes.is_empty() {
                default_sizes()
            } else {
                sizes
            }
        }
        Err(_) => default_sizes(),
    }
}

fn default_sizes() -> Vec<u32> {
    vec![64, 256, 512, 1000]
}

fn assets() -> AssetManager {
    let mut assets = AssetManager::in_memory();
    assets.insert("tiles", Texture::solid(64, 64, [255, 255, 255, 255])).expect("tile texture");
    assets
}

fn run_case(assets: &AssetManager, size: u32, cameras: u32, view: View, coverage: Coverage, density: Density) {
    eprint!("{size}x{size} {} {} {} cams={cameras} ... ", view.name(), coverage.name(), density.name());
    let (world, populated) = world_for(size, cameras, view, coverage, density);
    let sim = time_propagate(world);
    let (warmup, samples) = sample_count(size);
    for _ in 0..warmup {
        black_box(extract_scene(&sim.world, assets, VIEWPORT).expect("cameras"));
    }

    let mut times = Vec::with_capacity(samples);
    for _ in 0..samples {
        let start = Instant::now();
        let extracted = extract_scene(&sim.world, assets, VIEWPORT).expect("cameras");
        let extracted = black_box(extracted);
        drop(extracted);
        times.push(start.elapsed().as_secs_f64() * 1e6);
    }
    times.sort_by(f64::total_cmp);

    ALLOC_BYTES.store(0, Ordering::Relaxed);
    ALLOC_COUNT.store(0, Ordering::Relaxed);
    let scene = extract_scene(&sim.world, assets, VIEWPORT).expect("cameras");
    let alloc_bytes = ALLOC_BYTES.load(Ordering::Relaxed);
    let alloc_count = ALLOC_COUNT.load(Ordering::Relaxed);

    let mut visible = 0u32;
    let mut culled = 0u32;
    let mut instances = 0usize;
    let mut batches = 0usize;
    for pass in &scene.passes {
        visible += pass.stats.sprites_visible;
        culled += pass.stats.sprites_culled;
        instances += pass.cutout_sprites.instances.len();
        batches += pass.cutout_sprites.batches.len();
        instances += pass.opaque_sprites.instances.len();
        batches += pass.opaque_sprites.batches.len();
        instances += pass.mask_sprites.instances.len();
        batches += pass.mask_sprites.batches.len();
        instances += pass.blended_sprites.instances.len();
        batches += pass.blended_sprites.batches.len();
        instances += pass.meshes.len();
        instances += pass.blended_meshes.len();
    }
    let cells = size as u64 * size as u64 * cameras as u64;
    let denom = populated.saturating_mul(cameras as u64).max(1) as f64;
    let fraction = visible as f64 / denom;

    println!(
        "{size},{cameras},{},{},{},{populated},{samples},{:.1},{:.1},{:.1},{:.1},{visible},{culled},{instances},{batches},{cells},{:.1},{alloc_count},{alloc_bytes},{fraction:.4}",
        view.name(),
        coverage.name(),
        density.name(),
        median(&times),
        percentile(&times, 0.95),
        times[0],
        times[times.len() - 1],
        sim.median_us,
    );
    eprintln!("median {:.1} us, visible {:.1}%", median(&times), fraction * 100.0);
}

struct Prepared {
    world: World,
    median_us: f64,
}

fn time_propagate(mut world: World) -> Prepared {
    propagate_transforms(&mut world);
    let mut times = Vec::with_capacity(5);
    for _ in 0..5 {
        let start = Instant::now();
        propagate_transforms(&mut world);
        times.push(start.elapsed().as_secs_f64() * 1e6);
    }
    times.sort_by(f64::total_cmp);
    Prepared { world, median_us: median(&times) }
}

fn sample_count(size: u32) -> (usize, usize) {
    match size {
        0..=64 => (3, 21),
        65..=256 => (2, 15),
        257..=512 => (1, 9),
        _ => (1, 5),
    }
}

fn world_for(size: u32, cameras: u32, view: View, coverage: Coverage, density: Density) -> (World, u64) {
    let mut world = World::new();
    let n = size as f32;
    world.spawn((ground_transform(n), tilemap(size, density)));
    for index in 0..cameras {
        let mut camera = Camera::new(projection(view, n, coverage));
        camera.order = index as i32;
        world.spawn((camera, camera_transform(n, view, coverage, index, cameras)));
    }
    let populated = match density {
        Density::Dense => size as u64 * size as u64,
        Density::Sparse => sparse_count(size),
    };
    (world, populated)
}

fn ground_transform(n: f32) -> Transform {
    let rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    let center = rotation.rotate(Vec3::new(n * 0.5, n * 0.5, 0.0));
    Transform::from_position(-center).with_rotation(rotation)
}

fn tilemap(size: u32, density: Density) -> TileMap {
    let mut map = TileMap::new("tiles", 4, 4, Vec2::ONE, size, size);
    match density {
        Density::Dense => map.tiles.fill(0),
        Density::Sparse => {
            for y in 0..size {
                for x in 0..size {
                    if is_sparse(x, y) {
                        map.set(x, y, 0);
                    }
                }
            }
        }
    }
    map
}

fn is_sparse(x: u32, y: u32) -> bool {
    (x + y.wrapping_mul(3)).is_multiple_of(8)
}

fn sparse_count(size: u32) -> u64 {
    let mut count = 0u64;
    for y in 0..size {
        for x in 0..size {
            if is_sparse(x, y) {
                count += 1;
            }
        }
    }
    count
}

fn projection(view: View, n: f32, coverage: Coverage) -> CameraProjection {
    let far = n * 8.0 + 100.0;
    match view {
        View::Perspective => CameraProjection::perspective(PERSPECTIVE_FOV).with_clip_range(0.05, far),
        View::Orthographic | View::Isometric => {
            let height = match coverage {
                Coverage::Mostly => n * 1.6,
                Coverage::Partial => n * 0.28,
            };
            CameraProjection::orthographic(height).with_clip_range(0.05, far)
        }
    }
}

fn camera_transform(n: f32, view: View, coverage: Coverage, index: u32, count: u32) -> Transform {
    let target = match coverage {
        Coverage::Mostly => Vec3::ZERO,
        Coverage::Partial => {
            let angle = index as f32 * std::f32::consts::TAU / count.max(1) as f32;
            Vec3::new(angle.cos() * n * 0.18, 0.0, angle.sin() * n * 0.18)
        }
    };
    let distance = match (view, coverage) {
        (View::Perspective, Coverage::Mostly) => n * 2.2,
        (View::Perspective, Coverage::Partial) => n * 0.38,
        (_, Coverage::Mostly) => n * 1.8,
        (_, Coverage::Partial) => n * 0.95,
    };
    let rotation = Quat::from_rotation_y(view.yaw(index, count)) * Quat::from_rotation_x(view.pitch());
    let forward = Transform::from_rotation(rotation).forward();
    Transform::from_position(target - forward * distance).with_rotation(rotation)
}

fn median(sorted: &[f64]) -> f64 {
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) * 0.5
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    let rank = ((n as f64) * p).ceil() as usize;
    sorted[rank.saturating_sub(1).min(n - 1)]
}
