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
* **Variable-step writes.** The marker means the local is written in `fixed_update`. Writing it again in `update` makes the blend include that write. That is not supported.
* **Rotation at 180 degrees.** `Quat::slerp` takes the shortest arc and is unstable exactly opposite. No special case.
* **Scale that changes sign.** Lerp can pass through zero. No special case.
* **Shear.** `Transform` is translation, rotation and scale. The blend is of those three, not of a general matrix.

Parent and child: a marked parent contributes an interpolated local, and an unmarked child contributes
its current local, so the child follows the smoothed parent. A marked child interpolates its own
local on top of whatever its ancestors contributed. Neither case writes the simulation transforms.

Entities without the marker, which is every entity in the current examples, extract exactly as they
do at `6dbabb8`.

## Consequences and follow-up

* M3a-2 implements the stages and ports the examples without moving their systems out of `update`.
* M3a-5 implements `alpha`, the marker, the previous local, and the extract-time blend.
* M3b, collision, and animation events attach to these stages. They do not add a scheduler.
* This decision does not add pause, time scale, or a fixed-step elapsed clock.

## Evidence and revisit conditions

`Schedule` and `Game::step` are the code this replaces. `Quat::slerp` already takes the shortest arc
(`crates/engine-math/src/quat.rs`). Reopen the stage list if a real system cannot be expressed as
`before` / `after` inside one of these stages. Reopen interpolation if a hierarchy case above is
wrong in a test, not because a smoother blend would look better.
