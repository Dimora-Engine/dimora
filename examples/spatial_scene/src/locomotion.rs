//! Opt-in course for `spatial_scene --locomotion`.
//!
//! The gameplay root owns the transform, capsule, motor, and locomotion components. The sprite or
//! mesh is a child with no collider. Movement is a [`LocomotionIntent`] written on the fixed step.
//! The motor and the locomotion system turn that into a transform. This module does not integrate
//! its own velocity.
//!
//! The course sits near the example camera focus so the existing perspective, orthographic, and
//! isometric presets frame it. Those preset functions are unchanged. World axes stay +X right,
//! +Y up, and +Z deeper. IJKL move on those axes, not relative to the camera.
//!
//! Layout, floor top at Y = 0. +X climbs. The ramp wall blocks the +Z side of the ramp, so the
//! start lane runs into it. The −Z side stays open and reaches the deck.
//!
//! * a 60° face at the floor's −X edge; the default 45° limit does not walk it
//! * a broad flat floor, a non-blocking sensor, and a vertical slide wall on +Z
//! * a 30° ramp
//! * a vertical wall across the +Z half of that ramp
//! * a deck at the high lip, reached along the open −Z side
//! * a lower court under and beyond that deck

use engine::prelude::*;

use crate::cameras::{CameraRig, NAMED};
use crate::scene::{self, BodyVisual, Scene, BODY_HALF_HEIGHT, BODY_RADIUS};

/// Feet of the character on the start floor.
pub const START: Vec3 = Vec3::new(-30.0, 0.0, 230.0);

/// Held run. Not used by the camera or by `--collision`.
pub const RUN_ACTION: &str = "run";
/// Jump edge. Not used by the camera or by `--collision`.
pub const JUMP_ACTION: &str = "jump";

const CUBE: f32 = 80.0;
/// Floor top is Y = 0. X runs from −110 to 90. Z runs from 120 to 420.
const FLOOR_CENTER: Vec3 = Vec3::new(-10.0, -24.0, 270.0);
const FLOOR_HALF: Vec3 = Vec3::new(100.0, 24.0, 150.0);
/// Low lip of the 30° ramp. The floor continues past this so the capsule can step on.
const RAMP_LIP: Vec3 = Vec3::new(60.0, 0.0, 270.0);

/// Builds the course, the camera for `config`, and the character. Does not publish environment A.
pub fn build_course(world: &mut World, config: usize, visual: BodyVisual) -> Scene {
    scene::add_meshes(world);
    let instance = world.load_scene(|_builder| Ok(())).expect("the course is not a scene template");
    let (camera, transform) = NAMED[config].1();
    let projection = camera.projection;
    let rig = CameraRig::from_transform(&transform);
    let camera = world.spawn((camera, transform, Name::new("camera")));
    let walker = spawn_character(world, visual);
    let visual_entity = walker.1;
    spawn_course(world);
    Scene {
        camera,
        walker: walker.0,
        cube: None,
        figures: Vec::new(),
        rig,
        projection,
        config,
        figure_orientation: SpriteOrientation::Billboard,
        title_timer: 0.0,
        environment: scene::Environment::A,
        instance,
        pending: None,
        collision: false,
        locomotion: true,
        walker_visual: Some(visual_entity),
        pace: None,
        pace_direction: 1.0,
    }
}

/// Fixed-step producer. Registered before `sync_collision`. Copies held movement, held run, and
/// the jump edge into one [`LocomotionIntent`]. It does not write [`Transform`] or [`MoveIntent`].
pub fn locomotion_motion(world: &mut World) {
    let scene = world.resource::<Scene>();
    if !scene.locomotion {
        return;
    }
    let walker = scene.walker;
    drop(scene);
    if world.component::<CharacterLocomotion>(walker).is_err() {
        return;
    }
    let input = world.resource::<Input>();
    let actions = world.resource::<Actions>();
    let wish = Vec3::new(actions.axis(&input, scene::WALK_X_ACTION), 0.0, actions.axis(&input, scene::WALK_Z_ACTION));
    let run = actions.down(&input, RUN_ACTION);
    let jump = actions.fixed_pressed(JUMP_ACTION);
    drop(actions);
    drop(input);
    world.insert(walker, (LocomotionIntent { wish, jump, run },)).expect("walker");
}

/// Update-stage reader. Landing and trigger events are visible here and gone when `Game::step` returns.
pub fn locomotion_feedback(world: &mut World) {
    if !world.resource::<Scene>().locomotion {
        return;
    }
    for event in world.events::<LocomotionEvent>() {
        log::info!("locomotion {event:?}");
    }
    for event in world.events::<TriggerEvent>() {
        log::info!("trigger {event:?}");
    }
}

/// Printed when the example launches. World axes, not camera-relative.
pub fn control_guide() -> &'static str {
    "\
spatial_scene --locomotion
  J / L       left / right (−X / +X)
  K / I       back / forward (−Z / +Z)
  H           hold to run
  N           jump
  release     decelerate
  W / S       camera along its heading
  A / D       camera strafe
  Space/Shift camera up / down
  arrows      camera yaw / pitch
  C           next camera configuration
  P           perspective / orthographic
  O           sprite orientation
  R           reset the camera pose
  Esc         quit
  --body mesh uses a box child instead of the figure sprite
  --camera perspective|orthographic|isometric keeps the same course"
}

fn spawn_character(world: &mut World, visual: BodyVisual) -> (Entity, Entity) {
    let walker = world.spawn((
        Transform::from_xyz(START.x, START.y, START.z),
        scene::character_collider(),
        CharacterMotor::default(),
        CharacterLocomotion::default(),
        CharacterMotion::default(),
        Name::new("walker"),
    ));
    let visual_entity = match visual {
        BodyVisual::Sprite => {
            let mut sprite = scene::figure(Color::rgb(0.95, 0.55, 0.95));
            sprite.anchor = Some(Vec2::new(0.5, 0.0));
            world.spawn_child(walker, (Transform::IDENTITY, sprite, Name::new("walker visual")))
        }
        BodyVisual::Mesh => {
            let height = 2.0 * (BODY_HALF_HEIGHT + BODY_RADIUS);
            let width = BODY_RADIUS * 2.0;
            let scale = Vec3::new(width / CUBE, height / CUBE, width / CUBE);
            let pose = Transform::from_xyz(0.0, height * 0.5, 0.0).with_scale(scale);
            world.spawn_child(
                walker,
                (
                    pose,
                    Mesh::new("cube").with_material(scene::panel(Color::rgb(0.95, 0.55, 0.95))),
                    Name::new("walker visual"),
                ),
            )
        }
    };
    (walker, visual_entity)
}

fn spawn_course(world: &mut World) {
    spawn_box(world, "start floor", FLOOR_CENTER, FLOOR_HALF, Quat::IDENTITY, Color::rgb(0.55, 0.58, 0.52));
    // Near face at Z = 390. The start lane (Z ≈ 230) hits it. The ramp's open side does not.
    spawn_box(
        world,
        "slide wall",
        Vec3::new(-20.0, 70.0, 400.0),
        Vec3::new(70.0, 70.0, 10.0),
        Quat::IDENTITY,
        Color::rgb(0.62, 0.45, 0.72),
    );
    spawn_sensor(world);

    let steep = sloped_face(-60.0, Vec3::new(55.0, 14.0, 70.0), Vec3::new(-110.0, 0.0, FLOOR_CENTER.z));
    spawn_box(world, "steep face", steep.center, steep.half, steep.rotation, Color::rgb(0.78, 0.28, 0.22));

    let ramp = sloped_face(30.0, Vec3::new(100.0, 16.0, 130.0), RAMP_LIP);
    spawn_box(world, "walk ramp", ramp.center, ramp.half, ramp.rotation, Color::rgb(0.72, 0.55, 0.28));

    // Cuts the +Z half. The start lane (Z = 230) runs into this face. Z below about 220 walks past.
    let run = ramp.high_lip.x - ramp.low_lip.x;
    let wall_x = ramp.low_lip.x + run * 0.42;
    let wall_y = ramp.low_lip.y + (ramp.high_lip.y - ramp.low_lip.y) * 0.42;
    spawn_box(
        world,
        "ramp wall",
        Vec3::new(wall_x + 8.0, wall_y + 20.0, ramp.low_lip.z + 40.0),
        Vec3::new(8.0, 70.0, 90.0),
        Quat::IDENTITY,
        Color::rgb(0.45, 0.48, 0.62),
    );

    // The near face meets the high lip. Pulling it back leaves a riser above the ramp, and the
    // capsule stops on that face. The deck top is the lip height, so the step off the far side falls.
    let deck_half = Vec3::new(55.0, 16.0, 120.0);
    let deck_center = Vec3::new(ramp.high_lip.x + deck_half.x, ramp.high_lip.y - deck_half.y, 250.0);
    spawn_box(world, "upper deck", deck_center, deck_half, Quat::IDENTITY, Color::rgb(0.42, 0.56, 0.62));

    let deck_far_x = deck_center.x + deck_half.x;
    let court_half = Vec3::new(130.0, 24.0, 150.0);
    let court_center = Vec3::new(deck_far_x - 20.0, -24.0, deck_center.z);
    spawn_box(world, "lower court", court_center, court_half, Quat::IDENTITY, Color::rgb(0.36, 0.42, 0.36));
}

fn spawn_sensor(world: &mut World) {
    // Sits in the body's torso. The bottom is above the floor so the pair is the character, not the ground.
    let center = Vec3::new(START.x, 50.0, 290.0);
    let half = Vec3::new(22.0, 20.0, 8.0);
    let sensor = world.spawn((
        Transform::from_xyz(center.x, center.y, center.z),
        Collider::aabb_box(half).sensor(),
        Name::new("course sensor"),
    ));
    let scale = Vec3::new(half.x * 2.0 / CUBE, half.y * 2.0 / CUBE, half.z * 2.0 / CUBE);
    world.spawn_child(
        sensor,
        (
            Transform::from_scale(scale),
            Mesh::new("cube").with_material(scene::panel(Color::rgb(0.35, 0.82, 0.55))),
            Name::new("course sensor marker"),
        ),
    );
}

fn spawn_box(world: &mut World, name: &str, center: Vec3, half: Vec3, rotation: Quat, color: Color) -> Entity {
    let entity = world.spawn((
        Transform::from_position(center).with_rotation(rotation),
        Collider::aabb_box(half),
        Name::new(name),
    ));
    let scale = Vec3::new(half.x * 2.0 / CUBE, half.y * 2.0 / CUBE, half.z * 2.0 / CUBE);
    world.spawn_child(
        entity,
        (
            Transform::from_scale(scale),
            Mesh::new("cube").with_material(scene::panel(color)),
            Name::new(format!("{name} mesh")),
        ),
    );
    entity
}

struct Face {
    center: Vec3,
    rotation: Quat,
    half: Vec3,
    low_lip: Vec3,
    high_lip: Vec3,
}

/// A rotated box whose top face meets `low_lip`. Positive degrees rise toward +X.
fn sloped_face(degrees: f32, half: Vec3, low_lip: Vec3) -> Face {
    let rotation = Quat::from_rotation_z(degrees.to_radians());
    let sign = if degrees >= 0.0 { -1.0 } else { 1.0 };
    let low_local = Vec3::new(sign * half.x, half.y, 0.0);
    let high_local = Vec3::new(-sign * half.x, half.y, 0.0);
    let center = low_lip - rotation.rotate(low_local);
    let high_lip = center + rotation.rotate(high_local);
    Face { center, rotation, half, low_lip, high_lip }
}
