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

/// Full size of the pillar mesh. The mesh is centered on [`PILLAR_CENTER`], so the occupied box is
/// that center ± half of this size: X −25..25, Y 0..200, Z 175..225.
pub const PILLAR_SIZE: Vec3 = Vec3::new(50.0, 200.0, 50.0);
/// World center of the pillar mesh. Shared by environment A, the yard pillar mesh, and the solid collider.
pub const PILLAR_CENTER: Vec3 = Vec3::new(0.0, 100.0, 200.0);

/// Horizontal radius of the `--collision` capsule, in world units. This is not the 64×96 sprite.
pub const BODY_RADIUS: f32 = 12.0;
/// Half-length of the capsule's vertical segment. Total height is `2 * (half_height + radius)` = 80.
pub const BODY_HALF_HEIGHT: f32 = 28.0;
/// Local center of the capsule. `Capsule::new_y` extends `half_height + radius` from this point, so a
/// gameplay root at Y = 0 is the feet and the capsule stands on that ground.
pub const BODY_OFFSET: Vec3 = Vec3::new(0.0, BODY_HALF_HEIGHT + BODY_RADIUS, 0.0);
/// Controlled-character speed in world units per second. Diagonal input is normalized before this scale.
pub const WALK_SPEED: f32 = 90.0;
/// Scripted second character, world units per second along X.
pub const PACE_SPEED: f32 = 70.0;
/// Feet of the controlled character. Used only by `--collision`; the patrol stays at [`WALK_Z`].
pub const CHARACTER_START: Vec3 = Vec3::new(0.0, 0.0, 100.0);
/// Feet of the scripted character, behind the pillar (the pillar ends at Z = 225).
pub const PACE_START: Vec3 = Vec3::new(0.0, 0.0, 250.0);

/// Sensor slab in front of the pillar. Z is 129.5..130.5. A +Z stop against the pillar leaves the
/// capsule's trailing edge near Z = 151, so the body clears this slab before it is blocked.
pub const SENSOR_CENTER: Vec3 = Vec3::new(0.0, 40.0, 130.0);
pub const SENSOR_HALF: Vec3 = Vec3::new(30.0, 50.0, 0.5);
/// Extra solid for a diagonal slide. The near face is X = 62.
pub const SLIDE_CENTER: Vec3 = Vec3::new(70.0, 50.0, 145.0);
pub const SLIDE_HALF: Vec3 = Vec3::new(8.0, 50.0, 40.0);
/// Bumper centers on X. The inner faces are at X = ±90.
pub const BUMPER_X: f32 = 102.0;
pub const BUMPER_HALF: Vec3 = Vec3::new(12.0, 40.0, 20.0);

/// Gameplay axes for `--collision`. `J`/`L` are X and `K`/`I` are Z. They are not camera-relative.
pub const WALK_X_ACTION: &str = "walk_x";
pub const WALK_Z_ACTION: &str = "walk_z";

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
    /// `--collision` is on. The sine patrol does not write the walker, and the pillar can carry a collider.
    pub collision: bool,
    /// `--locomotion` is on. The walker is a character root on the course, not the sine patrol.
    pub locomotion: bool,
    /// Non-colliding presentation child of [`Scene::walker`]. `None` until collision mode installs it.
    pub walker_visual: Option<Entity>,
    /// Second gameplay root. It is not a scene member, so a replacement leaves it in place.
    pub pace: Option<Entity>,
    /// `+1` walks toward +X. Flips only when the previous step was blocked on that side.
    pub pace_direction: f32,
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
        collision: false,
        locomotion: false,
        walker_visual: None,
        pace: None,
        pace_direction: 1.0,
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
        Transform::from_xyz(PILLAR_CENTER.x, PILLAR_CENTER.y, PILLAR_CENTER.z),
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

pub(crate) fn figure(tint: Color) -> Sprite {
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

pub(crate) fn panel(color: Color) -> Material {
    Material { base_color: color, base_texture: Some(AssetHandle::from_name("panel")), lit: false }
}

pub(crate) fn add_meshes(world: &mut World) {
    let mut assets = world.resource_mut::<AssetManager>();
    let mut add = |name: &str, mesh: MeshData| {
        assets.insert(name, mesh).expect("example asset names are distinct");
    };
    add(
        "floor_tile",
        MeshData::cuboid(Vec3::new(100.0, 8.0, 100.0)).expect("cuboid dimensions are finite and positive"),
    );
    add("pad", MeshData::cuboid(Vec3::new(84.0, 2.0, 84.0)).expect("cuboid dimensions are finite and positive"));
    add("pillar", MeshData::cuboid(PILLAR_SIZE).expect("cuboid dimensions are finite and positive"));
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
        Transform::from_xyz(PILLAR_CENTER.x, PILLAR_CENTER.y, PILLAR_CENTER.z),
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
/// Both are plain `Transform` writes unless `--collision` or `--locomotion` is on. In those modes
/// the walker's position comes from the fixed-step motor, and this system leaves it alone. The
/// walker is not a scene member, so it keeps moving after a replacement. Environment B has no
/// entity named `cube`.
pub fn motion_system(world: &mut World) {
    let t = world.resource::<Time>().elapsed_secs();
    let (cube, walker, collision, locomotion) = {
        let scene = world.resource::<Scene>();
        (scene.cube, scene.walker, scene.collision, scene.locomotion)
    };
    if let Some(cube) = cube {
        if let Ok(mut transform) = world.component_mut::<Transform>(cube) {
            transform.rotation = Quat::from_rotation_y(t * 0.7) * Quat::from_rotation_x(0.4);
        }
    }
    // Collision mode moves the gameplay root from a fixed-step intent. The patrol write would
    // replace that position every frame.
    if !collision && !locomotion {
        if let Ok(mut transform) = world.component_mut::<Transform>(walker) {
            transform.position = Vec3::new(WALK_RANGE * (t * 0.8).sin(), 0.0, WALK_Z);
        }
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
    if scene.collision && scene.environment == Environment::A {
        attach_pillar(world);
    }
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
    let visual = scene.walker_visual;
    let collision = scene.collision && scene.environment == Environment::A;
    world.insert_resource(scene);
    for (_, entity) in figures {
        if let Ok(mut sprite) = world.component_mut::<Sprite>(entity) {
            sprite.orientation = orientation;
        }
    }
    if let Some(visual) = visual {
        if let Ok(mut sprite) = world.component_mut::<Sprite>(visual) {
            sprite.orientation = orientation;
        }
    }
    // The replacement already despawned the previous pillar. The new environment A pillar is a new
    // entity, so the collider has to be attached here. `sync_collision` has already run this step;
    // the next fixed iteration indexes the new pillar and drops the despawned id.
    if collision {
        attach_pillar(world);
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

/// Swaps the two built-in projections. `P` uses this.
///
/// A [`CameraProjection::Custom`] matrix is returned unchanged. The key is a shortcut between
/// perspective and orthographic, not a replacement for the example's oblique matrix. `C` still
/// cycles every configuration, including `custom`.
pub fn swap_builtin_projection(projection: CameraProjection) -> CameraProjection {
    if matches!(projection, CameraProjection::Custom(_)) {
        return projection;
    }
    if projection.is_perspective() {
        CameraProjection::orthographic(ORTHO_HEIGHT).with_clip_range(1.0, 4000.0)
    } else {
        CameraProjection::perspective(FOV_DEGREES).with_clip_range(1.0, 4000.0)
    }
}

/// Switches the scene's camera to configuration `index` (wrapping), keeping the same world.
pub fn select_config(scene: &mut Scene, index: usize) {
    scene.config = index % NAMED.len();
    let (camera, transform) = NAMED[scene.config].1();
    scene.rig = CameraRig::from_transform(&transform);
    scene.projection = camera.projection;
}

pub use cameras::{FOV_DEGREES, ORTHO_HEIGHT};

/// What the gameplay root displays. The collider stays on the root either way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyVisual {
    /// The figure sprite, bottom-center anchor, identity local transform.
    Sprite,
    /// A mesh child. Used to show that the visual does not own the collider.
    Mesh,
}

/// Capsule for a character whose root sits at the feet.
pub fn character_collider() -> Collider {
    Collider::capsule(BODY_RADIUS, BODY_HALF_HEIGHT).with_offset(BODY_OFFSET)
}

/// Turns the persistent walker into a gameplay root and adds the pillar collider, the gate sensor,
/// the slide wall, and the scripted character.
///
/// The walker stays named `walker` and stays outside both environments. Its sprite moves to a child
/// with no collider. The sensor, the slide wall, and the pace character are also outside the
/// environments, so Tab leaves them in place. The pillar is a member of environment A. Switching to
/// B despawns it; switching back attaches a collider to the new pillar entity on the next publish.
/// Environment B's yard pillar is a different name and stays visual only.
///
/// After a switch to B the environment A pillar no longer blocks. The sensor, the slide wall, and
/// the pace character remain. Nothing here keeps the old pillar id.
pub fn install_collision(world: &mut World, visual: BodyVisual) {
    let walker = world.resource::<Scene>().walker;
    world.component_mut::<Transform>(walker).expect("walker").position = CHARACTER_START;
    let visual_entity = attach_body_visual(world, walker, visual);
    world.insert(walker, (character_collider(), CharacterMotor::default())).expect("walker");
    {
        let mut scene = world.resource_mut::<Scene>();
        scene.collision = true;
        scene.walker_visual = Some(visual_entity);
        scene.pace_direction = 1.0;
    }
    attach_pillar(world);
    spawn_gate(world);
    spawn_slide_wall(world);
    let pace = spawn_pace(world);
    world.resource_mut::<Scene>().pace = Some(pace);
}

fn attach_body_visual(world: &mut World, walker: Entity, visual: BodyVisual) -> Entity {
    match visual {
        BodyVisual::Sprite => {
            let mut sprite = *world.component::<Sprite>(walker).expect("the walker starts as a sprite");
            sprite.anchor = Some(Vec2::new(0.5, 0.0));
            world.remove::<Sprite>(walker).expect("walker sprite");
            world.spawn_child(walker, (Transform::IDENTITY, sprite, Name::new("walker visual")))
        }
        BodyVisual::Mesh => {
            world.remove::<Sprite>(walker).expect("walker sprite");
            let mesh = Mesh::new("cube").with_material(panel(Color::rgb(0.95, 0.55, 0.95)));
            world.spawn_child(walker, (Transform::IDENTITY, mesh, Name::new("walker visual")))
        }
    }
}

fn attach_pillar(world: &mut World) {
    let Some(pillar) = named(world, "pillar") else {
        return;
    };
    if world.component::<Collider>(pillar).is_ok() {
        return;
    }
    world.insert(pillar, (Collider::aabb_box(PILLAR_SIZE * 0.5),)).expect("pillar");
}

fn spawn_gate(world: &mut World) {
    let sensor = world.spawn((
        Transform::from_xyz(SENSOR_CENTER.x, SENSOR_CENTER.y, SENSOR_CENTER.z),
        Collider::aabb_box(SENSOR_HALF).sensor(),
        Name::new("gate sensor"),
    ));
    // Cube mesh is 80 units on a side. Scale it to the sensor slab. The collider stays on the parent.
    let scale = Vec3::new(SENSOR_HALF.x * 2.0 / 80.0, SENSOR_HALF.y * 2.0 / 80.0, SENSOR_HALF.z * 2.0 / 80.0);
    world.spawn_child(
        sensor,
        (
            Transform::from_scale(scale),
            Mesh::new("cube").with_material(panel(Color::rgb(0.45, 0.95, 0.75))),
            Name::new("gate marker"),
        ),
    );
}

fn spawn_slide_wall(world: &mut World) {
    let wall = world.spawn((
        Transform::from_xyz(SLIDE_CENTER.x, SLIDE_CENTER.y, SLIDE_CENTER.z),
        Collider::aabb_box(SLIDE_HALF),
        Name::new("slide wall"),
    ));
    let scale = Vec3::new(SLIDE_HALF.x * 2.0 / 80.0, SLIDE_HALF.y * 2.0 / 80.0, SLIDE_HALF.z * 2.0 / 80.0);
    world.spawn_child(
        wall,
        (
            Transform::from_scale(scale),
            Mesh::new("cube").with_material(panel(Color::rgb(0.75, 0.55, 0.85))),
            Name::new("slide wall mesh"),
        ),
    );
}

fn spawn_pace(world: &mut World) -> Entity {
    let pace = world.spawn((
        Transform::from_xyz(PACE_START.x, PACE_START.y, PACE_START.z),
        character_collider(),
        CharacterMotor::default(),
        Name::new("pace"),
    ));
    let mut sprite = figure(Color::rgb(0.35, 0.75, 0.95));
    sprite.anchor = Some(Vec2::new(0.5, 0.0));
    world.spawn_child(pace, (Transform::IDENTITY, sprite, Name::new("pace visual")));
    spawn_bumper(world, BUMPER_X, "bumper +x");
    spawn_bumper(world, -BUMPER_X, "bumper -x");
    pace
}

fn spawn_bumper(world: &mut World, x: f32, name: &str) {
    let bumper =
        world.spawn((Transform::from_xyz(x, 40.0, PACE_START.z), Collider::aabb_box(BUMPER_HALF), Name::new(name)));
    let scale = Vec3::new(BUMPER_HALF.x * 2.0 / 80.0, BUMPER_HALF.y * 2.0 / 80.0, BUMPER_HALF.z * 2.0 / 80.0);
    world.spawn_child(
        bumper,
        (Transform::from_scale(scale), Mesh::new("cube").with_material(panel(Color::rgb(0.55, 0.58, 0.62)))),
    );
}

/// Fixed-step producer. Registered before `sync_collision`. Writes a world-axis `MoveIntent` for
/// one fixed iteration and does not touch `Transform`.
///
/// Held `walk_x` / `walk_z` become a direction on XZ. A diagonal is normalized so the two axes do
/// not stack. The displacement is that direction times [`WALK_SPEED`] times the fixed delta. Y is
/// zero because the keys do not request it; the motor does not lock Y. No intent is written when
/// the stick is idle, so a later fixed system can still place one.
///
/// The pace character always receives its own X intent. It reverses only after a blocked step that
/// is still on the side it was walking toward.
pub fn collision_motion(world: &mut World) {
    let scene = world.resource::<Scene>();
    if !scene.collision {
        return;
    }
    let (walker, pace, mut direction) = (scene.walker, scene.pace, scene.pace_direction);
    drop(scene);

    let dt = world.resource::<Time>().delta_secs();
    let input = world.resource::<Input>();
    let actions = world.resource::<Actions>();
    let wish = Vec3::new(actions.axis(&input, WALK_X_ACTION), 0.0, actions.axis(&input, WALK_Z_ACTION));
    drop(actions);
    drop(input);
    let len_sq = wish.length_squared();
    if len_sq > 0.0 {
        let direction = if len_sq > 1.0 { wish.normalized() } else { wish };
        world.insert(walker, (MoveIntent::new(direction * WALK_SPEED * dt),)).expect("walker");
    }

    let Some(pace) = pace else {
        return;
    };
    let blocked = world.component::<MoveResult>(pace).is_ok_and(|result| result.blocked());
    let x = world.component::<Transform>(pace).map(|transform| transform.position.x).unwrap_or(0.0);
    if blocked && direction > 0.0 && x > 0.0 {
        direction = -1.0;
    } else if blocked && direction < 0.0 && x < 0.0 {
        direction = 1.0;
    }
    world.resource_mut::<Scene>().pace_direction = direction;
    world.insert(pace, (MoveIntent::new(Vec3::new(direction * PACE_SPEED * dt, 0.0, 0.0)),)).expect("pace");
}

/// Update-stage reader. `TriggerEvent` is visible here and gone when `Game::step` returns.
pub fn collision_feedback(world: &mut World) {
    if !world.resource::<Scene>().collision {
        return;
    }
    for event in world.events::<TriggerEvent>() {
        log::info!("trigger {event:?}");
    }
}
