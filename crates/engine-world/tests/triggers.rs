//! Sensor overlap and trigger events. No window and no GPU.

use engine_math::{Quat, Transform, Vec3};
use engine_world::{sync_collision, CharacterMotor, Collider, MoveIntent, MoveResult, MoveStatus, TriggerEvent, World};

const EPS: f32 = 1e-4;

fn fresh() -> World {
    let mut world = World::new();
    sync_collision(&mut world);
    world
}

fn motor(skin: f32) -> CharacterMotor {
    CharacterMotor { skin, ..CharacterMotor::default() }
}

fn take_events(world: &mut World) -> Vec<TriggerEvent> {
    world.promote_events();
    let events = world.events::<TriggerEvent>();
    world.clear_events();
    events
}

fn slide(world: &mut World, entity: engine_world::Entity, translation: Vec3) -> MoveResult {
    world.insert(entity, (MoveIntent::new(translation),)).unwrap();
    sync_collision(world);
    (*world.component::<MoveResult>(entity).unwrap()).clone()
}

fn entered(a: engine_world::Entity, b: engine_world::Entity) -> TriggerEvent {
    let (a, b) = if a < b { (a, b) } else { (b, a) };
    TriggerEvent::Entered { a, b }
}

fn exited(a: engine_world::Entity, b: engine_world::Entity) -> TriggerEvent {
    let (a, b) = if a < b { (a, b) } else { (b, a) };
    TriggerEvent::Exited { a, b }
}

fn near(actual: Vec3, expected: Vec3) {
    assert!(actual.approx_eq(expected, EPS), "position {actual:?}, expected {expected:?}");
}

fn at(point: Vec3) -> Transform {
    Transform::from_xyz(point.x, point.y, point.z)
}

fn thin(axis: Vec3) -> Collider {
    let half = Vec3::splat(1.5) - axis.abs() * 1.45;
    Collider::aabb_box(half).sensor()
}

#[test]
fn an_initial_overlap_emits_one_enter_and_an_idle_sync_is_silent() {
    let mut world = fresh();
    let solid = world.spawn((Transform::IDENTITY, Collider::sphere(0.5)));
    let sensor = world.spawn((Transform::IDENTITY, Collider::sphere(0.5).sensor()));
    sync_collision(&mut world);
    assert_eq!(take_events(&mut world), vec![entered(solid, sensor)]);
    sync_collision(&mut world);
    assert!(take_events(&mut world).is_empty());
}

#[test]
fn continuing_overlap_is_silent_and_leaving_emits_one_exit() {
    let mut world = fresh();
    let character = world.spawn((Transform::IDENTITY, Collider::sphere(0.4), motor(0.0)));
    let sensor = world.spawn((Transform::IDENTITY, Collider::sphere(2.0).sensor()));
    sync_collision(&mut world);
    assert_eq!(take_events(&mut world), vec![entered(character, sensor)]);

    slide(&mut world, character, Vec3::new(0.2, 0.0, 0.0));
    assert!(take_events(&mut world).is_empty());

    slide(&mut world, character, Vec3::new(6.0, 0.0, 0.0));
    assert_eq!(take_events(&mut world), vec![exited(character, sensor)]);
}

#[test]
fn a_new_sensor_and_a_reenabled_sensor_each_emit_one_enter() {
    let mut world = fresh();
    let solid = world.spawn((Transform::IDENTITY, Collider::sphere(0.5)));
    sync_collision(&mut world);
    assert!(take_events(&mut world).is_empty());

    let sensor = world.spawn((Transform::IDENTITY, Collider::sphere(0.5).sensor()));
    sync_collision(&mut world);
    assert_eq!(take_events(&mut world), vec![entered(solid, sensor)]);

    world.component_mut::<Collider>(sensor).unwrap().enabled = false;
    sync_collision(&mut world);
    assert_eq!(take_events(&mut world), vec![exited(solid, sensor)]);

    world.component_mut::<Collider>(sensor).unwrap().enabled = true;
    sync_collision(&mut world);
    assert_eq!(take_events(&mut world), vec![entered(solid, sensor)]);
}

#[test]
fn sensor_pairs_emit_one_event_and_solids_and_rejected_masks_emit_none() {
    let mut world = fresh();
    let first = world.spawn((Transform::IDENTITY, Collider::sphere(0.5).sensor()));
    let second = world.spawn((Transform::IDENTITY, Collider::sphere(0.5).sensor()));
    let solid_a = world.spawn((Transform::from_xyz(0.0, 4.0, 0.0), Collider::sphere(0.5)));
    let solid_b = world.spawn((Transform::from_xyz(0.2, 4.0, 0.0), Collider::sphere(0.5)));
    let masked = world.spawn((Transform::from_xyz(0.0, 8.0, 0.0), Collider::sphere(0.5).with_layers(1).with_mask(1)));
    let excluded =
        world.spawn((Transform::from_xyz(0.0, 8.0, 0.0), Collider::sphere(0.5).sensor().with_layers(2).with_mask(2)));
    sync_collision(&mut world);
    assert_eq!(take_events(&mut world), vec![entered(first, second)]);
    let _ = (solid_a, solid_b, masked, excluded);
}

#[test]
fn a_thin_sensor_crossed_on_each_axis_emits_enter_then_exit() {
    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        let mut world = fresh();
        let character = world.spawn((Transform::IDENTITY, Collider::sphere(0.4), motor(0.0)));
        let sensor = world.spawn((at(axis * 2.0), thin(axis)));
        sync_collision(&mut world);
        assert!(take_events(&mut world).is_empty(), "axis {axis:?} overlapped at the start");
        let result = slide(&mut world, character, axis * 5.0);
        assert_eq!(result.status, MoveStatus::Completed);
        near(result.applied, axis * 5.0);
        assert_eq!(take_events(&mut world), vec![entered(character, sensor), exited(character, sensor)]);
    }
}

#[test]
fn sphere_capsule_and_box_movers_and_a_parented_mover_cross_a_sensor() {
    for shape in [Collider::sphere(0.4), Collider::capsule(0.3, 0.4), Collider::aabb_box(Vec3::splat(0.3))] {
        let mut world = fresh();
        let character = world.spawn((Transform::IDENTITY, shape, motor(0.0)));
        let sensor = world.spawn((Transform::from_xyz(2.0, 0.0, 0.0), thin(Vec3::X)));
        sync_collision(&mut world);
        assert!(take_events(&mut world).is_empty());
        slide(&mut world, character, Vec3::new(5.0, 0.0, 0.0));
        assert_eq!(take_events(&mut world), vec![entered(character, sensor), exited(character, sensor)]);
    }

    let mut world = fresh();
    let parent = world.spawn((Transform::from_xyz(10.0, 0.0, 0.0),));
    let child = world.spawn_child(parent, (Transform::from_xyz(-10.0, 0.0, 0.0), Collider::sphere(0.4), motor(0.0)));
    let sensor = world.spawn((Transform::from_xyz(2.0, 0.0, 0.0), thin(Vec3::X)));
    sync_collision(&mut world);
    assert!(take_events(&mut world).is_empty());
    slide(&mut world, child, Vec3::new(5.0, 0.0, 0.0));
    assert_eq!(take_events(&mut world), vec![entered(child, sensor), exited(child, sensor)]);
    let _ = parent;
}

#[test]
fn two_sensors_are_reported_in_entity_order_not_path_order() {
    let mut world = fresh();
    let character = world.spawn((Transform::IDENTITY, Collider::sphere(0.4), motor(0.0)));
    let far = world.spawn((Transform::from_xyz(4.0, 0.0, 0.0), thin(Vec3::X)));
    let near = world.spawn((Transform::from_xyz(2.0, 0.0, 0.0), thin(Vec3::X)));
    assert!(far < near);
    sync_collision(&mut world);
    assert!(take_events(&mut world).is_empty());
    slide(&mut world, character, Vec3::new(8.0, 0.0, 0.0));
    assert_eq!(
        take_events(&mut world),
        vec![entered(character, far), exited(character, far), entered(character, near), exited(character, near)]
    );
}

#[test]
fn repeated_crossings_of_one_pair_collapse_to_one_enter_and_one_exit() {
    // Request (6, 0, 8) against the face x = 3. The committed path is (2.7, 0, 3.6) then
    // (0, 0, 4.4). The sensor sphere meets both segments and misses the start, the corner, and
    // the end, so a per-crossing timeline would emit two pairs.
    let mut world = fresh();
    world.spawn((Transform::from_xyz(8.0, 0.0, 0.0), Collider::aabb_box(Vec3::new(5.0, 5.0, 40.0))));
    let character = world.spawn((Transform::IDENTITY, Collider::sphere(0.3), motor(0.0)));
    let sensor = world.spawn((Transform::from_xyz(-3.6, 0.0, 5.7), Collider::sphere(6.12).sensor()));
    sync_collision(&mut world);
    assert!(take_events(&mut world).is_empty(), "the repeated-crossing sensor overlapped an endpoint");
    let result = slide(&mut world, character, Vec3::new(6.0, 0.0, 8.0));
    near(result.applied, Vec3::new(2.7, 0.0, 8.0));
    assert_eq!(take_events(&mut world), vec![entered(character, sensor), exited(character, sensor)]);
}

#[test]
fn a_slide_triggers_the_segment_it_travels_and_not_the_net_chord_or_the_far_side_of_the_wall() {
    let mut world = fresh();
    // A thin face at x = 0. The blocking box from the movement fixture would contain the
    // sensor that sits on the discarded +X side, and that static overlap is a different pair.
    world.spawn((Transform::from_xyz(0.05, 0.0, 0.0), Collider::aabb_box(Vec3::new(0.05, 5.0, 20.0))));
    let character = world.spawn((Transform::from_xyz(-2.5, 0.0, 0.0), Collider::sphere(0.5), motor(0.0)));
    let on_slide =
        world.spawn((Transform::from_xyz(-0.5, 0.0, 3.2), Collider::aabb_box(Vec3::new(0.2, 0.5, 0.05)).sensor()));
    let on_chord = world.spawn((Transform::from_xyz(-1.15, 0.0, 3.0), Collider::aabb_box(Vec3::splat(0.05)).sensor()));
    let beyond = world.spawn((Transform::from_xyz(2.0, 0.0, 1.0), Collider::aabb_box(Vec3::splat(0.3)).sensor()));
    sync_collision(&mut world);
    assert!(take_events(&mut world).is_empty());
    let result = slide(&mut world, character, Vec3::new(4.0, 0.0, 4.0));
    near(result.applied, Vec3::new(2.0, 0.0, 4.0));
    assert!(result.blocked());
    assert_eq!(take_events(&mut world), vec![entered(character, on_slide), exited(character, on_slide)]);
    let _ = (on_chord, beyond);
}

#[test]
fn the_recovery_push_can_pass_through_a_sensor() {
    let mut world = fresh();
    world.spawn((Transform::from_xyz(5.0, 0.0, 0.0), Collider::aabb_box(Vec3::splat(5.0))));
    let character = world.spawn((Transform::from_xyz(0.3, 0.0, 0.0), Collider::sphere(0.5), motor(0.01)));
    let sensor = world.spawn((Transform::from_xyz(-0.1, 0.48, 0.0), Collider::sphere(0.05).sensor()));
    sync_collision(&mut world);
    assert!(take_events(&mut world).is_empty());
    let result = slide(&mut world, character, Vec3::X);
    near(result.applied, Vec3::new(-0.81, 0.0, 0.0));
    assert_eq!(take_events(&mut world), vec![entered(character, sensor), exited(character, sensor)]);
}

#[test]
fn two_motors_cross_one_sensor_in_pair_order() {
    let mut world = fresh();
    let sensor = world.spawn((Transform::IDENTITY, Collider::aabb_box(Vec3::new(0.2, 2.0, 2.0)).sensor()));
    let left = world.spawn((Transform::from_xyz(-3.0, 0.0, 0.0), Collider::sphere(0.4), motor(0.0)));
    let right = world.spawn((Transform::from_xyz(3.0, 0.0, 1.0), Collider::sphere(0.4), motor(0.0)));
    assert!(sensor < left && left < right);
    sync_collision(&mut world);
    assert!(take_events(&mut world).is_empty());
    world.insert(left, (MoveIntent::new(Vec3::new(6.0, 0.0, 0.0)),)).unwrap();
    world.insert(right, (MoveIntent::new(Vec3::new(-6.0, 0.0, 0.0)),)).unwrap();
    sync_collision(&mut world);
    assert_eq!(
        take_events(&mut world),
        vec![entered(sensor, left), exited(sensor, left), entered(sensor, right), exited(sensor, right)]
    );
}

#[test]
fn two_moving_sensors_emit_one_pair_from_the_earlier_movers_snapshot() {
    let mut world = fresh();
    let first = world.spawn((Transform::IDENTITY, Collider::sphere(0.4).sensor(), motor(0.0)));
    let second = world.spawn((Transform::from_xyz(4.0, 0.0, 0.0), Collider::sphere(0.4).sensor(), motor(0.0)));
    assert!(first < second);
    sync_collision(&mut world);
    assert!(take_events(&mut world).is_empty());
    world.insert(first, (MoveIntent::new(Vec3::new(5.0, 0.0, 0.0)),)).unwrap();
    world.insert(second, (MoveIntent::new(Vec3::new(-5.0, 0.0, 0.0)),)).unwrap();
    sync_collision(&mut world);
    near(world.component::<Transform>(first).unwrap().position, Vec3::new(5.0, 0.0, 0.0));
    near(world.component::<Transform>(second).unwrap().position, Vec3::new(-1.0, 0.0, 0.0));
    assert_eq!(take_events(&mut world), vec![entered(first, second), exited(first, second)]);
}

#[test]
fn disabling_removing_or_despawning_a_stored_pair_emits_exit() {
    for change in [Change::Disable, Change::Remove, Change::Despawn] {
        let mut world = fresh();
        let solid = world.spawn((Transform::IDENTITY, Collider::sphere(0.5)));
        let sensor = world.spawn((Transform::IDENTITY, Collider::sphere(0.5).sensor()));
        sync_collision(&mut world);
        assert_eq!(take_events(&mut world), vec![entered(solid, sensor)]);
        match change {
            Change::Disable => world.component_mut::<Collider>(sensor).unwrap().enabled = false,
            Change::Remove => world.remove::<Collider>(sensor).unwrap(),
            Change::Despawn => world.despawn(sensor).unwrap(),
        }
        sync_collision(&mut world);
        assert_eq!(take_events(&mut world), vec![exited(solid, sensor)]);
    }
}

#[test]
fn a_teleport_across_a_sensor_is_endpoint_only() {
    let mut world = fresh();
    let character = world.spawn((Transform::IDENTITY, Collider::sphere(0.4)));
    let sensor = world.spawn((Transform::from_xyz(5.0, 0.0, 0.0), thin(Vec3::X)));
    sync_collision(&mut world);
    assert!(take_events(&mut world).is_empty());

    world.component_mut::<Transform>(character).unwrap().position = Vec3::new(10.0, 0.0, 0.0);
    sync_collision(&mut world);
    assert!(take_events(&mut world).is_empty());

    world.component_mut::<Transform>(character).unwrap().position = Vec3::new(5.0, 0.0, 0.0);
    sync_collision(&mut world);
    assert_eq!(take_events(&mut world), vec![entered(character, sensor)]);
}

#[test]
fn a_parented_yaw_does_not_change_pair_order() {
    let mut world = fresh();
    let parent = world.spawn((Transform::from_xyz(0.0, 3.0, 0.0).with_rotation(Quat::from_rotation_y(0.4)),));
    let child = world.spawn_child(parent, (Transform::IDENTITY, Collider::sphere(0.4).sensor()));
    let other = world.spawn((Transform::from_xyz(0.0, 3.0, 0.0), Collider::sphere(0.4).sensor()));
    sync_collision(&mut world);
    assert_eq!(take_events(&mut world), vec![entered(child, other)]);
}

#[derive(Clone, Copy)]
enum Change {
    Disable,
    Remove,
    Despawn,
}
