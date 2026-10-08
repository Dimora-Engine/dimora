use engine_assets::{Asset, AssetHandle, AssetKind, AssetManager};
use engine_core::Time;
use engine_math::Transform;
use engine_world::World;

use crate::skeleton::SkeletonInstance;

#[derive(Clone, Copy, Debug)]
pub struct Keyframe {
    /// Seconds from the clip start; keys in a track must be sorted by time.
    pub time: f32,
    /// Absolute local transform of the bone at this time.
    pub transform: Transform,
}

#[derive(Clone, Debug)]
pub struct BoneTrack {
    /// Index of the bone in the skeleton.
    pub bone: usize,
    pub keys: Vec<Keyframe>,
}

impl BoneTrack {
    /// Interpolated local transform at `time` (clamped to the first/last key).
    pub fn sample(&self, time: f32) -> Option<Transform> {
        let (first, last) = (self.keys.first()?, self.keys.last()?);
        if time <= first.time {
            return Some(first.transform);
        }
        if time >= last.time {
            return Some(last.transform);
        }
        let next = self.keys.partition_point(|k| k.time <= time);
        let (a, b) = (&self.keys[next - 1], &self.keys[next]);
        let t = (time - a.time) / (b.time - a.time);
        Some(a.transform.lerp(&b.transform, t))
    }
}

/// Keyframed animation for a skeleton. Bones without a track keep their current transform.
#[derive(Clone, Debug)]
pub struct AnimationClip {
    pub duration: f32,
    pub looping: bool,
    pub tracks: Vec<BoneTrack>,
}

impl Asset for AnimationClip {
    const KIND: AssetKind = AssetKind::Animation;
}

/// Plays one clip on the skeleton that shares this entity.
#[derive(Clone, Copy, Debug)]
pub struct AnimationPlayer {
    pub clip: AssetHandle<AnimationClip>,
    pub time: f32,
    pub speed: f32,
    pub playing: bool,
}

impl AnimationPlayer {
    pub fn new(clip: &str) -> Self {
        Self { clip: AssetHandle::from_name(clip), time: 0.0, speed: 1.0, playing: true }
    }

    /// Switches clip, restarting only if it actually changed.
    pub fn play(&mut self, clip: AssetHandle<AnimationClip>) {
        if self.clip != clip {
            self.clip = clip;
            self.time = 0.0;
        }
        self.playing = true;
    }
}

/// Advances every [`AnimationPlayer`] and writes the sampled pose into its skeleton's bone
/// `Transform`s. Needs the `Time` and `AssetManager` resources. Run before transform propagation.
pub fn animate_system(world: &mut World) {
    let dt = world.resource::<Time>().delta_secs();
    let assets = world.resource::<AssetManager>();
    for (_, (player, skeleton)) in world.query::<(&mut AnimationPlayer, &SkeletonInstance)>().iter() {
        let Some(clip) = assets.get(player.clip) else { continue };
        if player.playing {
            player.time += dt * player.speed;
            if clip.looping && clip.duration > 0.0 {
                player.time = player.time.rem_euclid(clip.duration);
            } else if player.time >= clip.duration {
                player.time = clip.duration;
                player.playing = false;
            }
        }
        for track in &clip.tracks {
            let (Some(sample), Some(&entity)) = (track.sample(player.time), skeleton.bones.get(track.bone)) else {
                continue;
            };
            if let Ok(mut t) = world.component_mut::<Transform>(entity) {
                *t = sample;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skeleton::{spawn_skeleton, Skeleton};
    use core::time::Duration;
    use engine_math::Vec3;

    fn track() -> BoneTrack {
        BoneTrack {
            bone: 0,
            keys: vec![
                Keyframe { time: 0.0, transform: Transform::from_xyz(0.0, 0.0, 0.0) },
                Keyframe { time: 1.0, transform: Transform::from_xyz(10.0, 0.0, 0.0) },
                Keyframe { time: 2.0, transform: Transform::from_xyz(10.0, 4.0, 0.0) },
            ],
        }
    }

    #[test]
    fn sampling_interpolates_and_clamps() {
        let t = track();
        assert_eq!(t.sample(-1.0).unwrap().position, Vec3::ZERO);
        assert_eq!(t.sample(0.5).unwrap().position, Vec3::new(5.0, 0.0, 0.0));
        assert_eq!(t.sample(1.5).unwrap().position, Vec3::new(10.0, 2.0, 0.0));
        assert_eq!(t.sample(9.0).unwrap().position, Vec3::new(10.0, 4.0, 0.0));
        assert!(BoneTrack { bone: 0, keys: vec![] }.sample(0.0).is_none());
    }

    #[test]
    fn player_drives_bone_entities_and_loops() {
        let mut assets = AssetManager::in_memory();
        assets.insert("slide", AnimationClip { duration: 2.0, looping: true, tracks: vec![track()] });
        let mut world = World::new();
        world.insert_resource(assets);
        world.insert_resource(Time::default());

        let mut skeleton = Skeleton::new();
        skeleton.add_bone("root", None, Transform::IDENTITY).unwrap();
        let root = world.spawn((Transform::IDENTITY, AnimationPlayer::new("slide")));
        let inst = spawn_skeleton(&mut world, root, &skeleton);

        world.resource_mut::<Time>().advance(Duration::from_millis(500));
        animate_system(&mut world);
        assert_eq!(world.component::<Transform>(inst.bones[0]).unwrap().position, Vec3::new(5.0, 0.0, 0.0));

        world.resource_mut::<Time>().advance(Duration::from_millis(2000));
        animate_system(&mut world);
        assert!((world.component::<AnimationPlayer>(root).unwrap().time - 0.5).abs() < 1e-5, "looped");
    }
}
