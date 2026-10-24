//! Debug segments are copied into the render scene and shared by every camera. No GPU.

use engine_core::Color;
use engine_math::{Transform, Vec2, Vec3};
use engine_renderer::{extract_scene, Camera, DebugDraw, ViewportRect};
use engine_world::{propagate_transforms, World};

const SIZE: Vec2 = Vec2::new(128.0, 128.0);

fn cameras(count: i32) -> World {
    let mut world = World::new();
    for order in 0..count {
        let viewport = if count == 1 {
            ViewportRect::FULL
        } else {
            ViewportRect::new(order as f32 / count as f32, 0.0, 1.0 / count as f32, 1.0)
        };
        world.spawn((Camera { order, viewport, ..Camera::orthographic(4.0) }, Transform::from_xyz(0.0, 0.0, -8.0)));
    }
    propagate_transforms(&mut world);
    world
}

#[test]
fn missing_or_empty_debug_adds_no_segments() {
    let assets = engine_assets::AssetManager::in_memory();
    let world = cameras(1);
    let scene = extract_scene(&world, &assets, SIZE).expect("camera");
    assert!(scene.debug_lines.is_empty());
    assert_eq!(scene.stats.debug_segments, 0);

    let mut world = cameras(2);
    world.insert_resource(DebugDraw::default());
    let scene = extract_scene(&world, &assets, SIZE).expect("cameras");
    assert_eq!(scene.passes.len(), 2);
    assert!(scene.debug_lines.is_empty());
    assert_eq!(scene.stats.debug_segments, 0);
}

#[test]
fn cameras_share_one_copied_list() {
    let assets = engine_assets::AssetManager::in_memory();
    let mut world = cameras(2);
    world.insert_resource(DebugDraw::default());
    world.resource_mut::<DebugDraw>().line(
        Vec3::new(-1.0, 2.0, 3.0),
        Vec3::new(4.0, 5.0, 6.0),
        Color::rgb(0.25, 0.5, 0.75),
    );
    world.resource_mut::<DebugDraw>().ray(Vec3::ZERO, Vec3::new(0.0, 0.0, 2.0), 3.0, Color::WHITE);

    let scene = extract_scene(&world, &assets, SIZE).expect("cameras");
    assert_eq!(scene.passes.len(), 2);
    assert_eq!(scene.debug_lines.len(), 2);
    assert_eq!(scene.stats.debug_segments, 2, "the count is once per frame, not once per camera");
    assert!(scene.debug_lines[0].start.approx_eq(Vec3::new(-1.0, 2.0, 3.0), 1e-5));
    assert!(scene.debug_lines[0].end.approx_eq(Vec3::new(4.0, 5.0, 6.0), 1e-5));
    assert_eq!(scene.debug_lines[0].color, [0.25, 0.5, 0.75, 1.0]);
    assert!(scene.debug_lines[1].end.approx_eq(Vec3::new(0.0, 0.0, 3.0), 1e-4));

    world.resource_mut::<DebugDraw>().clear();
    assert_eq!(scene.debug_lines.len(), 2, "extraction copied the segments");
    let again = extract_scene(&world, &assets, SIZE).expect("cameras");
    assert!(again.debug_lines.is_empty());
    assert_eq!(again.passes.len(), 2);
}
