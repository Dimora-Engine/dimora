//! Trigger delivery through `Game::step`. No GPU.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use engine::core::FixedTimestep;
use engine::prelude::*;
use engine::renderer::PlacedCamera;

fn fixed_step() -> Duration {
    FixedTimestep::new(60).step()
}

fn watch(game: &mut Game) -> Rc<RefCell<Vec<TriggerEvent>>> {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let slot = seen.clone();
    game.add_system("triggers", move |world| {
        *slot.borrow_mut() = world.events::<TriggerEvent>();
    });
    seen
}

fn entered(a: Entity, b: Entity) -> TriggerEvent {
    let (a, b) = if a < b { (a, b) } else { (b, a) };
    TriggerEvent::Entered { a, b }
}

fn exited(a: Entity, b: Entity) -> TriggerEvent {
    let (a, b) = if a < b { (a, b) } else { (b, a) };
    TriggerEvent::Exited { a, b }
}

#[test]
fn a_later_fixed_iteration_cannot_see_events_and_update_sees_them_in_send_order() {
    let mut game = Game::new(GameConfig::default());
    let solid = game.world.spawn((Transform::IDENTITY, Collider::sphere(0.5)));
    let sensor = game.world.spawn((Transform::IDENTITY, Collider::sphere(0.5).sensor()));
    let seen = watch(&mut game);
    let leaked = Rc::new(Cell::new(false));
    let flag = leaked.clone();
    game.add_fixed_system("peek", move |world| {
        if !world.events::<TriggerEvent>().is_empty() {
            flag.set(true);
        }
    });
    let iteration = Rc::new(Cell::new(0u32));
    let count = iteration.clone();
    game.add_fixed_system("mask", move |world| {
        let index = count.get();
        count.set(index + 1);
        if index == 1 {
            world.component_mut::<Collider>(sensor).unwrap().mask = 0;
        }
    });

    game.step(fixed_step() * 2);
    assert!(!leaked.get(), "a fixed iteration saw a trigger event");
    assert_eq!(*seen.borrow(), vec![entered(solid, sensor), exited(solid, sensor)]);
}

#[test]
fn a_deferred_despawn_exits_on_the_next_rebuild() {
    let mut game = Game::new(GameConfig::default());
    let solid = game.world.spawn((Transform::IDENTITY, Collider::sphere(0.5)));
    let sensor = game.world.spawn((Transform::IDENTITY, Collider::sphere(0.5).sensor()));
    let seen = watch(&mut game);
    game.step(fixed_step());
    assert_eq!(*seen.borrow(), vec![entered(solid, sensor)]);

    let once = Rc::new(Cell::new(false));
    let flag = once.clone();
    game.add_fixed_system("later", move |world| {
        if !flag.replace(true) {
            world.commands().despawn(sensor);
        }
    });
    game.step(fixed_step());
    assert!(seen.borrow().is_empty(), "the despawn flushed before the rebuild, {:?}", seen.borrow());
    assert!(!game.world.contains(sensor));

    game.step(fixed_step());
    assert_eq!(*seen.borrow(), vec![exited(solid, sensor)]);
}

#[test]
fn a_zero_fixed_step_keeps_the_pair_and_emits_nothing() {
    let mut game = Game::new(GameConfig::default());
    let solid = game.world.spawn((Transform::IDENTITY, Collider::sphere(0.5)));
    let sensor = game.world.spawn((Transform::IDENTITY, Collider::sphere(0.5).sensor()));
    let seen = watch(&mut game);
    game.step(fixed_step());
    assert_eq!(*seen.borrow(), vec![entered(solid, sensor)]);

    game.step(Duration::ZERO);
    assert!(seen.borrow().is_empty());
    assert_eq!(game.world.resource::<CollisionWorld>().generation(), 1);

    game.step(fixed_step());
    assert!(seen.borrow().is_empty(), "the zero-step frame dropped the stored pair");
}

#[test]
fn cameras_and_visual_children_do_not_change_trigger_events() {
    let bare = trace(None, false, false);
    let cameras = [
        None,
        Some(Camera::perspective(50.0)),
        Some(Camera::orthographic(12.0)),
        Some(PlacedCamera::isometric(Vec3::ZERO, 16.0, 10.0).unwrap().camera),
        Some(PlacedCamera::side(Vec3::ZERO, 16.0, 10.0).unwrap().camera),
    ];
    for camera in cameras {
        assert_eq!(trace(camera, false, false), bare);
    }
    assert_eq!(trace(None, true, false), bare);
    assert_eq!(trace(None, false, true), bare);
}

fn trace(camera: Option<Camera>, sprite: bool, mesh: bool) -> Vec<&'static str> {
    let mut game = Game::new(GameConfig::default());
    let seen = watch(&mut game);
    if let Some(camera) = camera {
        game.world.spawn((Transform::from_xyz(3.0, 4.0, -8.0), camera));
    }
    let character = game.world.spawn((
        Transform::IDENTITY,
        Collider::sphere(0.4),
        CharacterMotor { skin: 0.0, ..CharacterMotor::default() },
    ));
    if sprite {
        game.world.spawn_child(character, (Transform::from_xyz(0.0, 1.0, 0.0), Sprite::new("hero")));
    }
    if mesh {
        game.world.spawn_child(character, (Transform::from_xyz(0.0, 1.0, 0.0), Mesh::new("hero")));
    }
    let sensor =
        game.world.spawn((Transform::from_xyz(2.0, 0.0, 0.0), Collider::aabb_box(Vec3::new(0.05, 1.5, 1.5)).sensor()));
    game.add_fixed_system("go", move |world| {
        let _ = world.insert(character, (MoveIntent::new(Vec3::new(5.0, 0.0, 0.0)),));
    });
    game.step(fixed_step());
    let events = seen.borrow().clone();
    assert_eq!(events, vec![entered(character, sensor), exited(character, sensor)]);
    events
        .iter()
        .map(|event| match event {
            TriggerEvent::Entered { .. } => "entered",
            TriggerEvent::Exited { .. } => "exited",
        })
        .collect()
}
