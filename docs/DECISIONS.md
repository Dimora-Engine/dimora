# Architecture decision backlog

This is the list of architectural decisions that are **not yet made**, when each is needed, and what
waits on it. It is not a design for the answers. Where a default is proposed it is a starting point
for the decision, not a commitment.

Settled decisions live in [ARCHITECTURE.md](ARCHITECTURE.md#decisions-already-made). They are not
reopened unless the current implementation exposes a concrete problem; the rule for reopening is at
the bottom of this file.

Milestone ids (`M2`, ...) refer to [../ROADMAP.md](../ROADMAP.md). Known-issue ids (`KI-n`) refer to
[../TASKS.md](../TASKS.md).

## How to use this file

* Decide **at the latest** by the "Needed by" milestone, and before the first task in that milestone
  that depends on the answer.
* A decision is closed by moving it to the ARCHITECTURE.md table with the chosen option and the
  trade-off accepted, and marking it `Decided` here with the commit.
* Prefer the option that keeps the next milestone small. Do not decide earlier than needed: an early
  answer made without a working feature to test it against is the likeliest to be wrong.

Status values: **Open**, **Proposed** (a default exists, awaiting acceptance), **Decided**.

## Summary

| Id | Decision | Needed by | Blocks | Status |
|---|---|---|---|---|
| D-03 | Despawn semantics for entities with children | M2 (T3) | M2 hierarchy fix, M4 | Proposed |
| D-04 | GPU cache lifetime | M2 (T2) | M2 eviction, M13 unload | Proposed |
| D-22 | Code host and CI provider | M2 | CI config, release | Open (credentials) |
| D-01 | Scheduling model | M3 | M3, M7, M12 | Open |
| D-02 | Change detection | M3 | M3, M4 (dirty transforms), M18 | Open |
| D-10 | Events and deferred commands | M3 | M3, M4, M7 | Open |
| D-27 | Units and pixels per unit | M5 | M5, M8, M10 | Open |
| D-05 | Materials, alpha modes, blended meshes | M5 | M5, M11 | Open |
| D-06 | Pixel-art policy (snapping, MSAA) | M6 | M6, M8, M10 | Open |
| D-07 | Depth convention (reverse-Z) | M6 | M6, M11 shadows | Open |
| D-08 | Multi-camera and render targets | M6 | M6, M11, M17 | Open |
| D-25 | Custom projections inside `Camera` | M6 | M6, M10 (oblique tiles) | Open |
| D-26 | Additional sprite orientations | M6 | none hard | Open, low priority |
| D-09 | Physics: custom or library | M7 | M7, M8 | Open |
| D-11 | Audio | M8 | M8 | Open |
| D-24 | UI strategy | M8 | M17 | Open |
| D-12 | Skeletal approach and formats | M9 | M9, M13 importers | Open |
| D-13 | Sprite lighting, normal maps, light culling | M11 | M11 | Open |
| D-14 | Shadow technique | M11 | M11 | Open |
| D-15 | Particles: CPU or GPU, sorting | M12 | M12 | Open |
| D-16 | Asset identity and dependencies | M13 | M13, M14, M15 | Open |
| D-17 | Importer and dependency policy | M13 | M13 | Open |
| D-18 | Scene format, serde, component registry | M4 (registry shape), M14 | M4, M14, M17 | Open |
| D-19 | Bundle format | M15 | M15, M19 | Open |
| D-20 | Editor technology | M17 | M17 | Open |
| D-21 | Logging and profiling crates | M16 | M16, M18 | Open |
| D-23 | Platform targets, MSRV policy, crates.io | M2 (MSRV), M19 | M19 | Open |

## Needed during M2 (hardening)

### D-03 Despawn semantics for entities with children

* **Problem.** `World::despawn` on a parent leaves the children alive with a `Parent` pointing at a dead
  entity (KI-8). `propagate_transforms` then treats them inconsistently.
* **Options.** (a) Recursive despawn of the subtree. (b) Detach: children survive and become roots,
  keeping their world transform. (c) Make the caller choose (`despawn` vs `despawn_recursive`).
* **Proposed default.** (a) for `despawn`, because bones and attachments are owned by their parent
  and orphaned bones are never wanted; offer (b) as an explicit `detach_children` call.
* **Needed by.** M2 task T3.
* **Blocks.** The hierarchy fix; scene unload in M4 depends on the answer.

### D-04 GPU cache lifetime

* **Problem.** The wgpu caches keyed by `AssetId` are never evicted (KI-4), and `AssetManager` has no
  `remove`.
* **Options.** (a) Evict when the asset is removed from the manager. (b) Also evict entries unused for
  N frames. (c) Explicit `release` only.
* **Proposed default.** (a) plus (b) with a small N, so a removed asset is freed promptly and an asset
  that is merely off-screen is not re-uploaded every frame.
* **Needed by.** M2 task T2.
* **Blocks.** Eviction; asset unload for scenes (M4) and hot reload (M13).

### D-22 Code host and CI provider

* **Problem.** The repository lives on Origin only. A GitHub destination was requested but no
  credentials are available in this environment (B-1), so no CI provider is chosen.
* **Options.** Origin's own checks if available; GitHub Actions once a token or repo exists; a plain
  `scripts/check.sh` run by hand until then.
* **Needed by.** M2, to run `scripts/check.sh` automatically.
* **Blocks.** CI configuration only. Everything else in M2 proceeds with the script alone.

### D-23 Platform targets, MSRV policy, crates.io

* **Problem.** There is no MSRV or toolchain pin (KI-1). The workspace needs a recent compiler and
  fails on 1.83 because a dependency uses edition 2024.
* **Options.** MSRV tracking latest stable; MSRV pinned to the lowest version that builds and bumped
  deliberately; plus a decision on desktop-only for now vs web/mobile after M19, and whether to publish
  to crates.io.
* **Proposed default.** Pin `rust-toolchain.toml` to a stable release and record the verified MSRV in
  `Cargo.toml` (`rust-version`). Desktop only until after M19. Publishing deferred to M19.
* **Needed by.** MSRV part by M2 (T1). Platform and publishing parts by M19.

## Needed during M3-M4 (input, scheduling, scenes)

### D-01 Scheduling model

* **Problem.** The `Schedule` is a sequential list of `FnMut(&mut World)` and the engine stage order is
  hard-wired in `Game::step` (KI-10).
* **Options.** (a) Named stages with user systems attached per stage and `before`/`after` by name.
  (b) A dependency-graph scheduler with automatic ordering from declared access. (c) Keep sequential
  and only expose stage hooks.
* **Leaning.** (a). (b) needs access declarations that the `RefCell` resources cannot provide yet, and
  there is no parallelism requirement.
* **Needed by.** M3. **Blocks.** M3, and M7/M12 which add stages.

### D-02 Change detection

* **Problem.** `propagate_transforms` visits every entity every frame; nothing can ask "what changed".
* **Options.** (a) None; keep full passes until profiling (M18) shows a cost. (b) A dirty flag on
  `Transform` set through a mutation API. (c) Switch ECS or add a tick-based layer.
* **Leaning.** (a) unless M3 or M4 needs change data for a feature. Profile first (KI-19).
* **Needed by.** M3. **Blocks.** Dirty-transform propagation in M4, any M18 optimisation.

### D-10 Events and deferred commands

* **Problem.** Systems mutate the world directly, resources are `RefCell` so a double borrow panics at
  run time (KI-9), and there is no way to queue a spawn or despawn during iteration or to publish an
  event (collision, scene loaded, asset failed).
* **Options.** (a) Command buffer applied at stage boundaries plus typed event queues cleared per
  frame. (b) Immediate observers/callbacks. (c) Defer until a feature forces it.
* **Leaning.** (a); collision (M7) and scene loading (M4) both need it, so (c) only delays the need.
* **Needed by.** M3. **Blocks.** M3, M4, M7.

### D-18 Scene format, serde, component registry

* **Problem.** There is no way to enumerate or name components, so nothing can be serialized.
* **Options.** Format: RON, JSON, or a custom text format. Registry: explicit registration of each
  component type with a stable name and (de)serialize functions, versus derive-based reflection.
  Whether to take a `serde` dependency (a deliberate one, not incidental).
* **Needed by.** The registry's *shape* by M4 (so scene membership does not need rework); the format
  and `serde` choice by M14. **Blocks.** M4, M14, M17.

## Needed during M5-M6 (materials, renderer)

### D-27 Units and pixels per unit

* **Problem.** World units are unitless. Sprites map texture pixels to world units through the
  sprite's size, with no engine-wide convention.
* **Options.** A global pixels-per-unit constant; a per-asset or per-sprite value; keep it
  per-sprite and document it.
* **Needed by.** M5 (texture import), at the latest M8. **Blocks.** Consistent sizing for the example
  game and tilemaps (M10).

### D-05 Materials, alpha modes, blended meshes

* **Problem.** `Material` is a flat colour plus optional texture. Alpha handling for meshes is limited
  and blended meshes are not sorted with blended sprites (KI-6, KI-14).
* **Options.** Explicit alpha modes (opaque, cutout, blended); whether blended meshes join the sprite
  sort or get their own pass; the material extension path for normal maps (D-13).
* **Needed by.** M5. **Blocks.** M5, M11.

### D-06 Pixel-art policy

* **Problem.** There is no policy for texel snapping, integer scaling or MSAA, and the nearest-neighbour
  look at fractional camera positions can shimmer.
* **Options.** Opt-in snapping per camera; integer-scale viewport mode; MSAA off for pixel-art cameras.
* **Needed by.** M6. **Blocks.** M8 visual quality, M10 tile seams.

### D-07 Depth convention (reverse-Z)

* **Problem.** Depth is standard 0..1 with `Depth32Float`. Large perspective ranges lose precision
  far away. Shadow maps (M11) will want a consistent choice.
* **Options.** Keep standard Z; adopt reverse-Z.
* **Leaning.** Keep standard Z unless a concrete precision artefact is shown in M6 testing.
* **Needed by.** M6. **Blocks.** M11.

### D-08 Multi-camera and render targets

* **Problem.** Only one camera is rendered per frame (KI-5), and there is no render-to-texture.
* **Options.** Several cameras drawn in `(order, id)` order into the surface; camera-to-texture
  targets; both.
* **Needed by.** M6. **Blocks.** Shadow passes (M11), editor viewports (M17), minimap-style effects.

### D-25 Custom projections inside `Camera`

* **Problem.** The `Projection` trait exists but `Camera` stores a closed enum (KI-11), so an oblique
  or isometric projection cannot be supplied.
* **Options.** Add enum variants (oblique); store a boxed trait object; store a plain matrix override.
* **Needed by.** M6; earlier only if tilemaps (M10) need oblique projection.
* **Blocks.** Custom projections; M10 if it chooses oblique tiles.

### D-26 Additional sprite orientations

* **Problem.** `Billboard`, `Upright`, `Fixed` cover current needs. Axis-aligned billboards or
  camera-position-facing billboards may be wanted.
* **Needed by.** Nothing is blocked. Decide only when a real scene needs it.

## Needed during M7-M8 (gameplay, vertical slice)

### D-09 Physics: custom or library

* **Options.** A small custom AABB/shape collision layer with queries and triggers; or a library
  (rapier). A library brings a dependency and its own world, which must be reconciled with the
  `Transform` hierarchy and the fixed step.
* **Leaning.** Custom, minimal, because the first game needs overlap tests, triggers and simple
  resolution, not a full solver. Revisit if the game needs more.
* **Needed by.** M7. **Blocks.** M7, M8.

### D-11 Audio

* **Options.** Out of scope; a thin wrapper over a library (`rodio`/`kira`); or a custom backend.
* **Needed by.** M8, only if the vertical slice includes sound. **Blocks.** Nothing else.

### D-24 UI strategy

* **Options.** Sprite-drawn text only; an immediate-mode library; a custom retained UI.
* **Needed by.** M8 chooses "sprite text only" unless it needs more; a real decision before M17.
* **Blocks.** Editor technology (D-20).

## Needed during M9-M12 (animation, lighting, particles)

### D-12 Skeletal approach and formats

* **Options.** Sprite-part skeletons only (current data model); mesh deformation and skinning; both.
  Clip source format: custom, glTF, or a 2D tool format.
* **Needed by.** M9. **Blocks.** M9, M13 importers.

### D-13 Sprite lighting, normal maps, light culling

* **Problem.** Forward shading handles at most 16 lights and sprites use wrap lighting; there are no
  normal maps (KI-6).
* **Options.** Add a normal-map slot to `Material`; per-tile or clustered light culling; keep forward
  with a hard light cap.
* **Needed by.** M11 (the normal-map slot is prepared in M5). **Blocks.** M11.

### D-14 Shadow technique

* **Options.** Shadow maps (directional first); blob shadows for sprites; projected planar shadows.
  Interaction with blended sprites and the depth convention (D-07) must be settled.
* **Needed by.** M11. **Blocks.** M11.

### D-15 Particles

* **Options.** CPU simulation with instanced draw; GPU simulation. Sorting policy against blended
  sprites (global sort vs per-system).
* **Leaning.** CPU and instanced, since it reuses the sprite pipeline.
* **Needed by.** M12.

## Needed during M13-M15 (assets, serialization, bundles)

### D-16 Asset identity and dependencies

* **Problem.** `AssetId` is an FNV-1a hash of the name; collisions are not detected (KI-12) and
  there is no dependency graph for cascading reload.
* **Options.** Keep name hashes with a collision guard (M2 adds the guard); stable GUIDs with a
  name index; path-derived ids.
* **Needed by.** M13, though the collision guard lands in M2 (T3) regardless.
* **Blocks.** M13, M14 (what scenes reference), M15.

### D-17 Importer and dependency policy

* **Options.** Which formats and which crates (PNG exists; glTF, audio, fonts, atlases). A rule for
  accepting a new dependency: it must unblock an acceptance criterion in the current milestone.
* **Needed by.** M13.

### D-19 Bundle format

* **Options.** A custom archive with an index and content hashes, optional compression; versus an
  existing container format.
* **Needed by.** M15. **Blocks.** M15, M19.

## Needed later

### D-20 Editor technology

* **Options.** An immediate-mode library inside a dedicated crate; a separate application; a web UI.
* **Needed by.** M17, after D-24 and D-18. **Blocks.** M17 only.

### D-21 Logging and profiling crates

* **Options.** `log` plus a backend, or `tracing`; a profiling scope macro with a feature flag.
* **Needed by.** M16. **Blocks.** M16 and M18 measurement.

## Reopening a settled decision

A decision in ARCHITECTURE.md may be reopened only when all of the following hold:

1. A specific failing test, measurement or concrete limitation from the repository is cited.
2. The cost of working around it is stated.
3. The proposed change names what it replaces and which tests must still pass.

"It would be cleaner" or "another engine does it differently" is not sufficient.
