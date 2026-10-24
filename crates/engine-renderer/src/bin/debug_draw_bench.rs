//! Release timings for debug-line submission and extraction. No GPU, no extra crates.
//!
//! Generation is the `DebugDraw` calls. Extraction is `extract_scene` after the resource is
//! already filled, including dropping the `RenderScene`. Allocation figures are one extra
//! extract after the timed samples. They count allocator requests, including a reallocation at
//! its new size, and they do not subtract frees. Implied draws are 0 when there are no segments
//! and otherwise one line draw per camera. This process does not submit those draws.
//!
//! Refuses a debug build so a profile mix cannot be recorded as a measurement.

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use engine_assets::AssetManager;
use engine_core::Color;
use engine_math::{Transform, Vec2, Vec3};
use engine_renderer::{extract_scene, Camera, DebugDraw};
use engine_world::{propagate_transforms, World};

const VIEWPORT: Vec2 = Vec2::new(1280.0, 720.0);
const WARMUP: usize = 4;
const SAMPLES: usize = 11;

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

fn main() {
    if cfg!(debug_assertions) {
        eprintln!("debug_draw_bench: refusing a debug build; use cargo run --release");
        std::process::exit(2);
    }

    println!("debug_draw_bench warmup={WARMUP} samples={SAMPLES} viewport={}x{}", VIEWPORT.x, VIEWPORT.y);
    println!("segments cameras vertices implied_draws gen_ns extract_ns alloc_bytes alloc_count");

    for cameras in [1, 2] {
        report_extract("absent", 0, cameras, None, 0);
        report_extract("empty", 0, cameras, Some(DebugDraw::default()), 0);
    }
    for segments in [0, 100, 1_000, 10_000] {
        for cameras in [1, 2] {
            let (draw, gen_ns) = timed_fill(segments);
            report_extract(&segments.to_string(), segments, cameras, Some(draw), gen_ns);
        }
    }
}

fn report_extract(label: &str, segments: usize, cameras: usize, draw: Option<DebugDraw>, gen_ns: u64) {
    let assets = AssetManager::in_memory();
    let mut world = world(cameras);
    if let Some(draw) = draw {
        world.insert_resource(draw);
    }
    let extract_ns = median_ns(|| {
        let scene = extract_scene(&world, &assets, VIEWPORT);
        black_box(&scene);
    });
    ALLOC_BYTES.store(0, Ordering::Relaxed);
    ALLOC_COUNT.store(0, Ordering::Relaxed);
    let scene = extract_scene(&world, &assets, VIEWPORT);
    black_box(&scene);
    drop(scene);
    let alloc_bytes = ALLOC_BYTES.load(Ordering::Relaxed);
    let alloc_count = ALLOC_COUNT.load(Ordering::Relaxed);
    let vertices = segments * 2;
    let implied_draws = if segments == 0 { 0 } else { cameras };
    println!("{label} {cameras} {vertices} {implied_draws} {gen_ns} {extract_ns} {alloc_bytes} {alloc_count}");
}

fn timed_fill(segments: usize) -> (DebugDraw, u64) {
    let mut samples = Vec::with_capacity(SAMPLES);
    let mut last = DebugDraw::default();
    for index in 0..WARMUP + SAMPLES {
        let started = Instant::now();
        last = fill(segments);
        let elapsed = started.elapsed().as_nanos() as u64;
        black_box(last.len());
        if index >= WARMUP {
            samples.push(elapsed);
        }
    }
    (last, median(&mut samples))
}

fn fill(segments: usize) -> DebugDraw {
    let mut draw = DebugDraw::default();
    let color = Color::rgb(1.0, 0.2, 0.1);
    for index in 0..segments {
        let x = index as f32 * 0.01;
        draw.line(Vec3::new(x, 0.0, 0.0), Vec3::new(x, 1.0, 0.0), color);
    }
    draw
}

fn world(cameras: usize) -> World {
    let mut world = World::new();
    for order in 0..cameras {
        world.spawn((
            Camera { order: order as i32, ..Camera::orthographic(20.0) },
            Transform::from_xyz(0.0, 1.0, -30.0),
        ));
    }
    propagate_transforms(&mut world);
    world
}

fn median_ns(mut body: impl FnMut()) -> u64 {
    for _ in 0..WARMUP {
        body();
    }
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        body();
        samples.push(started.elapsed().as_nanos() as u64);
    }
    median(&mut samples)
}

fn median(samples: &mut [u64]) -> u64 {
    samples.sort_unstable();
    samples[samples.len() / 2]
}
