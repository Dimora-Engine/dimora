# ADR-0025: Kinematic collision uses a private Parry query index

* **Status:** Settled
* **Date:** 2026-10-27
* **Backlog id:** D-09
* **Decided by:** project owner (approved the direction and the corrections in this record)
* **Affects:** M7 collision components, `CollisionWorld`, the fixed-step collision system, M9 character roots, M13 tile collision later

M7 code does not exist yet. This record is the contract for that implementation. It does not add the crate.

*Amendment, 2026-10-27.* M7-1A later added the private `parry3d` dependency and the query index. The sentence above is this record at settlement. The slide, triggers, and `CharacterMotor` are still not implemented.

## Context and problem

M7 needs overlap, ray casts, triggers, and a kinematic slide in the same 3D world as rendering. It does not need a rigid-body solver. `VISION.md` keeps that solver, ragdolls, cloth, and vehicles as non-goals.

`Transform` is the gameplay pose. `propagate_transforms` writes `GlobalTransform` and, today, runs only in `post_update`. `fixed_update` can run up to eight times per `Game::step` and does not propagate. Events stay pending until the promote after the whole fixed loop (`ADR-0008`). Simulation stays on the calling thread (`ADR-0006`). There is no change detection (`ADR-0009`).

A library that owns body positions would fight that model. A hand-written narrowphase for oriented boxes and capsules would be the long-term bug source. The spike below checks a query-only dependency against the published crate.

## Requirements and constraints

* Sphere, box, and capsule in world space. Equal X/Y with a Z gap larger than the extents do not overlap.
* Layers and masks, sensors, and trigger events on the existing queues.
* A swept slide that does not tunnel, and that does not lengthen a diagonal move into a wall.
* Locked movement axes stay locked in world space for the whole resolution.
* The camera is not an input.
* The same inputs over 1,000 fixed steps hash the same in one process. Cross-platform bitwise identity is not required.
* Public types do not include Parry types.
* Colliders sit on the gameplay entity, not on a visual child.

## Options considered

### A. Custom shapes and a grid

* Advantages: no dependency; ordering is ours.
* Disadvantages: oriented box and capsule contact and sweeps are the part that fails quietly.

### B. Rapier as the simulation world

* Advantages: a known character controller and a solver.
* Disadvantages: a second pose owner, a dynamics pipeline M7 must not step, and `nalgebra` plus `glamx` beside Dimora's math. `rapier3d` 0.36.1 is MSRV 1.86 and builds on this toolchain, and it is not the choice.

### C. Private Parry queries, ECS-owned transforms

* Advantages: the published 0.31.1 crate already has the three shapes, overlap, contact, rays, and translational casts. The ECS keeps the pose. The public API stays Dimora's.
* Disadvantages: sweep normals need an explicit frame conversion, and the slide policy is ours.

## Decision and rationale

C. `engine-world` will depend on `parry3d` 0.31.1 when M7 is implemented, and not before:

```toml
parry3d = { version = "0.31.1", default-features = false, features = ["dim3", "f32", "std", "alloc"] }
```

The published `[lib] required-features = ["required-features"]` does not have to be named again. Cargo 1.90.0 and Cargo 1.99.0 both built that library for a downstream crate with `dim3`, `f32`, and `std`. M7-1B-1 adds `alloc` because support-map shape casts are compiled only with that feature, and `std` does not turn it on. `spade`, `parallel`, `simd8`, `serde-serialize`, and `enhanced-determinism` stay off.

The crate's math is `glamx`. `Vector` is `Vec3`, `Pose` is `Pose3`, and `Rotation` is `glam::Quat`. Mapping from Dimora is a component copy. Both use the right-hand rule. A `+90°` yaw about Y sends local +X to world −Z. "Left-handed" in this engine is the camera looking along +Z. Collision does not read a camera.

Published query results, checked by a throwaway probe (not committed):

| Call | Result frame |
|---|---|
| `intersection_test` | `Result<ShapeIntersection, Unsupported>`. The boolean is `ShapeIntersection.intersecting`. It is not a `bool`. |
| `contact` | World space, after `transform_by_mut`. `normal1` points from shape 1 toward shape 2. `dist < 0` is penetration. |
| `RayCast::cast_ray_and_get_normal` | World space. The implementation runs `transform_by` on the local hit. A unit direction makes the time of impact a distance. |
| `cast_shapes` | **Not converted to world.** With the mover at the identity, `normal1` for a +X hit was +X. With the mover yawed `+90°` about Y and travelling along world −Z, `normal1` was still local +X. The world wall normal is `-(pose.rotation * normal1)`. |

`Ball::new(radius)`, `Cuboid::new(half_extents)`, and `Capsule::new_y(half_height, radius)` are the constructors. The capsule is centered and runs along local +Y. `cast_shapes` does not rotate the shape during the sweep.

The crate metadata is Apache-2.0. The unpacked 0.31.1 package has no `NOTICE` and no `LICENSE` file. This commit does not add the dependency. KI-17 already defers third-party notices to M24. When M7 adds the crate, that notices file must include the Apache-2.0 text for `parry3d`. Dimora's own licence stays the proposed `MIT OR Apache-2.0` (OD-3). This decision does not confirm OD-3.

There is no `rust-version` in the published manifest. `cargo +1.99.0 run` and `cargo +1.90.0 run` both printed `parry 0.31.1 probe ok` on this machine (Apple arm64, 2026-10-27). The 1.90 link needed `SDKROOT` pointed at `MacOSX26.5.sdk`; the default macOS 27 SDK failed in `proc-macro2`'s build script with an unknown `arm64e.x1` architecture, before Parry compiled. That failure is the local SDK, not the crate. Parry does not raise Dimora's 1.90 floor. `ordered-float` 5 is already why that floor exists, and this crate depends on it too.

## Public data

Persistent and inspectable. `offset` lives on the collider, whether or not a motor is present. It is the entity-local point, default zero, in the same unscaled local space as the entity's `Transform` children.

```rust
struct Collider {
    shape: ColliderShape,
    offset: Vec3,
    layers: u32,   // default 1
    mask: u32,     // default u32::MAX
    sensor: bool,  // default false
    enabled: bool, // default true
}

struct CharacterMotor {
    skin: f32,          // default 0.01, finite, >= 0
    max_slides: u8,     // default 4
    locked_axes: BVec3, // default none; world axes
}

struct MoveIntent {
    translation: Vec3, // world-space displacement for this fixed iteration
}
```

`radius` and each half-extent must be finite and `> 0`. `half_height` must be finite and `>= 0`. A bad value skips that collider. `MoveIntent` is not saved and is not gameplay state that survives a step.

`CollisionWorld` is a resource: a `Vec` of entries sorted by `hecs::Entity`, the sorted sensor-pair list, and the previous skip set. Candidate tests are a private function over that `Vec`. No Parry type is public.

The shape center is the full matrix, including signed scale:

```text
world_center = global_matrix.transform_point3(collider.offset)
```

A root at the origin with scale `(-2, 1, 1)` and offset `(1, 0, 0)` has world center `(-2, 0, 0)`. Absolute scale alone would put it at `+2` and is wrong. The rigid orientation and the positive extents are a separate extraction: orthogonal columns, a negative determinant folded into a proper rotation, extents from the absolute column lengths. Sphere, box, and capsule are symmetric, so that fold does not change the volume. The sweep starts at `world_center` and writes the entity origin, not the offset. The offset stays on the component.

## Supported transforms

A collider is indexed only when the 3×3 has finite, non-zero, mutually orthogonal columns (normalized dots at most `1e-3`).

Supported: a root; a translated parent; a uniformly scaled and rotated parent; a non-uniform box whose columns stay orthogonal, including a 90° yaw under a non-uniform parent; negative scale once the reflection is folded. Unsupported and skipped: shear (a 45° yaw under a non-uniform parent); a non-uniform sphere or capsule; a non-finite or zero scale; a singular matrix.

`World::despawn` removes the subtree, so a `Parent` does not dangle after a normal despawn. If a `Parent` still names a missing entity, the collider is skipped.

Writeback uses `Mat4::inverse` on the parent matrix, or the identity when there is no parent. A root does `Transform.position += world_delta`. Rotation and scale are not written. `GlobalTransform` is not written.

Parent at `(10, 0, 5)`, yaw `+90°` about Y, uniform scale `2`, child local `(1, 0, 0)`, is world `(10, 0, 3)`. A world delta of `+1` X writes local `(1, 0, 0.5)`, which propagates back to `(11, 0, 3)`. A non-invertible parent drops the move.

## Who may move

`propagate_transforms` is a full pass. There is no subtree propagate. After a mover is written, patching only that mover's index entry leaves a descendant collider at the old pose.

M7 rejects the move when any descendant is currently in the index. The intent is still consumed. The transform is not written, so those entries stay true. A child with a valid parent inverse may itself be the mover. A moving platform that carries collider children is a later change, and it has to refresh every affected entry before the index is called synchronized.

A motor whose own collider is absent from the index (disabled, sheared, or non-uniform sphere or capsule) does not move.

## Fixed-step lifecycle

One `collision_system`, registered after the fixed systems that write `MoveIntent`. It is the only fixed-step caller of `propagate_transforms`. `post_update` still propagates, so animation stays after collision. Interpolation still snapshots locals at the start of each iteration and does not read `CollisionWorld`.

Each of the `0..=8` iterations:

1. Earlier fixed systems write locals and `MoveIntent`.
2. `collision_system` calls `propagate_transforms` once.
3. It rebuilds the sorted list. Queries are valid only after this rebuild.
4. It takes motors in ascending `Entity` order. For each one it locks the intent, sweeps, writes `Transform.position` or rejects the move, patches that one entry, and removes `MoveIntent`.
5. A later mover sees those patches. A rejected ancestor does not change descendant entries.
6. It diffs sensor pairs along the path actually applied, and sends events.
7. When `CollisionDebug` is enabled, it submits `DebugDraw` from the final poses. The resource defaults to off. Indexing and queries run either way, and debug geometry is generated only when the flag is on. `Game::step` clears debug lines once per step, so several enabled iterations append wires. The first checkpoint uses one iteration.

Then the existing promote runs. `update`, `animate`, and `post_update` can read the events. `clear_events` drops them at the end of the step. A fixed system cannot see them.

After the fixed loop, including a loop of zero iterations, every remaining `MoveIntent` is removed. A frame that does not simulate does not keep the intent for a later frame. An intent written in `update` waits until the next step's fixed loop, and a following zero-step frame discards it unused.

There is no second clock. Queries do not propagate and do not rebuild. The snapshot matches the end of the last `collision_system` call. A later write to `Transform`, `Parent`, or `Collider` makes it stale. `ADR-0009` means the resource cannot notice that write. Callers must not treat it as live afterward. `update` may read it only as that last fixed snapshot.

## Filters

Two colliders interact when both are enabled, the entities differ, and both `(layers & other.mask)` tests are non-zero. A free query tests `layers` against the query mask only, excludes an optional entity, and includes sensors unless the caller says otherwise. Sensors never block. Solid/solid pairs emit no events. Several entities may share a layer. `enabled = false` drops the entry on the next rebuild. Removal and despawn are observed on the next pass that rebuilds without them, which is the next iteration, or the next step when the command flush is after this system.

Results and pairs are sorted by `Entity`.

## Triggers

The resource keeps a sorted `Vec<(Entity, Entity)>`, lower id first, of overlaps where at least one side is a sensor. The list is runtime state. A step with no fixed iteration leaves it unchanged. When the rebuilt index has no sensors and the list is empty, that sync does not run the pair pass.

`TriggerEvent` is one typed queue. `Entered { a, b }` and `Exited { a, b }` store the two entities in ascending `Entity` order. A sensor/sensor overlap is one pair. Solid/solid pairs emit nothing. Enter is in the new set only. Exit is in the old set only. Both means no event.

Pass-through follows committed segments. `MoveResult.applied` is the net origin change and is not one of those segments. A committed segment is a displacement the sweep wrote: the piece before an impact, each later slide, a clear of the remaining remainder, and a recovery push when that push is written. A rejected move, a failed parent writeback, a blocking stop, and the tail dropped after `max_slides` are not segments. Each segment is cast from its own start pose with `target_distance` 0. Motor skin does not enlarge the trigger volume. A sensor hit on that path:

* overlaps the final pose and was outside: one `Entered`
* overlaps the final pose and was inside: no event
* outside the final pose and was inside: one `Exited`
* outside both poses and was hit on a committed segment: `Entered` then `Exited`, adjacent

A pair crossed more than once in the same iteration still produces only that outcome. One iteration emits in pair order, and each pair's events finish before the next pair. Later fixed iterations append to the same queue. The queue is promoted once, after the fixed loop.

Motors run in ascending `Entity` order. A segment cast sees earlier movers already patched and later movers at their start pose. Two bodies that both move in one iteration are not one simultaneous sweep. A direct `Transform` write has no segments, so it changes only the endpoint sets. A path with more than `max_slides` segments does not cast the discarded tail. That is the limit.

## Sweep

`remainder` starts as the intent with locked world axes cleared. Rotation stays at the propagated pose.

For each slide while `remainder` is longer than `1e-6` and the count is below `max_slides`:

1. For each solid candidate in the swept bounds, compute the closest-feature gap and wall. The wall is world-space and points from the obstacle toward the mover. Do not rotate it again. A sphere core is its center. A capsule core is its local-Y segment. A cuboid core is the finite box. Cuboid against cuboid uses Parry's SAT contact, and the wall is the negation of world-space `normal1`. A non-finite gap drops the remainder and stops.
2. Skin band: `-1e-4 < gap <= skin + 1e-4`. Do not use the shape-cast time or `normal1`. If `dot(remainder, wall) < -1e-4`, the candidate blocks at time zero. Otherwise it is tangent or separating for this sweep: drop it and keep testing the other candidates over the same `remainder`. `1e-4` is an absolute world-unit tolerance, not a fraction of the request. A missing wall drops the remainder and stops. The cast normal is not a substitute.
3. Deeper than the band (`gap <= -1e-4`) keeps the one recovery push. A time-zero cast whose `contact` distance is negative pushes once by the unlocked part of `-normal1 * (-dist + skin)`. That `normal1` is already world-space. If the unlocked part is shorter than `1e-4` while the locked part was required, apply nothing and log. Do not push a second time. A cast that does not confirm the penetration does not contribute its time of impact. A separating remainder does not block. An approaching remainder blocks at time zero on the feature wall when `dot(remainder, wall) < -1e-4`.
4. Farther than the band (`gap > skin + 1e-4`), cast with `max_time_of_impact = 1`, `target_distance = skin`, `stop_at_penetration = false`, and `compute_impact_geometry_on_penetration = true`. Velocity is `remainder`. A time of impact in `(1e-6, 1]` blocks, and the slide normal is the feature wall at the impact pose. A time-zero cast is ignored because it disagrees with the gap. A missing impact wall drops the remainder and stops.
5. Among blocking hits within `1e-5` of the earliest time, the lowest `Entity` wins. No blocking hit: apply `remainder`, then stop.
6. Otherwise `traveled = remainder * toi` and `unconsumed = remainder * (1 - toi)`. Apply `traveled` only. The full request is never reused.
7. Project only `unconsumed` onto the plane: if `dot(unconsumed, wall) < 0`, subtract that component. Then clear locked axes again. That vector is the next `remainder`. A dropped surface stays in the world and is eligible on the next segment.

After `max_slides`, drop whatever remains. `Err(Unsupported)` or a non-finite position drops the rest and keeps the last finite origin. The applied world delta is written through the parent inverse. Then that entry is patched.

Worked slide. Request `(4, 0, 4)` into a wall whose outward normal is `-X`, hit at `toi = 0.5`. `traveled = (2, 0, 2)`, `unconsumed = (2, 0, 2)`, slide `(0, 0, 2)`, total `(2, 0, 2)`. Length `sqrt(8)`, under the request's `sqrt(32)`. Reusing the full `(4, 0, 4)` after the partial move would add another `(0, 0, 4)` and finish at `(2, 0, 6)`, which is longer than the request. The test forbids that.

A locked axis is cleared on the intent, on the post-projection remainder, and on the recovery push. A Y-locked mover keeps the same world Y within `1e-4`, including under a rotated parent. The constraint is not a second 2D world.

Two movers do not solve together. The lower `Entity` is already patched.

Wall A is already within skin and the remainder points away from it, so A does not block. Wall B stands in that same remainder. The sweep drops A for this segment, keeps casting, and stops at B's near face plus skin. The full remainder past B fails. A is eligible again on the next segment. This case is in the matrix below. M7-1A-R does not implement the slide.

## Broadphase

One sorted `Vec`, rebuilt once per iteration, plus the one-entry patch after a successful move. Linear AABB tests. A full pair pass and each sweep are quadratic in the collider count. That is accepted for a few dozen colliders. It is not a tilemap budget, and this record does not promise M13's frame cost. M13 replaces the private candidate function. `Bvh` is not built in the first checkpoint.

## What this does not decide

Rigid bodies, joints, forces, gravity, autostep, slope limits, jumps, angular sweeps, triangle meshes, convex hulls, and a second collider on one entity. Gameplay facing stays D-48. Root motion stays D-28.

## Consequences and follow-up

M7 implements this contract and adds the dependency then, with the M24 notice. The first checkpoint is the matrix below. `spatial_scene`'s blocked walker stays a later M7 check, after the headless matrix passes.

Reopen only under the rule in `DECISIONS.md`. A dynamics solver, or a switch to Rapier's character controller, is a new ADR.

## Amendments

### 2026-10-27, owner approved (M7-1A-R)

Two corrections. The numbered sections above are the contract after these amendments. The slide was not implemented in M7-1A-R. M7-1B-1 implements it.

Sweep step 6 previously said that a non-negative `dist` at time zero, when `dot(remainder, wall) > 0`, ignores that hit and applies `remainder`. That applied the whole remainder and skipped later candidates. Wall A can be a nonblocking skin contact while wall B still blocks the same remainder. The replacement is the step 6 now in Sweep: exclude that contact for this sweep, keep scanning, take the earliest remaining blocking contact with the same tie break, and apply `remainder` only when nothing else blocks. The surface is eligible again on the next slide segment.

Lifecycle step 7 previously said the system submits `DebugDraw` from the final poses. Submission is opt-in through `CollisionDebug`, default off. Indexing and queries do not read that flag. Wires are generated only when it is enabled, and several enabled iterations append because `Game::step` clears `DebugDraw` once.

Deferred commands queued by a fixed system flush after the rebuild in that iteration. A step with zero fixed iterations does not rebuild and does not flush from the fixed loop. `time` and `loads` still flush before the loop. Those snapshot rules are in `docs/ARCHITECTURE.md`. They are not a change to the sweep.

### 2026-10-27, M7-1B-1 API

`BVec3` is a Dimora type in `engine-math`: public `x`, `y`, and `z` booleans, default all false. `CharacterMotor.locked_axes` uses it. No glam or Parry boolean vector is public.

`MoveResult` is a runtime component, not saved data. `requested` is the intent before locked axes are cleared. `applied` is the change in the entity's world origin, including a recovery push when one was applied. `status` is `Completed`, `Blocked`, or `Rejected`. Blocked and rejected are exclusive. A rejection writes nothing to the transform or the index, reports zero applied displacement, and stores no contacts. `normals` are world-space obstacle normals in slide order, pointing back toward the mover. `recovery_normal` is that same kind of normal when a push was applied. A step that does not process a new intent leaves the previous result alone. A processed zero move replaces it, so an old normal is not a new contact. The result is the last resolution. It does not mean the character is grounded.

`parry3d`'s `alloc` feature is enabled. Ball, cuboid, and capsule sweeps go through the support-map cast, and that code is behind `alloc`. `spade` stays off. The sweep, filter, transform, and fixed-step rules above are unchanged.

### 2026-10-27, owner approved (M7-1B-1G)

Near-tangent shape casts are not a wall normal. Parry's support-map cast replaces its normal from a contact query only when the time of impact is below `1e-4`. Hits just above that cutoff can return a normal aligned with the motion and a tiny positive time. The M7-1B-1 sweep treated that hit as blocking and consumed the move. On this machine, a sphere of radius 0.5 with skin 0.01, already on a box whose Z half-extent is 8000, applied about `0.00049` of a `+Z` request of length 4. The same request along `-Z` applied the full 4. A capsule on that wall left the surface. Those are two faults: the cast normal, and the solver spending the move on it.

The replacement is the sweep above. The closest-feature wall is selected from the primitive cores, in world space, after collider offset and signed scale have already been folded into the indexed pose. Faces, edges, and corners are the closest point on that core. The pair is evaluated in either order, and the wall is then oriented from the obstacle toward the mover. Shape-cast `normal1` stays mover-local. Contact `normal1` stays world-space. A world-space normal is not rotated again.

Inside the skin band, tangent and separating motion drop that candidate only. Other candidates are still tested over the same remainder, so a near wall does not hide a blocking wall ahead. Inward motion at or above the `1e-4` threshold blocks at time zero and the unconsumed remainder is projected. A true approach from outside the band still uses the cast time and the existing earliest-`Entity` order. The one-push recovery, locked world axes, parent writeback, index patch, and `MoveResult` fields are unchanged.

If the cores meet and no face, edge, or corner direction is usable, or the gap is not finite, the remainder is dropped and the move stops at the last finite position. That path does not fall back to the cast normal, does not tunnel, and does not write a non-finite position. Triangle meshes and convex hulls are outside this checkpoint. Cuboid-cuboid still depends on Parry's SAT contact. The `1e-4` band is a numerical compromise at this scale: a true approach smaller than that can be absorbed, and a wrong cast time while the gap is outside the band is not repaired.

The overlap fixture in the movement tests is one case. The sphere center starts at `x = 0.3` against the face `x = 0`, radius `0.5`, skin `0.01`. The overlap is `0.8`. The push is `0.81` along `-X`, so the applied displacement is `-0.81` and the final center is `-0.51`.

### 2026-10-28, owner approved (M7-1B-2)

Trigger delivery uses one `TriggerEvent` queue, as written in Triggers above. `Entered` and `Exited` are variants, not two event types, so a pass-through stays adjacent across fixed iterations. A repeated crossing of the same pair in one iteration collapses to that section's single outcome. The recovery push is a committed segment when it is written. An uncommitted remainder is not tested. `MoveResult.applied` stays the net origin change.

This amendment does not change the sweep. Sensor obstacles are still skipped. The sweep does not read the mover's own sensor flag, so a motor whose collider is a sensor is still stopped by solids. That is the obstacle rule in Filters ("sensors never block"), not a ghost mover. Changing it would be a motor change and is outside M7-1B-2.

## Evidence and revisit conditions

Probe, not in the repo: `/tmp/dimora-parry-probe`, `parry3d` 0.31.1, 2026-10-27, America/Chicago. `cargo +1.99.0 run` and `cargo +1.90.0 run` (SDKROOT `MacOSX26.5.sdk`) both exited 0. Asserted overlap, a penetrating `contact` with world `normal1 = +X`, a ray time of impact `4` and a world outward normal, an identity sweep time of impact `3` with local `normal1 = +X`, and the yawed sweep whose world wall normal is `+Z`.

## First checkpoint matrix

Headless, one fixed step of `1/60` s, no window. Positions include both signs. Tolerances: positions `1e-4`, time-of-impact ties `1e-5`, boolean overlap when the gap exceeds `1e-3`. The 1,000-step hash is `to_bits` in one process.

| Case | Expectation |
|---|---|
| Sphere, box, capsule pairs, including a yawed box | Boolean overlap |
| Ray per shape | Distance and world normal |
| Equal X/Y, Z gap larger than the extents | No overlap |
| Scale `(-2, 1, 1)`, offset `(1, 0, 0)` | World center `x = -2` |
| Translated parent and the yaw/scale parent above | Local position matches the writeback |
| 45° child under non-uniform scale | Absent from the index |
| Non-uniform sphere | Absent from the index |
| Displacement longer than a thin wall | Stops at the near face plus skin |
| Diagonal `(4, 0, 4)` into an X wall | Path length ≤ request; no extra Z |
| Tangent length 4 on a flat wall, skin `0` and `0.01`, including Z half-extent `8000` | Advances about 4. Lateral drift within `1e-4`. The normal stays a face normal |
| Twenty `(4, 0, 4)` steps from contact | Tangent sum only. No accumulated penetration and no amplification |
| Into `0.001` and `0.05`, and an outward request | The into component is removed. The outward request is applied |
| Forty inward steps of `5e-5` | Penetration stays inside `2e-4` |
| A skin-band wall beside the mover and another wall ahead | Stops at the wall ahead. No lateral drift |
| Sphere or capsule into a corner or a vertical edge | Diagonal feature normal |
| Two walls | Crease or stop, within `1e-4` |
| Skin contact with wall A, remainder toward wall B | Stops at B's near face plus skin; does not tunnel. A is eligible on the next segment |
| Spawned overlapping a solid | At most one recovery push |
| `locked_axes.y` | World Y unchanged within `1e-4` |
| Sensor still overlapped | One enter, no duplicate on an idle step |
| Sensor crossed by an applied segment | Enter then exit, adjacent |
| Sensor behind a solid that stopped the move | No pass-through event |
| Mask excludes an obstacle | Absent; same-layer peers sorted by `Entity` |
| `enabled = false`, and a despawn | Exit on the next observing pass |
| Ancestor motor with an indexed descendant | Transform unchanged; descendant pose unchanged |
| Two movers | Lower `Entity` is patched first |
| Two cameras, unread | Same hash |
| 1,000 steps | Same hash |
| `CollisionDebug` off | No collision wires. Indexing and queries still run |
| `CollisionDebug` on | Finite segments for each indexed collider. Several fixed iterations append |

M7-1A writes the geometry, filter, ordering, camera, and debug rows. M7-1B-1 writes the movement rows, including the thin wall, the diagonal slide, wall A then wall B, one recovery push, locked axes, and a same-process 1,000-step `to_bits` hash of a sphere stopped against another sphere. M7-1B-1G writes the tangent, repeated-step, tiny-into, and corner rows. M7-1B-2 writes the sensor rows: one enter, no duplicate while the overlap continues, enter then exit for a committed crossing, and no event for a sensor past a solid that stopped the move. M7-1B-1 measured a tilted cast normal on repeated grazes along a large box. The skin-band amendment corrects that classification. The 1,000-step hash is still the sphere-sphere stop, not that graze.
