//! The same world under every camera configuration: what stays coherent and what does not.
//!
//! GPU tests skip without an adapter unless `DIMORA_REQUIRE_GPU=1`, like the engine's own. Tests named
//! `known_conflict_*` pin a behaviour that is a recorded rendering conflict (docs/SPATIAL_RENDERING.md,
//! "Findings"). They are expected to fail the day a mechanism removes the conflict, and then the finding and
//! the test should be updated together.

mod common;

use common::*;
use engine::prelude::*;
use spatial_scene::cameras::{self, NAMED};
use spatial_scene::scene::{self, WALK_RANGE, WALK_Z};
use std::time::Duration;

// ---- one world, many cameras (no GPU needed) ---------------------------------------------------------

/// Everything that is placed in the world, as comparable data: sprites, meshes and tile maps.
fn placed(game: &Game) -> Vec<(String, [u32; 3])> {
    let mut out: Vec<(String, [u32; 3])> = game
        .world
        .query::<(&GlobalTransform, Option<&Name>, Option<&Sprite>, Option<&Mesh>)>()
        .iter()
        .filter(|(_, (_, _, s, m))| s.is_some() || m.is_some())
        .map(|(e, (g, n, _, _))| {
            let p = g.translation();
            (n.map_or(format!("#{}", e.id()), |n| n.0.clone()), [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()])
        })
        .collect();
    out.sort();
    out
}

#[test]
fn every_configuration_shows_the_same_world() {
    let reference = placed(&game(0));
    assert!(reference.len() > 400, "ground, sprites and meshes: {}", reference.len());
    for (i, name) in config_names() {
        let g = game(i);
        assert_eq!(placed(&g), reference, "{name}: the placed content differs");
        let scene = g.extract().unwrap_or_else(|| panic!("{name}: no active camera"));
        assert_eq!(scene.stats.missing_assets, 0, "{name}");
        assert!(scene.stats.sprites_visible > 0 && scene.stats.meshes_visible > 0, "{name}: {:?}", scene.stats);
    }
}

#[test]
fn a_configuration_is_a_camera_and_a_transform_over_one_projection_set() {
    let mut perspective = 0;
    for (name, make) in NAMED {
        let (camera, transform) = make();
        perspective += camera.projection.is_perspective() as u32;
        assert!(camera.active && transform.scale == Vec3::ONE, "{name}: an ordinary camera with an ordinary transform");
    }
    assert_eq!(perspective, 2, "perspective and steep are perspective; four are orthographic; custom is neither");
    let (custom, _) = NAMED[cameras::by_name("custom").unwrap()].1();
    assert!(matches!(custom.projection, CameraProjection::Custom(_)));
    assert!(!custom.projection.is_perspective());
}

#[test]
fn projecting_a_world_point_and_picking_through_its_pixel_agree_in_every_configuration() {
    for (i, name) in config_names() {
        let g = game(i);
        let view = view(&g);
        for label in ["figure A", "figure B", "figure C", "pillar", "cube", "tree 0"] {
            let point = position(&g, label) + Vec3::Y * 30.0;
            let Some(p) = view.project(point) else { continue };
            if !p.is_visible() {
                continue;
            }
            let (origin, dir) = view.ray_from_pixels(p.pixels).expect("invertible");
            let along = (point - origin).dot(dir);
            let miss = (origin + dir * along - point).length();
            assert!(miss < 0.5, "{name}: the ray through the pixel of `{label}` misses it by {miss}");
        }
    }
}

#[test]
fn the_walker_moves_by_a_plain_transform_write_and_stays_on_its_line() {
    let mut g = game(0);
    let start = position(&g, "walker");
    let mut xs = vec![start.x];
    for _ in 0..16 {
        g.step(Duration::from_millis(500));
        let p = position(&g, "walker");
        assert_eq!((p.y, p.z), (0.0, WALK_Z), "only X changes");
        assert!(p.x.abs() <= WALK_RANGE + 1e-3);
        xs.push(p.x);
    }
    assert!(xs.windows(2).all(|w| w[0] != w[1]), "it moves every step: {xs:?}");
    assert!(xs.iter().any(|&x| x > 100.0) && xs.iter().any(|&x| x < -100.0), "it patrols both sides: {xs:?}");
}

/// Vertical extent in pixels of the one quad extracted for `name` with `orientation`, alone in the world.
fn quad_height_px(config: usize, name: &str, orientation: SpriteOrientation) -> f32 {
    let mut g = game(config);
    isolate(&mut g, &[name]);
    set_orientation(&mut g, name, orientation);
    g.step(Duration::from_millis(16));
    let scene = g.extract().unwrap();
    let [instance] = &scene.sole_pass().cutout_sprites.instances[..] else { panic!("expected exactly one quad") };
    let (view, m) = (view(&g), instance.model);
    let (bottom, top) = (m.transform_point3(Vec3::new(0.5, 0.0, 0.0)), m.transform_point3(Vec3::new(0.5, 1.0, 0.0)));
    let (b, t) = (view.project(bottom).unwrap(), view.project(top).unwrap());
    (b.pixels.y - t.pixels.y).abs()
}

#[test]
fn an_upright_sprite_is_foreshortened_by_the_cosine_of_the_pitch_in_orthographic_views() {
    // Orthographic, so the ratio to a billboard is exact: cos(pitch).
    for (name, pitch) in [("orthographic", 0.3_f32), ("isometric", cameras::ISOMETRIC_PITCH), ("side", 0.0)] {
        let i = cameras::by_name(name).unwrap();
        let ratio = quad_height_px(i, "tree 0", SpriteOrientation::Upright)
            / quad_height_px(i, "tree 0", SpriteOrientation::Billboard);
        assert!((ratio - pitch.cos()).abs() < 0.01, "{name}: ratio {ratio}, cos(pitch) {}", pitch.cos());
    }
}

#[test]
fn an_upright_sprite_collapses_to_a_line_when_the_camera_looks_straight_down() {
    let top_down = cameras::by_name("top-down").unwrap();
    let upright = quad_height_px(top_down, "tree 0", SpriteOrientation::Upright);
    let billboard = quad_height_px(top_down, "tree 0", SpriteOrientation::Billboard);
    assert!(upright < 0.5, "edge-on: {upright} px");
    assert!(billboard > 5.0, "a billboard faces the camera: {billboard} px");
    let steep = cameras::by_name("steep").unwrap();
    let ratio = quad_height_px(steep, "tree 0", SpriteOrientation::Upright)
        / quad_height_px(steep, "tree 0", SpriteOrientation::Billboard);
    assert!((0.4..0.65).contains(&ratio), "a 60 degree pitch roughly halves it: {ratio}");
}

// ---- depth in every configuration (GPU) --------------------------------------------------------------

/// A small blue unlit cube and a larger red cutout billboard on the camera's view axis, `near` and `far`
/// along it. Returns the pixel in the middle of the image and one beside the cube.
fn probe(config: usize, mesh_is_nearer: bool) -> Option<([u8; 4], [u8; 4])> {
    let mut g = game(config);
    let camera = g.world.resource::<scene::Scene>().camera;
    let t = *g.world.component::<Transform>(camera).unwrap();
    let (near, far) = (t.position + t.forward() * 100.0, t.position + t.forward() * 140.0);
    let (mesh_at, sprite_at) = if mesh_is_nearer { (near, far) } else { (far, near) };
    g.world.spawn((
        Transform::from_position(mesh_at).with_uniform_scale(0.2),
        Mesh::new("cube").with_material(Material::unlit(Color::rgb(0.0, 0.0, 1.0))),
    ));
    g.world.spawn((
        Transform::from_position(sprite_at),
        Sprite::new("swatch")
            .with_size(Vec2::splat(80.0))
            .with_anchor(Vec2::splat(0.5))
            .with_tint(Color::rgb(1.0, 0.0, 0.0))
            .cutout()
            .unlit(),
    ));
    g.step(Duration::from_millis(16));
    let img = render(&mut g)?;
    let beside = pixel_of(&g, sprite_at + t.right() * 30.0).unwrap();
    // The six built-in placements put the view axis at the framebuffer centre. The oblique
    // matrix shifts that axis, so the custom case reads the projected point instead.
    let centre = if matches!(g.world.component::<Camera>(camera).unwrap().projection, CameraProjection::Custom(_)) {
        let at = if mesh_is_nearer { mesh_at } else { sprite_at };
        let (x, y) = pixel_of(&g, at).unwrap();
        (x, y)
    } else {
        (W as i32 / 2, H as i32 / 2)
    };
    Some((px(&img, centre.0, centre.1), px(&img, beside.0, beside.1)))
}

#[test]
fn a_nearer_mesh_occludes_a_farther_sprite_and_the_reverse_in_every_configuration() {
    for (i, name) in config_names() {
        let Some((centre, beside)) = probe(i, true) else { return };
        assert!(near(centre, [0, 0, 255]), "{name}: the nearer cube hides the sprite behind it: {centre:?}");
        assert!(near(beside, [255, 0, 0]), "{name}: the sprite shows beside the cube: {beside:?}");
        let (centre, _) = probe(i, false).unwrap();
        assert!(near(centre, [255, 0, 0]), "{name}: the nearer sprite hides the cube behind it: {centre:?}");
    }
}

// ---- sprite orientation under each configuration (GPU) -----------------------------------------------

fn tree_pixels(config: usize, orientation: SpriteOrientation) -> Option<usize> {
    let mut g = game(config);
    isolate(&mut g, &["tree 0"]);
    set_orientation(&mut g, "tree 0", orientation);
    let base = position(&g, "tree 0");
    aim_at(&mut g, base + Vec3::Y * 80.0);
    let img = render(&mut g)?;
    Some(count(&img, is_green))
}

#[test]
fn orientation_modes_under_each_configuration() {
    for (i, name) in config_names() {
        let Some(billboard) = tree_pixels(i, SpriteOrientation::Billboard) else { return };
        let upright = tree_pixels(i, SpriteOrientation::Upright).unwrap();
        let fixed = tree_pixels(i, SpriteOrientation::Fixed).unwrap();
        assert!(billboard > 50, "{name}: a billboard faces every camera: {billboard}");
        if name == "top-down" {
            assert_eq!((upright, fixed), (0, 0), "a vertical quad is edge-on when looking straight down");
        } else {
            assert!(upright > 50 && fixed > 50, "{name}: upright {upright}, fixed {fixed}");
        }
    }
}

fn figure_pixels(config: usize, keep: &[&str]) -> Option<usize> {
    let mut g = game(config);
    isolate(&mut g, keep);
    let figure = entity(&g, "figure A");
    g.world.component_mut::<Sprite>(figure).unwrap().tint = Color::rgb(0.0, 0.2, 1.0);
    let base = position(&g, "figure A");
    aim_at(&mut g, base + Vec3::Y * 48.0);
    let img = render(&mut g)?;
    Some(count(&img, |p| p[2] as i32 > p[0] as i32 + 60 && p[2] as i32 > p[1] as i32 + 40))
}

#[test]
fn a_billboard_standing_on_a_mesh_is_cut_by_it_when_the_view_is_steep_enough() {
    for (i, name) in config_names() {
        let Some(alone) = figure_pixels(i, &["figure A"]) else { return };
        let with_pad = figure_pixels(i, &["figure A", "pad A"]).unwrap() as f32 / alone as f32;
        if name == "top-down" {
            assert!(with_pad < 0.7, "top-down: the billboard lies in the ground plane under the pad: {with_pad}");
        } else {
            assert!(with_pad > 0.9, "{name}: {with_pad}");
        }
    }
}

// ---- ordering of overlapping and coplanar content (GPU) ----------------------------------------------

fn bench(config: usize) -> Option<(Game, Vec<u8>)> {
    let mut g = game(config);
    aim_at(&mut g, BENCH_CENTRE);
    let img = render(&mut g)?;
    Some((g, img))
}

/// Overlap region of the decal pair at depth `z`: both squares are 70 wide and 30 apart in X.
fn overlap(g: &Game, img: &[u8], y: f32, z: f32) -> Vec<[u8; 4]> {
    sample_grid(g, img, y, (-563.0, -527.0), (z - 30.0, z + 30.0), 9)
}

#[test]
fn decals_a_little_apart_in_height_resolve_by_depth_in_every_configuration() {
    for (i, name) in looking_down() {
        let Some((g, img)) = bench(i) else { return };
        let stack = overlap(&g, &img, DECAL_TOP, 370.0);
        assert!(fraction(&stack, BLUE) >= 0.8, "{name}: the higher cutout decal wins: {:.2}", fraction(&stack, BLUE));
        let above = overlap(&g, &img, DECAL_TOP, 610.0);
        assert!(
            fraction(&above, CYAN_ON_ORANGE) >= 0.8,
            "{name}: a blended decal above a cutout one shows: {:.2}",
            fraction(&above, CYAN_ON_ORANGE)
        );
    }
}

const DECAL_TOP: f32 = scene::DECAL_LIFT + 1.0;

#[test]
fn equal_height_cutout_decals_are_decided_by_draw_order_not_by_space() {
    for (i, name) in looking_down() {
        let Some((g, img)) = bench(i) else { return };
        let p = overlap(&g, &img, scene::DECAL_LIFT, 250.0);
        assert!(
            fraction(&p, RED) >= 0.99,
            "{name}: the decal drawn first (a) keeps the overlap: {:.2}",
            fraction(&p, RED)
        );
    }
    // Swap the colours: the winner follows the entity, not the colour, so nothing spatial decided it.
    let top_down = cameras::by_name("top-down").unwrap();
    let mut g = game(top_down);
    let (a, b) = (entity(&g, "decal equal a"), entity(&g, "decal equal b"));
    g.world.component_mut::<Sprite>(a).unwrap().tint = Color::rgb(0.1, 0.8, 0.1);
    g.world.component_mut::<Sprite>(b).unwrap().tint = Color::rgb(1.0, 0.1, 0.1);
    aim_at(&mut g, BENCH_CENTRE);
    let Some(img) = render(&mut g) else { return };
    let p = overlap(&g, &img, scene::DECAL_LIFT, 250.0);
    assert!(fraction(&p, GREEN) >= 0.99, "the first entity still wins: {:.2}", fraction(&p, GREEN));
}

#[test]
fn known_conflict_a_decal_coplanar_with_a_mesh_is_hidden_or_shown_by_rounding() {
    let mut shown = Vec::new();
    for (i, name) in looking_down() {
        let Some((g, img)) = bench(i) else { return };
        let p = sample_grid(&g, &img, 0.0, (-585.0, -535.0), (95.0, 145.0), 9);
        shown.push((name, fraction(&p, RED)));
    }
    assert!(shown.iter().any(|&(_, f)| f < 0.1), "hidden by the slab under at least one configuration: {shown:?}");
    assert!(shown.iter().any(|&(_, f)| f > 0.9), "fully shown under at least one other: {shown:?}");
}

#[test]
fn known_conflict_a_blended_decal_at_the_same_height_as_a_cutout_one_depends_on_the_camera() {
    let mut visible = Vec::new();
    for (i, name) in looking_down() {
        let Some((g, img)) = bench(i) else { return };
        let p = overlap(&g, &img, scene::DECAL_LIFT, 490.0);
        visible.push((name, fraction(&p, CYAN_ON_ORANGE)));
    }
    assert!(visible.iter().any(|&(_, f)| f > 0.8), "blended decal visible under one configuration: {visible:?}");
    assert!(visible.iter().any(|&(_, f)| f < 0.2), "and hidden under another: {visible:?}");
}

/// Colour at the middle of the overlap of the "ordered" ghost pair, alone in the world, with explicit
/// `RenderOrder`s for `a` and `b` if given.
fn ghost_overlap(config: usize, order: Option<(RenderOrder, RenderOrder)>) -> Option<[u8; 4]> {
    let mut g = game(config);
    isolate(&mut g, &["ghost ordered a", "ghost ordered b"]);
    if let Some((oa, ob)) = order {
        let (ea, eb) = (entity(&g, "ghost ordered a"), entity(&g, "ghost ordered b"));
        g.world.insert(ea, (oa,)).unwrap();
        g.world.insert(eb, (ob,)).unwrap();
    }
    // The middle of both bodies: a billboard's up is the camera's up, not the world's.
    let at = Vec3::new(-345.0, 0.0, 380.0) + view(&g).up * 46.0;
    aim_at(&mut g, at);
    let img = render(&mut g)?;
    let p = pixel_of(&g, at).unwrap();
    Some(px(&img, p.0, p.1))
}

#[test]
fn render_order_layer_always_decides_between_overlapping_blended_sprites() {
    for (i, name) in config_names().filter(|(_, n)| *n != "top-down") {
        let Some(b_on_top) = ghost_overlap(i, Some((RenderOrder::new(0, 0), RenderOrder::new(1, 0)))) else { return };
        let a_on_top = ghost_overlap(i, Some((RenderOrder::new(1, 0), RenderOrder::new(0, 0)))).unwrap();
        assert_ne!(a_on_top, b_on_top, "{name}: layer dominates depth, whichever way the camera looks");
    }
}

#[test]
fn render_order_order_only_breaks_ties_at_equal_view_depth() {
    // The pair is at equal world depth (same Z). Whether their *view* depths tie depends on the camera: with
    // the camera yawed (isometric) they differ and `order` is not consulted. Where they tie exactly
    // (perspective, orthographic, side, and custom, which keeps the orthographic pose) `order` decides.
    // Under `steep` the depths agree only up to rounding.
    for name in ["perspective", "orthographic", "side", "isometric", "custom"] {
        let i = cameras::by_name(name).unwrap();
        let Some(natural) = ghost_overlap(i, None) else { return };
        let a_first = ghost_overlap(i, Some((RenderOrder::new(0, 1), RenderOrder::new(0, 0)))).unwrap();
        if name == "isometric" {
            assert_eq!(natural, a_first, "isometric: the view depths differ, so `order` changes nothing");
        } else {
            assert_ne!(natural, a_first, "{name}: at equal view depth `order` decides");
        }
    }
}

// ---- runtime motion shows up on screen (GPU) ---------------------------------------------------------

fn magenta(p: [u8; 4]) -> bool {
    p[0] as i32 > p[1] as i32 + 40 && p[2] as i32 > p[1] as i32 + 40
}

#[test]
fn the_walker_is_drawn_where_its_transform_says_it_is() {
    let mut g = game(0);
    isolate(&mut g, &["walker"]);
    let mut seen = Vec::new();
    for seconds in [0.5_f32, 3.85] {
        let t = g.world.resource::<Time>().elapsed_secs();
        g.step(Duration::from_secs_f32(seconds - if seen.is_empty() { t } else { 0.0 }));
        let at = position(&g, "walker") + Vec3::Y * 46.0;
        let Some(img) = render(&mut g) else { return };
        let (x, y) = pixel_of(&g, at).unwrap();
        assert!(magenta(px(&img, x, y)), "at x = {}: {:?}", at.x, px(&img, x, y));
        seen.push((x, count(&img, magenta)));
    }
    assert!((seen[0].0 - seen[1].0).abs() > 40, "it crossed the screen: {seen:?}");
    assert!(seen.iter().all(|&(_, n)| n > 100), "{seen:?}");
}

// ---- example-local custom projection ----------------------------------------------------------------

#[test]
fn cycling_appends_custom_and_wraps_to_perspective() {
    let g = game(0);
    let mut scene = g.world.resource_mut::<scene::Scene>();
    let side = cameras::by_name("side").unwrap();
    scene::select_config(&mut scene, side + 1);
    assert_eq!(scene.config_name(), "custom");
    assert!(matches!(scene.projection, CameraProjection::Custom(_)));
    let kept = scene::swap_builtin_projection(scene.projection);
    assert!(matches!(kept, CameraProjection::Custom(_)), "P does not replace the oblique matrix");
    let next = scene.config + 1;
    scene::select_config(&mut scene, next);
    assert_eq!((scene.config, scene.config_name()), (0, "perspective"));
    assert!(scene.projection.is_perspective());
    drop(scene);
    let ortho = cameras::orthographic().0.projection;
    assert!(scene::swap_builtin_projection(ortho).is_perspective());
}

#[test]
fn the_custom_configuration_shears_a_known_point_and_keeps_pose_on_the_transform() {
    let custom_i = cameras::by_name("custom").unwrap();
    let ortho_i = cameras::by_name("orthographic").unwrap();
    let custom = game(custom_i);
    let ortho = game(ortho_i);
    let point = position(&custom, "figure C") + Vec3::Y * 48.0;
    assert!(position(&ortho, "figure C").approx_eq(position(&custom, "figure C"), 1e-3));

    let cv = view(&custom);
    let ov = view(&ortho);
    let projected = cv.project(point).expect("figure C is in front of the custom camera");
    let ordinary = ov.project(point).expect("figure C is in front of the orthographic camera");
    assert!(projected.is_visible() && ordinary.is_visible());
    let shift = (projected.pixels - ordinary.pixels).length();
    assert!(shift > 20.0, "oblique shift is {shift} px");

    let (origin, dir) = cv.ray_from_pixels(projected.pixels).expect("the oblique matrix is invertible");
    let along = (point - origin).dot(dir);
    let miss = (origin + dir * along - point).length();
    assert!(miss < 0.5, "the showcase ray misses figure C by {miss}");

    let near = cv.project(position(&custom, "figure A") + Vec3::Y * 48.0).unwrap();
    let far = cv.project(position(&custom, "figure D") + Vec3::Y * 48.0).unwrap();
    assert!(near.is_visible() && far.is_visible());
    assert!(near.view_depth < far.view_depth && near.depth < far.depth);
    assert!((0.0..=1.0).contains(&near.depth) && (0.0..=1.0).contains(&far.depth));

    assert!(cv.position.approx_eq(ov.position, 1e-3));
    assert!(cv.forward.approx_eq(ov.forward, 1e-4));
    assert!(!cv.proj.approx_eq(&ov.proj, 1e-4));

    let (camera, transform) = cameras::custom();
    let home = GlobalTransform { matrix: transform.to_matrix() };
    let wide = CameraView::new(&camera, &home, Vec2::new(320.0, 180.0));
    let square = CameraView::new(&camera, &home, Vec2::new(400.0, 400.0));
    assert!(wide.proj.approx_eq(&square.proj, 0.0), "the custom matrix stays fixed when the viewport aspect changes");
    let sample = Vec3::new(34.0, 48.0, 300.0);
    let (a, b) = (wide.project(sample).unwrap(), square.project(sample).unwrap());
    assert!((a.ndc.x - b.ndc.x).abs() < 1e-4 && (a.ndc.y - b.ndc.y).abs() < 1e-4);
    assert!((a.pixels.x / 320.0 - b.pixels.x / 400.0).abs() < 1e-3);

    let moved = GlobalTransform { matrix: transform.with_position(transform.position + Vec3::X * 40.0).to_matrix() };
    let shifted = CameraView::new(&camera, &moved, Vec2::new(320.0, 180.0));
    assert!(shifted.proj.approx_eq(&wide.proj, 0.0));
    assert!(!shifted.view.approx_eq(&wide.view, 1e-3), "moving the camera changes the view, not the projection");

    let ortho_cam = cameras::orthographic().0;
    let oa = CameraView::new(&ortho_cam, &home, Vec2::new(320.0, 180.0));
    let ob = CameraView::new(&ortho_cam, &home, Vec2::new(400.0, 400.0));
    assert!(!oa.proj.approx_eq(&ob.proj, 1e-3), "orthographic still tracks aspect");

    let frustum = engine::renderer::Frustum::from_view_proj(&cv.view_proj);
    for label in ["figure A", "figure C", "pillar", "cube"] {
        assert!(frustum.intersects_sphere(position(&custom, label), 1.0), "{label} is inside the custom frustum");
    }
    assert!(!frustum.intersects_sphere(Vec3::new(8000.0, 60.0, 300.0), 10.0), "a point far to the side is culled");
}

#[test]
fn the_custom_camera_renders_sheared_scene_content() {
    let i = cameras::by_name("custom").unwrap();
    let mut g = game(i);
    let Some(img) = render(&mut g) else { return };
    let extracted = g.extract().unwrap();
    assert!(extracted.stats.sprites_visible > 0 && extracted.stats.meshes_visible > 0, "{:?}", extracted.stats);

    let pillar = position(&g, "pillar");
    let (cx, cy) = pixel_of(&g, pillar).expect("pillar projects");
    let pillar_px = px(&img, cx, cy);
    assert!(
        pillar_px[3] == 255 && pillar_px[0] > pillar_px[2] + 30,
        "opaque pillar at the projected centre ({cx},{cy}): {pillar_px:?}"
    );

    let ortho = game(cameras::by_name("orthographic").unwrap());
    let (ox, oy) = pixel_of(&ortho, pillar).unwrap();
    assert!((cx - ox).abs() + (cy - oy).abs() > 15, "custom ({cx},{cy}) vs orthographic ({ox},{oy})");
    let elsewhere = px(&img, ox, oy);
    let colour_delta: i32 = (0..3).map(|c| (pillar_px[c] as i32 - elsewhere[c] as i32).abs()).sum();
    assert!(colour_delta > 40, "the orthographic pixel is not the pillar: {elsewhere:?} vs {pillar_px:?}");

    let body = position(&g, "figure A") + Vec3::Y * 46.0;
    let (fx, fy) = pixel_of(&g, body).unwrap();
    let cutout = (-3..=3).any(|dy| {
        (-3..=3).any(|dx| {
            let p = px(&img, fx + dx, fy + dy);
            p[3] == 255 && p[0] > 150 && p[0] > p[2] + 25
        })
    });
    assert!(cutout, "cutout figure A is visible around its projected body ({fx},{fy})");

    let mut ghosts = game(i);
    isolate(&mut ghosts, &["ghost"]);
    let at = position(&ghosts, "ghost") + view(&ghosts).up * 46.0;
    let Some(ghost_img) = render(&mut ghosts) else { return };
    let (gx, gy) = pixel_of(&ghosts, at).unwrap();
    let blended = px(&ghost_img, gx, gy);
    let sky = px(&ghost_img, 1, 1);
    assert_ne!(blended, sky, "the blended ghost changes the clear colour");
    assert!(blended[2] > blended[0], "the pale ghost stays blue over the sky: {blended:?}, sky {sky:?}");
}
