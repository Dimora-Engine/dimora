//! Fixed-step interpolation through `Game::step`. Rendering samples the world between ticks.
//! The blend is one tick behind immediately after a step that consumes its leftover time.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use engine::prelude::*;
use engine::renderer_wgpu::RenderError;

fn prepared() -> Game {
    let mut game = Game::new(GameConfig::default());
    {
        let mut assets = game.assets();
        assets.insert("cube", MeshData::cuboid(Vec3::ONE).unwrap()).unwrap();
        let texture = assets.insert("white", Texture::solid(1, 1, [255, 0, 0, 255])).unwrap();
        assets.insert("sprite", SpriteAsset::whole_texture(texture, 1, 1)).unwrap();
    }
    game.world.spawn((Transform::from_xyz(0.0, 0.0, -8.0), Camera::orthographic(30.0)));
    game
}

fn step_dt(game: &Game) -> Duration {
    game.world.resource::<FixedTimestep>().step()
}

fn sim_x(game: &Game, entity: Entity) -> f32 {
    game.world.component::<Transform>(entity).unwrap().position.x
}

fn extracted_x(game: &Game) -> f32 {
    game.extract().expect("camera").meshes[0].model.translation().x
}

#[test]
fn the_first_frame_with_no_history_renders_the_simulation_pose() {
    let mut game = prepared();
    let entity = game.world.spawn((Transform::from_xyz(5.0, 0.0, 0.0), Interpolate::new(), Mesh::new("cube")));
    game.step(Duration::ZERO);
    assert_eq!(game.world.resource::<Diagnostics>().fixed_steps, 0);
    assert!(game.world.component::<Interpolate>(entity).unwrap().previous.is_none());
    assert_eq!(extracted_x(&game), 5.0);
    assert_eq!(sim_x(&game, entity), 5.0);
    assert_eq!(game.world.component::<GlobalTransform>(entity).unwrap().translation().x, 5.0);
}

#[test]
fn zero_one_and_many_fixed_steps_and_the_stall_cap() {
    let mut game = prepared();
    let entity = game.world.spawn((Transform::from_xyz(0.0, 0.0, 0.0), Interpolate::new(), Mesh::new("cube")));
    let seen = Rc::new(RefCell::new(Vec::new()));
    let globals = Rc::new(RefCell::new(Vec::new()));
    let record = Rc::clone(&seen);
    let record_global = Rc::clone(&globals);
    game.add_fixed_system("move", move |world| {
        let x = world.component::<Transform>(entity).unwrap().position.x;
        record.borrow_mut().push(x);
        if let Ok(global) = world.component::<GlobalTransform>(entity) {
            record_global.borrow_mut().push(global.translation().x);
        }
        world.component_mut::<Transform>(entity).unwrap().position.x = x + 1.0;
    });

    let step = step_dt(&game);
    game.step(step / 2);
    assert_eq!(game.world.resource::<Diagnostics>().fixed_steps, 0);
    assert!(seen.borrow().is_empty(), "no fixed system ran");
    assert!(game.world.component::<Interpolate>(entity).unwrap().previous.is_none());
    assert_eq!(extracted_x(&game), 0.0, "no history yet, so a partial step still shows the simulation");
    assert!((game.world.resource::<FixedTimestep>().alpha() - 0.5).abs() < 1e-4);

    game.step(step - step / 2);
    assert_eq!(game.world.resource::<Diagnostics>().fixed_steps, 1);
    assert_eq!(seen.borrow().as_slice(), &[0.0]);
    assert_eq!(sim_x(&game, entity), 1.0);
    assert_eq!(game.world.resource::<FixedTimestep>().alpha(), 0.0);
    assert_eq!(extracted_x(&game), 0.0, "alpha 0 shows the pose from the start of the tick");
    assert_eq!(game.world.component::<GlobalTransform>(entity).unwrap().translation().x, 1.0);

    let again = extracted_x(&game);
    game.step(Duration::ZERO);
    assert_eq!(game.world.resource::<Diagnostics>().fixed_steps, 0);
    assert_eq!(sim_x(&game, entity), 1.0);
    assert_eq!(extracted_x(&game), again);
    assert_eq!(extracted_x(&game), again, "repeated extraction does not move");

    game.step(step / 2);
    assert_eq!(game.world.resource::<Diagnostics>().fixed_steps, 0);
    assert!((game.world.resource::<FixedTimestep>().alpha() - 0.5).abs() < 1e-4);
    assert!((extracted_x(&game) - 0.5).abs() < 1e-3);
    assert_eq!(sim_x(&game, entity), 1.0);
    let stable = extracted_x(&game);
    assert_eq!(extracted_x(&game), stable);

    game.step(step * 2);
    assert_eq!(game.world.resource::<Diagnostics>().fixed_steps, 2);
    assert_eq!(sim_x(&game, entity), 3.0);
    assert_eq!(game.world.component::<Interpolate>(entity).unwrap().previous.unwrap().position.x, 2.0);
    assert!((game.world.resource::<FixedTimestep>().alpha() - 0.5).abs() < 1e-4);
    assert!((extracted_x(&game) - 2.5).abs() < 1e-3, "previous is the local from before the last iteration");
    assert_eq!(seen.borrow().as_slice(), &[0.0, 1.0, 2.0]);
    assert_eq!(
        globals.borrow().as_slice(),
        &[0.0, 1.0, 1.0],
        "later fixed iterations still see the last propagated simulation, not the render blend"
    );

    game.step(Duration::from_secs(10));
    assert_eq!(game.world.resource::<Diagnostics>().fixed_steps, 8);
    assert_eq!(game.world.resource::<FixedTimestep>().alpha(), 0.0);
    assert_eq!(sim_x(&game, entity), 11.0);
    assert_eq!(game.world.component::<Interpolate>(entity).unwrap().previous.unwrap().position.x, 10.0);
    assert!((extracted_x(&game) - 10.0).abs() < 1e-3, "the cap drops the leftover and alpha is 0");
    let held = extracted_x(&game);
    game.step(Duration::ZERO);
    assert_eq!(sim_x(&game, entity), 11.0);
    assert_eq!(extracted_x(&game), held);
    assert_eq!(game.world.resource::<FixedTimestep>().alpha(), 0.0);
}

#[test]
fn a_variable_step_write_is_what_update_reads_and_is_included_in_the_blend() {
    let mut game = prepared();
    let entity = game.world.spawn((Transform::IDENTITY, Interpolate::new(), Mesh::new("cube")));
    let update_saw = Rc::new(Cell::new(f32::NAN));
    let saw = Rc::clone(&update_saw);
    game.add_fixed_system("move", move |world| {
        world.component_mut::<Transform>(entity).unwrap().position.x += 1.0;
    });
    game.add_system("nudge", move |world| {
        let x = world.component::<Transform>(entity).unwrap().position.x;
        saw.set(x);
        world.component_mut::<Transform>(entity).unwrap().position.x = x + 0.25;
    });

    let step = step_dt(&game);
    game.step(step);
    assert_eq!(update_saw.get(), 1.0, "update reads the fixed write, not the render pose");
    assert_eq!(sim_x(&game, entity), 1.25);
    assert_eq!(extracted_x(&game), 0.0);

    game.step(step / 2);
    assert_eq!(game.world.resource::<Diagnostics>().fixed_steps, 0);
    assert_eq!(update_saw.get(), 1.25);
    assert_eq!(sim_x(&game, entity), 1.5);
    assert!((extracted_x(&game) - 0.75).abs() < 1e-3, "the blend includes the variable-step write");
    assert_eq!(game.world.component::<GlobalTransform>(entity).unwrap().translation().x, 1.5);
}

#[test]
fn spawning_and_despawning_do_not_invent_history() {
    let mut game = prepared();
    let spawned = Rc::new(Cell::new(None));
    let slot = Rc::clone(&spawned);
    game.add_fixed_system("spawn", move |world| {
        let Some(entity) = slot.get() else {
            let entity = world.spawn(());
            world
                .commands()
                .insert(entity, (Transform::from_xyz(4.0, 0.0, 0.0), Interpolate::new(), Mesh::new("cube")));
            slot.set(Some(entity));
            return;
        };
        world.component_mut::<Transform>(entity).unwrap().position.x += 1.0;
    });

    let step = step_dt(&game);
    game.step(step);
    let entity = spawned.get().unwrap();
    assert!(game.world.component::<Interpolate>(entity).unwrap().previous.is_none());
    assert_eq!(sim_x(&game, entity), 4.0);
    assert_eq!(extracted_x(&game), 4.0);

    game.step(step);
    assert_eq!(game.world.component::<Interpolate>(entity).unwrap().previous.unwrap().position.x, 4.0);
    assert_eq!(sim_x(&game, entity), 5.0);
    assert_eq!(game.world.resource::<FixedTimestep>().alpha(), 0.0);
    assert_eq!(extracted_x(&game), 4.0);

    game.world.despawn(entity).unwrap();
    assert!(game.extract().unwrap().meshes.is_empty());
}

#[test]
fn gaining_and_losing_the_marker_returns_to_the_simulation_pose() {
    let mut game = prepared();
    let entity = game.world.spawn((Transform::from_xyz(2.0, 0.0, 0.0), Mesh::new("cube")));
    game.add_fixed_system("move", move |world| {
        world.component_mut::<Transform>(entity).unwrap().position.x += 1.0;
    });
    let step = step_dt(&game);
    game.step(step / 2);
    assert_eq!(extracted_x(&game), sim_x(&game, entity));

    game.world.insert(entity, (Interpolate::new(),)).unwrap();
    assert_eq!(extracted_x(&game), sim_x(&game, entity), "no stored previous yet");

    game.step(step - step / 2);
    assert_eq!(sim_x(&game, entity), 3.0);
    assert_eq!(extracted_x(&game), 2.0);

    game.world.remove::<Interpolate>(entity).unwrap();
    assert_eq!(extracted_x(&game), 3.0);
    assert_eq!(game.world.component::<GlobalTransform>(entity).unwrap().translation().x, 3.0);
}

#[test]
fn sixty_hertz_motion_sampled_at_one_hundred_forty_four_hertz_stays_behind_and_smooth() {
    let mut game = prepared();
    let camera = game.world.query::<&Camera>().iter().next().unwrap().0;
    *game.world.component_mut::<Camera>(camera).unwrap() = Camera::orthographic(500.0);
    let entity = game.world.spawn((Transform::IDENTITY, Interpolate::new(), Mesh::new("cube")));
    game.add_fixed_system("move", move |world| {
        world.component_mut::<Transform>(entity).unwrap().position.x += 1.0;
    });

    let frame = Duration::from_secs_f64(1.0 / 144.0);
    let mut last_sim = 0.0;
    let mut last_extracted = 0.0;
    let mut saw_delay = false;
    for _ in 0..144 {
        game.step(frame);
        let steps = game.world.resource::<Diagnostics>().fixed_steps;
        let sim = sim_x(&game, entity);
        if steps == 0 {
            assert_eq!(sim, last_sim);
        } else {
            assert_eq!(steps, 1, "a 144 Hz frame cannot contain two 60 Hz ticks");
            assert_eq!(sim, last_sim + 1.0);
        }
        let before = sim_x(&game, entity);
        let extracted = extracted_x(&game);
        assert_eq!(sim_x(&game, entity), before);
        assert_eq!(game.world.component::<GlobalTransform>(entity).unwrap().translation().x, sim);

        let alpha = game.world.resource::<FixedTimestep>().alpha();
        assert!(alpha.is_finite() && (0.0..=1.0).contains(&alpha));
        match game.world.component::<Interpolate>(entity).unwrap().previous {
            Some(previous) => {
                let expected = previous.position.x + (sim - previous.position.x) * alpha;
                assert!((extracted - expected).abs() < 1e-3, "extracted {extracted} expected {expected}");
                if alpha < 0.25 && sim >= previous.position.x + 1.0 {
                    assert!(extracted < sim - 0.5, "extracted {extracted} sim {sim} alpha {alpha}");
                    saw_delay = true;
                }
            }
            None => assert_eq!(extracted, sim),
        }
        assert!(extracted + 1e-4 >= last_extracted, "extracted went from {last_extracted} to {extracted}");
        last_sim = sim;
        last_extracted = extracted;
    }
    assert!(saw_delay, "a consuming tick leaves the picture on the previous pose");
    assert!((58.0..=60.0).contains(&last_sim), "about one second of 60 Hz ticks, got {last_sim}");
    assert!(last_extracted + 1.0 >= last_sim && last_extracted <= last_sim);
}

#[test]
fn interpolated_sprites_and_meshes_move_on_the_gpu_between_fixed_steps() {
    let mut game = Game::new(GameConfig::default().with_size(64, 64));
    {
        let mut assets = game.assets();
        let texture = assets.insert("white", Texture::solid(1, 1, [255, 0, 0, 255])).unwrap();
        assets.insert("sprite", SpriteAsset::whole_texture(texture, 1, 1)).unwrap();
        assets.insert("cube", MeshData::cuboid(Vec3::splat(0.35)).unwrap()).unwrap();
    }
    let camera = game.world.spawn((Transform::from_xyz(0.0, 0.0, -5.0), Camera::orthographic(4.0)));
    let marked = game.world.spawn((
        Transform::from_xyz(-0.8, 0.8, 0.0),
        Interpolate::new(),
        Sprite::new("sprite").with_size(Vec2::splat(0.3)).with_anchor(Vec2::splat(0.5)).unlit().cutout(),
    ));
    let marked_mesh = game.world.spawn((
        Transform::from_xyz(-0.8, 0.0, 0.0),
        Interpolate::new(),
        Mesh::new("cube").with_material(Material::unlit(Color::rgb(0.0, 0.0, 1.0))),
    ));
    game.world.spawn((
        Transform::from_xyz(0.8, -0.8, 0.0),
        Sprite::new("sprite").with_size(Vec2::splat(0.3)).with_anchor(Vec2::splat(0.5)).unlit().cutout(),
    ));
    let moved = Rc::new(Cell::new(false));
    let once = Rc::clone(&moved);
    game.add_fixed_system("move", move |world| {
        if once.get() {
            return;
        }
        once.set(true);
        world.component_mut::<Transform>(marked).unwrap().position.x = 0.8;
        world.component_mut::<Transform>(marked_mesh).unwrap().position.x = 0.8;
    });

    let step = step_dt(&game);
    game.step(step);
    assert_eq!(sim_x(&game, marked), 0.8);
    assert!((extracted_mesh_x(&game) + 0.8).abs() < 1e-3, "alpha 0 stays on the previous pose");
    let Some(at_previous) = render(&mut game) else { return };
    let (marked_left, control_right, mesh_left) = columns(&at_previous);

    game.step(step / 2);
    assert!((game.world.resource::<FixedTimestep>().alpha() - 0.5).abs() < 1e-4);
    assert!(extracted_mesh_x(&game).abs() < 1e-3);
    assert_eq!(sim_x(&game, marked), 0.8);
    assert_eq!(sim_x(&game, marked_mesh), 0.8);
    let Some(halfway) = render(&mut game) else { return };
    let (marked_mid, control_mid, mesh_mid) = columns(&halfway);

    assert!(marked_mid > marked_left + 4.0, "sprite moved right: {marked_left} -> {marked_mid}");
    assert!(mesh_mid > mesh_left + 4.0, "mesh moved right: {mesh_left} -> {mesh_mid}");
    assert!((control_mid - control_right).abs() < 1.5, "the unmarked sprite stays put");
    assert!(marked_mid + 4.0 < control_mid, "the marked sprite has not caught the simulation pose");
    assert_eq!(sim_x(&game, marked), 0.8, "rendering does not write the simulation");

    *game.world.component_mut::<Camera>(camera).unwrap() = Camera::perspective(50.0);
    let Some(perspective) = render(&mut game) else { return };
    let (marked_persp, control_persp, _) = columns(&perspective);
    assert!(marked_persp + 2.0 < control_persp, "perspective uses the same interpolated pose");
}

fn extracted_mesh_x(game: &Game) -> f32 {
    let scene = game.extract().unwrap();
    let mesh = scene.meshes.iter().find(|mesh| mesh.color[2] > 0.5).expect("blue mesh");
    mesh.model.translation().x
}

fn render(game: &mut Game) -> Option<Vec<u8>> {
    let before = sim_positions(game);
    match game.render_to_rgba(64, 64) {
        Ok(pixels) => {
            assert_eq!(sim_positions(game), before, "rendering does not write transforms");
            Some(pixels)
        }
        Err(RenderError::NoAdapter(message)) if std::env::var("DIMORA_REQUIRE_GPU").ok().as_deref() != Some("1") => {
            eprintln!("skipping GPU interpolation test: {message}");
            None
        }
        Err(err) => panic!("render failed: {err}"),
    }
}

fn sim_positions(game: &Game) -> Vec<(Entity, Transform)> {
    let mut poses: Vec<_> =
        game.world.query::<&Transform>().iter().map(|(entity, transform)| (entity, *transform)).collect();
    poses.sort_by_key(|(entity, _)| entity.id());
    poses
}

fn columns(image: &[u8]) -> (f32, f32, f32) {
    (centroid(image, is_red, 0, 26), centroid(image, is_red, 38, 64), centroid(image, is_blue, 26, 38))
}

fn centroid(image: &[u8], pred: fn([u8; 4]) -> bool, y0: u32, y1: u32) -> f32 {
    let mut sum = 0.0;
    let mut count = 0.0;
    for y in y0..y1 {
        for x in 0..64 {
            if pred(pixel(image, x, y)) {
                sum += x as f32;
                count += 1.0;
            }
        }
    }
    assert!(count > 0.0, "no matching pixels in y {y0}..{y1}");
    sum / count
}

fn pixel(image: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * 64 + x) * 4) as usize;
    [image[i], image[i + 1], image[i + 2], image[i + 3]]
}

fn is_red(pixel: [u8; 4]) -> bool {
    pixel[0] > 200 && pixel[1] < 40 && pixel[2] < 40
}

fn is_blue(pixel: [u8; 4]) -> bool {
    pixel[2] > 200 && pixel[0] < 40 && pixel[1] < 40
}
