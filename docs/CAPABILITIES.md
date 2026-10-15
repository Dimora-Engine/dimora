# System capability matrix

The single place that records, for every major system, what exists **today** and what is intended. It
replaces the capability-audit table that used to sit in ROADMAP.md; the roadmap now only orders the work.

Audited 2026-10-13 at commit `4857158` (`main`, after M2 tasks T1 to T3) by reading the code and running
`scripts/check.sh`. Keep this file in step with the code: a change that moves a system's status updates
its row in the same commit.

## How to read it

**Status** means what the repository demonstrates, not what exists as a type:

| Status | Meaning |
|---|---|
| **Working** | Implemented, exercised by an example or test, and the tests pass. Limits still listed |
| **Partial** | Part of the capability works and is tested; named parts are missing |
| **Placeholder** | A type or stub exists so other code compiles; it is not the intended design |
| **Absent** | Nothing in the code (a decision may exist) |

*Planned* and *Later* are intent only. A system is never marked Working because its API exists.
`KI-n` is [TASKS.md](../TASKS.md); `D-n` is [DECISIONS.md](DECISIONS.md); `Mn` is [ROADMAP.md](../ROADMAP.md);
`R-n` is [ARCHITECTURE.md](ARCHITECTURE.md#cross-system-requirements-and-risks).

## Verification record (2026-10-13)

| Check | Result |
|---|---|
| `scripts/check.sh` (fmt, clippy `-D warnings`, tests with `DIMORA_REQUIRE_GPU=1`, rustdoc `-D warnings`) | Pass on Linux x86_64, rustc 1.99.0, adapter `llvmpipe (LLVM 20.1.2)` Vulkan software. **138 tests, 0 failed, no GPU test skipped** |
| Same on macOS arm64 (Metal) | Reported in TASKS.md (T1 to T3); not re-run for this audit |
| Tests by crate | `engine-math` 22, `engine-core` 5, `engine-world` 19, `engine-assets` 24 + 1 doctest, `engine-renderer` 2 + 27 (`camera.rs`) + 13 (`spatial.rs`), `engine-renderer-wgpu` 1 + 18 (`headless.rs`: 13 render/cache tests, 5 for the GPU-required switch), `engine-animation` 4, `engine-platform` 1, `engine` 1 doctest. Examples have no tests |
| MSRV | `cargo +1.90.0 test --workspace --locked` on rustc 1.90.0: **138 passed, 0 failed** on x86_64 Linux (local and CI), aarch64 macOS (CI) and x86_64 Windows MSVC (CI). Closes KI-27 |
| Windows | Built and tested for the first time in CI: `scripts/check.sh` passes (138 tests, none skipped) on the WARP software adapter, both examples render headless. Windowed path, real GPUs and DPI are unverified |
| CI | GitHub Actions on the pull-request branch ([run 38475229601](https://github.com/Dimora-Engine/dimora/actions/runs/38475229601)): Linux, macOS, Windows `check` and `msrv` jobs all green. Software or paravirtual adapters only. Active on `main` once merged (ADR-0005, Provisional) |

Code size: about 8,000 lines of Rust in nine library crates and two examples. `unsafe`: none in the
repository. Logging: none (no `log`/`tracing`, no `println!` in library code).

## Summary

Systems are grouped as in the audit request. "Needs by" is the first milestone that cannot be built
without it.

| # | System | Status today | Evidence in one line | Milestone |
|---|---|---|---|---|
| **Core runtime** | | | | |
| 1 | Math, conventions, transforms | Working | 22 tests; `f32`; shear not representable | done |
| 2 | ECS, world, hierarchy | Working, thin | `hecs` wrapper; commands and events included (ADR-0008). No change detection (D-02 provisional, not built) | M3, M4 |
| 3 | Frame loop and scheduling | Working | Named stages, deferred commands, typed events, logging, diagnostics, and opt-in extract-only `Interpolate` (M3a, local, not pushed). `FixedTimestep::alpha` is finite in `0..=1`. The blend does not write `Transform`. The public-API review (M3a-6) added no new `hecs` type. Limits: no pause or time scale, no variable-step mix on a marked entity, no parallel systems | M3 |
| 4 | Events and deferred commands | Working | FIFO commands flush per stage run and per fixed iteration. Typed events are visible to later stages and cleared after `input_clear` (ADR-0008). Reading copies the buffer (`T: Clone`). No observers | M3 |
| 5 | Platform, window, timing | Working, narrow | winit loop, resize, exit, display scale (`9c99edf`), borderless fullscreen, requested versus effective vsync, optional frame cap, cursor visibility and grab (M3b Phase 4, checkpointed locally). Exclusive fullscreen and multi-window are not built | M3, M10 |
| 6 | Input and action mapping | Working for M3b | Fixed-step latch and `Actions` (Phases 1–2). Physical pointer stays physical; logical conversion is on `Viewport` (`9c99edf`). A grabbed cursor takes `mouse_delta` from device motion; a free cursor uses position changes; focus loss releases held input and the grab (ADR-0013). Gamepad, text, and a rebinding UI are deferred | M3 |
| 7 | Errors, logging, diagnostics, config | Partial | `log` 0.4 and `Diagnostics` (ADR-0012, M3a-4). No runtime config. No profiler, no history | M20, M24 |
| **Rendering** | | | | |
| 8 | Cameras and projection | Working; six views demonstrated by the example | Ortho + perspective, viewport, picking rays; 27 camera tests. `spatial_scene` also renders isometric, top-down, steep and side through example-local constructors (not an engine preset API). One camera/frame; closed projection enum; custom projection still absent | M6 |
| 9 | Spatial sprites | Working | Orientation modes, region, tint, cutout/alpha; batching test; no atlas packer | M5 |
| 10 | Depth, transparency, ordering | Partial | One depth buffer shared by meshes and sprites (GPU tests); blended sprites sort by centre; meshes cannot blend. Three coplanar conflicts are verified and unsolved (D-48) | M5, M6, M14 |
| 11 | Meshes and materials | Partial / Placeholder | Indexed instanced meshes; `Material` = colour + albedo + `lit`, not an asset | M5 |
| 12 | Textures, atlases | Partial | PNG; mipless, clamp-only, always sRGB; whole-texture re-upload | M5 |
| 13 | Lighting and fog | Working, forward only | Ambient/directional/point/spot (16), fog; 1 GPU test; no shadows, normal maps | M14 |
| 14 | Shadows | Absent | | M14 |
| 15 | Tilemaps | Working, basic | Expanded to quads every frame; no chunking, collision, loader | M13 |
| 16 | Particles | Absent | | M15 |
| 17 | Parallax | Working (emergent) | Falls out of perspective projection; tested; no per-layer factor by design | done |
| 18 | Renderer resources and performance | Partial | Texture/mesh GPU caches with removal and idle eviction (T2); instance buffers only grow; performance unmeasured | M6 |
| **Characters and animation** | | | | |
| 19 | Sprite-sheet animation | Absent | Only `Sprite.region/size/anchor` building blocks (KI-21) | M8 |
| 20 | Skeletal 2D animation | Partial (data model) | Bones as entities, linear clips, one player; 4 tests; no blending, events, IK | M11 |
| 21 | 3D skinned characters | Absent | `Vertex` has no joints/weights; no importer | M12 |
| 22 | Character gameplay layer | Absent | The demo's own system drives the animation player | M9 |
| **Gameplay** | | | | |
| 23 | Collision and spatial queries | Absent | | M7 |
| 24 | Physics and movement | Absent | Kinematic controller planned; no solver | M7 |
| 25 | Audio | Absent | | M16 |
| **UI** | | | | |
| 26 | UI runtime (screen and world space) | Absent | No fonts, layout, widgets, overlay pass | M10 |
| **Assets and content** | | | | |
| 27 | Asset manager | Partial | Name-hash ids, typed handles, revisions, remove, collision guard (T3); synchronous; 24 tests | M17 |
| 28 | Importers, metadata, hot reload | Absent | PNG decode only; sprites, meshes, skeletons, clips are code-built | M17 |
| 29 | Compiler, bundles, format versioning | Absent | `AssetSource` is the plug-in point | M21 |
| **Scripting and extensibility** | | | | |
| 30 | Scripting | Absent | No runtime, registry, commands or events | M18 |
| 31 | Plugins and native extensions | Absent, not planned | Rust crates are the extension mechanism | none |
| **Scenes, tools, production** | | | | |
| 32 | Scenes and lifecycle | Absent | World is flat; `Name`, `Visibility` only | M4 |
| 33 | Serialization (scenes) | Absent | No `serde`; no component registry | M19 |
| 34 | Save games and persistent state | Absent, unscheduled | Distinct from scene files | add (M19+) |
| 35 | Debugging and developer tooling | Partial | `Diagnostics` and `log` (M3a-4). Examples print entity count, fixed steps, and CPU step time in the window title. No inspector, debug draw, or overlay | M4, M6, M10, M20 |
| 36 | Editor and inspection (core goal) | Absent | Foundations (registry, commands, ids) not yet shaped for it, R-15 | M22 |
| 37 | Build, CI, packaging | Partial | Pinned toolchain, `check.sh`; no CI, no licence files, no release | M2, M24 |
| 38 | Docs, examples, tests, onboarding | Partial | Accurate README and docs, 2 examples, 138 tests; no rustdoc guide, no tutorial | M24 |

Missing from the original category list and now tracked: **save games** (34), **diagnostics and logging**
(7), **determinism/replay**, **gamepad and text input**, **camera behaviours** (follow, shake),
**tweening/timelines**, **AI and pathfinding**, **localisation and accessibility**, **crash reporting**,
**dependency and licence policy**. They are in "Additional categories" at the end.

---

## Core runtime

### 1. Math, conventions, transforms
* **Purpose.** One numeric and spatial vocabulary for every system.
* **Now.** `Vec2/3/4`, `Quat`, `Mat3/4`, `Rect`, `Transform` (TRS), `f32`, column-major, left-handed, +Y up,
  +Z away from the default camera, NDC depth 0..1. 22 tests. Conventions are specified in
  [SPATIAL_RENDERING.md](SPATIAL_RENDERING.md).
* **Required.** Exact composition, inverse, look-at, decomposition; documented handedness conversion at
  import boundaries.
* **Planned.** Shear-safe handling (see risks); a unit/pixels-per-unit convention (D-27); angle and
  easing utilities as gameplay needs them.
* **Depends on.** Nothing. Everything depends on it.
* **Integration.** Imports must convert handedness (glTF, D-36). Serialization stores TRS, not matrices.
* **Performance.** Plain `Copy` types; no SIMD (measure before adding).
* **Tests/examples.** Existing unit tests; add property tests for composition if shear work starts.
* **Unresolved.** Non-uniform scale under a rotated parent shears; `Transform::inverse` exact for uniform
  scale only; f32 precision in very large maps (R-9).
* **Not initially.** SIMD, `f64`, dual quaternions, a units library.

### 2. ECS, world, hierarchy
* **Purpose.** Entities, components, resources, parent/child transforms.
* **Now.** `World` wraps `hecs` plus typed resources; `Parent`/`Children`; `GlobalTransform` via a full
  `propagate_transforms` pass; `despawn` removes the subtree and `detach_children` keeps children (D-03);
  transform-less entities are identity (T3). 19 tests. **Weak point:** `World.entities` is a public
  `hecs::World`, and `get`/`query` return hecs types, so storage is *not* swappable despite the earlier
  claim (R-7).
* **Required.** Safe hierarchy, deterministic iteration, entity references that survive serialization
  and scripting (generation-checked).
* **Planned.** Hierarchical visibility (KI-18), name/tag lookup, scene membership (M4), dirty
  transforms only if profiling asks (D-02), component registry (D-18).
* **Depends on.** Math. **Integration.** Everything is an entity; extraction, animation, UI anchors,
  colliders, scripts and the editor all read it.
* **Performance.** `propagate_transforms` is O(entities) every frame; extraction walks all renderables.
  Unmeasured (KI-19).
* **Tests/examples.** Hierarchy invariants test (every `Parent` valid), 10k-entity propagation timing (M6).
* **Unresolved.** D-18 (registry). D-02 is provisional (ADR-0009: none until a profile). D-45 is decided (ADR-0010): three tiers, no `hecs` wrap in M3a, the public list is still M9.
* **Not initially.** Parallel systems, reactive change detection, archetype-level APIs.

### 3. Frame loop and scheduling
* **Purpose.** A defined, documented frame so systems can depend on stage order.
* **Now.** `Game::step` runs `time`, `loads`, `fixed_update` (0 to 8 times), `update`, `animate`,
  `post_update`, `input_clear`. `before` / `after` order systems inside a stage; unconstrained systems
  keep registration order. `advance_time` and `process_loads` stay first; `propagate_transforms` and
  `end_input_frame` stay last; `animate` accepts an explicit system on either side. During each fixed
  iteration `Time::delta` is the fixed step, then the frame delta is restored by `step` itself.
  At the start of each fixed iteration, before user systems, `Interpolate::previous` is copied from
  the current local. `elapsed` and `frame` advance once per step. Frame input edges last until `input_clear`.
  Fixed-step edges are concealed after the first fixed iteration (ADR-0013).
  Extract and render stay outside `step`. `FixedTimestep::alpha()` is the render blend factor, finite
  and in `0.0..=1.0`. Both examples stay on `add_system` (`update`) and do not opt in.
* **Interpolation.** Opt-in, extraction only (ADR-0007, M3a-5). `Transform::lerp` blends the stored
  previous local toward the current one. The render world matrix is the product of those locals.
  `Transform` and `GlobalTransform` stay the simulation. Fixed and variable systems read those.
  An unmarked entity, including every entity in the current examples, extracts from `GlobalTransform`.
  A camera interpolates only when it or an ancestor has a stored previous local. Teleport is
  `Interpolate::snap`. `detach_children` stores the rewritten world local as that child's previous
  local; descendants keep their own history. `set_parent` and removing `Parent` leave both samples
  unchanged, so the entity pops instead of sliding between world poses. Not supported: variable-step
  writes of a marked local (the blend includes them; fixed and variable systems still read
  `Transform`), rotation exactly opposite (`Quat::slerp` is unstable there), scale sign changes
  (they pass through zero), shear, and skeletal or physics interpolation.
* **Required.** A headless `step` exists. M3a-6 reviewed the re-exports (ADR-0010). The frame-model
  slice is complete locally and is not pushed. M3b Phases 1–3 are checkpointed locally (`57dbc4a`,
  `83286a9`, `9c99edf`). Phase 4 (window controls) is checkpointed locally. M3b is not pushed.
* **Planned.** Nothing further in M3a. **Depends on.** D-01 (implemented), D-02, D-10, D-44.
  **Integration.** Collision (fixed stage), animation (animate stage, camera read from previous
  propagation, D-31), UI (its own stage), scripts (one stage), scenes (apply at a boundary).
* **Performance.** Sequential is the rule (ADR-0006) until a measured gameplay workload cannot meet its budget on one thread. The render-matrix walk runs only when some entity has a stored previous local.
* **Tests/examples.** `engine-world` schedule tests and `engine` `tests/frame.rs` cover order, cycles,
  the lifecycle anchors, the fixed delta and input edges. `engine-renderer/tests/interpolation.rs` and
  `engine/tests/interpolation.rs` cover alpha, the blend, hierarchy, teleport, lifecycle, the 60 Hz
  versus 144 Hz sample, and a GPU sprite/mesh frame. The examples do not enable interpolation.
* **Decided, not built.** D-02 (ADR-0009, provisional). D-44 is settled and matches the code.
  **Not initially.** Parallel execution, a dependency-graph scheduler, skeletal interpolation.

### 4. Events and deferred commands
* **Purpose.** Communicate between systems without borrowing each other's state; mutate safely mid-stage.
* **Now.** `World::commands` queues `spawn`, `despawn`, `insert`, `remove`, `set_parent` and
  `detach_children`. The queue flushes in call order at the end of each stage run and between fixed
  iterations, through the existing world methods. A failed command writes nothing, is recorded on
  `command_failures`, and does not stop the queue. `spawn` reserves an id immediately; queries see it
  only after the flush. Inserting `Parent` or `Children` in a bundle fails; hierarchy edits use
  `set_parent`, `detach_children` and `remove`. `World::send` / `World::events` are typed queues.
  An event is invisible inside the sending stage, visible to later stages of the same `step`, and
  cleared after `input_clear`. A send in `input_clear` is not delivered next frame;
  `discarded_events` counts it until the next step. Fixed iterations do not see events sent in
  `fixed_update`. `events` returns a copy, so two systems can read the same events. `commands()`
  does not hold the queue borrow, so a second writer in the same system is safe. `try_commands()`
  returns `None` when the queue is already borrowed.   `command_failures()` keeps each `CommandError`
  with the stage that flushed it, until the next step. `Game::step` logs that list once at the end
  of the step. A double borrow of one resource still
  panics (KI-9). There is no observer list.
* **Required.** Met by ADR-0008 for the command set and the event lifetime. Collision, animation and
  UI events are later consumers, not part of this queue.
* **Depends on.** D-01. **Integration.** Collision events, animation events, scene load/unload, asset
  failure, UI callbacks, script mutations. The scripting boundary requires it.
* **Tests/examples.** `engine-world` command and event tests, and `engine` `tests/deferred.rs`: queue
  during a query, stage-boundary visibility, FIFO, hierarchy rejection, fixed-iteration flush, same-stage
  and cross-stage events, frame clearing, and a direct `spawn` that stays immediate.
* **Not initially.** Observers/reactive triggers, event persistence, networking. Change detection (D-02).

### 5. Platform, window, timing
* **Now.** winit 0.30.13 loop, resize, exit, and title. `Time` and `FixedTimestep` are tested. The
  window scale factor is forwarded (M3b Phase 3, `9c99edf`). `Viewport` stores physical pixels, a
  finite positive scale, and logical size (`physical / scale`). A scale event does not resize; the
  following `Resized` stores the physical size. A zero resize keeps the last positive framebuffer.
  Single window. Phase 4, checkpointed locally, adds borderless fullscreen on the
  current monitor, a vsync request whose effective mode is chosen from the surface's listed present
  modes, an optional frame cap via `ControlFlow::WaitUntil`, and cursor visibility plus grab
  (locked, then confined). The previous windowed logical size is restored on leaving fullscreen.
  `WindowControl` is the request. `WindowState` is what the runner last applied, including
  `present_mode` (`None` until a surface exists) and `cursor_grab`. A requested cap is a scheduling
  target. `Game::step` still receives the redraw delta. Uncapped frames still use `ControlFlow::Poll`.
  Exclusive fullscreen, display-mode selection, and multi-window are not built.
* **Required (desktop).** Monitor enumeration beyond the current monitor remains. DPI scale and its
  changes are implemented for the single window. Borderless fullscreen, vsync, the frame cap, cursor
  visibility, and cursor grab are implemented.
* **Planned.** M10 for UI. **Depends on.** Nothing.
  **Integration.** UI maps a physical pointer through `Viewport::physical_to_logical`. Reference-resolution
  fit modes stay in D-39.
* **Performance.** An uncapped loop still polls. The optional cap waits until a deadline instead of
  spinning. It does not pace against the display blank; vsync can still hold a frame for one refresh.
* **Tests/examples.** `Viewport` size, scale, zero resize, and pointer conversion are pure tests
  (`crates/engine/src/viewport.rs`, `crates/engine/tests/viewport.rs`). Present-mode selection, the
  fullscreen toggle, the frame-cap deadline, and cursor-grab fallback are pure tests. A real window
  is a manual check, per OS. Phase 4's manual check is macOS only.
* **Unresolved.** D-39 (UI capture, focus navigation, text, and reference-resolution fit modes), D-23 (platforms).
  **Not initially.** Multi-window, exclusive fullscreen, IME, touch, mobile lifecycle.

### 6. Input and action mapping
* **Now.** Physical keys through F12, US punctuation, navigation, and the numeric keypad (`NumpadEnter`
  distinct from `Enter`; F13 and above unmapped). Mouse buttons, position, delta, and vertical scroll.
  Key and mouse press and release edges last one frame. `axis()` cancels. Left and right modifiers
  share one variant. The platform keeps that logical key down until the last physical side releases.
  OS key repeat is dropped. `Actions` maps `&'static str` names to keys and mouse buttons: several
  bindings, digital buttons, 1D axes, `axis2`, and `set_button` / `set_axis` rebinding. An action
  press is the aggregate going down, not the OR of the physical edges. Frame action edges clear in
  `input_clear`. Fixed-step edges on `Input` and `Actions` are published at the start of `step` and
  concealed after the first fixed iteration, so a later iteration does not see them. A step with no
  fixed iteration leaves them published. Queries read that published set from any stage. Held state
  stays live. Phases 1 and 2 are checkpointed locally and are not pushed (ADR-0013). `mouse_position`
  stays in physical pixels. Logical coordinates are `Viewport::physical_to_logical`, top-left, +X
  right, +Y down, with no Y flip and no clamp.   Phase 3 is checkpointed locally as `9c99edf`.
  Phase 4, checkpointed locally, selects one `mouse_delta` source: device motion while the cursor is
  grabbed (locked or confined), and cursor-position changes while it is free. A grab transition
  swallows one position delta. Focus loss releases every held key and mouse button, clears
  `mouse_delta`, releases the grab, and restores the requested cursor visibility. It does not grab
  again when focus returns. Collapsed Shift, Control, and Alt drop both physical sides, so a logical
  modifier is not left down. Device-motion units are winit's; this is not unaccelerated HID input.
  macOS rejects confined grab. X11 rejects locked grab. A failed grab is recorded and does not panic.
* **Required.** An input record/replay format (M9), UI input capture before gameplay.
* **Planned.** Gamepad after M9 (unscheduled device layer; actions must not assume keyboard);
  text input only with UI text fields. **Depends on.** D-10 (events).
* **Integration.** UI focus navigation and scripts use **actions**, never raw devices.
* **Tests/examples.** Latch and action tests (aggregate press, rebinding, zero and many fixed steps);
  `spatial_scene` on actions, `basic_2_5d` on raw `Input`.
* **Not initially.** Gamepad, touch, IME, rebinding UI, per-device profiles.

### 7. Errors, logging, diagnostics, configuration
* **Now.** Crates return typed errors (`AssetError`, `HierarchyError`, `RenderError`, `CommandError`,
  `PlatformError`). The `log` crate (0.4, ADR-0012) is the logger. `Game::run` installs a stderr logger
  at Info only when no logger is installed, and does not change an existing filter. A failed asset load
  logs one warning naming the asset when the load becomes failed; a later `process_loads` does not repeat
  it; a new failing load does. A load that is still pending does not log. Extraction skips a sprite, mesh,
  or tilemap whose asset is missing, failed, or invalid, counts it on `ExtractStats.missing_assets`, and
  logs one warning per asset id. It remembers at most 64 ids and does not rotate that memory. A loaded
  asset is forgotten, so a later miss logs again. A lost or outdated surface logs one warning and the
  renderer reconfigures. Window initialization and render errors log the cause and the runner still exits.
  Timeout and occlusion are not logged. Each new `CommandFailure` is logged once at the end of `step` and
  stays readable until the next step. `Diagnostics` holds the simulation frame delta, CPU time for each
  stage (the fixed-update field is the whole loop), the fixed-step count, the entity count, the command
  failure count, the discarded-event count, and the latest `RenderStats`. `step` does not replace the
  render statistics. There is no sample history and no per-system timing. No runtime configuration file.
* **Required.** Errors that carry positions, an in-game surface (M20), a panic hook that writes a log
  file (M24), explicit config. Profiling stays in M20.
* **Planned.** M20 may add `tracing` or another profiler without replacing these records. **Depends on.**
  D-21, decided for logging and still open for profiling.
* **Integration.** Script errors (M18) use the same crate. Replays and CI are later.
* **Tests/examples.** Failed-load warning, pending-load silence, missing-renderable dedup, surface-loss
  warning, render and window errors, command-failure log, stage timings, fixed-step count, render stats
  after `render_to_rgba`, discarded events with no log line, and an application logger left in place.
  Both examples include entity count, fixed steps, and CPU step time in the window title.
* **Not initially.** Remote telemetry, metrics backends, crash upload, per-system profiles, GPU timestamps.

---

## Rendering

### 8. Cameras and projection
* **Now.** `Camera` component (`Orthographic`, `Perspective`, normalised viewport, clear colour, order),
  `CameraView` (view, projection, viewport pixels, basis), `Projection` trait, picking rays, frustum.
  27 camera tests including parallax. Exactly one camera renders per frame (the lowest `(order, id)`
  active one); `Camera` holds the closed enum, not the trait (KI-5, KI-11).
* **Camera configurations (ADR-0004; intended capabilities, status stated plainly).**

  | Configuration | Status today |
  |---|---|
  | Perspective, orthographic | Implemented and tested (camera maths, GPU tests, `spatial_scene`) |
  | Isometric (orthographic, rotated) | **Demonstrated** by an example-local constructor: orthographic, yaw 45°, pitch `atan(1/sqrt(2))`. Not a projection type and not an engine preset |
  | Top-down | **Demonstrated** (orthographic, looking along −Y). `Upright` quads are edge-on; that behaviour is observed, not accepted as the final design (D-26) |
  | Steep (about 60°) | **Demonstrated** (perspective). `Upright` is foreshortened to roughly half a billboard. Same caveat (D-26) |
  | Side view | **Demonstrated** (orthographic, looking along +Z). Not a 2D-style game: no gameplay movement plane |
  | Custom projection | `Projection` trait exists; `Camera` stores a closed enum, so game code cannot supply one (KI-11, D-25) |

  The six rows above share one world, one extraction path and one wgpu backend. What each one guarantees is
  tabulated in [SPATIAL_RENDERING.md](SPATIAL_RENDERING.md). Constructors live in
  `examples/spatial_scene/src/cameras.rs` and return a `Camera` plus a `Transform`. **There is no engine-level
  preset API.** Signatures and default angles stay deferred (ADR-0004, with D-25).
* **Presets and style policy (ADR-0004; accepted 2026-10-13).** Camera presets will be constructors
  over the one camera model and stay editable; style policies sit at the object, material, camera, renderer or
  scene level with no universal style type. Today: sprite orientation is per `Sprite`, filtering per `Texture`.
  The example's six constructors are **not** that API: nothing in an engine crate builds a preset, and there is
  no camera-level or renderer-level style policy.
* **Required.** Multiple cameras with viewports, camera-to-texture, extensible projection (oblique,
  custom), resize safety, and the same behaviour of picking, culling, depth and lighting in every
  configuration. **Planned.** The rest of M6. The configuration test scene is done (below). **Depends on.**
  D-25, D-08, D-48, D-07; D-06 is opt-in and not a dependency of other views.
* **Integration.** UI world anchors, picking, audio listener, directional frames (D-31) and the editor
  viewport all read the active camera's view.
* **Performance.** Per-camera extraction multiplies CPU cost; cull per camera.
* **Tests/examples.** `examples/spatial_scene/tests/configurations.rs` (16 tests, 2026-10-14): the same world
  under all six configurations, project-then-pick, mesh/sprite occlusion both ways, orientation, and the three
  coplanar conflicts. Two viewports in one target, and picture-in-picture, are still to do.
* **Not initially.** Cameras as render graph nodes, stereo, post-processing.

### 9. Spatial sprites
* **Now.** `Sprite` + `Transform`: world-space quad, `Billboard`/`Upright`/`Fixed`, region, size,
  anchor, flips, tint, cutout or alpha blend, lit flag; extracted, culled by bounding sphere, sorted and
  batched by texture (5,000 sprites one draw in a test).
* **Required.** Atlases, sheets, trim/pivot handling, pixel-art policy, consistent size units.
* **Planned.** Atlas builder and sub-region update (M5), sheets (M8). **Depends on.** D-27, D-06.
* **Integration.** Sprites are bones' children, tiles' sibling path, particles' target path.
* **Performance.** One instanced quad path; CPU extraction per frame.
* **Tests/examples.** Existing; add table tests for sheet slicing (M8).
* **Unresolved.** KI-21 (natural size ignores region). D-26: `Upright` is observed, not settled. In the test
  scene an orthographic `Upright` quad is `cos(pitch)` as tall as a billboard, a straight-down view makes it
  edge-on, and a perspective view at about 60° leaves it roughly half as tall. That is what extraction does
  today. It is not accepted as the final meaning for steep or oblique views. The three modes stay a candidate
  set, not an exhaustive one.
* **Not initially.** Sprite meshes (non-quad), 9-slice (UI), per-sprite shaders.

### 10. Depth, transparency, occlusion, render ordering
* **Now.** One `Depth32Float` buffer, standard Z (`Less`). Order inside the single pass: opaque meshes,
  cutout sprites and tiles (depth write), then alpha sprites back to front (depth test, no write).
  `RenderOrder { layer, order }` overrides blended sprite order only. By decision (ADR-0004, amended 2026-10-13) spatial depth, transparency sorting, render layers or queues and explicit order are distinct concepts. A deliberate-ordering mechanism for coplanar opaque and cutout content is **not implemented and not chosen**. The M6 scene verified three limits, each pinned by a test in `configurations.rs` (D-48): equal-height cutout decals follow spawn order (`equal_height_cutout_decals_are_decided_by_draw_order_not_by_space`); a cutout decal coplanar with a slab is shown or hidden depending on the camera (`known_conflict_a_decal_coplanar_with_a_mesh_is_hidden_or_shown_by_rounding`); a blended decal at the same height as a cutout one is likewise camera-dependent (`known_conflict_a_blended_decal_at_the_same_height_as_a_cutout_one_depends_on_the_camera`). GPU tests: nearer mesh occludes a
  sprite, farther does not, under all six example configurations; blended sprite interleaves with meshes by depth in the single-camera test (not re-run per configuration). Limits: blended sprites sort
  by quad centre (KI-14); **meshes cannot be blended**; `RenderScene` is partitioned by primitive type
  (meshes, cutout, blended sprites), not by transparency class, so a blended mesh has no place to sort
  against blended sprites (R-1).
* **Required.** One documented rule for opaque, cutout and blended content from sprites, meshes, tiles,
  particles and skinned characters; defined behaviour when blended objects intersect.
* **Planned.** Alpha modes (M5, D-05), reverse-Z only if an artefact is shown (D-07). **Depends on.** D-05.
* **Integration.** Shadows, particles and skinned meshes all pick a class; UI is outside this model.
* **Tests/examples.** A blended mesh in front of and behind a blended sprite (GPU test).
* **Not initially.** Order-independent transparency, per-pixel sorting.

### 11. Meshes and materials
* **Now.** `MeshData` (position, normal, uv, `u32` indices), `Mesh` + `Material` component (colour, one
  albedo texture, `lit`), `cuboid` and `cone` generators, back-face culled, instanced. Code-built only;
  no loader. `Material` is a component field, not an asset.
* **Required.** Material assets with alpha modes and a normal-map slot, more generators, tangents, a
  loader, skin data later. **Planned.** M5, M12. **Depends on.** D-05, D-36, D-17.
* **Integration.** Sprites and meshes should share a material concept where sensible but need not share
  a pipeline (D-05, D-13).
* **Tests/examples.** Pixel tests per alpha mode; glTF fixtures (M12).
* **Not initially.** PBR, custom shaders, texture compression, LODs.

### 12. Textures and atlases
* **Now.** PNG decode via `image`; nearest or linear; clamp; no mipmaps; always sRGB; any change
  re-uploads the whole texture (KI-6, KI-25).
* **Required.** Mipmaps, wrap/filter/anisotropy, colour-space flag, sub-region update, runtime atlas
  packer with padding. **Planned.** M5. **Depends on.** D-27, D-06.
* **Integration.** The glyph atlas (M10), sheets (M8), normal maps (M14), render targets (M6).
* **Tests/examples.** GPU tests in M5's acceptance list.
* **Not initially.** Texture streaming, compression, virtual textures.

### 13. Lighting and fog
* **Now.** Ambient plus up to 16 directional/point/spot lights, forward world-space `shade()` shared by
  sprites, tiles and meshes, wrap lighting for sprites, exponential distance fog. 1 GPU test
  (falloff) and extraction tests. No shadows, normal maps, or hooks for them.
* **Planned.** Normal-mapped sprites, light culling, layers (M14). **Depends on.** D-13, M5.
* **Performance.** 16 lights per pixel; no culling.
* **Not initially.** GI, PBR, baked lighting, volumetrics, post-processing.

### 14. Shadows
* **Now.** Absent. **Planned.** M14 after all character types exist so casters include animated sprite
  frames and skinned meshes. **Depends on.** D-14, D-07, D-08 (shadow passes need extra passes/targets).
* **Not initially.** Soft shadow filtering beyond basic PCF, cascades.

### 15. Tilemaps
* **Now.** `TileMap` component expanded to cutout quads on every extraction (80x120 in the demo); one
  extraction test. No chunking, flip flags, layers, animation, collision, loader.
* **Planned.** M13: chunked storage, layers, collision layer. **Depends on.** M6, M7, M5.
* **Integration.** Tiles live in world space (a map is an entity with a `Transform`), so physics and
  queries use world coordinates and a map can be rotated or stacked at depth.
* **Performance.** Per-frame cost must be proportional to visible chunks, not tiles (M13 criterion).
* **Not initially.** Hex/isometric grids, auto-tiling, infinite maps.

### 16. Particles
* **Now.** Absent. **Planned.** M15, CPU simulation, instanced through the sprite path, seeded
  deterministic. **Depends on.** M3, M5, M6, D-15. Needs a transparency class (R-1).

### 17. Parallax
* **Now.** Emerges from perspective projection of real depth; tested (`parallax_emerges_from_perspective_depth_alone`,
  `orthographic_has_no_parallax`, parallax screenshot). No per-layer scroll factor, deliberately.
* **Not initially.** A "parallax layer" component.

### 18. Renderer resources, limits and performance
* **Now.** GPU texture and mesh caches keyed by asset id and revision; evicted when the asset is removed
  or idle for N frames (default 120) with counts exposed (`CacheStats`); instance buffers grow and never
  shrink; surface loss and resize handled; frame stats. **No performance measurements on real hardware**;
  device limits are not queried (e.g. max texture size, instance buffer limits); no GPU timing.
* **Planned.** Baseline and `docs/PERFORMANCE.md` (M6). **Depends on.** D-07, D-08.
* **Tests/examples.** Cache-count tests exist; benchmark binary (M6).
* **Unresolved.** Device-limit policy; device-lost recovery (not implemented).

---

## Characters and animation

### 19. Sprite-sheet animation
* **Now.** Absent. Building blocks: `Sprite.region/size/anchor`, `SpriteAsset` uv/pivot. A sprite's natural
  size ignores `region` (KI-21).
* **Required.** Sheet and frame data, clips with loop/once/ping-pong, events fired once per crossing,
  named transitions with parameters, directional sets relative to the camera, one draw per sheet.
* **Planned.** M8 (parallel with M7). **Depends on.** M3, M5, D-27, D-29, D-30, D-31, D-06.
* **Integration.** Writes `Sprite` on the visual child of a gameplay entity; reads facing and camera
  yaw; emits events via M3 channels. Must not import skeleton code.
* **Performance.** Thousands of animators in the animate stage with no per-frame allocation.
* **Tests/examples.** Table tests for slicing and frame sequences; determinism at 30 and 144 Hz;
  walker in `spatial_scene`.
* **Not initially.** Frame blending, palette swaps, Aseprite import (M17).

### 20. Skeletal 2D animation
* **Now.** `Skeleton` (bone hierarchy with rest transforms), bones as child entities, `AnimationClip`
  (TRS keyframes, linear, looping), `AnimationPlayer`, `animate_system` writing straight onto bone
  entities, used by the `basic_2_5d` character; 4 tests. No blending, events, IK, constraints, skins,
  file loading. `animate_system` only visits entities with a `SkeletonInstance` (KI-22).
* **Planned.** M11: pose-first evaluation (D-34), blending, state machine, IK, sockets, skins.
  **Depends on.** M3, M5, M9, D-12, D-29, D-32, D-34.
* **Integration.** Parts are sprites in the world; sockets feed equipment and particles.
* **Performance.** Entity-per-bone makes propagation cost grow with bones (D-33).
* **Not initially.** Mesh deformation (D-12), Spine import, ragdoll, retargeting.

### 21. 3D skinned characters
* **Now.** Absent (KI-23). Rigid mesh children of bones work as attachment only.
* **Planned.** M12: skinned vertex layout, skin asset, GPU skinning with a joint palette in `RenderScene`
  (D-35), minimal glTF importer with a tested handedness rule (D-36). **Depends on.** M5, M6, M9, M11.
* **Integration.** Same `Transform`, hierarchy, collider and gameplay entity as other characters.
* **Not initially.** Morph targets, cloth, LODs, retargeting.

### 22. Character gameplay layer and customisation
* **Now.** Absent. In `basic_2_5d` the example's movement system calls `AnimationPlayer::play`
  directly (gameplay and one visual representation are mixed).
* **Planned.** M9 (D-28): gameplay entity with a replaceable visual subtree, one-way data flow, events
  back; M11 adds equipment slots, skins and part swapping.
* **Tests/examples.** The swap-trace test (identical position and collision trace across visuals) and a
  scan that gameplay code never names rendering types (M9).
* **Not initially.** Root motion (open), a general character editor.

---

## Gameplay

### 23. Collision and spatial queries
* **Now.** Absent. **Planned.** M7: 3D colliders (sphere/box/capsule, layers), a `CollisionWorld`
  resource rebuilt in the fixed stage, raycast and overlap, triggers via events, debug draw.
  **Depends on.** M3, M4, M6 (debug draw), D-09, D-10.
* **Integration.** World-space 3D, attached to the **gameplay** entity; tilemap collision layer (M13)
  and foot placement (M12) feed it; scripts and UI hit-testing may use queries.
* **Performance.** Broadphase must scale with entity count; deterministic across runs.
* **Not initially.** Rigid bodies, joints, continuous collision for dynamic bodies, convex colliders, navmesh.

### 24. Physics and movement
* **Now.** Absent. **Planned.** Kinematic move-and-slide with swept tests in M7. A physics library is an
  open question (D-09); the leaning is a small custom layer.
* **Not initially.** Rigid-body dynamics, joints, ragdolls, vehicles.

### 25. Audio
* **Now.** Absent. **Planned.** M16 behind a backend interface with a null backend; spatial audio from the
  listener's real transform; buses; decoders per D-17. **Depends on.** D-11, M3, M7, M8.
* **Integration.** Triggered by animation and collision events and by scripts.
* **Not initially.** Reverb/effects, HRTF, streamed music with seeking.

---

## UI

### 26. UI runtime (screen-space and world-space)
* **Now.** Absent. No font loading, text, layout, widgets, focus or overlay pass; `RenderScene` is
  world-only. The device scale and logical pointer conversion exist (M3b Phase 3, `9c99edf`). Whole-texture
  re-upload still blocks a glyph atlas (KI-25). Reference-resolution UI scaling is not built (D-39).
* **Planned.** M10: a retained node tree with pure-function layout, a plain-data draw list, an overlay
  pass (no depth, painter's order, scissor), shared device/texture cache/atlases, world-anchored UI by
  projecting an entity through the active camera, input captured before gameplay. Runtime UI is
  separate from editor tooling.
* **Depends on.** M3, M5, M6, M9; D-24, D-37, D-38, D-39. **Performance.** Draw list for 500 widgets
  within a recorded budget.
* **Tests/examples.** Layout golden tests, interaction tests without a GPU, pixel tests at scale 1 and 2.
* **Not initially.** Visual UI editor, rich text, shaping/bidi, IME, localisation, theming, accessibility.

---

## Assets and content pipeline

### 27. Asset manager
* **Now.** `AssetId` = 64-bit FNV-1a of the logical name (collision between different names of one
  type is an error, T3); typed copyable `AssetHandle<T>` that components can create before the asset
  exists; per-type stores with state, revision (never reused), dependency ids and error string;
  `insert`/`load`/`reload`/`remove`; filesystem (rejects `..`) and memory sources; synchronous
  `process_loads` per frame. 24 tests. `Asset` requires `Send + Sync`.
* **Required.** Stable ids that survive renames, dependency-driven reload and unload, errors with
  context, retention rules (who keeps an asset alive: nothing today; handles are not reference counted).
* **Planned.** M17 (D-16). **Depends on.** D-16, D-17, D-18.
* **Integration.** Components hold handles built from names, so a serialized scene must persist names or
  ids and a reverse lookup does not exist (R-4).
* **Performance.** Synchronous decode on the main thread; worker-pool loading in M17.
* **Not initially.** Streaming, compression, remote assets.

### 28. Importers, metadata, validation, hot reload
* **Now.** PNG only. No metadata files, validation, watcher.
* **Planned.** M17: importers (glTF, sheet descriptors, fonts, audio, Tiled, skeleton/clip format),
  file watcher, dependency graph, async loading, placeholder plus logged error. **Depends on.** M5 to M13
  data shapes; D-16, D-17.
* **Not initially.** DCC plugins, a GUI importer.

### 29. Compiler, bundles, format versioning
* **Now.** Absent. **Planned.** M21: `dimora-assets` CLI, deterministic bundle with index and content
  hashes read through `AssetSource`, incremental builds, validation. **Depends on.** M17, M19, D-19.
* **Decided, not built.** Every persisted format carries a version from its first line (ADR-0011). No format exists yet.

---

## Scripting and extensibility

### 30. Scripting
* **Now.** Absent. Prerequisites are absent too: component registry, commands, events, `serde`.
* **Planned.** M18, one runtime chosen after a measured spike among Lua (`mlua`), Rhai (and WebAssembly only
  for untrusted code); a hand-written versioned script API over opaque entity handles; commands applied at
  stage boundaries; sandbox with budgets; errors that name script and line; hot reload.
  The comparison in D-40 is general knowledge and **has not been measured**; the leaning (Lua, Rhai as
  fallback) is not a decision.
* **Depends on.** M3, M4, M9, M10, M16, M17; D-40 to D-43. **Not initially.** A second language, visual
  scripting, debugger protocol, mod distribution.

### 31. Plugins and native extensions
* **Now.** Absent and **not planned.** A game extends the engine by depending on its crates; Rust has no
  stable ABI, so dynamic native plugins would need a C-style API and a versioning scheme the project
  cannot afford early. Revisit only with a concrete use (mods, editor tools) and an ADR. Scripts are the
  intended extension path for behaviour.

---

## Scenes, tools, production

### 32. Scenes and lifecycle
* **Now.** Absent. The world is flat. Entities are created in code; there is no scene unit, no
  load/unload, no prefab convention; `Visibility::Hidden` does not hide children (KI-18).
* **Planned.** M4: scene membership and manager (applied at a stage boundary), hierarchical
  visibility, name/tag lookup, a component registry **shape**, code-defined prefabs. **Depends on.** M2, M3, D-18.
* **Tests/examples.** Load/unload returns entity and GPU counts to baseline; `spatial_scene` switches scenes.
* **Not initially.** Async/streamed scenes, multiple worlds, nested prefab variants.

### 33. Scene serialization and versioning
* **Now.** Absent (no `serde`). **Planned.** M19: a text format, serialization through the registry,
  entity-reference remapping, asset references by id, versions and migrations. **Depends on.** M4, M17,
  M18; D-18. **Tests.** Save-load-save byte identity; migration test.

### 34. Save games and persistent game state
* **Now.** Absent and **unscheduled.** A save file is not a scene file: it holds *game state* (progress,
  inventory, settings), is written by shipped games, must survive engine upgrades, and may be edited or
  corrupted by players. It needs its own versioning and a per-OS save directory (M24 names the directory
  only). **Proposed:** a typed game-state store designed with M18's (the script API already lists one),
  and a save API in M19 or M24. Needs a decision before M18 fixes the store's shape.

### 35. Debugging and profiling
* **Now.** `log` records and a `Diagnostics` resource (M3a-4). Examples print a few of those numbers in
  the window title. No on-screen log, no inspector. **Planned (incremental, ADR-0003; ROADMAP
  "Incremental developer tooling").** Read-only scene and entity inspection (M4), asset
  validation (M5, M17, M21), render debugging with debug draw and a depth view (M6), an on-screen overlay
  (M10), then M20 consolidates with an edit-capable inspector, profiling scopes (D-21) and collider, skeleton,
  UI and script overlays. **Depends on.** M3a stages, the M4 registry, M6, M10, M19.
* **Not initially.** Remote debugging, GPU profiler integration.

### 36. Editor and inspection
* **Purpose.** A user-friendly editor for building spatial worlds visually, on the same scenes and
  components as code. A core product goal ([VISION.md](VISION.md)), not an optional extra.
* **Now.** Absent. **Planned.** First usable version at M22, behind the stability gate in ADR-0003 (stable
  scene model and registry, asset references, versioned serialization, frozen runtime API surface); not moved
  earlier without a recorded dependency or product need. The foundations it needs are designed in from M3 (R-15). Editor and runtime are separate: the
  runtime must run with the editor absent and nothing may be possible only in the editor. The editor edits
  through the component registry and commands, not renderer internals. Undo/redo is command-based.
* **Required.** Scene and entity editing with a real-renderer viewport, inspector over registered fields,
  asset browser, save/load, undo/redo, play-in-editor.
* **Depends on.** M4 registry, M6 camera-to-texture, M10 (if the UI runtime is reused), M19, M20; D-10, D-16,
  D-18, D-20.
* **Later, in order of need.** Tilemap painting, character and animation setup, material and light editing,
  sprite-sheet and atlas tools; the visual editor for game UI comes after the runtime is stable.
* **Not initially.** Plugins and custom tool windows, visual scripting.

### 37. Build, CI, packaging, platform compatibility
* **Now.** Pinned toolchain (1.99.0), MSRV 1.90, `scripts/check.sh`, `DIMORA_REQUIRE_GPU`. **CI** on
  GitHub Actions for Windows, macOS and Linux (first run green, ADR-0005; software adapters only, active on `main` once merged), **no licence files** (KI-17, the licence is unconfirmed). No packaging, crash handling or release process.
* **Planned.** CI then multi-OS matrix (M23), release builds and docs (M24). Needs decisions on dependency
  and licence policy and on crash diagnostics.

### 38. Documentation, examples, testing, onboarding
* **Now.** README, ROADMAP, TASKS, ARCHITECTURE, DECISIONS, SPATIAL_RENDERING, this file, VISION; 2
  examples (windowed and headless); 138 tests including offscreen pixel tests; doc comments exist on many public items (not audited for
  coverage). No tutorial, no rustdoc landing page, no project template.
* **Planned.** M24 template and docs; each milestone ships its own example (the process in README).
* **Gaps.** No coverage, no benchmark or fuzz targets, no golden-image comparison across GPU vendors
  (pixel assertions use thresholds; llvmpipe and Metal both pass today).

---

## Additional categories this audit adds

These are not on any milestone's task list today. None is a request to build now; each is a "consider
before the related foundation freezes".

| Category | Why it matters | Where it should enter |
|---|---|---|
| Diagnostics and logging facade | Failures need a record a game can capture | Implemented in M3a-4 (`log` crate, `Diagnostics`, ADR-0012). Profiling remains M20 |
| DPI and logical versus physical pixels | Device scale, physical and logical `Viewport`, and pointer conversion are implemented (M3b Phase 3, `9c99edf`). UI fit modes and text are not | M10 for UI scaling policy (D-39) |
| Threading model (`Send`/`Sync`, main thread rules) | Async asset loading, parallel extraction and scripting all depend on it; resources are `RefCell` | Decided (ADR-0006): calling thread owns `World`. Already how the code works |
| Public API tiers | The facade re-exports every crate including `hecs`, `winit` and `wgpu` | Rule decided (ADR-0010). The list is still M9 |
| Save games and settings | Shipped games need them; format promises start at first release | Decision by M18 |
| Determinism across platforms | Replay hashes must not depend on libm differences | Policy at M3, enforced at M9 |
| Gamepad input | Desktop players expect it; action maps must allow it | After M9 (device layer), actions in M3 |
| Camera behaviours (follow, shake, bounds) | Every game needs them; they use the camera contract | After M9, small |
| Tweening, timelines, cutscenes | UI and gameplay polish | After M10 |
| AI and pathfinding | 2.5D worlds need navigation on tilemaps and meshes | After M7/M13, unscheduled |
| Localisation, accessibility | Text and input architecture decisions (D-37, D-39) can foreclose them | Note in D-37/D-39 |
| Crash diagnostics | Shipped games need logs and panic reports | M24 |
| Dependency and licence policy | Several decisions add crates (glTF, audio, fonts); licence files absent | D-17, KI-17 |
| Build size and startup time | Shipping budgets | M23 |
