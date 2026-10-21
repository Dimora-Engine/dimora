//! A small spatial 2.5D scene.
//!
//! Everything below shares one coordinate system (X right, Y up, +Z deeper into the scene): a
//! tilemap lying on the ground, sprite trees and rocks at different depths, a skeletal player made
//! of sprite parts, and real 3D meshes (a building, mountains, a spinning crate). One camera
//! projects it all; one set of lights shades it all.
//!
//! Controls: WASD/arrows move, Q/E tilt the camera, mouse wheel zooms, P toggles
//! perspective/orthographic, L toggles the lantern, Esc quits.
//!
//! Headless: `cargo run -p basic_2_5d -- --screenshot out.png [--ortho] [--size 1280x720] [--distance 10]`

mod art;
mod character;

use engine::prelude::*;
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};
use std::time::Duration;

const GROUND_ORIGIN: Vec3 = Vec3::new(-40.0, 0.0, -8.0);
const LANTERN_INTENSITY: f32 = 1.7;
const FOG_COLOR: Color = Color::rgb(0.52, 0.66, 0.80);

struct Demo {
    player: Entity,
    camera: Entity,
    lantern: Entity,
    crate_entity: Entity,
    walk: AssetHandle<AnimationClip>,
    idle: AssetHandle<AnimationClip>,
    cam_target: Vec3,
    pitch: f32,
    distance: f32,
    perspective: bool,
    lantern_on: bool,
    title_timer: f32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().any(|a| a == name);
    let value = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let (width, height) = value("--size")
        .and_then(|s| s.split_once('x').map(|(w, h)| (w.parse().ok(), h.parse().ok())))
        .and_then(|(w, h)| Some((w?, h?)))
        .unwrap_or((1280, 720));

    let mut game = Game::new(GameConfig::default().with_title("2.5D engine - basic_2_5d").with_size(width, height));
    art::register(&mut game.assets());
    let mut demo = build_world(&mut game.world, !flag("--ortho"));
    if let Some(d) = value("--distance").and_then(|d| d.parse().ok()) {
        demo.distance = d;
    }
    game.world.insert_resource(demo);

    game.add_system("player", player_system);
    game.add_system("camera", camera_system);
    game.add_system("props", props_system);
    game.add_system("hud", hud_system);

    match value("--screenshot") {
        Some(path) => screenshot(game, &path, width, height),
        None => game.run(),
    }
}

/// Simulates a second of walking right and away from the camera, then saves a frame.
fn screenshot(mut game: Game, path: &str, width: u32, height: u32) -> Result<(), Box<dyn std::error::Error>> {
    use engine::core::InputEvent;
    for key in [Key::D, Key::W] {
        game.send_input(InputEvent::Key { key, pressed: true });
    }
    for _ in 0..75 {
        game.step(Duration::from_millis(16));
    }
    let pixels = game.render_to_rgba(width, height)?;
    image::save_buffer(path, &pixels, width, height, image::ColorType::Rgba8)?;
    let stats = *game.world.resource::<RenderStats>();
    println!("saved {path}: {:?}", stats);
    Ok(())
}

fn build_world(world: &mut World, perspective: bool) -> Demo {
    world.insert_resource(AmbientLight { color: Color::rgb(0.62, 0.72, 1.0), intensity: 0.42 });
    world.insert_resource(Fog { color: FOG_COLOR, density: 0.0115 });

    // Ground: a tilemap laid flat. Rotating +90 degrees about X sends the map's local +Y to world +Z.
    let (cols, rows) = (80u32, 120u32);
    let ground = TileMap::new("ground", 2, 2, Vec2::ONE, cols, rows).filled_with(|x, y| {
        let path_x = 40.0 + 3.0 * (y as f32 * 0.16).sin();
        if (x as f32 - path_x).abs() < 1.6 {
            2
        } else {
            let h = (x * 73 + y * 151) % 17;
            if h == 0 {
                3
            } else if h % 3 == 0 {
                1
            } else {
                0
            }
        }
    });
    world.spawn((
        Transform::from_position(GROUND_ORIGIN).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
        ground,
        Name::new("ground"),
    ));

    // Sprites standing on the ground at assorted depths.
    let tree_spots = [
        (-7.0, 7.0),
        (6.0, 11.0),
        (-3.5, 16.0),
        (9.0, 20.0),
        (-11.0, 24.0),
        (3.0, 27.0),
        (-6.0, 33.0),
        (12.0, 38.0),
        (-14.0, 42.0),
        (1.0, 47.0),
        (-9.0, 53.0),
        (14.0, 58.0),
    ];
    for (x, z) in tree_spots {
        world.spawn((Transform::from_xyz(x, 0.0, z), Sprite::new("tree").cutout().upright()));
    }
    for (x, z) in [(-2.0, 5.0), (4.5, 8.0), (-8.5, 14.0), (7.0, 17.0), (0.5, 24.0)] {
        world.spawn((Transform::from_xyz(x, 0.0, z), Sprite::new("rock").cutout().upright()));
    }
    // Thousands of tufts: all on one texture, so they cost a single draw call.
    for i in 0..3000u32 {
        let h = i.wrapping_mul(2_654_435_761);
        let x = ((h >> 8) & 0xFFF) as f32 / 4095.0 * 60.0 - 30.0;
        let z = ((h.wrapping_mul(40503) >> 6) & 0xFFF) as f32 / 4095.0 * 55.0 - 4.0;
        world.spawn((
            Transform::from_xyz(x, 0.0, z),
            Sprite::new("tuft").with_size(Vec2::splat(0.5)).cutout().upright(),
        ));
    }

    // Real 3D geometry in the same world.
    {
        let mut assets = world.resource_mut::<AssetManager>();
        assets
            .insert("cube", MeshData::cuboid(Vec3::ONE).expect("cuboid dimensions are finite and positive"))
            .expect("example asset names are distinct");
        assets
            .insert(
                "building",
                MeshData::cuboid(Vec3::new(4.0, 4.0, 4.0)).expect("cuboid dimensions are finite and positive"),
            )
            .expect("example asset names are distinct");
        assets.insert("roof", MeshData::cone(3.1, 2.2, 4)).expect("example asset names are distinct");
        assets.insert("mountain", MeshData::cone(1.0, 1.0, 28)).expect("example asset names are distinct");
    }
    world.spawn((
        Transform::from_xyz(10.0, 2.0, 15.0),
        Mesh::new("building").with_material(Material::textured("brick")),
    ));
    world.spawn((
        Transform::from_xyz(10.0, 4.0, 15.0).with_rotation(Quat::from_rotation_y(FRAC_PI_4)),
        Mesh::new("roof").with_material(Material::color(Color::from_srgb8(92, 52, 44))),
    ));
    let mountain = Material::color(Color::from_srgb8(110, 124, 118));
    for (x, z, r, h) in [(-40.0, 62.0, 26.0, 24.0), (-8.0, 80.0, 34.0, 36.0), (34.0, 70.0, 28.0, 26.0)] {
        world.spawn((
            Transform::from_xyz(x, 0.0, z).with_scale(Vec3::new(r, h, r)),
            Mesh::new("mountain").with_material(mountain),
        ));
    }
    let crate_entity = world
        .spawn((Transform::from_xyz(-3.2, 0.9, 6.5), Mesh::new("cube").with_material(Material::textured("crate"))));

    // Lights.
    let sun_dir = Vec3::new(-0.45, -0.75, 0.5).normalized();
    world.spawn((
        Transform::from_rotation(Quat::look_rotation(sun_dir, Vec3::Y)),
        Light::directional(Color::rgb(1.0, 0.93, 0.8), 1.15),
    ));
    world.spawn((
        Transform::from_xyz(-4.0, 6.0, 19.0).with_rotation(Quat::look_rotation(Vec3::new(0.2, -1.0, 0.0), Vec3::Z)),
        Light::spot(Color::rgb(0.5, 0.75, 1.0), 2.4, 14.0, 0.25, 0.6),
    ));
    let lantern = world
        .spawn((Transform::from_xyz(0.0, 1.6, 3.0), Light::point(Color::rgb(1.0, 0.62, 0.28), LANTERN_INTENSITY, 8.0)));

    // Player and camera.
    let ch = character::spawn(world, Vec3::new(0.0, 0.0, 4.0));
    let pitch = 0.36;
    let camera =
        world.spawn((Camera::perspective(45.0).with_clear_color(FOG_COLOR), Transform::IDENTITY, Name::new("camera")));
    Demo {
        player: ch.root,
        camera,
        lantern,
        crate_entity,
        walk: ch.walk,
        idle: ch.idle,
        cam_target: Vec3::new(0.0, 1.2, 4.0),
        pitch,
        distance: 10.0,
        perspective,
        lantern_on: true,
        title_timer: 0.0,
    }
}

fn player_system(world: &mut World) {
    let (dt, input) = (world.resource::<Time>().delta_secs(), world.resource::<Input>());
    let dir = Vec2::new(
        input.axis(Key::A, Key::D) + input.axis(Key::Left, Key::Right),
        input.axis(Key::S, Key::W) + input.axis(Key::Down, Key::Up),
    );
    let escape = input.key_pressed(Key::Escape);
    let (toggle_lantern, toggle_projection) = (input.key_pressed(Key::L), input.key_pressed(Key::P));
    drop(input);

    let mut demo = world.resource_mut::<Demo>();
    let dir = dir.normalized();
    let moving = dir != Vec2::ZERO;
    {
        let mut t = world.component_mut::<Transform>(demo.player).unwrap();
        t.position += Vec3::new(dir.x, 0.0, dir.y) * 4.2 * dt;
        t.position.x = t.position.x.clamp(-30.0, 30.0);
        t.position.z = t.position.z.clamp(-4.0, 60.0);
        if dir.x != 0.0 {
            t.scale.x = dir.x.signum();
        }
    }
    let clip = if moving { demo.walk } else { demo.idle };
    world.component_mut::<AnimationPlayer>(demo.player).unwrap().play(clip);

    if toggle_lantern {
        demo.lantern_on = !demo.lantern_on;
    }
    if toggle_projection {
        demo.perspective = !demo.perspective;
    }
    let player_pos = world.component::<Transform>(demo.player).unwrap().position;
    world.component_mut::<Transform>(demo.lantern).unwrap().position = player_pos + Vec3::new(0.0, 1.7, -0.9);
    world.component_mut::<Light>(demo.lantern).unwrap().intensity =
        if demo.lantern_on { LANTERN_INTENSITY } else { 0.0 };
    if escape {
        world.resource_mut::<WindowControl>().exit = true;
    }
}

fn camera_system(world: &mut World) {
    let dt = world.resource::<Time>().delta_secs();
    let input = world.resource::<Input>();
    let (tilt, zoom) = (input.axis(Key::Q, Key::E), input.scroll());
    drop(input);

    let mut demo = world.resource_mut::<Demo>();
    demo.pitch = (demo.pitch + tilt * 0.8 * dt).clamp(0.05, 1.3);
    demo.distance = (demo.distance - zoom * 0.8).clamp(2.5, 40.0);
    let player = world.component::<Transform>(demo.player).unwrap().position;
    let goal = player + Vec3::new(0.0, 1.2, 0.0);
    // Frame-rate independent exponential smoothing.
    demo.cam_target = demo.cam_target.lerp(goal, 1.0 - (-6.0 * dt).exp());

    let forward = Vec3::new(0.0, -demo.pitch.sin(), demo.pitch.cos());
    let mut t = world.component_mut::<Transform>(demo.camera).unwrap();
    t.position = demo.cam_target - forward * demo.distance;
    t.rotation = Quat::look_rotation(forward, Vec3::Y);

    // Same framing for both projections, so toggling shows only what the projection changes.
    let fov = 45f32.to_radians();
    world.component_mut::<Camera>(demo.camera).unwrap().projection = if demo.perspective {
        CameraProjection::perspective(45.0)
    } else {
        CameraProjection::orthographic(2.0 * demo.distance * (fov * 0.5).tan())
    };
}

fn props_system(world: &mut World) {
    let t = world.resource::<Time>().elapsed_secs();
    let demo = world.resource::<Demo>();
    let mut tr = world.component_mut::<Transform>(demo.crate_entity).unwrap();
    tr.rotation = Quat::from_rotation_y(t * 0.9) * Quat::from_rotation_x(0.35);
    tr.position.y = 1.1 + (t * 1.6).sin() * 0.25;
}

fn hud_system(world: &mut World) {
    let dt = world.resource::<Time>().delta_secs();
    let mut demo = world.resource_mut::<Demo>();
    demo.title_timer -= dt;
    if demo.title_timer > 0.0 {
        return;
    }
    demo.title_timer = 0.25;
    let stats = *world.resource::<RenderStats>();
    let diagnostics = *world.resource::<Diagnostics>();
    let player = world.component::<Transform>(demo.player).unwrap().position;
    let title = format!(
        "2.5D | {} | player ({:.1}, {:.1}, {:.1}) | {} sprites, {} meshes, {} lights | {} draws | gpu cache {} tex {} mesh | {:.0} fps | {} ents, {} fixed, {:.2} ms cpu",
        if demo.perspective { "perspective" } else { "orthographic" },
        player.x,
        player.y,
        player.z,
        stats.extract.sprites_visible,
        stats.extract.meshes_visible,
        stats.extract.lights,
        stats.frame.draw_calls,
        stats.cache.textures,
        stats.cache.meshes,
        if dt > 0.0 { 1.0 / dt } else { 0.0 },
        diagnostics.entities,
        diagnostics.fixed_steps,
        diagnostics.stages.total().as_secs_f64() * 1000.0,
    );
    world.resource_mut::<WindowControl>().title = Some(title);
}
