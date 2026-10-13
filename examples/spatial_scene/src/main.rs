//! One spatial world, one camera: sprites and 3D meshes at real depths.
//!
//! * Four identical figures stand at world Z = 0, 100, 300 and 600 (A, B, C, D). Nothing marks them
//!   as "far" or "near"; the camera's projection alone makes D smaller than A.
//! * A pillar (mesh) stands at Z = 200: figure B (Z = 100) is in front of it and figure C (Z = 300)
//!   is behind it. A spinning cube (mesh) cuts straight through a tree sprite at Z = 400.
//! * A translucent ghost sprite is alpha-blended after the opaque geometry, still depth-tested.
//! * Everything is unlit, so what you see is purely the spatial projection.
//!
//! Controls
//! ```text
//! W / S        move the camera along its horizontal heading (depth)
//! A / D        strafe (horizontal)
//! Space/Shift  move up / down (vertical)
//! Arrow keys   rotate the camera (yaw / pitch)
//! P            toggle perspective / orthographic
//! O            cycle the figures' sprite orientation: billboard, upright, fixed
//! R            reset camera
//! Esc          quit
//! ```
//!
//! Headless: `cargo run --release -p spatial_scene -- --screenshot out.png [--ortho]
//! [--cam x,y,z] [--yaw deg] [--pitch deg] [--size WxH] [--orientation billboard|upright|fixed] [--frames N]`
//!
//! `--frames N` renders N frames instead of one (the saved image is the last), prints the GPU cache counts
//! after the first and the last frame, and exits with an error if they grew.

mod art;

use engine::prelude::*;
use std::time::Duration;

const FOV_DEGREES: f32 = 50.0;
const MOVE_SPEED: f32 = 240.0;
const TURN_SPEED: f32 = 1.1;
const SKY: Color = Color::rgb(0.62, 0.74, 0.88);
const FIGURE_SIZE: Vec2 = Vec2::new(64.0, 96.0);
/// Vertical extent of the orthographic view in world units. Fixed, so moving the camera in depth
/// never zooms an orthographic image (that is the point of the comparison).
const ORTHO_HEIGHT: f32 = 400.0;

/// The four figures: label, world position and tint. Z is the only thing that differs in depth.
const FIGURES: [(char, Vec3, [f32; 3]); 4] = [
    ('A', Vec3::new(-170.0, 0.0, 0.0), [1.00, 0.45, 0.42]),
    ('B', Vec3::new(-18.0, 0.0, 100.0), [1.00, 0.85, 0.30]),
    ('C', Vec3::new(34.0, 0.0, 300.0), [0.45, 0.88, 0.50]),
    ('D', Vec3::new(170.0, 0.0, 600.0), [0.50, 0.68, 1.00]),
];

#[derive(Clone, Copy)]
struct CameraRig {
    position: Vec3,
    yaw: f32,
    pitch: f32,
}

impl CameraRig {
    const HOME: CameraRig = CameraRig { position: Vec3::new(0.0, 215.0, -420.0), yaw: 0.0, pitch: 0.3 };

    fn rotation(&self) -> Quat {
        Quat::from_rotation_y(self.yaw) * Quat::from_rotation_x(self.pitch)
    }

    fn transform(&self) -> Transform {
        Transform::from_position(self.position).with_rotation(self.rotation())
    }

    /// Unit vectors on the ground plane: where the camera is heading and its right.
    fn heading(&self) -> (Vec3, Vec3) {
        let (s, c) = self.yaw.sin_cos();
        (Vec3::new(s, 0.0, c), Vec3::new(c, 0.0, -s))
    }
}

struct Scene {
    camera: Entity,
    cube: Entity,
    figures: Vec<(char, Entity)>,
    rig: CameraRig,
    perspective: bool,
    figure_orientation: SpriteOrientation,
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

    let mut game = Game::new(GameConfig::default().with_title("Dimora - spatial_scene").with_size(width, height));
    art::register(&mut game.assets());
    let mut scene = build_world(&mut game.world, !flag("--ortho"));

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

    game.add_system("controls", controls_system);
    game.add_system("camera", camera_system);
    game.add_system("props", props_system);
    game.add_system("hud", hud_system);

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

fn build_world(world: &mut World, perspective: bool) -> Scene {
    {
        let mut assets = world.resource_mut::<AssetManager>();
        assets
            .insert("floor_tile", MeshData::cuboid(Vec3::new(100.0, 8.0, 100.0)))
            .expect("example asset names are distinct");
        assets.insert("pad", MeshData::cuboid(Vec3::new(84.0, 2.0, 84.0))).expect("example asset names are distinct");
        assets
            .insert("pillar", MeshData::cuboid(Vec3::new(50.0, 200.0, 50.0)))
            .expect("example asset names are distinct");
        assets.insert("cube", MeshData::cuboid(Vec3::splat(80.0))).expect("example asset names are distinct");
        assets.insert("pyramid", MeshData::cone(60.0, 130.0, 4)).expect("example asset names are distinct");
    }
    let panel =
        |color: Color| Material { base_color: color, base_texture: Some(AssetHandle::from_name("panel")), lit: false };

    // Floor: a checkerboard of real slabs, top face at Y = 0.
    let (light, dark) = (Color::rgb(0.80, 0.83, 0.86), Color::rgb(0.52, 0.57, 0.64));
    for i in 0..24 {
        for j in 0..26 {
            let (x, z) = (-1150.0 + i as f32 * 100.0, -950.0 + j as f32 * 100.0);
            let color = if (i + j) % 2 == 0 { light } else { dark };
            world.spawn((Transform::from_xyz(x, -4.0, z), Mesh::new("floor_tile").with_material(panel(color))));
        }
    }

    // A colored pad under each figure, so its depth can be read off the floor.
    for (_, pos, tint) in FIGURES {
        let color = Color::rgb(tint[0], tint[1], tint[2]);
        world.spawn((Transform::from_xyz(pos.x, 1.0, pos.z), Mesh::new("pad").with_material(panel(color))));
    }

    // Two feature meshes. The pillar sits between figures B (in front) and C (behind).
    world.spawn((
        Transform::from_xyz(0.0, 100.0, 200.0),
        Mesh::new("pillar").with_material(panel(Color::rgb(0.88, 0.52, 0.34))),
        Name::new("pillar"),
    ));
    let cube = world.spawn((
        Transform::from_xyz(-150.0, 70.0, 400.0),
        Mesh::new("cube").with_material(panel(Color::rgb(0.45, 0.78, 0.82))),
        Name::new("cube"),
    ));
    world.spawn((
        Transform::from_xyz(210.0, 0.0, 330.0).with_rotation(Quat::from_rotation_y(core::f32::consts::FRAC_PI_4)),
        Mesh::new("pyramid").with_material(panel(Color::rgb(0.92, 0.80, 0.40))),
    ));

    // Sprites: the four figures at Z = 0 / 100 / 300 / 600, plus trees as depth scale.
    let figures = FIGURES
        .iter()
        .map(|&(label, pos, t)| {
            let sprite =
                Sprite::new("figure").with_size(FIGURE_SIZE).with_tint(Color::rgb(t[0], t[1], t[2])).cutout().unlit();
            (label, world.spawn((Transform::from_position(pos), sprite, Name::new(format!("figure {label}")))))
        })
        .collect();
    for (x, z) in [(-320.0, 160.0), (300.0, 60.0), (-150.0, 400.0), (360.0, 520.0), (-340.0, 720.0), (250.0, 860.0)] {
        let tree = Sprite::new("tree").with_size(Vec2::new(120.0, 160.0)).cutout().upright().unlit();
        world.spawn((Transform::from_xyz(x, 0.0, z), tree));
    }
    // Alpha-blended: drawn after all opaque geometry, back to front, and still depth-tested.
    let ghost =
        Sprite::new("figure").with_size(FIGURE_SIZE).with_tint(Color { r: 0.75, g: 0.8, b: 1.0, a: 0.5 }).unlit();
    world.spawn((Transform::from_xyz(110.0, 0.0, 200.0), ghost));

    let rig = CameraRig::HOME;
    let camera =
        world.spawn((Camera::new(projection(perspective)).with_clear_color(SKY), rig.transform(), Name::new("camera")));
    Scene {
        camera,
        cube,
        figures,
        rig,
        perspective,
        figure_orientation: SpriteOrientation::Billboard,
        title_timer: 0.0,
    }
}

fn projection(perspective: bool) -> CameraProjection {
    let p = if perspective {
        CameraProjection::perspective(FOV_DEGREES)
    } else {
        CameraProjection::orthographic(ORTHO_HEIGHT)
    };
    p.with_clip_range(1.0, 4000.0)
}

fn controls_system(world: &mut World) {
    let dt = world.resource::<Time>().delta_secs();
    let input = world.resource::<Input>();
    let (strafe, advance) =
        (input.axis(Key::A, Key::D), input.axis(Key::S, Key::W) + input.scroll() * 0.4 / dt.max(1e-3));
    let lift = input.axis(Key::Shift, Key::Space);
    let (yaw, pitch) = (input.axis(Key::Left, Key::Right), input.axis(Key::Up, Key::Down));
    let (toggle_projection, cycle, reset, escape) = (
        input.key_pressed(Key::P),
        input.key_pressed(Key::O),
        input.key_pressed(Key::R),
        input.key_pressed(Key::Escape),
    );
    drop(input);

    let mut scene = world.resource_mut::<Scene>();
    scene.rig.yaw += yaw * TURN_SPEED * dt;
    scene.rig.pitch = (scene.rig.pitch + pitch * TURN_SPEED * dt).clamp(-1.4, 1.4);
    let (heading, right) = scene.rig.heading();
    scene.rig.position += (heading * advance + right * strafe + Vec3::Y * lift) * MOVE_SPEED * dt;
    scene.rig.position.y = scene.rig.position.y.max(5.0);
    if reset {
        scene.rig = CameraRig::HOME;
    }
    if toggle_projection {
        scene.perspective = !scene.perspective;
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
        world.component_mut::<Sprite>(e).unwrap().orientation = orientation;
    }
    if escape {
        world.resource_mut::<WindowControl>().exit = true;
    }
}

fn camera_system(world: &mut World) {
    let scene = world.resource::<Scene>();
    let (camera, rig, perspective) = (scene.camera, scene.rig, scene.perspective);
    drop(scene);
    *world.component_mut::<Transform>(camera).unwrap() = rig.transform();
    world.component_mut::<Camera>(camera).unwrap().projection = projection(perspective);
}

fn props_system(world: &mut World) {
    let t = world.resource::<Time>().elapsed_secs();
    let cube = world.resource::<Scene>().cube;
    world.component_mut::<Transform>(cube).unwrap().rotation =
        Quat::from_rotation_y(t * 0.7) * Quat::from_rotation_x(0.4);
}

/// Window title readout: camera pose plus, for each figure, its camera-space depth and the height
/// it covers on screen. The numbers are computed with the same `CameraView` the renderer uses.
fn hud_system(world: &mut World) {
    let dt = world.resource::<Time>().delta_secs();
    let mut scene = world.resource_mut::<Scene>();
    scene.title_timer -= dt;
    if scene.title_timer > 0.0 {
        return;
    }
    scene.title_timer = 0.25;
    let (rig, perspective, camera, orientation) =
        (scene.rig, scene.perspective, scene.camera, scene.figure_orientation);
    let figures = scene.figures.clone();
    drop(scene);

    let viewport = world.resource::<Viewport>().0;
    let cam = *world.component::<Camera>(camera).unwrap();
    let view = CameraView::new(&cam, &GlobalTransform { matrix: rig.transform().to_matrix() }, viewport);
    let readout: Vec<String> = figures
        .iter()
        .filter_map(|&(label, e)| {
            let base = world.component::<Transform>(e).ok()?.position;
            let (foot, head) = (view.project(base)?, view.project(base + Vec3::Y * FIGURE_SIZE.y)?);
            Some(format!("{label}: depth {:.0}, {:.0}px", foot.view_depth, foot.pixels.y - head.pixels.y))
        })
        .collect();
    let stats = *world.resource::<RenderStats>();
    let title = format!(
        "spatial_scene | {} | cam ({:.0}, {:.0}, {:.0}) | {:?} | {} | {} draws | gpu cache {} tex {} mesh",
        if perspective { "perspective" } else { "orthographic" },
        rig.position.x,
        rig.position.y,
        rig.position.z,
        orientation,
        readout.join("  "),
        stats.frame.draw_calls,
        stats.cache.textures,
        stats.cache.meshes,
    );
    world.resource_mut::<WindowControl>().title = Some(title);
}
