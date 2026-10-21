//! Extract-only interpolation. Simulation components are read back after every extract.

use core::f32::consts::{FRAC_PI_2, FRAC_PI_4};

use engine_assets::{AssetManager, MeshData, SpriteAsset, Texture};
use engine_core::{Color, FixedTimestep};
use engine_math::{Mat4, Quat, Transform, Vec2, Vec3};
use engine_renderer::*;
use engine_world::{propagate_transforms, GlobalTransform, Interpolate, Parent, World};

const VIEWPORT: Vec2 = Vec2::new(128.0, 128.0);

fn assets() -> AssetManager {
    let mut assets = AssetManager::in_memory();
    let texture = assets.insert("white", Texture::solid(4, 4, [255, 255, 255, 255])).unwrap();
    assets.insert("sprite", SpriteAsset::whole_texture(texture, 4, 4)).unwrap();
    assets.insert("tiles", Texture::solid(4, 4, [255, 255, 255, 255])).unwrap();
    assets.insert("cube", MeshData::cuboid(Vec3::ONE).unwrap()).unwrap();
    assets
}

fn world_with_camera(camera: Camera, pose: Transform) -> (World, AssetManager) {
    let mut world = World::new();
    world.spawn((pose, camera));
    (world, assets())
}

fn look_on() -> (World, AssetManager) {
    world_with_camera(Camera::orthographic(80.0), Transform::from_xyz(0.0, 0.0, -20.0))
}

fn mesh(color: Color) -> Mesh {
    Mesh::new("cube").with_material(Material::unlit(color))
}

fn sprite(orientation: SpriteOrientation) -> Sprite {
    Sprite::new("sprite").with_size(Vec2::ONE).with_anchor(Vec2::ZERO).with_orientation(orientation).unlit().cutout()
}

fn set_alpha(world: &mut World, fraction: f32) {
    let mut time = FixedTimestep::new(60);
    if fraction > 0.0 {
        let step = time.step();
        assert_eq!(time.accumulate(step / 2), 0);
    }
    assert!((time.alpha() - fraction).abs() < 1e-4, "{}", time.alpha());
    world.insert_resource(time);
}

fn mark(world: &mut World, entity: engine_world::Entity, previous: Transform) {
    world.insert(entity, (Interpolate { previous: Some(previous) },)).unwrap();
}

fn refresh(world: &mut World) {
    propagate_transforms(world);
}

fn extract(world: &World, assets: &AssetManager) -> RenderScene {
    let before = sim_stamp(world);
    let scene = extract_scene(world, assets, VIEWPORT).expect("camera");
    assert_eq!(sim_stamp(world), before, "extraction writes no simulation transform");
    scene
}

fn sim_stamp(world: &World) -> Vec<(Transform, Mat4)> {
    world.query::<(&Transform, &GlobalTransform)>().iter().map(|(_, (local, global))| (*local, global.matrix)).collect()
}

fn mesh_translation(scene: &RenderScene, color: Color) -> Vec3 {
    let rgba = color.to_array();
    scene
        .meshes
        .iter()
        .find(|mesh| mesh.color == rgba)
        .unwrap_or_else(|| panic!("no mesh {rgba:?} in {}", scene.meshes.len()))
        .model
        .translation()
}

#[test]
fn an_unmarked_entity_extracts_its_global_transform() {
    let (mut world, assets) = look_on();
    let entity = world.spawn((Transform::from_xyz(3.0, 4.0, 0.0), mesh(Color::WHITE)));
    refresh(&mut world);
    let scene = extract(&world, &assets);
    assert_eq!(scene.meshes[0].model, world.component::<GlobalTransform>(entity).unwrap().matrix);
}

#[test]
fn a_marker_with_no_history_keeps_the_global_transform() {
    let (mut world, assets) = look_on();
    let entity = world.spawn((Transform::from_xyz(3.0, 0.0, 0.0), Interpolate::new(), mesh(Color::WHITE)));
    refresh(&mut world);
    set_alpha(&mut world, 0.5);
    let scene = extract(&world, &assets);
    assert_eq!(scene.meshes[0].model, world.component::<GlobalTransform>(entity).unwrap().matrix);
    assert!(world.component::<Interpolate>(entity).unwrap().previous.is_none());
}

#[test]
fn translation_rotation_and_scale_follow_transform_lerp() {
    let (mut world, assets) = look_on();
    let previous = Transform::from_xyz(0.0, 0.0, 0.0)
        .with_rotation(Quat::from_rotation_y(0.0))
        .with_scale(Vec3::new(-4.0, 2.0, 1.0));
    let current = Transform::from_xyz(10.0, -2.0, 1.0)
        .with_rotation(Quat::from_rotation_y(FRAC_PI_2))
        .with_scale(Vec3::new(-2.0, 4.0, 1.0));
    let entity = world.spawn((current, mesh(Color::WHITE)));
    mark(&mut world, entity, previous);
    refresh(&mut world);
    set_alpha(&mut world, 0.5);
    let scene = extract(&world, &assets);
    let expected = previous.lerp(&current, 0.5);
    assert!(scene.meshes[0].model.approx_eq(&expected.to_matrix(), 1e-4));
    assert_eq!(*world.component::<Transform>(entity).unwrap(), current);
    assert_eq!(world.component::<GlobalTransform>(entity).unwrap().matrix, current.to_matrix());
    assert!(expected.scale.x < 0.0, "negative scale keeps its sign when both ends agree");
}

#[test]
fn rotation_takes_the_shortest_arc_and_scale_sign_changes_pass_through_zero() {
    let (mut world, assets) = look_on();
    let previous = Transform::from_rotation(Quat::IDENTITY).with_scale(Vec3::new(-2.0, 1.0, 1.0));
    let long_way = 190f32.to_radians();
    let current = Transform::from_rotation(Quat::from_rotation_y(long_way)).with_scale(Vec3::new(2.0, 1.0, 1.0));
    let entity = world.spawn((current, mesh(Color::WHITE)));
    mark(&mut world, entity, previous);
    refresh(&mut world);
    set_alpha(&mut world, 0.5);
    let scene = extract(&world, &assets);
    let blended = previous.lerp(&current, 0.5);
    assert!(scene.meshes[0].model.approx_eq(&blended.to_matrix(), 1e-4));
    let (_, angle) = blended.rotation.to_axis_angle();
    assert!((angle - 85f32.to_radians()).abs() < 0.05, "190 degrees blends along the short arc, angle {angle}");
    assert!(blended.scale.x.abs() < 1e-4, "a scale sign change passes through zero");

    let opposite = Transform::from_rotation(Quat::from_rotation_y(core::f32::consts::PI));
    *world.component_mut::<Transform>(entity).unwrap() = opposite;
    world.component_mut::<Interpolate>(entity).unwrap().previous = Some(Transform::IDENTITY);
    refresh(&mut world);
    let scene = extract(&world, &assets);
    assert!(scene.meshes[0].model.approx_eq(&Transform::IDENTITY.lerp(&opposite, 0.5).to_matrix(), 1e-4));
    assert!(scene.meshes[0].model.cols[0].x.is_finite());
}

#[test]
fn a_non_finite_history_renders_the_current_local() {
    let (mut world, assets) = look_on();
    let current = Transform::from_xyz(3.0, 1.0, 0.0);
    let entity = world.spawn((current, mesh(Color::WHITE)));
    let mut broken = current;
    broken.position.x = f32::NAN;
    mark(&mut world, entity, broken);
    refresh(&mut world);
    set_alpha(&mut world, 0.5);
    let scene = extract(&world, &assets);
    assert_eq!(scene.meshes[0].model.translation(), current.position);
    assert!(world.component::<Transform>(entity).unwrap().position.x.is_finite());
}

#[test]
fn parent_and_child_blend_locals_then_compose() {
    let (mut world, assets) = look_on();
    let parent_previous = Transform::from_xyz(0.0, 0.0, 0.0);
    let parent_current = Transform::from_xyz(10.0, 0.0, 0.0);
    let parent = world.spawn((parent_current, mesh(Color::rgb(1.0, 0.0, 0.0))));
    mark(&mut world, parent, parent_previous);
    let child = world.spawn_child(parent, (Transform::from_xyz(1.0, 0.0, 0.0), mesh(Color::rgb(0.0, 1.0, 0.0))));
    let grandchild_parent_space = Transform::from_xyz(0.0, 2.0, 0.0);
    let grandchild = world.spawn_child(child, (grandchild_parent_space, mesh(Color::rgb(0.0, 0.0, 1.0))));
    mark(&mut world, grandchild, Transform::from_xyz(0.0, 0.0, 0.0));
    refresh(&mut world);
    set_alpha(&mut world, 0.5);

    let scene = extract(&world, &assets);
    let parent_render = parent_previous.lerp(&parent_current, 0.5);
    let child_render = parent_render.mul_transform(&Transform::from_xyz(1.0, 0.0, 0.0));
    let grandchild_render = child_render.mul_transform(&Transform::IDENTITY.lerp(&grandchild_parent_space, 0.5));
    assert!(mesh_translation(&scene, Color::rgb(1.0, 0.0, 0.0)).approx_eq(parent_render.position, 1e-4));
    assert!(mesh_translation(&scene, Color::rgb(0.0, 1.0, 0.0)).approx_eq(child_render.position, 1e-4));
    assert!(mesh_translation(&scene, Color::rgb(0.0, 0.0, 1.0)).approx_eq(grandchild_render.position, 1e-4));
    assert_eq!(world.component::<GlobalTransform>(child).unwrap().translation(), Vec3::new(11.0, 0.0, 0.0));
    assert!(
        (mesh_translation(&scene, Color::rgb(0.0, 1.0, 0.0)).x - 6.0).abs() < 1e-4,
        "an unmarked child keeps its local offset from the smoothed parent"
    );
}

#[test]
fn a_rotated_parent_does_not_lerp_the_child_in_world_space() {
    let (mut world, assets) = look_on();
    let previous = Transform::IDENTITY;
    let current = Transform::from_rotation(Quat::from_rotation_y(FRAC_PI_2));
    let parent = world.spawn((current,));
    mark(&mut world, parent, previous);
    let child = world.spawn_child(parent, (Transform::from_xyz(1.0, 0.0, 0.0), mesh(Color::WHITE)));
    refresh(&mut world);
    set_alpha(&mut world, 0.5);
    let scene = extract(&world, &assets);
    let composed = previous.lerp(&current, 0.5).mul_transform(&Transform::from_xyz(1.0, 0.0, 0.0));
    assert!(mesh_translation(&scene, Color::WHITE).approx_eq(composed.position, 1e-4));
    let chord = Vec3::new(0.5, 0.0, -0.5);
    assert!(
        mesh_translation(&scene, Color::WHITE).distance(chord) > 0.1,
        "the child sits on the parent's arc, not on the chord between world positions"
    );
    assert!(world
        .component::<GlobalTransform>(child)
        .unwrap()
        .translation()
        .approx_eq(Vec3::new(0.0, 0.0, -1.0), 1e-4));
}

#[test]
fn a_transform_less_ancestor_stays_identity() {
    let (mut world, assets) = look_on();
    let root_previous = Transform::from_xyz(0.0, 0.0, 0.0);
    let root_current = Transform::from_xyz(8.0, 0.0, 0.0);
    let root = world.spawn((root_current,));
    mark(&mut world, root, root_previous);
    let group = world.spawn_child(root, ());
    let leaf = world.spawn_child(group, (Transform::from_xyz(1.0, 0.0, 0.0), mesh(Color::WHITE)));
    refresh(&mut world);
    set_alpha(&mut world, 0.5);
    let scene = extract(&world, &assets);
    assert!(mesh_translation(&scene, Color::WHITE).approx_eq(Vec3::new(5.0, 0.0, 0.0), 1e-4));
    assert_eq!(world.component::<GlobalTransform>(leaf).unwrap().translation(), Vec3::new(9.0, 0.0, 0.0));
    assert!(world.component::<GlobalTransform>(group).is_err());
}

#[test]
fn detach_snaps_the_rewritten_child_and_keeps_the_grandchild_blend() {
    let (mut world, assets) = look_on();
    let parent = world.spawn((Transform::from_xyz(10.0, 0.0, 0.0),));
    let child_local = Transform::from_xyz(1.0, 0.0, 0.0);
    let child = world.spawn_child(parent, (child_local, mesh(Color::rgb(1.0, 0.0, 0.0))));
    mark(&mut world, child, child_local);
    let grandchild_previous = Transform::from_xyz(0.0, 0.0, 0.0);
    let grandchild = world.spawn_child(child, (Transform::from_xyz(2.0, 0.0, 0.0), mesh(Color::rgb(0.0, 1.0, 0.0))));
    mark(&mut world, grandchild, grandchild_previous);
    refresh(&mut world);
    set_alpha(&mut world, 0.5);

    world.detach_children(parent).unwrap();
    refresh(&mut world);
    let rewritten = *world.component::<Transform>(child).unwrap();
    assert!((rewritten.position.x - 11.0).abs() < 1e-3);
    assert_eq!(world.component::<Interpolate>(child).unwrap().previous, Some(rewritten));
    assert_eq!(world.component::<Interpolate>(grandchild).unwrap().previous, Some(grandchild_previous));

    let scene = extract(&world, &assets);
    assert!(mesh_translation(&scene, Color::rgb(1.0, 0.0, 0.0)).approx_eq(rewritten.position, 1e-3));
    assert!(mesh_translation(&scene, Color::rgb(0.0, 1.0, 0.0)).approx_eq(Vec3::new(12.0, 0.0, 0.0), 1e-3));
    assert!((world.component::<GlobalTransform>(grandchild).unwrap().translation().x - 13.0).abs() < 1e-3);
}

#[test]
fn snap_renders_the_teleported_local() {
    let (mut world, assets) = look_on();
    let entity = world.spawn((Transform::from_xyz(1.0, 0.0, 0.0), mesh(Color::WHITE)));
    mark(&mut world, entity, Transform::from_xyz(0.0, 0.0, 0.0));
    let jumped = Transform::from_xyz(40.0, 0.0, 0.0);
    *world.component_mut::<Transform>(entity).unwrap() = jumped;
    world.component_mut::<Interpolate>(entity).unwrap().snap(jumped);
    refresh(&mut world);
    set_alpha(&mut world, 0.5);
    let scene = extract(&world, &assets);
    assert!(mesh_translation(&scene, Color::WHITE).approx_eq(jumped.position, 1e-3));
    assert_eq!(world.component::<GlobalTransform>(entity).unwrap().translation(), jumped.position);
}

#[test]
fn set_parent_keeps_the_local_blend_in_the_new_parent_space() {
    let (mut world, assets) = look_on();
    let from = world.spawn((Transform::from_xyz(0.0, 0.0, 0.0),));
    let to = world.spawn((Transform::from_xyz(10.0, 0.0, 0.0),));
    let previous = Transform::from_xyz(0.0, 0.0, 0.0);
    let current = Transform::from_xyz(2.0, 0.0, 0.0);
    let child = world.spawn_child(from, (current, mesh(Color::WHITE)));
    mark(&mut world, child, previous);
    world.set_parent(child, to).unwrap();
    refresh(&mut world);
    set_alpha(&mut world, 0.5);
    assert_eq!(world.component::<Interpolate>(child).unwrap().previous, Some(previous));
    assert_eq!(*world.component::<Transform>(child).unwrap(), current);
    let scene = extract(&world, &assets);
    // new parent * lerp(previous local, current local), not a lerp of the old and new world poses
    assert!(mesh_translation(&scene, Color::WHITE).approx_eq(Vec3::new(11.0, 0.0, 0.0), 1e-4));
    assert!(world
        .component::<GlobalTransform>(child)
        .unwrap()
        .translation()
        .approx_eq(Vec3::new(12.0, 0.0, 0.0), 1e-4));
}

#[test]
fn reparenting_an_ancestor_keeps_the_descendants_local_blend() {
    let (mut world, assets) = look_on();
    let root = world.spawn((Transform::from_xyz(20.0, 0.0, 0.0),));
    let parent_previous = Transform::from_xyz(0.0, 0.0, 0.0);
    let parent_current = Transform::from_xyz(4.0, 0.0, 0.0);
    let parent = world.spawn((parent_current, mesh(Color::rgb(1.0, 0.0, 0.0))));
    mark(&mut world, parent, parent_previous);
    let child = world.spawn_child(parent, (Transform::from_xyz(1.0, 0.0, 0.0), mesh(Color::rgb(0.0, 1.0, 0.0))));
    world.set_parent(parent, root).unwrap();
    refresh(&mut world);
    set_alpha(&mut world, 0.5);
    assert_eq!(world.component::<Interpolate>(parent).unwrap().previous, Some(parent_previous));
    let scene = extract(&world, &assets);
    assert!(mesh_translation(&scene, Color::rgb(1.0, 0.0, 0.0)).approx_eq(Vec3::new(22.0, 0.0, 0.0), 1e-3));
    assert!(mesh_translation(&scene, Color::rgb(0.0, 1.0, 0.0)).approx_eq(Vec3::new(23.0, 0.0, 0.0), 1e-3));
    assert!(world
        .component::<GlobalTransform>(parent)
        .unwrap()
        .translation()
        .approx_eq(Vec3::new(24.0, 0.0, 0.0), 1e-4));
    assert!(world
        .component::<GlobalTransform>(child)
        .unwrap()
        .translation()
        .approx_eq(Vec3::new(25.0, 0.0, 0.0), 1e-4));
}

#[test]
fn unlinking_a_parent_keeps_both_locals() {
    let (mut world, assets) = look_on();
    let parent = world.spawn((Transform::from_xyz(10.0, 0.0, 0.0),));
    let previous = Transform::from_xyz(0.0, 0.0, 0.0);
    let current = Transform::from_xyz(2.0, 0.0, 0.0);
    let child = world.spawn_child(parent, (current, mesh(Color::WHITE)));
    mark(&mut world, child, previous);
    world.remove::<Parent>(child).unwrap();
    refresh(&mut world);
    set_alpha(&mut world, 0.5);
    assert_eq!(world.component::<Interpolate>(child).unwrap().previous, Some(previous));
    assert_eq!(*world.component::<Transform>(child).unwrap(), current);
    let scene = extract(&world, &assets);
    assert!(mesh_translation(&scene, Color::WHITE).approx_eq(Vec3::new(1.0, 0.0, 0.0), 1e-3));
    assert!(world.component::<GlobalTransform>(child).unwrap().translation().approx_eq(current.position, 1e-4));
}

#[test]
fn despawning_a_parent_removes_the_interpolated_child() {
    let (mut world, assets) = look_on();
    let parent = world.spawn((Transform::from_xyz(3.0, 0.0, 0.0),));
    let child = world.spawn_child(parent, (Transform::from_xyz(1.0, 0.0, 0.0), mesh(Color::WHITE)));
    mark(&mut world, child, Transform::IDENTITY);
    refresh(&mut world);
    world.despawn(parent).unwrap();
    assert!(!world.contains(child));
    let scene = extract(&world, &assets);
    assert!(scene.meshes.is_empty());
}

#[test]
fn sprite_orientations_and_camera_configurations_share_one_matrix() {
    let previous = Transform::from_xyz(0.0, 0.0, 0.0).with_rotation(Quat::from_rotation_y(FRAC_PI_2));
    let current = Transform::from_xyz(2.0, 0.0, 0.0).with_rotation(Quat::from_rotation_y(FRAC_PI_2));
    let expected = previous.lerp(&current, 0.5);

    let (mut world, assets) = look_on();
    for orientation in [SpriteOrientation::Fixed, SpriteOrientation::Billboard, SpriteOrientation::Upright] {
        let entity = world.spawn((current, sprite(orientation)));
        mark(&mut world, entity, previous);
    }
    refresh(&mut world);
    set_alpha(&mut world, 0.5);
    let scene = extract(&world, &assets);
    assert_eq!(scene.cutout_sprites.instances.len(), 3);
    for instance in &scene.cutout_sprites.instances {
        assert!(instance.model.translation().approx_eq(expected.position, 1e-4));
    }
    let fixed_axis = scene
        .cutout_sprites
        .instances
        .iter()
        .find(|instance| instance.model.approx_eq(&expected.to_matrix(), 1e-4))
        .expect("fixed orientation keeps the interpolated matrix")
        .model
        .cols[0]
        .xyz();
    let turned = scene.cutout_sprites.instances.iter().any(|instance| {
        !instance.model.cols[0].xyz().approx_eq(fixed_axis, 1e-3)
            && instance.model.translation().approx_eq(expected.position, 1e-4)
    });
    assert!(turned, "billboard and upright rebuild axes from the same translation");

    let mesh_entity = world.spawn((current, mesh(Color::WHITE)));
    mark(&mut world, mesh_entity, previous);
    let configs = [
        (Camera::perspective(50.0), Transform::from_xyz(0.0, 0.0, -12.0)),
        (Camera::orthographic(40.0), Transform::from_xyz(0.0, 0.0, -12.0)),
        (
            Camera::orthographic(40.0),
            Transform::from_xyz(8.0, 6.0, -8.0).with_rotation(Quat::from_rotation_y(FRAC_PI_4)),
        ),
    ];
    let camera_entity = world.query::<&Camera>().iter().next().unwrap().0;
    let mut views = Vec::new();
    for (camera, pose) in configs {
        *world.component_mut::<Camera>(camera_entity).unwrap() = camera;
        *world.component_mut::<Transform>(camera_entity).unwrap() = pose;
        refresh(&mut world);
        let scene = extract(&world, &assets);
        assert!(mesh_translation(&scene, Color::WHITE).approx_eq(expected.position, 1e-4));
        views.push(scene.view.view);
    }
    assert!(views[0] != views[2], "the isometric-style pose changes the view, not the interpolated mesh");
    assert_eq!(views.len(), 3);
}

#[test]
fn a_marked_camera_and_a_child_camera_use_the_same_blend() {
    let (mut world, assets) = look_on();
    let camera = world.query::<&Camera>().iter().next().unwrap().0;
    let previous = Transform::from_xyz(0.0, 0.0, -20.0);
    let current = Transform::from_xyz(0.0, 0.0, -10.0);
    *world.component_mut::<Transform>(camera).unwrap() = current;
    mark(&mut world, camera, previous);
    refresh(&mut world);
    set_alpha(&mut world, 0.5);
    let scene = extract(&world, &assets);
    assert!(scene.view.position.approx_eq(Vec3::new(0.0, 0.0, -15.0), 1e-3));
    assert_eq!(world.component::<GlobalTransform>(camera).unwrap().translation(), current.position);

    world.remove::<Interpolate>(camera).unwrap();
    let parent = world.spawn((current,));
    mark(&mut world, parent, previous);
    world.set_parent(camera, parent).unwrap();
    *world.component_mut::<Transform>(camera).unwrap() = Transform::IDENTITY;
    refresh(&mut world);
    let scene = extract(&world, &assets);
    assert!(scene.view.position.approx_eq(Vec3::new(0.0, 0.0, -15.0), 1e-3));
    assert!(world.component::<GlobalTransform>(camera).unwrap().translation().approx_eq(current.position, 1e-4));
}

#[test]
fn lights_and_tilemaps_follow_a_marked_parent() {
    let (mut world, assets) = look_on();
    let previous = Transform::from_xyz(0.0, 0.0, 0.0);
    let current = Transform::from_xyz(6.0, 0.0, 0.0);
    let parent = world.spawn((current,));
    mark(&mut world, parent, previous);
    world.spawn_child(parent, (Transform::IDENTITY, Light::point(Color::WHITE, 1.0, 10.0)));
    let mut map = TileMap::new("tiles", 1, 1, Vec2::ONE, 1, 1);
    map.set(0, 0, 0);
    world.spawn_child(parent, (Transform::IDENTITY, map));
    refresh(&mut world);
    set_alpha(&mut world, 0.5);
    let scene = extract(&world, &assets);
    assert!(scene.lights[0].position.approx_eq(Vec3::new(3.0, 0.0, 0.0), 1e-4));
    assert!(scene.cutout_sprites.instances[0].model.translation().approx_eq(Vec3::new(3.0, 0.0, 0.0), 1e-4));
}

#[test]
fn removing_the_marker_restores_the_simulation_matrix() {
    let (mut world, assets) = look_on();
    let current = Transform::from_xyz(4.0, 0.0, 0.0);
    let entity = world.spawn((current, mesh(Color::WHITE)));
    mark(&mut world, entity, Transform::from_xyz(0.0, 0.0, 0.0));
    refresh(&mut world);
    set_alpha(&mut world, 0.5);
    let blended = extract(&world, &assets);
    assert!((mesh_translation(&blended, Color::WHITE).x - 2.0).abs() < 1e-4);
    world.remove::<Interpolate>(entity).unwrap();
    let plain = extract(&world, &assets);
    assert_eq!(plain.meshes[0].model, world.component::<GlobalTransform>(entity).unwrap().matrix);
}

#[test]
fn despawn_leaves_nothing_to_draw() {
    let (mut world, assets) = look_on();
    let entity = world.spawn((Transform::from_xyz(1.0, 0.0, 0.0), Interpolate::new(), mesh(Color::WHITE)));
    mark(&mut world, entity, Transform::IDENTITY);
    refresh(&mut world);
    world.despawn(entity).unwrap();
    let scene = extract(&world, &assets);
    assert!(scene.meshes.is_empty());
}
