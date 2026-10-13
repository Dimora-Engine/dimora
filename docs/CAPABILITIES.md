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
| MSRV on Linux | `DIMORA_REQUIRE_GPU=1 cargo +1.90.0 test --workspace --locked` on rustc 1.90.0 (1159e78c4), Linux x86_64, llvmpipe: **138 passed, 0 failed, none skipped** (closes the Linux half of KI-27; Windows still open) |
| Windows | Never built or run |
| CI | None (D-22) |

Code size: about 8,000 lines of Rust in nine library crates and two examples. `unsafe`: none in the
repository. Logging: none (no `log`/`tracing`, no `println!` in library code).

## Summary

Systems are grouped as in the audit request. "Needs by" is the first milestone that cannot be built
without it.

| # | System | Status today | Evidence in one line | Milestone |
|---|---|---|---|---|
| **Core runtime** | | | | |
| 1 | Math, conventions, transforms | Working | 22 tests; `f32`; shear not representable | done |
| 2 | ECS, world, hierarchy | Working, thin | `hecs` wrapper; 19 tests; no events, change detection, commands | M3, M4 |
| 3 | Frame loop and scheduling | Minimal | Sequential `Schedule`; stages hard-wired in `Game::step` | M3 |
| 4 | Events and deferred commands | Absent | Resources are `RefCell`; nothing queues spawns or events | M3 |
| 5 | Platform, window, timing | Working, narrow | winit loop, resize, exit; no DPI, fullscreen, vsync, multi-window | M3, M10 |
| 6 | Input and action mapping | Partial | Keys/mouse state with edges, `axis()`; no actions, gamepad, text | M3 |
| 7 | Errors, logging, diagnostics, config | Absent / Minimal | Typed errors in crates; no logging; no runtime config; window-title stats | M3 (logging), M20 |
| **Rendering** | | | | |
| 8 | Cameras and projection | Working | Ortho + perspective, viewport, picking rays; 27 tests. One camera/frame; closed projection enum | M6 |
| 9 | Spatial sprites | Working | Orientation modes, region, tint, cutout/alpha; batching test; no atlas packer | M5 |
| 10 | Depth, transparency, ordering | Partial | One depth buffer shared by meshes and sprites (GPU tests); blended sprites sort by centre; meshes cannot blend | M5, M14 |
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
| 35 | Debugging and profiling | Minimal | `RenderStats` in the window title | M20 |
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
* **Unresolved.** D-02 (change detection), D-18 (registry), D-45 (what of `hecs` is public).
* **Not initially.** Parallel systems, reactive change detection, archetype-level APIs.

### 3. Frame loop and scheduling
* **Purpose.** A defined, documented frame so systems can depend on stage order.
* **Now.** `Game::step`: time, asset loads, fixed update (0 to 8 steps), update systems, `animate_system`,
  `propagate_transforms`, input edge reset; then `extract` and `render`, called separately. User systems
  are `FnMut(&mut World)` in insertion order. Hard-wired engine stages; `FixedTimestep::alpha()` unused.
* **Required.** Named stages; user systems attached to a stage with `before`/`after`; fixed-step
  interpolation for rendering; a headless `step` that tests drive directly (exists).
* **Planned.** M3. **Depends on.** D-01, D-02, D-10, D-44. **Integration.** Collision (fixed stage),
  animation (animate stage, camera read from previous propagation, D-31), UI (input stage first), scripts
  (one stage), scenes (apply at a boundary).
* **Performance.** Sequential is fine until measured; do not add parallelism before D-44.
* **Tests/examples.** Ordering test, interpolation test, 144 Hz versus 60 Hz determinism test.
* **Unresolved.** D-01, D-44. **Not initially.** Parallel execution, a dependency-graph scheduler.

### 4. Events and deferred commands
* **Purpose.** Communicate between systems without borrowing each other's state; mutate safely mid-stage.
* **Now.** Nothing. Double borrow of a `RefCell` resource panics at run time (KI-9).
* **Required.** A command buffer flushed at stage boundaries and typed per-frame event channels (D-10).
* **Depends on.** D-01. **Integration.** Collision events, animation events, scene load/unload, asset
  failure, UI callbacks, script mutations. The scripting boundary requires it.
* **Tests/examples.** Commands queued while iterating; event visible later in the same frame and gone the
  next.
* **Not initially.** Observers/reactive triggers, event persistence, networking.

### 5. Platform, window, timing
* **Now.** winit 0.30 loop, resize, exit, title, `ControlFlow::Poll`, default present mode. `Time` and
  `FixedTimestep` tested. **No window scale factor anywhere in the code** (KI-24): physical pixels are
  used throughout. Single window.
* **Required (desktop).** DPI scale and changes, fullscreen, vsync/frame cap, cursor grab/visibility,
  focus loss, minimise (no surface), monitor info.
* **Planned.** M3 (config, scale factor forwarded early because `Viewport`, pointer and UI all depend on
  it), M10. **Depends on.** Nothing. **Integration.** `Viewport` carries physical size, logical size and
  scale; UI maps pointer to logical units.
* **Performance.** Poll loop burns CPU; a frame cap is needed before shipping.
* **Tests/examples.** Platform code is hard to test; keep logic in pure functions (key mapping exists).
  Manual checklist per OS.
* **Unresolved.** D-39 (scaling model), D-23 (platforms). **Not initially.** Multi-window, IME,
  touch, mobile lifecycle.

### 6. Input and action mapping
* **Now.** 47 keys, mouse buttons/position/scroll, pressed/released edges, `axis()`; 5 core tests.
* **Required.** Named actions and axes bound to keys/buttons, rebinding, an input record/replay format
  (M9), UI input capture before gameplay.
* **Planned.** M3 actions; gamepad after M9 (unscheduled device layer; actions must not assume keyboard);
  text input only with UI text fields. **Depends on.** D-10 (events).
* **Integration.** UI focus navigation and scripts use **actions**, never raw devices.
* **Tests/examples.** Action map unit tests (press, release, axis, rebinding); `spatial_scene` on actions.
* **Not initially.** Gamepad, touch, IME, rebinding UI, per-device profiles.

### 7. Errors, logging, diagnostics, configuration
* **Now.** Crates return typed errors (`AssetError`, `HierarchyError`, `RenderError`, `PlatformError`);
  missing assets are counted in `ExtractStats` and drawn as a placeholder (renderer) or skipped (sprites).
  **No logging at all**, so a failed asset load is visible only to code that polls its state. No runtime
  configuration file or CLI convention beyond example flags. `unwrap`/`expect` on external-data paths has not
  been audited (M23).
* **Required.** A logging facade used by every crate, errors that carry asset names and positions, an
  in-game surface for them (M20), a panic hook that writes a log file (M24), explicit config.
* **Planned.** The facade moves from M20 to M3 (D-21 split). **Depends on.** D-21.
* **Integration.** Asset failures, script errors, GPU device loss, replays and CI all report here.
* **Tests/examples.** A test that a failed load emits one error with the asset name.
* **Not initially.** Remote telemetry, metrics backends, crash upload.

---

## Rendering

### 8. Cameras and projection
* **Now.** `Camera` component (`Orthographic`, `Perspective`, normalised viewport, clear colour, order),
  `CameraView` (view, projection, viewport pixels, basis), `Projection` trait, picking rays, frustum.
  27 camera tests including parallax. Exactly one camera renders per frame (the lowest `(order, id)`
  active one); `Camera` holds the closed enum, not the trait (KI-5, KI-11).
* **Required.** Multiple cameras with viewports, camera-to-texture, extensible projection (oblique,
  isometric), resize safety. **Planned.** M6. **Depends on.** D-08, D-25, D-06, D-07.
* **Integration.** UI world anchors, picking, audio listener, directional frames (D-31) and the editor
  viewport all read the active camera's view.
* **Performance.** Per-camera extraction multiplies CPU cost; cull per camera.
* **Tests/examples.** Two viewports in one target (GPU test); picture-in-picture in `spatial_scene`.
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
* **Unresolved.** KI-21 (natural size ignores region), D-26 (more orientations).
* **Not initially.** Sprite meshes (non-quad), 9-slice (UI), per-sprite shaders.

### 10. Depth, transparency, occlusion, render ordering
* **Now.** One `Depth32Float` buffer, standard Z (`Less`). Order inside the single pass: opaque meshes,
  cutout sprites and tiles (depth write), then alpha sprites back to front (depth test, no write).
  `RenderOrder { layer, order }` overrides blended sprite order only. GPU tests: nearer mesh occludes a
  sprite, farther does not; blended sprite interleaves with meshes by depth. Limits: blended sprites sort
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
  world-only; no scale factor; whole-texture re-upload blocks a glyph atlas (KI-24, KI-25).
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
* **Unresolved.** Versioning and compatibility policy for every on-disk format (assets, scenes, bundles,
  saves): none exists. Proposed rule in R-6.

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
* **Now.** Window-title stats only. **Planned.** M20 overlay, entity inspector, profiling scopes (D-21),
  collider/skeleton/UI/script overlays; M6 debug draw. **Depends on.** M6, M10, M19.
* **Not initially.** Remote debugging, GPU profiler integration.

### 36. Editor and inspection
* **Purpose.** A user-friendly editor for building spatial worlds visually, on the same scenes and
  components as code. A core product goal ([VISION.md](VISION.md)), not an optional extra.
* **Now.** Absent. **Planned.** First usable version at M22, after scenes, assets and debug tools exist
  (D-20); the foundations it needs are designed in from M3 (R-15). Editor and runtime are separate: the
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
* **Now.** Pinned toolchain (1.99.0), MSRV 1.90, `scripts/check.sh`, `DIMORA_REQUIRE_GPU`. **No CI**
  (D-22), **no licence files** (KI-17), Windows untested. No packaging, crash handling or release process.
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
| Diagnostics and logging facade | Every silent failure today is invisible | M3 (D-21 split) |
| DPI and logical versus physical pixels | `Viewport`, pointer, UI scaling and screenshots all change | M3 platform slice |
| Threading model (`Send`/`Sync`, main thread rules) | Async asset loading, parallel extraction and scripting all depend on it; resources are `RefCell` | D-44, before M3 ships |
| Public API tiers | The facade re-exports every crate including `hecs`, `winit` and `wgpu` | D-45, by M9 |
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
