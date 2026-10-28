//! Release timings for `sync_collision`, including trigger maintenance.
//!
//! `cargo run --release -p engine-world --example movement_bench`
//!
//! The `sweep N` rows match the M7-1B-1G fixture: one sphere moves 30 units along +X into a
//! blocking box, and the other colliders are solids past that box. A world with no sensors skips
//! the trigger pair pass. The `sensors N` rows replace those extra solids with sensors standing
//! on the committed path, before the box. The `cross` row has no box: one motor passes through
//! every sensor. `small`, `mixed`, `crowd`, and `dense` are capsule motors with real wall contacts
//! and, except `small`, real sensor crossings. Warmup is 10 calls. Each reported row is 31 samples.
//! The mover is placed back at its start between samples; that reset is not timed. The timed call
//! is `sync_collision` only. It is not the fixed-update stage, not a frame budget, and it says
//! nothing about the GPU. The timed loop does not clear the event queue, matching the earlier
//! harness. The printed event count is one later sync after that queue is cleared, so it is not
//! the sum of the warmup and the samples.

use std::time::Instant;

use engine_math::{Transform, Vec3};
use engine_world::{sync_collision, CharacterMotor, Collider, CollisionWorld, MoveIntent, TriggerEvent, World};

const WARMUP: usize = 10;
const SAMPLES: usize = 31;

struct Movers {
    entities: Vec<engine_world::Entity>,
    starts: Vec<Vec3>,
    displacement: Vec3,
}

fn main() {
    if cfg!(debug_assertions) {
        eprintln!("movement_bench: refusing a debug build; rerun with --release");
        std::process::exit(1);
    }
    println!("movement bench (sync_collision only)");
    time_case("sweep 10", scene(10));
    time_case("sweep 100", scene(100));
    time_case("sensors 10", sensor_scene(10, true));
    time_case("sensors 100", sensor_scene(100, true));
    time_case("cross 8", sensor_scene(9, false));
    time_case("small", crowd(10, 10, 0, 10, false));
    time_case("mixed", crowd(50, 20, 10, 30, false));
    time_case("crowd", crowd(100, 30, 20, 20, false));
    time_case("dense", crowd(100, 40, 20, 60, true));
}

fn time_case(label: &str, mut world: World) {
    for _ in 0..WARMUP {
        rewind(&mut world);
        sync_collision(&mut world);
    }
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        rewind(&mut world);
        let started = Instant::now();
        sync_collision(&mut world);
        samples.push(started.elapsed().as_secs_f64() * 1.0e6);
        let entity = world.resource::<Movers>().entities[0];
        std::hint::black_box(world.component::<Transform>(entity).unwrap().position);
    }
    samples.sort_by(|left, right| left.partial_cmp(right).unwrap());
    let stopped = stopped_x(&world);
    let events = count_events(&mut world);
    let (indexed, motors, sensors) = counts(&world);
    println!(
        "{label}: median {:.1} us, q1 {:.1}, q3 {:.1}, min {:.1}, max {:.1}, indexed {indexed}, motors {motors}, sensors {sensors}, events {events}, stopped x {:.2}, last x {:.2}",
        samples[SAMPLES / 2],
        samples[7],
        samples[23],
        samples[0],
        samples[SAMPLES - 1],
        stopped.0,
        stopped.1,
    );
}

fn stopped_x(world: &World) -> (f32, f32) {
    let first_id = world.resource::<Movers>().entities[0];
    let last_id = *world.resource::<Movers>().entities.last().unwrap();
    let first = world.component::<Transform>(first_id).unwrap().position.x;
    let last = world.component::<Transform>(last_id).unwrap().position.x;
    (first, last)
}

fn count_events(world: &mut World) -> usize {
    world.clear_events();
    rewind(world);
    sync_collision(world);
    world.promote_events();
    let count = world.events::<TriggerEvent>().len();
    world.clear_events();
    count
}

fn counts(world: &World) -> (usize, usize, usize) {
    let sensors = world.query::<&Collider>().iter().filter(|(_, collider)| collider.sensor).count();
    let motors = world.query::<&CharacterMotor>().iter().count();
    let indexed = world.resource::<CollisionWorld>().len();
    (indexed, motors, sensors)
}

fn rewind(world: &mut World) {
    let plan = world.remove_resource::<Movers>().expect("movers");
    for (entity, start) in plan.entities.iter().copied().zip(plan.starts.iter().copied()) {
        world.component_mut::<Transform>(entity).unwrap().position = start;
        let _ = world.insert(entity, (MoveIntent::new(plan.displacement),));
    }
    world.insert_resource(plan);
}

fn remember(world: &mut World, entities: Vec<engine_world::Entity>, starts: Vec<Vec3>, displacement: Vec3) {
    world.insert_resource(Movers { entities, starts, displacement });
}

fn scene(count: usize) -> World {
    let mut world = World::new();
    let mover = world.spawn((
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
    remember(&mut world, vec![mover], vec![Vec3::ZERO], Vec3::new(30.0, 0.0, 0.0));
    sync_collision(&mut world);
    world
}

/// `blocked` keeps the wall and places sensors on the path that is actually applied.
/// Without it, every extra collider is a thin sensor and the motor runs the full 30 units.
fn sensor_scene(count: usize, blocked: bool) -> World {
    let mut world = World::new();
    let mover = world.spawn((
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
    remember(&mut world, vec![mover], vec![Vec3::ZERO], Vec3::new(30.0, 0.0, 0.0));
    sync_collision(&mut world);
    world
}

/// `walls + extras` solids. The first `walls` motors run into a box. The next `sensors` motors
/// pass through a thin sensor and finish the move. Remaining motors finish the move in an open
/// lane. `beside` places the extra solids next to those open lanes so the sweep bounds see them.
fn crowd(motors: usize, walls: usize, sensors: usize, extras: usize, beside: bool) -> World {
    let mut world = World::new();
    let mut entities = Vec::with_capacity(motors);
    let mut starts = Vec::with_capacity(motors);
    for index in 0..motors {
        let start = Vec3::new(0.0, 0.0, index as f32 * 3.0);
        let entity = world.spawn((
            Transform::from_xyz(start.x, start.y, start.z),
            Collider::capsule(0.4, 0.6).with_offset(Vec3::new(0.0, 1.0, 0.0)),
            CharacterMotor { skin: 0.01, ..CharacterMotor::default() },
        ));
        entities.push(entity);
        starts.push(start);
    }
    for index in 0..walls {
        let z = index as f32 * 3.0;
        world.spawn((Transform::from_xyz(4.0, 1.0, z), Collider::aabb_box(Vec3::new(0.5, 2.0, 0.8))));
    }
    for index in 0..sensors {
        let z = (walls + index) as f32 * 3.0;
        world.spawn((Transform::from_xyz(2.0, 1.0, z), Collider::aabb_box(Vec3::new(0.05, 2.0, 0.8)).sensor()));
    }
    for index in 0..extras {
        let (x, z) = if beside {
            let lane = (walls + sensors + index) % motors;
            (2.0, lane as f32 * 3.0 + 1.6)
        } else {
            (40.0, index as f32 * 1.5)
        };
        world.spawn((Transform::from_xyz(x, 1.0, z), Collider::aabb_box(Vec3::new(0.4, 2.0, 0.3))));
    }
    remember(&mut world, entities, starts, Vec3::new(6.0, 0.0, 0.0));
    sync_collision(&mut world);
    world
}
