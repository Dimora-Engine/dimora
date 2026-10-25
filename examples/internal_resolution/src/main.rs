//! One camera over a small spatial scene, presented from a 320×180 internal image.
//!
//! The default window is 1366×768, so the image is a 1280×720 block with bars. `I` switches that
//! camera back to direct rendering into the window. Esc quits. This is the existing camera and
//! the existing shaded or depth presentation. It is not a second renderer.
//!
//! ```text
//! cargo run -p internal_resolution
//! cargo run --release -p internal_resolution -- --screenshot bars.png --size 1366x768
//! cargo run --release -p internal_resolution -- --screenshot fit.png --size 1280x720
//! cargo run --release -p internal_resolution -- --screenshot direct.png --size 1366x768 --direct
//! cargo run --release -p internal_resolution -- --screenshot depth.png --size 1366x768 --depth
//! ```

use std::time::Duration;

use engine::prelude::*;

const INTERNAL: (u32, u32) = (320, 180);

struct Preview {
    camera: Entity,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().any(|arg| arg == name);
    let value = |name: &str| args.iter().position(|arg| arg == name).and_then(|index| args.get(index + 1)).cloned();
    let (width, height) = value("--size")
        .as_deref()
        .and_then(|size| size.split_once('x'))
        .and_then(|(width, height)| Some((width.parse().ok()?, height.parse().ok()?)))
        .unwrap_or((1366, 768));

    let mut game = Game::new(GameConfig::default().with_title("Dimora internal resolution").with_size(width, height));
    register(&mut game.assets());
    let camera = build(&mut game.world, !flag("--direct"), flag("--depth"));
    game.world.insert_resource(Preview { camera });
    game.add_system("internal-toggle", toggle);

    match value("--screenshot") {
        Some(path) => {
            game.step(Duration::from_millis(16));
            let pixels = game.render_to_rgba(width, height)?;
            image::save_buffer(&path, &pixels, width, height, image::ColorType::Rgba8)?;
            println!("saved {path}: {:?}", game.world.resource::<RenderStats>().frame);
            Ok(())
        }
        None => game.run(),
    }
}

fn register(assets: &mut AssetManager) {
    let white = assets.insert("white", Texture::solid(1, 1, [255, 255, 255, 255])).unwrap();
    assets.insert("quad", SpriteAsset::whole_texture(white, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    assets.insert("cube", MeshData::cuboid(Vec3::splat(1.2)).unwrap()).unwrap();
}

fn build(world: &mut World, internal: bool, depth: bool) -> Entity {
    world.insert_resource(AmbientLight { color: Color::rgb(0.85, 0.9, 1.0), intensity: 0.45 });
    let presentation = if depth { CameraPresentation::Depth } else { CameraPresentation::Shaded };
    let mut camera = Camera::perspective(48.0)
        .with_clear_color(Color::rgb(0.16, 0.28, 0.42))
        .with_letterbox_color(Color::rgb(0.05, 0.07, 0.1))
        .with_presentation(presentation);
    if internal {
        camera = camera.with_internal_resolution(INTERNAL.0, INTERNAL.1);
    }
    world.spawn((
        Transform::from_xyz(-1.6, 0.0, 1.2),
        Sprite::new("quad")
            .with_size(Vec2::new(1.4, 2.2))
            .with_anchor(Vec2::new(0.5, 0.0))
            .with_tint(Color::rgb(0.85, 0.28, 0.22))
            .cutout(),
    ));
    world.spawn((
        Transform::from_xyz(1.5, 0.0, 3.4),
        Sprite::new("quad")
            .with_size(Vec2::new(1.6, 2.4))
            .with_anchor(Vec2::new(0.5, 0.0))
            .with_tint(Color::rgb(0.25, 0.72, 0.38))
            .cutout(),
    ));
    world.spawn((
        Transform::from_xyz(0.2, 0.6, 2.2),
        Mesh::new("cube").with_material(Material::color(Color::rgb(0.9, 0.55, 0.2))),
    ));
    world.spawn((
        Transform::from_xyz(-2.2, 1.8, 2.6),
        Light { kind: LightKind::Point { range: 12.0 }, color: Color::rgb(1.0, 0.9, 0.75), intensity: 6.0 },
    ));
    world.spawn((camera, Transform::looking_at(Vec3::new(0.0, 1.6, -6.0), Vec3::new(0.1, 0.8, 2.0), Vec3::Y)))
}

fn toggle(world: &mut World) {
    let input = world.resource::<Input>();
    let (flip, escape) = (input.key_pressed(Key::I), input.key_pressed(Key::Escape));
    drop(input);
    if flip {
        let camera = world.resource::<Preview>().camera;
        let mut view = world.component_mut::<Camera>(camera).unwrap();
        view.internal_resolution = match view.internal_resolution {
            Some(_) => None,
            None => Some(INTERNAL),
        };
        let mode = if view.internal_resolution.is_some() { "320x180" } else { "direct" };
        drop(view);
        world.resource_mut::<WindowControl>().title = Some(format!("Dimora internal resolution — {mode}"));
    }
    if escape {
        world.resource_mut::<WindowControl>().exit = true;
    }
}
