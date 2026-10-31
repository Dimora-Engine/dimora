//! Headless `--locomotion` checks. They build the course and step `Game`.
//!
//! Pixels are not hashed. The gameplay root is the body that moves.

mod common;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use common::game as default_game;
use engine::core::InputEvent;
use engine::prelude::*;
use spatial_scene::locomotion::{self, JUMP_ACTION, RUN_ACTION, START};
use spatial_scene::scene::{self, BodyVisual, Scene};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Sample {
    position: Vec3,
    velocity: Vec3,
    phase: LocomotionPhase,
    support: SupportKind,
    landings: u32,
    triggers: u32,
    blocked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SupportKind {
    Unknown,
    Floor,
    Ramp,
    Deck,
    Court,
    Wall,
    Steep,
    Other,
    Air,
    Unavailable,
}

struct Script {
    step: u32,
    intent: Option<LocomotionIntent>,
}

fn fixed_dt(game: &Game) -> Duration {
    game.world.resource::<FixedTimestep>().step()
}

fn course(config: usize, visual: BodyVisual) -> Game {
    let mut game = Game::new(GameConfig::default().with_size(common::W, common::H));
    spatial_scene::art::register(&mut game.assets());
    let scene = locomotion::build_course(&mut game.world, config, visual);
    game.world.insert_resource(scene);
    {
        let mut actions = game.world.resource_mut::<Actions>();
        actions.bind_axis(scene::WALK_X_ACTION, ButtonSource::Key(Key::J), ButtonSource::Key(Key::L));
        actions.bind_axis(scene::WALK_Z_ACTION, ButtonSource::Key(Key::K), ButtonSource::Key(Key::I));
        actions.bind_button(RUN_ACTION, ButtonSource::Key(Key::H));
        actions.bind_button(JUMP_ACTION, ButtonSource::Key(Key::N));
    }
    game.world.insert_resource(Script { step: 0, intent: None });
    game.add_fixed_system("locomotion_motion", locomotion::locomotion_motion);
    game.add_fixed_system("script", apply_script);
    game.add_system("camera", scene::camera_system);
    game.add_system("motion", scene::motion_system);
    game
}

fn apply_script(world: &mut World) {
    let intent = world.resource::<Script>().intent;
    let walker = world.resource::<Scene>().walker;
    world.resource_mut::<Script>().step += 1;
    if let Some(intent) = intent {
        world.insert(walker, (intent,)).unwrap();
    }
}

fn walker(game: &Game) -> Entity {
    game.world.resource::<Scene>().walker
}

fn motion(game: &Game) -> CharacterMotion {
    *game.world.component::<CharacterMotion>(walker(game)).unwrap()
}

fn position(game: &Game) -> Vec3 {
    game.world.component::<Transform>(walker(game)).unwrap().position
}

fn step(game: &mut Game) {
    game.step(fixed_dt(game));
}

fn drive(game: &mut Game, wish: Vec3, run: bool, jump: bool) {
    game.world.resource_mut::<Script>().intent = Some(LocomotionIntent { wish, jump, run });
    step(game);
}

fn support_kind(game: &Game) -> SupportKind {
    let support = motion(game).support;
    let GroundSupport::Supported { entity, .. } = support else {
        return match support {
            GroundSupport::Unknown => SupportKind::Unknown,
            GroundSupport::Airborne => SupportKind::Air,
            GroundSupport::Unavailable => SupportKind::Unavailable,
            GroundSupport::Supported { .. } => SupportKind::Other,
        };
    };
    let name = game.world.component::<Name>(entity).map(|name| name.0.clone()).unwrap_or_default();
    match name.as_str() {
        "start floor" => SupportKind::Floor,
        "walk ramp" => SupportKind::Ramp,
        "upper deck" => SupportKind::Deck,
        "lower court" => SupportKind::Court,
        "slide wall" | "ramp wall" => SupportKind::Wall,
        "steep face" => SupportKind::Steep,
        _ => SupportKind::Other,
    }
}

fn landings(game: &mut Game) -> Rc<RefCell<Vec<LocomotionEvent>>> {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let slot = seen.clone();
    game.add_system("landings", move |world| {
        slot.borrow_mut().extend(world.events::<LocomotionEvent>());
    });
    seen
}

fn triggers(game: &mut Game) -> Rc<RefCell<Vec<TriggerEvent>>> {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let slot = seen.clone();
    game.add_system("triggers", move |world| {
        slot.borrow_mut().extend(world.events::<TriggerEvent>());
    });
    seen
}

fn horizontal(velocity: Vec3) -> f32 {
    Vec3::new(velocity.x, 0.0, velocity.z).length()
}

#[test]
fn the_default_scene_is_not_the_course() {
    let game = default_game(0);
    assert!(!game.world.resource::<Scene>().locomotion);
    assert!(game.world.find_by_name("start floor").into_iter().next().is_none());
    assert!(game.world.find_by_name("walk ramp").into_iter().next().is_none());
}

#[test]
fn the_character_spawns_grounded_on_the_floor() {
    let mut game = course(0, BodyVisual::Sprite);
    step(&mut game);
    let state = motion(&game);
    assert_eq!(support_kind(&game), SupportKind::Floor);
    assert_eq!(state.phase, LocomotionPhase::Idle);
    assert_eq!(state.velocity, Vec3::ZERO);
    assert!((position(&game).y - START.y).abs() < 0.05, "{:?}", position(&game));
    assert!(game.world.component::<Collider>(walker(&game)).unwrap().enabled);
    let visual = game.world.resource::<Scene>().walker_visual.unwrap();
    assert!(game.world.component::<Collider>(visual).is_err());
    assert_eq!(game.world.component::<Parent>(visual).unwrap().0, walker(&game));
}

#[test]
fn walking_and_running_accelerate_stop_reverse_and_stay_diagonal() {
    let mut game = course(0, BodyVisual::Sprite);
    step(&mut game);
    let mut previous = 0.0;
    for _ in 0..6 {
        drive(&mut game, Vec3::X, false, false);
        let speed = motion(&game).velocity.x;
        assert!(speed > previous + 5.0, "{speed} vs {previous}");
        assert!(speed <= 90.0 + 0.1, "{speed}");
        previous = speed;
        assert_eq!(motion(&game).phase, LocomotionPhase::Walk);
        assert_eq!(support_kind(&game), SupportKind::Floor);
    }

    for _ in 0..20 {
        drive(&mut game, Vec3::X, true, false);
    }
    let running = horizontal(motion(&game).velocity);
    assert!(running > 120.0, "{running}");
    assert!(running <= 140.1, "{running}");
    assert_eq!(motion(&game).phase, LocomotionPhase::Run);

    let mut slowing = running;
    for _ in 0..8 {
        drive(&mut game, Vec3::ZERO, false, false);
        let speed = horizontal(motion(&game).velocity);
        assert!(speed < slowing, "{speed} did not fall from {slowing}");
        slowing = speed;
    }
    for _ in 0..30 {
        drive(&mut game, Vec3::ZERO, false, false);
    }
    assert!(horizontal(motion(&game).velocity) < 0.05, "{:?}", motion(&game).velocity);
    assert_eq!(motion(&game).phase, LocomotionPhase::Idle);

    for _ in 0..8 {
        drive(&mut game, -Vec3::X, false, false);
    }
    assert!(motion(&game).velocity.x < -40.0, "{:?}", motion(&game).velocity);

    let mut diagonal = course(0, BodyVisual::Sprite);
    step(&mut diagonal);
    for _ in 0..40 {
        drive(&mut diagonal, Vec3::new(1.0, 0.0, 1.0), false, false);
    }
    let speed = horizontal(motion(&diagonal).velocity);
    assert!((speed - 90.0).abs() < 1.0, "diagonal amplified to {speed}");
}

#[test]
fn jump_from_rest_and_while_running_lands_once_and_steers_in_the_air() {
    let mut game = course(0, BodyVisual::Sprite);
    let landed = landings(&mut game);
    step(&mut game);
    drive(&mut game, Vec3::ZERO, false, true);
    assert_eq!(motion(&game).phase, LocomotionPhase::Jump);
    assert!(motion(&game).velocity.y > 150.0, "{:?}", motion(&game).velocity);
    let taken = motion(&game).velocity.x;
    assert!(taken.abs() < 0.1, "rest jump gained horizontal speed {taken}");
    drive(&mut game, Vec3::ZERO, false, true);
    assert!(motion(&game).velocity.y < 200.0, "a second jump stacked");
    let mut saw_air = false;
    for _ in 0..80 {
        drive(&mut game, Vec3::X, false, false);
        if motion(&game).phase == LocomotionPhase::Fall || motion(&game).phase == LocomotionPhase::Jump {
            saw_air = true;
        }
        if support_kind(&game) == SupportKind::Floor && motion(&game).velocity.y == 0.0 && saw_air {
            break;
        }
    }
    assert!(saw_air);
    assert_eq!(support_kind(&game), SupportKind::Floor);
    assert_eq!(landed.borrow().len(), 1, "{:?}", landed.borrow());

    let mut running = course(0, BodyVisual::Mesh);
    let landed = landings(&mut running);
    step(&mut running);
    for _ in 0..24 {
        drive(&mut running, Vec3::X, true, false);
    }
    let before = motion(&running).velocity.x;
    assert!(before > 100.0, "{before}");
    drive(&mut running, Vec3::Z, true, true);
    let after = motion(&running).velocity;
    assert!((after.x - before).abs() < 8.0, "takeoff dropped horizontal speed {before} -> {}", after.x);
    assert!(after.y > 150.0, "{after:?}");
    assert_eq!(motion(&running).phase, LocomotionPhase::Jump);
    for _ in 0..10 {
        drive(&mut running, Vec3::Z, false, false);
    }
    assert!(motion(&running).velocity.z > 20.0, "air steering did not take +Z {:?}", motion(&running).velocity);
    for _ in 0..90 {
        drive(&mut running, Vec3::ZERO, false, false);
        if support_kind(&running) == SupportKind::Floor && landed.borrow().len() == 1 {
            break;
        }
    }
    assert_eq!(landed.borrow().len(), 1, "{:?}", landed.borrow());
}

#[test]
fn the_ramp_wall_ledge_steep_face_and_sensor_use_the_motor() {
    let mut up = course(0, BodyVisual::Sprite);
    let landed = landings(&mut up);
    let tripped = triggers(&mut up);
    step(&mut up);
    let mut climbed = false;
    for _ in 0..180 {
        drive(&mut up, Vec3::X, false, false);
        if support_kind(&up) == SupportKind::Ramp && position(&up).y > 8.0 {
            climbed = true;
            break;
        }
    }
    assert!(climbed, "did not climb the ramp at {:?}", position(&up));
    assert_eq!(motion(&up).velocity.y, 0.0);
    assert_ne!(motion(&up).phase, LocomotionPhase::Jump);

    let before = position(&up);
    for _ in 0..24 {
        drive(&mut up, Vec3::Z, false, false);
    }
    let across = position(&up) - before;
    assert!(across.z > 6.0, "contour did not travel along the ramp {across:?}");
    assert!(across.y.abs() < 4.0, "contour climbed instead of following the ramp {across:?}");
    assert_eq!(support_kind(&up), SupportKind::Ramp);

    drive(&mut up, Vec3::X, false, true);
    assert_eq!(motion(&up).phase, LocomotionPhase::Jump);
    let mut back = false;
    for _ in 0..100 {
        drive(&mut up, Vec3::ZERO, false, false);
        if support_kind(&up) == SupportKind::Ramp && motion(&up).velocity.y == 0.0 {
            back = true;
            break;
        }
    }
    assert!(back, "did not land on the ramp at {:?}", position(&up));
    assert_eq!(landed.borrow().len(), 1, "{:?}", landed.borrow());

    let mut held = 0.0;
    for _ in 0..40 {
        drive(&mut up, Vec3::X, true, false);
        if support_kind(&up) == SupportKind::Ramp {
            held = position(&up).y;
        }
    }
    let stuck = position(&up).y;
    for _ in 0..20 {
        drive(&mut up, Vec3::X, true, false);
    }
    assert!((position(&up).y - stuck).abs() < 1.5, "the ramp wall bounced {:?}", position(&up));
    assert!(horizontal(motion(&up).velocity) < 5.0, "{:?}", motion(&up).velocity);
    assert!(stuck + 1.0 >= held);

    let mut down = course(0, BodyVisual::Sprite);
    step(&mut down);
    let mut climbed = false;
    for _ in 0..160 {
        drive(&mut down, Vec3::X, true, false);
        if support_kind(&down) == SupportKind::Ramp && position(&down).y > 12.0 {
            climbed = true;
            break;
        }
    }
    assert!(climbed, "did not get up the ramp before turning around at {:?}", position(&down));
    let high = position(&down).y;
    for _ in 0..80 {
        drive(&mut down, -Vec3::X, true, false);
    }
    assert!(position(&down).y < high - 4.0, "downhill did not lose height {:?}", position(&down));

    let mut slide = course(0, BodyVisual::Sprite);
    step(&mut slide);
    for _ in 0..80 {
        drive(&mut slide, Vec3::Z, true, false);
    }
    let blocked_z = position(&slide).z;
    for _ in 0..20 {
        drive(&mut slide, Vec3::new(1.0, 0.0, 1.0), true, false);
    }
    assert!(position(&slide).x > START.x + 10.0, "did not slide along the wall {:?}", position(&slide));
    assert!(position(&slide).z < blocked_z + 2.0, "slid through the wall {:?}", position(&slide));

    let mut steep = course(0, BodyVisual::Sprite);
    step(&mut steep);
    let start_y = position(&steep).y;
    for _ in 0..50 {
        drive(&mut steep, -Vec3::X, true, false);
    }
    assert!(position(&steep).y < start_y + 2.0, "the 60° face was walkable {:?}", position(&steep));
    assert_ne!(support_kind(&steep), SupportKind::Steep);

    assert!(tripped.borrow().is_empty(), "the ramp walk should not need the sensor");
    let mut sensed = course(0, BodyVisual::Sprite);
    let tripped = triggers(&mut sensed);
    step(&mut sensed);
    for _ in 0..80 {
        drive(&mut sensed, Vec3::Z, false, false);
    }
    assert!(!tripped.borrow().is_empty(), "sensor emitted nothing at {:?}", position(&sensed));
    let during = position(&sensed);
    drive(&mut sensed, Vec3::Z, false, false);
    assert!(position(&sensed).z > during.z, "the sensor blocked movement");

    let mut ledge = course(0, BodyVisual::Sprite);
    let landed = landings(&mut ledge);
    step(&mut ledge);
    // The ramp wall covers the start lane. Move to the open −Z side, then climb.
    for _ in 0..24 {
        drive(&mut ledge, -Vec3::Z, true, false);
    }
    assert!(position(&ledge).z < 210.0 && position(&ledge).z > 160.0, "left the open lane at {:?}", position(&ledge));
    let mut on_deck = false;
    for _ in 0..500 {
        drive(&mut ledge, Vec3::X, true, false);
        if support_kind(&ledge) == SupportKind::Deck {
            on_deck = true;
            break;
        }
    }
    assert!(on_deck, "did not reach the deck at {:?}", position(&ledge));
    // A 30° ramp meeting a flat deck is a sharp crest. Running onto it may leave the ground once.
    // Staying there must not keep emitting landings.
    let at_lip = landed.borrow().len();
    for _ in 0..24 {
        drive(&mut ledge, Vec3::ZERO, false, false);
    }
    let settled = landed.borrow().len();
    assert!(at_lip <= 2, "the ramp lip landed repeatedly at {:?}: {:?}", position(&ledge), landed.borrow());
    assert_eq!(settled, at_lip, "the deck kept landing {:?}", landed.borrow());
    assert_eq!(support_kind(&ledge), SupportKind::Deck, "left the deck while stopping {:?}", position(&ledge));
    let mut fell = false;
    for _ in 0..200 {
        drive(&mut ledge, Vec3::X, true, false);
        if motion(&ledge).phase == LocomotionPhase::Fall {
            fell = true;
        }
        if support_kind(&ledge) == SupportKind::Court && fell {
            break;
        }
    }
    assert!(fell, "did not leave the deck at {:?}", position(&ledge));
    assert_eq!(support_kind(&ledge), SupportKind::Court, "{:?}", position(&ledge));
    assert_eq!(landed.borrow().len(), settled + 1, "court landing {:?}", landed.borrow());
}

#[test]
fn cameras_and_visuals_do_not_change_the_root_trace() {
    fn trace(config: usize, visual: BodyVisual) -> Vec<Vec3> {
        let mut game = course(config, visual);
        let mut points = Vec::new();
        step(&mut game);
        for index in 0..40 {
            let run = index > 15;
            drive(&mut game, Vec3::X, run, index == 20);
            points.push(position(&game));
        }
        points
    }
    let perspective = cameras::index("perspective");
    let orthographic = cameras::index("orthographic");
    let isometric = cameras::index("isometric");
    let sprite = trace(perspective, BodyVisual::Sprite);
    assert_eq!(sprite, trace(orthographic, BodyVisual::Sprite));
    assert_eq!(sprite, trace(isometric, BodyVisual::Sprite));
    assert_eq!(sprite, trace(perspective, BodyVisual::Mesh));
}

#[test]
fn input_edges_zero_frames_and_missing_components_follow_the_fixed_step() {
    let mut game = course(0, BodyVisual::Sprite);
    step(&mut game);
    let rest = position(&game);
    game.world.resource_mut::<Script>().intent = None;
    game.step(Duration::ZERO);
    assert_eq!(position(&game), rest);
    match motion(&game).support {
        GroundSupport::Supported { entity, .. } => assert_eq!(entity, named(&game, "start floor")),
        other => panic!("a zero-iteration frame dropped support {other:?}"),
    }

    game.send_input(InputEvent::Key { key: Key::N, pressed: true });
    step(&mut game);
    assert_eq!(motion(&game).phase, LocomotionPhase::Jump);
    let rise = motion(&game).velocity.y;
    step(&mut game);
    assert!(motion(&game).velocity.y < rise, "the jump edge repeated");
    game.send_input(InputEvent::Key { key: Key::N, pressed: false });

    let mut several = course(0, BodyVisual::Sprite);
    step(&mut several);
    several.world.resource_mut::<Script>().intent = Some(LocomotionIntent::new(Vec3::X));
    let once = {
        let mut one = course(0, BodyVisual::Sprite);
        step(&mut one);
        one.world.resource_mut::<Script>().intent = Some(LocomotionIntent::new(Vec3::X));
        step(&mut one);
        position(&one).x
    };
    several.step(fixed_dt(&several) * 3);
    let gained = position(&several).x - START.x;
    let one_step = once - START.x;
    // Step k from rest adds 10 units/s. One step travels 1/6. Three steps travel 1.
    assert!((one_step - 1.0 / 6.0).abs() < 0.02, "one step {one_step}");
    assert!((gained - 1.0).abs() < 0.05, "three steps {gained}");

    let mut missing = course(0, BodyVisual::Sprite);
    step(&mut missing);
    let body = walker(&missing);
    missing.world.remove::<CharacterLocomotion>(body).unwrap();
    let stayed = position(&missing);
    missing.world.resource_mut::<Script>().intent = Some(LocomotionIntent::running(Vec3::X));
    step(&mut missing);
    assert!((position(&missing).x - stayed.x).abs() < 1e-3, "a missing component still locomoted");
    missing.world.insert(body, (MoveIntent::new(Vec3::X),)).unwrap();
    step(&mut missing);
    assert!(position(&missing).x > stayed.x + 0.5, "the motor did not accept a direct intent");

    let mut disabled = course(0, BodyVisual::Sprite);
    step(&mut disabled);
    for _ in 0..10 {
        drive(&mut disabled, Vec3::X, true, false);
    }
    assert!(motion(&disabled).velocity.x > 40.0);
    let mut settings = *disabled.world.component::<CharacterLocomotion>(walker(&disabled)).unwrap();
    settings.enabled = false;
    disabled.world.insert(walker(&disabled), (settings,)).unwrap();
    step(&mut disabled);
    assert_eq!(motion(&disabled).velocity, Vec3::ZERO);
}

#[test]
fn a_scripted_thousand_steps_replay_in_two_worlds() {
    fn play() -> Vec<Sample> {
        let mut game = course(0, BodyVisual::Sprite);
        let landed = landings(&mut game);
        let tripped = triggers(&mut game);
        let mut samples = Vec::with_capacity(1000);
        step(&mut game);
        for index in 0..1000 {
            let (wish, run, jump) = scripted(index);
            drive(&mut game, wish, run, jump);
            samples.push(Sample {
                position: position(&game),
                velocity: motion(&game).velocity,
                phase: motion(&game).phase,
                support: support_kind(&game),
                landings: landed.borrow().len() as u32,
                triggers: tripped.borrow().len() as u32,
                blocked: blocked(&game),
            });
        }
        samples
    }
    let left = play();
    let right = play();
    assert_eq!(left.len(), 1000);
    assert_eq!(left, right);
    assert!(left.iter().any(|sample| sample.phase == LocomotionPhase::Run), "the script never ran");
    assert!(left.iter().any(|sample| sample.phase == LocomotionPhase::Jump), "the script never jumped");
    assert!(left.iter().any(|sample| sample.phase == LocomotionPhase::Fall), "the script never fell");
    assert!(left.last().unwrap().landings >= 1, "the script never landed");
    assert!(left.iter().any(|sample| sample.support == SupportKind::Ramp), "the script never climbed");
    assert!(
        left.iter().any(|sample| sample.blocked && sample.position.z > 360.0),
        "the script never held against the slide wall"
    );
    assert!(left.iter().any(|sample| sample.triggers > 0), "the script never entered the sensor");
}

fn blocked(game: &Game) -> bool {
    game.world.component::<MoveResult>(walker(game)).map(|result| result.status).ok() == Some(MoveStatus::Blocked)
}

fn scripted(index: u32) -> (Vec3, bool, bool) {
    let jump = index == 40 || index == 210 || index == 700;
    let (wish, run) = match index {
        0..30 => (Vec3::X, false),
        30..50 => (Vec3::ZERO, false),
        50..90 => (-Vec3::X, false),
        90..110 => (Vec3::new(1.0, 0.0, 1.0), false),
        110..210 => (Vec3::Z, true),
        210..250 => (Vec3::new(1.0, 0.0, 1.0), true),
        250..340 => (-Vec3::Z, true),
        340..380 => (Vec3::ZERO, false),
        380..560 => (Vec3::X, true),
        560..640 => (-Vec3::X, true),
        640..690 => (-Vec3::Z, true),
        690..920 => (Vec3::X, true),
        _ => (Vec3::ZERO, false),
    };
    (wish, run, jump)
}

fn named(game: &Game, name: &str) -> Entity {
    game.world.find_by_name(name).into_iter().next().unwrap_or_else(|| panic!("missing {name}"))
}

mod cameras {
    pub fn index(name: &str) -> usize {
        spatial_scene::cameras::by_name(name).unwrap_or_else(|| panic!("missing camera {name}"))
    }
}
