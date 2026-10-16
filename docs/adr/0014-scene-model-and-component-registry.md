# ADR-0014: One world, code-defined scenes, and an explicit component registry

* **Status:** Settled for the scene model and the registry shape. Asset ownership, failed-spawn rollback, and persistent entity ids stay open (Follow-ups).
* **Date:** 2026-10-15
* **Backlog id:** D-18
* **Decided by:** project owner (approved the M4 architecture and the five-phase sequence, with the refinements in this record)
* **Affects:** `engine-world` registry and dumps, `Game` startup, M4 phases 2–5, M18 script exposure, M19 scene files

## Context and problem

M4 needs a scene unit and a way to name components. The world is one `hecs` store (`engine-world`). There is no scene membership, no registry, and no `serde` dependency. D-18 was open. ADR-0010 forbids a second ECS and a new public `hecs` type. ADR-0011 requires a version on the first byte of any persisted format and forbids `hecs` entity indices in files. D-16 (what a file stores for an asset) is still open. M19, not M4, owns scene files, and it waits on M17 and M18 as well.

`hecs` 0.10 `EntityRef::component_types` yields `TypeId`s. `TypeId` has no Rust type name. A dump can name only what was registered, and can still report every other `TypeId`.

## Requirements and constraints

* One spatial world. Scenes instantiate ordinary entities into it. No second ECS, renderer, asset manager, or world model.
* M4 is runtime scene management, registration, hierarchical visibility, name and tag lookup, code-defined prefabs, and read-only text inspection.
* Scene files, `serde`, RON, and JSON are out of M4.
* The registry shape has to leave room for a per-component and per-field script visibility flag (D-41) without running scripts.
* Field metadata is explicit. It is not a claim that it matches the Rust struct automatically.
* Unregistered components are reported, not omitted.
* Public additions follow ADR-0010. `Entity` stays the existing runtime id.

## Options considered

### A. Explicit registration, code-defined scenes, files deferred to M19
* Advantages: matches the roadmap split. The dump and the later serializer share one name table. No new crate and no reflection framework.
* Disadvantages: every built-in component needs a hand-written field list. A struct change does not update that list by itself.

### B. Derive-based reflection now
* Advantages: field lists would follow the struct.
* Disadvantages: a reflection framework, and a `serde` dependency, before M19 has a format or D-16 has an id. D-41's leaning is a hand-written script facade, not "expose every field".

### C. Wait until M19 to name components
* Advantages: less code in M4.
* Disadvantages: the M4 dump criterion needs names and fields now. M18 and M22 would then retrofit the registry onto components that had none.

## Decision and rationale

A, with the refinements below. The trade-off is hand-written metadata in exchange for not pretending the registry is a mirror of Rust.

### Settled

1. One `World` holds runtime entities and, when Phase 4 exists, multiple scene instances.
2. A scene definition is code. A scene instance is runtime state in that same world.
3. Scene-owned entities will carry `SceneMember`. Phase 4 adds the component. Phase 1 does not.
4. Components are registered explicitly with a stable name, a `TypeId`, a declared field list, a debug formatter, a persistence class, and script visibility flags.
5. Hierarchical visibility is resolved while extracting. It does not write a cached visibility component. Absence of `Visibility` means that entity is locally visible. An entity is omitted when it or any ancestor in its `Parent` chain has `Visibility::Hidden`. Local components are left as written. The walk stops on a repeated entity id and on a parent id that has no further `Parent`, including a stale id. Sprites, meshes, tilemaps, and lights use that rule. `Camera.active` and `order` still choose the camera; `Visibility` does not activate or deactivate one. A renderable parented under a hidden camera is still omitted. `AmbientLight` and `Fog` are resources and are not part of the chain. Depth testing and `RenderOrder` are unchanged.
6. Registry types live in `engine-world`. `engine` registers world, renderer, and animation components. `Game::new` does that once.
7. A code-defined prefab is a function that instantiates entities into the existing `World`. It is not a second object model. Phase 4 documents one such function. Phase 1 does not add it.
8. M4 writes no scene file.
9. M4 adds no `serde`, RON, or JSON dependency.

`register` rejects an empty component name, an empty field name, an empty field type name, a repeated field name, a second type using the same name, a second registration of the same type with the same metadata (`DuplicateType`), and a second registration of the same type whose name, fields, persistence, or script flags differ. The first registration stays. `register_idempotent` is the same check, except `DuplicateType` is success and the first formatter stays. Built-in initialization uses that path, so calling `register_builtin_components` again on an unchanged registry returns `Ok`. A conflicting name or field list is still an error. `register` itself still returns `DuplicateType`.

Script flags are inert. No system reads them to allow or deny anything.

`Persistence::Persist` and `Persistence::Runtime` are classifications for a future serializer. M4 does not write either kind. `Parent`, `Children`, `GlobalTransform`, `Interpolate`, and the animation instance components are `Runtime` because they are derived, hold playback clocks, or store `Entity` values. `GlobalTransform`'s dump value is the translation only. The field is still declared as `Mat4`, so the summary is not presented as the whole matrix. `TileMap` tile values are a length. `SkeletonInstance`'s private name index is omitted because it mirrors `bones`.

The registry is a resource inserted by `World::new`. Dumps borrow that resource and `World.entities` as sibling fields, so the `RefCell` guard does not cover entity storage. A mutable borrow of the registry held by the caller panics on dump, which is the existing resource rule. `EntityRef` stays inside the registry. The public formatter is `fn(&T) -> Vec<String>`, one string per declared field, in field order. A missing formatter prints the fields without values. A formatter whose length disagrees is reported and is not applied onto the wrong fields. The formatter function is not part of the metadata comparison, and a panic inside it is not caught. The field list is not checked against the Rust struct: a renamed field, a changed type, or a new struct field stays as written until someone edits the registration.

Debug text is deterministic for an unchanged world: roots follow `Entity`'s `Ord`, children follow the `Children` list, registered names are alphabetical, and unregistered `TypeId`s are ordered by their `Debug` text. An entity that cannot be reached from a root is listed under `unreached`. A cycle prints `(already listed)` once and stops. `dump_entity` on a dead or unknown id returns `DumpError::NoSuchEntity`. The printed entity token is `Entity`'s `Debug` form (`{id}v{generation}`), which is the runtime id, not a persistent id.

These types are contract (ADR-0010): `ComponentRegistry`, `ComponentDesc`, `ComponentInfo`, `FieldDesc`, `Persistence`, `RegistryError`, `DumpError`, and `register_builtin_components`. They are in the prelude. No new `hecs` type is re-exported.

### Follow-ups that stay open

**Asset ownership.** Scene membership does not own assets. `SceneManager` will not delete assets from a retain count. These cases have to stay safe: several scenes sharing one asset, a runtime entity using an asset a scene also uses, an asset the application loaded itself, a spawn that fails, and a scene that is unloaded and loaded again. Unload must not invalidate an asset something else still needs.

The smallest safe rule, until a later measurement says otherwise: scene code does not call `AssetManager::remove` because an entity disappeared. The application removes assets it owns. The existing GPU cache still drops a copy when `remove` is called, and still drops an unused copy after the idle window. That is not general-purpose reference counting, and it does not decide D-16.

An asset that is exclusively owned may be released when that exclusive ownership can be shown. M4 does not yet have a way to show it for an arbitrary handle, because handles are name hashes and the manager does not track users. The M4 GPU-baseline test, when Phase 4 writes it, uses assets inserted only for that test and calls `remove` itself after unload, then renders one frame. That is the test's ownership, not `SceneManager`'s.

**Spawn rollback.** A closure with `&mut World` can change entities and resources that already existed. Despawning the entities it created does not undo those changes. Full atomicity is not claimed. Phase 4 starts with a feasibility spike against `World`'s borrows and the command buffer, using a constrained builder that records the entities it creates. The spike has to show: new entities are tracked, parent and child links stay valid, a failed create leaves none of its entities, a failed replace keeps the previous instance, asset changes are either forbidden or recorded as non-transactional, and unrelated runtime entities are not destroyed. Descendants count, not only the entities the builder spawned directly. Until that spike, the Phase 4 API is not frozen.

**Identity.** Runtime ids are the existing `Entity` values (index and generation). `SceneInstanceId` will be a runtime instance id. Scene-local ids map entities inside one instance. A `u32` assigned in spawn order is acceptable for M4's code-defined scenes. It is not a persistent id. Editing the spawn function can renumber it. M19 is not required to serialize spawn-order ids. A later explicit stable id can be a separate field. Phase 4 must not treat the spawn-order number as the file id.

**Direct despawn.** Phase 4's map stores the `Entity`, generation included. A lookup returns it only when that entity is alive and its `SceneMember` still carries the same instance id and local id. `World::despawn` outside `SceneManager` makes `contains` false for that handle. hecs will not hand the same `Entity` value to a later spawn, so a stale map entry cannot point at the replacement. The entry is detectable and can be dropped by a later sweep. Phase 1 does not implement the map. `dump_entity` already uses the generation check: a despawned id is `NoSuchEntity`.

## Consequences and follow-up

* Phase 1 is the registry, built-in registration, and the text dumps. It is commit `cdb5ec3`, local, not pushed.
* Phase 2 is hierarchical visibility in `extract_scene`, as item 5 describes. It is commit `2c76f71`. Sprites, meshes, tilemaps, and lights share one ancestor walk (`AncestorVisibility::hides`).
* Phase 3 is name and tag lookup on the same `World`. `find_by_name` and `find_by_tag` scan live entities and return every exact match in `Entity` order. `Tags` stores plain strings, rejects `""`, keeps the first copy of a duplicate, and does not follow `Parent`. It is `Persistence::Persist` in the registry. There is no name index and no tag index. Phase 3 is checkpointed locally with this record. Phase 4 is instances, membership, and the code-defined spawn function, after the feasibility spike. Phase 5 is the `spatial_scene` switch.
* M19 still chooses the file encoding, under ADR-0011, after D-16. RON remains a candidate. It is not a dependency.
* D-16 and D-41 stay open. D-41 may later read the flags this registry stores.

## Evidence and revisit conditions

Phase 1 tests cover duplicate names, duplicate types, conflicting metadata, empty and repeated field names, declared fields, a missing formatter, a formatter length mismatch, unregistered `TypeId`s, hierarchy order, empty worlds, missing names, cycles, unreached entities, and dead entities. `Game::new` registers the built-ins. A second `register_builtin_components` on that world returns `Ok` and keeps the first formatters. A name already used by a different type is still `DuplicateName`.

Reopen the settled items only under the rule at the bottom of DECISIONS.md. Revisit asset ownership when some caller can prove an asset is unused by every remaining entity and system. Revisit rollback when the Phase 4 spike shows the constrained builder cannot track descendants with the current command buffer. Revisit scene-local ids when M19 has a real file and D-16 has settled asset ids.
