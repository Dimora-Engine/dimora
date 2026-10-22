//! Shared helpers: build the scene in a headless game, render it, and read pixels back.
#![allow(dead_code)]

use engine::prelude::*;
use engine::renderer_wgpu::RenderError;
use spatial_scene::cameras::NAMED;
use spatial_scene::{art, scene};
use std::time::Duration;

pub const W: u32 = 320;
pub const H: u32 = 180;
const REQUIRE_GPU_VAR: &str = "DIMORA_REQUIRE_GPU";

pub fn game(config: usize) -> Game {
    let mut game = Game::new(GameConfig::default().with_size(W, H));
    art::register(&mut game.assets());
    let s = scene::build_world(&mut game.world, config);
    game.world.insert_resource(s);
    game.add_system("camera", scene::camera_system);
    game.add_system("motion", scene::motion_system);
    game.step(Duration::from_millis(16));
    game
}

pub fn config_names() -> impl Iterator<Item = (usize, &'static str)> {
    NAMED.iter().enumerate().map(|(i, (n, _))| (i, *n))
}

/// Renders, or returns `None` (a skip) when there is no GPU adapter and `DIMORA_REQUIRE_GPU` is not `1`.
/// With `DIMORA_REQUIRE_GPU=1` a missing adapter fails the test, like the engine's own GPU tests.
pub fn render(game: &mut Game) -> Option<Vec<u8>> {
    match game.render_to_rgba(W, H) {
        Ok(img) => Some(img),
        Err(e @ RenderError::NoAdapter(_)) => {
            if std::env::var(REQUIRE_GPU_VAR).as_deref() == Ok("1") {
                panic!("{REQUIRE_GPU_VAR}=1 but no GPU adapter was found: {e}");
            }
            eprintln!("skipping GPU test: {e}");
            None
        }
        Err(e) => panic!("render failed: {e}"),
    }
}

pub fn px(img: &[u8], x: i32, y: i32) -> [u8; 4] {
    let (x, y) = (x.clamp(0, W as i32 - 1) as u32, y.clamp(0, H as i32 - 1) as u32);
    let i = ((y * W + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

pub fn entity(game: &Game, name: &str) -> Entity {
    game.world
        .query::<&Name>()
        .iter()
        .find(|(_, n)| n.0 == name)
        .map(|(e, _)| e)
        .unwrap_or_else(|| panic!("no entity named `{name}`"))
}

pub fn position(game: &Game, name: &str) -> Vec3 {
    game.world.component::<GlobalTransform>(entity(game, name)).unwrap().translation()
}

pub fn view(game: &Game) -> CameraView {
    let s = game.world.resource::<scene::Scene>();
    let camera = s.camera;
    drop(s);
    let cam = *game.world.component::<Camera>(camera).unwrap();
    let global = *game.world.component::<GlobalTransform>(camera).unwrap();
    CameraView::new(&cam, &global, Vec2::new(W as f32, H as f32))
}

/// Pixel of a world point, rounded.
pub fn pixel_of(game: &Game, world: Vec3) -> Option<(i32, i32)> {
    let p = view(game).project(world)?;
    Some((p.pixels.x.round() as i32, p.pixels.y.round() as i32))
}

/// Moves the camera along its own view axis plane so that `target` projects to the centre of the image at
/// the depth it had before. Only the camera's `Transform` changes: the configuration is otherwise untouched.
pub fn aim_at(game: &mut Game, target: Vec3) {
    let camera = game.world.resource::<scene::Scene>().camera;
    let t = *game.world.component::<Transform>(camera).unwrap();
    let depth = (target - t.position).dot(t.forward());
    game.world.resource_mut::<scene::Scene>().rig.position = target - t.forward() * depth;
    game.step(Duration::from_millis(16));
}

/// Counts pixels in the screen-space bounding box of the square `centre +/- half` lying in the XZ plane at
/// `centre.y` for which `class` is true. Also returns the number of pixels examined.
pub fn count_in_square(
    game: &Game,
    img: &[u8],
    centre: Vec3,
    half: f32,
    class: impl Fn([u8; 4]) -> bool,
) -> (u32, u32) {
    let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .map(|(sx, sz)| pixel_of(game, centre + Vec3::new(sx * half, 0.0, sz * half)).expect("in front of the camera"));
    let (x0, x1) = (corners.iter().map(|c| c.0).min().unwrap(), corners.iter().map(|c| c.0).max().unwrap());
    let (y0, y1) = (corners.iter().map(|c| c.1).min().unwrap(), corners.iter().map(|c| c.1).max().unwrap());
    let (mut hits, mut total) = (0, 0);
    for y in y0.max(0)..=y1.min(H as i32 - 1) {
        for x in x0.max(0)..=x1.min(W as i32 - 1) {
            total += 1;
            hits += class(px(img, x, y)) as u32;
        }
    }
    (hits, total)
}

pub fn near(p: [u8; 4], rgb: [u8; 3]) -> bool {
    (0..3).all(|i| (p[i] as i32 - rgb[i] as i32).abs() <= 6)
}

pub const RED: [u8; 3] = [255, 89, 89];
pub const GREEN: [u8; 3] = [89, 231, 89];
pub const YELLOW: [u8; 3] = [255, 243, 89];
pub const BLUE: [u8; 3] = [89, 124, 255];
pub const ORANGE: [u8; 3] = [255, 188, 89];
pub const CYAN_ON_SLAB: [u8; 3] = [88, 210, 210];

/// Pixels under an `n` by `n` grid of world points strictly inside the XZ rectangle `[x0, x1] x [z0, z1]` at
/// height `y` (points that project off the image are skipped).
pub fn sample_grid(
    game: &Game,
    img: &[u8],
    y: f32,
    (x0, x1): (f32, f32),
    (z0, z1): (f32, f32),
    n: u32,
) -> Vec<[u8; 4]> {
    let mut out = Vec::new();
    for i in 0..n {
        for j in 0..n {
            let (fx, fz) = ((i as f32 + 0.5) / n as f32, (j as f32 + 0.5) / n as f32);
            let p = Vec3::new(x0 + (x1 - x0) * fx, y, z0 + (z1 - z0) * fz);
            if let Some((px_, py_)) = pixel_of(game, p) {
                if (0..W as i32).contains(&px_) && (0..H as i32).contains(&py_) {
                    out.push(px(img, px_, py_));
                }
            }
        }
    }
    out
}

pub fn fraction(pixels: &[[u8; 4]], colour: [u8; 3]) -> f32 {
    pixels.iter().filter(|p| near(**p, colour)).count() as f32 / pixels.len().max(1) as f32
}

/// Hides every renderable except the named ones. Cameras stay. Only visibility changes.
pub fn isolate(game: &mut Game, keep: &[&str]) {
    let keep: Vec<Entity> = keep.iter().map(|n| entity(game, n)).collect();
    let all: Vec<Entity> = game.world.query::<&Transform>().iter().map(|(e, _)| e).collect();
    for e in all {
        if !keep.contains(&e) && game.world.component::<Camera>(e).is_err() {
            game.world.insert(e, (Visibility::Hidden,)).unwrap();
        }
    }
}

pub fn count(img: &[u8], class: impl Fn([u8; 4]) -> bool) -> usize {
    (0..H as i32).flat_map(|y| (0..W as i32).map(move |x| (x, y))).filter(|&(x, y)| class(px(img, x, y))).count()
}

pub fn is_green(p: [u8; 4]) -> bool {
    p[1] as i32 > p[0] as i32 + 30 && p[1] as i32 > p[2] as i32 + 30
}

/// Colour of a cutout decal's blended partner seen through it: cyan at 0.6 alpha over the orange decal.
pub const CYAN_ON_ORANGE: [u8; 3] = [170, 223, 200];

pub fn set_orientation(game: &mut Game, name: &str, orientation: SpriteOrientation) {
    let e = entity(game, name);
    game.world.component_mut::<Sprite>(e).unwrap().orientation = orientation;
}

/// Configurations in which the decal bench is visible (`side` leaves every flat decal edge-on).
pub fn looking_down() -> impl Iterator<Item = (usize, &'static str)> {
    config_names().filter(|(_, n)| *n != "side")
}

pub const BENCH_CENTRE: Vec3 = Vec3::new(-545.0, 0.5, 430.0);
