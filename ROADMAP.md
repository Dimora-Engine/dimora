# Dimora roadmap

Dimora is a purpose-built Rust engine for **spatial 2.5D games**. Its central principle:

> **2.5D is a spatial world, not a fake 2D layer system.** Sprites, skeletal characters, tiles,
> particles and 3D meshes all live in one world with real X/Y/Z, are seen through real cameras, and
> are lit by one lighting system.

It is not a general-purpose Unity clone. Features exist if a spatial 2.5D game needs them.

This roadmap is written against the code as audited on 2026-10-12 (commit `6ad872f`). The current
state is in [TASKS.md](TASKS.md); how the engine is built is in
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md); open design questions are in
[docs/DECISIONS.md](docs/DECISIONS.md).

Status values used below: **Done** (evidence cited), **Not started**. Nothing is marked in progress or
partially done unless TASKS.md shows commits for it. No milestone after M1 is complete.

## 1. Where the project stands

### Capability audit

"Working" means implemented, exercised by an example, and covered by a test unless stated. A type or
module existing is not counted.

| Area | State | Evidence and limits |
|---|---|---|
| Math (`Vec2/3/4`, `Quat`, `Mat3/4`, `Transform`, `Rect`) | Working | 22 unit tests. `f32` only. Non-uniform scale + rotated parent shears (not representable in `Transform`) |
| ECS / world | Working, thin | `hecs` wrapper, typed resources, hierarchy; 7 tests. No events, change detection, deferred commands |
| Transform hierarchy | Working on the happy path, 2 verified bugs | Parent/child tests pass; a child of a transform-less or despawned parent stays at the origin (scratch test, 2026-10-12). Full O(n) pass every frame |
| Camera and projection | Working | 27 tests: view/proj/NDC chain, ortho/perspective known points, viewport, parallax. Two projections only; `Camera` cannot hold a custom `Projection` |
| Rendering backend (wgpu) | Working | One forward pass, depth buffer, instancing; 7 GPU pixel tests (need a Vulkan adapter). One camera per frame, no render-to-texture |
| Sprites | Working | Spatial quads; billboard/upright/fixed; flips, region, tint, anchor; cutout and alpha blend; batching (5000 sprites = 1 draw in a test). No sprite-sheet animation, no atlas packer |
| 3D meshes | Working, basic | Indexed, instanced, culled, one albedo texture + colour. Only `cuboid` and `cone` generators. No loader, tangents, skinning, transparency |
| Materials | Placeholder | `Material` = colour + optional albedo + `lit` flag. Not an asset; no alpha modes |
| Textures | Partial | PNG decode; nearest/linear; clamp only; no mipmaps; always sRGB |
| Lighting | Working, forward only | Ambient, directional, point, spot (max 16), wrap-lit sprites, fog; 1 GPU test for falloff. **No shadows, no normal maps, no API stubs for either** |
| Tilemaps | Working, basic | `TileMap` expanded to cutout quads per frame (80x120 in the example); 1 extraction test. No chunking, loader, collision, animated tiles |
| Skeletal animation | Data model + simple player | Bones are entities; linear TRS keyframes; looping; 4 tests; used by the demo character. No blending, events, IK, skinning, file loading |
| Particles | Absent | |
| Collision / gameplay | Absent | |
| Input | Working, narrow | 47 keys, mouse buttons/position/scroll, edge states, `axis()`; 5 core tests. No actions, gamepad, touch, text, cursor grab, F-keys |
| Application lifecycle | Working | winit 0.30 loop, resize, exit, title; verified manually under Xvfb. Single window; `ControlFlow::Poll`; no fullscreen/vsync options |
| Time | Working | `Time`, `FixedTimestep` (tested, max 8 steps/frame); `alpha()` unused |
| Scheduling | Minimal | Sequential `Schedule`; engine stages hard-coded in `Game::step` |
| Assets | Partial | Typed handles, name-hash ids, load/reload/revisions/dependencies, filesystem + memory sources; 8 tests. Synchronous; PNG is the only decoder; no hot-reload watcher; no ids that survive a rename |
| Scene serialization | Absent | No `serde` in the workspace |
| Debug tools | Minimal | Window-title stats (`RenderStats`) only |
| Editor | Absent | |
| Asset compiler / bundles | Absent | `AssetSource` trait is the planned plug-in point |
| Packaging / release | Absent | `Cargo.toml` declares `MIT OR Apache-2.0` but there are no licence files; crates are `publish = false` |
| CI / toolchain | Absent | No CI, no `rust-version`, no toolchain pin |
| Examples | 2, both runnable windowed and headless | `basic_2_5d`, `spatial_scene` |
| Audio, UI, scripting, networking | Absent, not requested | Audio and UI become relevant at M8 (see "Unscheduled") |

### Check results (2026-10-12)

| Check | Result |
|---|---|
| `cargo fmt --check` | Pass |
| `cargo clippy --workspace --all-targets` | Pass, no warnings |
| `cargo test --workspace` (stable 1.99.0) | Pass: 99 tests, 0 failures |
| Same GPU tests with no Vulkan adapter | The 7 GPU tests **skip and still report `ok`** (seen on a fresh VM until `mesa-vulkan-drivers` was installed). With lavapipe: all 7 pass |
| `cargo build` on rustc 1.83 | **Fail**: dependency `wayland-protocols 0.32.13` needs Cargo `edition2024` |
| `cargo doc --workspace --no-deps` | 2 warnings (unresolved links in `crates/engine/src/lib.rs`) |
| CI | None exists |

Details and per-crate counts are in TASKS.md.

## 2. Ordering rules

1. Correctness of existing foundations (M2).
2. Small end-to-end working features (M3-M6).
3. A playable example game (M7-M8).
4. Distinctive 2.5D functionality (M9-M12).
5. Asset and developer workflows (M13-M16).
6. Advanced tools and optional features (M17-M19).

Do not create a crate for a subsystem until it has code. New crates in this plan: none before M15
(`dimora-assets` compiler CLI) and M17 (`dimora-editor`).

### How this maps to the requested stages

| Requested stage | Milestone | Note |
|---|---|---|
| 1 Core spatial renderer | M0, M1 (Done); residue in M6 | Spatial renderer works; one-camera limit and render targets remain |
| 2 World and scene management | M4 | |
| 3 Materials and textures | M5 | |
| 4 Renderer stabilization | M6 | |
| 5 Input and update scheduling | M3 | **Moved before 2-4**: collision, animation and scenes all need defined stages |
| 6 Collision and gameplay | M7, M8 | M8 (playable slice) added |
| 7 Skeletal animation | M9 | |
| 8 Tilemaps | M10 | |
| 9 Unified lighting and shadows | M11 | |
| 10 Particles | M12 | |
| 11 Asset management and importers | M13 | |
| 12 Asset compiler and bundles | M15 | **Moved after scene serialization**: the compiler must understand scene files |
| 13 Scene serialization | M14 | |
| 14 Debugging tools | M16 | A minimal line/debug-draw primitive is pulled forward into M6 for collision work |
| 15 Minimal editor | M17 | |
| 16 Performance and stability | M18 | Baseline benchmarks start in M6 |
| 17 Packaging and release | M19 | |
| (new) | M2 Foundation hardening and CI | Priority 1: known defects and a reproducible build before building on top |
| (new) | M8 Playable vertical slice | Priority 3: integration test for everything before it; finds gaps early |

### Dependency graph

```text
M2 hardening/CI
 ├─► M3 input + scheduling ──┬─► M4 scenes ──┐
 │                           │               ├─► M7 collision ─► M8 playable slice ─┬─► M9 skeletal anim
 └─► M5 materials/textures ─► M6 renderer stabilization ──┘                         ├─► M10 tilemaps
                                         │                                           ├─► M11 lighting+shadows (also needs M5, M6)
                                         └─ debug draw ─► M7                         └─► M12 particles
M4 + M9 + M10 ─► M13 assets/importers ─► M14 scene serialization ─► M15 compiler/bundles
M14 + M6 ─► M16 debug tools ─► M17 editor        M15 + M17 ─► M18 perf/stability ─► M19 packaging/release
```

---

## M0. Foundation: Done

Workspace, math, ECS wrapper, assets, extraction-based renderer, wgpu backend, animation data model,
platform layer, facade, `basic_2_5d`. Evidence: commits `6b0d016`..`278de15`; 99 tests pass.

## M1. Core spatial renderer: Done

Sprite orientation modes (`Billboard`/`Upright`/`Fixed`), `RenderOrder`, sprite `region`, camera
`ViewportRect`, documented conventions, `spatial_scene`, camera/projection/orientation tests.
Evidence: commits `83d9883`..`6ad872f`; `crates/engine-renderer/tests/camera.rs` (27 tests), GPU
tests for viewport and orientation, `docs/SPATIAL_RENDERING.md`.

---

## M2. Foundation hardening and CI: Not started

**Objective.** Make the build reproducible and fix the defects found in the audit before building
more on top.

**Prerequisites.** None.

**Tasks.**
- Determine the real MSRV by building with candidate toolchains; set `rust-version` and a
  `rust-toolchain.toml`.
- `scripts/check.sh`: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, `cargo doc --workspace --no-deps` with warnings denied.
- GPU tests: `DIMORA_REQUIRE_GPU=1` turns "no adapter" from skip into failure; document lavapipe setup.
- Fix the two rustdoc warnings.
- `AssetManager::remove`; renderer evicts GPU textures/meshes whose assets are gone or unused (policy
  from D-04).
- `propagate_transforms`: parents without `Transform` act as identity; `despawn` of a parent no longer
  leaves dangling children (policy from D-03).
- Asset-name collision check on insert (two names hashing to one id is an error, not an overwrite).
- Add licence files matching `Cargo.toml`.
- Pick the code host/CI provider (D-22), then add its config calling `scripts/check.sh`.

**Dependencies.** D-03, D-04, D-22 (CI provider only; everything else proceeds without it).

**Acceptance criteria.**
- A clean checkout with the pinned toolchain passes `scripts/check.sh` on a machine with lavapipe.
- With `DIMORA_REQUIRE_GPU=1` and no adapter, `cargo test -p engine-renderer-wgpu` fails and names the
  missing driver.
- Orphan-child and transform-less-parent behaviours are covered by tests that fail on the current
  code.
- A headless test shows GPU texture/mesh counts return to baseline after removing assets.
- Two assets whose names collide are reported, not overwritten.

**Tests/examples.** New unit tests per fix; both examples still render (screenshots compared by eye).

**Deferred.** Coverage tooling, multi-platform CI matrix (M18), release automation (M19).

---

## M3. Input and update scheduling: Not started

**Objective.** A deterministic, documented frame with ordered stages, and an input layer sufficient
for a real game.

**Prerequisites.** M2.

**Tasks.**
- Named stages: input -> fixed update (N steps) -> update -> animate -> post-update (transform
  propagation) -> extract -> render. User systems attach to a stage, with explicit ordering
  (`before`/`after` by name). Engine stages stop being hard-wired inside `Game::step` (D-01).
- Expose the fixed-step alpha and add opt-in transform interpolation for entities moved in the fixed
  stage.
- Deferred command buffer (`Commands`) flushed at stage boundaries.
- Input: full key set (F-keys, punctuation, numpad), cursor grab/visibility, an action map (named
  actions and axes bound to keys/buttons), raw mouse delta.
- Window/frame config: vsync on/off, optional frame cap, fullscreen toggle.

**Dependencies.** D-01, D-02, D-10.

**Acceptance criteria.**
- A test registers systems out of order across stages with `before`/`after` and observes the
  documented order.
- A fixed-step entity moved at 60 Hz with a 144 Hz render rate produces monotonically increasing
  interpolated positions in successive extracted scenes (test through `Game::step` + `extract`).
- Commands queued while iterating a query apply at the stage end (test).
- Action map unit tests cover press, release, axis and rebinding.
- `spatial_scene` uses actions instead of raw keys.

**Tests/examples.** Scheduler unit tests, interpolation test, action map tests; updated examples.

**Deferred.** Parallel system execution, gamepad, touch, text input/IME, rebinding UI.

---

## M4. World and scene management: Not started

**Objective.** A scene is a unit that can be built, switched and unloaded without leaking or
orphaning entities, and the world knows its component types by name.

**Prerequisites.** M2 (hierarchy fixes), M3 (stages, commands).

**Tasks.**
- Scene membership (`SceneMember`), `SceneManager` resource with load/unload/replace applied at a
  stage boundary.
- Hierarchical visibility (a hidden parent hides its subtree).
- Name/tag lookup.
- A component registry mapping stable names to component types (no serialization yet); this is the
  groundwork M14 and M16 need.
- Code-defined prefabs (`fn(&mut World, ...) -> Entity` by convention) and one documented pattern.

**Dependencies.** D-03, D-10, D-18 (registry shape).

**Acceptance criteria.**
- Loading then unloading a scene returns entity count and GPU resource counts to baseline (test).
- A hidden parent hides all descendants in the extracted scene (test).
- Despawning any entity cannot leave a dangling `Parent` (test).
- `spatial_scene` switches between two scenes at runtime.

**Tests/examples.** Scene lifecycle tests; `spatial_scene` scene-switch demo.

**Deferred.** Async/streamed scenes, world partitioning, multiple worlds, serialization.

---

## M5. Materials and textures: Not started

**Objective.** A small but real material and texture model that normal maps, shadows and the asset
pipeline can build on without rewriting the renderer.

**Prerequisites.** M2.

**Tasks.**
- Texture: mipmap generation, sampler parameters (filter, wrap, anisotropy), a colour-space flag
  (sRGB vs linear) so data textures are possible.
- Runtime sprite atlas builder (shelf packing) with padding/extrusion; `SpriteSheet` plus a simple
  flipbook animation system for sprites.
- `MaterialAsset` (base colour, albedo, reserved normal-map slot, alpha mode opaque/mask/blend, lit,
  double-sided) referenced by handle from sprites and meshes; batching keyed by pipeline and material.
- Mesh generators: plane, sphere, cylinder.
- Policy for blended meshes (D-05).

**Dependencies.** D-05, D-27.

**Acceptance criteria.**
- GPU test: a minified high-frequency texture averages to its mean colour with mipmaps, and aliases
  without them.
- GPU test: a linear-format texture reaches the shader with exact values.
- Atlas packer unit tests: no overlap, padding respected, deterministic.
- A flipbook sprite advances frames with `Game::step` (test).
- 1,000 sprites sharing a material remain one draw call.

**Tests/examples.** Above, plus an animated sprite-sheet figure in `spatial_scene`.

**Deferred.** PBR, custom shaders, texture compression, texture streaming, normal-map shading (M11).

---

## M6. Renderer stabilization: Not started

**Objective.** Remove renderer limits that would otherwise become baked into later features, and
establish a measured performance baseline.

**Prerequisites.** M5.

**Tasks.**
- Render every active camera in `order`, each with its own viewport and clear/load behaviour.
- Camera-to-texture render targets.
- Custom projections: let `Camera` hold more than the two enum variants (D-25).
- Tilemap chunking with cached instances.
- Debug draw (lines, boxes, spheres, rays; depth-tested) as the minimal debugging primitive for M7.
- Pixel-art policy: texel snapping / integer scaling / MSAA (D-06); depth convention (D-07).
- Benchmarks (a small timing binary, no new dependency) for extraction and render CPU time with N
  sprites/meshes/tiles; record results and budgets in `docs/PERFORMANCE.md`.

**Dependencies.** D-06, D-07, D-08, D-25.

**Acceptance criteria.**
- Two cameras with different viewports render into one target (GPU pixel test).
- A camera rendered to a texture and sampled by a sprite appears correctly (GPU test).
- Changing one tile re-extracts one chunk (test); a 1000x1000 map's per-frame extraction cost is
  recorded and bounded by chunk count rather than tile count.
- `docs/PERFORMANCE.md` contains baseline numbers with the machine they were measured on.

**Tests/examples.** GPU tests above; `spatial_scene` gains a picture-in-picture camera.

**Deferred.** Render graph, deferred shading, HDR/post-processing, compute-driven culling.

---

## M7. Collision and gameplay primitives: Not started

**Objective.** Spatial collision and a kinematic character controller; no rigid-body physics.

**Prerequisites.** M3 (fixed stage), M4 (events/commands), M6 (debug draw).

**Tasks.**
- Collider components (sphere, box, capsule; layers/masks) in **3D world space**, so an object at a
  different Z does not collide.
- A `CollisionWorld` resource rebuilt in the fixed stage (uniform grid or sweep-and-prune
  broadphase), raycast and overlap queries.
- Kinematic move-and-slide controller with swept tests.
- Trigger enter/exit events through an event channel.
- Debug visualisation through M6 debug draw.

**Dependencies.** D-09, D-10.

**Acceptance criteria.**
- Shape-pair overlap tests over a table of cases; raycast tests per shape.
- A character moving at high speed cannot tunnel through a thin wall (test) and slides along it.
- Simulation is deterministic: the same inputs over 1,000 fixed steps give an identical state hash.
- Two colliders with equal X/Y and different Z do not collide.
- `spatial_scene`: the walkable figure is blocked by the pillar.

**Tests/examples.** Collision unit/property tests; controller test; example.

**Deferred.** Rigid-body dynamics, joints, continuous collision for dynamic bodies, convex-mesh
colliders, navmesh.

---

## M8. Playable vertical slice: Not started

**Objective.** One complete playable game that exercises M3-M7 together and exposes what is missing.

**Prerequisites.** M3-M7.

**Tasks.**
- `examples/vertical_slice`: one level, controllable character (existing skeleton), collectibles
  (triggers), simple hazards/enemies, win/lose, restart via scene reload.
- Minimal on-screen text through a bitmap font drawn as sprites (no UI system).
- Input record/replay harness (small) for headless regression runs.
- Write up engine gaps found and fold them into TASKS.md and this roadmap.

**Dependencies.** D-11 (audio), D-24 (UI) decisions are made here at the latest.

**Acceptance criteria.**
- The game can be played from start to win and to lose.
- A recorded input log replays headlessly and produces the same final state hash (test).
- A 10-minute scripted soak run completes without panics or memory growth beyond a recorded budget.
- A gap list exists in TASKS.md with each gap assigned to a milestone.

**Tests/examples.** The example itself, replay test, soak script.

**Deferred.** Audio and full UI unless D-11/D-24 pull them in; content beyond one level.

---

## M9. Skeletal animation: Not started

**Objective.** Production-usable 2D skeletal animation on the existing `Skeleton`/bone-entity model.

**Prerequisites.** M3 (animate stage), M5 (atlas/materials for parts), M8 (a real consumer).

**Tasks.**
- Crossfade blending and a small animation state machine.
- Animation events fired exactly once per key, including across loop wrap.
- Interpolation modes (step, linear, cubic).
- Skin/slot swapping (swap part region/material).
- Optional two-bone IK, only if the slice needs it.
- Decide and implement part ordering authoring (real-Z offsets already work; add tooling/validation).
- File format for skeletons and clips defined together with the importers (M13).

**Dependencies.** D-12.

**Acceptance criteria.**
- Crossfade at 50% equals the lerp of the two sampled poses (unit test).
- Events fire once at their keyed times, and again after a loop (test).
- Sampling is independent of frame rate (determinism test).
- The slice character uses the state machine.

**Tests/examples.** Unit tests above; slice integration.

**Deferred.** Spine/other importers, mesh skinning (unless D-12 says otherwise), ragdoll, retargeting.

---

## M10. Tilemaps: Not started

**Objective.** Tilemaps that scale and integrate with collision, built on the existing instanced path.

**Prerequisites.** M6 (chunking), M7 (collision layer hook), M5 (atlas).

**Tasks.**
- Chunked tilemap storage and extraction; per-tile flip/rotate flags; multiple layers at distinct Z.
- Animated tiles; per-tile properties; collision layer feeding M7.
- Tileset asset (atlas reference + metadata) and an in-code builder.
- Vertical (wall) tilemaps through the same orientation model.

**Dependencies.** D-25 if isometric/oblique projection is wanted for tilemaps.

**Acceptance criteria.**
- A 1000x1000 map renders with per-frame CPU cost proportional to visible chunks (benchmark, recorded).
- `set_tile` touches one chunk (test).
- A tile collision layer blocks the character controller (test).
- Animated tiles advance with time (test).

**Tests/examples.** Tests above; slice level uses the chunked map.

**Deferred.** Tiled importer (M13), auto-tiling, hex/isometric grids, infinite/streamed maps.

---

## M11. Unified lighting and shadows: Not started

**Objective.** Sprites and meshes lit and shadowed by the same system, including normal-mapped
sprites.

**Prerequisites.** M5 (normal-map slot, linear textures), M6 (render targets and multiple passes), D-13.

**Tasks.**
- Shadow maps for directional and spot lights; alpha-tested sprite casters.
- Normal-mapped sprites (tangent frame from the orientation mode).
- Light culling beyond 16 lights (D-13).
- Light layers/masks; bias and filtering.

**Dependencies.** D-13, D-14.

**Acceptance criteria.**
- GPU test: an occluder darkens a receiver, and moving the light removes the shadow.
- A sprite casts a visible shadow on the ground.
- GPU test: a normal map changes sprite shading with light angle.
- 64 lights in a scene stay within the recorded frame budget.

**Tests/examples.** GPU tests; slice lighting pass.

**Deferred.** Global illumination, SSAO, PBR, baked lighting, volumetrics.

---

## M12. Particles: Not started

**Objective.** CPU-simulated particles in the same spatial world.

**Prerequisites.** M3 (stages), M5 (materials), M6 (budgets), M8 (consumer).

**Tasks.** Emitter component (rate, lifetime, velocity, gravity, colour/size curves), seeded
deterministic RNG, particles emitted into the existing instanced sprite path with depth testing,
emitters following hierarchy, per-system sort policy (D-15).

**Acceptance criteria.**
- Same seed gives the same particle state frame by frame (test).
- 100,000 live particles extract within the recorded budget.
- GPU test: particles are occluded by a nearer mesh and occlude a farther one.

**Tests/examples.** Above; slice effects.

**Deferred.** GPU particles, ribbons/trails, world collision, sub-emitters.

---

## M13. Asset management and importers: Not started

**Objective.** Real content workflows: files on disk, async loading, hot reload, stable ids.

**Prerequisites.** M5, M9/M10 data shapes, decisions D-16 to D-18.

**Tasks.**
- Stable asset identity and dependency graph with cascading reload/unload (D-16).
- Worker-pool async loading with main-thread apply.
- File-watcher hot reload.
- Importers: PNG (exists), glTF meshes/materials, sprite-atlas JSON (Aseprite-style), Tiled tilemaps,
  skeleton and clip format.
- Loading errors surface as logged errors plus the existing magenta placeholder.

**Dependencies.** D-16, D-17, D-18.

**Acceptance criteria.**
- Examples load their art from files with no procedural fallback.
- Editing a PNG on disk changes the running example without restart (integration test using reload).
- Changing a texture reloads dependent sprites (test).
- 1,000 assets load on workers without a frame-time spike beyond a recorded budget.
- A glTF cube loads with correct winding and bounds.

**Tests/examples.** Importer unit tests with fixture files; reload integration test.

**Deferred.** Bundles (M15), compression, DCC plugins.

---

## M14. Scene serialization: Not started

**Objective.** Save and load scenes as versioned, diffable text files.

**Prerequisites.** M4 (component registry), M13 (stable ids).

**Tasks.** Format and `serde` policy (D-18), component serialization through the registry, entity
reference remapping, hierarchy, asset references by id, version/migration scaffolding, basic prefab
instancing, loading scenes in `Game`.

**Acceptance criteria.**
- Save -> load -> save is byte-identical (deterministic ordering).
- A scene with a hierarchy and every renderable component round-trips and extracts an identical
  `RenderScene`.
- Unknown components and missing assets produce clear errors, not panics.
- A migration test upgrades a stored older-version scene.
- The slice level is a scene file.

**Tests/examples.** Round-trip, golden-file and migration tests.

**Deferred.** Binary scene format (M15), nested prefab variants, scripting references.

---

## M15. Asset compiler and bundles: Not started

**Objective.** Ship assets and scenes as one deterministic bundle read through `AssetSource`.

**Prerequisites.** M13, M14.

**Tasks.** New `dimora-assets` CLI crate; compile imported assets and scenes; bundle with an id
index, content hashes and optional compression (D-19); `BundleSource`; incremental cache keyed by
content hash; reference validation.

**Acceptance criteria.**
- Same inputs produce byte-identical bundles.
- The slice runs from a bundle with no loose files.
- Rebuilding unchanged inputs does nothing.
- A corrupted bundle is detected on load.
- Load time versus loose files is measured and recorded.

**Tests/examples.** Determinism, corruption and incremental-build tests.

**Deferred.** Patching/DLC, streaming, encryption, platform-specific texture formats.

---

## M16. Debugging tools: Not started

**Objective.** See and poke the running engine.

**Prerequisites.** M6 (debug draw), M8 (bitmap font), M14 (component registry for reflection).

**Tasks.** Overlay (frame graph, counters), entity inspector (read first, edit second), logging and
profiling scopes (D-21), screenshot and replay tooling promoted from M8, asset/hot-reload
notifications.

**Acceptance criteria.**
- A key toggles an overlay with live frame time and draw calls.
- The inspector lists entities and edits a `Transform` of the running `spatial_scene`.
- A profiling run writes a trace viewable in a standard viewer.

**Tests/examples.** Inspector unit tests over the registry; manual demo.

**Deferred.** Remote debugging, networked tools.

---

## M17. Minimal editor: Not started

**Objective.** Edit and save a scene visually.

**Prerequisites.** M6 (camera-to-texture), M14, M16, D-20.

**Tasks.** `dimora-editor` binary; viewport with fly camera and translate/rotate/scale gizmos;
hierarchy and inspector panels; asset browser; save/load; play-in-editor on a copy of the world;
minimal undo/redo.

**Acceptance criteria.**
- Open the slice scene, move an entity with a gizmo, save, run the game, see the change.
- Undo restores the previous state.
- Saving is atomic: a crash cannot leave a half-written scene.

**Tests/examples.** Command/undo unit tests; manual walkthrough recorded.

**Deferred.** Tilemap painter, animation editor, plugins, custom tool windows.

---

## M18. Performance and stability: Not started

**Objective.** Meet recorded budgets and harden against bad input.

**Prerequisites.** M15, M17 (and baselines from M6).

**Tasks.** Transform dirty tracking and/or parallel propagation; culling acceleration structure;
parallel extraction; memory budgets and leak soak; fuzzing of decoders (PNG path, glTF, scene,
bundle); audit of `unwrap`/`expect` on external-data paths; benchmark regression thresholds in CI;
testing on Windows, macOS and Linux GPUs.

**Acceptance criteria.**
- The budgets in `docs/PERFORMANCE.md` are met on the recorded reference machines.
- A one-hour soak shows no memory growth.
- Fuzz targets run for a fixed time without crashes.
- No panics from malformed external files.

**Tests/examples.** Benchmark suite, fuzz targets, soak script.

**Deferred.** GPU-driven rendering, multi-threaded GPU submission.

---

## M19. Packaging and release: Not started

**Objective.** Ship a desktop game and the engine itself.

**Prerequisites.** M15, M18.

**Tasks.** Desktop release builds (Windows, macOS, Linux) with a bundle; config and save-game
directories; log files; versioning policy and changelog; licence files; a project template; docs
(rustdoc and a short book); release automation; crates.io decision (D-23).

**Acceptance criteria.**
- A tagged release produces runnable artifacts for three operating systems from CI.
- A new user builds and runs the template game by following the docs.
- Licence files and attribution for dependencies are present.

**Tests/examples.** Release smoke test on each OS; template build test.

**Deferred.** Consoles, mobile, web (wasm), store integrations, code signing.

---

## Unscheduled but known

These are outside the requested scope but will matter; each has a decision slot.

- **Audio.** A playable game wants sound. Decide at M8 (D-11).
- **UI.** M8 uses sprite-drawn text only. A real UI system is out of scope until after M17 (D-24).
- **Gamepad/touch.** After M8.
- **Web/mobile targets.** After M19 (D-23).
- **Networking, scripting.** Not planned.
