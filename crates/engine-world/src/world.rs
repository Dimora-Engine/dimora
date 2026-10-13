use std::cell::{Ref, RefMut};

use engine_math::{Mat4, Transform};
use hecs::{Component, ComponentError, DynamicBundle, Entity, NoSuchEntity, Query, QueryBorrow};

use crate::hierarchy::{Children, HierarchyError, Parent};
use crate::resources::{missing, Resources};

/// Human-readable label, mainly for debugging and lookups.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name(pub String);

impl Name {
    pub fn new(s: impl Into<String>) -> Self {
        Name(s.into())
    }
}

/// Whether an entity should be drawn. Absence of the component means visible.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Visibility {
    #[default]
    Visible,
    Hidden,
}

/// Entities + components + resources. Systems are plain functions over `&mut World`
/// (see [`crate::Schedule`]).
#[derive(Default)]
pub struct World {
    /// The underlying component storage, exposed for queries:
    /// `world.entities.query::<(&Transform, &mut Velocity)>()`.
    pub entities: hecs::World,
    resources: Resources,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    // ---- entities -------------------------------------------------------------------------

    pub fn spawn(&mut self, bundle: impl DynamicBundle) -> Entity {
        self.entities.spawn(bundle)
    }

    /// Spawns `bundle` as a child of `parent` and registers it in the hierarchy.
    pub fn spawn_child(&mut self, parent: Entity, bundle: impl DynamicBundle) -> Entity {
        let child = self.entities.spawn(bundle);
        self.set_parent(child, parent).expect("a freshly spawned entity cannot form a cycle");
        child
    }

    /// Despawns `entity` together with all of its descendants, and removes it from its parent's
    /// child list. Returns `Err` without changing anything if `entity` does not exist.
    ///
    /// To remove only the entity and keep its children, call [`World::detach_children`] first.
    pub fn despawn(&mut self, entity: Entity) -> Result<(), NoSuchEntity> {
        if !self.contains(entity) {
            return Err(NoSuchEntity);
        }
        self.detach(entity);
        let mut stack = vec![entity];
        while let Some(e) = stack.pop() {
            if let Ok(children) = self.entities.get::<&Children>(e) {
                stack.extend(children.0.iter().copied());
            }
            let _ = self.entities.despawn(e);
        }
        Ok(())
    }

    /// Turns every child of `parent` into a root, keeping each one at the same world position,
    /// rotation and scale (their local `Transform` is rewritten to the old world transform).
    /// The children's own subtrees are untouched.
    ///
    /// A child with no `Transform` is a plain grouping node. If it has children of its own it is
    /// given a `Transform` so that they stay in place. Errors if `parent` does not exist.
    pub fn detach_children(&mut self, parent: Entity) -> Result<(), NoSuchEntity> {
        if !self.contains(parent) {
            return Err(NoSuchEntity);
        }
        let Ok(children) = self.entities.remove_one::<Children>(parent) else { return Ok(()) };
        let parent_matrix = self.world_matrix(parent);
        for child in children.0 {
            let _ = self.entities.remove_one::<Parent>(child);
            let local = self.entities.get::<&Transform>(child).ok().map(|t| t.to_matrix());
            match local {
                Some(local) => {
                    if let Ok(mut t) = self.entities.get::<&mut Transform>(child) {
                        *t = Transform::from_matrix(&(parent_matrix * local));
                    }
                }
                None if self.entities.get::<&Children>(child).is_ok() => {
                    let _ = self.entities.insert_one(child, Transform::from_matrix(&parent_matrix));
                }
                None => {}
            }
        }
        Ok(())
    }

    /// World matrix of `entity` from the local transforms along its chain of parents (an entity
    /// without a `Transform` counts as identity). Does not depend on `GlobalTransform` being fresh.
    fn world_matrix(&self, entity: Entity) -> Mat4 {
        let mut matrix = Mat4::IDENTITY;
        let mut cur = Some(entity);
        while let Some(e) = cur {
            if let Ok(t) = self.entities.get::<&Transform>(e) {
                matrix = t.to_matrix() * matrix;
            }
            cur = self.entities.get::<&Parent>(e).ok().map(|p| p.0);
        }
        matrix
    }

    pub fn contains(&self, entity: Entity) -> bool {
        self.entities.contains(entity)
    }

    pub fn insert(&mut self, entity: Entity, bundle: impl DynamicBundle) -> Result<(), NoSuchEntity> {
        self.entities.insert(entity, bundle)
    }

    pub fn get<'a, Q: Query>(&'a self, entity: Entity) -> Result<hecs::QueryOne<'a, Q>, NoSuchEntity> {
        self.entities.query_one::<Q>(entity)
    }

    /// Shared access to a single component.
    pub fn component<T: Component>(&self, entity: Entity) -> Result<hecs::Ref<'_, T>, ComponentError> {
        self.entities.get::<&T>(entity)
    }

    /// Exclusive access to a single component.
    pub fn component_mut<T: Component>(&self, entity: Entity) -> Result<hecs::RefMut<'_, T>, ComponentError> {
        self.entities.get::<&mut T>(entity)
    }

    pub fn query<Q: Query>(&self) -> QueryBorrow<'_, Q> {
        self.entities.query::<Q>()
    }

    // ---- hierarchy ------------------------------------------------------------------------

    /// Makes `child` a child of `parent`, removing it from any previous parent.
    pub fn set_parent(&mut self, child: Entity, parent: Entity) -> Result<(), HierarchyError> {
        if child == parent || self.is_ancestor(child, parent) {
            return Err(HierarchyError::Cycle);
        }
        if !self.contains(child) || !self.contains(parent) {
            return Err(HierarchyError::NoSuchEntity);
        }
        self.detach(child);
        let _ = self.entities.insert_one(child, Parent(parent));
        let existing = self.entities.get::<&mut Children>(parent).map(|mut c| c.0.push(child)).is_ok();
        if !existing {
            let _ = self.entities.insert_one(parent, Children(vec![child]));
        }
        Ok(())
    }

    /// True if `ancestor` appears in `entity`'s chain of parents.
    pub fn is_ancestor(&self, ancestor: Entity, entity: Entity) -> bool {
        let mut cur = entity;
        while let Ok(p) = self.entities.get::<&Parent>(cur) {
            if p.0 == ancestor {
                return true;
            }
            cur = p.0;
        }
        false
    }

    fn detach(&mut self, child: Entity) {
        let Ok(parent) = self.entities.remove_one::<Parent>(child) else { return };
        if let Ok(mut children) = self.entities.get::<&mut Children>(parent.0) {
            children.0.retain(|&c| c != child);
        }
    }

    // ---- resources ------------------------------------------------------------------------

    pub fn insert_resource<T: 'static>(&mut self, value: T) {
        self.resources.insert(value);
    }

    pub fn has_resource<T: 'static>(&self) -> bool {
        self.resources.contains::<T>()
    }

    /// Shared access to a resource. Panics if it was never inserted; see [`World::try_resource`].
    pub fn resource<T: 'static>(&self) -> Ref<'_, T> {
        self.resources.get::<T>().unwrap_or_else(|| missing::<T>())
    }

    /// Exclusive access to a resource. Panics if it is missing or already borrowed.
    pub fn resource_mut<T: 'static>(&self) -> RefMut<'_, T> {
        self.resources.get_mut::<T>().unwrap_or_else(|| missing::<T>())
    }

    pub fn try_resource<T: 'static>(&self) -> Option<Ref<'_, T>> {
        self.resources.get::<T>()
    }

    pub fn remove_resource<T: 'static>(&mut self) -> Option<T> {
        self.resources.remove::<T>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Transform;

    #[test]
    fn resources_are_typed_singletons() {
        let mut w = World::new();
        w.insert_resource(5u32);
        w.insert_resource("hi".to_string());
        *w.resource_mut::<u32>() += 1;
        assert_eq!(*w.resource::<u32>(), 6);
        assert_eq!(&*w.resource::<String>(), "hi");
        assert!(w.try_resource::<f32>().is_none());
    }

    #[test]
    fn hierarchy_bookkeeping() {
        let mut w = World::new();
        let a = w.spawn((Transform::IDENTITY,));
        let b = w.spawn_child(a, (Transform::IDENTITY,));
        let c = w.spawn_child(b, (Transform::IDENTITY,));
        assert_eq!(w.component::<Children>(a).unwrap().0, vec![b]);
        assert_eq!(w.set_parent(a, c), Err(HierarchyError::Cycle));
        assert_eq!(w.set_parent(a, a), Err(HierarchyError::Cycle));

        w.set_parent(c, a).unwrap();
        assert!(w.component::<Children>(b).unwrap().0.is_empty());
        assert_eq!(w.component::<Children>(a).unwrap().0, vec![b, c]);

        w.despawn(a).unwrap();
        assert!(!w.contains(a) && !w.contains(b) && !w.contains(c));
    }

    /// The hierarchy invariant: every `Parent` points at a live entity that lists the child, and every
    /// listed child is alive and points back.
    fn assert_hierarchy_is_consistent(w: &World) {
        for (e, parent) in w.entities.query::<&Parent>().iter() {
            assert!(w.contains(parent.0), "{e:?} has a Parent pointing at a dead entity {:?}", parent.0);
            let listed = w.component::<Children>(parent.0).map(|c| c.0.contains(&e)).unwrap_or(false);
            assert!(listed, "{e:?} is not listed in the Children of its parent {:?}", parent.0);
        }
        for (e, children) in w.entities.query::<&Children>().iter() {
            for &c in &children.0 {
                assert!(w.contains(c), "{e:?} lists a dead child {c:?}");
                assert_eq!(w.component::<Parent>(c).map(|p| p.0).ok(), Some(e), "{c:?} does not point back at {e:?}");
            }
        }
    }

    /// root -> middle -> leaf, plus a sibling of `middle` and a sibling of `leaf` that must never be touched.
    struct Chain {
        w: World,
        root: Entity,
        middle: Entity,
        leaf: Entity,
        middle_sibling: Entity,
        leaf_sibling: Entity,
    }

    fn chain() -> Chain {
        let mut w = World::new();
        let root = w.spawn((Transform::from_xyz(1.0, 0.0, 0.0),));
        let middle = w.spawn_child(root, (Transform::from_xyz(2.0, 0.0, 0.0),));
        let leaf = w.spawn_child(middle, (Transform::from_xyz(3.0, 0.0, 0.0),));
        let middle_sibling = w.spawn_child(root, (Transform::IDENTITY,));
        let leaf_sibling = w.spawn_child(middle, (Transform::IDENTITY,));
        assert_hierarchy_is_consistent(&w);
        Chain { w, root, middle, leaf, middle_sibling, leaf_sibling }
    }

    #[test]
    fn despawning_a_leaf_leaves_the_rest_of_the_tree_intact() {
        let Chain { mut w, root, middle, leaf, middle_sibling, leaf_sibling } = chain();
        w.despawn(leaf).unwrap();
        assert!(!w.contains(leaf));
        assert!(w.contains(root) && w.contains(middle) && w.contains(middle_sibling) && w.contains(leaf_sibling));
        assert_eq!(w.component::<Children>(middle).unwrap().0, vec![leaf_sibling]);
        assert_eq!(w.component::<Children>(root).unwrap().0, vec![middle, middle_sibling]);
        assert_hierarchy_is_consistent(&w);
    }

    #[test]
    fn despawning_a_middle_node_removes_its_subtree_and_nothing_else() {
        let Chain { mut w, root, middle, leaf, middle_sibling, leaf_sibling } = chain();
        w.despawn(middle).unwrap();
        for gone in [middle, leaf, leaf_sibling] {
            assert!(!w.contains(gone), "{gone:?} should be despawned with its parent");
        }
        assert!(w.contains(root) && w.contains(middle_sibling));
        assert_eq!(w.component::<Children>(root).unwrap().0, vec![middle_sibling]);
        assert_hierarchy_is_consistent(&w);
    }

    #[test]
    fn despawning_the_root_removes_the_whole_tree() {
        let Chain { mut w, root, middle, leaf, middle_sibling, leaf_sibling } = chain();
        let unrelated = w.spawn((Transform::IDENTITY,));
        w.despawn(root).unwrap();
        for gone in [root, middle, leaf, middle_sibling, leaf_sibling] {
            assert!(!w.contains(gone), "{gone:?} should be despawned with the root");
        }
        assert!(w.contains(unrelated));
        assert_eq!(w.entities.query::<&Parent>().iter().count(), 0, "no Parent survives");
        assert_hierarchy_is_consistent(&w);
    }

    #[test]
    fn despawning_a_missing_entity_is_an_error_and_changes_nothing() {
        let Chain { mut w, leaf, .. } = chain();
        w.despawn(leaf).unwrap();
        let before = w.entities.len();
        assert!(w.despawn(leaf).is_err());
        assert_eq!(w.entities.len(), before);
        assert_hierarchy_is_consistent(&w);
    }

    #[test]
    fn despawning_a_group_without_a_transform_removes_its_children_too() {
        let mut w = World::new();
        let group = w.spawn((Name::new("group"),));
        let child = w.spawn_child(group, (Transform::IDENTITY,));
        w.despawn(group).unwrap();
        assert!(!w.contains(child));
    }

    #[test]
    fn detach_children_keeps_children_alive_as_roots_at_the_same_world_position() {
        let mut w = World::new();
        let parent = w.spawn((Transform::from_xyz(10.0, 0.0, 5.0)
            .with_rotation(engine_math::Quat::from_rotation_y(core::f32::consts::FRAC_PI_2))
            .with_uniform_scale(2.0),));
        let child = w.spawn_child(parent, (Transform::from_xyz(1.0, 0.0, 0.0),));
        let grandchild = w.spawn_child(child, (Transform::from_xyz(0.0, 1.0, 0.0),));
        crate::propagate_transforms(&mut w);
        let (child_before, grandchild_before) = (
            w.component::<crate::GlobalTransform>(child).unwrap().matrix,
            w.component::<crate::GlobalTransform>(grandchild).unwrap().matrix,
        );

        w.detach_children(parent).unwrap();
        assert!(w.contains(child) && w.contains(grandchild));
        assert!(w.component::<Parent>(child).is_err(), "the child is a root now");
        assert!(w.component::<Children>(parent).is_err(), "the parent lists no children");
        assert_eq!(w.component::<Parent>(grandchild).unwrap().0, child, "the subtree below the child is untouched");
        assert_hierarchy_is_consistent(&w);

        crate::propagate_transforms(&mut w);
        let child_after = w.component::<crate::GlobalTransform>(child).unwrap().matrix;
        let grandchild_after = w.component::<crate::GlobalTransform>(grandchild).unwrap().matrix;
        assert!(child_after.approx_eq(&child_before, 1e-4), "{child_after:?} vs {child_before:?}");
        assert!(grandchild_after.approx_eq(&grandchild_before, 1e-4));

        w.despawn(parent).unwrap();
        assert!(w.contains(child) && w.contains(grandchild), "despawning the old parent no longer reaches them");
    }

    #[test]
    fn detach_children_through_a_transform_less_child_keeps_its_descendants_in_place() {
        let mut w = World::new();
        let parent = w.spawn((Transform::from_xyz(7.0, 0.0, 0.0),));
        let group = w.spawn_child(parent, (Name::new("group"),));
        let leaf = w.spawn_child(group, (Transform::from_xyz(1.0, 0.0, 0.0),));
        crate::propagate_transforms(&mut w);
        w.detach_children(parent).unwrap();
        crate::propagate_transforms(&mut w);
        let x = w.component::<crate::GlobalTransform>(leaf).unwrap().translation().x;
        assert_eq!(x, 8.0);
    }

    #[test]
    fn detach_children_on_a_leaf_or_a_missing_entity() {
        let Chain { mut w, leaf, .. } = chain();
        assert!(w.detach_children(leaf).is_ok(), "nothing to detach is fine");
        w.despawn(leaf).unwrap();
        assert!(w.detach_children(leaf).is_err());
        assert_hierarchy_is_consistent(&w);
    }
}
