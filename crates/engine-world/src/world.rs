use std::cell::{Ref, RefMut};

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

    /// Despawns one entity. Children are *not* touched; use [`World::despawn_recursive`].
    pub fn despawn(&mut self, entity: Entity) -> Result<(), NoSuchEntity> {
        self.detach(entity);
        self.entities.despawn(entity)
    }

    /// Despawns `entity` and all of its descendants.
    pub fn despawn_recursive(&mut self, entity: Entity) {
        self.detach(entity);
        let mut stack = vec![entity];
        while let Some(e) = stack.pop() {
            if let Ok(children) = self.entities.get::<&Children>(e) {
                stack.extend(children.0.iter().copied());
            }
            let _ = self.entities.despawn(e);
        }
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

        w.despawn_recursive(a);
        assert!(!w.contains(a) && !w.contains(b) && !w.contains(c));
    }
}
