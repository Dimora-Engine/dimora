use std::collections::{HashMap, HashSet};

use engine_assets::{AlphaMode, AssetHandle, AssetId, AssetManager, MaterialAsset, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Mat3, Mat4, Quat, Rect, Vec2, Vec3};
use engine_world::{Entity, GlobalTransform, Parent, Visibility, World};

use crate::camera::{Camera, CameraView};
use crate::components::{
    AmbientLight, Fog, Light, LightKind, Mesh, RenderOrder, Sprite, SpriteBlend, SpriteOrientation, TileMap,
};
use crate::frustum::Frustum;
use crate::interpolate::{render_poses, RenderPoses};
use crate::missing::{report_absent_texture, report_unrendered_mesh_blend, resolve, MissingAssetWarnings};
use crate::scene::*;

/// Snapshots the world into a [`RenderScene`] for the active camera. `target_size` is the render
/// target in pixels; the camera's own viewport rectangle is resolved against it.
///
/// Per frame, in order: pick the camera, gather lights, then for every visible renderable resolve
/// assets, build its world matrix, frustum-cull, and finally sort and batch. The world is only
/// read; nothing here touches a GPU. Returns `None` if there is no active camera.
///
/// `transforms` must be up to date: call `engine_world::propagate_transforms` first.
///
/// An entity with [`engine_world::Interpolate`] and a stored previous local is drawn from a
/// render matrix blended toward its current local. That blend is not written to `Transform` or
/// `GlobalTransform`. Sprites, meshes, tilemaps, lights, and the active camera share it. With no
/// stored previous local anywhere, every matrix is that entity's `GlobalTransform`.
///
/// A sprite, mesh, tilemap, or light is omitted when it or any ancestor has
/// [`Visibility::Hidden`]. Absence of `Visibility` means visible. The components are only read.
/// [`Camera::active`] still decides which camera renders; `Visibility` does not.
pub fn extract_scene(world: &World, assets: &AssetManager, target_size: Vec2) -> Option<RenderScene> {
    let hidden = AncestorVisibility::read(world);
    let poses = render_poses(world);
    let (camera, view) = active_camera(world, target_size, &poses)?;
    let frustum = Frustum::from_view_proj(&view.view_proj);
    let mut stats = ExtractStats::default();

    let ambient = world.try_resource::<AmbientLight>().map_or(AmbientLight::default(), |a| *a);
    let ambient = [ambient.color.r, ambient.color.g, ambient.color.b].map(|c| c * ambient.intensity);
    let fog = world.try_resource::<Fog>().map_or(Fog::default(), |f| *f);
    let lights = extract_lights(world, &view, &poses, &hidden);
    stats.lights = lights.len() as u32;

    let mut cutout: Vec<SpriteInstance> = Vec::new();
    let mut opaque: Vec<SpriteInstance> = Vec::new();
    let mut mask: Vec<SpriteInstance> = Vec::new();
    let mut blended: Vec<(SortKey, SpriteInstance)> = Vec::new();
    let mut watch = world.try_resource::<MissingAssetWarnings>().is_some_and(|warnings| !warnings.is_empty());

    for (entity, (sprite, global, order)) in world.query::<(&Sprite, &GlobalTransform, Option<&RenderOrder>)>().iter() {
        if hidden.hides(entity) {
            continue;
        }
        let Some(asset) = resolve(world, assets, sprite.asset, "sprite", &mut watch, &mut stats) else {
            continue;
        };
        let Some(appearance) = sprite_appearance(world, assets, sprite, asset, &mut watch, &mut stats) else {
            continue;
        };
        let size = sprite.size.unwrap_or_else(|| asset.world_size());
        let anchor = sprite.anchor.unwrap_or(asset.pivot);
        let placement = sprite_placement(&poses.matrix(entity, global), sprite.orientation, &view);
        let model = quad_matrix(&placement, size, anchor);
        if !quad_visible(&frustum, &model) {
            stats.sprites_culled += 1;
            continue;
        }
        stats.sprites_visible += 1;
        let instance = SpriteInstance {
            model,
            uv: flipped_uv(sub_region(asset.uv, sprite.region), sprite.flip_x, sprite.flip_y),
            tint: appearance.tint,
            lit: appearance.lit,
            texture: appearance.texture,
            shading: appearance.shading,
            material_id: appearance.material_id,
            material_revision: appearance.material_revision,
        };
        match appearance.shading {
            SpriteShading::LegacyCutout => cutout.push(instance),
            SpriteShading::Opaque { .. } => opaque.push(instance),
            SpriteShading::Mask { .. } => mask.push(instance),
            SpriteShading::LegacyBlend | SpriteShading::Blend { .. } => {
                let depth = view.view.transform_point3(quad_center(&model)).z;
                let RenderOrder { layer, order } = order.copied().unwrap_or_default();
                blended.push((SortKey { layer, depth, order }, instance));
            }
        }
    }

    for (entity, (map, global)) in world.query::<(&TileMap, &GlobalTransform)>().iter() {
        if hidden.hides(entity) {
            continue;
        }
        let Some(texture) = resolve(world, assets, map.texture, "tilemap", &mut watch, &mut stats) else {
            continue;
        };
        extract_tilemap(map, &poses.matrix(entity, global), texture, &frustum, &mut cutout, &mut stats);
    }

    let mut meshes = Vec::new();
    for (entity, (mesh, global)) in world.query::<(&Mesh, &GlobalTransform)>().iter() {
        if hidden.hides(entity) {
            continue;
        }
        let Some(data) = resolve(world, assets, mesh.asset, "mesh", &mut watch, &mut stats) else {
            continue;
        };
        let Some(surface) = mesh_surface(world, assets, mesh, &mut watch, &mut stats) else {
            continue;
        };
        let (center, radius) = data.bounding_sphere();
        let m = poses.matrix(entity, global);
        let max_scale = m.cols[0].xyz().length().max(m.cols[1].xyz().length()).max(m.cols[2].xyz().length());
        if !frustum.intersects_sphere(m.transform_point3(center), radius * max_scale) {
            stats.meshes_culled += 1;
            continue;
        }
        stats.meshes_visible += 1;
        meshes.push(MeshInstance {
            model: m,
            mesh: mesh.asset,
            color: surface.color,
            texture: surface.texture,
            lit: surface.lit,
            draw: surface.draw,
            cutoff: surface.cutoff,
            material_id: surface.material_id,
            material_revision: surface.material_revision,
        });
    }
    // Group identical (mesh, texture, draw) triples so the backend can merge them into instanced draws.
    meshes.sort_by_key(|m| (m.mesh.id(), m.texture.map(|t| t.id()), m.draw));

    // Far to near: larger view depth first. Stable, so equal keys keep ECS iteration order, which
    // is deterministic for a given sequence of spawns.
    blended
        .sort_by(|(a, _), (b, _)| a.layer.cmp(&b.layer).then(b.depth.total_cmp(&a.depth)).then(a.order.cmp(&b.order)));
    // Opaque and mask sprites do not need ordering for correctness. Group by texture, then by cull
    // mode, so a run shares one pipeline. Blended sprites stay in sort order.
    let group = |i: &SpriteInstance| (i.texture.id(), i.shading.pipeline());
    opaque.sort_by_key(group);
    mask.sort_by_key(group);
    cutout.sort_by_key(|i| i.texture.id());

    Some(RenderScene {
        view,
        clear_color: camera.clear_color,
        viewport_px: [view.viewport_origin.x, view.viewport_origin.y, view.viewport.x, view.viewport.y],
        ambient,
        fog: [fog.color.r, fog.color.g, fog.color.b, fog.density],
        lights,
        meshes,
        opaque_sprites: SpriteLayer::from_ordered(opaque),
        mask_sprites: SpriteLayer::from_ordered(mask),
        cutout_sprites: SpriteLayer::from_ordered(cutout),
        blended_sprites: SpriteLayer::from_ordered(blended.into_iter().map(|(_, i)| i).collect()),
        stats,
    })
}

struct SpriteAppearance {
    texture: AssetHandle<Texture>,
    tint: [f32; 4],
    lit: bool,
    shading: SpriteShading,
    material_id: Option<AssetId>,
    material_revision: u32,
}

fn sprite_appearance(
    world: &World,
    assets: &AssetManager,
    sprite: &Sprite,
    asset: &SpriteAsset,
    watch: &mut bool,
    stats: &mut ExtractStats,
) -> Option<SpriteAppearance> {
    let Some(handle) = sprite.material else {
        report_absent_texture(
            world,
            assets,
            asset.texture,
            "sprite texture",
            assets.name_of(sprite.asset).map(|name| ("sprite", name)),
            watch,
            stats,
        );
        let shading = match sprite.blend {
            SpriteBlend::Cutout => SpriteShading::LegacyCutout,
            SpriteBlend::Alpha => SpriteShading::LegacyBlend,
        };
        return Some(SpriteAppearance {
            texture: asset.texture,
            tint: sprite.tint.to_array(),
            lit: sprite.lit,
            shading,
            material_id: None,
            material_revision: 0,
        });
    };
    let owner = assets.name_of(sprite.asset).map(|name| ("sprite", name));
    let material = resolve(world, assets, handle, "material", watch, stats)?;
    let texture = match material.albedo {
        Some(texture) => {
            report_absent_texture(world, assets, texture, "material texture", owner, watch, stats);
            texture
        }
        None => {
            report_absent_texture(world, assets, asset.texture, "sprite texture", owner, watch, stats);
            asset.texture
        }
    };
    Some(SpriteAppearance {
        texture,
        tint: color_product(material.base_color, sprite.tint),
        lit: material.lit,
        shading: sprite_shading(material),
        material_id: Some(handle.id()),
        material_revision: assets.revision(handle),
    })
}

fn sprite_shading(material: &MaterialAsset) -> SpriteShading {
    match material.alpha_mode {
        AlphaMode::Opaque => SpriteShading::Opaque { double_sided: material.double_sided },
        AlphaMode::Mask { cutoff } => SpriteShading::Mask { cutoff: cutoff.get(), double_sided: material.double_sided },
        AlphaMode::Blend => SpriteShading::Blend { double_sided: material.double_sided },
    }
}

struct MeshSurface {
    color: [f32; 4],
    texture: Option<AssetHandle<Texture>>,
    lit: bool,
    draw: MeshDraw,
    cutoff: f32,
    material_id: Option<AssetId>,
    material_revision: u32,
}

fn mesh_surface(
    world: &World,
    assets: &AssetManager,
    mesh: &Mesh,
    watch: &mut bool,
    stats: &mut ExtractStats,
) -> Option<MeshSurface> {
    let Some(handle) = mesh.material_asset else {
        if let Some(texture) = mesh.material.base_texture {
            report_absent_texture(
                world,
                assets,
                texture,
                "mesh texture",
                assets.name_of(mesh.asset).map(|name| ("mesh", name)),
                watch,
                stats,
            );
        }
        return Some(MeshSurface {
            color: mesh.material.base_color.to_array(),
            texture: mesh.material.base_texture,
            lit: mesh.material.lit,
            draw: MeshDraw::Solid { double_sided: false },
            cutoff: 0.0,
            material_id: None,
            material_revision: 0,
        });
    };
    let material = resolve(world, assets, handle, "material", watch, stats)?;
    if matches!(material.alpha_mode, AlphaMode::Blend) {
        report_unrendered_mesh_blend(world, assets, handle, watch);
        return None;
    }
    if let Some(texture) = material.albedo {
        report_absent_texture(
            world,
            assets,
            texture,
            "material texture",
            assets.name_of(mesh.asset).map(|name| ("mesh", name)),
            watch,
            stats,
        );
    }
    let (draw, cutoff) = match material.alpha_mode {
        AlphaMode::Opaque => (MeshDraw::Solid { double_sided: material.double_sided }, 0.0),
        AlphaMode::Mask { cutoff } => (MeshDraw::Mask { double_sided: material.double_sided }, cutoff.get()),
        AlphaMode::Blend => unreachable!("blend was returned above"),
    };
    Some(MeshSurface {
        color: material.base_color.to_array(),
        texture: material.albedo,
        lit: material.lit,
        draw,
        cutoff,
        material_id: Some(handle.id()),
        material_revision: assets.revision(handle),
    })
}

fn color_product(base: Color, tint: Color) -> [f32; 4] {
    [base.r * tint.r, base.g * tint.g, base.b * tint.b, base.a * tint.a]
}

struct SortKey {
    layer: i32,
    depth: f32,
    order: i32,
}

/// Parent links and local [`Visibility::Hidden`] bits, copied out so extraction can walk
/// ancestors without borrowing the world again.
///
/// Sprites, meshes, tilemaps, and lights all call [`Self::hides`]. That is the only visibility
/// walk. A later optimization can replace this type. It does not add a cached component.
///
/// The walk stops when a parent id was already seen, or when an id has no [`Parent`]. A stale
/// parent id is not alive and is not in `parents`, so the walk ends there.
struct AncestorVisibility {
    locally_hidden: HashSet<Entity>,
    parents: HashMap<Entity, Entity>,
}

impl AncestorVisibility {
    fn read(world: &World) -> Self {
        let locally_hidden = world
            .query::<&Visibility>()
            .iter()
            .filter(|(_, visibility)| **visibility == Visibility::Hidden)
            .map(|(entity, _)| entity)
            .collect();
        let parents = world.query::<&Parent>().iter().map(|(entity, parent)| (entity, parent.0)).collect();
        Self { locally_hidden, parents }
    }

    fn hides(&self, mut entity: Entity) -> bool {
        let mut seen = Vec::new();
        loop {
            if seen.contains(&entity) {
                return false;
            }
            seen.push(entity);
            if self.locally_hidden.contains(&entity) {
                return true;
            }
            match self.parents.get(&entity) {
                Some(parent) => entity = *parent,
                None => return false,
            }
        }
    }
}

fn active_camera(world: &World, target_size: Vec2, poses: &RenderPoses) -> Option<(Camera, CameraView)> {
    let mut best: Option<(Entity, Camera, GlobalTransform)> = None;
    // `Visibility` is not an activation flag. `Camera.active` and `order` are.
    for (e, (camera, global)) in world.query::<(&Camera, &GlobalTransform)>().iter() {
        let better = match &best {
            None => true,
            Some((be, bc, _)) => (camera.order, e.id()) < (bc.order, be.id()),
        };
        if camera.active && better {
            let placed = GlobalTransform { matrix: poses.matrix(e, global) };
            best = Some((e, *camera, placed));
        }
    }
    let (_, camera, placed) = best?;
    Some((camera, CameraView::new(&camera, &placed, target_size)))
}

fn extract_lights(
    world: &World,
    view: &CameraView,
    poses: &RenderPoses,
    hidden: &AncestorVisibility,
) -> Vec<LightData> {
    let mut lights: Vec<LightData> = world
        .query::<(&Light, &GlobalTransform)>()
        .iter()
        .filter(|(entity, _)| !hidden.hides(*entity))
        .map(|(entity, (light, global))| {
            let c = light.color;
            let placed = GlobalTransform { matrix: poses.matrix(entity, global) };
            LightData {
                kind: light.kind,
                position: placed.translation(),
                direction: placed.forward().normalized(),
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

/// The sprite's placement frame: where its origin is and which way its local X/Y/Z axes point,
/// with the entity's scale folded into the axis lengths.
///
/// `Fixed` is the entity's own matrix. The billboard modes rebuild the axes from the camera so the
/// quad faces the view plane; see [`SpriteOrientation`] for the exact semantics. In every case the
/// quad ends up as real world-space geometry that the shared view-projection then projects.
pub(crate) fn sprite_placement(global: &Mat4, orientation: SpriteOrientation, view: &CameraView) -> Mat4 {
    let origin = global.cols[3].xyz();
    let (a, b, c) = (global.cols[0].xyz(), global.cols[1].xyz(), global.cols[2].xyz());
    let (sx, sy, sz) = (a.length(), b.length(), c.length());
    // A negative determinant means an odd number of mirrors (e.g. `scale.x = -1` to face left).
    // Billboards rebuild their axes, so the mirror is re-applied to the sprite's X axis.
    let mirror = if a.cross(b).dot(c) < 0.0 { -1.0 } else { 1.0 };
    let (right, up, forward) = match orientation {
        SpriteOrientation::Fixed => return *global,
        SpriteOrientation::Billboard => (view.right, view.up, view.forward),
        SpriteOrientation::Upright => {
            // Right is horizontal and perpendicular to the view direction. Looking straight along
            // world Y has no defined heading, so fall back to the camera's own right axis.
            let right = Vec3::Y.cross(view.forward);
            let right = if right.length_squared() > 1e-8 { right.normalized() } else { view.right };
            (right, Vec3::Y, right.cross(Vec3::Y))
        }
    };
    // Roll is the entity's twist about its own Z axis (swing-twist decomposition), so tilting or
    // turning the entity does not leak into a billboard but a bone can still swing the sprite.
    let rotation = Quat::from_mat3(&Mat3::from_cols(
        a * (mirror / sx.max(f32::EPSILON)),
        b / sy.max(f32::EPSILON),
        c / sz.max(f32::EPSILON),
    ));
    let roll = 2.0 * rotation.z.atan2(rotation.w);
    let (sin, cos) = roll.sin_cos();
    let (right, up) = (right * cos + up * sin, up * cos - right * sin);
    Mat4::from_cols(
        (right * (sx * mirror)).extend(0.0),
        (up * sy).extend(0.0),
        (forward * sz).extend(0.0),
        origin.extend(1.0),
    )
}

fn sub_region(uv: Rect, region: Option<Rect>) -> Rect {
    match region {
        None => uv,
        Some(r) => {
            let size = uv.size();
            Rect::new(
                uv.min + Vec2::new(r.min.x * size.x, r.min.y * size.y),
                uv.min + Vec2::new(r.max.x * size.x, r.max.y * size.y),
            )
        }
    }
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
    texture: &Texture,
    frustum: &Frustum,
    out: &mut Vec<SpriteInstance>,
    stats: &mut ExtractStats,
) {
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
            let model = *global
                * Mat4::from_translation(origin)
                * Mat4::from_scale(Vec3::new(map.tile_size.x, map.tile_size.y, 1.0));
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
                shading: SpriteShading::LegacyCutout,
                material_id: None,
                material_revision: 0,
            });
        }
    }
}
