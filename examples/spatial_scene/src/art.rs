//! Tiny procedural textures so the example needs no asset files.

use engine::prelude::*;

const CLEAR: [u8; 4] = [0, 0, 0, 0];

fn add_sprite(assets: &mut AssetManager, name: &str, tex: Texture) {
    let (w, h) = (tex.width, tex.height);
    let texture = assets.insert(name, tex).expect("example asset names are distinct");
    let sprite =
        assets.insert(name, SpriteAsset::whole_texture(texture, w, h)).expect("example asset names are distinct");
    assets.add_dependency(sprite, texture);
}

pub fn register(assets: &mut AssetManager) {
    // A plain white square for flat decals; tint gives it a colour.
    add_sprite(assets, "swatch", Texture::solid(8, 8, [255; 4]));

    // A little standing figure, 16x24 px. Drawn grey-white so a tint can recolor it.
    add_sprite(
        assets,
        "figure",
        Texture::from_fn(16, 24, |x, y| {
            let (fx, fy) = (x as f32 + 0.5 - 8.0, 24.0 - (y as f32 + 0.5)); // origin bottom-centre, +y up
            let head = (fx * fx + (fy - 19.0) * (fy - 19.0)).sqrt() < 4.2;
            let eye = (fy - 19.5).abs() < 0.8 && (fx.abs() - 1.8).abs() < 0.8;
            let body = fy > 8.0 && fy < 15.0 && fx.abs() < 4.8;
            let arm = fy > 8.5 && fy < 14.5 && (fx.abs() - 5.8).abs() < 1.2;
            let leg = fy < 8.0 && (fx.abs() - 2.2).abs() < 1.7;
            if eye {
                [30, 30, 40, 255]
            } else if head {
                [255, 255, 255, 255]
            } else if body || arm {
                [225, 225, 230, 255]
            } else if leg {
                [170, 170, 185, 255]
            } else {
                CLEAR
            }
        }),
    );

    // A round tree, 24x32 px.
    add_sprite(
        assets,
        "tree",
        Texture::from_fn(24, 32, |x, y| {
            let (fx, fy) = (x as f32 + 0.5 - 12.0, 32.0 - (y as f32 + 0.5));
            let crown = (fx * fx + (fy - 21.0) * (fy - 21.0) * 1.2).sqrt() < 10.5;
            let trunk = fy < 12.0 && fx.abs() < 2.2;
            if crown {
                let lit = if fx + fy > 14.0 { 1.12 } else { 0.9 };
                let g = |v: f32| (v * lit).min(255.0) as u8;
                [g(58.0), g(140.0), g(66.0), 255]
            } else if trunk {
                [110, 78, 52, 255]
            } else {
                CLEAR
            }
        }),
    );

    // Panel texture for every mesh: a light face with a darker bevel, so unlit meshes still show
    // their edges and their facets read as 3D.
    assets
        .insert(
            "panel",
            Texture::from_fn(16, 16, |x, y| {
                let edge = x.min(y).min(15 - x).min(15 - y);
                match edge {
                    0 => [150, 150, 150, 255],
                    1 => [205, 205, 205, 255],
                    _ => [255, 255, 255, 255],
                }
            }),
        )
        .expect("example asset names are distinct");
}
