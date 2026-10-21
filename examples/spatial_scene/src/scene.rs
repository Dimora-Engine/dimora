//! Two code-defined environments in one world, seen through the same camera configurations.
//!
//! Environment A is the original scene: ground, sprites at several depths, overlapping and coplanar
//! content, and meshes. Environment B is a smaller yard in the same view. The camera and the walker
//! are runtime entities. They are not members of either environment, so a replacement leaves them in
//! place. A is unlit. B adds one point light on a lit mesh and keeps its sprites unlit.
//!
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

/// Gameplay action bound to Tab. One press queues one replacement.
pub const SWITCH_ENVIRONMENT: &str = "switch_environment";

/// Which code-defined environment is published. The camera and the walker are not members of either.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Environment {
    /// The original spatial scene.
    A,
    /// The smaller yard used to show a scene switch.
    B,
}

impl Environment {
    pub fn other(self) -> Self {
        match self {
            Environment::A => Environment::B,
            Environment::B => Environment::A,
        }
    }
}

pub struct Scene {
    pub camera: Entity,
    pub walker: Entity,
    /// The spinning cube while environment A is published.
    pub cube: Option<Entity>,
    /// Everything whose sprite orientation the `O` key cycles.
    pub figures: Vec<(char, Entity)>,
    pub rig: CameraRig,
    pub projection: CameraProjection,
    /// Index into [`NAMED`].
    pub config: usize,
    pub figure_orientation: SpriteOrientation,
    pub title_timer: f32,
    /// Published environment. It changes only after a replacement reports success.
    pub environment: Environment,
    /// Instance id of [`Scene::environment`]. Not the camera or the walker.
    pub instance: SceneInstanceId,
    /// A replacement that has been queued and not yet dismissed.
    pub pending: Option<SceneOpId>,
}

impl Scene {
    pub fn config_name(&self) -> &'static str {
        NAMED[self.config].0
    }
}

/// Builds environment A, the persistent camera, and the persistent walker.
///
/// Mesh assets are registered once here. Loading either environment again does not insert them
/// again. The camera and the walker are ordinary runtime entities: they are not scene members.
/// `config` is an index into [`NAMED`]. Sprite and panel assets must already be registered with
/// [`crate::art::register`].
pub fn build_world(world: &mut World, config: usize) -> Scene {
    add_meshes(world);
    let instance = world.load_scene(build_environment_a).expect("environment A is a fixed build");
    let (camera, transform) = NAMED[config].1();
    let projection = camera.projection;
    let rig = CameraRig::from_transform(&transform);
    let camera = world.spawn((camera, transform, Name::new("camera")));
    let walker =
        world.spawn((Transform::from_xyz(0.0, 0.0, WALK_Z), figure(Color::rgb(0.95, 0.55, 0.95)), Name::new("walker")));
    let mut scene = Scene {
        camera,
        walker,
        cube: None,
        figures: Vec::new(),
        rig,
        projection,
        config,
        figure_orientation: SpriteOrientation::Billboard,
        title_timer: 0.0,
        environment: Environment::A,
        instance,
        pending: None,
    };
    refresh_tracked(world, &mut scene);
    scene
}

/// The original environment: ground, figures, meshes, and the coplanar bench.
///
/// The camera and the walker are not created here, so replacing this environment leaves them in place.
pub fn build_environment_a(builder: &mut SceneBuilder<'_>) -> Result<(), SceneError> {
    add_ground(builder);
    add_content(builder);
    add_overlap_and_coplanar_bench(builder);
    Ok(())
}

/// A small second environment in the same view, using the assets environment A already registered.
pub fn build_environment_b(builder: &mut SceneBuilder<'_>) -> Result<(), SceneError> {
    let (light, dark) = (Color::rgb(0.35, 0.55, 0.42), Color::rgb(0.20, 0.32, 0.28));
    for (i, (x, z)) in [(-50.0, 250.0), (50.0, 250.0), (-50.0, 350.0), (50.0, 350.0)].into_iter().enumerate() {
        let color = if i % 2 == 0 { light } else { dark };
        builder.spawn((
            Transform::from_xyz(x, -4.0, z),
            Mesh::new("floor_tile").with_material(panel(color)),
            Name::new(format!("yard floor {i}")),
        ));
    }
    builder.spawn((
        Transform::from_xyz(0.0, 40.0, 300.0),
        Mesh::new("cube").with_material(Material {
            base_color: Color::rgb(0.95, 0.45, 0.20),
            base_texture: Some(AssetHandle::from_name("panel")),
            lit: true,
        }),
        Name::new("yard cube"),
    ));
    builder.spawn((
        Transform::from_xyz(0.0, 100.0, 200.0),
        Mesh::new("pillar").with_material(panel(Color::rgb(0.25, 0.45, 0.85))),
        Name::new("yard pillar"),
    ));
    builder.spawn((
        Transform::from_xyz(-90.0, 0.0, 260.0),
        figure(Color::rgb(0.95, 0.25, 0.55)),
        Name::new("yard figure"),
    ));
    builder.spawn((Transform::from_xyz(120.0, 0.0, 220.0), tree(), Name::new("yard tree")));
    builder.spawn((
        Transform::from_xyz(0.0, 180.0, 300.0),
        Light::point(Color::rgb(1.0, 0.85, 0.55), 4.0, 600.0),
        Name::new("yard light"),
    ));
    Ok(())
}

fn figure(tint: Color) -> Sprite {
    Sprite::new("figure").with_size(FIGURE_SIZE).with_tint(tint).cutout().unlit()
}

fn tree() -> Sprite {
    Sprite::new("tree").with_size(Vec2::new(120.0, 160.0)).cutout().upright().unlit()
}

fn named(world: &World, name: &str) -> Option<Entity> {
    world.find_by_name(name).into_iter().next()
}

fn refresh_tracked(world: &World, scene: &mut Scene) {
    let mut figures = Vec::new();
    for (label, name) in [('A', "figure A"), ('B', "figure B"), ('C', "figure C"), ('D', "figure D")] {
        if let Some(entity) = named(world, name) {
            figures.push((label, entity));
        }
    }
    figures.push(('W', scene.walker));
    for (label, name) in [('E', "overlap front"), ('F', "overlap back")] {
        if let Some(entity) = named(world, name) {
            figures.push((label, entity));
        }
    }
    scene.figures = figures;
    scene.cube = named(world, "cube");
}

fn panel(color: Color) -> Material {
    Material { base_color: color, base_texture: Some(AssetHandle::from_name("panel")), lit: false }
}

fn add_meshes(world: &mut World) {
    let mut assets = world.resource_mut::<AssetManager>();
    let mut add = |name: &str, mesh: MeshData| {
        assets.insert(name, mesh).expect("example asset names are distinct");
    };
    add(
        "floor_tile",
        MeshData::cuboid(Vec3::new(100.0, 8.0, 100.0)).expect("cuboid dimensions are finite and positive"),
    );
    add("pad", MeshData::cuboid(Vec3::new(84.0, 2.0, 84.0)).expect("cuboid dimensions are finite and positive"));
    add("pillar", MeshData::cuboid(Vec3::new(50.0, 200.0, 50.0)).expect("cuboid dimensions are finite and positive"));
    add("cube", MeshData::cuboid(Vec3::splat(80.0)).expect("cuboid dimensions are finite and positive"));
    add("pyramid", MeshData::cone(60.0, 130.0, 4));
}

/// The ground: a checkerboard of real slabs, top face at Y = 0 (`+Y` up, `XZ` ground).
fn add_ground(builder: &mut SceneBuilder<'_>) {
    let (light, dark) = (Color::rgb(0.80, 0.83, 0.86), Color::rgb(0.52, 0.57, 0.64));
    for i in 0..24 {
        for j in 0..26 {
            let (x, z) = (-1150.0 + i as f32 * 100.0, -950.0 + j as f32 * 100.0);
            let color = if (i + j) % 2 == 0 { light } else { dark };
            builder.spawn((Transform::from_xyz(x, -4.0, z), Mesh::new("floor_tile").with_material(panel(color))));
        }
    }
}

fn add_content(builder: &mut SceneBuilder<'_>) {
    // A colored pad under each figure, so its depth can be read off the floor.
    for (label, pos, tint) in FIGURES {
        let color = Color::rgb(tint[0], tint[1], tint[2]);
        builder.spawn((
            Transform::from_xyz(pos.x, 1.0, pos.z),
            Mesh::new("pad").with_material(panel(color)),
            Name::new(format!("pad {label}")),
        ));
    }

    // Two feature meshes. The pillar sits between figures B (in front) and C (behind).
    builder.spawn((
        Transform::from_xyz(0.0, 100.0, 200.0),
        Mesh::new("pillar").with_material(panel(Color::rgb(0.88, 0.52, 0.34))),
        Name::new("pillar"),
    ));
    builder.spawn((
        Transform::from_xyz(-150.0, 70.0, 400.0),
        Mesh::new("cube").with_material(panel(Color::rgb(0.45, 0.78, 0.82))),
        Name::new("cube"),
    ));
    builder.spawn((
        Transform::from_xyz(210.0, 0.0, 330.0).with_rotation(Quat::from_rotation_y(core::f32::consts::FRAC_PI_4)),
        Mesh::new("pyramid").with_material(panel(Color::rgb(0.92, 0.80, 0.40))),
    ));

    // Cutout figures at Z = 0 / 100 / 300 / 600. The walker is a runtime entity, spawned outside this build.
    for (label, pos, tint) in FIGURES {
        builder.spawn((
            Transform::from_position(pos),
            figure(Color::rgb(tint[0], tint[1], tint[2])),
            Name::new(format!("figure {label}")),
        ));
    }

    // Two cutout figures one unit apart in depth, overlapping on screen.
    for (name, x, z, tint) in
        [("overlap front", 80.0, 520.0, [0.9, 0.3, 0.3]), ("overlap back", 100.0, 521.0, [0.3, 0.3, 0.9])]
    {
        builder.spawn((Transform::from_xyz(x, 0.0, z), figure(Color::rgb(tint[0], tint[1], tint[2])), Name::new(name)));
    }

    // Cutout foliage: single trees as depth scale, plus two that overlap at nearly equal depth.
    for (i, (x, z)) in
        [(-320.0, 160.0), (300.0, 60.0), (-150.0, 400.0), (360.0, 520.0), (-340.0, 720.0), (250.0, 860.0)]
            .into_iter()
            .enumerate()
    {
        builder.spawn((Transform::from_xyz(x, 0.0, z), tree(), Name::new(format!("tree {i}"))));
    }
    for (name, x, z) in [("foliage near", -60.0, 640.0), ("foliage far", -20.0, 650.0)] {
        builder.spawn((Transform::from_xyz(x, 0.0, z), tree(), Name::new(name)));
    }

    // Alpha-blended ghost, drawn after all opaque geometry, back to front, and still depth-tested.
    builder.spawn((
        Transform::from_xyz(110.0, 0.0, 200.0),
        ghost(Color { r: 0.75, g: 0.8, b: 1.0, a: 0.5 }),
        Name::new("ghost"),
    ));
}

fn ghost(tint: Color) -> Sprite {
    Sprite::new("figure").with_size(FIGURE_SIZE).with_tint(tint).unlit()
}

/// Content whose ordering depends on more than depth, kept together so a test can look at each case alone.
///
/// * Two blended ghosts overlapping at exactly equal depth, and a second pair that differs only by
///   `RenderOrder::order`.
/// * Flat cutout decals lying on the ground ([`flat`]): one coplanar with the slab tops, two coplanar with
///   each other, two a little apart in height.
/// * A flat blended decal coplanar with a flat cutout one, and another a little above it.
fn add_overlap_and_coplanar_bench(builder: &mut SceneBuilder<'_>) {
    let (blue, red) = (Color { r: 0.2, g: 0.3, b: 1.0, a: 0.5 }, Color { r: 1.0, g: 0.2, b: 0.2, a: 0.5 });
    builder.spawn((Transform::from_xyz(-360.0, 0.0, 300.0), ghost(blue), Name::new("ghost pair a")));
    builder.spawn((Transform::from_xyz(-330.0, 0.0, 300.0), ghost(red), Name::new("ghost pair b")));
    builder.spawn((
        Transform::from_xyz(-360.0, 0.0, 380.0),
        ghost(blue),
        RenderOrder::new(0, 0),
        Name::new("ghost ordered a"),
    ));
    builder.spawn((
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
    builder.spawn((flat(Vec3::new(x0, 0.0, 120.0)), cutout([1.0, 0.1, 0.1]), Name::new("decal on slab")));
    // B: two cutout decals at exactly the same height.
    builder.spawn((flat(Vec3::new(x0, lift, 250.0)), cutout([1.0, 0.1, 0.1]), Name::new("decal equal a")));
    builder.spawn((flat(Vec3::new(x1, lift, 250.0)), cutout([0.1, 0.8, 0.1]), Name::new("decal equal b")));
    // C: two cutout decals one unit apart in height.
    builder.spawn((flat(Vec3::new(x0, lift, 370.0)), cutout([1.0, 0.9, 0.1]), Name::new("decal stack low")));
    builder.spawn((flat(Vec3::new(x1, lift + 1.0, 370.0)), cutout([0.1, 0.2, 1.0]), Name::new("decal stack high")));
    // D: a blended decal at the same height as a cutout one.
    builder.spawn((flat(Vec3::new(x0, lift, 490.0)), cutout([1.0, 0.5, 0.1]), Name::new("decal cutout under equal")));
    builder.spawn((
        flat(Vec3::new(x1, lift, 490.0)),
        decal(Color { r: 0.0, g: 0.9, b: 0.9, a: 0.6 }),
        Name::new("decal blended equal"),
    ));
    // E: a blended decal above a cutout one.
    builder.spawn((flat(Vec3::new(x0, lift, 610.0)), cutout([1.0, 0.5, 0.1]), Name::new("decal cutout under above")));
    builder.spawn((
        flat(Vec3::new(x1, lift + 1.0, 610.0)),
        decal(Color { r: 0.0, g: 0.9, b: 0.9, a: 0.6 }),
        Name::new("decal blended above"),
    ));
}

/// A transform that lays a `Fixed` sprite (a quad in its local XY plane) flat on the ground at `position`.
pub fn flat(position: Vec3) -> Transform {
    Transform::from_position(position).with_rotation(Quat::from_rotation_x(core::f32::consts::FRAC_PI_2))
}

/// Per-frame motion: the cube spins while environment A is published, and the walker patrols.
///
/// Both are plain `Transform` writes. The walker is not a scene member, so it keeps moving after a
/// replacement. Environment B has no entity named `cube`.
pub fn motion_system(world: &mut World) {
    let t = world.resource::<Time>().elapsed_secs();
    let (cube, walker) = {
        let scene = world.resource::<Scene>();
        (scene.cube, scene.walker)
    };
    if let Some(cube) = cube {
        if let Ok(mut transform) = world.component_mut::<Transform>(cube) {
            transform.rotation = Quat::from_rotation_y(t * 0.7) * Quat::from_rotation_x(0.4);
        }
    }
    if let Ok(mut transform) = world.component_mut::<Transform>(walker) {
        transform.position = Vec3::new(WALK_RANGE * (t * 0.8).sin(), 0.0, WALK_Z);
    }
}

/// Publishes `environment` before the first step.
///
/// Gameplay uses [`request_environment_switch`] so the world stays unchanged until the command
/// flush. This path is for a launch that should open on one environment, including a headless
/// screenshot. It does not queue a second copy of the camera or the walker.
pub fn replace_environment(world: &mut World, scene: &mut Scene, environment: Environment) -> Result<(), SceneError> {
    if scene.environment == environment {
        return Ok(());
    }
    if let Some(operation) = scene.pending {
        return Err(SceneError::OperationPending { operation });
    }
    let build: fn(&mut SceneBuilder<'_>) -> Result<(), SceneError> = match environment {
        Environment::A => build_environment_a,
        Environment::B => build_environment_b,
    };
    let instance = world.replace_scene(scene.instance, build)?;
    scene.instance = instance;
    scene.environment = environment;
    refresh_tracked(world, scene);
    Ok(())
}

/// Queues a replacement for the other environment when [`SWITCH_ENVIRONMENT`] was pressed.
///
/// The press is the frame-scoped action edge, so a held key queues one replacement. A press while
/// `pending` is set queues nothing. The active instance id stays as it is until
/// [`apply_environment_switch`] sees [`SceneOpStatus::Replaced`].
pub fn request_environment_switch(world: &mut World) {
    if !world.resource::<Actions>().pressed(SWITCH_ENVIRONMENT) {
        return;
    }
    let scene = world.resource::<Scene>();
    if scene.pending.is_some() {
        return;
    }
    let instance = scene.instance;
    let destination = scene.environment.other();
    drop(scene);
    let build: fn(&mut SceneBuilder<'_>) -> Result<(), SceneError> = match destination {
        Environment::A => build_environment_a,
        Environment::B => build_environment_b,
    };
    match world.commands().replace_scene(instance, build) {
        Ok(operation) => world.resource_mut::<Scene>().pending = Some(operation),
        Err(error) => log::error!("scene switch was not queued: {error}"),
    }
}

/// Reads the queued switch after the stage flush and dismisses a completed record.
///
/// [`SceneOpStatus::Pending`] stays stored. [`SceneOpStatus::Replaced`] becomes the active
/// instance. [`SceneOpStatus::Failed`] leaves that instance as it was; the engine already logged
/// the failure when the status was stored. [`SceneOpStatus::Loaded`] and
/// [`SceneOpStatus::Unloaded`] are not results this switch requested. They are logged and the
/// operation record is dismissed. A published instance from an unexpected load stays published:
/// this switch did not build it and does not unload it. An unexpected unload is left as the
/// engine applied it; this switch does not rebuild that instance. The tracked environment id
/// changes only for [`SceneOpStatus::Replaced`].
pub fn apply_environment_switch(world: &mut World) {
    let Some(operation) = world.resource::<Scene>().pending else {
        return;
    };
    let status = match world.scene_operation(operation) {
        Ok(status) => status,
        Err(error) => {
            log::error!("scene switch {operation} could not be read: {error}");
            world.resource_mut::<Scene>().pending = None;
            return;
        }
    };
    match status {
        SceneOpStatus::Pending => {}
        SceneOpStatus::Replaced { new, .. } => {
            dismiss_switch(world, operation);
            {
                let mut scene = world.resource_mut::<Scene>();
                scene.pending = None;
                scene.instance = new;
                scene.environment = scene.environment.other();
            }
            retarget_tracked(world);
        }
        SceneOpStatus::Failed { .. } => {
            dismiss_switch(world, operation);
            world.resource_mut::<Scene>().pending = None;
        }
        SceneOpStatus::Loaded { instance } => {
            let environment = world.resource::<Scene>().environment;
            log::error!(
                "scene switch {operation} published instance {instance}, which this switch does not own; leaving it published and keeping {environment:?}"
            );
            dismiss_switch(world, operation);
            world.resource_mut::<Scene>().pending = None;
        }
        SceneOpStatus::Unloaded { instance } => {
            let scene = world.resource::<Scene>();
            let (environment, active) = (scene.environment, scene.instance);
            drop(scene);
            log::error!(
                "scene switch {operation} unloaded instance {instance}, which this switch does not own; leaving the world as that unload left it and keeping {environment:?} ({active})"
            );
            dismiss_switch(world, operation);
            world.resource_mut::<Scene>().pending = None;
        }
    }
}

fn dismiss_switch(world: &mut World, operation: SceneOpId) {
    if let Err(error) = world.dismiss_scene_operation(operation) {
        log::error!("scene switch {operation} could not be dismissed: {error}");
    }
}

fn retarget_tracked(world: &mut World) {
    let mut scene = world.remove_resource::<Scene>().expect("scene");
    refresh_tracked(world, &mut scene);
    let orientation = scene.figure_orientation;
    let figures = scene.figures.clone();
    world.insert_resource(scene);
    for (_, entity) in figures {
        if let Ok(mut sprite) = world.component_mut::<Sprite>(entity) {
            sprite.orientation = orientation;
        }
    }
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
