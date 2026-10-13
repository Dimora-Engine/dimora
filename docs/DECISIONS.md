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
| D-03 | Despawn semantics for entities with children | M2 (T3) | M2 hierarchy fix, M4 | Decided (2026-10-13) |
| D-04 | GPU cache lifetime | M2 (T2) | M2 eviction, M17 unload | Decided (2026-10-13) |
| D-22 | Code host and CI provider | M2 | CI config, release | Open (credentials) |
| D-23 | Platform targets, MSRV policy, crates.io | M2 (MSRV), M24 | M24 | MSRV part decided in T1 (1.90); platforms and crates.io still Open |
| D-01 | Scheduling model | M3 | M3, M7, M8, M10, M15 | Open |
| D-02 | Change detection | M3 | M3, M4 (dirty transforms), M23 | Open |
| D-10 | Events and deferred commands | M3 | M3, M4, M7, M8, M18 | Open |
| D-18 | Scene format, serde, component registry | M4 (registry shape), M19 | M4, M18, M19, M22 | Open |
| D-27 | Units and pixels per unit | M5 | M5, M8, M9, M13 | Open |
| D-05 | Materials, alpha modes, blended meshes | M5 | M5, M14 | Open |
| D-06 | Pixel-art policy (snapping, MSAA) | M6 | M6, M8, M9, M13 | Open |
| D-07 | Depth convention (reverse-Z) | M6 | M6, M14 shadows | Open |
| D-08 | Multi-camera and render targets | M6 | M6, M14, M22 | Open |
| D-25 | Custom projections inside `Camera` | M6 | M6, M13 (oblique tiles) | Open |
| D-26 | Additional sprite orientations | M6 | none hard | Open, low priority |
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
| D-21 | Logging and profiling crates | M20 | M20, M23 | Open |

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
* **Decided 2026-10-13 (owner accepted the proposed default).** `despawn` removes the whole subtree; the
  explicit `detach_children` call keeps children as roots with their world transform. Implemented in T3 (`despawn_recursive` removed).

### D-04 GPU cache lifetime

* **Problem.** The wgpu caches keyed by `AssetId` are never evicted (KI-4), and `AssetManager` has no
  `remove`.
* **Options.** (a) Evict when the asset is removed from the manager. (b) Also evict entries unused for
  N frames. (c) Explicit `release` only.
* **Proposed default.** (a) plus (b) with a small N, so a removed asset is freed promptly and an asset
  that is merely off-screen is not re-uploaded every frame.
* **Needed by.** M2 task T2.
* **Blocks.** Eviction; asset unload for scenes (M4) and hot reload (M17).
* **Decided 2026-10-13 (owner accepted the proposed default).** Both (a) and (b): a cached GPU texture or
  mesh is evicted when its asset is gone from the manager, and when it has not been used for a configurable
  number of frames. Implemented in T2.

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
  deliberately; plus a decision on desktop-only for now vs web/mobile after M24, and whether to publish
  to crates.io.
* **Proposed default.** Pin `rust-toolchain.toml` to a stable release and record the verified MSRV in
  `Cargo.toml` (`rust-version`). Desktop only until after M24. Publishing deferred to M24.
* **Needed by.** MSRV part by M2 (T1). Platform and publishing parts by M24.
* **MSRV part: decided by T1 (2026-10-13).** `rust-version = "1.90"` (lowest version the locked graph allows on
  every target, because of `ordered-float 5.5.0`), verified by `cargo test --workspace --locked` on exactly
  1.90.0 on macOS arm64; `rust-toolchain.toml` pins 1.99.0 for development. Policy followed: the MSRV is the
  lowest version that builds with the committed `Cargo.lock`, and is bumped deliberately, not tracked to
  stable. Not yet built on Linux or Windows (KI-27). The platform and crates.io parts remain open.

## Needed during M3-M4 (input, scheduling, scenes)

### D-01 Scheduling model

* **Problem.** The `Schedule` is a sequential list of `FnMut(&mut World)` and the engine stage order is
  hard-wired in `Game::step` (KI-10).
* **Options.** (a) Named stages with user systems attached per stage and `before`/`after` by name.
  (b) A dependency-graph scheduler with automatic ordering from declared access. (c) Keep sequential
  and only expose stage hooks.
* **Leaning.** (a). (b) needs access declarations that the `RefCell` resources cannot provide yet, and
  there is no parallelism requirement.
* **Needed by.** M3. **Blocks.** M3, and M7, M8, M10, M15 which add stages.

### D-02 Change detection

* **Problem.** `propagate_transforms` visits every entity every frame; nothing can ask "what changed".
* **Options.** (a) None; keep full passes until profiling (M23) shows a cost. (b) A dirty flag on
  `Transform` set through a mutation API. (c) Switch ECS or add a tick-based layer.
* **Leaning.** (a) unless M3 or M4 needs change data for a feature. Profile first (KI-19).
* **Needed by.** M3. **Blocks.** Dirty-transform propagation in M4, any M23 optimisation.

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
* **Needed by.** M6. **Blocks.** M8 and M9 visual quality (frame shimmer), M13 tile seams.

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
* **Needed by.** M6; earlier only if tilemaps (M13) need oblique projection.
* **Blocks.** Custom projections; M13 if it chooses oblique tiles.

### D-26 Additional sprite orientations

* **Problem.** `Billboard`, `Upright`, `Fixed` cover current needs. Axis-aligned billboards or
  camera-position-facing billboards may be wanted.
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

* **Options.** Reuse the engine's own UI runtime (M10); an immediate-mode library inside a dedicated
  crate; a separate application; a web UI.
* **Needed by.** M22, after D-24 and D-18. **Blocks.** M22 only.

### D-21 Logging and profiling crates

* **Options.** `log` plus a backend, or `tracing`; a profiling scope macro with a feature flag.
* **Needed by.** M20. **Blocks.** M20 and M23 measurement.

## Reopening a settled decision

A decision in ARCHITECTURE.md may be reopened only when all of the following hold:

1. A specific failing test, measurement or concrete limitation from the repository is cited.
2. The cost of working around it is stated.
3. The proposed change names what it replaces and which tests must still pass.

"It would be cleaner" or "another engine does it differently" is not sufficient.
