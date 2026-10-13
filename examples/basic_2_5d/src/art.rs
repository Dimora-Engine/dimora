//! Procedural pixel art so the example needs no external files. Everything is deterministic.

use engine::prelude::*;

fn hash(x: u32, y: u32, seed: u32) -> u32 {
    let mut h = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77) ^ seed.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^ (h >> 15)
}

fn noise(x: u32, y: u32, seed: u32) -> f32 {
    (hash(x, y, seed) & 0xFFFF) as f32 / 65535.0
}

fn shade(c: [u8; 3], k: f32) -> [u8; 4] {
    let f = |v: u8| (v as f32 * k).clamp(0.0, 255.0) as u8;
    [f(c[0]), f(c[1]), f(c[2]), 255]
}

/// Registers a texture plus a same-named sprite covering all of it (bottom-center pivot, 16 px/unit).
fn add_sprite(assets: &mut AssetManager, name: &str, tex: Texture) {
    let (w, h) = (tex.width, tex.height);
    let handle = assets.insert(name, tex).expect("example asset names are distinct");
    let sprite =
        assets.insert(name, SpriteAsset::whole_texture(handle, w, h)).expect("example asset names are distinct");
    assets.add_dependency(sprite, handle);
}

pub fn register(assets: &mut AssetManager) {
    // 2x2 atlas of 16px tiles: grass, grass with blades, dirt, flowers.
    assets
        .insert(
            "ground",
            Texture::from_fn(32, 32, |x, y| {
                let (tile, lx, ly) = ((x / 16) + (y / 16) * 2, x % 16, y % 16);
                let n = noise(lx, ly, tile);
                match tile {
                    0 => shade([78, 142, 62], 0.9 + n * 0.2),
                    1 => {
                        let blade = noise(lx / 2, ly, 7) > 0.8;
                        shade([70, 135, 58], if blade { 1.25 } else { 0.9 + n * 0.15 })
                    }
                    2 => shade([146, 112, 74], 0.88 + n * 0.24),
                    _ => {
                        let flower = noise(lx, ly, 11);
                        if flower > 0.965 {
                            [250, 240, 120, 255]
                        } else if flower > 0.93 {
                            [240, 120, 170, 255]
                        } else {
                            shade([78, 142, 62], 0.9 + n * 0.2)
                        }
                    }
                }
            }),
        )
        .expect("example asset names are distinct");

    assets.insert("white", Texture::solid(4, 4, [255; 4])).expect("example asset names are distinct");
    add_sprite(assets, "white", Texture::solid(4, 4, [255; 4]));

    add_sprite(
        assets,
        "tree",
        Texture::from_fn(32, 48, |x, y| {
            let (fx, fy) = (x as f32, y as f32);
            let in_circle = |cx: f32, cy: f32, r: f32| (fx - cx).powi(2) + (fy - cy).powi(2) <= r * r;
            let canopy = in_circle(16.0, 15.0, 14.0) || in_circle(9.0, 22.0, 9.0) || in_circle(23.0, 22.0, 9.0);
            let trunk = (13..19).contains(&x) && (28..48).contains(&y);
            if canopy {
                let light = 1.15 - (fy / 40.0) + (noise(x, y, 3) - 0.5) * 0.25 + if fx < 14.0 { 0.08 } else { -0.05 };
                shade([46, 120, 52], light)
            } else if trunk {
                shade([104, 72, 44], if x < 15 { 1.05 } else { 0.8 })
            } else {
                [0; 4]
            }
        }),
    );

    add_sprite(
        assets,
        "rock",
        Texture::from_fn(16, 12, |x, y| {
            let (dx, dy) = ((x as f32 - 7.5) / 7.5, (y as f32 - 7.0) / 5.0);
            if dx * dx + dy * dy <= 1.0 {
                shade([128, 128, 136], 1.15 - dy * 0.3 + (noise(x, y, 5) - 0.5) * 0.2)
            } else {
                [0; 4]
            }
        }),
    );

    add_sprite(
        assets,
        "tuft",
        Texture::from_fn(8, 8, |x, y| {
            let blades = [(1, 5), (3, 7), (5, 6), (6, 4)];
            let hit = blades.iter().any(|&(bx, h)| x == bx && y >= 8 - h);
            if hit {
                shade([60, 150, 56], 0.8 + y as f32 / 12.0)
            } else {
                [0; 4]
            }
        }),
    );

    // Soft round contact blob; alpha-blended, so it exercises the sorted path.
    add_sprite(
        assets,
        "blob",
        Texture::from_fn(16, 16, |x, y| {
            let (dx, dy) = ((x as f32 - 7.5) / 8.0, (y as f32 - 7.5) / 8.0);
            let a = (1.0 - (dx * dx + dy * dy).sqrt()).max(0.0).powf(0.6);
            [0, 0, 0, (a * 255.0 * 0.8) as u8]
        }),
    );

    assets
        .insert(
            "brick",
            Texture::from_fn(32, 32, |x, y| {
                let (row, offset) = (y / 8, if (y / 8) % 2 == 0 { 0 } else { 8 });
                let bx = (x + offset) % 16;
                if y % 8 == 0 || bx == 0 {
                    shade([190, 186, 176], 1.0)
                } else {
                    shade([168, 76, 56], 0.85 + noise(x, y, row) * 0.3)
                }
            }),
        )
        .expect("example asset names are distinct");

    assets
        .insert(
            "crate",
            Texture::from_fn(16, 16, |x, y| {
                let edge = x == 0 || y == 0 || x == 15 || y == 15;
                let brace = x == y || x + y == 15;
                let plank = y % 5 == 0;
                let wood = shade([176, 130, 76], 0.9 + noise(x, y, 9) * 0.2);
                if edge || brace {
                    shade([110, 76, 42], 1.0)
                } else if plank {
                    shade([140, 100, 58], 1.0)
                } else {
                    wood
                }
            }),
        )
        .expect("example asset names are distinct");
}
