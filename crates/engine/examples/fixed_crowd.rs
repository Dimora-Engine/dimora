//! Release timings for the whole fixed-update stage on the crowd fixtures.
//!
//! `cargo run --release -p engine --example fixed_crowd`
//!
//! This is not `sync_collision` alone. Each sample is one `Game::step` of exactly one fixed
//! iteration. The reported time is [`engine::Diagnostics::stages`] `fixed_update`: the loop,
//! `sync_collision`, and that iteration's command flush. Intents are written before `step`, so
//! the stage does not include a gameplay producer. Warmup is 10 steps and each row is 31 samples.
//! Event counts are the events from the last measured step. There is no historical baseline.

use engine::prelude::*;

const WARMUP: usize = 10;
const SAMPLES: usize = 31;

struct Movers {
    entities: Vec<Entity>,
    starts: Vec<Vec3>,
}

struct EventTally(u32);

fn main() {
    if cfg!(debug_assertions) {
        eprintln!("fixed_crowd: refusing a debug build; rerun with --release");
        std::process::exit(1);
    }
    println!("fixed crowd bench (full fixed_update stage, not sync_collision alone)");
    time_stage("small", 10, 10, 0, 10, false);
    time_stage("mixed", 50, 20, 10, 30, false);
    time_stage("crowd", 100, 30, 20, 20, false);
    time_stage("dense", 100, 40, 20, 60, true);
}

fn time_stage(label: &str, motors: usize, walls: usize, sensors: usize, extras: usize, beside: bool) {
    let mut game = Game::new(GameConfig::default());
    game.world.insert_resource(EventTally(0));
    game.add_system("tally", tally);
    let movers = build(&mut game.world, motors, walls, sensors, extras, beside);
    game.world.insert_resource(movers);
    let step = FixedTimestep::new(60).step();
    for _ in 0..WARMUP {
        rewind(&mut game);
        game.step(step);
    }
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        rewind(&mut game);
        game.step(step);
        let diagnostics = *game.world.resource::<Diagnostics>();
        assert_eq!(diagnostics.fixed_steps, 1);
        samples.push(diagnostics.stages.fixed_update.as_secs_f64() * 1.0e6);
        std::hint::black_box(diagnostics.fixed_steps);
    }
    samples.sort_by(|left, right| left.partial_cmp(right).unwrap());
    let events = game.world.resource::<EventTally>().0;
    let (indexed, motor_count, sensor_count) = counts(&game.world);
    let stopped = stopped_x(&game);
    println!(
        "{label}: median {:.1} us, q1 {:.1}, q3 {:.1}, min {:.1}, max {:.1}, indexed {indexed}, motors {motor_count}, sensors {sensor_count}, events {events}, fixed steps 1, stopped x {:.2}, last x {:.2}",
        samples[SAMPLES / 2],
        samples[7],
        samples[23],
        samples[0],
        samples[SAMPLES - 1],
        stopped.0,
        stopped.1,
    );
}

fn tally(world: &mut World) {
    let count = world.events::<TriggerEvent>().len() as u32;
    world.resource_mut::<EventTally>().0 = count;
}

fn rewind(game: &mut Game) {
    let movers = game.world.remove_resource::<Movers>().expect("movers");
    for (entity, start) in movers.entities.iter().copied().zip(movers.starts.iter().copied()) {
        game.world.component_mut::<Transform>(entity).unwrap().position = start;
        let _ = game.world.insert(entity, (MoveIntent::new(Vec3::new(6.0, 0.0, 0.0)),));
    }
    game.world.insert_resource(movers);
}

fn stopped_x(game: &Game) -> (f32, f32) {
    let first_id = game.world.resource::<Movers>().entities[0];
    let last_id = *game.world.resource::<Movers>().entities.last().unwrap();
    let first = game.world.component::<Transform>(first_id).unwrap().position.x;
    let last = game.world.component::<Transform>(last_id).unwrap().position.x;
    (first, last)
}

fn counts(world: &World) -> (usize, usize, usize) {
    let sensors = world.query::<&Collider>().iter().filter(|(_, collider)| collider.sensor).count();
    let motors = world.query::<&CharacterMotor>().iter().count();
    let indexed = world.resource::<CollisionWorld>().len();
    (indexed, motors, sensors)
}

fn build(world: &mut World, motors: usize, walls: usize, sensors: usize, extras: usize, beside: bool) -> Movers {
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
    Movers { entities, starts }
}
