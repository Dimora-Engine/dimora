use engine_assets::AssetManager;
use engine_math::{Mat4, Rect, Vec2, Vec3};
use engine_world::{Entity, GlobalTransform, Visibility, World};

use crate::camera::{Camera, CameraView};
use crate::components::{AmbientLight, Light, LightKind, Mesh, Sprite, SpriteBlend, TileMap};
use crate::frustum::Frustum;
use crate::scene::*;

/// Snapshots the world into a [`RenderScene`] for the active camera.
///
/// Per frame, in order: pick the camera, gather lights, then for every visible renderable resolve
/// assets, build its world matrix, frustum-cull, and finally sort and batch. The world is only
/// read; nothing here touches a GPU. Returns `None` if there is no active camera.
///
/// `transforms` must be up to date: call `engine_world::propagate_transforms` first.
pub fn extract_scene(world: &World, assets: &AssetManager, viewport: Vec2) -> Option<RenderScene> {
    let (camera, view) = active_camera(world, viewport)?;
    let frustum = Frustum::from_view_proj(&view.view_proj);
    let mut stats = ExtractStats::default();

    let ambient = world.try_resource::<AmbientLight>().map_or(AmbientLight::default(), |a| *a);
    let ambient = [ambient.color.r, ambient.color.g, ambient.color.b].map(|c| c * ambient.intensity);
    let lights = extract_lights(world, &view);
    stats.lights = lights.len() as u32;

    let mut cutout: Vec<SpriteInstance> = Vec::new();
    let mut blended: Vec<(SortKey, SpriteInstance)> = Vec::new();

    for (_, (sprite, global, vis)) in world.query::<(&Sprite, &GlobalTransform, Option<&Visibility>)>().iter() {
        if matches!(vis, Some(Visibility::Hidden)) {
            continue;
        }
        let Some(asset) = assets.get(sprite.asset) else {
            stats.missing_assets += 1;
            continue;
        };
        let size = sprite.size.unwrap_or_else(|| asset.world_size());
        let anchor = sprite.anchor.unwrap_or(asset.pivot);
        let model = quad_matrix(&global.matrix, size, anchor);
        if !quad_visible(&frustum, &model) {
            stats.sprites_culled += 1;
            continue;
        }
        stats.sprites_visible += 1;
        let instance = SpriteInstance {
            model,
            uv: flipped_uv(asset.uv, sprite.flip_x, sprite.flip_y),
            tint: sprite.tint.to_array(),
            lit: sprite.lit,
            texture: asset.texture,
        };
        match sprite.blend {
            SpriteBlend::Cutout => cutout.push(instance),
            SpriteBlend::Alpha => {
                let depth = view.view.transform_point3(quad_center(&model)).z;
                blended.push((SortKey { layer: sprite.layer, depth, order: sprite.order }, instance));
            }
        }
    }

    for (_, (map, global, vis)) in world.query::<(&TileMap, &GlobalTransform, Option<&Visibility>)>().iter() {
        if matches!(vis, Some(Visibility::Hidden)) {
            continue;
        }
        extract_tilemap(map, &global.matrix, assets, &frustum, &mut cutout, &mut stats);
    }

    let mut meshes = Vec::new();
    for (_, (mesh, global, vis)) in world.query::<(&Mesh, &GlobalTransform, Option<&Visibility>)>().iter() {
        if matches!(vis, Some(Visibility::Hidden)) {
            continue;
        }
        let Some(data) = assets.get(mesh.asset) else {
            stats.missing_assets += 1;
            continue;
        };
        let (center, radius) = data.bounding_sphere();
        let m = &global.matrix;
        let max_scale = m.cols[0].xyz().length().max(m.cols[1].xyz().length()).max(m.cols[2].xyz().length());
        if !frustum.intersects_sphere(m.transform_point3(center), radius * max_scale) {
            stats.meshes_culled += 1;
            continue;
        }
        stats.meshes_visible += 1;
        meshes.push(MeshInstance {
            model: *m,
            mesh: mesh.asset,
            color: mesh.material.base_color.to_array(),
            texture: mesh.material.base_texture,
            lit: mesh.material.lit,
        });
    }
    // Group identical (mesh, texture) pairs so the backend can merge them into instanced draws.
    meshes.sort_by_key(|m| (m.mesh.id(), m.texture.map(|t| t.id())));

    // Far to near: larger view depth first. Stable, so equal keys keep ECS iteration order, which
    // is deterministic for a given sequence of spawns.
    blended.sort_by(|(a, _), (b, _)| {
        a.layer.cmp(&b.layer).then(b.depth.total_cmp(&a.depth)).then(a.order.cmp(&b.order))
    });
    // Opaque sprites do not need ordering for correctness; group by texture to minimise batches.
    cutout.sort_by_key(|i| i.texture.id());

    Some(RenderScene {
        view,
        clear_color: camera.clear_color,
        ambient,
        lights,
        meshes,
        cutout_sprites: SpriteLayer::from_ordered(cutout),
        blended_sprites: SpriteLayer::from_ordered(blended.into_iter().map(|(_, i)| i).collect()),
        stats,
    })
}

struct SortKey {
    layer: i16,
    depth: f32,
    order: i16,
}

fn active_camera(world: &World, viewport: Vec2) -> Option<(Camera, CameraView)> {
    let mut best: Option<(Entity, Camera, GlobalTransform)> = None;
    for (e, (camera, global)) in world.query::<(&Camera, &GlobalTransform)>().iter() {
        let better = match &best {
            None => true,
            Some((be, bc, _)) => (camera.order, e.id()) < (bc.order, be.id()),
        };
        if camera.active && better {
            best = Some((e, *camera, *global));
        }
    }
    let (_, camera, global) = best?;
    Some((camera, CameraView::new(&camera, &global, viewport)))
}

fn extract_lights(world: &World, view: &CameraView) -> Vec<LightData> {
    let mut lights: Vec<LightData> = world
        .query::<(&Light, &GlobalTransform)>()
        .iter()
        .map(|(_, (light, global))| {
            let c = light.color;
            LightData {
                kind: light.kind,
                position: global.translation(),
                direction: global.forward().normalized(),
                color: [c.r, c.g, c.b].map(|v| v * light.intensity),
            }
        })
        .collect();
    if lights.len() > MAX_LIGHTS {
        // Directional lights always survive; positional ones are ranked by distance to the camera.
        let rank = |l: &LightData| match l.kind {
            LightKind::Directional => -1.0,
            _ => l.position.distance_squared(view.position),
        };
        lights.sort_by(|a, b| rank(a).total_cmp(&rank(b)));
        lights.truncate(MAX_LIGHTS);
    }
    lights
}

/// World matrix for a quad of `size` whose `anchor` sits at the entity origin.
fn quad_matrix(global: &Mat4, size: Vec2, anchor: Vec2) -> Mat4 {
    *global
        * Mat4::from_translation(Vec3::new(-anchor.x * size.x, -anchor.y * size.y, 0.0))
        * Mat4::from_scale(Vec3::new(size.x, size.y, 1.0))
}

fn quad_center(model: &Mat4) -> Vec3 {
    model.transform_point3(Vec3::new(0.5, 0.5, 0.0))
}

fn quad_visible(frustum: &Frustum, model: &Mat4) -> bool {
    let (a, b) = (model.cols[0].xyz(), model.cols[1].xyz());
    let radius = 0.5 * (a.length_squared() + b.length_squared()).sqrt();
    frustum.intersects_sphere(quad_center(model), radius)
}

fn flipped_uv(uv: Rect, flip_x: bool, flip_y: bool) -> [f32; 4] {
    let (mut u0, mut v0, mut u1, mut v1) = (uv.min.x, uv.min.y, uv.max.x, uv.max.y);
    if flip_x {
        core::mem::swap(&mut u0, &mut u1);
    }
    if flip_y {
        core::mem::swap(&mut v0, &mut v1);
    }
    [u0, v0, u1, v1]
}

fn extract_tilemap(
    map: &TileMap,
    global: &Mat4,
    assets: &AssetManager,
    frustum: &Frustum,
    out: &mut Vec<SpriteInstance>,
    stats: &mut ExtractStats,
) {
    let Some(texture) = assets.get(map.texture) else {
        stats.missing_assets += 1;
        return;
    };
    // Pull UVs in by a sliver of a texel so perspective minification never samples a neighbour.
    let inset = Vec2::new(0.02 / texture.width as f32, 0.02 / texture.height as f32);
    let cell = Vec2::new(1.0 / map.atlas_columns as f32, 1.0 / map.atlas_rows as f32);

    for y in 0..map.height {
        for x in 0..map.width {
            let tile = map.get(x, y);
            if tile == TileMap::EMPTY {
                continue;
            }
            let (col, row) = (tile as u32 % map.atlas_columns, tile as u32 / map.atlas_columns);
            let min = Vec2::new(col as f32 * cell.x, row as f32 * cell.y) + inset;
            let max = Vec2::new((col + 1) as f32 * cell.x, (row + 1) as f32 * cell.y) - inset;
            let origin = Vec3::new(x as f32 * map.tile_size.x, y as f32 * map.tile_size.y, 0.0);
            let model = *global * Mat4::from_translation(origin) * Mat4::from_scale(Vec3::new(map.tile_size.x, map.tile_size.y, 1.0));
            if !quad_visible(frustum, &model) {
                stats.sprites_culled += 1;
                continue;
            }
            stats.sprites_visible += 1;
            out.push(SpriteInstance {
                model,
                uv: [min.x, min.y, max.x, max.y],
                tint: [1.0; 4],
                lit: map.lit,
                texture: map.texture,
            });
        }
    }
}
