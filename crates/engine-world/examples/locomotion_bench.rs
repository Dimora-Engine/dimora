//! Release timings for one ground-support step beside a bare motor sweep.
//!
//! `cargo run --release -p engine-world --example locomotion_bench`
//!
//! Warmup is 10 calls. Each row is 31 samples. The rewind that puts each body back and writes the
//! next intent is not timed. The timed call is `sync_collision` or `sync_collision_with_step`
//! only. It is not a frame budget. The floor row's locomotion sweep also includes a 0.08 downward
//! stick, so its median is not the probe by itself.

use std::time::Instant;

use engine_math::{Transform, Vec3};
use engine_world::{
    sync_collision, sync_collision_with_step, CharacterLocomotion, CharacterMotion, CharacterMotor, Collider, Entity,
    LocomotionIntent, MoveIntent, World,
};

const WARMUP: usize = 10;
const SAMPLES: usize = 31;
const STEP: f32 = 1.0 / 60.0;

#[derive(Clone)]
struct Cast {
    entities: Vec<Entity>,
    starts: Vec<Vec3>,
    locomotion: bool,
}

fn main() {
    if cfg!(debug_assertions) {
        eprintln!("locomotion_bench: refusing a debug build; rerun with --release");
        std::process::exit(1);
    }
    println!("locomotion bench (one sync, 10 warmup, 31 samples)");
    time_case("bare motor, floor", scene(1, true, false));
    time_case("locomotion, floor", scene(1, true, true));
    time_case("locomotion, no floor", scene(1, false, true));
    time_case("locomotion, 8 on a floor", scene(8, true, true));
}

fn time_case(label: &str, mut world: World) {
    for _ in 0..WARMUP {
        rewind(&mut world);
        run(&mut world);
    }
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        rewind(&mut world);
        let started = Instant::now();
        run(&mut world);
        samples.push(started.elapsed().as_secs_f64() * 1.0e6);
        let entity = world.resource::<Cast>().entities[0];
        std::hint::black_box(world.component::<Transform>(entity).unwrap().position);
    }
    samples.sort_by(|left, right| left.partial_cmp(right).unwrap());
    let entity = world.resource::<Cast>().entities[0];
    let end = world.component::<Transform>(entity).unwrap().position;
    let indexed = world.resource::<engine_world::CollisionWorld>().len();
    println!(
        "{label}: median {:.1} us, q1 {:.1}, q3 {:.1}, min {:.1}, max {:.1}, indexed {indexed}, end ({:.3}, {:.3}, {:.3})",
        samples[SAMPLES / 2],
        samples[7],
        samples[23],
        samples[0],
        samples[SAMPLES - 1],
        end.x,
        end.y,
        end.z,
    );
}

fn run(world: &mut World) {
    let locomotion = world.resource::<Cast>().locomotion;
    if locomotion {
        sync_collision_with_step(world, Some(STEP));
    } else {
        sync_collision(world);
    }
}

fn rewind(world: &mut World) {
    let cast = world.resource::<Cast>().clone();
    for (entity, start) in cast.entities.iter().zip(cast.starts.iter()) {
        world.component_mut::<Transform>(*entity).unwrap().position = *start;
        if cast.locomotion {
            let _ = world.insert(*entity, (LocomotionIntent::new(Vec3::X),));
        } else {
            let _ = world.insert(*entity, (MoveIntent::new(Vec3::new(1.5, 0.0, 0.0)),));
        }
    }
}

/// `bodies` spheres, 16 solids that are not contacts, and an optional floor.
fn scene(bodies: usize, floor: bool, locomotion: bool) -> World {
    let mut world = World::new();
    if floor {
        world.spawn((Transform::from_xyz(0.0, -0.5, 0.0), Collider::aabb_box(Vec3::new(40.0, 0.5, 40.0))));
    }
    for index in 0..16 {
        world.spawn((Transform::from_xyz(30.0, 0.5, index as f32 * 2.0), Collider::aabb_box(Vec3::new(0.4, 0.4, 0.4))));
    }
    let mut entities = Vec::with_capacity(bodies);
    let mut starts = Vec::with_capacity(bodies);
    for index in 0..bodies {
        let start = Vec3::new(0.0, 0.5, index as f32 * 3.0);
        let entity = if locomotion {
            world.spawn((
                Transform::from_xyz(start.x, start.y, start.z),
                Collider::sphere(0.5),
                CharacterMotor::default(),
                CharacterLocomotion::default(),
                CharacterMotion::default(),
            ))
        } else {
            world.spawn((
                Transform::from_xyz(start.x, start.y, start.z),
                Collider::sphere(0.5),
                CharacterMotor::default(),
            ))
        };
        entities.push(entity);
        starts.push(start);
    }
    world.insert_resource(Cast { entities, starts, locomotion });
    world
}
