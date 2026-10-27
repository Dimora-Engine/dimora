# ADR-0007: Named stages on the existing `Schedule`

* **Status:** Settled
* **Date:** 2026-10-14
* **Backlog id:** D-01
* **Decided by:** project owner (approved the proposed direction)
* **Affects:** `Game::step`, `Schedule`, `Time`, extraction, fixed-step gameplay (M7), animation (M8, M11)

## Context and problem

`Schedule` is a list of `FnMut(&mut World)` run in insertion order. `Game::step` hard-wires asset
loads, the fixed schedule, the update schedule, `animate_system` and `propagate_transforms` (KI-10).
User systems cannot sit next to those stages. `FixedTimestep::alpha` exists and nothing reads it
(R-3). Both examples register only variable-step systems and read `Time::delta_secs` or
`elapsed_secs`.

The rendering regression baseline for this work is commit `6dbabb8` (the M6 `spatial_scene`).
Headless screenshots of `basic_2_5d` and of `spatial_scene` perspective and orthographic must stay
byte-identical to that commit unless a later change is meant to change the picture.

## Requirements and constraints

* One `World`, one spatial hierarchy, the same `Transform` gameplay already writes.
* Simulation, including animation evaluation, stays testable without a GPU or a window (ADR-0006).
* `engine-world` does not depend on the renderer. Extract and render cannot be `World` systems.
* A cycle in system order is an error, not a hang.
* Fixed-step code and variable-step code must be able to tell which delta they are seeing.

## Options considered

### A. Named stages on the current `Schedule`, with `before` / `after` by system name
* Advantages: the examples move unchanged into the `update` stage; no new ECS.
* Disadvantages: order is declared by hand.

### B. A dependency-graph scheduler from declared access
* Advantages: order falls out of the data.
* Disadvantages: needs access data the `RefCell` resources do not have (ADR-0006).

### C. Keep `Game::step` hard-wired and add a few hooks
* Advantages: smallest diff.
* Disadvantages: every new engine stage edits `step` again (KI-10 stays).

## Decision and rationale

A. `Game::step` runs a fixed list of named stages. A stage is a `Schedule`. Inside a stage,
`before` / `after` constraints order named systems. Systems with no constraint keep insertion order.
Building a cycle returns an error and does not run the frame.

World stages, in order:

| Stage | Who runs | When |
|---|---|---|
| `time` | Advance `Time` from the `dt` argument; accumulate the fixed step | Once per `step` |
| `loads` | `AssetManager::process_loads` | Once |
| `fixed_update` | User fixed systems | 0 to 8 times (`FixedTimestep`, 60 Hz unless `GameConfig::fixed_hz` says otherwise) |
| `update` | User variable systems. Both examples stay here | Once |
| `animate` | `animate_system`, once, using the frame delta | Once |
| `post_update` | `propagate_transforms` | Once |
| `input_clear` | `Input::end_frame` | Once |

The runner still applies input events before `step`, then calls `step`, then extract, then render.
Extract and render are frame phases in the `engine` crate. They are not stages in `engine-world`.

### Lifecycle anchors

Ordering inside a stage stays configurable, except four built-ins that are the ends of their stages.
A constraint that would put a system on the closed side is rejected and the schedule is left unchanged.

| Built-in | Stays |
|---|---|
| `advance_time` | First in `time`, so every system in that stage sees the frame that `step` just started |
| `process_loads` | First in `loads`, so a later system in that stage sees assets processed for this step |
| `propagate_transforms` | Last in `post_update`, so it reads transforms written by the systems before it |
| `end_input_frame` | Last in `input_clear`, so systems in that stage still see this step's input edges |

`animate` is first only for a system with no explicit constraint. An explicit `before` or `after` may
place a system on either side, so one system can write a pose before `animate_system` and another
can read the result after it.

`fixed_update` and `update` have no built-in. `Game::step` restores `Time::delta` to the frame delta
after the fixed loop and before `update`. That restore is not a system, so `before` / `after` cannot
move it.

### Fixed-step delta

During each `fixed_update` iteration, `Time::delta` (and `delta_secs`) is the fixed step. After the
iteration it is restored to the frame delta, so `update` and `animate` still see the frame. `Time::elapsed`
and `Time::frame` are frame clocks: `elapsed` is the sum of frame `dt`s, and `frame` increments once
per `step`, not once per fixed iteration. A fixed system that wants the tick length reads `delta`.
It does not get a second elapsed clock.

A stall still runs at most 8 steps and then drops the leftover, as `FixedTimestep` does today. After
that drop, `alpha` is 0.

### Interpolation

Interpolation is opt-in and happens at extraction. It never writes `Transform` or `GlobalTransform`.

An entity opts in with a marker. Extraction builds a render-local transform for every entity: if the
marker has a stored previous local, that is `slerp` of rotation (`Quat::slerp`, shortest arc) and
lerp of translation and scale, by `alpha`; otherwise the render-local is the current local.
The render world matrix is the product of those render-locals up the parent chain. It is not stored
on the entity. `propagate_transforms` is unchanged, so anything that reads `Transform` or
`GlobalTransform` sees the simulation.

Previous local is copied from current at the start of each fixed iteration, for marked entities
only, before those systems run. `alpha` then blends from that previous local toward the current
local. Several fixed iterations in one frame leave `previous` as the local from before the last
iteration.

Unsupported, on purpose, in this first version:

* **No history yet.** Until a fixed iteration has stored a previous local, extraction uses the current local. A newly created entity is not interpolated.
* **Teleport.** A discontinuous move will slide across the gap. The supported teleport is to set the stored previous local equal to the current local at that moment. There is no distance threshold.
* **Destruction.** A despawned entity is gone. There is nothing to interpolate.
* **Variable-step writes.** The marker means the local is written in `fixed_update`. Writing it again in `update` makes the blend include that write. Fixed-update and variable-update systems still read `Transform`, the simulation value. Arbitrary mixing of fixed-step and variable-step motion on one marked entity is not supported, and there is no second blend mode for it.
* **Rotation at 180 degrees.** `Quat::slerp` takes the shortest arc and is unstable exactly opposite. No special case.
* **Scale that changes sign.** Lerp can pass through zero. No special case.
* **Shear.** `Transform` is translation, rotation and scale. The blend is of those three, not of a general matrix.
* **Hierarchy discontinuities.** History is a local transform, and previous and current must stay in the same space. `detach_children` rewrites each direct child's local into world space and sets that child's stored previous local equal to the rewritten local, so extraction does not blend parent-space numbers with world-space numbers. A transform-less grouping child that gains a world `Transform` is stored the same way. Descendants keep their own locals and history. `set_parent` does not rewrite the local, so previous and current stay parent-local offsets: the entity pops onto the new parent, and extraction is `new parent * lerp(previous local, current local)`, not a lerp of the old world pose and the new one. Reparenting an ancestor leaves the marked entity's own local space unchanged; it pops with that ancestor and keeps its local blend. Removing `Parent` leaves the local unchanged, so both samples stay the old numbers and are both read as root-local. Despawning a parent removes the subtree, so there is no history left to blend. `Interpolate::snap` remains the explicit teleport. There is no hierarchy-animation system.
* **Non-finite values.** A non-finite previous local, current local, alpha, or blend result is not blended. Extraction uses the current local. Simulation values are left as they are.

Parent and child: a marked parent contributes an interpolated local, and an unmarked child contributes
its current local, so the child follows the smoothed parent. A marked child interpolates its own
local on top of whatever its ancestors contributed. The render matrix is that product. It is not a
lerp of two world matrices. Neither case writes the simulation transforms. A transform-less node
stays identity, as in `propagate_transforms`.

`FixedTimestep::alpha` is the accumulator divided by the step, clamped to a finite value in
`0.0..=1.0`. A zero-length step, a non-finite ratio, and the leftover dropped by the eight-step
stall are `0.0`. After a normal step the accumulator is below one step, so alpha is in `[0, 1)`.

The copy into the previous local is not a system. `Game::step` does it at the start of each fixed
iteration, before user fixed systems, so `before` / `after` cannot move it. A command flushed at
the end of that iteration is absent from the snapshot until a later iteration.

If no entity has a stored previous local, extraction reads `GlobalTransform` and does not build
another set of matrices. That matches blending every current local, once `propagate_transforms`
has run. Entities without the marker, which is every entity in the current examples, therefore
extract exactly as they do at `6dbabb8`.

Sprites, meshes, tilemaps, lights, and the active camera consume one render matrix per entity.
Billboard, upright, and fixed sprites, and perspective, orthographic, and isometric camera
placements, all use that matrix. A camera is interpolated only when it or an ancestor has a stored
previous local. An unmarked camera matches `GlobalTransform`. There is no separate camera-smoothing
path, and the examples do not mark the camera. Depth sorting and depth testing are unchanged; they
use the position in that matrix.

## Consequences and follow-up

* M3a-2 implements the stages and ports the examples without moving their systems out of `update`.
* M3a-5 implements `alpha`, the `Interpolate` marker, the previous local, and the extract-time blend. It is committed locally and is not pushed. Skeletal interpolation, physics, and a general animation history are not part of it.
* M3b keeps these frame edges for raw `Input` and variable-step systems. A separate fixed-step latch, specified in ADR-0013, delivers a one-shot once to the first fixed iteration and is not part of this stage list. Collision and animation events attach to these stages. They do not add a scheduler.
* This decision does not add pause, time scale, or a fixed-step elapsed clock.
* Follow-up, 2026-10-27. M7-1A adds `sync_collision` as the sealed last system of `fixed_update` (ADR-0025). Other fixed systems run before it. `update` still has no built-in. The decision section's sentence that `fixed_update` has no built-in described the schedule before that system existed.

## Evidence and revisit conditions

`Schedule` and `Game::step` are the code this replaces. `Quat::slerp` already takes the shortest arc
(`crates/engine-math/src/quat.rs`). Reopen the stage list if a real system cannot be expressed as
`before` / `after` inside one of these stages. Reopen interpolation if a hierarchy case above is
wrong in a test, not because a smoother blend would look better.
