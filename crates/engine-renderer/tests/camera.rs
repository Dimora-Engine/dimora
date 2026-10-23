//! The spatial pipeline, stage by stage:
//!
//! ```text
//! world -> view (camera Transform) -> clip (Projection) -> NDC -> viewport pixels
//! ```
//!
//! plus how sprite orientation turns an entity into world-space quad geometry. These are math and
//! extraction tests; nothing here touches a GPU.

use core::f32::consts::{FRAC_PI_2, FRAC_PI_4};

use engine_assets::{AssetManager, MeshData, SpriteAsset, Texture};
use engine_math::{Mat4, Quat, Transform, Vec2, Vec3, Vec4};
use engine_renderer::*;
use engine_world::{propagate_transforms, GlobalTransform, World};

const VIEWPORT: Vec2 = Vec2::new(800.0, 400.0); // aspect 2

fn view_of(camera: Camera, transform: Transform) -> CameraView {
    CameraView::new(&camera, &GlobalTransform { matrix: transform.to_matrix() }, VIEWPORT)
}

fn view_of_default(camera: Camera) -> CameraView {
    view_of(camera, Transform::IDENTITY)
}

// ---------------------------------------------------------------------------------------------
// Conventions
// ---------------------------------------------------------------------------------------------

#[test]
fn conventions_x_right_y_up_z_forward() {
    let view = view_of_default(Camera::orthographic(10.0));
    assert_eq!((view.right, view.up, view.forward), (Vec3::X, Vec3::Y, Vec3::Z));

    let origin = view.project(Vec3::new(0.0, 0.0, 10.0)).unwrap();
    let right = view.project(Vec3::new(1.0, 0.0, 10.0)).unwrap();
    let up = view.project(Vec3::new(0.0, 1.0, 10.0)).unwrap();
    assert!(right.ndc.x > origin.ndc.x, "+X is screen right");
    assert!(right.pixels.x > origin.pixels.x);
    assert!(up.ndc.y > origin.ndc.y, "+Y is NDC up");
    assert!(up.pixels.y < origin.pixels.y, "+Y is screen up (pixel rows grow downward)");
}

#[test]
fn yaw_turns_the_camera_towards_x() {
    let t = Transform::IDENTITY.with_rotation(Quat::from_rotation_y(FRAC_PI_2));
    let view = view_of(Camera::perspective(60.0), t);
    assert!(view.forward.approx_eq(Vec3::X, 1e-5));
    // A point on the +X axis is now straight ahead.
    let p = view.project(Vec3::new(20.0, 0.0, 0.0)).unwrap();
    assert!(p.ndc.x.abs() < 1e-4 && p.ndc.y.abs() < 1e-4);
    assert!((p.view_depth - 20.0).abs() < 1e-4);
}

// ---------------------------------------------------------------------------------------------
// View matrix
// ---------------------------------------------------------------------------------------------

#[test]
fn view_matrix_is_the_inverse_of_the_camera_transform() {
    let t = Transform::looking_at(Vec3::new(3.0, 2.0, -10.0), Vec3::new(0.0, 0.0, 5.0), Vec3::Y);
    let view = view_of(Camera::perspective(60.0), t);
    assert!((view.view * t.to_matrix()).approx_eq(&Mat4::IDENTITY, 1e-4));
    // The camera's own position is the view-space origin, and points ahead have +Z view depth.
    assert!(view.view.transform_point3(t.position).approx_eq(Vec3::ZERO, 1e-4));
    assert!(view.view.transform_point3(t.position + t.forward() * 7.0).approx_eq(Vec3::new(0.0, 0.0, 7.0), 1e-4));
}

#[test]
fn view_translation_only_camera() {
    let t = Transform::from_xyz(3.0, 2.0, -10.0);
    let view = view_of(Camera::perspective(60.0), t);
    assert!(view.view.transform_point3(Vec3::new(3.0, 2.0, 0.0)).approx_eq(Vec3::new(0.0, 0.0, 10.0), 1e-5));
    assert!(view.view.transform_point3(Vec3::new(4.0, 2.0, 0.0)).approx_eq(Vec3::new(1.0, 0.0, 10.0), 1e-5));
}

// ---------------------------------------------------------------------------------------------
// Orthographic projection
// ---------------------------------------------------------------------------------------------

#[test]
fn orthographic_known_points() {
    // 10 units tall, aspect 2 -> 20 wide. Visible extent: x in [-10, 10], y in [-5, 5].
    let view = view_of_default(Camera::orthographic(10.0));
    for z in [1.0, 50.0, 800.0] {
        let p = view.project(Vec3::new(5.0, 2.5, z)).unwrap();
        assert!((p.ndc.x - 0.5).abs() < 1e-5 && (p.ndc.y - 0.5).abs() < 1e-5, "z={z}: {p:?}");
        assert!((p.pixels.x - 600.0).abs() < 1e-3 && (p.pixels.y - 100.0).abs() < 1e-3, "z={z}: {p:?}");
    }
    let corner = view.project(Vec3::new(-10.0, 5.0, 3.0)).unwrap();
    assert!((corner.ndc.x + 1.0).abs() < 1e-5 && (corner.ndc.y - 1.0).abs() < 1e-5);
    assert!(corner.pixels.approx_eq(Vec2::ZERO, 1e-3), "top-left pixel");
}

#[test]
fn orthographic_apparent_size_does_not_depend_on_depth() {
    let view = view_of_default(Camera::orthographic(10.0));
    let width_at = |z: f32| {
        let a = view.project(Vec3::new(-1.0, 0.0, z)).unwrap();
        let b = view.project(Vec3::new(1.0, 0.0, z)).unwrap();
        b.pixels.x - a.pixels.x
    };
    let (near, far) = (width_at(1.0), width_at(900.0));
    assert!((near - far).abs() < 1e-3, "{near} vs {far}");
}

// ---------------------------------------------------------------------------------------------
// Perspective projection
// ---------------------------------------------------------------------------------------------

#[test]
fn perspective_known_points() {
    // 90 degree vertical fov: a point at height y and depth z sits at ndc.y = y / z, and
    // ndc.x = x / (z * aspect).
    let view = view_of_default(Camera::perspective(90.0));
    let p = view.project(Vec3::new(10.0, 5.0, 10.0)).unwrap();
    assert!((p.ndc.y - 0.5).abs() < 1e-5, "{p:?}");
    assert!((p.ndc.x - 0.5).abs() < 1e-5, "{p:?}"); // 10 / (10 * 2)
                                                    // The same ray at twice the distance lands on the same pixel.
    let q = view.project(Vec3::new(20.0, 10.0, 20.0)).unwrap();
    assert!(p.pixels.approx_eq(q.pixels, 1e-3));
    // On the view axis: centre of the viewport.
    let centre = view.project(Vec3::new(0.0, 0.0, 37.0)).unwrap();
    assert!(centre.pixels.approx_eq(Vec2::new(400.0, 200.0), 1e-3));
}

#[test]
fn perspective_distant_objects_appear_smaller() {
    let view = view_of(Camera::perspective(60.0), Transform::from_xyz(0.0, 0.0, -300.0));
    // An object 64 units tall at world Z = 0, 100, 300, 600.
    let heights: Vec<f32> = [0.0, 100.0, 300.0, 600.0]
        .iter()
        .map(|&z| {
            let bottom = view.project(Vec3::new(0.0, 0.0, z)).unwrap();
            let top = view.project(Vec3::new(0.0, 64.0, z)).unwrap();
            bottom.pixels.y - top.pixels.y
        })
        .collect();
    assert!(heights.windows(2).all(|w| w[0] > w[1]), "{heights:?}");
    // Apparent size is inversely proportional to distance from the camera (300, 400, 600, 900).
    assert!((heights[0] / heights[3] - 3.0).abs() < 0.01, "{heights:?}");
    assert!((heights[0] / heights[1] - 4.0 / 3.0).abs() < 0.01, "{heights:?}");
}

#[test]
fn depth_buffer_values_are_ordered_by_distance() {
    for camera in [Camera::perspective(60.0), Camera::orthographic(20.0)] {
        let view = view_of(camera, Transform::from_xyz(0.0, 0.0, -10.0));
        let depths: Vec<f32> = [-5.0, 0.0, 10.0, 100.0, 600.0]
            .iter()
            .map(|&z| view.project(Vec3::new(0.0, 0.0, z)).unwrap().depth)
            .collect();
        assert!(depths.windows(2).all(|w| w[0] > w[1]), "{camera:?}: {depths:?}");
        assert!(depths.iter().all(|d| (0.0..=1.0).contains(d)));
    }
}

#[test]
fn near_and_far_planes_map_to_one_and_zero() {
    let near_far = |camera: Camera| {
        let view = view_of_default(camera);
        let (n, f) = match camera.projection {
            CameraProjection::Perspective(p) => (p.near, p.far),
            CameraProjection::Orthographic(o) => (o.near, o.far),
            _ => unreachable!(),
        };
        (view.project(Vec3::new(0.0, 0.0, n)).unwrap().depth, view.project(Vec3::new(0.0, 0.0, f)).unwrap().depth)
    };
    for camera in [Camera::perspective(60.0), Camera::orthographic(10.0)] {
        let (near, far) = near_far(camera);
        assert!((near - 1.0).abs() < 1e-4 && far.abs() < 1e-4, "{camera:?}: {near} {far}");
    }
}

#[test]
fn points_behind_the_camera_do_not_project() {
    let view = view_of_default(Camera::perspective(60.0));
    assert!(view.project(Vec3::new(0.0, 0.0, -1.0)).is_none());
    assert!(!view.project(Vec3::new(0.0, 0.0, 2000.0)).unwrap().is_visible(), "beyond far");
}

// ---------------------------------------------------------------------------------------------
// Combined view-projection
// ---------------------------------------------------------------------------------------------

#[test]
fn view_proj_is_projection_times_view_and_matches_the_trait() {
    let camera = Camera::perspective(70.0);
    let t = Transform::looking_at(Vec3::new(2.0, 8.0, -20.0), Vec3::new(0.0, 0.0, 10.0), Vec3::Y);
    let view = view_of(camera, t);
    assert!(view.view_proj.approx_eq(&(view.proj * view.view), 1e-5));

    let p = Vec3::new(3.0, -1.0, 12.0);
    let clip = view.view_proj * p.extend(1.0);
    let ndc = Vec2::new(clip.x / clip.w, clip.y / clip.w);
    let screen = view.project(p).unwrap();
    assert!(screen.ndc.approx_eq(ndc, 1e-5));
    // The Projection trait path (what other projections implement) agrees with the camera view.
    assert!(camera.projection.project(p, &view).unwrap().pixels.approx_eq(screen.pixels, 1e-3));
}

#[test]
fn moving_the_camera_changes_view_depth_for_every_object_equally() {
    let objects = [0.0f32, 100.0, 300.0, 600.0];
    let depth = |cam_z: f32, z: f32| {
        view_of(Camera::perspective(60.0), Transform::from_xyz(0.0, 0.0, cam_z))
            .project(Vec3::new(0.0, 0.0, z))
            .unwrap()
            .view_depth
    };
    for z in objects {
        assert!((depth(-300.0, z) - (z + 300.0)).abs() < 1e-3);
        // Dolly the camera 100 units closer: every object gets exactly 100 closer.
        assert!((depth(-300.0, z) - depth(-200.0, z) - 100.0).abs() < 1e-3);
    }
}

#[test]
fn changing_z_changes_camera_space_depth() {
    let view =
        view_of(Camera::perspective(60.0), Transform::looking_at(Vec3::new(0.0, 5.0, -20.0), Vec3::ZERO, Vec3::Y));
    let mut previous = f32::NEG_INFINITY;
    for z in [0.0, 1.0, 10.0, 100.0] {
        let d = view.view.transform_point3(Vec3::new(2.0, 0.0, z)).z;
        assert!(d > previous, "depth must grow with world Z");
        previous = d;
    }
}

#[test]
fn camera_rotation_produces_screen_motion_but_not_perspective_scaling() {
    let p = Vec3::new(0.0, 0.0, 100.0);
    let size = |yaw: f32| {
        let view = view_of(Camera::perspective(60.0), Transform::IDENTITY.with_rotation(Quat::from_rotation_y(yaw)));
        view.project(p).map(|s| (s.pixels.x, s.view_depth))
    };
    let (x0, d0) = size(0.0).unwrap();
    let (x1, d1) = size(0.1).unwrap();
    assert!(x1 < x0, "turning right moves the object left on screen");
    assert!((d0 - 100.0).abs() < 1e-3 && d1 < d0 && d1 > 99.0);
}

// ---------------------------------------------------------------------------------------------
// Viewport
// ---------------------------------------------------------------------------------------------

#[test]
fn viewport_rect_resolves_against_the_target_and_drives_aspect() {
    let camera = Camera::perspective(60.0).with_viewport(ViewportRect::new(0.5, 0.0, 0.5, 1.0));
    let view = CameraView::new(&camera, &GlobalTransform::default(), VIEWPORT);
    assert_eq!((view.viewport_origin, view.viewport), (Vec2::new(400.0, 0.0), Vec2::new(400.0, 400.0)));
    assert!((view.aspect() - 1.0).abs() < 1e-6, "aspect comes from the sub-rectangle");
    // The view axis lands in the middle of the sub-rectangle (pixels are viewport-relative).
    assert!(view.project(Vec3::new(0.0, 0.0, 10.0)).unwrap().pixels.approx_eq(Vec2::new(200.0, 200.0), 1e-3));
}

#[test]
fn viewport_rect_is_clamped_to_the_target() {
    let (origin, size) = ViewportRect::new(0.9, 0.9, 0.5, 0.5).to_pixels(Vec2::new(100.0, 100.0));
    assert_eq!((origin, size), (Vec2::new(90.0, 90.0), Vec2::new(10.0, 10.0)));
    let (_, size) = ViewportRect::new(0.0, 0.0, 0.0, 0.0).to_pixels(Vec2::new(100.0, 100.0));
    assert_eq!(size, Vec2::ONE);
}

// ---------------------------------------------------------------------------------------------
// Sprite orientation: Transform + Sprite + Camera -> world-space quad
// ---------------------------------------------------------------------------------------------

fn assets() -> AssetManager {
    let mut a = AssetManager::in_memory();
    let t = a.insert("tex", Texture::solid(16, 16, [255; 4])).unwrap();
    a.insert("tex_sprite", SpriteAsset::whole_texture(t, 16, 16)).unwrap();
    a.insert("cube", MeshData::cuboid(Vec3::ONE).unwrap()).unwrap();
    a
}

/// Extracts one sprite and returns its quad corners: (bottom-left, bottom-right, top-left).
fn quad(camera: Transform, entity: Transform, sprite: Sprite) -> (Vec3, Vec3, Vec3) {
    let a = assets();
    let mut w = World::new();
    w.spawn((Camera::perspective(60.0), camera));
    w.spawn((entity, sprite.with_size(Vec2::new(2.0, 4.0)).with_anchor(Vec2::new(0.5, 0.0))));
    propagate_transforms(&mut w);
    let scene = extract_scene(&w, &a, VIEWPORT).expect("camera");
    let m = scene.sole_pass().blended_sprites.instances[0].model;
    (
        m.transform_point3(Vec3::ZERO),
        m.transform_point3(Vec3::new(1.0, 0.0, 0.0)),
        m.transform_point3(Vec3::new(0.0, 1.0, 0.0)),
    )
}

fn pitched_camera() -> Transform {
    // Looking forward and 40 degrees down, yawed a little.
    Transform::from_xyz(3.0, 12.0, -20.0).with_rotation(Quat::from_rotation_y(0.3) * Quat::from_rotation_x(0.7))
}

#[test]
fn fixed_sprites_follow_the_entity_transform_exactly() {
    let entity = Transform::from_xyz(1.0, 2.0, 30.0).with_rotation(Quat::from_rotation_y(FRAC_PI_4));
    let (bl, br, tl) = quad(pitched_camera(), entity, Sprite::new("tex_sprite").fixed());
    let expect = |p: Vec3| entity.transform_point(p);
    assert!(bl.approx_eq(expect(Vec3::new(-1.0, 0.0, 0.0)), 1e-4));
    assert!(br.approx_eq(expect(Vec3::new(1.0, 0.0, 0.0)), 1e-4));
    assert!(tl.approx_eq(expect(Vec3::new(-1.0, 4.0, 0.0)), 1e-4));
}

#[test]
fn billboards_face_the_camera_wherever_the_entity_points() {
    let cam = pitched_camera();
    let view = view_of(Camera::perspective(60.0), cam);
    // Entity rotation about X and Y must not matter to a billboard.
    for rotation in [Quat::IDENTITY, Quat::from_rotation_x(1.2), Quat::from_rotation_y(2.0)] {
        let entity = Transform::from_xyz(1.0, 2.0, 30.0).with_rotation(rotation);
        let (bl, br, tl) = quad(cam, entity, Sprite::new("tex_sprite"));
        let (x, y) = (br - bl, tl - bl);
        assert!((x.length() - 2.0).abs() < 1e-4 && (y.length() - 4.0).abs() < 1e-4, "size is preserved");
        assert!(x.normalized().approx_eq(view.right, 1e-4), "quad X is the camera's right: {x:?}");
        assert!(y.normalized().approx_eq(view.up, 1e-4), "quad Y is the camera's up: {y:?}");
        // The anchor (bottom-centre) sits exactly at the entity's position.
        assert!(((bl + br) * 0.5).approx_eq(entity.position, 1e-4));
    }
}

#[test]
fn billboard_position_is_spatial_but_orientation_is_shared() {
    // Two billboards far apart in world space get the same orientation (view-plane aligned), so a
    // perspective camera does not rotate or skew individual sprites.
    let cam = pitched_camera();
    let ahead = |distance: f32, sideways: f32| cam.position + cam.forward() * distance + cam.right() * sideways;
    let (a0, a1, _) = quad(cam, Transform::from_position(ahead(30.0, -8.0)), Sprite::new("tex_sprite"));
    let (b0, b1, _) = quad(cam, Transform::from_position(ahead(70.0, 12.0)), Sprite::new("tex_sprite"));
    assert!((a1 - a0).normalized().approx_eq((b1 - b0).normalized(), 1e-5));
}

#[test]
fn billboards_keep_roll_and_mirroring() {
    let cam = Transform::IDENTITY;
    let rolled = Transform::from_xyz(0.0, 0.0, 10.0).with_rotation(Quat::from_rotation_z(FRAC_PI_2));
    let (bl, br, tl) = quad(cam, rolled, Sprite::new("tex_sprite"));
    // Rolled 90 degrees about the view axis: the quad's X axis now points along world +Y.
    assert!((br - bl).normalized().approx_eq(Vec3::Y, 1e-4), "{:?}", br - bl);
    assert!((tl - bl).normalized().approx_eq(-Vec3::X, 1e-4));

    let mirrored = Transform::from_xyz(0.0, 0.0, 10.0).with_scale(Vec3::new(-1.0, 1.0, 1.0));
    let (bl, br, _) = quad(cam, mirrored, Sprite::new("tex_sprite"));
    assert!((br - bl).normalized().approx_eq(-Vec3::X, 1e-4), "negative scale mirrors the sprite");
}

#[test]
fn upright_sprites_stay_vertical_under_a_tilted_camera() {
    let cam = pitched_camera();
    let view = view_of(Camera::perspective(60.0), cam);
    let (bl, br, tl) = quad(cam, Transform::from_xyz(0.0, 0.0, 30.0), Sprite::new("tex_sprite").upright());
    let (x, y) = ((br - bl).normalized(), (tl - bl).normalized());
    assert!(y.approx_eq(Vec3::Y, 1e-5), "Y stays world up: {y:?}");
    assert!(x.y.abs() < 1e-5, "X is horizontal: {x:?}");
    assert!(x.dot(view.forward).abs() < 1e-4, "the quad is perpendicular to the view's horizontal heading");
}

#[test]
fn sprite_normal_faces_the_camera() {
    // Lighting and culling treat the quad as a surface: its normal -cross(X, Y) must point back at
    // the camera for every facing mode that is meant to face it.
    let cam = pitched_camera();
    let view = view_of(Camera::perspective(60.0), cam);
    for sprite in [Sprite::new("tex_sprite"), Sprite::new("tex_sprite").upright()] {
        let (bl, br, tl) = quad(cam, Transform::from_xyz(0.0, 0.0, 30.0), sprite);
        let normal = -(br - bl).cross(tl - bl).normalized();
        assert!(normal.dot(view.forward) < -0.5, "{sprite:?}: {normal:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Sprite fields: region, flips, tint carry through; RenderOrder is optional
// ---------------------------------------------------------------------------------------------

#[test]
fn sprite_region_selects_a_sub_rectangle_of_the_asset() {
    let a = assets();
    let mut w = World::new();
    w.spawn((Camera::perspective(60.0), Transform::IDENTITY));
    let region = engine_math::Rect::new(Vec2::new(0.5, 0.0), Vec2::new(1.0, 0.5));
    w.spawn((Transform::from_xyz(0.0, 0.0, 10.0), Sprite::new("tex_sprite").with_region(region).with_flip_x(true)));
    propagate_transforms(&mut w);
    let scene = extract_scene(&w, &a, VIEWPORT).unwrap();
    // Right-bottom quadrant (UV v grows downward in the asset), mirrored horizontally.
    assert_eq!(scene.sole_pass().blended_sprites.instances[0].uv, [1.0, 0.0, 0.5, 0.5]);
}

#[test]
fn entities_without_render_order_are_ordered_by_depth_alone() {
    let a = assets();
    let mut w = World::new();
    w.spawn((Camera::perspective(60.0), Transform::IDENTITY));
    for z in [10.0, 30.0, 20.0] {
        w.spawn((Transform::from_xyz(0.0, 0.0, z), Sprite::new("tex_sprite")));
    }
    propagate_transforms(&mut w);
    let scene = extract_scene(&w, &a, VIEWPORT).unwrap();
    let z: Vec<f32> = scene.sole_pass().blended_sprites.instances.iter().map(|i| i.model.translation().z).collect();
    assert_eq!(z, vec![30.0, 20.0, 10.0]);
}

// ---------------------------------------------------------------------------------------------
// Sprites and meshes share a world and a camera
// ---------------------------------------------------------------------------------------------

#[test]
fn a_sprite_and_a_mesh_at_the_same_world_position_project_to_the_same_pixel() {
    let a = assets();
    let mut w = World::new();
    w.spawn((Camera::perspective(60.0), pitched_camera()));
    let at = Vec3::new(4.0, 1.0, 40.0);
    w.spawn((Transform::from_position(at), Mesh::new("cube")));
    w.spawn((
        Transform::from_position(at),
        Sprite::new("tex_sprite").with_anchor(Vec2::splat(0.5)).with_size(Vec2::ONE),
    ));
    propagate_transforms(&mut w);
    let scene = extract_scene(&w, &a, VIEWPORT).unwrap();

    let mesh_centre = scene.sole_pass().meshes[0].model.translation();
    let sprite_centre = scene.sole_pass().blended_sprites.instances[0].model.transform_point3(Vec3::new(0.5, 0.5, 0.0));
    assert!(mesh_centre.approx_eq(sprite_centre, 1e-4));
    let (m, s) =
        (scene.sole_pass().view.project(mesh_centre).unwrap(), scene.sole_pass().view.project(sprite_centre).unwrap());
    assert_eq!((m.pixels, m.depth), (s.pixels, s.depth), "one camera, one world");
}

#[test]
fn nearer_things_get_larger_depth_values_regardless_of_kind() {
    // GPU depth resolves mesh-vs-cutout-sprite visibility; it is purely a function of position.
    let a = assets();
    let mut w = World::new();
    w.spawn((Camera::perspective(60.0), Transform::from_xyz(0.0, 0.0, -50.0)));
    w.spawn((Transform::from_xyz(0.0, 0.0, 20.0), Mesh::new("cube")));
    w.spawn((Transform::from_xyz(0.0, 0.0, 10.0), Sprite::new("tex_sprite").cutout()));
    w.spawn((Transform::from_xyz(0.0, 0.0, 30.0), Sprite::new("tex_sprite").cutout()));
    propagate_transforms(&mut w);
    let scene = extract_scene(&w, &a, VIEWPORT).unwrap();
    let depth = |p: Vec3| scene.sole_pass().view.project(p).unwrap().depth;
    let mesh = depth(scene.sole_pass().meshes[0].model.translation());
    let sprite_depths: Vec<f32> =
        scene.sole_pass().cutout_sprites.instances.iter().map(|i| depth(i.model.translation())).collect();
    let (in_front, behind) = (
        sprite_depths.iter().cloned().fold(f32::MIN, f32::max),
        sprite_depths.iter().cloned().fold(f32::MAX, f32::min),
    );
    assert!(in_front > mesh && mesh > behind, "nearer reverse-Z depth is larger: {in_front} {mesh} {behind}");
    assert_eq!(scene.sole_pass().cutout_sprites.instances.len(), 2);
}

// ---------------------------------------------------------------------------------------------
// Custom projection
// ---------------------------------------------------------------------------------------------

fn finite_custom(matrix: Mat4) -> CameraProjection {
    CameraProjection::custom(matrix).unwrap()
}

fn oblique_shear(aspect: f32, shear_x: f32) -> Mat4 {
    let ortho = CameraProjection::orthographic(10.0).matrix(aspect);
    let shear = Mat4::from_cols(
        Vec4::new(1.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 1.0, 0.0, 0.0),
        Vec4::new(shear_x, 0.0, 1.0, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    );
    ortho * shear
}

fn assert_on_ray(view: &CameraView, point: Vec3) {
    let screen = view.project(point).expect("point projects");
    let (origin, direction) = view.ray_from_pixels(screen.pixels).expect("invertible");
    let along = (point - origin).dot(direction);
    let closest = origin + direction * along;
    assert!(closest.approx_eq(point, 1e-3), "ray misses {point:?} by {:?}", closest - point);
    assert!(along.is_finite() && direction.length() > 0.0);
}

#[test]
fn custom_matrix_is_returned_unchanged() {
    let matrix = oblique_shear(1.7, 0.35);
    let projection = finite_custom(matrix);
    assert!(projection.matrix(0.25).approx_eq(&matrix, 0.0));
    assert!(projection.matrix(4.0).approx_eq(&matrix, 0.0));
}

#[test]
fn custom_projection_ignores_aspect() {
    let matrix = CameraProjection::perspective(50.0).matrix(1.0);
    let narrow = CameraView::new(
        &Camera::new(finite_custom(matrix)),
        &GlobalTransform { matrix: Mat4::IDENTITY },
        Vec2::new(200.0, 800.0),
    );
    let wide = CameraView::new(
        &Camera::new(finite_custom(matrix)),
        &GlobalTransform { matrix: Mat4::IDENTITY },
        Vec2::new(800.0, 200.0),
    );
    assert!(narrow.proj.approx_eq(&matrix, 0.0));
    assert!(wide.proj.approx_eq(&narrow.proj, 0.0));
    assert!((narrow.aspect() - 0.25).abs() < 1e-5);
    assert!((wide.aspect() - 4.0).abs() < 1e-5);
}

#[test]
fn perspective_matrix_still_tracks_aspect() {
    let camera = Camera::perspective(60.0);
    let square = view_of(camera, Transform::IDENTITY);
    let wide = CameraView::new(&camera, &GlobalTransform { matrix: Mat4::IDENTITY }, Vec2::new(800.0, 200.0));
    let expected_square = camera.projection.matrix(2.0);
    let expected_wide = camera.projection.matrix(4.0);
    assert!(square.proj.approx_eq(&expected_square, 1e-6));
    assert!(wide.proj.approx_eq(&expected_wide, 1e-6));
    assert!(!square.proj.approx_eq(&wide.proj, 1e-3));
}

#[test]
fn orthographic_matrix_still_tracks_aspect() {
    let camera = Camera::orthographic(10.0);
    let square = CameraView::new(&camera, &GlobalTransform { matrix: Mat4::IDENTITY }, Vec2::new(400.0, 400.0));
    let wide = view_of(camera, Transform::IDENTITY);
    assert!(square.proj.approx_eq(&camera.projection.matrix(1.0), 1e-6));
    assert!(wide.proj.approx_eq(&camera.projection.matrix(2.0), 1e-6));
    assert!(!square.proj.approx_eq(&wide.proj, 1e-3));
}

#[test]
fn custom_rejects_nan_and_infinity() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut matrix = Mat4::IDENTITY;
        matrix.cols[2].z = bad;
        assert_eq!(CameraProjection::custom(matrix), Err(ProjectionError::NonFinite));
        matrix = oblique_shear(1.0, 0.2);
        matrix.cols[3].x = bad;
        assert_eq!(CameraProjection::custom(matrix), Err(ProjectionError::NonFinite));
    }
    assert!(CameraProjection::custom(oblique_shear(2.0, -0.4)).is_ok());
    assert!(CameraProjection::custom(Mat4::orthographic_lh(-1.0, 4.0, -2.0, 2.0, 0.1, 50.0)).is_ok());
}

#[test]
fn with_clip_range_leaves_custom_unchanged() {
    let matrix = CameraProjection::perspective(40.0).matrix(1.25);
    let projection = finite_custom(matrix);
    let clipped = projection.with_clip_range(2.0, 30.0);
    assert_eq!(projection, clipped);
    assert!(clipped.matrix(3.0).approx_eq(&matrix, 0.0));
    let builtin = CameraProjection::perspective(40.0).with_clip_range(2.0, 30.0);
    match builtin {
        CameraProjection::Perspective(p) => assert_eq!((p.near, p.far), (2.0, 30.0)),
        other => panic!("built-in clip range still edits the descriptor, got {other:?}"),
    }
}

#[test]
fn repeated_custom_projection_is_deterministic() {
    let camera = Camera::new(finite_custom(oblique_shear(2.0, 0.15)));
    let transform = Transform::looking_at(Vec3::new(1.0, 2.0, -8.0), Vec3::new(0.0, 1.0, 4.0), Vec3::Y);
    let first = view_of(camera, transform);
    let second = view_of(camera, transform);
    assert!(first.view.approx_eq(&second.view, 0.0));
    assert!(first.proj.approx_eq(&second.proj, 0.0));
    assert!(first.view_proj.approx_eq(&second.view_proj, 0.0));
}

#[test]
fn custom_equivalent_perspective_matches_builtin_clip() {
    let builtin = Camera::perspective(60.0);
    let custom = Camera::new(finite_custom(builtin.projection.matrix(VIEWPORT.x / VIEWPORT.y)));
    let transform = Transform::from_xyz(1.0, -2.0, -6.0);
    let left = view_of(builtin, transform);
    let right = view_of(custom, transform);
    assert!(left.proj.approx_eq(&right.proj, 1e-6));
    assert!(left.view.approx_eq(&right.view, 1e-6));
    let point = Vec3::new(2.0, 1.5, 9.0);
    let a = left.project(point).unwrap();
    let b = right.project(point).unwrap();
    assert!(a.pixels.approx_eq(b.pixels, 1e-3));
    assert!(a.ndc.approx_eq(b.ndc, 1e-5));
    assert!((a.depth - b.depth).abs() < 1e-5);
    assert!((a.view_depth - b.view_depth).abs() < 1e-4);
    assert_on_ray(&right, point);
    let (origin, direction) = right.ray_from_pixels(b.pixels).unwrap();
    let (builtin_origin, builtin_direction) = left.ray_from_pixels(a.pixels).unwrap();
    assert!(origin.approx_eq(builtin_origin, 1e-3));
    assert!(direction.approx_eq(builtin_direction, 1e-4));
}

#[test]
fn custom_equivalent_orthographic_matches_builtin_clip() {
    let builtin = Camera::orthographic(10.0);
    let custom = Camera::new(finite_custom(builtin.projection.matrix(VIEWPORT.x / VIEWPORT.y)));
    let left = view_of_default(builtin);
    let right = view_of_default(custom);
    let point = Vec3::new(-3.0, 2.0, 40.0);
    let a = left.project(point).unwrap();
    let b = right.project(point).unwrap();
    assert!(a.pixels.approx_eq(b.pixels, 1e-3));
    assert!((a.depth - b.depth).abs() < 1e-5);
    assert!((a.view_depth - point.z).abs() < 1e-4);
    assert_on_ray(&right, point);
    let (_, direction) = right.ray_from_pixels(b.pixels).unwrap();
    let (_, builtin_direction) = left.ray_from_pixels(a.pixels).unwrap();
    assert!(direction.approx_eq(builtin_direction, 1e-4));
    assert!(direction.approx_eq(Vec3::Z, 1e-4), "orthographic rays stay parallel to view forward");
}

#[test]
fn oblique_shear_moves_a_known_point() {
    let plain = view_of_default(Camera::orthographic(10.0));
    let sheared = view_of_default(Camera::new(finite_custom(oblique_shear(2.0, 0.1))));
    let point = Vec3::new(0.0, 0.0, 10.0);
    let shifted = plain.project(Vec3::new(1.0, 0.0, 10.0)).unwrap();
    let oblique = sheared.project(point).unwrap();
    let unshifted = plain.project(point).unwrap();
    assert!(oblique.ndc.approx_eq(shifted.ndc, 1e-4), "x' = x + 0.1 z");
    assert!((oblique.ndc.x - unshifted.ndc.x).abs() > 0.05);
    assert!((oblique.view_depth - 10.0).abs() < 1e-4, "view depth ignores the projection");
    assert!(oblique.depth > 0.0 && oblique.depth < 1.0);
}

#[test]
fn view_stays_transform_inverse_with_custom_projection() {
    let transform = Transform::looking_at(Vec3::new(3.0, 2.0, -10.0), Vec3::new(0.0, 0.0, 5.0), Vec3::Y);
    let view = view_of(Camera::new(finite_custom(oblique_shear(2.0, 0.2))), transform);
    assert!((view.view * transform.to_matrix()).approx_eq(&Mat4::IDENTITY, 1e-4));
    assert!(view.proj.approx_eq(&oblique_shear(2.0, 0.2), 1e-6));
    assert!(view.view_proj.approx_eq(&(view.proj * view.view), 1e-5));
    assert!(!view.view.approx_eq(&view.proj, 1e-2));
}

#[test]
fn singular_custom_matrix_ray_returns_none() {
    let singular = Mat4::from_cols(
        Vec4::new(1.0, 0.0, 0.0, 0.0),
        Vec4::new(2.0, 0.0, 0.0, 0.0),
        Vec4::new(3.0, 0.0, 0.0, 0.0),
        Vec4::new(4.0, 5.0, 6.0, 0.0),
    );
    assert!(singular.inverse().is_none());
    let view = view_of_default(Camera::new(finite_custom(singular)));
    assert!(view.ray_from_pixels(Vec2::new(10.0, 20.0)).is_none());
    let _ = view.project(Vec3::new(0.0, 0.0, 5.0));
    let mut world = World::new();
    world.spawn((Camera::new(finite_custom(singular)), Transform::IDENTITY));
    propagate_transforms(&mut world);
    let scene = extract_scene(&world, &AssetManager::in_memory(), VIEWPORT).expect("a singular matrix still extracts");
    assert!(scene.sole_pass().view.proj.approx_eq(&singular, 0.0));
}

#[test]
fn projected_point_is_on_custom_perspective_ray() {
    let view = view_of(
        Camera::new(finite_custom(CameraProjection::perspective(70.0).matrix(2.0))),
        Transform::from_xyz(-2.0, 1.0, -12.0),
    );
    assert_on_ray(&view, Vec3::new(4.0, -1.0, 6.0));
}

#[test]
fn projected_point_is_on_custom_orthographic_ray() {
    let view = view_of(
        Camera::new(finite_custom(CameraProjection::orthographic(16.0).matrix(2.0))),
        Transform::from_xyz(0.0, 3.0, -8.0),
    );
    assert_on_ray(&view, Vec3::new(-2.0, 4.0, 15.0));
}

#[test]
fn projected_point_is_on_oblique_custom_ray() {
    let view = view_of(
        Camera::new(finite_custom(oblique_shear(2.0, 0.25))),
        Transform::looking_at(Vec3::new(2.0, 4.0, -15.0), Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
    );
    let point = Vec3::new(1.0, 2.0, 3.0);
    assert_on_ray(&view, point);
    let plain = view_of(
        Camera::orthographic(10.0),
        Transform::looking_at(Vec3::new(2.0, 4.0, -15.0), Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
    );
    let oblique = view.project(point).unwrap();
    let built_in = plain.project(point).unwrap();
    assert!(
        (oblique.pixels.x - built_in.pixels.x).abs() > 1.0,
        "the oblique ray is not the orthographic ray: {:?} vs {:?}",
        oblique.pixels,
        built_in.pixels
    );
}

#[test]
fn translated_custom_camera_picking() {
    let camera = Camera::new(finite_custom(CameraProjection::orthographic(10.0).matrix(2.0)));
    let view = view_of(camera, Transform::from_xyz(5.0, -2.0, 3.0));
    assert_on_ray(&view, Vec3::new(6.0, -1.0, 20.0));
    assert!(view.view.transform_point3(Vec3::new(5.0, -2.0, 3.0)).approx_eq(Vec3::ZERO, 1e-4));
}

#[test]
fn rotated_custom_camera_picking() {
    let camera = Camera::new(finite_custom(CameraProjection::perspective(55.0).matrix(2.0)));
    let transform = Transform::IDENTITY.with_rotation(Quat::from_rotation_y(FRAC_PI_2));
    let view = view_of(camera, transform);
    assert!(view.forward.approx_eq(Vec3::X, 1e-5));
    assert_on_ray(&view, Vec3::new(12.0, 1.0, -2.0));
}

#[test]
fn parent_transform_changes_view_not_projection() {
    let matrix = oblique_shear(2.0, -0.2);
    let local = Transform::from_xyz(0.0, 1.0, -5.0);
    let parent = Transform::from_xyz(4.0, -2.0, 3.0).with_rotation(Quat::from_rotation_y(0.4));
    let mut world = World::new();
    let parent_entity = world.spawn((parent,));
    let camera = Camera::new(finite_custom(matrix));
    let child = world.spawn_child(parent_entity, (camera, local));
    propagate_transforms(&mut world);
    let global = *world.component::<GlobalTransform>(child).unwrap();
    let view = CameraView::new(&camera, &global, VIEWPORT);
    let composed = parent.to_matrix() * local.to_matrix();
    assert!((view.view * composed).approx_eq(&Mat4::IDENTITY, 1e-4));
    assert!(view.proj.approx_eq(&matrix, 1e-6));
    let point = parent.position + Vec3::new(0.0, 2.0, 8.0);
    assert_on_ray(&view, point);
}

#[test]
fn custom_projection_viewport_resize_policy() {
    let matrix = CameraProjection::perspective(60.0).matrix(2.0);
    let camera = Camera::new(finite_custom(matrix));
    let placed = GlobalTransform { matrix: Mat4::IDENTITY };
    let wide = CameraView::new(&camera, &placed, Vec2::new(800.0, 400.0));
    let square = CameraView::new(&camera, &placed, Vec2::new(400.0, 400.0));
    assert!(wide.proj.approx_eq(&matrix, 0.0));
    assert!(square.proj.approx_eq(&wide.proj, 0.0));
    let point = Vec3::new(1.0, 0.0, 8.0);
    let wide_px = wide.project(point).unwrap();
    let square_px = square.project(point).unwrap();
    assert!(wide_px.ndc.approx_eq(square_px.ndc, 1e-5), "NDC stays with the fixed matrix");
    assert!((wide_px.pixels.x - square_px.pixels.x).abs() > 1.0, "pixels follow the new viewport");
    assert_on_ray(&square, point);
    let perspective = Camera::perspective(60.0);
    let perspective_wide = CameraView::new(&perspective, &placed, Vec2::new(800.0, 400.0));
    let perspective_square = CameraView::new(&perspective, &placed, Vec2::new(400.0, 400.0));
    assert!(!perspective_wide.proj.approx_eq(&perspective_square.proj, 1e-3));
}

#[test]
fn custom_projection_subviewport_picking() {
    let matrix = CameraProjection::orthographic(8.0).matrix(1.0);
    let camera = Camera::new(finite_custom(matrix)).with_viewport(ViewportRect::new(0.25, 0.1, 0.5, 0.8));
    let view = CameraView::new(&camera, &GlobalTransform { matrix: Mat4::IDENTITY }, Vec2::new(800.0, 500.0));
    assert!(view.proj.approx_eq(&matrix, 0.0));
    assert!(view.viewport_origin.x > 0.0 && view.viewport.x < 800.0);
    assert_on_ray(&view, Vec3::new(1.0, -0.5, 6.0));
}

#[test]
fn extract_keeps_the_custom_view_projection() {
    let matrix = CameraProjection::orthographic(12.0).matrix(2.0);
    let camera = Camera::new(finite_custom(matrix));
    let transform = Transform::from_xyz(0.0, 0.0, -3.0);
    let mut world = World::new();
    world.spawn((camera, transform));
    propagate_transforms(&mut world);
    let scene = extract_scene(&world, &AssetManager::in_memory(), VIEWPORT).unwrap();
    let expected = view_of(camera, transform);
    assert!(scene.sole_pass().view.proj.approx_eq(&matrix, 0.0));
    assert!(scene.sole_pass().view.view_proj.approx_eq(&expected.view_proj, 1e-5));
    assert!(!camera.projection.is_perspective());
}
