//! CPU encode/submit time and driver completion wait for the M6 scale scenes.
//!
//! `encode_*` is [`SceneRenderer::render`](engine_renderer_wgpu::SceneRenderer::render) after the
//! previous sample's wait, so the queue is idle. That call records command buffers and submits
//! them. It does not map a readback and it does not present.
//!
//! `completion_*` is the following `device.poll(wait_indefinitely)`. That waits until submitted
//! work finishes. It includes GPU execution and driver synchronization. It is not a timestamp
//! query inside the render pass, and it is not a pure shader time. If the wait fails, the column
//! is `unmeasured`.
//!
//! `first_encode_ns` is the first `render` after pipeline creation and includes texture and mesh
//! uploads and instance-buffer growth. Steady samples start after that frame and two more warmup
//! frames. `steady_buffer_grows` counts grows during the timed samples only.
//!
//! `alloc_bytes` is Rust global-allocator traffic during one steady `render`, using the same
//! request rules as `render_scale_bench`. It does not include Metal's own heaps.
//! `instance_capacity_bytes` is the renderer's reserved instance and debug-vertex buffers.
//!
//! `DIMORA_SCALE_CASES` selects labels. Without it, only cases marked for the GPU harness run.
//! `DIMORA_SCALE_LIST=1` prints every label. Refuses a debug build. Rows at or above 100,000
//! authored sprites or meshes use 1 warmup and 7 samples; smaller rows use 2 warmup and 11.

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use engine_renderer::bench_scene::{self, Case};
use engine_renderer::extract_scene;
use engine_renderer_wgpu::{GpuContext, SceneRenderer};

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
    let mut peak = WINDOW_PEAK.load(Ordering::Relaxed);
    while now > peak {
        match WINDOW_PEAK.compare_exchange_weak(peak, now, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(seen) => peak = seen,
        }
    }
}

fn note_free(bytes: usize) {
    CURRENT.fetch_sub(bytes as u64, Ordering::Relaxed);
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
        eprintln!("render_scale_gpu: refusing a debug build; use cargo run --release");
        std::process::exit(2);
    }
    let cases = selected();
    let gpu = GpuContext::headless().unwrap_or_else(|error| {
        eprintln!("render_scale_gpu: {error}");
        std::process::exit(1);
    });
    let info = gpu.adapter.get_info();
    println!(
        "render_scale_gpu adapter={} backend={:?} viewport={}x{} format=Rgba8Unorm",
        info.name,
        info.backend,
        bench_scene::VIEWPORT_WIDTH,
        bench_scene::VIEWPORT_HEIGHT
    );
    println!(
        "label encode_median_ns encode_p95_ns completion_median_ns completion_p95_ns first_encode_ns draw_calls world_draw_calls clear_draw_calls camera_passes offscreen_passes sprite_instances mesh_instances submitted_instances instance_capacity_bytes steady_buffer_grows texture_creates render_target_creates base_upload_bytes mesh_cache alloc_bytes alloc_count peak_extra_bytes samples"
    );
    if let Some(case) = cases.first().copied() {
        let built = (case.build)();
        let scene = extract_scene(&built.world, &built.assets, bench_scene::viewport());
        if let Some(scene) = scene {
            let mut renderer = SceneRenderer::new(&gpu, wgpu::TextureFormat::Rgba8Unorm);
            let target = color_target(&gpu);
            renderer.render(
                &gpu,
                &scene,
                &built.assets,
                &target.1,
                (bench_scene::VIEWPORT_WIDTH as u32, bench_scene::VIEWPORT_HEIGHT as u32),
            );
            let _ = wait(&gpu);
        }
    }
    for case in cases {
        report(&gpu, &case);
    }
}

fn color_target(gpu: &GpuContext) -> (wgpu::Texture, wgpu::TextureView) {
    let (width, height) = (bench_scene::VIEWPORT_WIDTH as u32, bench_scene::VIEWPORT_HEIGHT as u32);
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("scale color"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

fn report(gpu: &GpuContext, case: &Case) {
    let built = (case.build)();
    let viewport = bench_scene::viewport();
    let scene = extract_scene(&built.world, &built.assets, viewport).expect("camera");
    let mut renderer = SceneRenderer::new(gpu, wgpu::TextureFormat::Rgba8Unorm);
    let (width, height) = (bench_scene::VIEWPORT_WIDTH as u32, bench_scene::VIEWPORT_HEIGHT as u32);
    let target = color_target(gpu);
    let (warmup, samples) = repetitions(built.sprites, built.meshes, built.cameras);
    let first_encode_ns = once_ns(|| {
        renderer.render(gpu, &scene, &built.assets, &target.1, (width, height));
        black_box(renderer.stats());
    });
    let first_wait_ok = wait(gpu);
    for _ in 0..warmup {
        renderer.render(gpu, &scene, &built.assets, &target.1, (width, height));
        let _ = wait(gpu);
    }
    let grows_before = renderer.resource_stats().instance_buffer_grows;
    let mut encode = Vec::with_capacity(samples);
    let mut completion = Vec::new();
    let mut completion_ok = first_wait_ok;
    for _ in 0..samples {
        let _ = wait(gpu);
        encode.push(once_ns(|| {
            renderer.render(gpu, &scene, &built.assets, &target.1, (width, height));
            black_box(renderer.stats());
        }));
        let started = Instant::now();
        if wait(gpu) {
            completion.push(started.elapsed().as_nanos() as u64);
        } else {
            completion_ok = false;
        }
    }
    begin_window();
    renderer.render(gpu, &scene, &built.assets, &target.1, (width, height));
    let alloc_bytes = ALLOC_BYTES.load(Ordering::Relaxed);
    let alloc_count = ALLOC_COUNT.load(Ordering::Relaxed);
    let peak_extra_bytes = WINDOW_PEAK.load(Ordering::Relaxed).saturating_sub(WINDOW_BASE.load(Ordering::Relaxed));
    let _ = wait(gpu);
    encode.sort_unstable();
    let stats = renderer.stats();
    let resources = renderer.resource_stats();
    let completion_median = if completion_ok && !completion.is_empty() {
        completion.sort_unstable();
        completion[completion.len() / 2].to_string()
    } else {
        "unmeasured".to_string()
    };
    let completion_p95 = if completion_ok && !completion.is_empty() {
        let rank = ((0.95 * completion.len() as f64).ceil() as usize).saturating_sub(1);
        completion[rank.min(completion.len() - 1)].to_string()
    } else {
        "unmeasured".to_string()
    };
    println!(
        "{} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {}",
        case.label,
        encode[encode.len() / 2],
        encode[((0.95 * encode.len() as f64).ceil() as usize).saturating_sub(1)],
        completion_median,
        completion_p95,
        first_encode_ns,
        stats.draw_calls,
        stats.world_draw_calls,
        stats.clear_draw_calls,
        stats.camera_passes,
        stats.offscreen_passes,
        stats.sprite_instances,
        stats.mesh_instances,
        stats.submitted_instances,
        renderer.instance_buffer_capacity_bytes(),
        resources.instance_buffer_grows.saturating_sub(grows_before),
        resources.texture_creates,
        resources.render_target_creates,
        resources.base_upload_bytes,
        renderer.cache_stats().meshes,
        alloc_bytes,
        alloc_count,
        peak_extra_bytes,
        samples,
    );
}

fn wait(gpu: &GpuContext) -> bool {
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).is_ok()
}

fn repetitions(sprites: u32, meshes: u32, cameras: u32) -> (usize, usize) {
    let work = sprites.max(meshes).saturating_mul(cameras.max(1));
    if work >= 100_000 {
        (1, 7)
    } else {
        (2, 11)
    }
}

fn once_ns(mut body: impl FnMut()) -> u64 {
    let started = Instant::now();
    body();
    started.elapsed().as_nanos() as u64
}

fn selected() -> Vec<Case> {
    let all = bench_scene::cases();
    if std::env::var("DIMORA_SCALE_LIST").ok().as_deref() == Some("1") {
        for case in &all {
            println!("{}", case.label);
        }
        std::process::exit(0);
    }
    let Some(raw) = std::env::var("DIMORA_SCALE_CASES").ok() else {
        return all.into_iter().filter(|case| case.gpu).collect();
    };
    if raw.trim().is_empty() {
        return all.into_iter().filter(|case| case.gpu).collect();
    }
    let mut chosen = Vec::new();
    for name in raw.split(',').map(str::trim).filter(|name| !name.is_empty()) {
        match all.iter().find(|case| case.label == name) {
            Some(case) => chosen.push(*case),
            None => {
                eprintln!("unknown case {name}");
                std::process::exit(2);
            }
        }
    }
    chosen
}
