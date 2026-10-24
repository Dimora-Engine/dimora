//! Deterministic worlds for the M6 render-scale benchmark.
//!
//! This is benchmark data, not a gameplay API and not a scene format. Each [`Case`] builds a
//! fresh [`World`] and [`AssetManager`]. The
//! CPU harness times [`extract_scene`](crate::extract_scene). The GPU harness submits the same
//! worlds. Adding a later workload is a new [`Case`]; sprite sheets, skeletal poses, skinned
//! meshes, shadows, and editor viewports are not represented here because those systems are not
//! implemented.
//!
//! Visible and culled counts in [`Summary`] are summed across cameras, matching [`ExtractStats`](crate::ExtractStats).
//! Batch counts are summed the same way. Sorting and instance building are inside extraction and
//! are not timed apart from it.

use engine_assets::{AssetHandle, AssetManager, MaterialAsset, MeshData, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Quat, Transform, Vec2, Vec3};
use engine_world::{propagate_transforms, Entity, World};

use engine_assets::AlphaMode;

use crate::{
    Camera, CameraOutput, CoplanarPriority, DebugDraw, Light, Mesh, MeshInstance, RenderScene, RenderTargetId,
    RenderTargetStore, Sprite, SpriteOrientation, TileMap, TransparentBatch,
};

/// Surface size used by every harness. Offscreen targets in [`Case`] `offscreen-1k` use it too.
pub const VIEWPORT_WIDTH: f32 = 1280.0;
pub const VIEWPORT_HEIGHT: f32 = 720.0;

/// Ground tiles on each side of the lane scenes.
pub const LANE_TILES: u32 = 48;
/// Upright opaque actors in the lane.
pub const LANE_ACTORS: u32 = 160;
/// Upright mask trees in the lane.
pub const LANE_TREES: u32 = 48;
/// Billboard blended sprites in the lane.
pub const LANE_GLOWS: u32 = 24;
/// Shared-cuboid crates in the lane. Zero when the lane is sprite-only.
pub const LANE_CRATES: u32 = 16;
/// Shared-cuboid buildings in the lane. Zero when the lane is sprite-only.
pub const LANE_BUILDINGS: u32 = 6;
/// Crowd in the offscreen and two-surface pair, not counting the one sampling sprite.
pub const OFFSCREEN_CROWD: u32 = 1_000;

const ISO_PITCH: f32 = 0.615_479_7;
const CHILDREN: u32 = 10;

/// One catalog entry. `build` allocates a new world; it does not reuse a previous case.
#[derive(Clone, Copy)]
pub struct Case {
    pub label: &'static str,
    /// Included in the default GPU harness. A `DIMORA_SCALE_CASES` filter can still request it.
    pub gpu: bool,
    /// The CPU harness nudges sprite locals and re-propagates before the steady extract.
    pub motion: bool,
    pub build: fn() -> Built,
}

/// Authored scene. Counts are entities of that kind, not the visible subset.
pub struct Built {
    pub world: World,
    pub assets: AssetManager,
    pub sprites: u32,
    pub meshes: u32,
    pub lights: u32,
    pub tiles: u32,
    pub cameras: u32,
    /// Sprites, meshes, lights, cameras, tilemap entities, and hierarchy roots.
    pub entities: u32,
}

/// Counts taken from one [`RenderScene`]. Batch totals repeat for every camera that draws them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Summary {
    pub passes: u32,
    pub visible_sprites: u32,
    pub culled_sprites: u32,
    pub visible_meshes: u32,
    pub culled_meshes: u32,
    pub tiles_visited: u32,
    pub chunks_tested: u32,
    pub chunks_rejected: u32,
    pub chunks_rebuilt: u32,
    pub sprite_batches: u32,
    pub mesh_draws: u32,
    pub transparent_batches: u32,
    pub lights_min: u32,
    pub lights_max: u32,
    pub missing_assets: u32,
    pub missing_targets: u32,
    pub debug_segments: u32,
    /// Capacity of the scene's vectors, in bytes. This is retained until the scene is dropped.
    pub retained_bytes: u64,
}

pub fn viewport() -> Vec2 {
    Vec2::new(VIEWPORT_WIDTH, VIEWPORT_HEIGHT)
}

/// Index counts of the shared cuboid and the dense sphere used by the mesh cases.
pub fn reference_mesh_indices() -> (usize, usize) {
    (shared_cuboid().indices.len(), dense_sphere().indices.len())
}

pub fn summarize(scene: &RenderScene) -> Summary {
    let mut lights_min = u32::MAX;
    let mut lights_max = 0;
    let mut sprite_batches = 0;
    let mut mesh_draws = 0;
    let mut transparent_batches = 0;
    for pass in &scene.passes {
        lights_min = lights_min.min(pass.lights.len() as u32);
        lights_max = lights_max.max(pass.lights.len() as u32);
        sprite_batches += (pass.opaque_sprites.batches.len()
            + pass.mask_sprites.batches.len()
            + pass.cutout_sprites.batches.len()
            + pass.blended_sprites.batches.len()) as u32;
        mesh_draws += mesh_runs(&pass.meshes);
        mesh_draws +=
            pass.transparent.iter().filter(|batch| matches!(batch, TransparentBatch::Meshes { .. })).count() as u32;
        transparent_batches += pass.transparent.len() as u32;
    }
    if scene.passes.is_empty() {
        lights_min = 0;
    }
    let stats = scene.stats;
    Summary {
        passes: scene.passes.len() as u32,
        visible_sprites: stats.sprites_visible,
        culled_sprites: stats.sprites_culled,
        visible_meshes: stats.meshes_visible,
        culled_meshes: stats.meshes_culled,
        tiles_visited: stats.tiles_visited,
        chunks_tested: stats.chunks_tested,
        chunks_rejected: stats.chunks_rejected,
        chunks_rebuilt: stats.chunks_rebuilt,
        sprite_batches,
        mesh_draws,
        transparent_batches,
        lights_min,
        lights_max,
        missing_assets: stats.missing_assets,
        missing_targets: stats.missing_targets,
        debug_segments: stats.debug_segments,
        retained_bytes: retained_bytes(scene),
    }
}

pub fn retained_bytes(scene: &RenderScene) -> u64 {
    let mut bytes = vec_bytes(&scene.passes) + vec_bytes(&scene.targets) + vec_bytes(&scene.debug_lines);
    for pass in &scene.passes {
        bytes += vec_bytes(&pass.lights);
        bytes += vec_bytes(&pass.meshes);
        bytes += vec_bytes(&pass.blended_meshes);
        bytes += vec_bytes(&pass.transparent);
        bytes += layer_bytes(&pass.opaque_sprites.instances, &pass.opaque_sprites.batches);
        bytes += layer_bytes(&pass.mask_sprites.instances, &pass.mask_sprites.batches);
        bytes += layer_bytes(&pass.cutout_sprites.instances, &pass.cutout_sprites.batches);
        bytes += layer_bytes(&pass.blended_sprites.instances, &pass.blended_sprites.batches);
    }
    bytes
}

/// Every catalog case. Labels are stable so later commits can compare rows.
pub fn cases() -> Vec<Case> {
    vec![
        case("sprite-opaque-1k", true, false, sprite_opaque_1k),
        case("sprite-opaque-10k", true, false, sprite_opaque_10k),
        case("sprite-opaque-100k", true, false, sprite_opaque_100k),
        case("sprite-mask-1k", false, false, sprite_mask_1k),
        case("sprite-mask-10k", true, false, sprite_mask_10k),
        case("sprite-mask-100k", false, false, sprite_mask_100k),
        case("sprite-blend-1k", true, false, sprite_blend_1k),
        case("sprite-blend-10k", true, false, sprite_blend_10k),
        case("sprite-blend-100k", true, false, sprite_blend_100k),
        case("sprite-legacy-cutout-10k", false, false, sprite_legacy_cutout_10k),
        case("sprite-opaque-materials-10k", true, false, sprite_opaque_materials_10k),
        case("sprite-blend-materials-1k", true, false, sprite_blend_materials_1k),
        case("sprite-orient-10k", true, false, sprite_orient_10k),
        case("sprite-coplanar-10k", true, false, sprite_coplanar_10k),
        case("sprite-sparse-mostly-10k", false, false, sprite_sparse_mostly_10k),
        case("sprite-sparse-partial-10k", false, false, sprite_sparse_partial_10k),
        case("sprite-dense-partial-10k", true, false, sprite_dense_partial_10k),
        case("sprite-sparse-tight-10k", true, false, sprite_sparse_tight_10k),
        case("sprite-opaque-10k-unlit", true, false, sprite_opaque_10k_unlit),
        case("sprite-opaque-10k-move", false, true, sprite_opaque_10k),
        case("sprite-hierarchy-1k", false, false, sprite_hierarchy_1k),
        case("sprite-opaque-1k-debug", false, false, sprite_opaque_1k_debug),
        case("sprite-lights-4-10k", false, false, sprite_lights_4_10k),
        case("sprite-lights-16-10k", true, false, sprite_lights_16_10k),
        case("sprite-lights-32-10k", false, false, sprite_lights_32_10k),
        case("sprite-cam-2-10k", false, false, sprite_cam_2_10k),
        case("sprite-cam-7-10k", true, false, sprite_cam_7_10k),
        case("sprite-cam-persp-10k", false, false, sprite_cam_persp_10k),
        case("sprite-cam-iso-10k", false, false, sprite_cam_iso_10k),
        case("sprite-cam-cycle-7-10k", true, false, sprite_cam_cycle_7_10k),
        case("mesh-shared-100", true, false, mesh_shared_100),
        case("mesh-shared-1k", true, false, mesh_shared_1k),
        case("mesh-shared-10k", true, false, mesh_shared_10k),
        case("mesh-distinct-1k", true, false, mesh_distinct_1k),
        case("mesh-materials-1k", true, false, mesh_materials_1k),
        case("mesh-partial-1k", false, false, mesh_partial_1k),
        case("mesh-sphere-100", true, false, mesh_sphere_100),
        case("mesh-lights-4-1k", false, false, mesh_lights_4_1k),
        case("mesh-lights-16-1k", true, false, mesh_lights_16_1k),
        case("lane-sprites", false, false, lane_sprites),
        case("lane-mixed", true, false, lane_mixed),
        case("lane-ortho", false, false, lane_ortho),
        case("lane-iso", false, false, lane_iso),
        case("lane-cameras-2", false, false, lane_cameras_2),
        case("lane-cameras-7", true, false, lane_cameras_7),
        case("lane-lights-0", false, false, lane_lights_0),
        case("lane-lights-16", true, false, lane_lights_16),
        case("offscreen-1k", true, false, offscreen_1k),
        case("surface-2cam-1k", true, false, surface_2cam_1k),
        // D-06A comparison rows. Not part of the historical scale table and not in the GPU harness.
        case("sprite-opaque-10k-snap", false, false, sprite_opaque_10k_snap),
        case("sprite-cam-2-10k-snap", false, false, sprite_cam_2_10k_snap),
    ]
}

fn case(label: &'static str, gpu: bool, motion: bool, build: fn() -> Built) -> Case {
    Case { label, gpu, motion, build }
}

fn sprite_opaque_1k() -> Built {
    sprite_grid(SpriteSpec { count: 1_000, ..SpriteSpec::opaque() })
}
fn sprite_opaque_10k() -> Built {
    sprite_grid(SpriteSpec { count: 10_000, ..SpriteSpec::opaque() })
}
fn sprite_opaque_10k_snap() -> Built {
    enable_pixel_snap(sprite_opaque_10k())
}
fn sprite_cam_2_10k_snap() -> Built {
    enable_pixel_snap(sprite_cam_2_10k())
}
fn sprite_opaque_100k() -> Built {
    sprite_grid(SpriteSpec { count: 100_000, ..SpriteSpec::opaque() })
}
fn sprite_mask_1k() -> Built {
    sprite_grid(SpriteSpec { count: 1_000, class: SpriteClass::Mask, ..SpriteSpec::opaque() })
}
fn sprite_mask_10k() -> Built {
    sprite_grid(SpriteSpec { count: 10_000, class: SpriteClass::Mask, ..SpriteSpec::opaque() })
}
fn sprite_mask_100k() -> Built {
    sprite_grid(SpriteSpec { count: 100_000, class: SpriteClass::Mask, ..SpriteSpec::opaque() })
}
fn sprite_blend_1k() -> Built {
    sprite_grid(SpriteSpec { count: 1_000, class: SpriteClass::Blend, ..SpriteSpec::opaque() })
}
fn sprite_blend_10k() -> Built {
    sprite_grid(SpriteSpec { count: 10_000, class: SpriteClass::Blend, ..SpriteSpec::opaque() })
}
fn sprite_blend_100k() -> Built {
    sprite_grid(SpriteSpec { count: 100_000, class: SpriteClass::Blend, ..SpriteSpec::opaque() })
}
fn sprite_legacy_cutout_10k() -> Built {
    sprite_grid(SpriteSpec { count: 10_000, class: SpriteClass::LegacyCutout, ..SpriteSpec::opaque() })
}
fn sprite_opaque_materials_10k() -> Built {
    sprite_grid(SpriteSpec { count: 10_000, material_count: 8, ..SpriteSpec::opaque() })
}
fn sprite_blend_materials_1k() -> Built {
    sprite_grid(SpriteSpec { count: 1_000, class: SpriteClass::Blend, material_count: 8, ..SpriteSpec::opaque() })
}
fn sprite_orient_10k() -> Built {
    sprite_grid(SpriteSpec { count: 10_000, orientations: OrientationMix::AllThree, ..SpriteSpec::opaque() })
}
fn sprite_coplanar_10k() -> Built {
    sprite_grid(SpriteSpec { count: 10_000, coplanar: true, ..SpriteSpec::opaque() })
}
fn sprite_sparse_mostly_10k() -> Built {
    sprite_grid(SpriteSpec { count: 10_000, spacing: Spacing::Sparse, ..SpriteSpec::opaque() })
}
fn sprite_sparse_partial_10k() -> Built {
    sprite_grid(SpriteSpec {
        count: 10_000,
        spacing: Spacing::Sparse,
        frame: Frame::Fit(Coverage::Partial),
        ..SpriteSpec::opaque()
    })
}
fn sprite_dense_partial_10k() -> Built {
    sprite_grid(SpriteSpec { count: 10_000, frame: Frame::Fit(Coverage::Partial), ..SpriteSpec::opaque() })
}
fn sprite_sparse_tight_10k() -> Built {
    sprite_grid(SpriteSpec {
        count: 10_000,
        spacing: Spacing::Sparse,
        frame: Frame::FixedHeight(20.0),
        ..SpriteSpec::opaque()
    })
}
fn sprite_opaque_10k_unlit() -> Built {
    sprite_grid(SpriteSpec { count: 10_000, lit: false, ..SpriteSpec::opaque() })
}
fn sprite_hierarchy_1k() -> Built {
    sprite_grid(SpriteSpec { count: 1_000, hierarchy: true, ..SpriteSpec::opaque() })
}
fn sprite_opaque_1k_debug() -> Built {
    sprite_grid(SpriteSpec { count: 1_000, debug_segments: 1_000, ..SpriteSpec::opaque() })
}
fn sprite_lights_4_10k() -> Built {
    sprite_grid(SpriteSpec { count: 10_000, lights: 4, ..SpriteSpec::opaque() })
}
fn sprite_lights_16_10k() -> Built {
    sprite_grid(SpriteSpec { count: 10_000, lights: 16, ..SpriteSpec::opaque() })
}
fn sprite_lights_32_10k() -> Built {
    sprite_grid(SpriteSpec { count: 10_000, lights: 32, ..SpriteSpec::opaque() })
}
fn sprite_cam_2_10k() -> Built {
    sprite_grid(SpriteSpec { count: 10_000, cameras: 2, ..SpriteSpec::opaque() })
}
fn sprite_cam_7_10k() -> Built {
    sprite_grid(SpriteSpec { count: 10_000, cameras: 7, ..SpriteSpec::opaque() })
}
fn sprite_cam_persp_10k() -> Built {
    sprite_grid(SpriteSpec { count: 10_000, camera_layout: CameraLayout::Perspective, ..SpriteSpec::opaque() })
}
fn sprite_cam_iso_10k() -> Built {
    sprite_grid(SpriteSpec { count: 10_000, camera_layout: CameraLayout::Isometric, ..SpriteSpec::opaque() })
}
fn sprite_cam_cycle_7_10k() -> Built {
    sprite_grid(SpriteSpec { count: 10_000, cameras: 7, camera_layout: CameraLayout::Cycle, ..SpriteSpec::opaque() })
}
fn mesh_shared_100() -> Built {
    mesh_field(MeshSpec { count: 100, ..MeshSpec::shared() })
}
fn mesh_shared_1k() -> Built {
    mesh_field(MeshSpec { count: 1_000, ..MeshSpec::shared() })
}
fn mesh_shared_10k() -> Built {
    mesh_field(MeshSpec { count: 10_000, ..MeshSpec::shared() })
}
fn mesh_distinct_1k() -> Built {
    mesh_field(MeshSpec { count: 1_000, variety: MeshVariety::Eight, ..MeshSpec::shared() })
}
fn mesh_materials_1k() -> Built {
    mesh_field(MeshSpec { count: 1_000, material_count: 8, ..MeshSpec::shared() })
}
fn mesh_partial_1k() -> Built {
    mesh_field(MeshSpec { count: 1_000, frame: Frame::Fit(Coverage::Partial), ..MeshSpec::shared() })
}
fn mesh_sphere_100() -> Built {
    mesh_field(MeshSpec { count: 100, variety: MeshVariety::Sphere, ..MeshSpec::shared() })
}
fn mesh_lights_4_1k() -> Built {
    mesh_field(MeshSpec { count: 1_000, lights: 4, ..MeshSpec::shared() })
}
fn mesh_lights_16_1k() -> Built {
    mesh_field(MeshSpec { count: 1_000, lights: 16, ..MeshSpec::shared() })
}
fn lane_sprites() -> Built {
    lane(LaneSpec { meshes: false, ..LaneSpec::mixed() })
}
fn lane_mixed() -> Built {
    lane(LaneSpec::mixed())
}
fn lane_ortho() -> Built {
    lane(LaneSpec { camera_layout: CameraLayout::Orthographic, ..LaneSpec::mixed() })
}
fn lane_iso() -> Built {
    lane(LaneSpec { camera_layout: CameraLayout::Isometric, ..LaneSpec::mixed() })
}
fn lane_cameras_2() -> Built {
    lane(LaneSpec { cameras: 2, ..LaneSpec::mixed() })
}
fn lane_cameras_7() -> Built {
    lane(LaneSpec { cameras: 7, camera_layout: CameraLayout::Cycle, ..LaneSpec::mixed() })
}
fn lane_lights_0() -> Built {
    lane(LaneSpec { lights: 0, ..LaneSpec::mixed() })
}
fn lane_lights_16() -> Built {
    lane(LaneSpec { lights: 16, ..LaneSpec::mixed() })
}
fn offscreen_1k() -> Built {
    camera_pair(true)
}
fn surface_2cam_1k() -> Built {
    camera_pair(false)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SpriteClass {
    Opaque,
    Mask,
    Blend,
    LegacyCutout,
}

#[derive(Clone, Copy)]
enum OrientationMix {
    Billboard,
    AllThree,
}

#[derive(Clone, Copy)]
enum Spacing {
    Dense,
    Sparse,
}

#[derive(Clone, Copy)]
enum Coverage {
    Mostly,
    Partial,
}

#[derive(Clone, Copy)]
enum Frame {
    Fit(Coverage),
    FixedHeight(f32),
}

#[derive(Clone, Copy)]
enum CameraLayout {
    Orthographic,
    Perspective,
    Isometric,
    Cycle,
}

#[derive(Clone, Copy)]
enum ViewKind {
    Orthographic,
    Perspective,
    Isometric,
}

struct SpriteSpec {
    count: u32,
    class: SpriteClass,
    material_count: u32,
    orientations: OrientationMix,
    spacing: Spacing,
    frame: Frame,
    cameras: u32,
    camera_layout: CameraLayout,
    lights: u32,
    coplanar: bool,
    lit: bool,
    hierarchy: bool,
    debug_segments: u32,
}

impl SpriteSpec {
    fn opaque() -> Self {
        Self {
            count: 1,
            class: SpriteClass::Opaque,
            material_count: 1,
            orientations: OrientationMix::Billboard,
            spacing: Spacing::Dense,
            frame: Frame::Fit(Coverage::Mostly),
            cameras: 1,
            camera_layout: CameraLayout::Orthographic,
            lights: 0,
            coplanar: false,
            lit: true,
            hierarchy: false,
            debug_segments: 0,
        }
    }
}

#[derive(Clone, Copy)]
enum MeshVariety {
    Cuboid,
    Sphere,
    Eight,
}

struct MeshSpec {
    count: u32,
    variety: MeshVariety,
    material_count: u32,
    frame: Frame,
    cameras: u32,
    camera_layout: CameraLayout,
    lights: u32,
}

impl MeshSpec {
    fn shared() -> Self {
        Self {
            count: 1,
            variety: MeshVariety::Cuboid,
            material_count: 1,
            frame: Frame::Fit(Coverage::Mostly),
            cameras: 1,
            camera_layout: CameraLayout::Orthographic,
            lights: 0,
        }
    }
}

struct LaneSpec {
    meshes: bool,
    cameras: u32,
    camera_layout: CameraLayout,
    lights: u32,
}

impl LaneSpec {
    fn mixed() -> Self {
        Self { meshes: true, cameras: 1, camera_layout: CameraLayout::Perspective, lights: 4 }
    }
}

fn sprite_grid(spec: SpriteSpec) -> Built {
    assert!(spec.count > 0 && spec.cameras > 0);
    assert!(!spec.hierarchy || spec.count.is_multiple_of(CHILDREN));
    let mut assets = AssetManager::in_memory();
    let (sprite_asset, materials) = sprite_assets(&mut assets, &spec);
    let mut world = World::new();
    let groups = if spec.hierarchy { spec.count / CHILDREN } else { spec.count };
    let spacing = match spec.spacing {
        Spacing::Dense => 1.0,
        Spacing::Sparse => 8.0,
    };
    let span = grid_span(groups, spacing);
    let mut roots = 0;
    if spec.hierarchy {
        for group in 0..groups {
            let root =
                world
                    .spawn((Transform::from_xyz(grid_x(group, groups, spacing), 0.0, grid_z(group, groups, spacing)),));
            roots += 1;
            for child in 0..CHILDREN {
                let index = group * CHILDREN + child;
                let local = Transform::from_xyz(0.0, 0.0, child as f32 * 0.15);
                world.spawn_child(root, (local, sprite_for(&spec, sprite_asset, &materials, index)));
            }
        }
    } else {
        for index in 0..spec.count {
            let transform =
                Transform::from_xyz(grid_x(index, spec.count, spacing), 0.0, grid_z(index, spec.count, spacing));
            spawn_sprite(
                &mut world,
                transform,
                sprite_for(&spec, sprite_asset, &materials, index),
                spec.coplanar,
                index,
            );
        }
    }
    add_grid_cameras(&mut world, spec.cameras, spec.camera_layout, spec.frame, span);
    add_point_lights(&mut world, spec.lights, span);
    add_debug(&mut world, spec.debug_segments);
    propagate_transforms(&mut world);
    Built {
        world,
        assets,
        sprites: spec.count,
        meshes: 0,
        lights: spec.lights,
        tiles: 0,
        cameras: spec.cameras,
        entities: spec.count + roots + spec.cameras + spec.lights,
    }
}

fn mesh_field(spec: MeshSpec) -> Built {
    assert!(spec.count > 0 && spec.cameras > 0 && spec.material_count > 0);
    let mut assets = AssetManager::in_memory();
    let meshes = insert_meshes(&mut assets, spec.variety);
    let materials = insert_materials(&mut assets, spec.material_count, AlphaMode::Opaque, true);
    let mut world = World::new();
    let span = grid_span(spec.count, 1.5);
    for index in 0..spec.count {
        let mesh = Mesh {
            asset: meshes[index as usize % meshes.len()],
            material: crate::Material::default(),
            material_asset: Some(materials[index as usize % materials.len()]),
        };
        let transform = Transform::from_xyz(grid_x(index, spec.count, 1.5), 0.4, grid_z(index, spec.count, 1.5));
        world.spawn((transform, mesh));
    }
    add_grid_cameras(&mut world, spec.cameras, spec.camera_layout, spec.frame, span);
    add_point_lights(&mut world, spec.lights, span);
    propagate_transforms(&mut world);
    Built {
        world,
        assets,
        sprites: 0,
        meshes: spec.count,
        lights: spec.lights,
        tiles: 0,
        cameras: spec.cameras,
        entities: spec.count + spec.cameras + spec.lights,
    }
}

fn lane(spec: LaneSpec) -> Built {
    assert!(spec.cameras > 0);
    let mut assets = AssetManager::in_memory();
    let texture = assets.insert("lane-albedo", Texture::solid(4, 4, [70, 110, 60, 255])).expect("lane texture");
    assets.insert("lane-tiles", Texture::solid(4, 4, [50, 80, 40, 255])).expect("lane tiles");
    let sprite_asset = assets.insert("lane-sprite", SpriteAsset::whole_texture(texture, 4, 4)).expect("lane sprite");
    let actor = insert_one(&mut assets, "lane-actor", texture, AlphaMode::Opaque);
    let tree = insert_one(&mut assets, "lane-tree", texture, AlphaMode::mask_default());
    let glow = insert_one(&mut assets, "lane-glow", texture, AlphaMode::Blend);
    let crate_mesh = assets.insert("lane-crate", shared_cuboid()).expect("lane mesh");
    let crate_mat = insert_one(&mut assets, "lane-crate-mat", texture, AlphaMode::Opaque);
    let mut world = World::new();
    let map = TileMap::new("lane-tiles", 1, 1, Vec2::ONE, LANE_TILES, LANE_TILES).filled_with(|_, _| 0);
    world.spawn((ground_transform(LANE_TILES as f32), map));
    for index in 0..LANE_ACTORS {
        let column = index % 80;
        let z = if index < 80 { -3.0 } else { 3.0 };
        let sprite = Sprite::from_asset(sprite_asset).with_size(Vec2::new(0.7, 1.6)).upright().with_material(actor);
        world.spawn((Transform::from_xyz(street_x(column, 80), 0.0, z), sprite));
    }
    for index in 0..LANE_TREES {
        let column = index % 24;
        let z = if index < 24 { -7.0 } else { 7.0 };
        let sprite = Sprite::from_asset(sprite_asset).with_size(Vec2::new(1.4, 2.2)).upright().with_material(tree);
        world.spawn((Transform::from_xyz(street_x(column, 24), 0.0, z), sprite));
    }
    for index in 0..LANE_GLOWS {
        let sprite = Sprite::from_asset(sprite_asset).with_size(Vec2::splat(0.6)).with_material(glow);
        world.spawn((Transform::from_xyz(street_x(index, LANE_GLOWS), 2.0, 0.0), sprite));
    }
    let mesh_count = if spec.meshes { LANE_CRATES + LANE_BUILDINGS } else { 0 };
    if spec.meshes {
        for index in 0..LANE_CRATES {
            let mesh =
                Mesh { asset: crate_mesh, material: crate::Material::default(), material_asset: Some(crate_mat) };
            world.spawn((Transform::from_xyz(street_x(index, LANE_CRATES), 0.4, 0.8), mesh));
        }
        for index in 0..LANE_BUILDINGS {
            let mesh =
                Mesh { asset: crate_mesh, material: crate::Material::default(), material_asset: Some(crate_mat) };
            let z = if index % 2 == 0 { -12.0 } else { 12.0 };
            let transform = Transform::from_xyz(street_x(index, LANE_BUILDINGS), 1.6, z).with_scale(Vec3::splat(4.0));
            world.spawn((transform, mesh));
        }
    }
    add_lane_cameras(&mut world, spec.cameras, spec.camera_layout);
    add_point_lights(&mut world, spec.lights, 40.0);
    propagate_transforms(&mut world);
    let sprites = LANE_ACTORS + LANE_TREES + LANE_GLOWS;
    Built {
        world,
        assets,
        sprites,
        meshes: mesh_count,
        lights: spec.lights,
        tiles: LANE_TILES * LANE_TILES,
        cameras: spec.cameras,
        entities: sprites + mesh_count + spec.lights + spec.cameras + 1,
    }
}

fn camera_pair(offscreen: bool) -> Built {
    let mut built = sprite_grid(SpriteSpec { count: OFFSCREEN_CROWD, ..SpriteSpec::opaque() });
    let ids: Vec<Entity> = built.world.query::<&Camera>().iter().map(|(entity, _)| entity).collect();
    for id in ids {
        built.world.despawn(id).expect("camera");
    }
    let span = grid_span(OFFSCREEN_CROWD, 1.0);
    let target = if offscreen {
        let mut store = RenderTargetStore::new();
        let id = store.create(VIEWPORT_WIDTH as u32, VIEWPORT_HEIGHT as u32).expect("target");
        built.world.insert_resource(store);
        Some(id)
    } else {
        None
    };
    add_pair_cameras(&mut built.world, span, target);
    if let Some(id) = target {
        let sprite_asset = built.assets.sprite("spr-0");
        let material = built.assets.material("mat-0");
        let sprite =
            Sprite::from_asset(sprite_asset).with_size(Vec2::splat(2.0)).with_material(material).with_render_target(id);
        built.world.spawn((Transform::from_xyz(0.0, 1.0, 0.0), sprite));
        built.sprites += 1;
    }
    built.cameras = 2;
    built.entities = built.sprites + built.lights + built.cameras;
    propagate_transforms(&mut built.world);
    built
}

fn sprite_assets(
    assets: &mut AssetManager,
    spec: &SpriteSpec,
) -> (AssetHandle<SpriteAsset>, Vec<AssetHandle<MaterialAsset>>) {
    let count = if spec.class == SpriteClass::LegacyCutout { 1 } else { spec.material_count.max(1) };
    let mut materials = Vec::with_capacity(count as usize);
    let mut first_texture = None;
    for index in 0..count {
        let tint = 40u8.saturating_add((index as u8).saturating_mul(24));
        let texture =
            assets.insert(&format!("tex-{index}"), Texture::solid(4, 4, [tint, 60, 90, 255])).expect("texture");
        if index == 0 {
            first_texture = Some(texture);
        }
        if spec.class != SpriteClass::LegacyCutout {
            let mut material = MaterialAsset::textured(texture).with_alpha_mode(alpha_mode(spec.class)).double_sided();
            material.lit = spec.lit;
            materials.push(assets.insert(&format!("mat-{index}"), material).expect("material"));
        }
    }
    let sprite =
        assets.insert("spr-0", SpriteAsset::whole_texture(first_texture.expect("texture"), 4, 4)).expect("sprite");
    (sprite, materials)
}

fn alpha_mode(class: SpriteClass) -> AlphaMode {
    match class {
        SpriteClass::Opaque | SpriteClass::LegacyCutout => AlphaMode::Opaque,
        SpriteClass::Mask => AlphaMode::mask_default(),
        SpriteClass::Blend => AlphaMode::Blend,
    }
}

fn insert_one(
    assets: &mut AssetManager,
    name: &str,
    albedo: AssetHandle<Texture>,
    alpha: AlphaMode,
) -> AssetHandle<MaterialAsset> {
    let mut material = MaterialAsset::textured(albedo).with_alpha_mode(alpha).double_sided();
    material.lit = true;
    assets.insert(name, material).expect("material")
}

fn insert_materials(
    assets: &mut AssetManager,
    count: u32,
    alpha: AlphaMode,
    lit: bool,
) -> Vec<AssetHandle<MaterialAsset>> {
    (0..count)
        .map(|index| {
            let tint = 30 + (index as u8).saturating_mul(20);
            let texture = assets
                .insert(&format!("mesh-tex-{index}"), Texture::solid(4, 4, [tint, 70, 90, 255]))
                .expect("texture");
            let mut material = MaterialAsset::textured(texture).with_alpha_mode(alpha);
            material.lit = lit;
            assets.insert(&format!("mesh-mat-{index}"), material).expect("material")
        })
        .collect()
}

fn insert_meshes(assets: &mut AssetManager, variety: MeshVariety) -> Vec<AssetHandle<MeshData>> {
    let meshes = match variety {
        MeshVariety::Cuboid => vec![shared_cuboid()],
        MeshVariety::Sphere => vec![dense_sphere()],
        MeshVariety::Eight => vec![
            shared_cuboid(),
            MeshData::cuboid(Vec3::new(0.4, 1.2, 0.4)).expect("cuboid"),
            MeshData::cuboid(Vec3::new(1.2, 0.3, 1.2)).expect("cuboid"),
            MeshData::plane(1.0, 1.0).expect("plane"),
            MeshData::sphere(0.35, 8, 6).expect("sphere"),
            MeshData::cuboid(Vec3::new(0.5, 0.5, 1.5)).expect("cuboid"),
            MeshData::cuboid(Vec3::new(1.0, 0.25, 0.25)).expect("cuboid"),
            MeshData::sphere(0.25, 6, 4).expect("sphere"),
        ],
    };
    meshes
        .into_iter()
        .enumerate()
        .map(|(index, mesh)| assets.insert(&format!("mesh-{index}"), mesh).expect("mesh"))
        .collect()
}

fn shared_cuboid() -> MeshData {
    MeshData::cuboid(Vec3::splat(0.8)).expect("cuboid")
}

fn dense_sphere() -> MeshData {
    MeshData::sphere(0.4, 16, 12).expect("sphere")
}

fn sprite_for(
    spec: &SpriteSpec,
    asset: AssetHandle<SpriteAsset>,
    materials: &[AssetHandle<MaterialAsset>],
    index: u32,
) -> Sprite {
    let orientation = match spec.orientations {
        OrientationMix::Billboard => SpriteOrientation::Billboard,
        OrientationMix::AllThree => match index % 3 {
            0 => SpriteOrientation::Billboard,
            1 => SpriteOrientation::Upright,
            _ => SpriteOrientation::Fixed,
        },
    };
    let mut sprite = Sprite::from_asset(asset).with_size(Vec2::splat(0.8)).with_orientation(orientation);
    if spec.class == SpriteClass::LegacyCutout {
        sprite = sprite.cutout();
        if !spec.lit {
            sprite = sprite.unlit();
        }
        return sprite;
    }
    sprite = sprite.with_material(materials[index as usize % materials.len()]);
    sprite
}

fn spawn_sprite(world: &mut World, transform: Transform, sprite: Sprite, coplanar: bool, index: u32) {
    if coplanar {
        let priority = CoplanarPriority::new((index % 8) as u8).expect("priority");
        world.spawn((transform, sprite, priority));
    } else {
        world.spawn((transform, sprite));
    }
}

fn enable_pixel_snap(built: Built) -> Built {
    let ids: Vec<_> = built.world.query::<&Camera>().iter().map(|(entity, _)| entity).collect();
    for id in ids {
        built.world.component_mut::<Camera>(id).expect("camera").pixel_snap = true;
    }
    built
}

fn add_debug(world: &mut World, segments: u32) {
    if segments == 0 {
        return;
    }
    let mut draw = DebugDraw::default();
    for index in 0..segments {
        let x = index as f32 * 0.01;
        draw.line(Vec3::new(x, 0.0, 0.0), Vec3::new(x, 1.0, 0.0), Color::WHITE);
    }
    world.insert_resource(draw);
}

fn add_point_lights(world: &mut World, count: u32, span: f32) {
    let range = (span * 2.0).max(20.0);
    for index in 0..count {
        let angle = index as f32 * std::f32::consts::TAU / count.max(1) as f32;
        let radius = (span * 0.25).max(1.0);
        world.spawn((
            Light::point(Color::rgb(1.0, 0.95, 0.8), 2.0, range),
            Transform::from_xyz(angle.cos() * radius, 4.0, angle.sin() * radius),
        ));
    }
}

fn add_grid_cameras(world: &mut World, count: u32, layout: CameraLayout, frame: Frame, span: f32) {
    let height = match frame {
        Frame::Fit(Coverage::Mostly) => (span + 2.0).max(4.0) * 1.6,
        Frame::Fit(Coverage::Partial) => (span + 2.0).max(4.0) * 0.22,
        Frame::FixedHeight(height) => height,
    };
    let far = span * 8.0 + height * 4.0 + 200.0;
    for index in 0..count {
        let kind = view_kind(layout, index);
        let mut camera = match kind {
            ViewKind::Perspective => Camera::perspective(50.0),
            ViewKind::Orthographic | ViewKind::Isometric => Camera::orthographic(height),
        };
        camera.projection = camera.projection.with_clip_range(0.05, far);
        camera.order = index as i32;
        world.spawn((camera, grid_pose(kind, index, count, height, span)));
    }
}

fn add_lane_cameras(world: &mut World, count: u32, layout: CameraLayout) {
    for index in 0..count {
        let kind = view_kind(layout, index);
        let mut camera = match kind {
            ViewKind::Perspective => Camera::perspective(50.0),
            ViewKind::Orthographic | ViewKind::Isometric => Camera::orthographic(52.0),
        };
        camera.projection = camera.projection.with_clip_range(0.05, 400.0);
        camera.order = index as i32;
        let yaw = orbit_yaw(kind, index, count);
        let pitch = match kind {
            ViewKind::Perspective => 0.55,
            ViewKind::Isometric => ISO_PITCH,
            ViewKind::Orthographic => 0.7,
        };
        let rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch);
        let forward = Transform::from_rotation(rotation).forward();
        let distance = if matches!(kind, ViewKind::Perspective) { 55.0 } else { 48.0 };
        world.spawn((camera, Transform::from_position(-forward * distance).with_rotation(rotation)));
    }
}

fn add_pair_cameras(world: &mut World, span: f32, target: Option<RenderTargetId>) {
    let height = (span + 2.0).max(4.0) * 1.6;
    let far = span * 8.0 + 200.0;
    let pose = grid_pose(ViewKind::Orthographic, 0, 1, height, span);
    for (order, output) in [(0, target.map(CameraOutput::Target)), (1, None)] {
        let mut camera = Camera::orthographic(height);
        camera.projection = camera.projection.with_clip_range(0.05, far);
        camera.order = order;
        if let Some(output) = output {
            camera = camera.with_output(output);
        }
        world.spawn((camera, pose));
    }
}

fn grid_pose(kind: ViewKind, index: u32, count: u32, height: f32, span: f32) -> Transform {
    let radius = span * 0.75 + 2.0;
    let distance = match kind {
        ViewKind::Perspective => {
            let fov = 50.0_f32.to_radians();
            (height / (2.0 * (fov * 0.5).tan())).max(radius + 1.0)
        }
        ViewKind::Orthographic | ViewKind::Isometric => radius + 2.0,
    };
    let pitch = match kind {
        ViewKind::Perspective => 0.65,
        ViewKind::Isometric => ISO_PITCH,
        ViewKind::Orthographic => 0.55,
    };
    let rotation = Quat::from_rotation_y(orbit_yaw(kind, index, count)) * Quat::from_rotation_x(pitch);
    let forward = Transform::from_rotation(rotation).forward();
    Transform::from_position(-forward * distance).with_rotation(rotation)
}

fn orbit_yaw(kind: ViewKind, index: u32, count: u32) -> f32 {
    let orbit = if count <= 1 { 0.0 } else { index as f32 * std::f32::consts::TAU / count as f32 };
    let base = if matches!(kind, ViewKind::Isometric) { std::f32::consts::FRAC_PI_4 } else { 0.0 };
    base + orbit
}

fn view_kind(layout: CameraLayout, index: u32) -> ViewKind {
    match layout {
        CameraLayout::Orthographic => ViewKind::Orthographic,
        CameraLayout::Perspective => ViewKind::Perspective,
        CameraLayout::Isometric => ViewKind::Isometric,
        CameraLayout::Cycle => match index % 3 {
            0 => ViewKind::Orthographic,
            1 => ViewKind::Perspective,
            _ => ViewKind::Isometric,
        },
    }
}

fn grid_span(count: u32, spacing: f32) -> f32 {
    let side = (count as f32).sqrt().ceil().max(1.0);
    (side - 1.0) * spacing
}

fn grid_x(index: u32, count: u32, spacing: f32) -> f32 {
    let side = (count as f32).sqrt().ceil().max(1.0) as u32;
    let column = index % side;
    (column as f32 - (side - 1) as f32 * 0.5) * spacing
}

fn grid_z(index: u32, count: u32, spacing: f32) -> f32 {
    let side = (count as f32).sqrt().ceil().max(1.0) as u32;
    let row = index / side;
    (row as f32 - (side - 1) as f32 * 0.5) * spacing
}

fn street_x(index: u32, count: u32) -> f32 {
    if count <= 1 {
        return 0.0;
    }
    let t = index as f32 / (count - 1) as f32;
    (t - 0.5) * 40.0
}

fn ground_transform(n: f32) -> Transform {
    let rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    let center = rotation.rotate(Vec3::new(n * 0.5, n * 0.5, 0.0));
    Transform::from_position(-center).with_rotation(rotation)
}

fn mesh_runs(meshes: &[MeshInstance]) -> u32 {
    let mut runs = 0;
    let mut index = 0;
    while index < meshes.len() {
        index = MeshInstance::run_end(meshes, index);
        runs += 1;
    }
    runs
}

fn vec_bytes<T>(values: &Vec<T>) -> u64 {
    values.capacity() as u64 * std::mem::size_of::<T>() as u64
}

fn layer_bytes<T, B>(instances: &Vec<T>, batches: &Vec<B>) -> u64 {
    vec_bytes(instances) + vec_bytes(batches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract_scene;

    fn drawn(spec_world: Built) -> (Built, crate::RenderScene) {
        let scene = extract_scene(&spec_world.world, &spec_world.assets, viewport()).expect("camera");
        (spec_world, scene)
    }

    fn grid(spec: SpriteSpec) -> Summary {
        let (_, scene) = drawn(sprite_grid(spec));
        let summary = summarize(&scene);
        assert_eq!(summary.missing_assets, 0);
        summary
    }

    #[test]
    fn catalog_labels_are_unique_and_cover_the_matrix() {
        let cases = cases();
        let mut labels: Vec<_> = cases.iter().map(|case| case.label).collect();
        let count = labels.len();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), count);
        assert!(cases.iter().any(|case| case.label == "sprite-opaque-100k" && case.gpu));
        assert!(cases.iter().any(|case| case.label == "sprite-blend-100k" && case.gpu));
        assert!(cases.iter().any(|case| case.label == "lane-mixed" && case.gpu));
        assert!(cases.iter().any(|case| case.label == "offscreen-1k" && case.gpu));
        assert!(cases.iter().any(|case| case.label == "sprite-opaque-10k-move" && case.motion));
        assert!(cases.iter().any(|case| case.label == "sprite-cam-cycle-7-10k" && case.gpu));
        assert!(cases.iter().any(|case| case.label == "sprite-opaque-10k-snap" && !case.gpu));
    }

    #[test]
    fn pixel_snap_rows_match_the_unsnapped_scene_counts() {
        let plain =
            summarize(&extract_scene(&sprite_opaque_10k().world, &sprite_opaque_10k().assets, viewport()).unwrap());
        let snapped = sprite_opaque_10k_snap();
        assert!(snapped.world.query::<&Camera>().iter().all(|(_, camera)| camera.pixel_snap));
        let snap = summarize(&extract_scene(&snapped.world, &snapped.assets, viewport()).unwrap());
        assert_eq!(snap.sprite_batches, plain.sprite_batches);
        assert_eq!(snap.visible_sprites, plain.visible_sprites);
        assert_eq!(snap.mesh_draws, plain.mesh_draws);
        assert_eq!(snap.passes, 1);
        let two = sprite_cam_2_10k_snap();
        assert_eq!(two.cameras, 2);
        let summary = summarize(&extract_scene(&two.world, &two.assets, viewport()).unwrap());
        assert_eq!(summary.passes, 2);
        assert!(summary.sprite_batches >= plain.sprite_batches);
    }

    #[test]
    fn reference_sphere_has_more_indices_than_the_cuboid() {
        let (cuboid, sphere) = reference_mesh_indices();
        assert!(cuboid >= 36);
        assert!(sphere > cuboid);
    }

    #[test]
    fn opaque_sprites_share_one_batch_and_coplanar_levels_do_not_split_it() {
        let opaque = grid(SpriteSpec { count: 32, ..SpriteSpec::opaque() });
        assert_eq!(opaque.sprite_batches, 1);
        assert_eq!(opaque.visible_sprites, 32);
        assert_eq!(opaque.culled_sprites, 0);
        let coplanar = grid(SpriteSpec { count: 32, coplanar: true, ..SpriteSpec::opaque() });
        assert_eq!(coplanar.sprite_batches, 1);
        assert_eq!(coplanar.visible_sprites, 32);
    }

    #[test]
    fn eight_opaque_materials_are_eight_batches() {
        let summary = grid(SpriteSpec { count: 32, material_count: 8, ..SpriteSpec::opaque() });
        assert_eq!(summary.sprite_batches, 8);
    }

    #[test]
    fn orientations_stay_in_one_opaque_batch() {
        let summary = grid(SpriteSpec { count: 30, orientations: OrientationMix::AllThree, ..SpriteSpec::opaque() });
        assert_eq!(summary.sprite_batches, 1);
    }

    #[test]
    fn mask_blend_and_legacy_cutout_land_in_their_layers() {
        let (_, mask) = drawn(sprite_grid(SpriteSpec { count: 12, class: SpriteClass::Mask, ..SpriteSpec::opaque() }));
        let pass = mask.sole_pass();
        assert_eq!(pass.mask_sprites.batches.len(), 1);
        assert!(pass.opaque_sprites.instances.is_empty());
        let (_, blend) =
            drawn(sprite_grid(SpriteSpec { count: 12, class: SpriteClass::Blend, ..SpriteSpec::opaque() }));
        let pass = blend.sole_pass();
        assert_eq!(pass.blended_sprites.batches.len(), 1);
        assert_eq!(pass.transparent.len(), 1);
        let (_, cutout) =
            drawn(sprite_grid(SpriteSpec { count: 12, class: SpriteClass::LegacyCutout, ..SpriteSpec::opaque() }));
        let pass = cutout.sole_pass();
        assert_eq!(pass.cutout_sprites.batches.len(), 1);
        assert!(pass.mask_sprites.instances.is_empty());
    }

    #[test]
    fn blended_materials_split_when_depth_order_interleaves_them() {
        let summary =
            grid(SpriteSpec { count: 24, class: SpriteClass::Blend, material_count: 2, ..SpriteSpec::opaque() });
        assert!(summary.sprite_batches > 2, "batches {}", summary.sprite_batches);
        assert_eq!(summary.transparent_batches, summary.sprite_batches);
    }

    #[test]
    fn a_tight_camera_culls_a_sparse_grid() {
        let wide = grid(SpriteSpec { count: 400, ..SpriteSpec::opaque() });
        let tight = grid(SpriteSpec { count: 400, frame: Frame::Fit(Coverage::Partial), ..SpriteSpec::opaque() });
        let sparse = grid(SpriteSpec {
            count: 400,
            spacing: Spacing::Sparse,
            frame: Frame::FixedHeight(8.0),
            ..SpriteSpec::opaque()
        });
        assert_eq!(wide.culled_sprites, 0);
        assert!(tight.culled_sprites > tight.visible_sprites);
        assert!(tight.visible_sprites > 0);
        assert!(sparse.culled_sprites > sparse.visible_sprites);
    }

    #[test]
    fn extra_point_lights_keep_sixteen_and_a_directional_is_kept() {
        let summary = grid(SpriteSpec { count: 8, lights: 32, ..SpriteSpec::opaque() });
        assert_eq!(summary.lights_min, 16);
        assert_eq!(summary.lights_max, 16);
        let mut built = sprite_grid(SpriteSpec { count: 4, lights: 20, ..SpriteSpec::opaque() });
        built.world.spawn((crate::Light::directional(Color::WHITE, 1.0), Transform::from_xyz(0.0, 8.0, 0.0)));
        propagate_transforms(&mut built.world);
        let scene = extract_scene(&built.world, &built.assets, viewport()).expect("camera");
        let lights = &scene.sole_pass().lights;
        assert_eq!(lights.len(), 16);
        assert!(lights.iter().any(|light| matches!(light.kind, crate::LightKind::Directional)));
    }

    #[test]
    fn two_cameras_share_debug_segments_once() {
        let summary = grid(SpriteSpec { count: 6, cameras: 2, debug_segments: 4, ..SpriteSpec::opaque() });
        assert_eq!(summary.passes, 2);
        assert_eq!(summary.debug_segments, 4);
        assert_eq!(summary.visible_sprites, 12);
    }

    #[test]
    fn hierarchy_children_extract_and_repeat_builds_match() {
        let summary = grid(SpriteSpec { count: 40, hierarchy: true, ..SpriteSpec::opaque() });
        assert_eq!(summary.visible_sprites, 40);
        let once = grid(SpriteSpec { count: 24, material_count: 3, ..SpriteSpec::opaque() });
        let twice = grid(SpriteSpec { count: 24, material_count: 3, ..SpriteSpec::opaque() });
        assert_eq!(once, twice);
    }

    #[test]
    fn distinct_meshes_draw_once_per_asset_and_one_asset_draws_once() {
        let shared = summarize(&drawn(mesh_field(MeshSpec { count: 24, ..MeshSpec::shared() })).1);
        assert_eq!(shared.mesh_draws, 1);
        assert_eq!(shared.visible_meshes, 24);
        assert_eq!(shared.missing_assets, 0);
        let distinct =
            summarize(&drawn(mesh_field(MeshSpec { count: 24, variety: MeshVariety::Eight, ..MeshSpec::shared() })).1);
        assert_eq!(distinct.mesh_draws, 8);
        let materials =
            summarize(&drawn(mesh_field(MeshSpec { count: 24, material_count: 8, ..MeshSpec::shared() })).1);
        assert_eq!(materials.mesh_draws, 8);
    }

    #[test]
    fn the_lane_mixes_tiles_sprite_classes_and_meshes() {
        let (_, scene) = drawn(lane(LaneSpec::mixed()));
        let summary = summarize(&scene);
        assert_eq!(summary.missing_assets, 0);
        assert!(summary.tiles_visited > 0);
        assert!(summary.chunks_tested > 0);
        let pass = scene.sole_pass();
        assert!(!pass.opaque_sprites.instances.is_empty());
        assert!(!pass.mask_sprites.instances.is_empty());
        assert!(!pass.blended_sprites.instances.is_empty());
        assert!(!pass.meshes.is_empty());
        assert_eq!(summary.lights_min, 4);
        let sprites_only = summarize(&drawn(lane(LaneSpec { meshes: false, ..LaneSpec::mixed() })).1);
        assert_eq!(sprites_only.visible_meshes, 0);
        assert!(sprites_only.tiles_visited > 0);
    }

    #[test]
    fn offscreen_and_surface_pairs_keep_two_passes() {
        let (_, offscreen) = drawn(camera_pair(true));
        let summary = summarize(&offscreen);
        assert_eq!(summary.passes, 2);
        assert_eq!(offscreen.targets.len(), 1);
        assert_eq!(summary.missing_targets, 1);
        assert!(offscreen.passes.iter().any(|pass| matches!(pass.output, CameraOutput::Target(_))));
        let samples_target = offscreen
            .passes
            .iter()
            .flat_map(|pass| {
                pass.opaque_sprites
                    .instances
                    .iter()
                    .chain(pass.mask_sprites.instances.iter())
                    .chain(pass.blended_sprites.instances.iter())
            })
            .any(|sprite| sprite.render_target.is_some());
        assert!(samples_target);
        let (_, surface) = drawn(camera_pair(false));
        let summary = summarize(&surface);
        assert_eq!(summary.passes, 2);
        assert!(surface.targets.is_empty());
        assert_eq!(summary.missing_targets, 0);
    }
}
