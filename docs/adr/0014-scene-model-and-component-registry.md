# ADR-0014: One world, code-defined scenes, and an explicit component registry

* **Status:** Settled for the scene model, the registry, membership, synchronous load, unload, and replace, deferred scene operations, the `spatial_scene` switch, and one code-defined prefab. `create_scene_instance` and `track_scene_entity` are crate-private. D-16 stays open. Scene files stay in M19. M4 is complete in this working tree. M4-5 is committed locally and is not pushed. Nothing in M4 is pushed.
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
3. Scene-owned entities carry `SceneMember`. M4-4A adds the component, `SceneInstanceId`, and the internal instance map. Phase 1 did not. M4-4A does not spawn, unload, or replace a scene.
4. Components are registered explicitly with a stable name, a `TypeId`, a declared field list, a debug formatter, a persistence class, and script visibility flags.
5. Hierarchical visibility is resolved while extracting. It does not write a cached visibility component. Absence of `Visibility` means that entity is locally visible. An entity is omitted when it or any ancestor in its `Parent` chain has `Visibility::Hidden`. Local components are left as written. The walk stops on a repeated entity id and on a parent id that has no further `Parent`, including a stale id. Sprites, meshes, tilemaps, and lights use that rule. `Camera.active` and `order` still choose the camera; `Visibility` does not activate or deactivate one. A renderable parented under a hidden camera is still omitted. `AmbientLight` and `Fog` are resources and are not part of the chain. Depth testing and `RenderOrder` are unchanged.
6. Registry types live in `engine-world`. `engine` registers world, renderer, and animation components. `Game::new` does that once.
7. A code-defined prefab is a function that instantiates entities into the existing `World`. It is not a second object model. Phase 4 documents one such function. Phase 1 does not add it.
8. M4 writes no scene file.
9. M4 adds no `serde`, RON, or JSON dependency.

`register` rejects an empty component name, an empty field name, an empty field type name, a repeated field name, a second type using the same name, a second registration of the same type with the same metadata (`DuplicateType`), and a second registration of the same type whose name, fields, persistence, or script flags differ. The first registration stays. `register_idempotent` is the same check, except `DuplicateType` is success and the first formatter stays. Built-in initialization uses that path, so calling `register_builtin_components` again on an unchanged registry returns `Ok`. A conflicting name or field list is still an error. `register` itself still returns `DuplicateType`.

Script flags are inert. No system reads them to allow or deny anything.

`Persistence::Persist` and `Persistence::Runtime` are classifications for a future serializer. M4 does not write either kind. `SceneMember`, `Parent`, `Children`, `GlobalTransform`, `Interpolate`, and the animation instance components are `Runtime` because they are derived, hold playback clocks, or store runtime ids. `SceneMember` stores a `SceneInstanceId` and a scene-local `u32`, not a file id. `GlobalTransform`'s dump value is the translation only. The field is still declared as `Mat4`, so the summary is not presented as the whole matrix. `TileMap` tile values are a length. `SkeletonInstance`'s private name index is omitted because it mirrors `bones`.

The registry is a resource inserted by `World::new`. Dumps borrow that resource and `World.entities` as sibling fields, so the `RefCell` guard does not cover entity storage. A mutable borrow of the registry held by the caller panics on dump, which is the existing resource rule. `EntityRef` stays inside the registry. The public formatter is `fn(&T) -> Vec<String>`, one string per declared field, in field order. A missing formatter prints the fields without values. A formatter whose length disagrees is reported and is not applied onto the wrong fields. The formatter function is not part of the metadata comparison, and a panic inside it is not caught. The field list is not checked against the Rust struct: a renamed field, a changed type, or a new struct field stays as written until someone edits the registration.

Debug text is deterministic for an unchanged world: roots follow `Entity`'s `Ord`, children follow the `Children` list, registered names are alphabetical, and unregistered `TypeId`s are ordered by their `Debug` text. An entity that cannot be reached from a root is listed under `unreached`. A cycle prints `(already listed)` once and stops. `dump_entity` on a dead or unknown id returns `DumpError::NoSuchEntity`. The printed entity token is `Entity`'s `Debug` form (`{id}v{generation}`), which is the runtime id, not a persistent id.

These types are contract (ADR-0010): `ComponentRegistry`, `ComponentDesc`, `ComponentInfo`, `FieldDesc`, `Persistence`, `RegistryError`, `DumpError`, `register_builtin_components`, `SceneInstanceId`, `SceneMember`, `SceneError`, and, from M4-4B, `SceneBuilder`. They are in the prelude. `SceneManager` is not. No new `hecs` type is re-exported.

### Accepted for M4-4A (2026-10-15)

The feasibility spike's scene-management shape is accepted, with these rules. M4-4A implements identity, membership, and the instance map. It does not implement creation of a scene graph, rollback, unload, replace, or deferred scene commands.

1. One existing `World` holds runtime entities and multiple scene instances.
2. `SceneManager` is an internal `World` resource, inserted by `World::new`.
3. Public scene operations are `World` methods, so a `SceneManager` guard is not held across `spawn`.
4. `SceneBuilder` is how a scene closure creates entities. It does not hand out `&mut World`. `create_scene_instance` and `track_scene_entity` are `pub(crate)`. Nothing outside `engine-world` calls them. The developer-facing lifecycle is `load_scene`, `unload_scene`, and `replace_scene`.
5. An entity belongs to at most one scene instance.
6. `SceneInstanceId` is a runtime id. Issued ids start at 0 and increase. An id is not issued twice, including after a failed creation that consumed one.
7. Scene-local `u32`s are unique inside one instance, may repeat in another, and are not file ids. `load_scene` assigns them in spawn order. `track_scene_entity` assigns them in track order. A future stable id can be a separate field. Spawn order is not that field.
8. `SceneMember` is `Persistence::Runtime`.
9. Scene creation is synchronous. `load_scene` runs the builder and publishes the instance only after that returns `Ok`. The crate-private `create_scene_instance` still publishes an empty instance immediately.
10. Scene asset serialization stays in M19.
11. Scene construction does not insert or remove assets. `SceneBuilder` has no asset or resource methods.
12. Scene membership does not own referenced assets. Unload does not call `AssetManager::remove`.
13. Unload preserves entities that are not validated members of that instance, including other scenes and runtime entities.
14. A later unload lifts a non-member child to its world transform by walking that entity's parent chain. It does not run a full `propagate_transforms`, which would insert `GlobalTransform` on unrelated entities. M4-4A does not lift.
15. `World` hierarchy, transforms, the existing ECS, and extraction stay authoritative. `World::despawn` is unchanged and does not update the scene map.
16. Deferred scene work, when added, uses the existing command queue. There is no second queue. M4-4A does not enqueue scene commands.

Borrowing. A scene method takes `SceneManager` out of the resource map and inserts that same value before it returns `Ok` or `Err`. An ordinary error does not leave the manager missing. A nested call while the manager is out returns `SceneError::SceneManagerUnavailable` and does not insert a replacement. A panic inside the method is not a transaction: the insert does not run. No `unsafe` is used.

Stale rows. `scene_entity` returns a recorded entity only when it is alive and its `SceneMember` names that instance and local id. A dead id, a reused hecs slot, a removed component, or a component that names a different id does not resolve. The row stays until `sweep_scene_mappings`. Sweep drops those rows and does not reuse the local id or the instance id. Empty instances remain.

### M4-4B load and rollback (2026-10-15)

`World::load_scene` takes the manager out, issues a `SceneInstanceId`, and runs `SceneBuilder`. The builder can `spawn` and `spawn_child`. `spawn_child` accepts only a parent this build created. A `Parent` component aimed at any other entity fails validation. The closure does not receive `&mut World`, `CommandWriter`, or `AssetManager`.

On `Ok`, members are assigned in spawn order and the instance is inserted. On `Err`, created entities are despawned, an empty `Children` component this build introduced on a surviving entity is removed, and the instance is not inserted. The id stays consumed. Unrelated entities and resources are left as they were. Ordinary `Result` paths put the same manager value back. A panic does not. A nested scene operation while the manager is out returns `SceneManagerUnavailable` and does not insert a replacement.

`create_scene_instance` and `track_scene_entity` stay public. They can publish an empty instance or attach an existing entity outside `load_scene`. Making them `pub(crate)` later would break that surface. Nothing in the examples calls them. The recommendation is to keep them documented as low-level until a later checkpoint, then hide them if no caller needs them. This checkpoint does not change their visibility.

### M4-4C unload and replace (2026-10-15)

`unload_scene` checks the instance, then the live validated members, before it changes anything. A stale map row (dead entity, or a live entity whose `SceneMember` does not match) is not a member and is not despawned. The row disappears with the instance. A live entity that claims the instance while the map names a different entity is `InconsistentMembership`, and the world is unchanged. A parent cycle, a `Parent`/`Children` pair that does not agree, or a lifted transform that is not finite is the same. An unknown or already unloaded id is `NoSuchInstance`.

Destruction uses `despawn_one`, not `World::despawn`, so a non-member child is not removed with a member parent. A non-member whose parent is a member is lifted first. The lift walks that entity's parent chain into a matrix and stores `Transform::from_matrix`. Shear is discarded. Uniform scale, and a zero scale axis that still decomposes to a finite TRS, are stored as that TRS. A non-finite result refuses the unload. Interpolation history on the lifted entity is snapped to the new local. Descendants keep their locals and history. `GlobalTransform` is not written.

`replace_scene` preflights the old instance, then uses the same `load_scene` build path. A failed build leaves the old instance published and does not publish the new one. The new id is consumed. On success the old id is removed and is not reused. `SceneBuilder` cannot modify an entity it did not create. `validate_build` rejects a bundled `SceneMember`, a parent outside the build, or a child link that leaves the build, and it rolls those entities back before publish. After a successful preflight, the original unload therefore completes for engine-maintained state. The guard that would unload a published replacement if that unload returned an error is not reached through the public method. A panic is not undone, and a lift that already happened is not rolled back.

`Transform::from_matrix` stores translation, rotation, and scale. It drops shear. That is the existing decomposition, and unload uses it on purpose. A non-uniform parent scale combined with a rotated child is stored as that TRS and is not the exact world matrix. M4 does not change `Transform`.

`create_scene_instance` and `track_scene_entity` stay public methods on `World`. They can assemble membership outside load, unload, and replace. No example and no crate outside `engine-world`'s own tests calls them. The supported lifecycle is `load_scene`, `unload_scene`, and `replace_scene`. Before a stable public release they should become `pub(crate)`. This checkpoint does not change their visibility, so an external caller written against them still compiles.

Deferred commands and `SceneOpId` are not in M4-4C.

### M4-4D deferred scene operations (2026-10-16)

`CommandWriter::load_scene`, `unload_scene`, and `replace_scene` append to the existing command queue. There is no second queue and no async runtime. The flush still takes `Commands` out of the world, so those closures call the synchronous scene methods and do not enqueue another command.

Each call that is accepted returns a `SceneOpId` before the flush. The id is not a `SceneInstanceId` and it is not stored in a file. Issuing it stores `SceneOpStatus::Pending` on `SceneManager`. A missing manager returns `SceneError::SceneManagerUnavailable` and issues no id. A missing `Commands` resource is still the existing panic. This path does not add one.

`World::scene_operation` copies the status and leaves it stored. `Pending` means the flush has not run it. After the flush the status is `Loaded`, `Unloaded`, `Replaced`, or `Failed { error }`. `Failed` uses `SceneError`. A failed load or replace publishes nothing from that attempt. The command closure still returns `Ok` to the queue, so `CommandError` and `command_failures` stay dispatch records. `Diagnostics.command_failures` does not count a scene failure. Later commands in the same flush still run. The failure is logged once, through the `log` crate, when the status becomes `Failed`. A later step does not log that same record again. Completed results still accumulate until dismiss or clear.

`dismiss_scene_operation` removes one completed status and returns it. A pending id returns `OperationPending` and stays queued. `clear_scene_operations` removes every completed status and leaves pending ones. `Game::step` does not clear them. Unclaimed results stay until one of those calls. An unknown id is `NoSuchOperation`.

The operation runs at the existing flush: after the stage's systems, and between fixed iterations. A later system in the same stage still sees `Pending`. The next stage, and the next fixed iteration, sees the completed status. M4-4D is commit `8398e49`. The example that consumes these results is M4-4E.

### M4-4E runtime switching in `spatial_scene` (2026-10-16)

`build_world` registers mesh assets once, then `load_scene(build_environment_a)` publishes the original environment. `build_environment_b` is a smaller yard: floor tiles, a lit cube, a pillar, a figure, a tree, and one point light, using those same assets. Both functions take `SceneBuilder` and can run more than once. The example does not insert `SceneMember` itself and does not call `create_scene_instance` or `track_scene_entity`.

The camera and the walker are `World::spawn` calls after that load. They are not members of either instance. A replacement despawns the published environment and leaves those entities, their `Entity` values, and their transforms. The camera pose and projection stay on the example `Scene` resource and are written to the same camera entity. There is no persistent-entity subsystem.

Tab is bound to the action `switch_environment`. `request_environment_switch` runs in `update` and reads the frame-scoped press, so holding Tab queues one replacement. A press while a switch is already pending queues nothing. The call is `CommandWriter::replace_scene`. The returned `SceneOpId` is stored. `Scene.instance` stays unchanged until the result is `Replaced`.

`apply_environment_switch` runs in `post_update`, before `propagate_transforms`. `Pending` stays stored. `Replaced` is dismissed, and the new id becomes the active instance. `Failed` is dismissed and the active id stays. That failure was already logged when the status was stored; the example does not log it again and does not record a `CommandError`. `Loaded` or `Unloaded` is an unexpected result for this action. The example logs the instance id, says it does not own that instance, and dismisses the operation record. An unexpected load stays published. An unexpected unload is left as the engine applied it. This switch does not unload or rebuild either one, and it does not change the tracked id. Completed records are not left for a later frame.

A later `update` system still sees the old members and `Pending`. `animate` sees the completed replacement. The tracked id changes in `post_update`. No stage was added. Shared assets stay registered. D-16 stays open.

`--environment b` publishes B with synchronous `replace_scene` before the first step, for a screenshot. The default launch and the default screenshot path stay on A. M4-4E is commit `d58c66b`.

### M4-5 code-defined prefab and API cleanup (2026-10-16)

`spatial_scene::prefab::spawn_lamp` is the documented prefab. It takes `&mut World` and a position and returns the post `Entity`. The post is a lit `pillar` mesh. A point light is a child, offset toward the top of the pillar. Each call creates a new hierarchy. `World::despawn` on the root removes the light. The function does not register assets and does not insert `SceneMember`.

`place_lamp` builds the same parts with `SceneBuilder::spawn` and `spawn_child`. A scene closure uses that form. Those entities are members on success and are rolled back if the closure returns `Err`. `SceneBuilder` still does not hand out `&mut World`, so a scene cannot call `spawn_lamp` and keep the builder's rollback.

`create_scene_instance` and `track_scene_entity` are `pub(crate)`. Inspection stays public: `scene_entity`, `scene_entities`, `scene_instances`, `scene_membership`, `scene_operation`, `dismiss_scene_operation`, and `clear_scene_operations`. `SceneManager` stays internal. No new `hecs` type is public.

This checkpoint is committed locally and is not pushed. It does not add prefab assets, a prefab manager, or a scene file.

### Follow-ups that stay open

**Asset ownership.** Accepted: membership does not own assets, and scene code does not call `AssetManager::remove` because an entity disappeared. Shared use by another scene, a runtime entity, or the application stays valid. D-16 is still open. Handles are name hashes, and the manager does not track users, so M4 still cannot prove exclusive ownership of an arbitrary handle. The GPU-baseline test, when unload exists, inserts assets only that test uses, calls `remove` itself after unload, and renders one frame.

**Spawn rollback.** Implemented for `load_scene` and for a failed `replace_scene` build, for `Result` errors only. A panic while the manager is out of the world is not restored. Unload of a consistent instance is preflighted, then it mutates. It is not a panic-safe transaction.

**Identity.** Settled for M4 in "Accepted for M4-4A" above. M19 still chooses which id a file stores. Spawn-order local ids are not that id.

**Direct despawn.** Implemented for lookup and sweep. `World::despawn` does not update the map. `scene_entity` validates the live `Entity` and `SceneMember`. `sweep_scene_mappings` drops stale rows without reusing ids.

## Consequences and follow-up

* Phase 1 is the registry, built-in registration, and the text dumps. It is commit `cdb5ec3`, local, not pushed.
* Phase 2 is hierarchical visibility in `extract_scene`, as item 5 describes. It is commit `2c76f71`. Sprites, meshes, tilemaps, and lights share one ancestor walk (`AncestorVisibility::hides`).
* Phase 3 is name and tag lookup on the same `World`. `find_by_name` and `find_by_tag` scan live entities and return every exact match in `Entity` order. `Tags` stores plain strings, rejects `""`, keeps the first copy of a duplicate, and does not follow `Parent`. It is `Persistence::Persist` in the registry. There is no name index and no tag index. Phase 3 is commit `f9a4aab`.
* M4-4A is scene identity and membership: `SceneInstanceId`, `SceneMember`, `SceneError`, an internal `SceneManager`, and `World` methods that create an empty instance, track an existing entity, look it up, and sweep stale rows. It is commit `302622b`.
* M4-4B is `SceneBuilder` and synchronous `load_scene`, including rollback of a failed build. It is commit `128c70e`.
* M4-4C is synchronous `unload_scene` and `replace_scene`. It is commit `e12fb5b`.
* M4-4D queues load, unload, and replace on the existing command queue and stores a `SceneOpStatus` until the caller clears it. It is commit `8398e49`.
* M4-4E switches `spatial_scene` between two code-defined environments through that queue. The camera and the walker are not scene members. It is commit `d58c66b`.
* M4-5 documents `spawn_lamp` as the code-defined prefab and makes `create_scene_instance` and `track_scene_entity` crate-private. It is committed locally and is not pushed. M4 is complete in this working tree and is not pushed.
* M19 still chooses the file encoding, under ADR-0011, after D-16. RON remains a candidate. It is not a dependency.
* D-16 and D-41 stay open. D-41 may later read the flags this registry stores.

## Evidence and revisit conditions

Phase 1 tests cover duplicate names, duplicate types, conflicting metadata, empty and repeated field names, declared fields, a missing formatter, a formatter length mismatch, unregistered `TypeId`s, hierarchy order, empty worlds, missing names, cycles, unreached entities, and dead entities. `Game::new` registers the built-ins. A second `register_builtin_components` on that world returns `Ok` and keeps the first formatters. A name already used by a different type is still `DuplicateName`.

M4-4A tests cover distinct instance ids, repeated local ids on distinct entities, missing instances and local ids, entities with no membership, despawn, a reused hecs slot, a missing or wrong `SceneMember`, ordinary errors restoring the manager, a missing manager not being replaced, registry metadata, and dump text. That checkpoint is commit `302622b`. `scripts/check.sh` then passed with 318 tests.

M4-4B tests cover an empty load, one entity, a parent with children, a deep chain, repeated local ids on distinct entities, lookup, failure before any spawn, failure after one spawn, failure after a nested hierarchy, rejection of an outside parent, a `Parent` bundled onto an outside entity, rollback of entities and links, unchanged bystanders and resources, empty `Children` cleanup, no published instance, the manager remaining available, consumed ids, and a nested load. An extraction test compares a scene sprite with a directly spawned sprite. `scripts/check.sh` on 2026-10-15 passed with 328 tests. The four screenshots match commit `6dbabb8`. That checkpoint is commit `128c70e`.

M4-4C tests cover an empty unload, one entity, a nested hierarchy, siblings, an unknown id, a second unload of the same id, other instances and runtime entities remaining, a runtime child, a deep runtime descendant, a runtime subtree, a member under a runtime parent, a member under a runtime entity that sits under another member, scene B under scene A, several ownership transitions, closed `Parent` and `Children` links, uniform-scale world transforms, the non-uniform-plus-rotation TRS approximation, interpolation snap on the lifted entity, unchanged descendant history, unchanged bystander `GlobalTransform` values, stale rows, a forged `SceneMember`, a broken parent or child link, a non-finite lift, a zero scale axis, replacement of an empty and a populated instance, a failed replacement before any spawn and after nested spawns, consumed ids, and a missing manager. Shared assets used by two instances and by a runtime entity, and an application asset the scene never references, stay after unload. Reloading still resolves the same sprite. The GPU test starts from a camera-only cache of 0 textures and 0 meshes, records 2 textures and 1 mesh after the scene renders, and returns to that camera baseline after unload, an explicit `AssetManager::remove` of the test-owned assets, and another frame. `scripts/check.sh` on 2026-10-16 passed with 341 tests, including the replacement invariant test. The four screenshots match commit `6dbabb8`. That checkpoint is commit `e12fb5b`.

M4-4D tests cover a pending operation, a query that does not consume the status, refusal to dismiss a pending id, clearing completed results while a pending one stays, an unknown operation id, a missing manager at queue time, a failed load before and after nested spawns, a failed unload, a failed replacement that keeps the original, a successful unload that lifts a runtime entity, a successful replacement, FIFO instance ids, a later spawn command after a scene failure, and separation from `command_failures`. `Game::step` tests cover same-stage invisibility, visibility in `post_update`, and a flush between fixed iterations. `scripts/check.sh` on 2026-10-16 passed with 348 tests, including the one-time scene-failure log. The four screenshots match commit `6dbabb8`. That checkpoint is commit `8398e49`.

M4-4E tests in `examples/spatial_scene/tests/switching.rs` cover the initial environment, a pending replacement, dismissal, A to B and back, a repeated round trip, membership, no leftover members, the camera and walker surviving, extraction under every named camera, shared asset revisions, a rejected replacement, one queue entry for two request calls in one press, a held Tab, an unexpected load left published, and an unexpected unload left as the engine applied it. Perspective and orthographic renders of B differ from A and report one light. The check recorded before the unexpected-unload test reported 359 tests; that commit includes the twelfth switching test. The four original screenshots still match commit `6dbabb8`. Environment B's hashes are recorded in TASKS.md and do not replace those baselines. This checkpoint is commit `d58c66b` and is not pushed.

M4-5 tests in `examples/spatial_scene/tests/prefab.rs` cover one lamp, two distinct hierarchies, the post and light transforms, extraction through the existing scene, despawn of the root, scene membership through `place_lamp`, and rollback of a failed build. `scripts/check.sh` on 2026-10-16 passed with 365 tests, 0 failed, 0 ignored. `headless.rs` ran 20 tests on Apple M5, Metal. The four original screenshots still match commit `6dbabb8`. Environment B's perspective and orthographic hashes are unchanged from M4-4E. This checkpoint is committed locally and is not pushed.

Reopen the settled items only under the rule at the bottom of DECISIONS.md. Revisit asset ownership when some caller can prove an asset is unused by every remaining entity and system. Revisit the deferred scene-operation record if the stored `SceneOpId` cannot be read after flush without holding `SceneManager` across the command queue. Revisit scene-local ids when M19 has a real file and D-16 has settled asset ids.
