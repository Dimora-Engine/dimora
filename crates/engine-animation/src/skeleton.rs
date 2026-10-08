use std::collections::HashMap;

use engine_assets::{Asset, AssetKind};
use engine_math::Transform;
use engine_world::{Entity, Name, World};

#[derive(Clone, Debug)]
pub struct BoneDef {
    pub name: String,
    pub parent: Option<usize>,
    /// Local transform relative to the parent bone (or to the skeleton root for root bones).
    pub rest: Transform,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SkeletonError {
    DuplicateName(String),
    /// A bone's parent must appear before it, which keeps evaluation a single forward pass.
    ParentNotDefined {
        bone: String,
    },
}

impl core::fmt::Display for SkeletonError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DuplicateName(n) => write!(f, "duplicate bone name `{n}`"),
            Self::ParentNotDefined { bone } => write!(f, "parent of bone `{bone}` must be defined before it"),
        }
    }
}
impl std::error::Error for SkeletonError {}

/// Bone hierarchy definition. Parents always precede their children.
#[derive(Clone, Debug, Default)]
pub struct Skeleton {
    bones: Vec<BoneDef>,
    by_name: HashMap<String, usize>,
}

impl Skeleton {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a bone and returns its index.
    pub fn add_bone(&mut self, name: &str, parent: Option<usize>, rest: Transform) -> Result<usize, SkeletonError> {
        if self.by_name.contains_key(name) {
            return Err(SkeletonError::DuplicateName(name.to_owned()));
        }
        if matches!(parent, Some(p) if p >= self.bones.len()) {
            return Err(SkeletonError::ParentNotDefined { bone: name.to_owned() });
        }
        let index = self.bones.len();
        self.bones.push(BoneDef { name: name.to_owned(), parent, rest });
        self.by_name.insert(name.to_owned(), index);
        Ok(index)
    }

    pub fn bones(&self) -> &[BoneDef] {
        &self.bones
    }
    pub fn len(&self) -> usize {
        self.bones.len()
    }
    pub fn is_empty(&self) -> bool {
        self.bones.is_empty()
    }
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.by_name.get(name).copied()
    }

    /// World transform of every bone for `pose`, given the skeleton root's world transform. This is
    /// the CPU reference for what transform propagation computes for the spawned bone entities, and
    /// the form IK solvers will work in.
    pub fn world_transforms(&self, pose: &Pose, root: &Transform) -> Vec<Transform> {
        let mut out: Vec<Transform> = Vec::with_capacity(self.bones.len());
        for (i, bone) in self.bones.iter().enumerate() {
            let parent = bone.parent.map_or(*root, |p| out[p]);
            out.push(parent * pose.local[i]);
        }
        out
    }
}

impl Asset for Skeleton {
    const KIND: AssetKind = AssetKind::Skeleton;
}

/// Local transform per bone: the animatable state of a skeleton.
#[derive(Clone, Debug, PartialEq)]
pub struct Pose {
    pub local: Vec<Transform>,
}

impl Pose {
    pub fn rest(skeleton: &Skeleton) -> Self {
        Self { local: skeleton.bones.iter().map(|b| b.rest).collect() }
    }
}

/// Marks a bone entity and records which skeleton bone it represents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bone {
    pub index: usize,
}

/// Component on a skeleton's root entity listing its bone entities.
#[derive(Clone, Debug)]
pub struct SkeletonInstance {
    pub bones: Vec<Entity>,
    by_name: HashMap<String, Entity>,
}

impl SkeletonInstance {
    pub fn bone(&self, name: &str) -> Option<Entity> {
        self.by_name.get(name).copied()
    }
}

/// Spawns one entity per bone under `root` (root bones become children of `root`) with the rest
/// pose as their `Transform`, and inserts a [`SkeletonInstance`] on `root`.
pub fn spawn_skeleton(world: &mut World, root: Entity, skeleton: &Skeleton) -> SkeletonInstance {
    let mut bones: Vec<Entity> = Vec::with_capacity(skeleton.len());
    let mut by_name = HashMap::new();
    for (index, def) in skeleton.bones().iter().enumerate() {
        let parent = def.parent.map_or(root, |p| bones[p]);
        let e = world.spawn_child(parent, (def.rest, Bone { index }, Name::new(def.name.clone())));
        bones.push(e);
        by_name.insert(def.name.clone(), e);
    }
    let instance = SkeletonInstance { bones, by_name };
    world.insert(root, (instance.clone(),)).expect("skeleton root must exist");
    instance
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_math::Quat;
    use engine_world::{propagate_transforms, GlobalTransform};

    pub(crate) fn arm() -> Skeleton {
        let mut s = Skeleton::new();
        let torso = s.add_bone("torso", None, Transform::from_xyz(0.0, 1.0, 0.0)).unwrap();
        let upper = s.add_bone("upper_arm", Some(torso), Transform::from_xyz(0.5, 0.5, 0.0)).unwrap();
        s.add_bone("hand", Some(upper), Transform::from_xyz(0.0, -1.0, 0.0)).unwrap();
        s
    }

    #[test]
    fn validates_structure() {
        let mut s = Skeleton::new();
        s.add_bone("a", None, Transform::IDENTITY).unwrap();
        assert_eq!(s.add_bone("a", None, Transform::IDENTITY), Err(SkeletonError::DuplicateName("a".into())));
        assert!(matches!(s.add_bone("b", Some(5), Transform::IDENTITY), Err(SkeletonError::ParentNotDefined { .. })));
        assert_eq!(s.index_of("a"), Some(0));
    }

    #[test]
    fn bone_entities_match_cpu_evaluation() {
        let skeleton = arm();
        let mut pose = Pose::rest(&skeleton);
        pose.local[1].rotation = Quat::from_rotation_z(0.8);
        let root_transform =
            Transform::from_xyz(3.0, 0.0, 40.0).with_rotation(Quat::from_rotation_y(0.4)).with_uniform_scale(2.0);

        let mut world = World::new();
        let root = world.spawn((root_transform,));
        let instance = spawn_skeleton(&mut world, root, &skeleton);
        for (i, e) in instance.bones.iter().enumerate() {
            world.component_mut::<Transform>(*e).unwrap().rotation = pose.local[i].rotation;
        }
        propagate_transforms(&mut world);

        let expected = skeleton.world_transforms(&pose, &root_transform);
        for (i, e) in instance.bones.iter().enumerate() {
            let got = world.component::<GlobalTransform>(*e).unwrap().matrix;
            assert!(got.approx_eq(&expected[i].to_matrix(), 1e-4), "bone {i}");
        }
        assert_eq!(instance.bone("hand"), Some(instance.bones[2]));
    }
}
