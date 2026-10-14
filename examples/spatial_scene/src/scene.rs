//! The world of the test scene: ground, sprites at several depths, overlapping and coplanar content, meshes
//! and something that moves. It is the same world under every camera configuration.
//!
//! Nothing here is specific to a visual style. Everything is unlit, so what shows is spatial projection.
//! Entities that tests look up have a [`Name`].

use engine::prelude::*;

use crate::cameras::{self, CameraRig, NAMED};

pub const FIGURE_SIZE: Vec2 = Vec2::new(64.0, 96.0);

/// The four figures: label, world position and tint. Z is the only thing that differs in depth.
pub const FIGURES: [(char, Vec3, [f32; 3]); 4] = [
    ('A', Vec3::new(-170.0, 0.0, 0.0), [1.00, 0.45, 0.42]),
    ('B', Vec3::new(-18.0, 0.0, 100.0), [1.00, 0.85, 0.30]),
    ('C', Vec3::new(34.0, 0.0, 300.0), [0.45, 0.88, 0.50]),
    ('D', Vec3::new(170.0, 0.0, 600.0), [0.50, 0.68, 1.00]),
];

/// Where the walker patrols: along X at this depth, between `-WALK_RANGE` and `WALK_RANGE`.
pub const WALK_Z: f32 = -150.0;
pub const WALK_RANGE: f32 = 300.0;

/// Height above the floor slabs of the coplanar bench, small enough to be one surface to the eye.
pub const DECAL_LIFT: f32 = 0.5;

pub struct Scene {
    pub camera: Entity,
    pub cube: Entity,
    pub walker: Entity,
    /// Everything whose sprite orientation the `O` key cycles.
    pub figures: Vec<(char, Entity)>,
    pub rig: CameraRig,
    pub projection: CameraProjection,
    /// Index into [`NAMED`].
    pub config: usize,
    pub figure_orientation: SpriteOrientation,
    pub title_timer: f32,
}

impl Scene {
    pub fn config_name(&self) -> &'static str {
        NAMED[self.config].0
    }
}

/// Builds the whole scene and a camera in configuration `config` (an index into [`NAMED`]). The assets must
/// already be registered with [`crate::art::register`].
pub fn build_world(world: &mut World, config: usize) -> Scene {
    add_meshes(world);
    add_ground(world);
    let (cube, walker, figures) = add_content(world);
    add_overlap_and_coplanar_bench(world);

    let (camera, transform) = NAMED[config].1();
    let projection = camera.projection;
    let rig = CameraRig::from_transform(&transform);
    let camera = world.spawn((camera, transform, Name::new("camera")));
    Scene {
        camera,
        cube,
        walker,
        figures,
        rig,
        projection,
        config,
        figure_orientation: SpriteOrientation::Billboard,
        title_timer: 0.0,
    }
}

fn panel(color: Color) -> Material {
    Material { base_color: color, base_texture: Some(AssetHandle::from_name("panel")), lit: false }
}

fn add_meshes(world: &mut World) {
    let mut assets = world.resource_mut::<AssetManager>();
    let mut add = |name: &str, mesh: MeshData| {
        assets.insert(name, mesh).expect("example asset names are distinct");
    };
    add("floor_tile", MeshData::cuboid(Vec3::new(100.0, 8.0, 100.0)));
    add("pad", MeshData::cuboid(Vec3::new(84.0, 2.0, 84.0)));
    add("pillar", MeshData::cuboid(Vec3::new(50.0, 200.0, 50.0)));
    add("cube", MeshData::cuboid(Vec3::splat(80.0)));
    add("pyramid", MeshData::cone(60.0, 130.0, 4));
}

/// The ground: a checkerboard of real slabs, top face at Y = 0 (`+Y` up, `XZ` ground).
fn add_ground(world: &mut World) {
    let (light, dark) = (Color::rgb(0.80, 0.83, 0.86), Color::rgb(0.52, 0.57, 0.64));
    for i in 0..24 {
        for j in 0..26 {
            let (x, z) = (-1150.0 + i as f32 * 100.0, -950.0 + j as f32 * 100.0);
            let color = if (i + j) % 2 == 0 { light } else { dark };
            world.spawn((Transform::from_xyz(x, -4.0, z), Mesh::new("floor_tile").with_material(panel(color))));
        }
    }
}

fn add_content(world: &mut World) -> (Entity, Entity, Vec<(char, Entity)>) {
    // A colored pad under each figure, so its depth can be read off the floor.
    for (label, pos, tint) in FIGURES {
        let color = Color::rgb(tint[0], tint[1], tint[2]);
        world.spawn((
            Transform::from_xyz(pos.x, 1.0, pos.z),
            Mesh::new("pad").with_material(panel(color)),
            Name::new(format!("pad {label}")),
        ));
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

    // Cutout figures at Z = 0 / 100 / 300 / 600.
    let figure = |tint: Color| Sprite::new("figure").with_size(FIGURE_SIZE).with_tint(tint).cutout().unlit();
    let mut figures: Vec<(char, Entity)> = FIGURES
        .iter()
        .map(|&(label, pos, t)| {
            let sprite = figure(Color::rgb(t[0], t[1], t[2]));
            (label, world.spawn((Transform::from_position(pos), sprite, Name::new(format!("figure {label}")))))
        })
        .collect();

    // A figure that patrols along X: a runtime transform update on a sprite (see `motion_system`).
    let walker =
        world.spawn((Transform::from_xyz(0.0, 0.0, WALK_Z), figure(Color::rgb(0.95, 0.55, 0.95)), Name::new("walker")));
    figures.push(('W', walker));

    // Two cutout figures one unit apart in depth, overlapping on screen.
    for (name, x, z, tint) in
        [("overlap front", 80.0, 520.0, [0.9, 0.3, 0.3]), ("overlap back", 100.0, 521.0, [0.3, 0.3, 0.9])]
    {
        let e = world.spawn((
            Transform::from_xyz(x, 0.0, z),
            figure(Color::rgb(tint[0], tint[1], tint[2])),
            Name::new(name),
        ));
        figures.push((if name.ends_with("front") { 'E' } else { 'F' }, e));
    }

    // Cutout foliage: single trees as depth scale, plus two that overlap at nearly equal depth.
    let tree = || Sprite::new("tree").with_size(Vec2::new(120.0, 160.0)).cutout().upright().unlit();
    for (i, (x, z)) in
        [(-320.0, 160.0), (300.0, 60.0), (-150.0, 400.0), (360.0, 520.0), (-340.0, 720.0), (250.0, 860.0)]
            .into_iter()
            .enumerate()
    {
        world.spawn((Transform::from_xyz(x, 0.0, z), tree(), Name::new(format!("tree {i}"))));
    }
    for (name, x, z) in [("foliage near", -60.0, 640.0), ("foliage far", -20.0, 650.0)] {
        world.spawn((Transform::from_xyz(x, 0.0, z), tree(), Name::new(name)));
    }

    // Alpha-blended ghost, drawn after all opaque geometry, back to front, and still depth-tested.
    let ghost = |tint: Color| Sprite::new("figure").with_size(FIGURE_SIZE).with_tint(tint).unlit();
    world.spawn((
        Transform::from_xyz(110.0, 0.0, 200.0),
        ghost(Color { r: 0.75, g: 0.8, b: 1.0, a: 0.5 }),
        Name::new("ghost"),
    ));

    (cube, walker, figures)
}

/// Content whose ordering depends on more than depth, kept together so a test can look at each case alone.
///
/// * Two blended ghosts overlapping at exactly equal depth, and a second pair that differs only by
///   `RenderOrder::order`.
/// * Flat cutout decals lying on the ground ([`flat`]): one coplanar with the slab tops, two coplanar with
///   each other, two a little apart in height.
/// * A flat blended decal coplanar with a flat cutout one, and another a little above it.
fn add_overlap_and_coplanar_bench(world: &mut World) {
    let ghost = |tint: Color| Sprite::new("figure").with_size(FIGURE_SIZE).with_tint(tint).unlit();
    let (blue, red) = (Color { r: 0.2, g: 0.3, b: 1.0, a: 0.5 }, Color { r: 1.0, g: 0.2, b: 0.2, a: 0.5 });
    world.spawn((Transform::from_xyz(-360.0, 0.0, 300.0), ghost(blue), Name::new("ghost pair a")));
    world.spawn((Transform::from_xyz(-330.0, 0.0, 300.0), ghost(red), Name::new("ghost pair b")));
    world.spawn((
        Transform::from_xyz(-360.0, 0.0, 380.0),
        ghost(blue),
        RenderOrder::new(0, 0),
        Name::new("ghost ordered a"),
    ));
    world.spawn((
        Transform::from_xyz(-330.0, 0.0, 380.0),
        ghost(red),
        RenderOrder::new(0, 1),
        Name::new("ghost ordered b"),
    ));

    let decal = |tint: Color| {
        Sprite::new("swatch").with_size(Vec2::splat(70.0)).with_anchor(Vec2::splat(0.5)).with_tint(tint).fixed().unlit()
    };
    let cutout = |c: [f32; 3]| decal(Color::rgb(c[0], c[1], c[2])).cutout();
    let (x0, x1, lift) = (-560.0, -530.0, DECAL_LIFT);
    // A: coplanar with the floor slabs' top faces (a mesh).
    world.spawn((flat(Vec3::new(x0, 0.0, 120.0)), cutout([1.0, 0.1, 0.1]), Name::new("decal on slab")));
    // B: two cutout decals at exactly the same height.
    world.spawn((flat(Vec3::new(x0, lift, 250.0)), cutout([1.0, 0.1, 0.1]), Name::new("decal equal a")));
    world.spawn((flat(Vec3::new(x1, lift, 250.0)), cutout([0.1, 0.8, 0.1]), Name::new("decal equal b")));
    // C: two cutout decals one unit apart in height.
    world.spawn((flat(Vec3::new(x0, lift, 370.0)), cutout([1.0, 0.9, 0.1]), Name::new("decal stack low")));
    world.spawn((flat(Vec3::new(x1, lift + 1.0, 370.0)), cutout([0.1, 0.2, 1.0]), Name::new("decal stack high")));
    // D: a blended decal at the same height as a cutout one.
    world.spawn((flat(Vec3::new(x0, lift, 490.0)), cutout([1.0, 0.5, 0.1]), Name::new("decal cutout under equal")));
    world.spawn((
        flat(Vec3::new(x1, lift, 490.0)),
        decal(Color { r: 0.0, g: 0.9, b: 0.9, a: 0.6 }),
        Name::new("decal blended equal"),
    ));
    // E: a blended decal above a cutout one.
    world.spawn((flat(Vec3::new(x0, lift, 610.0)), cutout([1.0, 0.5, 0.1]), Name::new("decal cutout under above")));
    world.spawn((
        flat(Vec3::new(x1, lift + 1.0, 610.0)),
        decal(Color { r: 0.0, g: 0.9, b: 0.9, a: 0.6 }),
        Name::new("decal blended above"),
    ));
}

/// A transform that lays a `Fixed` sprite (a quad in its local XY plane) flat on the ground at `position`.
pub fn flat(position: Vec3) -> Transform {
    Transform::from_position(position).with_rotation(Quat::from_rotation_x(core::f32::consts::FRAC_PI_2))
}

/// Per-frame motion: the cube spins and the walker patrols. Both are plain `Transform` writes.
pub fn motion_system(world: &mut World) {
    let t = world.resource::<Time>().elapsed_secs();
    let (cube, walker) = {
        let scene = world.resource::<Scene>();
        (scene.cube, scene.walker)
    };
    world.component_mut::<Transform>(cube).unwrap().rotation =
        Quat::from_rotation_y(t * 0.7) * Quat::from_rotation_x(0.4);
    world.component_mut::<Transform>(walker).unwrap().position = Vec3::new(WALK_RANGE * (t * 0.8).sin(), 0.0, WALK_Z);
}

/// Writes the rig and projection onto the camera entity.
pub fn camera_system(world: &mut World) {
    let scene = world.resource::<Scene>();
    let (camera, rig, projection) = (scene.camera, scene.rig, scene.projection);
    drop(scene);
    *world.component_mut::<Transform>(camera).unwrap() = rig.transform();
    world.component_mut::<Camera>(camera).unwrap().projection = projection;
}

/// Switches the scene's camera to configuration `index` (wrapping), keeping the same world.
pub fn select_config(scene: &mut Scene, index: usize) {
    scene.config = index % NAMED.len();
    let (camera, transform) = NAMED[scene.config].1();
    scene.rig = CameraRig::from_transform(&transform);
    scene.projection = camera.projection;
}

pub use cameras::{FOV_DEGREES, ORTHO_HEIGHT};
