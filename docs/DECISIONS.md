# Architecture decision backlog and records

This file is the **backlog**: decisions that are open, proposed or recently made, when each is needed and
what waits on it. It is not a design for the answers. Where a default is proposed it is a starting point
for the decision, not a commitment. Each *made* decision is recorded as a short ADR in [adr/](adr/) (format
below); settled decisions that describe current code also appear in
[ARCHITECTURE.md](ARCHITECTURE.md#decisions-already-made). They are not reopened unless the current
implementation exposes a concrete problem; the rule for reopening is at the bottom of this file.

Milestone ids (`M2`, ...) refer to [../ROADMAP.md](../ROADMAP.md). Known-issue ids (`KI-n`) refer to
[../TASKS.md](../TASKS.md). Risk ids (`R-n`) refer to the register in
[ARCHITECTURE.md](ARCHITECTURE.md#cross-system-requirements-and-risks). The product principles that
decisions are weighed against are in [VISION.md](VISION.md).

## How to use this file

* Decide **at the latest** by the "Needed by" milestone, and before the first task in that milestone
  that depends on the answer.
* Prefer the option that keeps the next milestone small. Do not decide earlier than needed: an early
  answer made without a working feature to test it against is the likeliest to be wrong. The exception
  is anything a **data format, public API or cross-system contract** will bake in: those are decided
  before the first consumer is written, with a prototype if the answer is uncertain.
* A backlog entry stays short: problem, options, leaning, needed-by, blocks. When a decision is made,
  **write an ADR** (`docs/adr/NNNN-slug.md`, from [adr/TEMPLATE.md](adr/TEMPLATE.md)), replace the entry's
  body with a link, mark it `Decided` here, and update ARCHITECTURE.md if it changes how the code is
  described. Do not decide in a commit message or a chat.
* Status values: **Open**, **Proposed** (a default exists, awaiting acceptance), **Provisional** (accepted
  so work can proceed; the ADR names the evidence that triggers a revisit), **Decided** (settled).

### Decision record format (ADR)

One file per decision, numbered in order of acceptance, kept short (a page, not an essay). Required
sections, in this order: context and problem; requirements and constraints; options considered, each with
advantages and disadvantages; decision and rationale; consequences and follow-up; evidence and revisit
conditions. Header fields: status (Proposed, Provisional, Settled or Superseded), date, backlog id,
who decided, what it affects. A Provisional ADR must state its revisit condition. A superseded ADR is
never edited or deleted: the new ADR links back and the old one's status changes. Decisions already
embodied in code before this process (the table in ARCHITECTURE.md: `hecs`, `wgpu`, `winit`, handedness,
and so on) are not back-filled; they get an ADR only if reopened. Write an ADR for anything that
affects several systems, a public API, a file format, performance budgets or compatibility; small local
choices belong in code comments or the PR.

### Owner decisions

The owner's own decisions and open questions, kept apart from engineering recommendations so the two are
never confused. **Confirmed** means the owner stated it. **Recommended** means an engineer proposed it and it
is not approved; nothing marked Recommended may be described elsewhere as decided.

| Id | Topic | Status | Where recorded |
|---|---|---|---|
| OD-1 | Editor timing: the full editor stays late, behind a stability gate; developer tooling arrives incrementally; the runtime stays free of editor-specific state | **Confirmed** (2026-10-13) | [ADR-0003](adr/0003-incremental-tooling-editor-later.md), ROADMAP "Incremental developer tooling" |
| OD-2 | Dimora is not defined by HD-2D or any single view; one spatial world supports orthographic, perspective, isometric, top-down, side-scrolling and custom cameras as intended capabilities; projection is separate from gameplay orientation and style; the showcase is a reusable multi-camera test scene | **Confirmed** as direction (2026-10-13). The six open questions were answered the same day; the answers are classified (accepted principle, provisional convention, deferred API decision, needs prototype evidence) in ADR-0004 "Resolution of the open questions". Mechanisms, owner response 2026-10-13: camera presets as constructors **Accepted**; style as policy **Accepted with clarification** (per level, no universal style abstraction); gameplay orientation as data **Accepted**; coplanar ordering **Amended** (spatial depth, transparency sorting, render layers or queues and explicit render order stay distinct; no generalised occlusion framework yet). The test scene now exists in `spatial_scene` (example-local constructors, verified 2026-10-14). An engine-level preset API, style-policy APIs, gameplay-orientation components and a coplanar mechanism are still not implemented | [ADR-0004](adr/0004-one-spatial-world-many-camera-configurations.md), D-48 |
| OD-3 | Licence: `MIT OR Apache-2.0` | **Proposed, not confirmed.** No licence files are added until the owner approves. Copyright holder also unconfirmed | TASKS.md "Licence audit" (KI-17) |
| OD-4 | Use GitHub Actions if the repository is hosted on GitHub; validate on Windows, macOS and Linux; Windows early | **Confirmed** as direction (2026-10-13). That GitHub is the long-term host is **assumed** (the repository and its pull requests are on GitHub); the owner has not stated it | [ADR-0005](adr/0005-ci-on-github-actions.md) (Provisional), D-22 |
| OD-5 | Does a licence also need a contributor policy (a CLA or DCO), given contributions from several authors and AI agents? | **Open** | TASKS.md "Licence audit" |
| OD-6 | Is the product audience "programmers who also want visual tools" or "creators who may not write Rust"? How much of a game must be buildable without Rust? | **Open** (direction: both, editor is a goal) | VISION.md open questions |

### What to resolve first

Ordered by how many systems, APIs and formats a decision touches and how costly a late change is (see
the risk register). **Prototype where the answer depends on a measurement; do not guess.**

| Tier | Decisions | Why now | How to resolve |
|---|---|---|---|
| **1. Before M3 code** | D-44 threading, D-01 scheduling, D-10 events and commands, D-02 change detection, D-45 public API tiers, D-47 format versioning, D-21 (logging half) | Every later system is written against the frame, mutation and threading model; R-5, R-7, R-8 get more expensive with every system added | **Decided 2026-10-14** (ADR-0006 through ADR-0012). D-02 is Provisional ("none until profiled"). Stages (D-01), commands and events (D-10), and logging (D-21) are implemented. Interpolation and change detection are not. Profiling is not |
| **2. Before M4/M5** | D-18 (registry shape), D-16 (asset identity), D-05 (alpha classes and materials), D-27 (units) | Persisted data, render model and sprite sizing hang on them; R-1, R-4 | D-16 by a small prototype of a name table plus id; D-05 with a GPU test of a blended mesh against a blended sprite |
| **3. Before M6** | D-25 projection and D-08 multi-camera and targets first, D-48 camera configurations, D-07 depth, then D-06 pixel policy (opt-in, lowest of these) | Shape of `RenderScene` and the pass list; R-2 | Prototype two cameras into one target |
| **4. Before M9** | D-28, D-29 (vocabulary), D-30, D-31, D-11 (record) | Define the character and scene API that M10 to M18 depend on | Built against M8's sprite-sheet implementation and the second consumer |
| **5. Only at their milestone, after a spike** | D-40 scripting runtime, D-09 physics library, D-24 UI model, D-12, D-14, D-15, D-33, D-35, D-36 | They need measurements or a working neighbour; deciding early would be invention | Each milestone's first task is the spike |
| **6. Defer, low impact** | D-19 bundle format, D-20 editor technology, D-26 orientations, D-23 platform/crates.io part | Nothing depends on them until late | Revisit when their milestone is next |

## Summary

| Id | Decision | Needed by | Blocks | Status |
|---|---|---|---|---|
| D-03 | Despawn semantics for entities with children | M2 (T3) | M2 hierarchy fix, M4 | Decided, [ADR-0001](adr/0001-despawn-removes-the-subtree.md) (2026-10-13) |
| D-04 | GPU cache lifetime | M2 (T2) | M2 eviction, M17 unload | Decided, [ADR-0002](adr/0002-gpu-cache-lifetime.md) (2026-10-13) |
| D-22 | Code host and CI provider | M2 | CI config, release | Provisional, [ADR-0005](adr/0005-ci-on-github-actions.md) (GitHub Actions; first-run results in TASKS.md) |
| D-23 | Platform targets, MSRV policy, crates.io | M2 (MSRV), M24 | M24 | MSRV part decided in T1 (1.90); platforms and crates.io still Open |
| D-01 | Scheduling model | M3 | M3, M7, M8, M10, M15 | Decided, [ADR-0007](adr/0007-named-stages-on-the-existing-schedule.md) (2026-10-14). Not implemented |
| D-02 | Change detection | M3 | M3, M4 (dirty transforms), M23 | Provisional, [ADR-0009](adr/0009-no-change-detection-until-measured.md) (2026-10-14): none until a profile |
| D-10 | Events and deferred commands | M3 | M3, M4, M7, M8, M18 | Decided, [ADR-0008](adr/0008-commands-and-per-frame-events.md) (2026-10-14). Implemented in M3a-3 |
| D-18 | Scene format, serde, component registry | M4 (registry shape), M19 | M4, M18, M19, M22 | Open |
| D-27 | Units and pixels per unit | M5 | M5, M8, M9, M13 | Open |
| D-05 | Materials, alpha modes, blended meshes | M5 | M5, M14 | Open |
| D-06 | Pixel-art filtering, texel snapping, MSAA: policies at the level of each technique, **not a default or a foundation** | M6 (after D-25 and D-08) | M6, M8, M9, M13 | Principles accepted (ADR-0004); API ownership and defaults **deferred pending a rendering prototype** |
| D-07 | Depth convention (reverse-Z) | M6 | M6, M14 shadows | Open |
| D-08 | Multi-camera and render targets | M6 | M6, M14, M22 | Open |
| D-25 | Custom projections inside `Camera` | M6 (**first in M6**, [ADR-0004](adr/0004-one-spatial-world-many-camera-configurations.md)) | M6, M13 (oblique tiles) | Open |
| D-48 | Camera configurations: gameplay orientation, coplanar ordering, orientation modes per view | M6 (decide before M7 and M8) | M6, M7, M8, M13, M14 | Principles accepted and `+Y` up / `XZ` ground retained as a **provisional convention** (ADR-0004, 2026-10-13). Prototype evidence recorded 2026-10-14 (three coplanar conflicts; no mechanism chosen). Gameplay-orientation components still deferred to M7 and M8 |
| D-26 | Sprite orientation modes beyond `Billboard`, `Upright`, `Fixed`; `Upright` semantics for steep and oblique views | M6 | none hard | Candidate initial set kept, not exhaustive (ADR-0004). Observed 2026-10-14: orthographic height is `cos(pitch)` of a billboard, straight-down is edge-on, a 60° perspective view is roughly half. **Not accepted as the final design.** Extensions still open |
| D-09 | Physics: custom or library | M7 | M7, M9 | Open |
| D-28 | Character gameplay versus visuals | M8 (provisional), M9 | M9, M10, M11, M12, M18 | Open |
| D-29 | Shared animation control vocabulary and state machine | M8 (vocabulary), M11 | M8, M9, M11, M12 | Open |
| D-30 | Sprite-sheet data model and frame selection | M8 | M8, M14, M17 | Open |
| D-31 | Directional sets and camera-relative facing | M8 | M8 | Open |
| D-24 | UI model and ownership | M10 (start) | M10, D-20 | Open |
| D-37 | UI text and fonts | M10 | M10 | Open |
| D-38 | UI rendering integration and world-space UI | M10 | M10, M22 | Open |
| D-39 | UI input, focus and scaling | M10 | M10 | Open |
| D-32 | One skeleton type or two (2D and 3D) | M11 | M11, M12 | Open |
| D-34 | Pose-first evaluation | M11 | M11, M12 | Proposed |
| D-12 | 2D skeletal approach and formats | M11 | M11, M17 | Open |
| D-33 | Bone storage: entity per bone or pose buffer | M12 | M12 | Open |
| D-35 | Skinning execution | M12 | M12, M14 | Open |
| D-36 | glTF import and handedness conversion | M12 | M12, M17 | Open |
| D-13 | Sprite lighting, normal maps, light culling | M14 | M14 | Open |
| D-14 | Shadow technique | M14 | M14 | Open |
| D-15 | Particles: CPU or GPU, sorting | M15 | M15 | Open |
| D-11 | Audio | M9 (record), M16 | M16, M18 | Open |
| D-16 | Asset identity and dependencies | M17 | M17, M19, M21 | Open |
| D-17 | Importer and dependency policy | M10, M12, M16, M17 | same | Open |
| D-40 | Scripting runtime (Lua, Rhai, Wasm) | M18 (after spike) | M18 | Open |
| D-41 | Script API boundary and exposure | M4 (shape), M18 | M18 | Open |
| D-42 | Script execution model, errors, determinism | M18 | M18, M19 | Open |
| D-43 | Script hot reload, state, debugging | M18 | M18, M19 | Open |
| D-19 | Bundle format | M21 | M21, M24 | Open |
| D-20 | Editor technology | M22 | M22 | Open |
| D-21 | Logging and profiling crates | **Logging: M3**; profiling: M20 | M3 and every error path; M20, M23 | Logging decided, [ADR-0012](adr/0012-logging-uses-the-log-crate.md) (2026-10-14). Implemented in M3a-4 (committed locally, not pushed). Profiling still Open (M20) |
| D-44 | Threading and `Send`/`Sync` model | M3 (design note) | M3, M17, M18, M23 | Decided, [ADR-0006](adr/0006-simulation-stays-single-threaded.md) (2026-10-14). Matches the code |
| D-45 | Public API tiers; how much of `hecs`, `winit`, `wgpu` is public | M3 (rule), M9 (list) | M9, M18, M22 | Decided (the rule), [ADR-0010](adr/0010-three-tier-public-api.md) (2026-10-14). The list is still M9 |
| D-46 | Save games and persisted game state | M18 (shape), M19 | M18, M19, M24 | Open |
| D-47 | Versioning policy for every persisted format | Before M17 | M17, M19, M21, D-46 | Decided, [ADR-0011](adr/0011-persisted-formats-carry-a-version.md) (2026-10-14). No format exists yet |

## Needed during M2 (hardening)

### D-03 Despawn semantics for entities with children

**Decided 2026-10-13: [ADR-0001](adr/0001-despawn-removes-the-subtree.md).** `despawn` removes the subtree;
`detach_children` keeps children as roots. Implemented in T3.

### D-04 GPU cache lifetime

**Decided 2026-10-13: [ADR-0002](adr/0002-gpu-cache-lifetime.md).** Evict on asset removal and after N idle
frames. Implemented in T2.

### D-22 Code host and CI provider

* **Problem.** There was no CI and no chosen host (B-1).
* **Direction (owner, 2026-10-13).** Use GitHub Actions if the repository is hosted on GitHub; validate on
  Windows, macOS and Linux; add Windows early.
* **Provisional, 2026-10-13: [ADR-0005](adr/0005-ci-on-github-actions.md).** The repository is on GitHub,
  and `.github/workflows/ci.yml` runs `scripts/check.sh`, example smoke runs and an MSRV job on all three
  systems. The decision stays Provisional until the first runs are recorded and the owner confirms GitHub
  as the long-term host.

### D-23 Platform targets, MSRV policy, crates.io

* **Problem.** There is no MSRV or toolchain pin (KI-1). The workspace needs a recent compiler and
  fails on 1.83 because a dependency uses edition 2024.
* **Options.** MSRV tracking latest stable; MSRV pinned to the lowest version that builds and bumped
  deliberately; plus a decision on desktop-only for now vs web/mobile after M24, and whether to publish
  to crates.io.
* **Proposed default.** Pin `rust-toolchain.toml` to a stable release and record the verified MSRV in
  `Cargo.toml` (`rust-version`). Desktop only until after M24. Publishing deferred to M24.
* **Needed by.** MSRV part by M2 (T1). Platform and publishing parts by M24.
* **MSRV part: decided by T1 (2026-10-13).** `rust-version = "1.90"` (lowest version the locked graph allows on
  every target, because of `ordered-float 5.5.0`), verified by `cargo test --workspace --locked` on exactly
  1.90.0 on macOS arm64; `rust-toolchain.toml` pins 1.99.0 for development. Policy followed: the MSRV is the
  lowest version that builds with the committed `Cargo.lock`, and is bumped deliberately, not tracked to
  stable. Since verified on Linux, macOS and Windows by CI (KI-27 closed). The platform and crates.io parts remain open.

## Needed during M3-M4 (input, scheduling, scenes)

### D-01 Scheduling model

**Decided 2026-10-14: [ADR-0007](adr/0007-named-stages-on-the-existing-schedule.md).** Named stages and
`before`/`after` on the existing `Schedule`. During each fixed iteration, `Time::delta` is the fixed
step and is restored to the frame delta afterward. Interpolation is opt-in and extraction-only, and it
never writes `Transform` or `GlobalTransform`. `Game::step` is still the hard-wired order; M3a-2
implements the stages. Rendering regression baseline: commit `6dbabb8`.

### D-02 Change detection

**Provisional 2026-10-14: [ADR-0009](adr/0009-no-change-detection-until-measured.md).** No change
detection until a recorded profile shows the full propagation pass is the cost. The previous-local
store used for interpolation is not change detection. Revisit only with that profile (KI-19, M23).

### D-10 Events and deferred commands

**Decided 2026-10-14: [ADR-0008](adr/0008-commands-and-per-frame-events.md).** One FIFO command queue,
flushed at the end of each stage run, including each fixed iteration, through the existing `World`
methods. A failed command changes nothing and does not stop the queue. Typed events become visible at
the next stage boundary, not to other systems in the emitting stage, and not across fixed iterations.
Queues clear at the end of `step`. No observers. Implemented in M3a-3 (`World::commands`,
`World::send`, `World::events`). The writer does not hold the queue borrow; `try_commands` returns
`None` when the queue is already borrowed. Each failure is a `CommandFailure` and stays readable
after `step` until the next step starts. Events still pending at the end-of-step clear are counted
by `discarded_events` for that same window and are not delivered on the next step. The throwaway
check that preceded the implementation is in the ADR.

### D-18 Scene format, serde, component registry

* **Problem.** There is no way to enumerate or name components, so nothing can be serialized.
* **Options.** Format: RON, JSON, or a custom text format. Registry: explicit registration of each
  component type with a stable name and (de)serialize functions, versus derive-based reflection. The
  registry also needs a per-component and per-field **script visibility** attribute (D-41).
  Whether to take a `serde` dependency (a deliberate one, not incidental).
* **Needed by.** The registry's *shape* by M4 (so scene membership and script exposure do not need
  rework); the format and `serde` choice by M19. **Blocks.** M4, M18, M19, M22.

## Needed during M5-M6 (materials, renderer)

### D-27 Units and pixels per unit

* **Problem.** World units are unitless. Sprites map texture pixels to world units through the
  sprite's size, with no engine-wide convention.
* **Options.** A global pixels-per-unit constant; a per-asset or per-sprite value; keep it
  per-sprite and document it.
* **Needed by.** M5 (texture import), at the latest M8 (sprite sheets). **Blocks.** Consistent sizing for the example
  game and tilemaps (M13).

### D-05 Materials, alpha modes, blended meshes

* **Problem.** `Material` is a flat colour plus optional texture. Alpha handling for meshes is limited
  and blended meshes are not sorted with blended sprites (KI-6, KI-14).
* **Options.** Explicit alpha modes (opaque, cutout, blended); whether blended meshes join the sprite
  sort or get their own pass; the material extension path for normal maps (D-13).
* **Needed by.** M5. **Blocks.** M5, M14.

### D-06 Pixel-art policy

* **Problem.** There is no policy for texel snapping, integer scaling or MSAA, and the nearest-neighbour
  look at fractional camera positions can shimmer.
* **Options.** Opt-in snapping per camera; integer-scale viewport mode; MSAA off for pixel-art cameras.
* **Guidance accepted (ADR-0004, 2026-10-13).** Sampling configuration (filter, wrap, anisotropy) belongs at the
  texture or sampler level where appropriate. A pixel-art look may need coordinated settings across materials,
  sprites, cameras and renderer configuration. Texel snapping is implemented at the level its technique needs
  (view-level for camera snapping, object or shader level otherwise). There is no universal style abstraction;
  a convenience preset may configure several settings together while each stays independently settable.
* **Deferred.** Exact API ownership and defaults, until a rendering prototype validates the requirements.
* **Needed by.** M6 (lowest priority there). **Blocks.** M8 and M9 visual quality (frame shimmer), M13 tile seams.

### D-07 Depth convention (reverse-Z)

* **Problem.** Depth is standard 0..1 with `Depth32Float`. Large perspective ranges lose precision
  far away. Shadow maps (M14) will want a consistent choice.
* **Options.** Keep standard Z; adopt reverse-Z.
* **Leaning.** Keep standard Z unless a concrete precision artefact is shown in M6 testing.
* **Needed by.** M6. **Blocks.** M14.

### D-08 Multi-camera and render targets

* **Problem.** Only one camera is rendered per frame (KI-5), and there is no render-to-texture.
* **Options.** Several cameras drawn in `(order, id)` order into the surface; camera-to-texture
  targets; both.
* **Needed by.** M6. **Blocks.** Shadow passes (M14), editor viewports (M22), minimap-style effects.

### D-25 Custom projections inside `Camera`

* **Problem.** The `Projection` trait exists but `Camera` stores a closed enum (KI-11), so an oblique
  or isometric projection cannot be supplied.
* **Options.** Add enum variants (oblique); store a boxed trait object; store a plain matrix override.
* **Presets (ADR-0004).** Presets use whatever `Camera` ends up holding; how a preset takes a custom projection is
  decided with this design, not before.
* **Needed by.** M6; earlier only if tilemaps (M13) need oblique projection.
* **Blocks.** Custom projections; M13 if it chooses oblique tiles.

### D-26 Additional sprite orientations

* **Problem.** `Billboard`, `Upright`, `Fixed` cover current needs. Axis-aligned billboards or
  camera-position-facing billboards may be wanted.
* **Direction (ADR-0004, 2026-10-13).** `Billboard`, `Upright` and `Fixed` remain candidate initial modes, not an
  exhaustive or permanent set (`SpriteOrientation` is `#[non_exhaustive]`). Orientation stays independent of camera
  projection. The semantics of `Upright` under steep, top-down and oblique views are clarified during
  implementation, from the M6 test scene.
* **Needed by.** Nothing is blocked. Decide only when a real scene needs it.

## Needed during M7 (gameplay)

### D-09 Physics: custom or library

* **Options.** A small custom AABB/shape collision layer with queries and triggers; or a library
  (rapier). A library brings a dependency and its own world, which must be reconciled with the
  `Transform` hierarchy and the fixed step.
* **Leaning.** Custom, minimal, because the first game needs overlap tests, triggers and simple
  resolution, not a full solver. Revisit if the game needs more.
* **Needed by.** M7. **Blocks.** M7, M9.

## Needed for characters (M8, M9, M11, M12)

Three representations (sprite sheet, skeletal 2D, 3D skinned) must share one gameplay architecture
without being forced into one animation implementation. These decisions define that line. Their
detail and the proposed boundaries are in [ARCHITECTURE.md](ARCHITECTURE.md#proposed-boundaries-not-implemented).

### D-28 Separating character gameplay from character visuals

* **Problem.** Today the `basic_2_5d` example's own system calls `AnimationPlayer::play`, so gameplay
  and one particular representation are mixed. Nothing defines which entity owns collision, facing or
  the transform that the camera follows.
* **Options.** (a) Convention: a gameplay entity with a visual **child subtree** that a representation
  adapter owns; data flows gameplay to visuals (state, parameters) and back only as events. (b) A single
  `Character` component with a representation enum. (c) Trait-object visuals behind a handle.
* **Leaning.** (a). It matches how bones and attachments already work (`spawn_skeleton(root, ..)`
  spawns children of a root) and needs no new abstraction. Open sub-questions: who owns facing; whether
  root motion (animation moving the gameplay entity) exists at all, and if so as an explicit opt-in
  channel; how ground contact (feet pivot versus collider base) is expressed.
* **Needed by.** M8 starts against the provisional convention; final at the start of M9.
* **Blocks.** M9 and everything that consumes the character API (M10 bindings, M11, M12, M18).

### D-29 Shared animation control vocabulary and state machine

* **Problem.** The three representations differ in what "playing an animation" means (choosing frames;
  sampling and blending poses), but gameplay should issue the same kind of requests.
* **Options.** (a) Share only a control vocabulary: a named clip or state, playback speed, loop mode,
  named float/bool/trigger parameters, a finished signal and named events. Each representation has its
  own evaluator. (b) One `Animator` component with a backend enum. (c) Fully separate APIs per
  representation.
* **Leaning.** (a), with the state machine written first for sprite sheets (M8) and extracted into a
  shared piece only after the skeletal implementation (M11) shows the same shape. Two implementations
  that happen to look alike are not yet an abstraction.
* **Needed by.** The vocabulary by M8; the state-machine question by M11. **Blocks.** M8, M9, M11, M12.

### D-30 Sprite-sheet data model and frame selection

* **Problem.** The primitives exist (`Sprite.region`, `size`, `anchor`; `SpriteAsset` uv and pivot) but
  natural size ignores `region` (KI-21), and nothing defines a sheet or frame.
* **Options.** (a) A `SpriteSheet` asset owns frames (rect, pivot, trim, original size) and the animator
  writes `region`, `size` and `anchor` on the `Sprite` each frame. (b) The sheet produces one
  `SpriteAsset` per frame and the animator swaps `Sprite.asset` (many small assets, name-hash ids, D-16
  collision risk). (c) Add a frame field to `Sprite`.
* **Leaning.** (a): no renderer change and no extra assets. Decide whether per-frame data (pivot,
  duration, events) lives in the sheet or the clip, and whether `region` should scale the natural size.
* **Needed by.** M8. **Blocks.** M8, M14 (normal sheets share frames), M17 (importers).

### D-31 Directional animation sets and camera-relative facing

* **Problem.** In a spatial world the camera can orbit a billboarded character, so "which direction
  frames to show" depends on the camera as well as the entity's facing.
* **Options.** (a) Direction from facing relative to the active camera's yaw, 4 or 8 directions, with an
  optional mirror rule. (b) Direction from world facing only (fixed-camera games). (c) Selectable per
  character.
* **Leaning.** (c) with (a) as the default, because it is what distinguishes this engine. The animate
  stage must then read the camera's transform from the previous propagation, which fixes a one-frame
  latency rule to document.
* **Needed by.** M8. **Blocks.** M8 directional sets.

### D-32 One skeleton type or two (2D and 3D)

* **Problem.** `Skeleton`, `Pose` and `AnimationClip` already use 3D `Transform` (position, `Quat` with
  slerp, scale), yet are described as 2D. 2D needs slots, draw order and sprite attachments; 3D needs
  inverse bind matrices and vertex weights.
* **Options.** (a) One TRS skeleton, pose and clip for both, with 2D-only data (slots, parts, skins) and
  3D-only data (skin binding) layered beside it. (b) `Skeleton2D` and `Skeleton3D` separately.
* **Leaning.** (a) for hierarchy, pose, sampling, blending and IK maths; (b)-style separation for
  everything that is genuinely different. Settle before M11 adds blending so it is written once.
* **Needed by.** M11 (start). **Blocks.** M11, M12.

### D-33 Bone storage: entity per bone or pose buffer with sockets

* **Problem.** Today every bone is an entity, so 2D parts can be ordinary child sprites, but transform
  propagation is a full pass over all entities (KI-19) and a 3D character can have dozens of bones.
* **Options.** (a) Entity per bone everywhere. (b) Entity per bone for 2D; a pose buffer for 3D with
  entities only for named attachment points (sockets). (c) Pose buffer plus sockets for both.
* **Leaning.** Decide by measurement: M6 records a baseline, and M11 and M12 test bone counts against it.
* **Needed by.** M12 (earlier measurements from M6). **Blocks.** M12 attachments and crowd cost.

### D-34 Pose-first evaluation

* **Problem.** `animate_system` writes sampled transforms directly onto bone entities, so there is no
  step where blending, constraints or IK can run (KI-22).
* **Proposed default.** Sample to `Pose`, blend, constrain, solve IK, then apply to entities. The
  evaluator depends on neither the world nor the renderer. This is a refactor of existing code, not a
  new feature, and it is the first task of M11.
* **Status.** Proposed. **Needed by.** M11. **Blocks.** M11, M12.

### D-35 Skinning execution

* **Problem.** The backend sees only `RenderScene` (plain data). A skinned instance needs joint matrices.
* **Options.** (a) GPU skinning: a joint palette per instance in `RenderScene`, applied in the vertex
  shader; the vertex layout gains joints and weights. (b) CPU skinning: deform vertices each frame and
  upload (simple, costly for many characters).
* **Leaning.** (a). The palette is plain data, so the seam keeps its shape.
* **Needed by.** M12. **Blocks.** M12, and skinned shadow casters in M14.

### D-36 glTF import

* **Problem.** A 3D character needs model, skin and animation data, and no loader exists.
* **Options.** The `gltf` crate; a hand-written parser; another format first. Scope: meshes, base-colour
  materials, hierarchy, skins, TRS animation (cubic spline later).
* **Must be decided with it.** The coordinate conversion. glTF is right-handed, +Y up, model front along
  +Z; Dimora is left-handed with +Z deeper. The rule (mirror axis, winding flip, clip handedness) is a
  documented and tested decision, not an accident of the importer.
* **Needed by.** M12, with a minimal importer; M17 generalises it. **Blocks.** M12, M17.

### D-12 2D skeletal approach and formats

* **Options.** Sprite-part skeletons only (the current data model); 2D mesh deformation with weights;
  both. Clip source format: custom, or a 2D tool format.
* **Needed by.** M11. **Blocks.** M11, M17 importers.

## Needed for the UI runtime (M10)

### D-24 UI model and ownership

* **Problem.** Layout, interaction and styling need a shape before any widget code. Anchoring to world
  entities, keyboard and gamepad focus and hover state all need widgets with persistent identity.
* **Options.** (a) A small custom retained tree built from code. (b) An immediate-mode API (own or a
  library such as egui). (c) A library for game UI. A library must match the project's `wgpu` and
  `winit` versions, which has to be checked, and game-style skinning and focus navigation vary in how
  well they are supported.
* **Leaning.** (a), because focus order, world anchoring and scrolling want persistent nodes. M10's
  first task is a design note with a spike if the leaning is in doubt.
* **Needed by.** The start of M10 (the M9 slice informs it). **Blocks.** M10, and D-20 (the editor may
  reuse the runtime).

### D-37 UI text and fonts

* **Options.** Rasteriser (`fontdue`, `ab_glyph`) with a glyph atlas; a fuller text stack
  (`cosmic-text`/`swash`) with shaping; bitmap or signed-distance-field fonts. Scope: Latin first, or
  complex scripts and bidirectional text.
* **Constraint found in the code.** Textures re-upload whole on any change (KI-25), so a glyph atlas
  needs the sub-rectangle update planned in M5.
* **Needed by.** M10 (before text work). **Blocks.** M10 text, localisation later.

### D-38 UI rendering integration and world-space UI

* **Options.** Where the draw-list type lives (the UI crate, with the backend depending on it, or
  `engine-renderer`); an overlay pass after the world (no depth, painter's order, scissor clipping); and
  for world-space UI either projecting an entity position into screen space or rendering UI into a
  texture drawn on a world quad.
* **Leaning.** Overlay pass plus projected anchors first; render-to-texture panels only if a real need
  appears. The UI draw list is plain data like `RenderScene`.
* **Needed by.** M10 (start). **Blocks.** M10 rendering, M22 if the editor reuses it.

### D-39 UI input, focus and scaling

* **Options.** Input capture (does a pressed pointer or key reach gameplay after UI consumed it);
  navigation model (spatial or ordered focus, activation through M3 actions); text input scope (none for
  the first runtime, or text fields with IME); scaling (logical units, reference resolution, device scale
  factor, which the platform layer does not forward today, KI-24).
* **Needed by.** M10 (start). **Blocks.** M10.

## Needed for lighting, particles and audio (M14, M15, M16)

### D-13 Sprite lighting, normal maps, light culling

* **Problem.** Forward shading handles at most 16 lights and sprites use wrap lighting; there are no
  normal maps (KI-6).
* **Options.** Add a normal-map slot to `Material`; per-tile or clustered light culling; keep forward
  with a hard light cap.
* **Needed by.** M14 (the normal-map slot is prepared in M5). **Blocks.** M14.

### D-14 Shadow technique

* **Options.** Shadow maps (directional first); blob shadows for sprites; projected planar shadows.
  Interaction with blended sprites and the depth convention (D-07) must be settled, and casters now
  include animated sprite frames and skinned meshes.
* **Needed by.** M14. **Blocks.** M14.

### D-15 Particles

* **Options.** CPU simulation with instanced draw; GPU simulation. Sorting policy against blended
  sprites (global sort vs per-system).
* **Leaning.** CPU and instanced, since it reuses the sprite pipeline.
* **Needed by.** M15.

### D-11 Audio

* **Options.** A thin wrapper over a library (`rodio`, `kira`); or a custom backend. Always with a null
  backend for tests and CI.
* **Needed by.** M9 records whether the slice includes sound; M16 chooses the backend. It was
  previously unscheduled and is now a milestone because scripts and animation events call it.
* **Blocks.** M16, M18's audio API.

## Needed for scripting (M18)

Scripting is planned for after the core gameplay APIs are stable (M9). These four decisions must be made
before committing to a runtime; none should be made earlier than the start of M18's spike, because an
early answer would be made without real gameplay APIs to test against.

### D-40 Scripting runtime

Candidates: **Lua** (through `mlua`), **Rhai**, and **WebAssembly** only if its use case is real.
Exactly one ships; no custom language.

The table below comes from general knowledge of these projects. **None of it has been measured in this
repository.** M18's first task is a spike that checks the criteria that matter to Dimora and replaces
this text with results.

| Criterion | Lua via `mlua` | Rhai | WebAssembly (`wasmtime` or `wasmi`) |
|---|---|---|---|
| Rust integration | Mature binding; several Lua flavours (5.x, LuaJIT, Luau). Lua itself is C, compiled by a build script, so a C toolchain is needed | Pure Rust; registers Rust types and functions directly; no C toolchain | Embedder crate plus an interface definition for host calls; guests are compiled separately |
| Runtime performance | Fast interpreter; LuaJIT faster but not on every platform | Slower AST interpreter; fine for light logic, to be measured | Near native for guest code; every host call crosses a boundary |
| Embedding complexity | Moderate (C build, lifetimes of userdata) | Lowest | Highest (ABI, marshalling, a toolchain per guest language) |
| API bindings | Userdata with methods or tables of functions; hand-written per type | Custom types and functions registered in Rust; hand-written per type | Host functions on integers and memory; bulk data needs a shared-memory design |
| Error reporting | Messages with chunk name, line and traceback | Errors with line and column | Traps with a backtrace; source mapping depends on guest tooling |
| Hot reload | Re-run a chunk; state kept in tables has to be managed | Recompile the AST (a value) | Instantiate a new module; state migration is manual |
| Sandboxing and permissions | Load only chosen standard libraries; instruction hooks for budgets; optional memory limit; Luau is designed for sandboxing | Sandboxed by default; operation, call-depth and size limits built in | Strongest isolation; fuel metering and memory caps |
| Debugging | Debug hooks and existing editor/debug-adapter ecosystem | A debugging interface exists; little tooling around it | DWARF debugging possible but heavy for script authors |
| Long-term maintenance | Very stable language, large ecosystem, familiar to game developers; binding maintained by a small team | Smaller community; 1.x API | Standard backed by large vendors; embedder APIs and the component model still change |

* **Leaning.** Lua through `mlua` with a restricted standard library, with Rhai as the fallback if a C
  dependency is unacceptable. WebAssembly only if the use case is running **untrusted third-party code
  or distributed mods**, since otherwise its toolchain and ABI cost buys nothing for gameplay scripts.
* **Criteria to decide on.** Measured per-call overhead for the 1,000-entity budget; build and CI impact
  on all three desktop platforms; quality of errors with file and line; reload behaviour; effort to
  enforce the sandbox in D-42; and whether the team accepts a C dependency.
* **Needed by.** The end of M18's spike. **Blocks.** All of M18.

### D-41 Script API boundary and exposure model

* **Problem.** Scripts must reach gameplay concepts and nothing else. Reflecting everything the ECS has
  would expose internals and freeze them.
* **Options.** (a) A hand-written, versioned facade with opaque entity handles; components reachable
  only by registry name and only if marked script-visible, with read/write flags. (b) Derive-based
  reflection exposing every registered component. (c) Per-script capability declarations on top of (a).
* **Leaning.** (a), with (c) considered once there is more than one kind of script. The component
  registry (D-18) must allow a visibility attribute, which M4 leaves room for.
* **Needed by.** The registry's shape by M4; the API surface by M18. **Blocks.** M18.

### D-42 Script execution model, errors and determinism

* **Options.** Per-entity script components with callbacks versus script-defined systems; the stage or
  stages in which scripts run (fixed update for gameplay); mutation only through the M3 command buffer;
  whether a faulting script instance is disabled or aborts the game; per-frame instruction and memory
  budgets; seeded random only, with no wall-clock access, so replay (M9) stays deterministic.
* **Needed by.** M18 (start). **Blocks.** M18, M19 (script state in scenes).

### D-43 Script hot reload, state and debugging

* **Options.** Reload preserving instance state, or restarting instances; behaviour on a failed reload
  (keep the old version); how errors surface in the M20 overlay; whether a debugger protocol is in scope
  (not at first).
* **Needed by.** M18, after D-40. **Blocks.** M18 hot reload; M19 if state is serialized.

## Needed for assets, serialization and bundles (M17, M19, M21)

### D-16 Asset identity and dependencies

* **Problem.** `AssetId` is an FNV-1a hash of the name; collisions are not detected (KI-12) and
  there is no dependency graph for cascading reload.
* **Added by the 2026-10-13 audit (R-4).** `AssetHandle<T>` holds only the hash, so a serializer cannot
  recover a name from a handle, nothing is reference counted, and an id changes when a name changes.
  Whatever is chosen must say what a scene file or bundle stores for an asset reference, how rename
  works, and who keeps an asset alive. The first persisted asset format (M8 sheet descriptors, in M17)
  must not ship before this is decided.
* **Options.** Keep name hashes with a collision guard (M2 adds the guard); stable GUIDs with a
  name index; path-derived ids.
* **Needed by.** M17, though the collision guard landed in M2 (T3, per asset type). Fonts (M10) and sprite
  sheets (M8) use whatever exists then.
* **Blocks.** M17, M19 (what scenes reference), M21.

### D-17 Importer and dependency policy

* **Options.** Which formats and which crates (PNG exists; glTF for M12, audio decoders for M16, fonts for
  M10, atlas JSON, Tiled). A rule for accepting a new dependency: it must unblock an acceptance criterion
  in the current milestone.
* **Needed by.** The glTF and font crates by M12 and M10; the audio decoders by M16; the policy by M17.

### D-19 Bundle format

* **Options.** A custom archive with an index and content hashes, optional compression; versus an
  existing container format.
* **Needed by.** M21. **Blocks.** M21, M24.

## Needed later

### D-20 Editor technology

* **Context (2026-10-13).** The editor is a core product goal: a user-friendly tool for building spatial
  worlds, not only for code authors. Owner direction ([ADR-0003](adr/0003-incremental-tooling-editor-later.md)):
  the full editor stays at M22 behind a stability gate, developer tooling arrives incrementally, and the
  runtime holds no editor-specific state. Its *technology* can wait, but its *requirements* shape earlier
  decisions (D-10 commands and undo, D-18 registry with field metadata, D-16 stable ids, D-08 camera targets,
  D-47 formats). Those decisions must list the editor as a consumer.
* **Options.** Reuse the engine's own UI runtime (M10); an immediate-mode library inside a dedicated
  crate; a separate application; a web UI. Whichever is chosen must give end users a friendly experience,
  which weighs against a purely developer-style tool.
* **Needed by.** M22, after D-24 and D-18 and the stability gate in ADR-0003. A short technology spike
  at the end of M10 is allowed to test the choice; it does not start the editor. **Blocks.** M22 only.

## Added by the 2026-10-13 audit

These are cross-system decisions the earlier backlog did not list (and D-21, which the audit re-timed).
Logging (D-21), D-44, D-45 and D-47 were decided on 2026-10-14. Profiling (the other half of D-21) and
D-46 are still open.

### D-21 Logging and profiling crates

**Logging decided 2026-10-14: [ADR-0012](adr/0012-logging-uses-the-log-crate.md).** Use the `log` crate,
added in M3a-4, with a tiny stderr backend when no logger is installed. Not `tracing`, and not an
engine-owned facade crate. "Once" means once per occurrence, not once per frame while a failure remains.
Implemented in M3a-4 (committed locally, not pushed): failed loads, skipped renderables (at most 64 remembered ids), surface
loss, render and window errors, and `CommandFailure` records. `Diagnostics` is the counter resource.
Profiling scopes and tooling stay in M20 and are still open.

### D-44 Threading and `Send`/`Sync` model

**Decided 2026-10-14: [ADR-0006](adr/0006-simulation-stays-single-threaded.md).** The calling thread owns
the `World` for the whole step. Systems are not `Send`. `World` stays `!Sync`. Other threads see only
`Send` snapshots. This is already how the code works. M3a does not add parallel systems. Revisit only
if a measured gameplay workload cannot meet its budget on one thread.

### D-45 Public API tiers

**Decided 2026-10-14: [ADR-0010](adr/0010-three-tier-public-api.md).** Three tiers: contract, backend
seam, and internal. M3a adds no public `hecs` type and does not wrap `hecs`. Existing `winit` and
`wgpu` re-exports stay until M9 writes `docs/API_SURFACE.md`. The leak is still in the code.

### D-46 Save games and persisted game state

* **Problem.** Scene files (M19) are authored content. Save games are runtime state written by shipped
  games, which players keep across engine upgrades. No milestone owns them; M18's script API already
  promises a "typed game-state store".
* **Options.** (a) A game-state store resource plus a save API with its own versioned format. (b) Save
  by serializing the world with the scene machinery. (c) Leave to each game.
* **Leaning.** (a): saves must not depend on entity ids or scene internals. Decide with D-47.
* **Needed by.** The store's shape by M18; the format by M19 or M24. **Blocks.** M18, M19, M24.

### D-47 Versioning policy for persisted formats

**Decided 2026-10-14: [ADR-0011](adr/0011-persisted-formats-carry-a-version.md).** Every persisted format
(asset descriptors, compiled assets, scenes, bundles, saves, settings) carries a version from its first
line. Loaders return an error for an unknown version. A bump ships a migration or a documented break,
plus a test that loads an older fixture. Ids in files are the stable ids of D-16, which is still open,
never `hecs` indices. No format and no `serde` dependency in M3a.

### D-48 Camera configurations: gameplay plane, coplanar ordering, orientation per view

* **Problem.** [ADR-0004](adr/0004-one-spatial-world-many-camera-configurations.md) commits to one world seen
  through many camera configurations (R-16). Three things are unspecified and each decides how top-down,
  side-scrolling and isometric games feel: (1) **gameplay orientation**: where the ground plane and up axis
  are for movement, collision and directional animation, independent of the camera; (2) **coplanar content**:
  cutout sprites and tiles at the same depth z-fight, and `RenderOrder` only orders blended sprites, so a
  a flat 2D layering needs a defined ordering (spatial depth first, with a separate explicit override, see below); (3) **sprite orientation per view**: `Upright`
  sprites are edge-on under a straight-down camera.
* **Decided by the owner (2026-10-13, [ADR-0004](adr/0004-one-spatial-world-many-camera-configurations.md)).**
  Gameplay orientation is data, separate from world transforms and visual presentation, never derived only
  from the camera or the sprite, and not stored twice where one value can be derived from another. Coplanar
  ordering: spatial depth, transparency sorting, render layers or queues and explicit render order are four
  distinct concepts; spatial visibility stays with transforms, camera depth and the depth test; deliberate
  ordering of coplanar content is supported as an explicit override and is not presented as depth; room is left
  for sorting or occlusion policies for isometric and similar styles, and no generalised occlusion framework is
  built before a scene shows the need.
* **Provisional convention (owner, 2026-10-13).** `+Y` up and an `XZ` ground plane, left-handed, `+Z` forward,
  retained after inspecting the implementation (no material incompatibility; `Upright` is the only world-up
  reader in the renderer). A movement plane is a gameplay constraint chosen per game (M7 movement option), not
  the ground plane. World transform, movement direction, gameplay-facing direction and visual orientation are
  distinct concepts; values are derived where meanings align and stored only where behaviour requires. A per-game
  up axis is not provided or planned. Details and implications: ADR-0004, question 3.
* **Deferred API decisions.** The gameplay-orientation component design (until M7 movement and M8
  directional-animation requirements exist); preset signatures and defaults (with D-25).
* **Prototype evidence (recorded 2026-10-14, Apple M5, Metal; tests in
  `examples/spatial_scene/tests/configurations.rs`).** No mechanism was selected.
  1. **Equal-height cutout decals** follow spawn order, not space
     (`equal_height_cutout_decals_are_decided_by_draw_order_not_by_space`).
  2. **A cutout decal coplanar with a mesh** is shown under some cameras and hidden under others
     (`known_conflict_a_decal_coplanar_with_a_mesh_is_hidden_or_shown_by_rounding`). On that GPU, orthographic
     showed it and the other four downward configurations hid it. The test locks the split, not the name.
  3. **A blended decal at the same height as a cutout one** is likewise camera-dependent
     (`known_conflict_a_blended_decal_at_the_same_height_as_a_cutout_one_depends_on_the_camera`). On that GPU,
     isometric showed it and the other four hid it. Same caveat.
  Decals one unit apart in height resolve by the depth test in all five downward configurations. Isometric did
  not show a sorting failure beyond these coplanar cases, so no isometric occlusion policy is justified yet.
  `Upright` measurements are under D-26: current behaviour, not a chosen design. Candidates for a later
  mechanism, none preferred: an offset helper, a depth bias, extending `RenderOrder` to cutout content, a
  layer or queue.
* **Needed by.** Decided during M6, before M7 and M8 build on it. **Blocks.** M7, M8, M13, M14.

## Reopening a settled decision

A decision in ARCHITECTURE.md may be reopened only when all of the following hold:

1. A specific failing test, measurement or concrete limitation from the repository is cited.
2. The cost of working around it is stated.
3. The proposed change names what it replaces and which tests must still pass.

"It would be cleaner" or "another engine does it differently" is not sufficient.
