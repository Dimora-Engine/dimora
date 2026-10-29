//! Release timings for one locomotion step beside a bare motor sweep.
//!
//! `cargo run --release -p engine-world --example locomotion_bench`
//!
//! Warmup is 10 calls. Each row is 31 samples. The rewind that puts each body back and writes the
//! next intent is not timed. The timed call is `sync_collision` or `sync_collision_with_step`
//! only. It is not a frame budget. The floor row's locomotion sweep also includes a 0.08 downward
//! stick, so its median is not the probe by itself.
//!
//! `locomotion, no floor` keeps gravity at 0 so that row stays a horizontal airborne step, matching
//! the Locomotion-1A measurement. Free fall, jump takeoff, and the mixed row are the 1B cases.
//! Slope rows are the 1C cases. They add one ramp and do not replace the flat rows.

use std::time::Instant;

use engine_math::{Quat, Transform, Vec3};
use engine_world::{
    sync_collision, sync_collision_with_step, CharacterLocomotion, CharacterMotion, CharacterMotor, Collider, Entity,
    LocomotionIntent, MoveIntent, World,
};

const WARMUP: usize = 10;
const SAMPLES: usize = 31;
const STEP: f32 = 1.0 / 60.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum BenchMotion {
    Motor,
    Walk,
    Fall,
    Jump,
    Mixed,
    SlopeUp,
    SlopeDown,
    SlopeStand,
    Steep,
    SlopeCrowd,
    SlopeMixed,
}

#[derive(Clone)]
struct Cast {
    entities: Vec<Entity>,
    starts: Vec<Vec3>,
    motion: BenchMotion,
}

fn main() {
    if cfg!(debug_assertions) {
        eprintln!("locomotion_bench: refusing a debug build; rerun with --release");
        std::process::exit(1);
    }
    println!("locomotion bench (one sync, 10 warmup, 31 samples)");
    time_case("bare motor, floor", scene(1, true, BenchMotion::Motor, None));
    time_case("locomotion, floor", scene(1, true, BenchMotion::Walk, None));
    time_case("locomotion, no floor", scene(1, false, BenchMotion::Walk, Some(0.0)));
    time_case("locomotion, 8 on a floor", scene(8, true, BenchMotion::Walk, None));
    time_case("free fall", scene(1, false, BenchMotion::Fall, None));
    time_case("jump takeoff", scene(1, true, BenchMotion::Jump, None));
    time_case("mixed, 4 ground + 4 air", scene(8, true, BenchMotion::Mixed, None));
    time_case("slope 30° uphill", slope_scene(1, BenchMotion::SlopeUp));
    time_case("slope 30° downhill", slope_scene(1, BenchMotion::SlopeDown));
    time_case("slope 30° standing", slope_scene(1, BenchMotion::SlopeStand));
    time_case("steep 60° unsupported", slope_scene(1, BenchMotion::Steep));
    time_case("8 on a 30° slope", slope_scene(8, BenchMotion::SlopeCrowd));
    time_case("mixed flat, slope, air", slope_scene(8, BenchMotion::SlopeMixed));
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
    let motion = world.resource::<Cast>().motion;
    if motion == BenchMotion::Motor {
        sync_collision(world);
    } else {
        sync_collision_with_step(world, Some(STEP));
    }
}

fn rewind(world: &mut World) {
    let cast = world.resource::<Cast>().clone();
    for (index, (entity, start)) in cast.entities.iter().zip(cast.starts.iter()).enumerate() {
        world.component_mut::<Transform>(*entity).unwrap().position = *start;
        match cast.motion {
            BenchMotion::Motor => {
                let _ = world.insert(*entity, (MoveIntent::new(Vec3::new(1.5, 0.0, 0.0)),));
            }
            BenchMotion::Walk => {
                let _ = world.insert(*entity, (LocomotionIntent::new(Vec3::X),));
            }
            BenchMotion::Fall => {
                let _ = world.insert(*entity, (CharacterMotion::default(),));
            }
            BenchMotion::Jump => {
                let _ = world.insert(*entity, (CharacterMotion::default(), LocomotionIntent::jumping(Vec3::ZERO)));
            }
            BenchMotion::Mixed => {
                let _ = world.insert(*entity, (CharacterMotion::default(),));
                if index < cast.entities.len() / 2 {
                    let _ = world.insert(*entity, (LocomotionIntent::new(Vec3::X),));
                }
            }
            BenchMotion::SlopeUp | BenchMotion::SlopeCrowd => {
                let _ = world.insert(*entity, (LocomotionIntent::new(Vec3::X),));
            }
            BenchMotion::SlopeDown => {
                let _ = world.insert(*entity, (LocomotionIntent::new(-Vec3::X),));
            }
            BenchMotion::SlopeStand => {
                let _ = world.insert(*entity, (LocomotionIntent::new(Vec3::ZERO),));
            }
            BenchMotion::Steep => {
                let _ = world.insert(*entity, (CharacterMotion::default(), LocomotionIntent::new(Vec3::X)));
            }
            BenchMotion::SlopeMixed => {
                let _ = world.insert(*entity, (CharacterMotion::default(),));
                if index < 6 {
                    let _ = world.insert(*entity, (LocomotionIntent::new(Vec3::X),));
                }
            }
        }
    }
}

/// One 30° or 60° ramp, the same 16 distant boxes as the flat fixture, and `bodies` spheres.
///
/// `SlopeMixed` uses three spheres on a flat floor, three on the ramp, and two airborne.
fn slope_scene(bodies: usize, motion: BenchMotion) -> World {
    let mut world = World::new();
    for index in 0..16 {
        world.spawn((Transform::from_xyz(30.0, 0.5, index as f32 * 2.0), Collider::aabb_box(Vec3::new(0.4, 0.4, 0.4))));
    }
    let degrees = if motion == BenchMotion::Steep { 60.0 } else { 30.0 };
    let ramp_z = if motion == BenchMotion::SlopeMixed { 40.0 } else { 0.0 };
    let normal = spawn_ramp(&mut world, degrees, ramp_z);
    if motion == BenchMotion::SlopeMixed {
        world.spawn((Transform::from_xyz(0.0, -0.5, -30.0), Collider::aabb_box(Vec3::new(16.0, 0.5, 8.0))));
    }
    let mut entities = Vec::with_capacity(bodies);
    let mut starts = Vec::with_capacity(bodies);
    for index in 0..bodies {
        let start = match motion {
            BenchMotion::SlopeMixed if index < 3 => Vec3::new(0.0, 0.5, -30.0 + index as f32 * 3.0),
            BenchMotion::SlopeMixed if index >= 6 => Vec3::new(0.0, 8.0, -30.0 + (index - 6) as f32 * 3.0),
            _ => normal * 0.5 + Vec3::new(0.0, 0.0, ramp_z + index as f32 * 3.0),
        };
        let entity = world.spawn((
            Transform::from_xyz(start.x, start.y, start.z),
            Collider::sphere(0.5),
            CharacterMotor::default(),
            CharacterLocomotion::default(),
            CharacterMotion::default(),
        ));
        entities.push(entity);
        starts.push(start);
    }
    world.insert_resource(Cast { entities, starts, motion });
    world
}

fn spawn_ramp(world: &mut World, degrees: f32, z_offset: f32) -> Vec3 {
    let rotation = Quat::from_rotation_z(degrees.to_radians());
    let normal = rotation.rotate(Vec3::Y).normalized();
    let half = Vec3::new(16.0, 2.0, 24.0);
    let center = -normal * half.y + Vec3::new(0.0, 0.0, z_offset);
    world.spawn((Transform::from_position(center).with_rotation(rotation), Collider::aabb_box(half)));
    normal
}

/// `bodies` spheres, 16 solids that are not contacts, and an optional floor.
///
/// `gravity` replaces the default only for a locomotion body. `None` keeps −400.
fn scene(bodies: usize, floor: bool, motion: BenchMotion, gravity: Option<f32>) -> World {
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
        let elevated = motion == BenchMotion::Fall || (motion == BenchMotion::Mixed && index >= bodies / 2);
        let start = Vec3::new(0.0, if elevated { 8.0 } else { 0.5 }, index as f32 * 3.0);
        let entity = if motion == BenchMotion::Motor {
            world.spawn((
                Transform::from_xyz(start.x, start.y, start.z),
                Collider::sphere(0.5),
                CharacterMotor::default(),
            ))
        } else {
            let mut locomotion = CharacterLocomotion::default();
            if let Some(gravity) = gravity {
                locomotion.gravity = gravity;
            }
            world.spawn((
                Transform::from_xyz(start.x, start.y, start.z),
                Collider::sphere(0.5),
                CharacterMotor::default(),
                locomotion,
                CharacterMotion::default(),
            ))
        };
        entities.push(entity);
        starts.push(start);
    }
    world.insert_resource(Cast { entities, starts, motion });
    world
}
