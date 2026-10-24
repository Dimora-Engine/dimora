//! One camera over a mixed sprite and mesh scene.
//!
//! `V` switches that camera between shaded color and the depth presentation. The world, materials,
//! and projection stay put. Esc quits.
//!
//! ```text
//! cargo run -p depth_view
//! cargo run --release -p depth_view -- --screenshot shaded.png --size 1280x720
//! cargo run --release -p depth_view -- --screenshot depth.png --size 1280x720 --depth
//! ```
//!
//! Nearer written fragments are brighter. Unwritten pixels are purple. A blended sprite does not
//! write a new depth value.

use std::time::Duration;

use engine::prelude::*;

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
        .unwrap_or((1280, 720));

    let mut game = Game::new(GameConfig::default().with_title("Dimora depth view").with_size(width, height));
    let glass = register(&mut game.assets());
    let camera = build(&mut game.world, glass, flag("--depth"));
    game.world.insert_resource(Preview { camera });
    game.add_system("depth-toggle", toggle);

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

fn register(assets: &mut AssetManager) -> AssetHandle<MaterialAsset> {
    let white = assets.insert("white", Texture::solid(1, 1, [255, 255, 255, 255])).unwrap();
    assets.insert("quad", SpriteAsset::whole_texture(white, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    assets.insert("cube", MeshData::cuboid(Vec3::splat(1.6)).unwrap()).unwrap();
    assets
        .insert("glass", MaterialAsset::unlit(Color::rgba(0.85, 0.95, 1.0, 0.45)).with_alpha_mode(AlphaMode::Blend))
        .unwrap()
}

fn build(world: &mut World, glass: AssetHandle<MaterialAsset>, depth: bool) -> Entity {
    world.insert_resource(AmbientLight { color: Color::rgb(0.85, 0.9, 1.0), intensity: 0.55 });
    world.insert_resource(Fog { color: Color::rgb(0.55, 0.62, 0.72), density: 0.015 });
    let mut ground = TileMap::new("white", 1, 1, Vec2::splat(1.5), 8, 10).filled_with(|_, _| 0);
    ground.lit = true;
    let mut ground_pose = Transform::from_xyz(-6.0, 0.0, -1.0);
    ground_pose.rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    world.spawn((ground_pose, ground));

    let card = |tint: Color, size: f32| {
        Sprite::new("quad").with_size(Vec2::splat(size)).with_anchor(Vec2::new(0.5, 0.0)).with_tint(tint).cutout()
    };
    world.spawn((Transform::from_xyz(-2.4, 0.0, 0.5), card(Color::rgb(0.85, 0.25, 0.22), 2.2)));
    world.spawn((Transform::from_xyz(0.15, 0.0, 5.0), card(Color::rgb(0.25, 0.72, 0.38), 2.2)));
    world.spawn((Transform::from_xyz(2.5, 0.0, 10.5), card(Color::rgb(0.25, 0.45, 0.9), 2.4)));
    world.spawn((
        Transform::from_xyz(0.0, 0.8, 7.2),
        Mesh::new("cube").with_material(Material::color(Color::rgb(0.9, 0.55, 0.2))),
    ));
    world.spawn((
        Transform::from_xyz(0.0, 1.5, 4.2),
        Sprite::new("quad").with_size(Vec2::new(1.6, 2.2)).with_anchor(Vec2::new(0.5, 0.0)).with_material(glass),
    ));
    world.spawn((
        Transform::from_xyz(-4.5, 2.2, 6.0),
        Light { kind: LightKind::Point { range: 18.0 }, color: Color::rgb(1.0, 0.9, 0.75), intensity: 8.0 },
    ));

    let presentation = if depth { CameraPresentation::Depth } else { CameraPresentation::Shaded };
    let camera = Camera {
        projection: CameraProjection::perspective(48.0).with_clip_range(1.0, 28.0),
        presentation,
        ..Camera::perspective(48.0).with_clear_color(Color::rgb(0.10, 0.12, 0.16))
    };
    world.spawn((camera, Transform::looking_at(Vec3::new(0.0, 3.4, -11.0), Vec3::new(0.2, 1.1, 4.0), Vec3::Y)))
}

fn toggle(world: &mut World) {
    let input = world.resource::<Input>();
    let (flip, escape) = (input.key_pressed(Key::V), input.key_pressed(Key::Escape));
    drop(input);
    if flip {
        let camera = world.resource::<Preview>().camera;
        let mut view = world.component_mut::<Camera>(camera).unwrap();
        view.presentation = match view.presentation {
            CameraPresentation::Shaded => CameraPresentation::Depth,
            CameraPresentation::Depth => CameraPresentation::Shaded,
        };
        let mode = match view.presentation {
            CameraPresentation::Depth => "depth",
            CameraPresentation::Shaded => "shaded",
        };
        drop(view);
        world.resource_mut::<WindowControl>().title = Some(format!("Dimora depth view — {mode}"));
    }
    if escape {
        world.resource_mut::<WindowControl>().exit = true;
    }
}
