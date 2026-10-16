//! Name and tag lookup.
//!
//! Both scans read live entities through a query. Neither follows [`crate::Parent`], neither
//! stores an index, and neither means scene membership.

use hecs::Entity;

use crate::world::Name;
use crate::World;

/// Exact labels for [`World::find_by_tag`].
///
/// Each tag is a plain string. Matching is case-sensitive. `""` is not stored. A repeated
/// tag keeps the first insertion and is not stored again. Tags do not copy onto children,
/// do not change [`crate::Visibility`] or [`crate::Transform`], and do not mark an entity as
/// belonging to a scene.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tags {
    values: Vec<String>,
}

impl Tags {
    pub fn new() -> Self {
        Self { values: Vec::new() }
    }

    /// Inserts each tag. Empty strings are skipped. A duplicate keeps its first position.
    pub fn from_tags<I, S>(tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut stored = Self::new();
        for tag in tags {
            stored.insert(tag.as_ref());
        }
        stored
    }

    /// Stores `tag` when it is non-empty and not already present.
    ///
    /// Returns whether the list changed. `""` is rejected and leaves the list as it was.
    pub fn insert(&mut self, tag: &str) -> bool {
        if tag.is_empty() || self.contains(tag) {
            return false;
        }
        self.values.push(tag.to_string());
        true
    }

    /// Removes one exact tag. Returns whether it was present.
    pub fn remove(&mut self, tag: &str) -> bool {
        let Some(index) = self.values.iter().position(|existing| existing == tag) else {
            return false;
        };
        self.values.remove(index);
        true
    }

    pub fn contains(&self, tag: &str) -> bool {
        self.values.iter().any(|existing| existing == tag)
    }

    /// Stored tags, in first-insertion order.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.values.iter().map(String::as_str)
    }
}

impl World {
    /// Live entities whose [`Name`] equals `name`, ordered by [`Entity`]'s `Ord`.
    ///
    /// The comparison is exact, including case. `""` matches a [`Name`] whose string is
    /// empty, and does not match an entity that has no [`Name`]. Several entities may share
    /// one name. A despawned id is absent because the query only sees live entities.
    ///
    /// The order is the runtime id (index, then generation). It is stable for an unchanged
    /// world. It is not a persistent id, and this method does not keep an index.
    pub fn find_by_name(&self, name: &str) -> Vec<Entity> {
        let mut found: Vec<Entity> = self
            .query::<&Name>()
            .iter()
            .filter(|(_, component)| component.0 == name)
            .map(|(entity, _)| entity)
            .collect();
        found.sort();
        found
    }

    /// Live entities whose [`Tags`] contain `tag`, ordered by [`Entity`]'s `Ord`.
    ///
    /// The comparison is exact and case-sensitive. Each entity is listed once. Entities
    /// without [`Tags`] are skipped. `""` matches nothing, because an empty tag is not stored.
    /// A parent's tags are not visible on its children.
    pub fn find_by_tag(&self, tag: &str) -> Vec<Entity> {
        let mut found: Vec<Entity> =
            self.query::<&Tags>().iter().filter(|(_, tags)| tags.contains(tag)).map(|(entity, _)| entity).collect();
        found.sort();
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::register_world_components;
    use crate::registry::{ComponentRegistry, Persistence};
    use crate::Transform;

    #[test]
    fn find_by_name_lists_every_exact_match_in_entity_order() {
        let mut world = World::new();
        assert!(world.find_by_name("Player").is_empty());

        let slot = world.spawn(());
        let later = world.spawn((Name::new("Player"),));
        world.despawn(slot).unwrap();
        let reused = world.spawn((Name::new("Player"),));
        assert!(reused < later);
        let other = world.spawn((Name::new("player"),));
        let unnamed = world.spawn((Transform::IDENTITY,));

        assert_eq!(world.find_by_name("Player"), vec![reused, later]);
        assert_eq!(world.find_by_name("Player"), world.find_by_name("Player"));
        assert_eq!(world.find_by_name("player"), vec![other]);
        assert!(!world.find_by_name("Player").contains(&unnamed));
        assert!(!world.find_by_name("").contains(&unnamed));

        world.component_mut::<Name>(later).unwrap().0 = "Guide".to_string();
        assert_eq!(world.find_by_name("Player"), vec![reused]);
        assert_eq!(world.find_by_name("Guide"), vec![later]);

        world.despawn(reused).unwrap();
        assert!(world.find_by_name("Player").is_empty());
    }

    #[test]
    fn an_empty_name_matches_only_an_empty_name_component() {
        let mut world = World::new();
        let blank = world.spawn((Name::new(""),));
        let named = world.spawn((Name::new("Player"),));
        world.spawn((Transform::IDENTITY,));
        assert_eq!(world.find_by_name(""), vec![blank]);
        assert_eq!(world.find_by_name("Player"), vec![named]);
    }

    #[test]
    fn tags_reject_empties_and_duplicates_and_lookup_is_exact() {
        let mut world = World::new();
        let entity = world.spawn((Transform::from_xyz(1.0, 2.0, 3.0), crate::Visibility::Visible));
        let before = *world.component::<Transform>(entity).unwrap();
        let visibility = *world.component::<crate::Visibility>(entity).unwrap();

        world.insert(entity, (Tags::from_tags(["interactable", "", "interactable", "Door"]),)).unwrap();
        {
            let tags = world.component::<Tags>(entity).unwrap();
            assert_eq!(tags.iter().collect::<Vec<_>>(), vec!["interactable", "Door"]);
        }
        {
            let mut tags = world.component_mut::<Tags>(entity).unwrap();
            assert!(!tags.insert(""));
            assert!(!tags.insert("interactable"));
            assert!(tags.insert("door"));
            assert!(tags.remove("Door"));
            assert!(!tags.remove("Door"));
        }
        assert_eq!(world.component::<Tags>(entity).unwrap().iter().collect::<Vec<_>>(), vec!["interactable", "door"]);
        assert_eq!(*world.component::<Transform>(entity).unwrap(), before);
        assert_eq!(*world.component::<crate::Visibility>(entity).unwrap(), visibility);

        assert_eq!(world.find_by_tag("interactable"), vec![entity]);
        assert_eq!(world.find_by_tag("door"), vec![entity]);
        assert!(world.find_by_tag("Door").is_empty());
        assert!(world.find_by_tag("").is_empty());
        assert_eq!(world.find_by_tag("interactable").len(), 1);

        world.component_mut::<Tags>(entity).unwrap().remove("interactable");
        assert!(world.find_by_tag("interactable").is_empty());
        world.despawn(entity).unwrap();
        assert!(world.find_by_tag("door").is_empty());
    }

    #[test]
    fn tags_do_not_follow_parents_and_lookup_spans_hierarchies() {
        let mut world = World::new();
        let parent = world.spawn((Name::new("root"), Tags::from_tags(["group"])));
        let child = world.spawn_child(parent, (Name::new("child"), Transform::IDENTITY));
        let other = world.spawn((Name::new("child"), Tags::from_tags(["group", "interactable"])));

        assert_eq!(world.find_by_tag("group"), {
            let mut ids = vec![parent, other];
            ids.sort();
            ids
        });
        assert!(!world.find_by_tag("group").contains(&child));
        assert_eq!(world.find_by_tag("interactable"), vec![other]);
        assert_eq!(world.find_by_name("child"), {
            let mut ids = vec![child, other];
            ids.sort();
            ids
        });
        assert_eq!(world.find_by_name("root"), vec![parent]);
    }

    #[test]
    fn tags_are_registered_and_the_dump_lists_stored_values() {
        let mut world = World::new();
        register_world_components(&mut world.resource_mut()).unwrap();
        {
            let registry = world.resource::<ComponentRegistry>();
            let info = registry.get::<Tags>().unwrap();
            assert_eq!(info.name, "Tags");
            assert_eq!(info.fields.iter().map(|field| field.name).collect::<Vec<_>>(), ["values"]);
            assert_eq!(info.persistence, Persistence::Persist);
            assert!(!info.script_visible && !info.script_writable);
        }

        let entity = world.spawn((Tags::from_tags(["door", "a,b"]),));
        let dump = world.dump_entity(entity).unwrap();
        assert!(dump.contains("Tags\n"));
        assert!(dump.contains("persistence: persist"));
        assert!(dump.contains(r#"values: Vec<String> script_visible=false script_writable=false = "door", "a,b""#));
        assert!(!dump.contains("formatter returned"));
    }
}
