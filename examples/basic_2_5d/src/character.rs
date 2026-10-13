//! A skeletal 2D character: a skeleton of bone entities with flat sprite parts attached.
//!
//! Parts are *spatial*: the far arm sits slightly behind the torso and the eyes slightly in front
//! of the face, expressed as small real Z offsets. The depth buffer resolves the overlap; there is
//! no z-index anywhere.

use engine::prelude::*;
use std::f32::consts::PI;

struct Part {
    bone: &'static str,
    offset: Vec3,
    size: Vec2,
    anchor: Vec2,
    color: Color,
}

pub struct Character {
    pub root: Entity,
    pub walk: AssetHandle<AnimationClip>,
    pub idle: AssetHandle<AnimationClip>,
}

fn skeleton() -> Skeleton {
    let mut s = Skeleton::new();
    let hips = s.add_bone("hips", None, Transform::from_xyz(0.0, 0.62, 0.0)).unwrap();
    let torso = s.add_bone("torso", Some(hips), Transform::IDENTITY).unwrap();
    s.add_bone("head", Some(torso), Transform::from_xyz(0.0, 0.72, 0.0)).unwrap();
    s.add_bone("arm_l", Some(torso), Transform::from_xyz(-0.30, 0.66, 0.0)).unwrap();
    s.add_bone("arm_r", Some(torso), Transform::from_xyz(0.30, 0.66, 0.0)).unwrap();
    s.add_bone("leg_l", Some(hips), Transform::from_xyz(-0.11, 0.0, 0.0)).unwrap();
    s.add_bone("leg_r", Some(hips), Transform::from_xyz(0.11, 0.0, 0.0)).unwrap();
    s
}

fn parts() -> Vec<Part> {
    let c = |r, g, b| Color::from_srgb8(r, g, b);
    let (skin, shirt, pants, hair, boots) =
        (c(236, 188, 150), c(66, 112, 200), c(52, 58, 84), c(86, 52, 34), c(40, 34, 32));
    let (top, mid) = (Vec2::new(0.5, 1.0), Vec2::new(0.5, 0.5));
    let bottom = Vec2::new(0.5, 0.0);
    let p = |bone, x, y, z, w, h, anchor, color| Part {
        bone,
        offset: Vec3::new(x, y, z),
        size: Vec2::new(w, h),
        anchor,
        color,
    };
    vec![
        p("arm_r", 0.0, 0.0, 0.07, 0.15, 0.56, top, shirt.scaled(0.7)),
        p("arm_r", 0.0, -0.56, 0.07, 0.15, 0.13, top, skin.scaled(0.75)),
        p("leg_r", 0.0, 0.0, 0.03, 0.18, 0.62, top, pants.scaled(0.75)),
        p("leg_r", 0.0, -0.62, 0.03, 0.2, 0.1, top, boots),
        p("leg_l", 0.0, 0.0, -0.01, 0.18, 0.62, top, pants),
        p("leg_l", 0.0, -0.62, -0.01, 0.2, 0.1, top, boots),
        p("torso", 0.0, 0.0, 0.0, 0.5, 0.74, bottom, shirt),
        p("head", 0.0, 0.0, -0.02, 0.46, 0.44, bottom, skin),
        p("head", 0.0, 0.3, -0.05, 0.5, 0.16, bottom, hair),
        p("head", -0.1, 0.17, -0.06, 0.07, 0.09, mid, Color::BLACK),
        p("head", 0.1, 0.17, -0.06, 0.07, 0.09, mid, Color::BLACK),
        p("arm_l", 0.0, 0.0, -0.09, 0.15, 0.56, top, shirt),
        p("arm_l", 0.0, -0.56, -0.09, 0.15, 0.13, top, skin),
    ]
}

/// `(time, position offset from rest, rotation about Z)`.
type Key3 = (f32, Vec3, f32);

fn clip(skeleton: &Skeleton, duration: f32, tracks: &[(&str, Vec<Key3>)]) -> AnimationClip {
    let tracks = tracks
        .iter()
        .map(|(bone, keys)| {
            let index = skeleton.index_of(bone).expect("bone exists");
            let rest = skeleton.bones()[index].rest;
            BoneTrack {
                bone: index,
                keys: keys
                    .iter()
                    .map(|&(time, offset, angle)| Keyframe {
                        time,
                        transform: Transform {
                            position: rest.position + offset,
                            rotation: Quat::from_rotation_z(angle),
                            ..rest
                        },
                    })
                    .collect(),
            }
        })
        .collect();
    AnimationClip { duration, looping: true, tracks }
}

pub fn spawn(world: &mut World, position: Vec3) -> Character {
    let skeleton = skeleton();
    let z = Vec3::ZERO;
    let swing = |a: f32| vec![(0.0, z, a), (0.32, z, -a), (0.64, z, a)];
    let walk = clip(
        &skeleton,
        0.64,
        &[
            ("leg_l", swing(0.6)),
            ("leg_r", swing(-0.6)),
            ("arm_l", swing(-0.7)),
            ("arm_r", swing(0.7)),
            (
                "hips",
                (0..=4)
                    .map(|i| (i as f32 * 0.16, Vec3::new(0.0, if i % 2 == 1 { 0.05 } else { 0.0 }, 0.0), 0.0))
                    .collect(),
            ),
            ("torso", vec![(0.0, z, 0.04), (0.32, z, -0.04), (0.64, z, 0.04)]),
        ],
    );
    let breathe = |a: f32| vec![(0.0, z, 0.0), (1.0, Vec3::new(0.0, a, 0.0), 0.0), (2.0, z, 0.0)];
    let idle = clip(
        &skeleton,
        2.0,
        &[
            ("hips", breathe(0.02)),
            ("arm_l", vec![(0.0, z, 0.05), (1.0, z, 0.1), (2.0, z, 0.05)]),
            ("arm_r", vec![(0.0, z, -0.05), (1.0, z, -0.1), (2.0, z, -0.05)]),
        ],
    );

    let (walk_h, idle_h) = {
        let mut assets = world.resource_mut::<AssetManager>();
        (
            assets.insert("walk", walk).expect("example asset names are distinct"),
            assets.insert("idle", idle).expect("example asset names are distinct"),
        )
    };

    let root = world.spawn((Transform::from_position(position), Name::new("player"), AnimationPlayer::new("idle")));
    let instance = spawn_skeleton(world, root, &skeleton);
    for part in parts() {
        let bone = instance.bone(part.bone).expect("part references a bone");
        world.spawn_child(
            bone,
            (
                Transform::from_position(part.offset),
                Sprite::new("white")
                    .with_size(part.size)
                    .with_anchor(part.anchor)
                    .with_tint(part.color)
                    .cutout()
                    .upright(),
            ),
        );
    }
    // Soft blob under the feet: a sprite lying flat on the ground, alpha-blended.
    world.spawn_child(
        root,
        (
            Transform::from_xyz(0.0, 0.03, 0.0).with_rotation(Quat::from_rotation_x(PI / 2.0)),
            Sprite::new("blob").with_size(Vec2::new(1.0, 0.6)).with_anchor(Vec2::splat(0.5)).unlit().fixed(),
        ),
    );
    Character { root, walk: walk_h, idle: idle_h }
}
