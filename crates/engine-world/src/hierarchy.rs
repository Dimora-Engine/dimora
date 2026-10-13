use engine_math::{Mat4, Transform, Vec3};
use hecs::Entity;

use crate::World;

/// Points at an entity's parent. Maintain it through [`World::set_parent`] / `spawn_child`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parent(pub Entity);

/// Ordered list of an entity's children, kept in sync with [`Parent`] by the world.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Children(pub Vec<Entity>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HierarchyError {
    Cycle,
    NoSuchEntity,
}

impl core::fmt::Display for HierarchyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            HierarchyError::Cycle => f.write_str("reparenting would create a cycle"),
            HierarchyError::NoSuchEntity => f.write_str("entity does not exist"),
        }
    }
}
impl std::error::Error for HierarchyError {}

/// World-space transform, computed from the local [`Transform`] chain by
/// [`propagate_transforms`]. Read-only for gameplay: edit `Transform` instead.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlobalTransform {
    pub matrix: Mat4,
}

impl Default for GlobalTransform {
    fn default() -> Self {
        Self { matrix: Mat4::IDENTITY }
    }
}

impl GlobalTransform {
    pub fn translation(&self) -> Vec3 {
        self.matrix.translation()
    }
    /// World-space direction of the entity's local +Z axis (unnormalized if scaled).
    pub fn forward(&self) -> Vec3 {
        self.matrix.transform_vector3(Vec3::Z)
    }
    pub fn to_transform(&self) -> Transform {
        Transform::from_matrix(&self.matrix)
    }
}

/// Recomputes every [`GlobalTransform`] from local transforms and the parent/child hierarchy.
/// Entities that have a `Transform` but no `GlobalTransform` get one.
///
/// An entity in the hierarchy that has no `Transform` (a plain grouping node) counts as identity: its
/// children are placed relative to its own parent, or relative to the world if it has none. Such an
/// entity gets no `GlobalTransform` of its own.
///
/// Run once per frame after gameplay/animation systems and before extraction. There is no dirty
/// tracking yet: the cost is linear in the number of transformed entities.
pub fn propagate_transforms(world: &mut World) {
    let missing: Vec<Entity> =
        world.entities.query::<&Transform>().without::<&GlobalTransform>().iter().map(|(e, _)| e).collect();
    for e in missing {
        let _ = world.entities.insert_one(e, GlobalTransform::default());
    }

    let mut stack: Vec<(Entity, Mat4)> = Vec::new();
    for (e, (local, global)) in world.entities.query::<(&Transform, &mut GlobalTransform)>().without::<&Parent>().iter()
    {
        global.matrix = local.to_matrix();
        stack.push((e, global.matrix));
    }

    let groups: Vec<Entity> = world
        .entities
        .query::<&Children>()
        .without::<&Transform>()
        .without::<&Parent>()
        .iter()
        .map(|(e, _)| e)
        .collect();
    stack.extend(groups.into_iter().map(|e| (e, Mat4::IDENTITY)));

    while let Some((entity, parent_matrix)) = stack.pop() {
        let Ok(children) = world.entities.get::<&Children>(entity) else { continue };
        for &child in &children.0 {
            let Ok(mut q) = world.entities.query_one::<(&Transform, &mut GlobalTransform)>(child) else { continue };
            match q.get() {
                Some((local, global)) => {
                    global.matrix = parent_matrix * local.to_matrix();
                    stack.push((child, global.matrix));
                }
                None => stack.push((child, parent_matrix)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Name;
    use engine_math::Quat;

    #[test]
    fn child_inherits_parent_rotation_scale_and_position() {
        let mut w = World::new();
        let parent = w.spawn((Transform::from_xyz(10.0, 0.0, 5.0)
            .with_rotation(Quat::from_rotation_y(core::f32::consts::FRAC_PI_2))
            .with_uniform_scale(2.0),));
        let child = w.spawn_child(parent, (Transform::from_xyz(1.0, 0.0, 0.0),));
        let grandchild = w.spawn_child(child, (Transform::from_xyz(0.0, 1.0, 0.0),));
        propagate_transforms(&mut w);

        // Local +X of the parent points along world -Z after a +90deg yaw.
        let c = w.component::<GlobalTransform>(child).unwrap().translation();
        assert!(c.approx_eq(Vec3::new(10.0, 0.0, 3.0), 1e-5), "{c:?}");
        let g = w.component::<GlobalTransform>(grandchild).unwrap().translation();
        assert!(g.approx_eq(Vec3::new(10.0, 2.0, 3.0), 1e-5), "{g:?}");
    }

    #[test]
    fn moving_the_parent_moves_children() {
        let mut w = World::new();
        let parent = w.spawn((Transform::IDENTITY,));
        let child = w.spawn_child(parent, (Transform::from_xyz(0.0, 0.0, 1.0),));
        propagate_transforms(&mut w);
        w.component_mut::<Transform>(parent).unwrap().position = Vec3::new(3.0, 0.0, 0.0);
        propagate_transforms(&mut w);
        assert!(w.component::<GlobalTransform>(child).unwrap().translation().approx_eq(Vec3::new(3.0, 0.0, 1.0), 1e-6));
    }

    #[test]
    fn reparenting_moves_the_child_into_the_new_parents_space() {
        let mut w = World::new();
        let a = w.spawn((Transform::from_xyz(100.0, 0.0, 0.0),));
        let b = w.spawn((Transform::from_xyz(0.0, 50.0, 0.0),));
        let child = w.spawn_child(a, (Transform::from_xyz(0.0, 0.0, 1.0),));
        propagate_transforms(&mut w);
        assert!(w
            .component::<GlobalTransform>(child)
            .unwrap()
            .translation()
            .approx_eq(Vec3::new(100.0, 0.0, 1.0), 1e-6));
        w.set_parent(child, b).unwrap();
        propagate_transforms(&mut w);
        assert!(w
            .component::<GlobalTransform>(child)
            .unwrap()
            .translation()
            .approx_eq(Vec3::new(0.0, 50.0, 1.0), 1e-6));
    }

    #[test]
    fn global_transform_equals_the_product_of_the_chain() {
        let mut w = World::new();
        let (ta, tb, tc) = (
            Transform::from_xyz(1.0, 2.0, 3.0).with_rotation(Quat::from_rotation_z(0.4)).with_uniform_scale(1.5),
            Transform::from_xyz(-2.0, 0.5, 4.0).with_rotation(Quat::from_rotation_x(-0.8)),
            Transform::from_xyz(0.0, 3.0, -1.0).with_rotation(Quat::from_rotation_y(1.1)).with_uniform_scale(0.5),
        );
        let a = w.spawn((ta,));
        let b = w.spawn_child(a, (tb,));
        let c = w.spawn_child(b, (tc,));
        propagate_transforms(&mut w);
        let expected = ta.to_matrix() * tb.to_matrix() * tc.to_matrix();
        assert!(w.component::<GlobalTransform>(c).unwrap().matrix.approx_eq(&expected, 1e-4));
    }

    fn world_x(w: &World, e: Entity) -> f32 {
        w.component::<GlobalTransform>(e).unwrap().translation().x
    }

    #[test]
    fn a_child_of_an_entity_without_a_transform_treats_the_parent_as_identity() {
        let mut w = World::new();
        let parent = w.spawn((Name::new("group, no transform"),));
        let child = w.spawn_child(parent, (Transform::from_xyz(5.0, 0.0, 0.0),));
        propagate_transforms(&mut w);
        assert_eq!(w.component::<GlobalTransform>(child).unwrap().translation(), Vec3::new(5.0, 0.0, 0.0));
        assert!(w.component::<GlobalTransform>(parent).is_err(), "no Transform, so no GlobalTransform is invented");
    }

    #[test]
    fn a_transform_less_entity_in_the_middle_of_a_chain_is_identity() {
        let mut w = World::new();
        let root = w.spawn((Transform::from_xyz(10.0, 0.0, 0.0),));
        let group = w.spawn_child(root, (Name::new("group"),));
        let inner = w.spawn_child(group, (Name::new("inner group"),));
        let leaf = w.spawn_child(inner, (Transform::from_xyz(1.0, 0.0, 0.0),));
        propagate_transforms(&mut w);
        assert_eq!(world_x(&w, leaf), 11.0);
        w.component_mut::<Transform>(root).unwrap().position.x = 20.0;
        propagate_transforms(&mut w);
        assert_eq!(world_x(&w, leaf), 21.0, "and it keeps following the root afterwards");
    }

    #[test]
    fn a_transform_less_root_does_not_hide_a_whole_subtree() {
        let mut w = World::new();
        let group = w.spawn((Name::new("scene root"),));
        let a = w.spawn_child(group, (Transform::from_xyz(2.0, 0.0, 0.0),));
        let b = w.spawn_child(a, (Transform::from_xyz(3.0, 0.0, 0.0),));
        let sibling = w.spawn_child(group, (Transform::from_xyz(-4.0, 0.0, 0.0),));
        propagate_transforms(&mut w);
        assert_eq!((world_x(&w, a), world_x(&w, b), world_x(&w, sibling)), (2.0, 5.0, -4.0));
    }

    #[test]
    fn a_transform_less_parent_keeps_the_rotation_of_its_own_ancestors() {
        let mut w = World::new();
        let root = w.spawn((
            Transform::from_xyz(0.0, 0.0, 0.0).with_rotation(Quat::from_rotation_y(core::f32::consts::FRAC_PI_2)),
        ));
        let group = w.spawn_child(root, (Name::new("group"),));
        let leaf = w.spawn_child(group, (Transform::from_xyz(1.0, 0.0, 0.0),));
        propagate_transforms(&mut w);
        let p = w.component::<GlobalTransform>(leaf).unwrap().translation();
        assert!(p.approx_eq(Vec3::new(0.0, 0.0, -1.0), 1e-5), "{p:?}");
    }
}
