use engine_math::Transform;

use crate::World;

/// Opt-in render interpolation of this entity's local [`Transform`].
///
/// Adding the component does not move the entity. Simulation keeps writing [`Transform`].
/// At the start of each fixed iteration, [`snapshot_interpolation`] copies that local into
/// [`Interpolate::previous`]. Extraction blends `previous` toward the current local by the
/// fixed-timestep accumulator fraction. [`Transform`] and [`GlobalTransform`](crate::GlobalTransform)
/// are not written.
///
/// `previous` is `None` until the first snapshot, and extraction then uses the current local.
/// [`Interpolate::snap`] stores a local you pass so a teleport does not slide across the gap.
///
/// The stored local is in the same parent space as the current local. [`World::detach_children`]
/// rewrites a direct child's local into world space and sets that child's `previous` to the
/// rewritten local. Descendants are not rewritten. [`World::set_parent`] and removing
/// [`Parent`](crate::Parent) leave both samples unchanged, so they stay comparable and the entity
/// pops into the new space.
///
/// A variable-step write of a marked local is included in the blend. The marker is for locals
/// written in `fixed_update`. Mixing a variable-step write with that history is not supported.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Interpolate {
    /// Local transform from the start of the latest fixed iteration, if one has been stored.
    pub previous: Option<Transform>,
}

impl Interpolate {
    /// Marks an entity for interpolation with no stored history yet.
    pub const fn new() -> Self {
        Self { previous: None }
    }

    /// Stores `current` as the previous local so the next extraction does not blend across a jump.
    ///
    /// Pass the entity's current [`Transform`]. There is no distance threshold.
    pub fn snap(&mut self, current: Transform) {
        self.previous = Some(current);
    }
}

/// Copies the current local [`Transform`] into [`Interpolate::previous`] for every entity that
/// has both. Unmarked entities are left unchanged.
///
/// The frame loop calls this at the start of each fixed iteration, before fixed-update systems.
/// A later iteration in the same frame overwrites `previous`, leaving the local from before the
/// last iteration. It is public so that loop, in another crate, can call it. It is not part of
/// the engine prelude.
pub fn snapshot_interpolation(world: &mut World) {
    for (_, (local, interpolate)) in world.entities.query::<(&Transform, &mut Interpolate)>().iter() {
        interpolate.previous = Some(*local);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_math::Vec3;

    #[test]
    fn snapshot_stores_only_marked_locals() {
        let mut world = World::new();
        let marked = world.spawn((Transform::from_xyz(1.0, 2.0, 3.0), Interpolate::new()));
        let plain = world.spawn((Transform::from_xyz(9.0, 0.0, 0.0),));
        snapshot_interpolation(&mut world);
        assert_eq!(world.component::<Interpolate>(marked).unwrap().previous, Some(Transform::from_xyz(1.0, 2.0, 3.0)));
        world.component_mut::<Transform>(marked).unwrap().position = Vec3::new(4.0, 5.0, 6.0);
        snapshot_interpolation(&mut world);
        assert_eq!(world.component::<Interpolate>(marked).unwrap().previous, Some(Transform::from_xyz(4.0, 5.0, 6.0)));
        assert_eq!(world.component::<Transform>(plain).unwrap().position, Vec3::new(9.0, 0.0, 0.0));
        assert!(world.component::<Interpolate>(plain).is_err());
    }

    #[test]
    fn snap_records_the_local_it_is_given() {
        let mut world = World::new();
        let entity = world.spawn((Transform::from_xyz(1.0, 0.0, 0.0), Interpolate::new()));
        snapshot_interpolation(&mut world);
        let jumped = Transform::from_xyz(40.0, 0.0, 0.0);
        *world.component_mut::<Transform>(entity).unwrap() = jumped;
        world.component_mut::<Interpolate>(entity).unwrap().snap(jumped);
        assert_eq!(world.component::<Interpolate>(entity).unwrap().previous, Some(jumped));
        assert_eq!(*world.component::<Transform>(entity).unwrap(), jumped);
    }

    #[test]
    fn despawn_removes_the_stored_local() {
        let mut world = World::new();
        let entity = world.spawn((Transform::IDENTITY, Interpolate::new()));
        snapshot_interpolation(&mut world);
        world.despawn(entity).unwrap();
        assert!(!world.contains(entity));
    }

    #[test]
    fn a_marker_without_a_transform_is_skipped() {
        let mut world = World::new();
        let entity = world.spawn((Interpolate::new(),));
        snapshot_interpolation(&mut world);
        assert!(world.component::<Interpolate>(entity).unwrap().previous.is_none());
    }
}
