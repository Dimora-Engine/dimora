//! Release timings for one fixed collision phase, with debug wires off and on.
//!
//! `cargo run --release -p engine --example collision_phase_bench`
//!
//! The timed body is the same work as the sealed `sync_collision` system: propagate and rebuild,
//! then, only when [`engine::prelude::CollisionDebug`] is enabled, copy finite wire segments into
//! `DebugDraw`. Overlap and ray queries are not part of this phase. The index is still a linear
//! scan. These numbers are not a broadphase budget.

use std::time::Instant;

use engine::prelude::*;
use engine::world::sync_collision;

const WARMUP: usize = 10;
const SAMPLES: usize = 31;

fn main() {
    if cfg!(debug_assertions) {
        eprintln!("collision_phase_bench: refusing a debug build; rerun with --release");
        std::process::exit(1);
    }
    println!("collision phase bench");
    time_phase(10, false);
    time_phase(10, true);
    time_phase(100, false);
    time_phase(100, true);
}

fn time_phase(count: usize, debug: bool) {
    let mut world = grid(count);
    for _ in 0..WARMUP {
        world.resource_mut::<DebugDraw>().clear();
        collision_phase(&mut world, debug);
    }
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        world.resource_mut::<DebugDraw>().clear();
        let started = Instant::now();
        collision_phase(&mut world, debug);
        samples.push(started.elapsed().as_secs_f64() * 1.0e6);
        std::hint::black_box(world.resource::<DebugDraw>().len());
    }
    samples.sort_by(|left, right| left.partial_cmp(right).unwrap());
    let median = samples[SAMPLES / 2];
    let indexed = world.resource::<CollisionWorld>().len();
    let wires = world.resource::<DebugDraw>().len();
    let mode = if debug { "debug on" } else { "debug off" };
    println!("phase {count} {mode}: median {median:.1} us, indexed {indexed}, wires {wires}");
}

/// Matches `sync_collision_system` in `crates/engine/src/game.rs`.
fn collision_phase(world: &mut World, debug: bool) {
    world.resource_mut::<CollisionDebug>().enabled = debug;
    sync_collision(world);
    if !world.resource::<CollisionDebug>().enabled {
        return;
    }
    let segments = world.resource::<CollisionWorld>().wire_segments();
    let mut draw = world.resource_mut::<DebugDraw>();
    let color = Color::rgb(0.15, 0.85, 0.4);
    for (start, end) in segments {
        draw.line(start, end, color);
    }
}

fn grid(count: usize) -> World {
    let mut world = World::new();
    world.insert_resource(CollisionDebug::off());
    world.insert_resource(DebugDraw::default());
    for index in 0..count {
        world.spawn((Transform::from_xyz(index as f32 * 3.0, 0.0, 0.0), Collider::sphere(0.5)));
    }
    world
}
