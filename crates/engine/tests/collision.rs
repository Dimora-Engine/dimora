//! Collision scheduling, debug lines, and camera independence. No GPU.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use engine::core::FixedTimestep;
use engine::prelude::*;
use engine::world::{sync_collision, ScheduleError};

fn fixed_step() -> Duration {
    FixedTimestep::new(60).step()
}

fn fixture(world: &mut World) -> (Entity, Entity) {
    let sphere = world.spawn((Transform::IDENTITY, Collider::sphere(1.0)));
    let box_entity = world.spawn((Transform::from_xyz(3.0, 0.0, 0.0), Collider::aabb_box(Vec3::splat(0.5))));
    (sphere, box_entity)
}

fn snapshot(world: &World) -> (Vec<Entity>, Option<(Entity, f32, Vec3)>) {
    let collision = world.resource::<CollisionWorld>();
    let overlap = collision.overlap(ColliderShape::Sphere { radius: 1.0 }, &Transform::IDENTITY, CollisionQuery::all());
    let ray = collision.ray(Vec3::new(-4.0, 0.0, 0.0), Vec3::X, 20.0, CollisionQuery::all());
    let ray = ray.map(|hit| (hit.entity, hit.distance, hit.normal));
    (overlap, ray)
}

#[test]
fn sync_collision_runs_after_fixed_systems_and_a_zero_step_does_not_rebuild() {
    let mut game = Game::new(GameConfig::default());
    let entity = game.world.spawn((Transform::IDENTITY, Collider::sphere(1.0)));
    game.add_fixed_system("slide_origin", move |world| {
        world.component_mut::<Transform>(entity).unwrap().position.x = 4.0;
    });
    assert_eq!(game.system_order(Stage::FixedUpdate), vec!["slide_origin", Stage::SYNC_COLLISION]);
    let rejected = game.add_to_stage_after(Stage::FixedUpdate, "too_late", Stage::SYNC_COLLISION, |_world| {});
    assert_eq!(
        rejected.err(),
        Some(ScheduleError::MustRunBefore { system: "too_late", anchor: Stage::SYNC_COLLISION })
    );

    game.step(Duration::ZERO);
    assert_eq!(game.world.resource::<CollisionWorld>().generation(), 0);
    assert!(game.world.resource::<CollisionWorld>().center(entity).is_none());

    game.step(fixed_step());
    let center = game.world.resource::<CollisionWorld>().center(entity).unwrap();
    assert!(center.approx_eq(Vec3::new(4.0, 0.0, 0.0), 1e-4));
    assert_eq!(game.world.resource::<CollisionWorld>().generation(), 1);

    game.world.component_mut::<Transform>(entity).unwrap().position.x = 9.0;
    game.step(Duration::ZERO);
    let stale = game.world.resource::<CollisionWorld>().center(entity).unwrap();
    assert!(stale.approx_eq(Vec3::new(4.0, 0.0, 0.0), 1e-4));
}

fn finite(draw: &DebugDraw) {
    assert!(draw.len() > 12);
    for segment in draw.segments() {
        assert!(segment.start.x.is_finite() && segment.end.x.is_finite());
        assert!(segment.start.y.is_finite() && segment.end.y.is_finite());
        assert!(segment.start.z.is_finite() && segment.end.z.is_finite());
    }
}

fn shapes(game: &mut Game) {
    game.world.spawn((Transform::from_xyz(1.0, 2.0, -3.0), Collider::sphere(0.75)));
    game.world.spawn((Transform::IDENTITY, Collider::aabb_box(Vec3::splat(0.4))));
    game.world.spawn((Transform::from_xyz(0.0, 1.0, 0.0), Collider::capsule(0.3, 0.6)));
}

#[test]
fn a_default_step_creates_no_collision_wires() {
    let mut game = Game::new(GameConfig::default());
    assert!(!game.world.resource::<CollisionDebug>().enabled);
    shapes(&mut game);
    game.world.spawn((Transform::from_xyz(0.0, 0.0, -6.0), Camera::orthographic(10.0)));
    game.step(fixed_step());
    assert!(game.world.resource::<CollisionWorld>().len() >= 3);
    assert!(game.world.resource::<DebugDraw>().is_empty());
    let scene = game.extract().expect("camera");
    assert!(scene.debug_lines.is_empty());
    assert_eq!(scene.stats.debug_segments, 0);
}

#[test]
fn enabled_collision_debug_draws_finite_segments_and_appends_across_iterations() {
    let mut once = Game::new(GameConfig::default());
    shapes(&mut once);
    once.world.resource_mut::<CollisionDebug>().enabled = true;
    once.step(fixed_step());
    assert_eq!(once.world.resource::<Diagnostics>().fixed_steps, 1);
    let one = once.world.resource::<DebugDraw>().len();
    finite(&once.world.resource::<DebugDraw>());

    let mut twice = Game::new(GameConfig::default());
    shapes(&mut twice);
    twice.world.resource_mut::<CollisionDebug>().enabled = true;
    twice.step(fixed_step() * 2);
    assert_eq!(twice.world.resource::<Diagnostics>().fixed_steps, 2);
    assert_eq!(twice.world.resource::<DebugDraw>().len(), one * 2);
}

#[test]
fn a_fixed_command_lands_after_the_rebuild_and_a_zero_step_leaves_the_snapshot() {
    let mut game = Game::new(GameConfig::default());
    let queued = Rc::new(Cell::new(None));
    let slot = Rc::clone(&queued);
    game.add_fixed_system("queue_spawn", move |world| {
        if slot.get().is_some() {
            return;
        }
        let entity = world.commands().spawn((Transform::from_xyz(2.0, 0.0, 0.0), Collider::sphere(0.5)));
        slot.set(Some(entity));
    });
    let direct = game.world.spawn((Transform::IDENTITY, Collider::sphere(0.25)));
    game.step(fixed_step());
    let spawned = queued.get().expect("queued");
    assert!(game.world.component::<Collider>(spawned).is_ok());
    assert!(game.world.resource::<CollisionWorld>().center(spawned).is_none());
    assert!(game.world.resource::<CollisionWorld>().center(direct).is_some());
    let generation = game.world.resource::<CollisionWorld>().generation();

    game.step(Duration::ZERO);
    assert_eq!(game.world.resource::<Diagnostics>().fixed_steps, 0);
    assert_eq!(game.world.resource::<CollisionWorld>().generation(), generation);
    assert!(game.world.resource::<CollisionWorld>().center(spawned).is_none());

    game.step(fixed_step());
    let center = game.world.resource::<CollisionWorld>().center(spawned).expect("next rebuild");
    assert!(center.approx_eq(Vec3::new(2.0, 0.0, 0.0), 1e-4));
}

#[test]
fn a_command_queued_before_step_flushes_before_the_fixed_loop() {
    let mut game = Game::new(GameConfig::default());
    let entity = game.world.commands().spawn((Transform::from_xyz(1.0, 0.0, 0.0), Collider::sphere(0.5)));
    game.step(fixed_step());
    assert!(game.world.resource::<CollisionWorld>().center(entity).is_some());

    let mut idle = Game::new(GameConfig::default());
    let waiting = idle.world.commands().spawn((Transform::IDENTITY, Collider::sphere(0.5)));
    idle.step(Duration::ZERO);
    assert!(idle.world.component::<Collider>(waiting).is_ok());
    assert_eq!(idle.world.resource::<CollisionWorld>().generation(), 0);
    assert!(idle.world.resource::<CollisionWorld>().center(waiting).is_none());
}

#[test]
fn cameras_do_not_change_overlap_or_ray_results() {
    let mut bare = World::new();
    let expected_entities = fixture(&mut bare);
    sync_collision(&mut bare);
    let expected = snapshot(&bare);
    assert!(expected.0.contains(&expected_entities.0));

    let cameras: Vec<Option<(Transform, Camera)>> = vec![
        None,
        Some((Transform::from_xyz(0.0, 2.0, -8.0), Camera::perspective(50.0))),
        Some((
            Transform::from_xyz(0.0, 8.0, -8.0),
            Camera::orthographic(12.0).with_pixel_snap(true).with_internal_resolution(320, 180),
        )),
        Some({
            let placed = PlacedCamera::isometric(Vec3::ZERO, 16.0, 10.0).unwrap();
            (placed.transform, placed.camera)
        }),
    ];

    for camera in cameras {
        let mut game = Game::new(GameConfig::default());
        fixture(&mut game.world);
        if let Some((transform, camera)) = camera {
            game.world.spawn((transform, camera));
        }
        game.step(fixed_step());
        assert_eq!(snapshot(&game.world), expected);
    }
}
