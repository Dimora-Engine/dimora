use std::collections::{HashMap, HashSet};

use engine_assets::{AlphaMode, AssetHandle, AssetId, AssetManager, MaterialAsset, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Mat3, Mat4, Quat, Rect, Vec2, Vec3};
use engine_world::{Entity, GlobalTransform, Parent, Visibility, World};

use crate::camera::{Camera, CameraClear, CameraOutput, CameraView};
use crate::components::{
    AmbientLight, Fog, Light, LightKind, Mesh, RenderOrder, Sprite, SpriteBlend, SpriteOrientation, TileMap,
};
use crate::coplanar::{resolve_coplanar_thickness, CoplanarPriority, CoplanarSettings};
use crate::debug::DebugDraw;
use crate::frustum::Frustum;
use crate::interpolate::{render_poses, RenderPoses};
use crate::missing::{report_absent_texture, resolve, MissingAssetWarnings};
use crate::render_target::{RenderTargetId, RenderTargetStore};
use crate::scene::*;

/// Snapshots the world into a [`RenderScene`] for every active camera. `target_size` is the
/// **surface** in pixels. A camera whose output is an offscreen target resolves its viewport,
/// aspect, and frustum against that target's dimensions instead.
///
/// Active cameras are sorted by ascending `(Camera.order, entity id)`. Each one is culled,
/// oriented, and transparency-sorted on its own. Materials and missing-asset warnings are
/// resolved once for the frame. The world is only read; nothing here touches a GPU. Returns
/// `None` when no active camera has a drawable destination. A camera whose offscreen target is
/// missing is counted on the way and skipped. It is not redirected to the surface.
///
/// `transforms` must be up to date: call `engine_world::propagate_transforms` first.
///
/// An entity with [`engine_world::Interpolate`] and a stored previous local is drawn from a
/// render matrix blended toward its current local. That blend is not written to `Transform` or
/// `GlobalTransform`. Sprites, meshes, tilemaps, lights, and cameras share it. With no stored
/// previous local anywhere, every matrix is that entity's `GlobalTransform`.
///
/// A sprite, mesh, tilemap, or light is omitted when it or any ancestor has
/// [`Visibility::Hidden`]. Absence of `Visibility` means visible. The components are only read.
/// [`Camera::active`] decides which cameras render; `Visibility` does not.
///
/// [`DebugDraw`] segments are copied onto the scene and are not removed from the world. Every
/// pass shares that one list. A missing resource is an empty list. The copy is not frustum-culled.
pub fn extract_scene(world: &World, assets: &AssetManager, target_size: Vec2) -> Option<RenderScene> {
    let hidden = AncestorVisibility::read(world);
    let poses = render_poses(world);
    let (cameras, missing_outputs) = ordered_cameras(world, target_size, &poses);
    if cameras.is_empty() {
        return None;
    }

    let mut shared = ExtractStats::default();
    let mut watch = world.try_resource::<MissingAssetWarnings>().is_some_and(|warnings| !warnings.is_empty());
    let sprites = prepare_sprites(world, assets, &poses, &hidden, &mut watch, &mut shared);
    let tiles = prepare_tiles(world, assets, &poses, &hidden, &mut watch, &mut shared);
    let meshes = prepare_meshes(world, assets, &poses, &hidden, &mut watch, &mut shared);
    let lights = prepare_lights(world, &poses, &hidden);

    let ambient = world.try_resource::<AmbientLight>().map_or(AmbientLight::default(), |a| *a);
    let ambient = [ambient.color.r, ambient.color.g, ambient.color.b].map(|c| c * ambient.intensity);
    let fog = world.try_resource::<Fog>().map_or(Fog::default(), |f| *f);
    let fog = [fog.color.r, fog.color.g, fog.color.b, fog.density];
    let coplanar_thickness =
        resolve_coplanar_thickness(world.try_resource::<CoplanarSettings>().map(|settings| *settings));

    let mut written = HashSet::new();
    let mut missing_targets = missing_outputs;
    let mut passes = Vec::with_capacity(cameras.len());
    for camera in &cameras {
        let pass =
            build_pass(world, assets, camera, &sprites, &tiles, &meshes, &lights, &written, &mut missing_targets);
        if let CameraOutput::Target(id) = pass.output {
            written.insert(id);
        }
        passes.push(pass);
    }
    let targets = world.try_resource::<RenderTargetStore>().map(|store| store.descriptions()).unwrap_or_default();
    let debug_lines = world.try_resource::<DebugDraw>().map(|draw| draw.segments().to_vec()).unwrap_or_default();
    let debug_segments = u32::try_from(debug_lines.len()).unwrap_or(u32::MAX);
    let mut stats = ExtractStats {
        missing_assets: shared.missing_assets,
        missing_targets,
        debug_segments,
        ..ExtractStats::default()
    };
    for pass in &passes {
        stats.sprites_visible += pass.stats.sprites_visible;
        stats.sprites_culled += pass.stats.sprites_culled;
        stats.meshes_visible += pass.stats.meshes_visible;
        stats.meshes_culled += pass.stats.meshes_culled;
        stats.lights += pass.stats.lights;
        stats.tiles_visited += pass.stats.tiles_visited;
        stats.chunks_tested += pass.stats.chunks_tested;
        stats.chunks_rejected += pass.stats.chunks_rejected;
        stats.chunks_rebuilt += pass.stats.chunks_rebuilt;
    }
    Some(RenderScene { passes, targets, ambient, fog, coplanar_thickness, debug_lines, stats })
}

/// One active camera in draw order. Stored by value on each [`CameraPass`].
/// [`Self::output`] is the surface or an offscreen target. [`active_surface_cameras`] keeps only
/// the surface entries.
#[derive(Clone, Copy, Debug)]
pub struct OrderedCamera {
    pub camera_id: u32,
    pub order: i32,
    pub output: CameraOutput,
    pub clear: CameraClear,
    pub clear_color: Color,
    pub view: CameraView,
}

/// The topmost surface camera under a window pixel, plus that pixel in viewport-local space.
///
/// This chooses a camera. It does not ask which object is visible at the pixel.
#[derive(Clone, Copy, Debug)]
pub struct SurfaceHit {
    pub camera_id: u32,
    pub order: i32,
    pub view: CameraView,
    /// `window_pixel - viewport_origin`, ready for [`CameraView::ray_from_pixels`].
    pub viewport_pixel: Vec2,
}

/// Active surface cameras in draw order: ascending [`Camera::order`], then ascending entity id.
///
/// An inactive camera is absent. `Visibility` is not consulted. Offscreen cameras are absent;
/// [`camera_view_of`] still resolves one by id, using that target's dimensions. The same ordering
/// feeds extraction, so tests do not keep a second sort.
pub fn active_surface_cameras(world: &World, target_size: Vec2) -> Vec<OrderedCamera> {
    ordered_cameras(world, target_size, &render_poses(world))
        .0
        .into_iter()
        .filter(|camera| camera.output == CameraOutput::Surface)
        .collect()
}

/// The last drawn active surface camera whose viewport contains `window_pixel`.
///
/// Overlap uses that later camera. A pixel in no surface viewport returns `None`. Offscreen
/// cameras are not candidates, including one drawn later than every surface camera. The returned
/// [`SurfaceHit::viewport_pixel`] is viewport-local to the surface.
pub fn top_surface_camera(world: &World, window_pixel: Vec2, target_size: Vec2) -> Option<SurfaceHit> {
    active_surface_cameras(world, target_size)
        .into_iter()
        .rfind(|camera| camera.view.contains_window_pixel(window_pixel))
        .map(|camera| SurfaceHit {
            camera_id: camera.camera_id,
            order: camera.order,
            view: camera.view,
            viewport_pixel: camera.view.window_to_viewport(window_pixel),
        })
}

/// The view of one camera entity, whether or not it is the topmost or even active.
///
/// `target_size` is the surface. An offscreen camera uses its target's dimensions instead, and
/// this returns `None` when that target is missing. It also returns `None` when no camera entity
/// has this id. Callers that already know which camera they want use this, then
/// [`CameraView::window_to_viewport`] and [`CameraView::ray_from_pixels`]. Offscreen pixels are
/// target-local, not window pixels.
pub fn camera_view_of(world: &World, camera_id: u32, target_size: Vec2) -> Option<CameraView> {
    let poses = render_poses(world);
    world.query::<(&Camera, &GlobalTransform)>().iter().find(|(entity, _)| entity.id() == camera_id).and_then(
        |(entity, (camera, global))| {
            let size = destination_size(world, camera.output, target_size)?;
            let placed = GlobalTransform { matrix: poses.matrix(entity, global) };
            Some(CameraView::new(camera, &placed, size))
        },
    )
}

fn destination_size(world: &World, output: CameraOutput, surface: Vec2) -> Option<Vec2> {
    match output {
        CameraOutput::Surface => Some(surface),
        CameraOutput::Target(id) => {
            let store = world.try_resource::<RenderTargetStore>()?;
            let desc = *store.get(id)?;
            Some(Vec2::new(desc.width as f32, desc.height as f32))
        }
    }
}

fn ordered_cameras(world: &World, target_size: Vec2, poses: &RenderPoses) -> (Vec<OrderedCamera>, u32) {
    let mut cameras = Vec::new();
    let mut missing_outputs = 0u32;
    for (entity, (camera, global)) in world.query::<(&Camera, &GlobalTransform)>().iter() {
        if !camera.active {
            continue;
        }
        let Some(size) = destination_size(world, camera.output, target_size) else {
            missing_outputs += 1;
            if let CameraOutput::Target(id) = camera.output {
                report_target(
                    world,
                    id,
                    entity.id(),
                    "missing as a camera output and was not redirected to the surface",
                );
            }
            continue;
        };
        let placed = GlobalTransform { matrix: poses.matrix(entity, global) };
        cameras.push(OrderedCamera {
            camera_id: entity.id(),
            order: camera.order,
            output: camera.output,
            clear: camera.clear,
            clear_color: camera.clear_color,
            view: CameraView::new(camera, &placed, size),
        });
    }
    cameras.sort_by_key(|camera| (camera.order, camera.camera_id));
    (cameras, missing_outputs)
}

fn sprite_sample(
    world: &World,
    camera: &OrderedCamera,
    sprite: &PreparedSprite,
    written: &HashSet<RenderTargetId>,
    missing_targets: &mut u32,
) -> (Option<RenderTargetId>, bool, [f32; 4]) {
    let Some(id) = sprite.render_target else {
        return (None, false, sprite.uv);
    };
    let uv = flipped_uv(Rect::new(Vec2::ZERO, Vec2::ONE), sprite.flip_x, sprite.flip_y);
    let stored = world.try_resource::<RenderTargetStore>().is_some_and(|store| store.get(id).is_some());
    let writing_it = camera.output == CameraOutput::Target(id);
    if stored && written.contains(&id) && !writing_it {
        return (Some(id), false, uv);
    }
    *missing_targets += 1;
    let reason = if !stored {
        "is missing and was not sampled"
    } else if writing_it {
        "is this camera's current output and was not sampled"
    } else {
        "has not been written earlier in this frame and was not sampled"
    };
    report_target(world, id, camera.camera_id, reason);
    (Some(id), true, uv)
}

fn report_target(world: &World, id: RenderTargetId, camera_id: u32, reason: &str) {
    let fresh = world.try_resource::<RenderTargetStore>().is_none_or(|store| store.warn_once(id));
    if fresh {
        log::warn!("render target {id} {reason} (camera {camera_id})");
    }
}

struct PreparedSprite {
    matrix: Mat4,
    orientation: SpriteOrientation,
    size: Vec2,
    anchor: Vec2,
    uv: [f32; 4],
    flip_x: bool,
    flip_y: bool,
    render_target: Option<RenderTargetId>,
    appearance: SpriteAppearance,
    order: RenderOrder,
    coplanar_level: Option<u8>,
}

fn accepted_priority(entity: Entity, priority: Option<CoplanarPriority>) -> Option<u8> {
    match priority.map(CoplanarPriority::checked) {
        Some(Ok(level)) => Some(level),
        Some(Err(error)) => {
            log::warn!("coplanar priority on entity {} {error}; that sprite keeps its real depth", entity.id());
            None
        }
        None => None,
    }
}

struct PreparedMesh {
    instance: MeshInstance,
    world_center: Vec3,
    world_radius: f32,
    order: RenderOrder,
}

struct PreparedTile {
    entity: Entity,
    matrix: Mat4,
    texture: AssetHandle<Texture>,
}

fn prepare_sprites(
    world: &World,
    assets: &AssetManager,
    poses: &RenderPoses,
    hidden: &AncestorVisibility,
    watch: &mut bool,
    stats: &mut ExtractStats,
) -> Vec<PreparedSprite> {
    let mut prepared = Vec::new();
    for (entity, (sprite, global, order, priority)) in
        world.query::<(&Sprite, &GlobalTransform, Option<&RenderOrder>, Option<&CoplanarPriority>)>().iter()
    {
        if hidden.hides(entity) {
            continue;
        }
        let Some(asset) = resolve(world, assets, sprite.asset, "sprite", watch, stats) else {
            continue;
        };
        let Some(appearance) = sprite_appearance(world, assets, sprite, asset, watch, stats) else {
            continue;
        };
        prepared.push(PreparedSprite {
            matrix: poses.matrix(entity, global),
            orientation: sprite.orientation,
            size: sprite.size.unwrap_or_else(|| asset.world_size()),
            anchor: sprite.anchor.unwrap_or(asset.pivot),
            uv: flipped_uv(sub_region(asset.uv, sprite.region), sprite.flip_x, sprite.flip_y),
            flip_x: sprite.flip_x,
            flip_y: sprite.flip_y,
            render_target: sprite.render_target,
            appearance,
            order: order.copied().unwrap_or_default(),
            coplanar_level: accepted_priority(entity, priority.copied()),
        });
    }
    prepared
}

fn prepare_tiles(
    world: &World,
    assets: &AssetManager,
    poses: &RenderPoses,
    hidden: &AncestorVisibility,
    watch: &mut bool,
    stats: &mut ExtractStats,
) -> Vec<PreparedTile> {
    let mut prepared = Vec::new();
    for (entity, (map, global)) in world.query::<(&TileMap, &GlobalTransform)>().iter() {
        if hidden.hides(entity) {
            continue;
        }
        let Some(_) = resolve(world, assets, map.texture, "tilemap", watch, stats) else {
            continue;
        };
        prepared.push(PreparedTile { entity, matrix: poses.matrix(entity, global), texture: map.texture });
    }
    prepared
}

fn prepare_meshes(
    world: &World,
    assets: &AssetManager,
    poses: &RenderPoses,
    hidden: &AncestorVisibility,
    watch: &mut bool,
    stats: &mut ExtractStats,
) -> Vec<PreparedMesh> {
    let mut prepared = Vec::new();
    for (entity, (mesh, global, order)) in world.query::<(&Mesh, &GlobalTransform, Option<&RenderOrder>)>().iter() {
        if hidden.hides(entity) {
            continue;
        }
        let Some(data) = resolve(world, assets, mesh.asset, "mesh", watch, stats) else {
            continue;
        };
        let Some(surface) = mesh_surface(world, assets, mesh, watch, stats) else {
            continue;
        };
        let (center, radius) = data.bounding_sphere();
        let model = poses.matrix(entity, global);
        let max_scale =
            model.cols[0].xyz().length().max(model.cols[1].xyz().length()).max(model.cols[2].xyz().length());
        prepared.push(PreparedMesh {
            world_center: model.transform_point3(center),
            world_radius: radius * max_scale,
            order: order.copied().unwrap_or_default(),
            instance: MeshInstance {
                model,
                mesh: mesh.asset,
                color: surface.color,
                texture: surface.texture,
                lit: surface.lit,
                draw: surface.draw,
                cutoff: surface.cutoff,
                material_id: surface.material_id,
                material_revision: surface.material_revision,
            },
        });
    }
    prepared
}

fn prepare_lights(world: &World, poses: &RenderPoses, hidden: &AncestorVisibility) -> Vec<LightData> {
    world
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
        .collect()
}

fn select_lights(lights: &[LightData], view: &CameraView) -> Vec<LightData> {
    let mut lights = lights.to_vec();
    if lights.len() > MAX_LIGHTS {
        let rank = |light: &LightData| match light.kind {
            LightKind::Directional => -1.0,
            _ => light.position.distance_squared(view.position),
        };
        lights.sort_by(|a, b| rank(a).total_cmp(&rank(b)));
        lights.truncate(MAX_LIGHTS);
    }
    lights
}

#[allow(clippy::too_many_arguments)] // target availability is the written set plus the missing counter
fn build_pass(
    world: &World,
    assets: &AssetManager,
    camera: &OrderedCamera,
    sprites: &[PreparedSprite],
    tiles: &[PreparedTile],
    meshes: &[PreparedMesh],
    lights: &[LightData],
    written: &HashSet<RenderTargetId>,
    missing_targets: &mut u32,
) -> CameraPass {
    let view = &camera.view;
    let frustum = Frustum::from_view_proj(&view.view_proj);
    let mut stats = ExtractStats::default();
    let mut cutout: Vec<SpriteInstance> = Vec::new();
    let mut opaque: Vec<SpriteInstance> = Vec::new();
    let mut mask: Vec<SpriteInstance> = Vec::new();
    let mut sequence = 0u32;
    let mut blended: Vec<BlendItem> = Vec::new();

    for sprite in sprites {
        let placement = sprite_placement(&sprite.matrix, sprite.orientation, view);
        let model = quad_matrix(&placement, sprite.size, sprite.anchor);
        if !quad_visible(&frustum, &model) {
            stats.sprites_culled += 1;
            continue;
        }
        stats.sprites_visible += 1;
        let appearance = &sprite.appearance;
        let (render_target, unavailable_target, uv) = sprite_sample(world, camera, sprite, written, missing_targets);
        let instance = SpriteInstance {
            model,
            uv,
            tint: appearance.tint,
            lit: appearance.lit,
            texture: appearance.texture,
            render_target,
            unavailable_target,
            shading: appearance.shading,
            material_id: appearance.material_id,
            material_revision: appearance.material_revision,
            coplanar_level: sprite.coplanar_level,
        };
        match appearance.shading {
            SpriteShading::LegacyCutout => cutout.push(instance),
            SpriteShading::Opaque { .. } => opaque.push(instance),
            SpriteShading::Mask { .. } => mask.push(instance),
            SpriteShading::LegacyBlend | SpriteShading::Blend { .. } => {
                let depth = view.view.transform_point3(quad_center(&model)).z;
                blended.push(BlendItem {
                    layer: sprite.order.layer,
                    depth,
                    order: sprite.order.order,
                    sequence,
                    kind: BlendKind::Sprite(instance),
                });
                sequence += 1;
            }
        }
    }

    for tile in tiles {
        let Some(map) = world.component::<TileMap>(tile.entity).ok() else { continue };
        let Some(texture) = assets.get(tile.texture) else { continue };
        extract_tilemap(&map, &tile.matrix, texture, &frustum, &mut cutout, &mut stats);
    }

    let mut drawn_meshes = Vec::new();
    for mesh in meshes {
        if !frustum.intersects_sphere(mesh.world_center, mesh.world_radius) {
            stats.meshes_culled += 1;
            continue;
        }
        stats.meshes_visible += 1;
        if matches!(mesh.instance.draw, MeshDraw::Blend { .. }) {
            let depth = view.view.transform_point3(mesh.world_center).z;
            blended.push(BlendItem {
                layer: mesh.order.layer,
                depth,
                order: mesh.order.order,
                sequence,
                kind: BlendKind::Mesh(mesh.instance),
            });
            sequence += 1;
        } else {
            drawn_meshes.push(mesh.instance);
        }
    }
    drawn_meshes.sort_by_key(|m| (m.mesh.id(), m.texture.map(|t| t.id()), m.draw, m.material_id, m.material_revision));
    blended.sort_by(|a, b| {
        a.layer
            .cmp(&b.layer)
            .then(b.depth.total_cmp(&a.depth))
            .then(a.order.cmp(&b.order))
            .then(a.sequence.cmp(&b.sequence))
    });
    let (blended_sprites, blended_meshes, transparent) = pack_transparent(blended);
    let group = |i: &SpriteInstance| {
        let source = if i.unavailable_target {
            (0u8, 0u64)
        } else if let Some(id) = i.render_target {
            (1u8, id.raw())
        } else {
            (2u8, i.texture.id().0)
        };
        (source, i.shading.pipeline(), i.material_id, i.material_revision)
    };
    opaque.sort_by_key(group);
    mask.sort_by_key(group);
    cutout.sort_by_key(group);

    let lights = select_lights(lights, view);
    stats.lights = lights.len() as u32;
    CameraPass {
        camera_id: camera.camera_id,
        order: camera.order,
        view: *view,
        output: camera.output,
        clear: camera.clear,
        clear_color: camera.clear_color,
        viewport_px: [view.viewport_origin.x, view.viewport_origin.y, view.viewport.x, view.viewport.y],
        lights,
        meshes: drawn_meshes,
        opaque_sprites: SpriteLayer::from_ordered(opaque),
        mask_sprites: SpriteLayer::from_ordered(mask),
        cutout_sprites: SpriteLayer::from_ordered(cutout),
        blended_sprites,
        blended_meshes,
        transparent,
        stats,
    }
}

enum BlendKind {
    Sprite(SpriteInstance),
    Mesh(MeshInstance),
}

struct BlendItem {
    layer: i32,
    depth: f32,
    order: i32,
    /// Extraction visit order. Sprites take the earlier numbers, then meshes.
    sequence: u32,
    kind: BlendKind,
}

/// Packs the sorted blend list. A run extends only when the new item is the same kind as the
/// previous one and [`SpriteInstance::shares_batch`] or [`MeshInstance::shares_batch`] accepts it.
/// A different material, pipeline, texture, or mesh asset ends the run. Nothing here reorders.
fn pack_transparent(items: Vec<BlendItem>) -> (SpriteLayer, Vec<MeshInstance>, Vec<TransparentBatch>) {
    let mut sprites = Vec::new();
    let mut meshes = Vec::new();
    let mut transparent = Vec::new();
    let mut sprite_batches = Vec::new();
    for item in items {
        match item.kind {
            BlendKind::Sprite(instance) => append_sprite(&mut sprites, &mut transparent, &mut sprite_batches, instance),
            BlendKind::Mesh(instance) => append_mesh(&mut meshes, &mut transparent, instance),
        }
    }
    (SpriteLayer { instances: sprites, batches: sprite_batches }, meshes, transparent)
}

fn append_sprite(
    sprites: &mut Vec<SpriteInstance>,
    transparent: &mut Vec<TransparentBatch>,
    sprite_batches: &mut Vec<SpriteBatch>,
    instance: SpriteInstance,
) {
    let index = sprites.len() as u32;
    let texture = instance.texture;
    let render_target = instance.render_target;
    let unavailable_target = instance.unavailable_target;
    let pipeline = instance.shading.pipeline();
    let extend = sprites.last().is_some_and(|previous| previous.shares_batch(&instance))
        && matches!(transparent.last(), Some(TransparentBatch::Sprites(_)));
    sprites.push(instance);
    if extend {
        if let Some(TransparentBatch::Sprites(batch)) = transparent.last_mut() {
            batch.range.end = index + 1;
        }
        sprite_batches.last_mut().expect("a sprite run has a matching sprite batch").range.end = index + 1;
        return;
    }
    let batch = SpriteBatch { texture, render_target, unavailable_target, pipeline, range: index..index + 1 };
    transparent.push(TransparentBatch::Sprites(batch.clone()));
    sprite_batches.push(batch);
}

fn append_mesh(meshes: &mut Vec<MeshInstance>, transparent: &mut Vec<TransparentBatch>, instance: MeshInstance) {
    let index = meshes.len() as u32;
    let mesh = instance.mesh;
    let texture = instance.texture;
    let draw = instance.draw;
    let extend = meshes.last().is_some_and(|previous| previous.shares_batch(&instance))
        && matches!(transparent.last(), Some(TransparentBatch::Meshes { .. }));
    meshes.push(instance);
    if extend {
        if let Some(TransparentBatch::Meshes { range, .. }) = transparent.last_mut() {
            range.end = index + 1;
        }
        return;
    }
    transparent.push(TransparentBatch::Meshes { mesh, texture, draw, range: index..index + 1 });
}

#[derive(Clone, Copy)]
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

#[derive(Clone, Copy)]
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
        AlphaMode::Blend => (MeshDraw::Blend { double_sided: material.double_sided }, 0.0),
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
    // UVs are computed from the texture loaded for this frame. A reload that changes the texture
    // size is picked up here. It does not need a chunk rebuild. Tiles have no material asset.
    // `CoplanarPriority` is not read: tiles stay at `coplanar_level: None`. A future opt-in would
    // be copied onto the instance at this push and would not be a batch key (ADR-0020).
    let inset = Vec2::new(0.02 / texture.width as f32, 0.02 / texture.height as f32);
    let cell = Vec2::new(1.0 / map.atlas_columns as f32, 1.0 / map.atlas_rows as f32);
    let edge = TileMap::CHUNK_EDGE;
    map.ensure_chunks();
    let chunks_x = map.width.div_ceil(edge);
    let chunks_y = map.height.div_ceil(edge);
    let mut visible = vec![false; (chunks_x * chunks_y) as usize];

    for cy in 0..chunks_y {
        for cx in 0..chunks_x {
            let slot = (cy * chunks_x + cx) as usize;
            stats.chunks_tested += 1;
            let (rebuilt, occupied) = map.chunk_occupied(cx, cy);
            if rebuilt {
                stats.chunks_rebuilt += 1;
            }
            if chunk_visible(global, map, cx, cy, frustum) {
                visible[slot] = true;
            } else {
                stats.chunks_rejected += 1;
                stats.sprites_culled += occupied;
            }
        }
    }

    for y in 0..map.height {
        let cy = y / edge;
        let mut x = 0;
        while x < map.width {
            let cx = x / edge;
            let x1 = ((cx + 1) * edge).min(map.width);
            if !visible[(cy * chunks_x + cx) as usize] {
                x = x1;
                continue;
            }
            stats.tiles_visited += x1 - x;
            for x in x..x1 {
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
                    render_target: None,
                    unavailable_target: false,
                    shading: SpriteShading::LegacyCutout,
                    material_id: None,
                    material_revision: 0,
                    // Tiles do not opt into coplanar priority. See the note above this function.
                    coplanar_level: None,
                });
            }
            x = x1;
        }
    }
}

/// World-space sphere around a chunk rectangle after `global`. The sphere uses the transformed
/// corners, so a rotated, scaled, or sheared map is not treated as axis-aligned.
fn chunk_visible(global: &Mat4, map: &TileMap, cx: u32, cy: u32, frustum: &Frustum) -> bool {
    let edge = TileMap::CHUNK_EDGE;
    let x0 = cx * edge;
    let y0 = cy * edge;
    let x1 = (x0 + edge).min(map.width);
    let y1 = (y0 + edge).min(map.height);
    let (sx, sy) = (map.tile_size.x, map.tile_size.y);
    let corners = [
        Vec3::new(x0 as f32 * sx, y0 as f32 * sy, 0.0),
        Vec3::new(x1 as f32 * sx, y0 as f32 * sy, 0.0),
        Vec3::new(x0 as f32 * sx, y1 as f32 * sy, 0.0),
        Vec3::new(x1 as f32 * sx, y1 as f32 * sy, 0.0),
    ];
    let world = corners.map(|corner| global.transform_point3(corner));
    let center = (world[0] + world[1] + world[2] + world[3]) * 0.25;
    let radius = world.iter().fold(0.0_f32, |radius, corner| radius.max((*corner - center).length()));
    // A hair of padding keeps a corner tile from falling out of a sphere that already contains it
    // once the matrix multiply rounds.
    frustum.intersects_sphere(center, radius * (1.0 + 1e-5) + 1e-4)
}
