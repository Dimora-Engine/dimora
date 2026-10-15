//! Text dumps of a world. Read-only. No window and no editor state.

use std::any::TypeId;
use std::collections::HashSet;
use std::fmt::Write as _;

use hecs::{Entity, NoSuchEntity};

use crate::hierarchy::{Children, Parent};
use crate::registry::{ComponentInfo, ComponentRegistry, Persistence};
use crate::world::Name;
use crate::World;

/// [`World::dump_entity`] could not read `entity`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DumpError {
    /// `entity` is not alive. A despawned id is not reused for this error: the handle's generation
    /// still names the entity that was destroyed.
    NoSuchEntity { entity: Entity },
}

impl core::fmt::Display for DumpError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DumpError::NoSuchEntity { entity } => write!(f, "entity {entity:?} does not exist"),
        }
    }
}

impl std::error::Error for DumpError {}

impl World {
    /// Registers `desc` on the world's [`ComponentRegistry`].
    ///
    /// The registry is inserted by [`World::new`]. A mutable borrow of that resource, held across
    /// this call, panics. That is the same rule as [`World::resource_mut`].
    pub fn register_component<T: hecs::Component>(
        &self,
        desc: crate::registry::ComponentDesc<T>,
    ) -> Result<(), crate::registry::RegistryError> {
        self.resource_mut::<ComponentRegistry>().register(desc)
    }

    /// Hierarchy listing. Roots are ordered by [`Entity`]'s `Ord`. Children follow the `Children`
    /// list. Component names on a line are alphabetical. Unregistered components are counted and
    /// their `TypeId`s are listed, sorted by their `Debug` text.
    ///
    /// An entity that is not reached from a root is listed under `unreached` instead of being
    /// dropped. A cycle prints `(already listed)` and stops.
    ///
    /// Calling this twice on an unchanged world returns the same string.
    ///
    /// The registry borrow is the resource's borrow, so entity storage can be read in the same
    /// call. A mutable borrow of [`ComponentRegistry`] held by the caller panics, same as any
    /// other resource.
    pub fn dump_hierarchy(&self) -> String {
        let registry = self
            .resources
            .get::<ComponentRegistry>()
            .unwrap_or_else(|| crate::resources::missing::<ComponentRegistry>());
        hierarchy(&self.entities, &registry)
    }

    /// One entity, including declared fields and formatter values when the registration has them.
    pub fn dump_entity(&self, entity: Entity) -> Result<String, DumpError> {
        if !self.entities.contains(entity) {
            return Err(DumpError::NoSuchEntity { entity });
        }
        let registry = self
            .resources
            .get::<ComponentRegistry>()
            .unwrap_or_else(|| crate::resources::missing::<ComponentRegistry>());
        Ok(entity_dump(&self.entities, &registry, entity))
    }
}

fn hierarchy(storage: &hecs::World, registry: &ComponentRegistry) -> String {
    let mut entities: Vec<Entity> = storage.iter().map(|entity| entity.entity()).collect();
    entities.sort();
    let mut out = String::new();
    let _ = writeln!(out, "entities: {}", entities.len());
    if entities.is_empty() {
        return out;
    }

    let roots: Vec<Entity> = entities.iter().copied().filter(|entity| parent_of(storage, *entity).is_none()).collect();
    let mut visited = HashSet::new();
    for root in roots {
        write_node(&mut out, storage, registry, root, 0, &mut visited);
    }

    let mut unreached: Vec<Entity> = entities.into_iter().filter(|entity| !visited.contains(entity)).collect();
    if !unreached.is_empty() {
        unreached.sort();
        let _ = writeln!(out, "unreached:");
        for entity in unreached {
            write_node(&mut out, storage, registry, entity, 1, &mut visited);
        }
    }
    out
}

fn entity_dump(storage: &hecs::World, registry: &ComponentRegistry, entity: Entity) -> String {
    let mut out = String::new();
    let name = storage.get::<&Name>(entity).ok().map(|name| name.0.clone());
    let _ = writeln!(out, "entity {}{}", entity_token(entity), quoted_name(name.as_deref()));
    match parent_of(storage, entity) {
        Some(parent) => {
            let _ = writeln!(out, "parent: {}", entity_token(parent));
        }
        None => {
            let _ = writeln!(out, "parent: none");
        }
    }
    let view = match storage.entity(entity) {
        Ok(view) => view,
        Err(NoSuchEntity) => return out,
    };
    let (registered, unregistered) = classify(registry, &view);
    for listed in registered {
        write_registered(&mut out, &listed.info, listed.values);
    }
    write_unregistered(&mut out, 0, &unregistered);
    out
}

fn write_node(
    out: &mut String,
    storage: &hecs::World,
    registry: &ComponentRegistry,
    entity: Entity,
    depth: usize,
    visited: &mut HashSet<Entity>,
) {
    if !visited.insert(entity) {
        line(out, depth, &format!("{} (already listed)", entity_token(entity)));
        return;
    }
    let Ok(view) = storage.entity(entity) else {
        line(out, depth, &format!("{} (missing)", entity_token(entity)));
        return;
    };
    let name = view.get::<&Name>().map(|name| name.0.clone());
    let (registered, unregistered) = classify(registry, &view);
    line(out, depth, &format!("{}{}", entity_token(entity), quoted_name(name.as_deref())));
    let names = if registered.is_empty() {
        "(none)".to_string()
    } else {
        registered.iter().map(|listed| listed.info.name).collect::<Vec<_>>().join(", ")
    };
    line(out, depth + 1, &format!("components: {names}"));
    write_unregistered(out, depth + 1, &unregistered);

    let children = children_of(storage, entity);
    if children.is_empty() {
        line(out, depth + 1, "children: (none)");
        return;
    }
    line(out, depth + 1, "children:");
    for child in children {
        write_node(out, storage, registry, child, depth + 2, visited);
    }
}

struct Listed {
    info: ComponentInfo,
    values: Option<Vec<String>>,
}

fn classify(registry: &ComponentRegistry, view: &hecs::EntityRef<'_>) -> (Vec<Listed>, Vec<TypeId>) {
    let type_ids: Vec<TypeId> = view.component_types().collect();
    let mut registered = Vec::new();
    let mut unregistered = Vec::new();
    for type_id in type_ids {
        let info = registry.info_of(type_id).cloned();
        match info {
            Some(info) => registered.push(Listed { info, values: registry.values_of(type_id, view) }),
            None => unregistered.push(type_id),
        }
    }
    registered.sort_by(|a, b| a.info.name.cmp(b.info.name));
    unregistered.sort_by_key(|type_id| format!("{type_id:?}"));
    (registered, unregistered)
}

fn write_registered(out: &mut String, info: &ComponentInfo, values: Option<Vec<String>>) {
    let _ = writeln!(out, "{}", info.name);
    let _ = writeln!(out, "  rust_type: {}", info.rust_type);
    let _ = writeln!(out, "  persistence: {}", persistence_label(info.persistence));
    let _ = writeln!(out, "  script_visible: {}", info.script_visible);
    let _ = writeln!(out, "  script_writable: {}", info.script_writable);
    if info.fields.is_empty() {
        let _ = writeln!(out, "  fields: (none)");
        return;
    }
    let _ = writeln!(out, "  fields:");
    let matched = values.as_ref().is_some_and(|produced| produced.len() == info.fields.len());
    for (index, field) in info.fields.iter().enumerate() {
        let flags = format!("script_visible={} script_writable={}", field.script_visible, field.script_writable);
        match &values {
            Some(produced) if matched => {
                let _ = writeln!(out, "    {}: {} {flags} = {}", field.name, field.type_name, produced[index]);
            }
            Some(_) | None => {
                let _ = writeln!(out, "    {}: {} {flags}", field.name, field.type_name);
            }
        }
    }
    if let Some(produced) = values.as_ref().filter(|produced| produced.len() != info.fields.len()) {
        let _ =
            writeln!(out, "  values: formatter returned {} strings for {} fields", produced.len(), info.fields.len());
    }
}

fn write_unregistered(out: &mut String, depth: usize, type_ids: &[TypeId]) {
    line(out, depth, &format!("unregistered: {}", type_ids.len()));
    let mut labels: Vec<String> = type_ids.iter().map(|type_id| format!("{type_id:?}")).collect();
    labels.sort();
    for label in labels {
        line(out, depth + 1, &label);
    }
}

fn children_of(storage: &hecs::World, entity: Entity) -> Vec<Entity> {
    storage.get::<&Children>(entity).map(|children| children.0.clone()).unwrap_or_default()
}

fn parent_of(storage: &hecs::World, entity: Entity) -> Option<Entity> {
    storage.get::<&Parent>(entity).ok().map(|parent| parent.0)
}

fn quoted_name(name: Option<&str>) -> String {
    match name {
        Some(name) => format!(" \"{}\"", escape_name(name)),
        None => String::new(),
    }
}

fn entity_token(entity: Entity) -> String {
    format!("{entity:?}")
}

fn escape_name(name: &str) -> String {
    let mut out = String::new();
    for character in name.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out
}

fn persistence_label(persistence: Persistence) -> &'static str {
    match persistence {
        Persistence::Persist => "persist",
        Persistence::Runtime => "runtime",
    }
}

fn line(out: &mut String, depth: usize, text: &str) {
    for _ in 0..depth {
        out.push_str("  ");
    }
    out.push_str(text);
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use std::any::type_name;

    use hecs::Entity;

    use super::*;
    use crate::builtin::register_world_components;
    use crate::hierarchy::{Children, Parent};
    use crate::registry::{ComponentDesc, FieldDesc, Persistence, RegistryError};
    use crate::Transform;

    struct Alpha;
    struct Beta;

    const ONE_FIELD: &[FieldDesc] = &[FieldDesc::new("n", "u8")];
    const TWO_FIELDS: &[FieldDesc] = &[FieldDesc::new("n", "u8"), FieldDesc::new("extra", "u8")];

    fn marker(name: &'static str) -> ComponentDesc<Alpha> {
        ComponentDesc::new(name, ONE_FIELD, Persistence::Runtime, None)
    }

    #[test]
    fn registration_records_metadata_and_rejects_duplicates() {
        let world = World::new();
        world.register_component(marker("Alpha")).unwrap();
        let registry = world.resource::<ComponentRegistry>();
        let info = registry.get::<Alpha>().unwrap();
        assert_eq!(info.name, "Alpha");
        assert_eq!(info.fields, ONE_FIELD);
        assert_eq!(info.persistence, Persistence::Runtime);
        assert!(!info.script_visible && !info.script_writable);
        assert_eq!(info.rust_type, type_name::<Alpha>());
        drop(registry);

        let again = world.register_component(marker("Alpha")).unwrap_err();
        assert_eq!(again, RegistryError::DuplicateType { name: "Alpha", rust_type: type_name::<Alpha>() });

        let conflict = world
            .register_component(ComponentDesc::new(
                "Alpha",
                TWO_FIELDS,
                Persistence::Runtime,
                None::<fn(&Alpha) -> Vec<String>>,
            ))
            .unwrap_err();
        assert_eq!(
            conflict,
            RegistryError::ConflictingMetadata {
                rust_type: type_name::<Alpha>(),
                existing_name: "Alpha",
                new_name: "Alpha",
            }
        );
        assert_eq!(world.resource::<ComponentRegistry>().get::<Alpha>().unwrap().fields.len(), 1);

        let renamed = world
            .register_component(
                ComponentDesc::new("Other", ONE_FIELD, Persistence::Persist, None::<fn(&Alpha) -> Vec<String>>)
                    .script_visible(),
            )
            .unwrap_err();
        assert!(matches!(renamed, RegistryError::ConflictingMetadata { .. }));

        let taken =
            world.register_component(ComponentDesc::<Beta>::new("Alpha", &[], Persistence::Runtime, None)).unwrap_err();
        assert_eq!(
            taken,
            RegistryError::DuplicateName {
                name: "Alpha",
                existing_type: type_name::<Alpha>(),
                new_type: type_name::<Beta>()
            }
        );
        assert!(world.resource::<ComponentRegistry>().get::<Beta>().is_none());
        assert_eq!(
            world.register_component(ComponentDesc::<Alpha>::new("", &[], Persistence::Runtime, None)),
            Err(RegistryError::EmptyName)
        );
    }

    #[test]
    fn script_flags_are_stored_and_printed_and_do_not_change_spawn() {
        let mut world = World::new();
        const FIELDS: &[FieldDesc] = &[FieldDesc::new("n", "u8").script_visible().script_writable()];
        world
            .register_component(
                ComponentDesc::new("Flagged", FIELDS, Persistence::Persist, Some(|value: &u8| vec![value.to_string()]))
                    .script_visible()
                    .script_writable(),
            )
            .unwrap();
        let entity = world.spawn((7u8,));
        assert!(world.contains(entity));
        let registry = world.resource::<ComponentRegistry>();
        let info = registry.get::<u8>().unwrap();
        assert!(info.script_visible && info.script_writable);
        assert!(info.fields[0].script_visible && info.fields[0].script_writable);
        drop(registry);
        let dump = world.dump_entity(entity).unwrap();
        assert!(dump.contains("script_visible: true"));
        assert!(dump.contains("script_writable: true"));
        assert!(dump.contains("n: u8 script_visible=true script_writable=true = 7"));
    }

    #[test]
    fn known_world_components_list_their_fields_and_values() {
        let mut world = World::new();
        register_world_components(&mut world.resource_mut()).unwrap();
        let registry = world.resource::<ComponentRegistry>();
        let transform = registry.get::<Transform>().unwrap();
        assert_eq!(
            transform.fields.iter().map(|field| field.name).collect::<Vec<_>>(),
            ["position", "rotation", "scale"]
        );
        assert_eq!(transform.persistence, Persistence::Persist);
        assert_eq!(registry.get::<Parent>().unwrap().persistence, Persistence::Runtime);
        drop(registry);

        let entity = world.spawn((Name::new("hero"), Transform::from_xyz(0.0, 1.0, 0.0)));
        let dump = world.dump_entity(entity).unwrap();
        assert!(dump.contains("position: Vec3 script_visible=false script_writable=false = (0.0, 1.0, 0.0)"));
        assert!(dump.contains("rotation: Quat script_visible=false script_writable=false = (0.0, 0.0, 0.0, 1.0)"));
        assert!(dump.contains("scale: Vec3 script_visible=false script_writable=false = (1.0, 1.0, 1.0)"));
        assert!(dump.contains("value: String script_visible=false script_writable=false = hero"));
        assert!(dump.contains(&format!("rust_type: {}", type_name::<Transform>())));
        assert!(!dump.contains("formatter returned"));
    }

    #[test]
    fn a_formatter_that_disagrees_with_the_field_list_is_reported() {
        let mut world = World::new();
        world
            .register_component(ComponentDesc::new(
                "Broken",
                TWO_FIELDS,
                Persistence::Runtime,
                Some(|_: &Alpha| vec!["only".to_string()]),
            ))
            .unwrap();
        let entity = world.spawn((Alpha,));
        let dump = world.dump_entity(entity).unwrap();
        assert!(dump.contains("n: u8 script_visible=false script_writable=false\n"));
        assert!(!dump.contains("= only"));
        assert!(dump.contains("values: formatter returned 1 strings for 2 fields"));
    }

    #[test]
    fn unregistered_components_are_listed_by_type_id() {
        let mut world = World::new();
        register_world_components(&mut world.resource_mut()).unwrap();
        struct Secret;
        let entity = world.spawn((Transform::IDENTITY, Secret));
        let dump = world.dump_entity(entity).unwrap();
        assert!(dump.contains("Transform\n"));
        assert!(dump.contains("unregistered: 1\n"));
        assert!(dump.contains("TypeId("));
        assert_eq!(world.dump_entity(entity).unwrap(), dump);
    }

    #[test]
    fn hierarchy_dump_is_deterministic_and_follows_parent_links() {
        let mut world = World::new();
        register_world_components(&mut world.resource_mut()).unwrap();
        let root = world.spawn((Name::new("root"), Transform::IDENTITY));
        let later = world.spawn((Transform::from_xyz(2.0, 0.0, 0.0),));
        let earlier = world.spawn((Transform::from_xyz(1.0, 0.0, 0.0),));
        world.set_parent(later, root).unwrap();
        world.set_parent(earlier, root).unwrap();

        let dump = world.dump_hierarchy();
        assert_eq!(dump, world.dump_hierarchy());
        let expected = format!(
            "\
entities: 3
{root:?} \"root\"
  components: Children, Name, Transform
  unregistered: 0
  children:
    {later:?}
      components: Parent, Transform
      unregistered: 0
      children: (none)
    {earlier:?}
      components: Parent, Transform
      unregistered: 0
      children: (none)
"
        );
        assert_eq!(dump, expected);

        let entity_dump = world.dump_entity(earlier).unwrap();
        assert!(entity_dump.starts_with(&format!("entity {earlier:?}\nparent: {root:?}\n")));
        assert!(!entity_dump.contains("parent: none"));
    }

    #[test]
    fn an_entity_without_a_name_omits_the_quoted_label() {
        let mut world = World::new();
        let entity = world.spawn(());
        let dump = world.dump_hierarchy();
        assert_eq!(
            dump,
            format!("entities: 1\n{entity:?}\n  components: (none)\n  unregistered: 0\n  children: (none)\n")
        );
        assert!(!world.dump_entity(entity).unwrap().contains('"'));
    }

    #[test]
    fn an_empty_world_dumps_a_zero_count() {
        let world = World::new();
        assert_eq!(world.dump_hierarchy(), "entities: 0\n");
    }

    #[test]
    fn roots_follow_entity_order_when_iteration_order_differs() {
        let mut world = World::new();
        let slot = world.spawn(());
        let later = world.spawn((Name::new("later"),));
        world.despawn(slot).unwrap();
        let reused = world.spawn((Transform::IDENTITY,));
        assert!(reused < later, "the recycled id has to sort before the later entity");
        let dump = world.dump_hierarchy();
        let reused_at = dump.find(&format!("{reused:?}")).unwrap();
        let later_at = dump.find(&format!("{later:?}")).unwrap();
        assert!(reused_at < later_at);
    }

    #[test]
    fn missing_and_despawned_entities_are_errors() {
        let mut world = World::new();
        let entity = world.spawn((Name::new("gone"),));
        world.despawn(entity).unwrap();
        assert_eq!(world.dump_entity(entity), Err(DumpError::NoSuchEntity { entity }));
        assert_eq!(world.dump_entity(Entity::DANGLING), Err(DumpError::NoSuchEntity { entity: Entity::DANGLING }));
        assert_eq!(world.dump_hierarchy(), "entities: 0\n");
    }

    #[test]
    fn a_cycle_and_a_missing_child_do_not_drop_or_loop() {
        let mut world = World::new();
        let root = world.spawn((Name::new("root"),));
        world.entities.insert_one(root, Children(vec![root])).unwrap();
        let dump = world.dump_hierarchy();
        assert!(dump.contains("(already listed)"));
        assert_eq!(dump.matches("(already listed)").count(), 1);

        let mut other = World::new();
        let parent = other.spawn((Name::new("parent"),));
        let child = other.spawn(());
        other.despawn(child).unwrap();
        other.entities.insert_one(parent, Children(vec![child])).unwrap();
        assert!(other.dump_hierarchy().contains(&format!("{child:?} (missing)")));
    }

    #[test]
    fn a_missing_formatter_prints_fields_without_values() {
        let mut world = World::new();
        world.register_component(ComponentDesc::<Alpha>::new("Bare", ONE_FIELD, Persistence::Runtime, None)).unwrap();
        let entity = world.spawn((Alpha,));
        let dump = world.dump_entity(entity).unwrap();
        assert!(dump.contains("n: u8 script_visible=false script_writable=false\n"));
        assert!(!dump.contains(" = "));
        assert!(!dump.contains("formatter returned"));
    }

    #[test]
    fn malformed_field_lists_are_rejected_and_idempotent_registration_keeps_the_first() {
        const EMPTY_NAME: &[FieldDesc] = &[FieldDesc::new("", "u8")];
        const EMPTY_TYPE: &[FieldDesc] = &[FieldDesc::new("n", "")];
        const DUPLICATE: &[FieldDesc] = &[FieldDesc::new("n", "u8"), FieldDesc::new("n", "u8")];

        let mut world = World::new();
        let empty_field =
            world.register_component(ComponentDesc::<Alpha>::new("Alpha", EMPTY_NAME, Persistence::Runtime, None));
        assert_eq!(empty_field, Err(RegistryError::EmptyFieldName { component: "Alpha" }));
        let empty_type =
            world.register_component(ComponentDesc::<Alpha>::new("Alpha", EMPTY_TYPE, Persistence::Runtime, None));
        assert_eq!(empty_type, Err(RegistryError::EmptyFieldType { component: "Alpha", field: "n" }));
        let duplicated =
            world.register_component(ComponentDesc::<Alpha>::new("Alpha", DUPLICATE, Persistence::Runtime, None));
        assert_eq!(duplicated, Err(RegistryError::DuplicateFieldName { component: "Alpha", field: "n" }));
        assert!(world.resource::<ComponentRegistry>().get::<Alpha>().is_none());

        world
            .register_component(ComponentDesc::new(
                "Alpha",
                ONE_FIELD,
                Persistence::Runtime,
                Some(|_: &Alpha| vec!["first".to_string()]),
            ))
            .unwrap();
        let again = world.register_component(marker("Alpha")).unwrap_err();
        assert!(matches!(again, RegistryError::DuplicateType { .. }));
        world.resource_mut::<ComponentRegistry>().register_idempotent(marker("Alpha")).unwrap();
        world
            .resource_mut::<ComponentRegistry>()
            .register_idempotent(ComponentDesc::new(
                "Alpha",
                ONE_FIELD,
                Persistence::Runtime,
                Some(|_: &Alpha| vec!["second".to_string()]),
            ))
            .unwrap();
        let entity = world.spawn((Alpha,));
        let dump = world.dump_entity(entity).unwrap();
        assert!(dump.contains("= first"));
        assert!(!dump.contains("second"));

        let conflict = world.resource_mut::<ComponentRegistry>().register_idempotent(ComponentDesc::new(
            "Renamed",
            ONE_FIELD,
            Persistence::Persist,
            None::<fn(&Alpha) -> Vec<String>>,
        ));
        assert!(matches!(conflict, Err(RegistryError::ConflictingMetadata { .. })));
        let taken = world.resource_mut::<ComponentRegistry>().register_idempotent(ComponentDesc::<Beta>::new(
            "Alpha",
            &[],
            Persistence::Runtime,
            None,
        ));
        assert!(matches!(taken, Err(RegistryError::DuplicateName { .. })));
    }

    #[test]
    fn an_entity_parented_to_a_dead_entity_is_unreached() {
        let mut world = World::new();
        let kept = world.spawn((Name::new("kept"),));
        let dead = world.spawn(());
        world.despawn(dead).unwrap();
        world.entities.insert_one(kept, Parent(dead)).unwrap();
        let dump = world.dump_hierarchy();
        assert!(dump.contains("unreached:\n"));
        assert!(dump.contains("\"kept\""));
        assert!(!dump.lines().next().unwrap().contains("kept"));
    }
}
