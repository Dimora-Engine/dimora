//! Two spatial environments seen through several camera configurations: sprites and 3D meshes at real depths.
//!
//! The process starts on environment A, the original scene. Tab queues a deferred replacement of the
//! other environment. The camera and the walker stay; they are not members of either environment.
//!
//! * Four identical figures stand at world Z = 0, 100, 300 and 600 (A, B, C, D). Nothing marks them
//!   as "far" or "near"; the camera's projection alone makes D smaller than A.
//! * A pillar (mesh) stands at Z = 200: figure B (Z = 100) is in front of it and figure C (Z = 300)
//!   is behind it. A spinning cube (mesh) cuts straight through a tree sprite at Z = 400.
//! * A walker (W) patrols along X: a sprite moved by a plain `Transform` write each frame. It is not
//!   part of either environment, so switching scenes leaves it where it is.
//! * Translucent ghosts are alpha-blended after the opaque geometry, still depth-tested; two pairs overlap
//!   at equal depth, one with an explicit `RenderOrder`.
//! * At the left, flat decals lie on the ground at equal and nearly equal heights, cutout and blended: the
//!   coplanar cases that depth alone cannot order (see docs/SPATIAL_RENDERING.md, "Findings").
//! * Environment A is unlit, so what you see there is the spatial projection. Environment B is a
//!   smaller yard in the same view: a lit mesh, unlit sprites, and one point light.
//!
//! Configurations (`--camera NAME`, key `C`): perspective, orthographic, isometric, top-down, steep, side,
//! custom. `custom` is an example-local oblique matrix. It is not an engine preset. Each configuration is a
//! `Camera` plus a `Transform` over the same world and the same renderer.
//!
//! Controls. These keys are gameplay actions. The mouse wheel stays raw input.
//! ```text
//! W / S        move the camera along its horizontal heading (depth)
//! A / D        strafe (horizontal)
//! Space/Shift  move up / down (vertical)
//! Arrow keys   rotate the camera (yaw / pitch)
//! C            next camera configuration (`custom` is last, then perspective)
//! P            toggle perspective / orthographic; a custom projection is left unchanged
//! O            cycle the figures' sprite orientation: billboard, upright, fixed
//! R            reset to the current configuration
//! Tab          switch between environment A and environment B
//! Esc          quit
//! ```
//!
//! Tab is one gameplay action. A press queues one replacement and the stage flush runs it. Holding
//! Tab does not queue another. The active environment changes only after that replacement reports
//! success. A failed replacement leaves the current environment in place; the engine logs the failure.
//!
//! Headless: `cargo run --release -p spatial_scene -- --screenshot out.png [--camera NAME | --ortho]
//! [--environment a|b] [--cam x,y,z] [--yaw deg] [--pitch deg] [--size WxH]
//! [--orientation billboard|upright|fixed] [--frames N]`
//!
//! The default environment is A. `--environment b` publishes B before the first step, for a
//! screenshot of that yard. It does not change the default launch.
//!
//! `--frames N` renders N frames instead of one (the saved image is the last), prints the GPU cache counts
//! after the first and the last frame, and exits with an error if they grew.

use engine::prelude::*;
use spatial_scene::art;
use spatial_scene::cameras;
use spatial_scene::scene::{self, Scene, FIGURE_SIZE};
use std::time::Duration;

const MOVE_SPEED: f32 = 240.0;
const TURN_SPEED: f32 = 1.1;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().any(|a| a == name);
    let value = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let (width, height) = value("--size")
        .and_then(|s| s.split_once('x').map(|(w, h)| (w.parse().ok(), h.parse().ok())))
        .and_then(|(w, h)| Some((w?, h?)))
        .unwrap_or((1280, 720));

    let config = cameras::config_from_args(value("--camera").as_deref(), flag("--ortho"))?;

    let mut game = Game::new(GameConfig::default().with_title("Dimora - spatial_scene").with_size(width, height));
    art::register(&mut game.assets());
    let mut scene = scene::build_world(&mut game.world, config);
    match value("--environment").as_deref().unwrap_or("a") {
        "a" => {}
        "b" => scene::replace_environment(&mut game.world, &mut scene, scene::Environment::B)
            .map_err(|error| format!("environment B: {error}"))?,
        other => {
            return Err(format!("unknown environment `{other}`; known: a, b").into());
        }
    }

    if let Some(cam) = value("--cam") {
        let v: Vec<f32> = cam.split(',').filter_map(|n| n.trim().parse().ok()).collect();
        if let [x, y, z] = v[..] {
            scene.rig.position = Vec3::new(x, y, z);
        }
    }
    if let Some(yaw) = value("--yaw").and_then(|v| v.parse::<f32>().ok()) {
        scene.rig.yaw = yaw.to_radians();
    }
    if let Some(pitch) = value("--pitch").and_then(|v| v.parse::<f32>().ok()) {
        scene.rig.pitch = pitch.to_radians();
    }
    if let Some(mode) = value("--orientation") {
        scene.figure_orientation = match mode.as_str() {
            "upright" => SpriteOrientation::Upright,
            "fixed" => SpriteOrientation::Fixed,
            _ => SpriteOrientation::Billboard,
        };
    }
    game.world.insert_resource(scene);
    bind_controls(&mut game);

    game.add_system("controls", controls_system);
    game.add_system("camera", scene::camera_system);
    game.add_system("motion", scene::motion_system);
    game.add_system("switch_environment", scene::request_environment_switch);
    game.add_system("hud", hud_system);
    game.add_to_stage(Stage::PostUpdate, "apply_environment", scene::apply_environment_switch)
        .expect("post_update accepts the switch result");

    match value("--screenshot") {
        Some(path) => {
            let frames = value("--frames").and_then(|v| v.parse().ok()).unwrap_or(1);
            screenshot(game, &path, width, height, frames)
        }
        None => game.run(),
    }
}

fn screenshot(
    mut game: Game,
    path: &str,
    width: u32,
    height: u32,
    frames: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    // A few fixed steps so the camera, props and transform hierarchy have settled.
    for _ in 0..3 {
        game.step(Duration::from_millis(100));
    }
    let mut pixels = game.render_to_rgba(width, height)?;
    let first = game.world.resource::<RenderStats>().cache;
    for _ in 1..frames {
        game.step(Duration::from_millis(16));
        pixels = game.render_to_rgba(width, height)?;
    }
    image::save_buffer(path, &pixels, width, height, image::ColorType::Rgba8)?;
    let stats = *game.world.resource::<RenderStats>();
    println!("saved {path}: {:?}", stats);
    if frames > 1 {
        println!("gpu cache after frame 1: {first:?}; after frame {frames}: {:?}", stats.cache);
        if stats.cache.textures > first.textures || stats.cache.meshes > first.meshes {
            return Err(format!("GPU cache grew over {frames} frames").into());
        }
    }
    Ok(())
}

fn bind_controls(game: &mut Game) {
    let mut actions = game.world.resource_mut::<Actions>();
    actions.bind_axis("strafe", ButtonSource::Key(Key::A), ButtonSource::Key(Key::D));
    actions.bind_axis("advance", ButtonSource::Key(Key::S), ButtonSource::Key(Key::W));
    actions.bind_axis("lift", ButtonSource::Key(Key::Shift), ButtonSource::Key(Key::Space));
    actions.bind_axis("yaw", ButtonSource::Key(Key::Left), ButtonSource::Key(Key::Right));
    actions.bind_axis("pitch", ButtonSource::Key(Key::Up), ButtonSource::Key(Key::Down));
    actions.bind_button("toggle_projection", ButtonSource::Key(Key::P));
    actions.bind_button("next_config", ButtonSource::Key(Key::C));
    actions.bind_button("cycle_orientation", ButtonSource::Key(Key::O));
    actions.bind_button("reset", ButtonSource::Key(Key::R));
    actions.bind_button(scene::SWITCH_ENVIRONMENT, ButtonSource::Key(Key::Tab));
    actions.bind_button("quit", ButtonSource::Key(Key::Escape));
}

fn controls_system(world: &mut World) {
    let dt = world.resource::<Time>().delta_secs();
    let input = world.resource::<Input>();
    let actions = world.resource::<Actions>();
    let strafe = actions.axis(&input, "strafe");
    let advance = actions.axis(&input, "advance") + input.scroll() * 0.4 / dt.max(1e-3);
    let lift = actions.axis(&input, "lift");
    let yaw = actions.axis(&input, "yaw");
    let pitch = actions.axis(&input, "pitch");
    let toggle_projection = actions.pressed("toggle_projection");
    let next_config = actions.pressed("next_config");
    let cycle = actions.pressed("cycle_orientation");
    let reset = actions.pressed("reset");
    let escape = actions.pressed("quit");
    drop(actions);
    drop(input);

    let mut scene = world.resource_mut::<Scene>();
    scene.rig.yaw += yaw * TURN_SPEED * dt;
    scene.rig.pitch = (scene.rig.pitch + pitch * TURN_SPEED * dt).clamp(-1.57, 1.57);
    let (heading, right) = scene.rig.heading();
    scene.rig.position += (heading * advance + right * strafe + Vec3::Y * lift) * MOVE_SPEED * dt;
    scene.rig.position.y = scene.rig.position.y.max(5.0);
    if next_config {
        let next = scene.config + 1;
        scene::select_config(&mut scene, next);
    }
    if reset {
        let current = scene.config;
        scene::select_config(&mut scene, current);
    }
    if toggle_projection {
        scene.projection = scene::swap_builtin_projection(scene.projection);
    }
    if cycle {
        scene.figure_orientation = match scene.figure_orientation {
            SpriteOrientation::Billboard => SpriteOrientation::Upright,
            SpriteOrientation::Upright => SpriteOrientation::Fixed,
            _ => SpriteOrientation::Billboard,
        };
    }
    let orientation = scene.figure_orientation;
    let figures: Vec<Entity> = scene.figures.iter().map(|&(_, e)| e).collect();
    drop(scene);
    for e in figures {
        if let Ok(mut sprite) = world.component_mut::<Sprite>(e) {
            sprite.orientation = orientation;
        }
    }
    if escape {
        world.resource_mut::<WindowControl>().exit = true;
    }
}

/// Window title readout: configuration and camera pose plus, for each lettered figure, its camera-space depth
/// and the height it covers on screen. The numbers are computed with the same `CameraView` the renderer uses.
fn hud_system(world: &mut World) {
    let dt = world.resource::<Time>().delta_secs();
    let mut scene = world.resource_mut::<Scene>();
    scene.title_timer -= dt;
    if scene.title_timer > 0.0 {
        return;
    }
    scene.title_timer = 0.25;
    let (rig, camera, orientation, name, environment) =
        (scene.rig, scene.camera, scene.figure_orientation, scene.config_name(), scene.environment);
    let figures = scene.figures.clone();
    drop(scene);

    let viewport = world.resource::<Viewport>().physical_size_pixels();
    let cam = *world.component::<Camera>(camera).unwrap();
    let projection_name = match cam.projection {
        CameraProjection::Perspective(_) => "perspective",
        CameraProjection::Orthographic(_) => "orthographic",
        CameraProjection::Custom(_) => "custom",
        _ => "projection",
    };
    let view = CameraView::new(&cam, &GlobalTransform { matrix: rig.transform().to_matrix() }, viewport);
    let readout: Vec<String> = figures
        .iter()
        .filter(|(label, _)| "ABCDW".contains(*label))
        .filter_map(|&(label, e)| {
            let base = world.component::<Transform>(e).ok()?.position;
            let (foot, head) = (view.project(base)?, view.project(base + Vec3::Y * FIGURE_SIZE.y)?);
            Some(format!("{label}: depth {:.0}, {:.0}px", foot.view_depth, foot.pixels.y - head.pixels.y))
        })
        .collect();
    let stats = *world.resource::<RenderStats>();
    let diagnostics = *world.resource::<Diagnostics>();
    let title = format!(
        "spatial_scene | env {environment:?} | {name} ({}) | cam ({:.0}, {:.0}, {:.0}) | {:?} | {} | {} draws | gpu cache {} tex {} mesh | {} ents, {} fixed, {:.2} ms cpu",
        projection_name,
        rig.position.x,
        rig.position.y,
        rig.position.z,
        orientation,
        readout.join("  "),
        stats.frame.draw_calls,
        stats.cache.textures,
        stats.cache.meshes,
        diagnostics.entities,
        diagnostics.fixed_steps,
        diagnostics.stages.total().as_secs_f64() * 1000.0,
    );
    world.resource_mut::<WindowControl>().title = Some(title);
}
