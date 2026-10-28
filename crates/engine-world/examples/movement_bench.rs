//! Release timings for `sync_collision`, including trigger maintenance.
//!
//! `cargo run --release -p engine-world --example movement_bench`
//!
//! The `sweep N` rows match the M7-1B-1G fixture: one sphere moves 30 units along +X into a
//! blocking box, and the other colliders are solids past that box. A world with no sensors skips
//! the trigger pair pass. The `sensors N` rows replace those extra solids with sensors standing
//! on the committed path, before the box. The `cross` row has no box: one motor passes through
//! every sensor. Warmup is 10 calls and the reported value is the median of 31. The mover is
//! placed back at the origin between samples; that reset is not timed. This is a linear scan,
//! not a frame-time budget, and it says nothing about the GPU.

use std::time::Instant;

use engine_math::{Transform, Vec3};
use engine_world::{sync_collision, CharacterMotor, Collider, CollisionWorld, MoveIntent, World};

const WARMUP: usize = 10;
const SAMPLES: usize = 31;

fn main() {
    if cfg!(debug_assertions) {
        eprintln!("movement_bench: refusing a debug build; rerun with --release");
        std::process::exit(1);
    }
    println!("movement bench");
    time_case("sweep 10", scene(10));
    time_case("sweep 100", scene(100));
    time_case("sensors 10", sensor_scene(10, true));
    time_case("sensors 100", sensor_scene(100, true));
    time_case("cross 8", sensor_scene(9, false));
}

fn time_case(label: &str, mut world: World) {
    let mover = *world.resource::<CollisionWorld>().entities().first().unwrap();
    for _ in 0..WARMUP {
        rewind(&mut world, mover);
        sync_collision(&mut world);
    }
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        rewind(&mut world, mover);
        let started = Instant::now();
        sync_collision(&mut world);
        samples.push(started.elapsed().as_secs_f64() * 1.0e6);
        std::hint::black_box(world.component::<Transform>(mover).unwrap().position);
    }
    samples.sort_by(|left, right| left.partial_cmp(right).unwrap());
    let median = samples[SAMPLES / 2];
    let indexed = world.resource::<CollisionWorld>().len();
    let stopped = world.component::<Transform>(mover).unwrap().position.x;
    println!("{label}: median {median:.1} us, indexed {indexed}, stopped x {stopped:.2}");
}

fn rewind(world: &mut World, mover: engine_world::Entity) {
    world.component_mut::<Transform>(mover).unwrap().position = Vec3::ZERO;
    let _ = world.insert(mover, (MoveIntent::new(Vec3::new(30.0, 0.0, 0.0)),));
}

fn scene(count: usize) -> World {
    let mut world = World::new();
    world.spawn((
        Transform::IDENTITY,
        Collider::sphere(0.4),
        CharacterMotor { skin: 0.01, ..CharacterMotor::default() },
    ));
    world.spawn((Transform::from_xyz(12.0, 0.0, 0.0), Collider::aabb_box(Vec3::new(0.5, 3.0, 3.0))));
    let extras = count.saturating_sub(2);
    for index in 0..extras {
        let x = 14.0 + index as f32 * 0.15;
        world.spawn((Transform::from_xyz(x, 0.0, 0.0), Collider::sphere(0.4)));
    }
    sync_collision(&mut world);
    world
}

/// `blocked` keeps the wall and places sensors on the path that is actually applied.
/// Without it, every extra collider is a thin sensor and the motor runs the full 30 units.
fn sensor_scene(count: usize, blocked: bool) -> World {
    let mut world = World::new();
    world.spawn((
        Transform::IDENTITY,
        Collider::sphere(0.4),
        CharacterMotor { skin: 0.01, ..CharacterMotor::default() },
    ));
    let extras = if blocked {
        let _ = world.spawn((Transform::from_xyz(12.0, 0.0, 0.0), Collider::aabb_box(Vec3::new(0.5, 3.0, 3.0))));
        count.saturating_sub(2)
    } else {
        count.saturating_sub(1)
    };
    for index in 0..extras {
        let x = 1.0 + index as f32 * (if blocked { 9.0 } else { 20.0 }) / extras.max(1) as f32;
        world.spawn((Transform::from_xyz(x, 0.0, 0.0), Collider::aabb_box(Vec3::new(0.05, 1.0, 1.0)).sensor()));
    }
    sync_collision(&mut world);
    world
}
