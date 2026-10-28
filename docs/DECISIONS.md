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
| OD-2 | Dimora is not defined by HD-2D or any single view; one spatial world supports orthographic, perspective, isometric, top-down, side-scrolling and custom cameras as intended capabilities; projection is separate from gameplay orientation and style; the showcase is a reusable multi-camera test scene | **Confirmed** as direction (2026-10-13). The six open questions were answered the same day; the answers are classified (accepted principle, provisional convention, deferred API decision, needs prototype evidence) in ADR-0004 "Resolution of the open questions". Mechanisms, owner response 2026-10-13: camera presets as constructors **Accepted**; style as policy **Accepted with clarification** (per level, no universal style abstraction); gameplay orientation as data **Accepted**; coplanar ordering **Amended** (spatial depth, transparency sorting, render layers or queues and explicit render order stay distinct; no generalised occlusion framework yet). The test scene now exists in `spatial_scene` (verified 2026-10-14). Public camera constructors exist (`16ed495`). Coplanar sprite priority is [ADR-0020](adr/0020-coplanar-priority.md). Style-policy APIs and gameplay-orientation components are still not implemented | [ADR-0004](adr/0004-one-spatial-world-many-camera-configurations.md), D-48 |
| OD-3 | Licence: `MIT OR Apache-2.0` | **Proposed, not confirmed.** No licence files are added until the owner approves. Copyright holder also unconfirmed | TASKS.md "Licence audit" (KI-17) |
| OD-4 | Use GitHub Actions if the repository is hosted on GitHub; validate on Windows, macOS and Linux; Windows early | **Confirmed** as direction (2026-10-13). That GitHub is the long-term host is **assumed** (the repository and its pull requests are on GitHub); the owner has not stated it | [ADR-0005](adr/0005-ci-on-github-actions.md) (Provisional), D-22 |
| OD-5 | Does a licence also need a contributor policy (a CLA or DCO), given contributions from several authors and AI agents? | **Open** | TASKS.md "Licence audit" |
| OD-6 | Is the product audience "programmers who also want visual tools" or "creators who may not write Rust"? How much of a game must be buildable without Rust? | **Open** (direction: both, editor is a goal) | VISION.md open questions |

### What to resolve first

Ordered by how many systems, APIs and formats a decision touches and how costly a late change is (see
the risk register). **Prototype where the answer depends on a measurement; do not guess.**

| Tier | Decisions | Why now | How to resolve |
|---|---|---|---|
| **1. Before M3 code** | D-44 threading, D-01 scheduling, D-10 events and commands, D-02 change detection, D-45 public API tiers, D-47 format versioning, D-21 (logging half) | Every later system is written against the frame, mutation and threading model; R-5, R-7, R-8 get more expensive with every system added | **Decided 2026-10-14** (ADR-0006 through ADR-0012). D-02 is Provisional ("none until profiled"). Stages (D-01), commands and events (D-10), logging (D-21), and interpolation (D-01, M3a-5, committed locally, not pushed) are implemented. Change detection is not. Profiling is not |
| **2. Before M4/M5** | D-18 (registry shape, decided in part), D-16 (asset identity), D-05 (alpha classes and materials), D-27 (units) | Persisted data, render model and sprite sizing hang on them; R-1, R-4 | D-18's scene model and registry shape are ADR-0014. D-16 stays open. D-05 is ADR-0015 (settled). M5-5C is `ab1b6f7`. M5-5D is `a7efd6c`. M5-6 is `92286ce`. M5 is accepted locally. D-27 is ADR-0016 (per-sprite `pixels_per_unit`) |
| **3. Before M6** | D-25 projection and D-08 multi-camera and targets first, D-48 camera configurations, D-07 depth, then D-06 pixel policy (opt-in, lowest of these) | Shape of `RenderScene` and the pass list; R-2 | Prototype two cameras into one target |
| **4. Before M9** | D-28, D-29 (vocabulary), D-30, D-31, D-11 (record) | Define the character and scene API that M10 to M18 depend on | Built against M8's sprite-sheet implementation and the second consumer |
| **5. Only at their milestone, after a spike** | D-40 scripting runtime, D-24 UI model, D-12, D-14, D-15, D-33, D-35, D-36 | They need measurements or a working neighbour; deciding early would be invention | Each milestone's first task is the spike. D-09 was settled on 2026-10-27 ([ADR-0025](adr/0025-kinematic-collision.md)). M7-1A implements the query index. M7-1B-1 implements the kinematic slide. M7-1B-2 implements trigger events. M7-1B-2R makes a sensor pair nonblocking from either side. Locomotion is not implemented |
| **6. Defer, low impact** | D-19 bundle format, D-20 editor technology, D-23 platform/crates.io part | Nothing depends on them until late | Revisit when their milestone is next |

## Summary

| Id | Decision | Needed by | Blocks | Status |
|---|---|---|---|---|
| D-03 | Despawn semantics for entities with children | M2 (T3) | M2 hierarchy fix, M4 | Decided, [ADR-0001](adr/0001-despawn-removes-the-subtree.md) (2026-10-13) |
| D-04 | GPU cache lifetime | M2 (T2) | M2 eviction, M17 unload | Decided, [ADR-0002](adr/0002-gpu-cache-lifetime.md) (2026-10-13) |
| D-22 | Code host and CI provider | M2 | CI config, release | Provisional, [ADR-0005](adr/0005-ci-on-github-actions.md) (GitHub Actions; first-run results in TASKS.md) |
| D-23 | Platform targets, MSRV policy, crates.io | M2 (MSRV), M24 | M24 | MSRV part decided in T1 (1.90); platforms and crates.io still Open |
| D-01 | Scheduling model | M3 | M3, M7, M8, M10, M15 | Decided, [ADR-0007](adr/0007-named-stages-on-the-existing-schedule.md) (2026-10-14). Stages in M3a-2. Interpolation in M3a-5 (committed locally, not pushed) |
| D-02 | Change detection | M3 | M3, M4 (dirty transforms), M23 | Provisional, [ADR-0009](adr/0009-no-change-detection-until-measured.md) (2026-10-14): none until a profile |
| D-10 | Events and deferred commands | M3 | M3, M4, M7, M8, M18 | Decided, [ADR-0008](adr/0008-commands-and-per-frame-events.md) (2026-10-14). Implemented in M3a-3 |
| D-49 | Input actions, fixed-step one-shots, viewport scale, window controls | M3b | M3b, M10 | Decided, [ADR-0013](adr/0013-input-actions-and-window-controls.md) (2026-10-15). Phases 1–4 are checkpointed locally (`57dbc4a`, `83286a9`, `9c99edf`, `cfdf358`) and are not pushed. M3b's acceptance criteria are met |
| D-18 | Scene format, serde, component registry | M4 (registry shape), M19 | M4, M18, M19, M22 | Decided in part, [ADR-0014](adr/0014-scene-model-and-component-registry.md) (2026-10-16). Registry, scene model, membership, load, unload, replace, deferred scene operations, the `spatial_scene` switch, and one code-defined prefab are settled for M4. Phase 1 is `cdb5ec3`. Phase 2 is `2c76f71`. Phase 3 is `f9a4aab`. M4-4A is `302622b`. M4-4B is `128c70e`. M4-4C is `e12fb5b`. M4-4D is `8398e49`. M4-4E is `d58c66b`. M4-5 is `2b79806`. None is pushed. No scene file in M4. D-16 stays open |
| D-27 | Units and pixels per unit | M5 | M5, M8, M9, M13 | Decided, [ADR-0016](adr/0016-per-sprite-pixels-per-unit.md) (2026-10-16). Per-sprite `pixels_per_unit`, default 16. Not a code change |
| D-05 | Materials, alpha modes, blended meshes | M5 | M5, M14 | Decided, [ADR-0015](adr/0015-material-alpha-classes.md) (2026-10-16). Three classes, one blended sort, and the bounding-sphere-centre depth for a blended mesh. CPU `MaterialAsset` is `e0c29ef`. M5-5B is `3b3cd4a`. M5-5C is `ab1b6f7`. M5-5D is `a7efd6c`. Procedural meshes are [ADR-0018](adr/0018-procedural-mesh-generators.md) (M5-6, `92286ce`). M5 is accepted locally |
| D-06 | Pixel-art filtering, texel snapping, MSAA: policies at the level of each technique, **not a default or a foundation** | M6 (after D-25 and D-08) | M6, M8, M9, M13 | Closed for the approved policy (owner, 2026-10-26). Sampling is [ADR-0017](adr/0017-texture-sampling-colour-space-and-atlases.md). Orthographic camera-view snap is [ADR-0022](adr/0022-orthographic-pixel-snap.md) (option A, `4a87bb0`). Integer-resolution presentation is [ADR-0023](adr/0023-integer-resolution-camera-presentation.md) (`d491bda`). Optional 1×/4× MSAA is [ADR-0024](adr/0024-optional-4x-msaa.md). Per-object snap (ADR-0022 option B), `Load` through an internal image, other small-window scales, alpha-to-coverage, and other sample counts stay deferred and are not implemented. M6 is not complete |
| D-07 | Depth convention (reverse-Z) | M6 | M6, M14 shadows | Decided, [ADR-0019](adr/0019-reverse-z.md) (2026-10-23). Finite reverse-Z, compare `Greater`, clear 0. Verified on Apple M5, Metal. Vulkan, DirectX, OpenGL, and lavapipe are not verified |
| D-08 | Multi-camera and render targets | M6 | M6, M14, M22 | Implemented locally. M6-2B is `6a47882` (surface cameras). M6-2C is committed locally and not pushed (offscreen targets and camera-to-texture). No render graph. M6 is not complete. D-07 is decided. Coplanar ordering is [ADR-0020](adr/0020-coplanar-priority.md). Gameplay orientation stays open. D-06 is closed for the approved policy. D-26 is [ADR-0021](adr/0021-sprite-orientation.md) |
| D-25 | Custom projections inside `Camera` | M6 (**first in M6**, [ADR-0004](adr/0004-one-spatial-world-many-camera-configurations.md)) | M6, M13 (oblique tiles) | Implemented in M6-1B (`a38cb30`, not pushed): `CameraProjection::Custom(Mat4)`. The example `custom` camera is M6-1C (committed locally, not pushed). Public oblique builders (`16ed495`) store that same variant and do not change this matrix contract. Coplanar ordering is [ADR-0020](adr/0020-coplanar-priority.md). D-26 is [ADR-0021](adr/0021-sprite-orientation.md) |
| D-48 | Camera configurations: gameplay orientation, coplanar ordering, orientation modes per view | M6 (decide before M7 and M8) | M6, M7, M8, M13, M14 | Coplanar ordering decided, [ADR-0020](adr/0020-coplanar-priority.md) (2026-10-23): opt-in `CoplanarPriority`, depth-only, fallback thickness `1/256`. Verified on Apple M5, Metal. Gameplay-orientation components still deferred to M7 and M8. D-26 is [ADR-0021](adr/0021-sprite-orientation.md). `+Y` up / `XZ` ground stays a provisional convention (ADR-0004) |
| D-26 | Sprite orientation modes beyond `Billboard`, `Upright`, `Fixed`; `Upright` semantics for steep and oblique views | M6 | none hard | Decided, [ADR-0021](adr/0021-sprite-orientation.md) (2026-10-23). Keep `Billboard`, `Upright`, and `Fixed`. Top-down `Upright` stays edge-on. No fourth mode. Extraction is unchanged |
| D-09 | Physics: custom or library | M7 | M7, M9 | Settled, [ADR-0025](adr/0025-kinematic-collision.md) (2026-10-27). Private `parry3d` 0.31.1 queries. The ECS owns transforms. No rigid-body pipeline. M7-1A implements the query index. M7-1B-1 implements the kinematic slide, `BVec3`, and `MoveResult`. M7-1B-2 implements one `TriggerEvent` queue. M7-1B-2R makes a sensor pair nonblocking from either side. Locomotion is not implemented |
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
| D-45 | Public API tiers; how much of `hecs`, `winit`, `wgpu` is public | M3 (rule), M9 (list) | M9, M18, M22 | Decided (the rule), [ADR-0010](adr/0010-three-tier-public-api.md) (2026-10-14). M3a-6 reviewed the M3a additions. The list is still M9 |
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
never writes `Transform` or `GlobalTransform`. `Game::step` is still the hard-wired order. M3a-2
implements the stages. M3a-5 implements the marker, the previous local, and the extract-time blend
(committed locally, not pushed). Rendering regression baseline: commit `6dbabb8`.

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
check that preceded the implementation is in the ADR. Device input is not one of these events.
M3b keeps it on `Input` (ADR-0013).

### D-49 Input actions and window controls

**Decided 2026-10-15: [ADR-0013](adr/0013-input-actions-and-window-controls.md).** Frame edges on `Input`
stay visible to every fixed iteration and clear in `input_clear`. Phase 2 adds a latch that a fixed
iteration sees once, including a transition that arrived on a frame with no fixed step. Several
transitions before that delivery collapse to one press bit and one release bit. `Actions` follows
both timings. Phase 1 widens `Key` and adds `mouse_released` on the frame clock. Viewport scale uses
winit 0.30.13 `ScaleFactorChanged`. Window controls are borderless fullscreen, vsync with an explicit
capability fallback, an event-loop frame cap, and cursor grab. D-39's UI capture, focus, and text stay
open. Phases 1–3 are checkpointed locally (`57dbc4a`, `83286a9`, `9c99edf`) and are not pushed.
Phase 3 stores physical size, logical size (`physical / scale`), and a validated scale, with pointer
conversion and no new `winit` type on that API. Phase 4 is checkpointed locally as `cfdf358`: borderless fullscreen with a restored windowed size, vsync chosen from the surface's
listed modes (`Fifo` when on; `Immediate`, then `Mailbox`, then `Fifo` when off), an optional
`WaitUntil` frame cap, cursor visibility and grab (locked, then confined), device-motion
`mouse_delta` while grabbed, and focus-loss release of held input and the grab. Exclusive fullscreen
is not implemented. None of M3b is pushed.

### D-18 Scene format, serde, component registry

**Decided in part 2026-10-15: [ADR-0014](adr/0014-scene-model-and-component-registry.md).** One world,
code-defined scenes, `SceneMember` for scene-owned entities, and an explicit registry with stable names
and hand-written field metadata. Hierarchical visibility is an extract-time walk. Registry types live in
`engine-world`; `engine` registers built-ins. Code-defined prefabs are functions that spawn into that
world. M4 has no scene file and no `serde`, RON, or JSON dependency. Phase 1 (registry and text dumps)
is commit `cdb5ec3` and is not pushed. Phase 2 (hierarchical visibility in extraction) is commit
`2c76f71` and is not pushed. `Camera.active` still selects the camera. Phase 3 looks up `Name` and
`Tags` by scanning live entities. It is commit `f9a4aab` and is not pushed.

M4-4A is commit `302622b`. It tracks empty scene instances and `SceneMember` on existing entities. `SceneManager` is an internal resource. Public operations are `World` methods. `SceneInstanceId` and scene-local `u32`s are runtime ids and are not reused. `SceneMember` is `Persistence::Runtime`. One entity is in at most one instance. Lookup checks the live `Entity`, the component, the instance id, and the local id. `World::despawn` does not update the map. Stale rows fail lookup and stay until `sweep_scene_mappings`. Ordinary `Result` errors put the manager back. A panic while it is taken out does not.

M4-4B is commit `128c70e`. It adds `SceneBuilder` and synchronous `load_scene`. The builder spawns entities and parents them only under entities that same build created. A failed load despawns what it created, consumes the instance id, and does not publish the instance.

M4-4C is commit `e12fb5b`. It adds `unload_scene` and `replace_scene`. Unload destroys validated members of that instance and lifts a non-member child to a TRS approximation of its world matrix. It does not call `World::despawn`, and it does not remove assets. A failed replacement build leaves the original instance in place. After a successful preflight, the public builder cannot make the following unload fail. `create_scene_instance` and `track_scene_entity` stay public low-level methods. They should become `pub(crate)` before a stable public release.

M4-4D is commit `8398e49`. It queues load, unload, and replace on the existing command queue. The caller receives a `SceneOpId` immediately. The outcome stays as `SceneOpStatus` until `dismiss_scene_operation` or `clear_scene_operations`. A scene failure is not a `CommandError`.

M4-4E is commit `d58c66b`. It switches `spatial_scene` between environment A and environment B with that queue. Tab is the `switch_environment` action. The camera and the walker are not members of either environment. The example dismisses a completed result in `post_update`. A failed replacement leaves the current environment published. An unexpected load stays published, and an unexpected unload is left as the engine applied it, because that switch does not own the instance.

M4-5, commit `2b79806` and not pushed, adds `spawn_lamp` as the code-defined prefab (`fn(&mut World, ...) -> Entity`) and `place_lamp` for `SceneBuilder`. `create_scene_instance` and `track_scene_entity` are `pub(crate)`.

Still open: which id M19 will persist. D-16 stays open. The file encoding, including whether to take `serde`, is M19's decision. Script flags are stored and unused (D-41). Membership does not own assets. `Transform::from_matrix` drops shear; a non-uniform parent scale with a rotated child is a TRS approximation.

## Needed during M5-M6 (materials, renderer)

### D-27 Units and pixels per unit

Decided in [ADR-0016](adr/0016-per-sprite-pixels-per-unit.md). The per-sprite `pixels_per_unit` stays, default 16. M5 does not add a global constant. The atlas packer works in texels. No code change in M5-1.

### D-05 Materials, alpha modes, blended meshes

Decided in [ADR-0015](adr/0015-material-alpha-classes.md). Opaque, Mask, and Blend are material classes. Blended sprites and blended meshes share one sort and one GPU submission list. A blended mesh uses the camera-space Z of its world-space bounding-sphere centre. That point is an object-level approximation: intersections, cyclic overlaps, and a mesh that spans another depth are not solved. Texture sampling, including the corrected anisotropy, upload alignment, split caches, and the measured atlas layouts, is [ADR-0017](adr/0017-texture-sampling-colour-space-and-atlases.md). Descriptors, GPU formats, mips, rectangle uploads, the CPU shelf packer (`bf4d9bb`), and GPU atlas samples (`1666695`) are implemented. The CPU `MaterialAsset` is commit `e0c29ef`. M5-5B (`3b3cd4a`) draws assigned sprites and assigned opaque or mask meshes. M5-5C (`ab1b6f7`) draws blended meshes in the shared sequence. M5-5D (`a7efd6c`) batches by pipeline, material identity and revision, and texture. One thousand compatible opaque sprites are one instanced draw. Procedural plane, cube, cuboid, sphere, and cylinder meshes are [ADR-0018](adr/0018-procedural-mesh-generators.md), commit `92286ce`. M5 is accepted locally. The acceptance record is in TASKS.md. Localized mip-texel regeneration stays deferred.

### D-06 Pixel-art policy

* **Problem.** A nearest-neighbour image can shimmer when an orthographic camera sits between pixels.
  Integer scaling is the settled D-06B contract. MSAA is the settled D-06C contract in [ADR-0024](adr/0024-optional-4x-msaa.md). The policy close is the 2026-10-26 decision below.
* **Options.** Opt-in snapping per camera; integer-scale viewport mode; MSAA off for pixel-art cameras.
* **Guidance accepted (ADR-0004, 2026-10-13).** Sampling configuration (filter, wrap, anisotropy) belongs at the
  texture or sampler level where appropriate. A pixel-art look may need coordinated settings across materials,
  sprites, cameras and renderer configuration. Texel snapping is implemented at the level its technique needs
  (view-level for camera snapping, object or shader level otherwise). There is no universal style abstraction;
  a convenience preset may configure several settings together while each stays independently settable.
* **Decided for D-06A only ([ADR-0022](adr/0022-orthographic-pixel-snap.md), 2026-10-25, `4a87bb0`).** `Camera.pixel_snap` defaults to `false`. An orthographic camera quantizes its view-space X and Y translation onto that viewport's pixel grid inside `CameraView::new`. The same view is used for rendering, culling, and picking. View-space Z, rotation, the projection, and the entity transform stay. Perspective and `Custom`, including oblique builders, keep the flag and do not snap. Presets do not enable it. Sprites, tiles, meshes, and debug lines share that view. Nearest and linear sampling stay per texture. This is not integer-resolution output and it does not close D-06.
* **Decided for D-06B only ([ADR-0023](adr/0023-integer-resolution-camera-presentation.md), 2026-10-25, `d491bda`).** `Camera.internal_resolution` defaults to `None` and draws directly. `Some` renders the existing scene at that size and presents it with integer nearest scaling inside the destination viewport. Bars use `letterbox_color` (default black). Scale 0 fills the viewport and skips the scene. A zero or oversized size is rejected. `CameraClear::Load` with an internal resolution is omitted and is not drawn as `Color`. A future editor must show that rejection and must not present the skipped camera as a successful composite. Orthographic snap uses the internal height. Letterbox bars own the pixel and return no ray. Private images are renderer resources keyed by the live entity index, not render targets and not a new entity-id system. Presets do not enable it. Apple M5 Metal sRGB presentation matches one encode of the direct path within one channel. Other GPU backends are not verified.
* **Decided for D-06C only ([ADR-0024](adr/0024-optional-4x-msaa.md), 2026-10-26).** `MsaaSamples::One` is the default. `Four` is renderer-wide, checked against the color format and `Depth32Float`, and never reported as active while the frame is one sample. The resolve is a viewport-scoped shader average, not a hardware resolve of the whole attachment. A later direct `Load` copies the resolved color into the four samples of its viewport. Depth presentation shows the nearest reverse-Z sample. Offscreen targets and internal-resolution cameras resolve into their existing one-sample images. A 4× window resolves surface cameras into a private single-sample image and copies it onto a swapchain that is `RENDER_ATTACHMENT` only. The engine must not assume a surface is shader-readable, even when the selected format supports sampling. On Apple M5 Metal the surface omits `TEXTURE_BINDING` while `Bgra8UnormSrgb` format features include it. Headless `Bgra8UnormSrgb` readback of that private-frame copy matched the one-sample image and the IEC sRGB encode. That readback is not the swapchain. `msaa4_camera_skip` remains public and returns `None`. `Load` with an internal resolution stays rejected. `alpha_to_coverage` stays off. Only one and four samples exist. Returning to one sample drops the 4× images and the private frame and keeps the pipelines. This does not close D-06.
* **Still deferred, and not implemented by the close.** Per-object snapping, which remains ADR-0022 option B. Compositing `Load` through an internal image, which ADR-0023 rejects. A small-window policy other than the scale-0 letterbox. Alpha-to-coverage. Sample counts other than one and four.
* **Closed for the approved policy (owner, 2026-10-26).** D-06's rendering policy is those four records: per-texture sampling ([ADR-0017](adr/0017-texture-sampling-colour-space-and-atlases.md)), orthographic camera-view snap (ADR-0022 option A), integer-resolution presentation (ADR-0023), and optional one or four samples (ADR-0024). The deferred list above is an accepted limit of that close. Other GPU backends, a swapchain pixel readback, lost or outdated injection, and GPU pass timestamps are outside this policy close. M6 is not complete.
* **Needed by.** M6, now satisfied for the policy. **Blocks.** Nothing further in M8, M9, or M13 waits on a new D-06 mechanism. Remaining shimmer and tile-seam cases use the deferred techniques above, or the settled opt-in controls.

### D-07 Depth convention (reverse-Z)

* **Problem.** Depth is standard 0..1 with `Depth32Float`. Large perspective ranges lose precision
  far away. Shadow maps (M14) will want a consistent choice.
* **Options.** Keep standard Z; adopt finite reverse-Z in the projection helpers.
* **Decision (2026-10-23, [ADR-0019](adr/0019-reverse-z.md)).** Finite reverse-Z in `perspective_lh` and `orthographic_lh`. NDC 1 at the near plane, 0 at the far plane, `CompareFunction::Greater`, clear 0. No shader complement. `CameraProjection::Custom` is stored as given and is not converted. The M6-3B checkpoint on `spike/m6-3b-d48-coplanar` (`a176502`), Apple M5, Metal, separated a 1/1024 gap at perspective distance 200. The 0-near formula quantized that gap away. Screenshot ties are explained in the ADR. Vulkan, DirectX, OpenGL, and lavapipe are not verified.
* **Needed by.** M6. **Blocks.** Nothing further inside D-07. M14 uses this mapping when shadow maps are built. Coplanar ordering is [ADR-0020](adr/0020-coplanar-priority.md). Gameplay orientation stays open. D-26 is [ADR-0021](adr/0021-sprite-orientation.md).

### D-08 Multi-camera and render targets

* **Problem.** Only one camera was rendered per frame (KI-5), and there was no render-to-texture.
* **Surface contract (approved, implemented in M6-2B, commit `6a47882`).** Every `Camera.active` camera
  with `CameraOutput::Surface` renders to the surface. `Visibility` does not select cameras. Passes sort ascending by
  `(Camera.order, entity id)`. A lower order draws first. A higher order draws later and can overlay
  it. An equal order uses the lower entity id first, so the higher id is on top. Each camera keeps its
  own viewport, projection, view, frustum, billboard and upright basis, visible instances, and
  transparent sequence. The transparent sort is unchanged and runs per camera: `RenderOrder.layer`
  ascending, camera-space depth descending, `RenderOrder.order` ascending, then extraction order.
  `CameraClear::Color` (the default) replaces that camera's viewport with `clear_color` before its
  scene.   `CameraClear::Load` keeps the color already there. The first camera that writes a destination clears
  that whole image to its `clear_color`, including when its policy is `Load`, because that image has no
  defined previous contents. A later `Color` camera on the same destination fills only its rectangle with a
  color-only draw: no depth write, no alpha blend, alpha 1 to match the attachment clear. A later
  `Load` camera does not fill. `LoadOp::Clear` is not used for that later rectangle, because it would
  clear the whole attachment. One full-surface camera stays on that single attachment clear and does
  not draw the fill. Zero active cameras extract nothing. Depth is `Depth32Float`. M6-2B compared less and
  cleared every pass to 1.0. D-07 ([ADR-0019](adr/0019-reverse-z.md), settled 2026-10-23) compares greater and
  clears every pass to 0.0. The clear is discarded, so one camera's depth does not reject the next.
  Opaque and Mask write depth. Blend tests depth and does not write it. That is the M5 alpha contract.
  Globals and instance buffers are uploaded and `queue.submit`ted per camera. A later
  `write_buffer` does not replace bytes an earlier submission already recorded. Texture, sampler, and
  mesh-geometry caches stay shared. Mesh vertices are not copied per camera. Lights are chosen per
  camera because the nearest-16 cap depends on camera position. Ambient and fog stay on the scene.
  `FrameStats` is the frame total. `PassFrameStats` is one camera. A viewport fill is a clear draw,
  not a world draw, and not a submitted instance. An entity drawn by two cameras counts twice.
  `top_surface_camera` returns the last drawn active surface camera whose viewport contains the window
  pixel, or `None`. Offscreen cameras are not candidates. `camera_view_of` resolves one camera by id,
  including an inactive camera, and an offscreen camera uses its target's pixels. Callers subtract
  `viewport_origin` before `ray_from_pixels`. There is no input router.
* **Offscreen contract (implemented locally in M6-2C, committed locally, not pushed).** `RenderTargetId` identities start
  at 1 and are never reused. `RenderTargetStore` is a world resource of plain descriptions. The renderer
  owns the color image, its view, a private `Depth32Float` image, and the sampling bind group. The color
  format is the renderer's existing RGBA8. Depth is not sampleable. A camera's `CameraOutput` is
  `Surface` by default or `Target(id)`. A missing output is skipped and is not redirected to the surface.
  Viewport, aspect, projection, and `CameraView::project` use that destination's size. `Custom` matrices
  stay fixed. `Sprite.render_target` samples the full target in `[0, 1]`, flips included, instead of the
  sprite texture and material albedo. Material class, tint, `base_color`, and revision are unchanged.
  Sampling is valid only after an earlier camera in the same frame has finished writing that id, and a
  camera cannot sample the target it is writing. Otherwise the missing-texture placeholder is bound and
  one diagnostic is recorded for that id. Order is not rearranged. There is no previous-frame fallback.
  Several writers keep their camera order. A resize bumps `generation`, replaces both images and the bind
  group before the passes, and leaves the new pixels undefined until a camera writes them. Unchanged
  targets are not reallocated. `read_render_target` is a test readback. Camera-to-texture does not upload
  the image into an asset. The attachment clear stores alpha 1, and blending over that clear keeps
  framebuffer alpha at 1, so a target is not a transparent-background layer. Sampling uses the default
  nearest, clamp sampler. Each axis is capped at 8192, and a tighter device limit still refuses the
  image. These GPU tests were run on Apple M5, Metal.
* **Not in this decision.** A render graph, automatic dependency scheduling, previous-frame feedback,
  HDR, other color formats, and sampling the depth image. MSAA is proposed separately in [ADR-0024](adr/0024-optional-4x-msaa.md) and is not part of this offscreen contract. Gameplay orientation stays open. D-06 is closed for the approved policy. D-26 is [ADR-0021](adr/0021-sprite-orientation.md). D-07 and the coplanar mechanism ([ADR-0020](adr/0020-coplanar-priority.md)) are decided separately.
  M6 is not complete. Editor viewport UI remains M22. Shadow passes remain M14.
* **Needed by.** M6. **Blocks.** Nothing further inside D-08. Shadow passes (M14) and the editor
  viewport (M22) can sample a target. They are not built here.

### D-25 Custom projections inside `Camera`

* **Problem.** The `Projection` trait exists but `Camera` stores a closed enum (KI-11), so an oblique
  or isometric projection cannot be supplied.
* **Options.** Add enum variants (oblique); store a boxed trait object; store a plain matrix override.
* **Decision (M6-1A approved, M6-1B implemented 2026-10-22).** `CameraProjection::Custom(Mat4)`, constructed
  with `CameraProjection::custom`. The matrix is view-to-clip in the existing left-handed, depth `[0, 1]`
  convention. It is not rebuilt from aspect. `with_clip_range` does not edit it. Non-finite elements are
  `ProjectionError::NonFinite`. Singular matrices render; picking returns `None` when the inverse fails.
  No trait object, no shader branch, no preset. A degenerate frustum plane fails open. A negative scale can
  preserve divided NDC and the corrected CPU frustum without preserving GPU homogeneous clipping. A custom
  matrix is stored as given. Settled D-07 does not convert a 0-near or OpenGL matrix; rebuild it
  with `perspective_lh` or `orthographic_lh`. [ADR-0021](adr/0021-sprite-orientation.md) keeps the camera basis under oblique shear (D-26).
  A coplanar tie stays a tie unless a sprite opts into `CoplanarPriority` ([ADR-0020](adr/0020-coplanar-priority.md)).
* **Presets (ADR-0004).** The original six example constructors, and the M6-1C `custom` constructor, now call
  public constructors (`16ed495`). `PlacedCamera` returns an ordinary `Camera` and `Transform`.
  `CameraProjection::oblique_orthographic` and `oblique_perspective` store `Custom`. They were not part of
  the M6-1B decision. They do not convert a matrix already stored with `custom`, they do not rebuild it when
  the viewport changes, and `with_clip_range` still leaves `Custom` unchanged. An arbitrary matrix is still
  not claimed to follow reverse-Z.
* **Needed by.** M6; earlier only if tilemaps (M13) need oblique projection.
* **Blocks.** Nothing further for supplying a matrix. M13 oblique tiles can call `custom` or
  `oblique_orthographic`. This decision still does not add a projection variant.

### D-26 Additional sprite orientations

* **Problem.** `Billboard`, `Upright`, `Fixed` cover current needs. Axis-aligned billboards or
  camera-position-facing billboards may be wanted. ADR-0004 left `Upright` under steep, top-down, and
  oblique views undecided.
* **Decided ([ADR-0021](adr/0021-sprite-orientation.md), 2026-10-23).** Keep the three modes. `Fixed`
  plus a transform is ground alignment. `Upright` stays world-vertical, foreshortens, and is edge-on
  when the view is exactly `±Y`, with no history and no blend into `Billboard`. `Billboard` is the
  readable overhead and screen-facing policy. Oblique shear does not change the placement basis. A
  `Fixed` child does not inherit a parent's rendered billboard basis. A 2D skeleton, in M11, applies
  one policy at the visual root; bone sprites stay `Fixed`. Extraction was not changed. Tests lock the
  existing math.
* **Needed by.** Nothing further inside D-26. M7 and M8 still wait on gameplay orientation. M11 waits
  on the visual-root frame, which this decision requires and does not implement.

## Needed during M7 (gameplay)

### D-09 Physics: custom or library

* **Settled (2026-10-27, [ADR-0025](adr/0025-kinematic-collision.md)).** Geometric queries come from
  private `parry3d` 0.31.1 (`dim3`, `f32`, `std`, and `alloc` for support-map sweeps). `Collider` on
  the gameplay entity is the data, including the local offset. `CollisionWorld` is a Dimora resource.
  A fixed-step system propagates transforms once, rebuilds a sorted AABB list, and applies a kinematic
  slide that consumes only the untravelled remainder. Sensors use the existing event queues. No Rapier
  dynamics pipeline, rigid body, joint, or gravity solver. Same-process determinism only.
* **Implemented in part.** M7-1A adds the crate to `engine-world` and the geometric query index.
  M7-1A-R stores `mask` on each index entry. Owner approved on 2026-10-27: a nonblocking zero-time
  skin hit is dropped for that sweep and the other candidates are still tested, and `CollisionDebug`
  is opt-in and off by default. M7-1B-1 adds `BVec3` in `engine-math`, persistent `CharacterMotor`,
  runtime `MoveIntent` and `MoveResult`, and the sweep inside `sync_collision`. `MoveResult` reports
  the last resolution. It does not report a grounded state. M7-1B-1G, owner approved 2026-10-27,
  classifies a gap inside `(-1e-4, skin + 1e-4]` with the closest-feature wall and does not trust a
  near-tangent shape-cast normal. M7-1B-2, owner approved 2026-10-28, sends `TriggerEvent::Entered`
  and `TriggerEvent::Exited` on one queue, in canonical `Entity` order. A pass-through is those two
  variants adjacent. A pair crossed more than once in one fixed iteration still produces that one
  outcome. The path is the committed segments, including a recovery push that was written, and not
  the net `MoveResult.applied` vector. M7-1B-2R, owner approved 2026-10-28, makes that sensor rule
  symmetric: a pair blocks only when both colliders are solid. A sensor mover passes through a
  solid without a contact normal and still emits `TriggerEvent` for the committed segment. One
  gameplay entity has one collider. An indexed descendant still rejects the ancestor motor.
  Locomotion is not implemented. M7 is not complete.
* **Needed by.** M7. **Blocks.** M7 and M9 until that implementation lands. The decision itself no
  longer blocks starting M7.

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
* **Constraint found in the code.** M5-3B can upload a proved rectangle and then rebuilds the whole mip
  chain. The CPU shelf packer exists (`bf4d9bb`). The glyph-atlas consumer does not. History older than eight
  region transitions uploads the whole base level. Localized mip-texel regeneration is deferred and has no milestone.
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
  factor). The device scale factor, physical and logical viewport sizes, and logical pointer conversion
  are implemented for M3b in [ADR-0013](adr/0013-input-actions-and-window-controls.md) (checkpoint
  `9c99edf`). winit 0.30.13 dispatches `WindowEvent::ScaleFactorChanged { scale_factor, inner_size_writer }`.
  The handler does not write `inner_size_writer`. Capture, focus navigation, text, and
  reference-resolution fit modes stay open.
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
  The depth convention is settled (D-07). An opted-in sprite's depth pull ([ADR-0020](adr/0020-coplanar-priority.md)) would be what a shadow map sees if it used the same shader. Shadows are not built. Interaction with blended sprites is still open, and casters now
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
* **Leaning.** (a), with (c) considered once there is more than one kind of script. ADR-0014 stores
  per-component and per-field script flags as inert metadata. No script reads them. D-41 stays open.
* **Needed by.** The API surface by M18. The registry shape is settled (ADR-0014). **Blocks.** M18.

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
seam, and internal. M3a-6 reviewed the re-exports after M3a-5 (`523f553`). M3a adds no public `hecs`
type, no new `hecs` re-export, and no `wgpu` or `winit` type on a new public interface. It does not
wrap `hecs`. `CommandWriter` uses the existing `hecs::DynamicBundle` trait. Contract additions are
`Interpolate`, `FixedTimestep::alpha`, commands, events, `Diagnostics` and `StageDurations`.
`snapshot_interpolation` is public on `engine-world` and not in the prelude. Existing `winit` and
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
  `Upright` measurements are the design in [ADR-0021](adr/0021-sprite-orientation.md).
* **Decided for coplanar sprites (2026-10-23, [ADR-0020](adr/0020-coplanar-priority.md)).** `CoplanarPriority(u8)` in `0..=7` on a sprite. No component means ordinary spatial depth. `CoplanarSettings.thickness` is one world-unit step. A missing resource uses `1/256`. The sprite shader writes depth only, `(1 + level) * thickness` toward the camera, and keeps `clip.xy` and `clip.w`. Reverse-Z compare and clear are unchanged. Priority is not a batch key and does not order blend against blend. Meshes and tile maps do not read it. `1/1024` and `1/64` passed the same inside-pull and outside-pull check; `1/64` was not selected because level 7 then pulls 0.125. Verified on Apple M5, Metal. Other backends are not verified by that record. The `spatial_scene` decals still do not opt in during play.
* **Follow-up (2026-10-26).** The three prototype bullets above are the 2026-10-14 Metal observations. They are not the contract. Lavapipe did not reproduce those winners. The configuration tests now check participant-only overlaps, separated depth, and opt-in priority. See [SPATIAL_RENDERING.md](SPATIAL_RENDERING.md).
* **Still open inside D-48.** Gameplay-orientation components, deferred until M7 and M8 have requirements. D-26 is [ADR-0021](adr/0021-sprite-orientation.md).
* **Needed by.** The coplanar mechanism was required before M7 and M8. Those milestones still wait on gameplay orientation. **Blocks.** M7, M8, M13, and M14 still need the gameplay-orientation decision. They can call `CoplanarPriority` and `SpriteOrientation` as they stand.

## Reopening a settled decision

A decision in ARCHITECTURE.md may be reopened only when all of the following hold:

1. A specific failing test, measurement or concrete limitation from the repository is cited.
2. The cost of working around it is stated.
3. The proposed change names what it replaces and which tests must still pass.

"It would be cleaner" or "another engine does it differently" is not sufficient.
