//! Release timings for sprite, mesh, mixed, and multi-camera extraction.
//!
//! `extract_median_ns` is `extract_scene` plus dropping the `RenderScene`, after warmup.
//! `first_extract_ns` is the call before warmup. `propagate_median_ns` is a later
//! `propagate_transforms` on an already-propagated world. `motion_prepare_median_ns` is
//! zero except for the move case, where each sample adds 0.01 to every sprite's local X
//! and propagates. That prepare is not inside `extract_median_ns`.
//!
//! `alloc_bytes` counts allocator requests during one later extract, including a
//! reallocation at its new size, and does not subtract frees. `peak_extra_bytes` is the
//! high water of outstanding allocated bytes during that extract, minus the outstanding
//! bytes at the start. `retained_bytes` is the capacity of the returned scene's vectors.
//! None of these is resident memory or a GPU allocation.
//!
//! Visible, culled, and batch columns are summed across cameras. `transparent_batches`
//! is the combined blend sequence and overlaps the blended part of `sprite_batches`.
//! Sorting is inside the extract time and is not timed on its own. There is no GPU submit.
//!
//! `DIMORA_SCALE_CASES` is a comma-separated label filter. `DIMORA_SCALE_LIST=1` prints
//! labels and exits. Refuses a debug build.
//!
//! Rows at or above 100,000 authored sprites or meshes use 1 warmup and 7 samples.
//! Smaller rows use 4 warmup and 11 samples. The median is the middle sample. p95 is
//! nearest-rank.

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use engine_renderer::bench_scene::{self, Case, Summary};
use engine_renderer::extract_scene;
use engine_renderer::Sprite;
use engine_world::{propagate_transforms, Entity, World};

const WARMUP: usize = 4;
const SAMPLES: usize = 11;

static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
static ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);
static CURRENT: AtomicU64 = AtomicU64::new(0);
static WINDOW_BASE: AtomicU64 = AtomicU64::new(0);
static WINDOW_PEAK: AtomicU64 = AtomicU64::new(0);

struct CountingAlloc;

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            note_alloc(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        note_free(layout.size());
        unsafe { System.dealloc(ptr, layout) };
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !ptr.is_null() {
            note_free(layout.size());
            note_alloc(new_size);
        }
        ptr
    }
}

fn note_alloc(bytes: usize) {
    let now = CURRENT.fetch_add(bytes as u64, Ordering::Relaxed) + bytes as u64;
    ALLOC_BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
    ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
    raise_peak(now);
}

fn note_free(bytes: usize) {
    CURRENT.fetch_sub(bytes as u64, Ordering::Relaxed);
}

fn raise_peak(now: u64) {
    let mut peak = WINDOW_PEAK.load(Ordering::Relaxed);
    while now > peak {
        match WINDOW_PEAK.compare_exchange_weak(peak, now, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(seen) => peak = seen,
        }
    }
}

fn begin_window() {
    ALLOC_BYTES.store(0, Ordering::Relaxed);
    ALLOC_COUNT.store(0, Ordering::Relaxed);
    let current = CURRENT.load(Ordering::Relaxed);
    WINDOW_BASE.store(current, Ordering::Relaxed);
    WINDOW_PEAK.store(current, Ordering::Relaxed);
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

fn main() {
    if cfg!(debug_assertions) {
        eprintln!("render_scale_bench: refusing a debug build; use cargo run --release");
        std::process::exit(2);
    }
    let cases = selected(false);
    let (cuboid, sphere) = bench_scene::reference_mesh_indices();
    println!(
        "render_scale_bench viewport={}x{} cuboid_indices={cuboid} sphere_indices={sphere} sprite_instance_bytes={} mesh_instance_bytes={}",
        bench_scene::VIEWPORT_WIDTH,
        bench_scene::VIEWPORT_HEIGHT,
        std::mem::size_of::<engine_renderer::SpriteInstance>(),
        std::mem::size_of::<engine_renderer::MeshInstance>(),
    );
    println!(
        "label sprites meshes lights tiles cameras passes visible_sprites culled_sprites visible_meshes culled_meshes tiles_visited chunks_tested chunks_rejected chunks_rebuilt sprite_batches mesh_draws transparent_batches lights_min lights_max missing_assets debug_segments first_extract_ns extract_median_ns extract_p95_ns propagate_median_ns motion_prepare_median_ns alloc_bytes alloc_count peak_extra_bytes retained_bytes samples"
    );
    if let Some(case) = cases.first().copied() {
        let built = (case.build)();
        for _ in 0..2 {
            let scene = extract_scene(&built.world, &built.assets, bench_scene::viewport());
            black_box(&scene);
        }
    }
    for case in cases {
        report(&case);
    }
}

fn report(case: &Case) {
    let mut built = (case.build)();
    let (warmup, samples) = repetitions(built.sprites, built.meshes, built.cameras);
    let viewport = bench_scene::viewport();
    let first_extract_ns = once_ns(|| {
        let scene = extract_scene(&built.world, &built.assets, viewport);
        black_box(&scene);
    });
    let propagate_median_ns = median_ns(warmup, samples, || {
        propagate_transforms(&mut built.world);
    });
    let motion_prepare_median_ns = if case.motion {
        let ids = sprite_ids(&built.world);
        median_ns(warmup, samples, || {
            nudge(&mut built.world, &ids, 0.01);
            propagate_transforms(&mut built.world);
        })
    } else {
        0
    };
    let (extract_median_ns, extract_p95_ns) = median_p95(warmup, samples, || {
        let scene = extract_scene(&built.world, &built.assets, viewport);
        black_box(&scene);
    });
    begin_window();
    let scene = extract_scene(&built.world, &built.assets, viewport).expect("camera");
    black_box(&scene);
    let summary = bench_scene::summarize(&scene);
    let alloc_bytes = ALLOC_BYTES.load(Ordering::Relaxed);
    let alloc_count = ALLOC_COUNT.load(Ordering::Relaxed);
    let peak_extra_bytes = WINDOW_PEAK.load(Ordering::Relaxed).saturating_sub(WINDOW_BASE.load(Ordering::Relaxed));
    drop(scene);
    print_row(
        case,
        &built,
        &summary,
        first_extract_ns,
        extract_median_ns,
        extract_p95_ns,
        propagate_median_ns,
        motion_prepare_median_ns,
        alloc_bytes,
        alloc_count,
        peak_extra_bytes,
        samples,
    );
}

#[allow(clippy::too_many_arguments)] // one argument per output column
fn print_row(
    case: &Case,
    built: &bench_scene::Built,
    summary: &Summary,
    first_extract_ns: u64,
    extract_median_ns: u64,
    extract_p95_ns: u64,
    propagate_median_ns: u64,
    motion_prepare_median_ns: u64,
    alloc_bytes: u64,
    alloc_count: u64,
    peak_extra_bytes: u64,
    samples: usize,
) {
    println!(
        "{} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {}",
        case.label,
        built.sprites,
        built.meshes,
        built.lights,
        built.tiles,
        built.cameras,
        summary.passes,
        summary.visible_sprites,
        summary.culled_sprites,
        summary.visible_meshes,
        summary.culled_meshes,
        summary.tiles_visited,
        summary.chunks_tested,
        summary.chunks_rejected,
        summary.chunks_rebuilt,
        summary.sprite_batches,
        summary.mesh_draws,
        summary.transparent_batches,
        summary.lights_min,
        summary.lights_max,
        summary.missing_assets,
        summary.debug_segments,
        first_extract_ns,
        extract_median_ns,
        extract_p95_ns,
        propagate_median_ns,
        motion_prepare_median_ns,
        alloc_bytes,
        alloc_count,
        peak_extra_bytes,
        summary.retained_bytes,
        samples,
    );
}

fn sprite_ids(world: &World) -> Vec<Entity> {
    world.query::<&Sprite>().iter().map(|(entity, _)| entity).collect()
}

fn nudge(world: &mut World, ids: &[Entity], dx: f32) {
    for id in ids {
        if let Ok(mut transform) = world.component_mut::<engine_math::Transform>(*id) {
            transform.position.x += dx;
        }
    }
}

fn repetitions(sprites: u32, meshes: u32, cameras: u32) -> (usize, usize) {
    let work = sprites.max(meshes).saturating_mul(cameras.max(1));
    if work >= 100_000 {
        (1, 7)
    } else {
        (WARMUP, SAMPLES)
    }
}

fn selected(gpu_only: bool) -> Vec<Case> {
    let all = bench_scene::cases();
    if std::env::var("DIMORA_SCALE_LIST").ok().as_deref() == Some("1") {
        for case in &all {
            println!("{}", case.label);
        }
        std::process::exit(0);
    }
    let Some(raw) = std::env::var("DIMORA_SCALE_CASES").ok() else {
        return if gpu_only { all.into_iter().filter(|case| case.gpu).collect() } else { all };
    };
    if raw.trim().is_empty() {
        return if gpu_only { all.into_iter().filter(|case| case.gpu).collect() } else { all };
    }
    let mut chosen = Vec::new();
    for name in raw.split(',').map(str::trim).filter(|name| !name.is_empty()) {
        match all.iter().find(|case| case.label == name) {
            Some(case) => chosen.push(*case),
            None => {
                eprintln!("unknown case {name}");
                eprintln!("known: {}", all.iter().map(|case| case.label).collect::<Vec<_>>().join(","));
                std::process::exit(2);
            }
        }
    }
    chosen
}

fn once_ns(mut body: impl FnMut()) -> u64 {
    let started = Instant::now();
    body();
    started.elapsed().as_nanos() as u64
}

fn median_ns(warmup: usize, samples: usize, body: impl FnMut()) -> u64 {
    let (median, _) = median_p95(warmup, samples, body);
    median
}

fn median_p95(warmup: usize, sample_count: usize, mut body: impl FnMut()) -> (u64, u64) {
    for _ in 0..warmup {
        body();
    }
    let mut taken = Vec::with_capacity(sample_count);
    for _ in 0..sample_count {
        let started = Instant::now();
        body();
        taken.push(started.elapsed().as_nanos() as u64);
    }
    taken.sort_unstable();
    let median = taken[taken.len() / 2];
    let rank = ((0.95 * taken.len() as f64).ceil() as usize).saturating_sub(1);
    (median, taken[rank.min(taken.len() - 1)])
}
