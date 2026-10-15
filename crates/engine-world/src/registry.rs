//! Explicit component registry.
//!
//! Registration is hand-written. Field lists are metadata for dumps and for later milestones.
//! They are not derived from the Rust struct, and nothing in M4 checks that a name or a type
//! string exists on that struct. A formatter is not part of the metadata comparison, and a
//! formatter that panics is not caught. Script flags are stored and no system reads them.
//! Persistence is a classification for a future serializer. This module does not write files.

use std::any::{type_name, TypeId};
use std::collections::HashMap;
use std::fmt;

use hecs::{Component, EntityRef};

/// Whether a future scene file may store this component.
///
/// M4 does not serialize either kind. [`Persistence::Runtime`] marks derived data, playback
/// clocks, and values that hold runtime [`hecs::Entity`] ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Persistence {
    /// Plain data a future scene file may store.
    Persist,
    /// Skipped by a future serializer until a later decision says otherwise.
    Runtime,
}

/// One declared field. The names are the registry's, not a reflection of the struct.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldDesc {
    pub name: &'static str,
    pub type_name: &'static str,
    /// Stored for M18. No script reads this in M4.
    pub script_visible: bool,
    /// Stored for M18. No script reads this in M4.
    pub script_writable: bool,
}

impl FieldDesc {
    pub const fn new(name: &'static str, type_name: &'static str) -> Self {
        Self { name, type_name, script_visible: false, script_writable: false }
    }

    pub const fn script_visible(mut self) -> Self {
        self.script_visible = true;
        self
    }

    pub const fn script_writable(mut self) -> Self {
        self.script_writable = true;
        self
    }
}

/// What the dump needs in order to print one component type.
///
/// `values` returns one string per [`FieldDesc`], in the same order. A missing formatter prints
/// the fields and no values. A formatter whose length disagrees is reported by the dump and is
/// not matched up with the wrong fields. The function is not compared when deciding whether two
/// registrations agree, and a panic inside it is not caught.
pub struct ComponentDesc<T: 'static> {
    pub name: &'static str,
    pub fields: &'static [FieldDesc],
    pub persistence: Persistence,
    /// Stored for M18. No script reads this in M4.
    pub script_visible: bool,
    /// Stored for M18. No script reads this in M4.
    pub script_writable: bool,
    pub values: Option<fn(&T) -> Vec<String>>,
}

impl<T: 'static> ComponentDesc<T> {
    pub const fn new(
        name: &'static str,
        fields: &'static [FieldDesc],
        persistence: Persistence,
        values: Option<fn(&T) -> Vec<String>>,
    ) -> Self {
        Self { name, fields, persistence, script_visible: false, script_writable: false, values }
    }

    pub const fn script_visible(mut self) -> Self {
        self.script_visible = true;
        self
    }

    pub const fn script_writable(mut self) -> Self {
        self.script_writable = true;
        self
    }
}

/// The registered view of one component type. The value formatter stays inside the registry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentInfo {
    pub name: &'static str,
    pub type_id: TypeId,
    pub rust_type: &'static str,
    pub fields: &'static [FieldDesc],
    pub persistence: Persistence,
    pub script_visible: bool,
    pub script_writable: bool,
}

type ValueFn = Box<dyn for<'a> Fn(&EntityRef<'a>) -> Vec<String> + Send + Sync>;

struct Entry {
    info: ComponentInfo,
    values: Option<ValueFn>,
}

/// Names and field metadata for component types, keyed by [`TypeId`] and by stable name.
///
/// [`crate::World::new`] inserts an empty registry. [`crate::World::register_component`] fills it.
/// [`Self::register`] rejects a second registration of the same type. [`Self::register_idempotent`]
/// accepts that second registration when the metadata matches. A second type that reuses a name is
/// rejected either way.
#[derive(Default)]
pub struct ComponentRegistry {
    entries: HashMap<TypeId, Entry>,
    by_name: HashMap<&'static str, TypeId>,
}

impl ComponentRegistry {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get<T: 'static>(&self) -> Option<&ComponentInfo> {
        self.entries.get(&TypeId::of::<T>()).map(|entry| &entry.info)
    }

    pub fn get_by_name(&self, name: &str) -> Option<&ComponentInfo> {
        let type_id = self.by_name.get(name)?;
        self.entries.get(type_id).map(|entry| &entry.info)
    }

    pub(crate) fn values_of(&self, type_id: TypeId, entity: &EntityRef<'_>) -> Option<Vec<String>> {
        self.entries.get(&type_id)?.values.as_ref().map(|values| values(entity))
    }

    pub(crate) fn info_of(&self, type_id: TypeId) -> Option<&ComponentInfo> {
        self.entries.get(&type_id).map(|entry| &entry.info)
    }

    /// Records `desc` for `T`.
    ///
    /// The first registration wins. An identical repeat is [`RegistryError::DuplicateType`].
    /// The same type with different name, fields, persistence, or script flags is
    /// [`RegistryError::ConflictingMetadata`]. A different type that uses the same name is
    /// [`RegistryError::DuplicateName`].
    pub fn register<T: Component>(&mut self, desc: ComponentDesc<T>) -> Result<(), RegistryError> {
        if desc.name.is_empty() {
            return Err(RegistryError::EmptyName);
        }
        validate_fields(desc.name, desc.fields)?;
        let type_id = TypeId::of::<T>();
        let rust_type = type_name::<T>();
        if let Some(existing) = self.entries.get(&type_id) {
            return Err(if same_metadata(&existing.info, &desc) {
                RegistryError::DuplicateType { name: existing.info.name, rust_type }
            } else {
                RegistryError::ConflictingMetadata { rust_type, existing_name: existing.info.name, new_name: desc.name }
            });
        }
        if let Some(owner) = self.by_name.get(desc.name).and_then(|id| self.entries.get(id)) {
            return Err(RegistryError::DuplicateName {
                name: desc.name,
                existing_type: owner.info.rust_type,
                new_type: rust_type,
            });
        }

        let values = desc.values.map(|format_values| {
            let erased: ValueFn = Box::new(move |entity| match entity.get::<&T>() {
                Some(value) => format_values(&value),
                None => Vec::new(),
            });
            erased
        });
        self.by_name.insert(desc.name, type_id);
        self.entries.insert(
            type_id,
            Entry {
                info: ComponentInfo {
                    name: desc.name,
                    type_id,
                    rust_type,
                    fields: desc.fields,
                    persistence: desc.persistence,
                    script_visible: desc.script_visible,
                    script_writable: desc.script_writable,
                },
                values,
            },
        );
        Ok(())
    }

    /// Registers `desc`, and treats an identical existing registration of `T` as success.
    ///
    /// Built-in initialization uses this so a second call keeps the registry as it is.
    /// [`RegistryError::DuplicateName`], [`RegistryError::ConflictingMetadata`], and malformed
    /// field lists are still errors. The first formatter stays; this does not compare function
    /// pointers.
    pub fn register_idempotent<T: Component>(&mut self, desc: ComponentDesc<T>) -> Result<(), RegistryError> {
        match self.register(desc) {
            Err(RegistryError::DuplicateType { .. }) => Ok(()),
            result => result,
        }
    }
}

fn validate_fields(component: &'static str, fields: &[FieldDesc]) -> Result<(), RegistryError> {
    for (index, field) in fields.iter().enumerate() {
        if field.name.is_empty() {
            return Err(RegistryError::EmptyFieldName { component });
        }
        if field.type_name.is_empty() {
            return Err(RegistryError::EmptyFieldType { component, field: field.name });
        }
        if fields[..index].iter().any(|earlier| earlier.name == field.name) {
            return Err(RegistryError::DuplicateFieldName { component, field: field.name });
        }
    }
    Ok(())
}

fn same_metadata<T>(info: &ComponentInfo, desc: &ComponentDesc<T>) -> bool {
    info.name == desc.name
        && info.fields == desc.fields
        && info.persistence == desc.persistence
        && info.script_visible == desc.script_visible
        && info.script_writable == desc.script_writable
}

/// Why [`ComponentRegistry::register`] refused a component.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistryError {
    /// The stable name was `""`.
    EmptyName,
    /// A declared field of `component` has an empty name.
    EmptyFieldName { component: &'static str },
    /// Field `field` of `component` has an empty type name.
    EmptyFieldType { component: &'static str, field: &'static str },
    /// `component` declares `field` more than once.
    DuplicateFieldName { component: &'static str, field: &'static str },
    /// `name` is already the stable name of `existing_type`.
    DuplicateName { name: &'static str, existing_type: &'static str, new_type: &'static str },
    /// `rust_type` is already registered as `name`, with the same metadata.
    DuplicateType { name: &'static str, rust_type: &'static str },
    /// `rust_type` is already registered, and the new metadata disagrees.
    ConflictingMetadata { rust_type: &'static str, existing_name: &'static str, new_name: &'static str },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::EmptyName => f.write_str("component name is empty"),
            RegistryError::EmptyFieldName { component } => {
                write!(f, "component `{component}` has a field with an empty name")
            }
            RegistryError::EmptyFieldType { component, field } => {
                write!(f, "field `{field}` of component `{component}` has an empty type name")
            }
            RegistryError::DuplicateFieldName { component, field } => {
                write!(f, "component `{component}` declares field `{field}` more than once")
            }
            RegistryError::DuplicateName { name, existing_type, new_type } => {
                write!(f, "component name `{name}` is already used by `{existing_type}`, not `{new_type}`")
            }
            RegistryError::DuplicateType { name, rust_type } => {
                write!(f, "component `{rust_type}` is already registered as `{name}`")
            }
            RegistryError::ConflictingMetadata { rust_type, existing_name, new_name } => {
                write!(
                    f,
                    "component `{rust_type}` is already registered as `{existing_name}` and cannot be registered again as `{new_name}` with different metadata"
                )
            }
        }
    }
}

impl std::error::Error for RegistryError {}
