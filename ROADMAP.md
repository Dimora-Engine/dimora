# Dimora roadmap

Dimora is a Rust game engine built around a unified spatial model for 2.5D game development, combining
powerful programming APIs with purpose-built visual tools, including a user-friendly editor. Its central
principle:

> **2.5D is a spatial world, not a fake 2D layer system.** Sprites, skeletal characters, tiles,
> particles and 3D meshes all live in one world with real X/Y/Z, are seen through real cameras, and
> are lit by one lighting system.

It is not a general-purpose engine: features exist if a spatial 2.5D game needs them. The editor is a core
product goal (M22), built on foundations that are designed for it from the start (see "Considered now,
implemented later").

**Document map.** This file *orders* the work. It does not describe what exists or why:

| Question | Where |
|---|---|
| What is Dimora for, and what is out of scope? | [docs/VISION.md](docs/VISION.md) |
| What does each system do today, and what is planned? | [docs/CAPABILITIES.md](docs/CAPABILITIES.md) |
| How is it built, what must hold across systems, what could be expensive to redo? | [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) |
| Which decisions are open, which are made and why? | [docs/DECISIONS.md](docs/DECISIONS.md), records in [docs/adr/](docs/adr/) |
| What is being worked on now, known issues, task acceptance criteria? | [TASKS.md](TASKS.md) |

This roadmap was audited and re-ordered on 2026-10-13 against commit `4857158` (after M2 tasks T1 to T3).
It had been expanded earlier that day to plan three character representations, a UI runtime and gameplay
scripting. That audit's snapshot (about 138 tests, and no scenes, actions, or logging) is historical.
The current state is the list below. Evidence for each claim is in [TASKS.md](TASKS.md).

Status values used below: **Done** (evidence cited), **In progress**, **Not started**. Nothing is marked in
progress or partially done unless TASKS.md shows commits for it. Planning a system here does not mean it
exists: [docs/CAPABILITIES.md](docs/CAPABILITIES.md) is the record of what does.

## 1. Where the project stands

**Complete locally, not pushed.** M0 and M1. M2's numbered tasks T1 to T3, with licence files (KI-17) and
the owner's confirmation of GitHub as host still carried. M3a and M3b. M4, through the lamp prefab and the
crate-private scene helpers, at commit `2b79806`.

**In progress.** M6 has one verified piece: the `spatial_scene` camera-configuration prototype (commit
`6dbabb8`, tests in `examples/spatial_scene/tests/configurations.rs`). Custom projection, multiple
viewports, camera-to-texture, tilemap chunking, and the performance baseline are not done. M6 is not
pulled into M5.

**Next implementation.** M5, materials and textures. M5-1 records the decisions (ADR-0015, ADR-0016,
ADR-0017). No M5 renderer, texture, material, sampler, or atlas code exists yet.

**Implemented and tested, beyond the 2026-10-13 audit.** Named stages, commands and events, the `log`
crate and `Diagnostics`, actions and window controls, scene load/unload/replace on the command flush,
hierarchical visibility, name and tag lookup, the component registry and text dumps, and one code-defined
prefab. The last recorded workspace check is in TASKS.md under M4-5 (365 tests). This introduction does
not add a new test run.

**Still partial.** Tilemaps have no chunking. Skeletal animation is a data model and one player. Assets
are PNG and synchronous. Materials are an inline colour, one albedo, and a lit flag. Textures are RGBA8,
sRGB, clamp, and nearest or linear, with no mipmaps.

**Still absent.** Collision, sprite-sheet animation, 3D characters, UI, audio, particles, shadows,
scripting, serialization, a general importer, bundles, an editor, and packaging. CI exists as a workflow
(ADR-0005); the host confirmation is still the carried M2 item.

Per-system status, evidence and limits are in [docs/CAPABILITIES.md](docs/CAPABILITIES.md). The audit that
produced this order found that the largest long-term risks are not missing features but a few
cross-system shapes (asset identity, the render scene's structure, the mutation and threading model,
public API and format promises); the risk register is in
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md#cross-system-requirements-and-risks) and drives the "decide
first" tiers in [docs/DECISIONS.md](docs/DECISIONS.md#what-to-resolve-first).

### Current milestone and immediate scope

* **M2 is closed for planning.** T1 to T3 are done. Two items remain, both blocked on the owner rather
  than on code: licence files (KI-17) and confirmation of GitHub as host. CI itself exists (ADR-0005).
  Neither item blocks later milestones.
* **M3a and M3b are complete locally and are not pushed.** Named stages, commands and events, logging,
  opt-in interpolation, and the public-API review are done. M3b's four phases are checkpointed locally
  (`57dbc4a`, `83286a9`, `9c99edf`, `cfdf358`).
* **M4 is complete locally at `2b79806` and is not pushed.** The registry and dumps are `cdb5ec3`.
  Hierarchical visibility is `2c76f71`. Name and tag lookup is `f9a4aab`. Scene identity is `302622b`.
  Synchronous load is `128c70e`. Unload and replace are `e12fb5b`. Deferred scene operations are
  `8398e49`. The `spatial_scene` switch is `d58c66b`. The lamp prefab and the crate-private scene helpers
  are `2b79806`. Scene files stay in M19. D-16 stays open.
* **Now: M5, materials and textures.** Decisions are recorded (ADR-0015, ADR-0016, ADR-0017). M5-2A,
  the CPU texture descriptors, is committed locally as `c687d5b` and is not pushed. M5-2B, GPU formats
  and the sampler cache, is commit `11b914f` and is not pushed. M5-2C, GPU mip generation, is
  commit `622ef80` and is not pushed. M5-3A, bounded CPU region transitions, is commit `b51f1fb` and
  is not pushed. M5-3B, GPU rectangle uploads and whole-chain mip rebuilds, is in the working tree and
  is not committed. Localized mip regeneration and the atlas packer have not started. The milestone is
  in progress and is not complete.
* **M6 stays after M5.** The camera-configuration prototype is verified. The rest of M6 is not started.
  M7 and M8 can proceed in parallel after M3a, M4, M5, and M6 as stated below. M9 freezes the foundational
  gameplay, scene, and component-integration contracts. Later milestones may add public APIs through a
  recorded decision. That freeze does not forbid every future public type.
* **Considered now, built later:** see "Considered now, implemented later" after the dependency graph.

## 2. Ordering rules

1. Correctness of existing foundations (M2).
2. Small end-to-end working features (M3-M8).
3. A playable example game, and the stable APIs it proves (M7, M9).
4. Subsystems that sit on those APIs: UI, the other character representations, distinctive 2.5D
   functionality (M10-M16).
5. Asset, scripting and developer workflows (M17-M21).
6. Advanced tools and production readiness (M22-M24).

Order follows technical dependency, not the length of the list. Where two systems do not depend on each
other they are marked as parallel tracks below and either order is fine.

Do not create a crate for a subsystem until it has code. New crates in this plan: none before M10
(a UI crate, created when it has code), M18 (`dimora-script`), M21 (`dimora-assets` compiler CLI) and M22
(`dimora-editor`). Audio (M16) and a glTF importer (M12) get a module or crate only when the milestone
starts, and only if the dependency boundary needs one.

### Three character representations

A character is one gameplay entity (transform, collider, gameplay state) with a visual subtree. The
representation of that visual subtree is replaceable (D-28, ARCHITECTURE.md). All three live in the same
world and the same depth buffer.

| | A. Sprite sheet | B. Skeletal 2D | C. 3D skinned |
|---|---|---|---|
| Milestone | M8 | M11 | M12 |
| Data | Frames from a grid or irregular atlas, clips, directions | Bones, rest pose, sprite parts, clips | Skinned mesh, 3D skeleton, imported clips |
| Needs a skeleton | No | Yes | Yes |
| Evaluator | Frame selection, writes `Sprite` | Pose sampling, blending, IK, writes bones | Same pose machinery as B, plus skin palette |
| Shared with the others | Control vocabulary only (clip name, speed, loop, parameters, events) | Pose, blending, IK maths with C | Pose, blending, IK maths with B |
| Gameplay | Identical for all three: M9 character API |  |  |

### How the requested order was applied

| Requested step | Milestone | Note |
|---|---|---|
| 1 Spatial renderer and world foundations | M0, M1 (Done); M6 (In progress: test scene only); M2, M4, M5 | Hardening, scenes, materials and renderer limits come first |
| 2 Input and update scheduling | M3 | Moved before scenes and materials: collision, animation and scenes all need defined stages and events |
| 3 Collision and basic gameplay | M7 | |
| 4 Sprite-sheet animation | M8 | **Parallel with M7**, not after it: it does not need collision. Both feed M9 |
| 5 Stable character and scene APIs | M9 | Delivered together with a playable vertical slice, because an API with no consumer is not known to be stable |
| 6 Basic UI runtime | M10 | After M9 for the bindings; layout, text and rendering can start after M6 |
| 7 Skeletal 2D animation | M11 | Parallel with M10 |
| 8 3D characters | M12 | After M11 because it reuses the pose, blending and IK work. Contains a **minimal glTF importer** (pulled forward from step 9): a skinned character cannot be built or tested without model data |
| 9 Asset importing and compilation | M17 importers; M21 compiler | **Split**: importing generalises M12's importer and gives scripts a reload path; the compiler waits for scene files (M19) |
| 10 Audio, lighting, shadows, particles | M14 lighting and shadows, M15 particles, M16 audio | Separate milestones; independent of each other. Lighting comes after all three character types so shadows handle all of them. Audio is before scripting because scripts call it |
| 11 Gameplay scripting | M18 | After M9, M3, M4, M10, M16, M17. **Before** scene serialization, so the scene format can hold script components from the start |
| 12 Scene serialization and developer tools | M19, M20 | |
| 13 Editor (first usable version) | M22 | A core product goal; its foundations are designed in from M3 |
| 14 Production readiness | M23, M24 | |
| Not in the list | M13 tilemaps | After M6's chunking and M7. Reuses that chunking; it does not start a second one |

The earlier 17-stage list is covered as follows: stages 1 to 6 as above; 7 skeletal animation is M11;
8 tilemaps M13; 9 lighting and shadows M14; 10 particles M15; 11 asset management M17; 12 compiler M21;
13 serialization M19; 14 debugging M20; 15 editor M22 (first version); 16 performance M23; 17 packaging M24. Two
milestones are additions: **M2** (known defects and a reproducible build before building more) and **M9**
(the integration point for gameplay, character and scene APIs).

### Parallel tracks

After the stated prerequisites, these can be built independently and in any order:

| Track | Milestones | Joins at |
|---|---|---|
| Gameplay | M7 collision | M9 |
| Sprite-sheet characters | M8 | M9 |
| Skeletal 2D | M11 | M12 (shared pose machinery), M14 |
| UI layout, text, rendering | M10 (from M6) | M9 for game bindings |
| Tilemaps | M13 | M17 |
| Particles | M15 | M20 |
| Audio | M16 | M18 |

Things that must not be parallel: M2 before everything; M3 before M4, M7, M8; M9 before M18; M11 before
M12; M19 before M21; M17 and M19 before M22.

### Dependency graph

```text
M2 hardening/CI
 ├─► M3 input + scheduling ──┬─► M4 scenes ───────────────┐
 │                           ├────────────► M8 sprite sheets ──┐
 └─► M5 materials/textures ──┴─► M6 renderer stabilization    │
        │ (atlas, sampler)           │ debug draw             │
        │                            └─► M7 collision ────────┴─► M9 character/scene API + playable slice
        │                                                         │ (the API freeze point)
        │      ┌──────────────────────────┬──────────────────────┼───────────────────────┐
        │      ▼                          ▼                      ▼                       ▼
        │  M10 UI runtime        M11 skeletal 2D ─► M12 3D     M13 tilemaps       (M8, M11, M12
        │  (layout/text/render            characters (+glTF)    (also M6, M7)       feed M14)
        │   may start after M6)           │
        │      │                          ▼
        └──────┴───────────────►  M14 lighting+shadows   M15 particles   M16 audio   (parallel)
M5 + D-16 ─► M17 core identity and loading
M17 importers follow their content: M8 sheets, M10 fonts, M11 clips, M12 glTF (generalised, not a second parser), M13 tilemaps, M16 audio
M3 + M4 + M9 + M10 + M16 + M17 ─► M18 gameplay scripting
M4 + M17 + M18 ─► M19 scene serialization ─► M21 compiler/bundles
M4 registry + M6 debug draw + M10 + M19 serialization ─► M20 debug tools ─► M22 editor
M21 + M22 ─► M23 perf/stability ─► M24 packaging/release
```

### Incremental developer tooling

Owner direction ([ADR-0003](docs/adr/0003-incremental-tooling-editor-later.md)): the full editor stays at
M22, but useful tools are delivered where they first pay off, so the engine is built with the tools it will
need. Each tool reads data the runtime already exposes (registry, diagnostics, events), is read-only first,
and contains no editor-specific state in the runtime. Logging and runtime diagnostics are committed
locally (M3a-4, not pushed). The later rows are not built.

| Tool | First delivered | Content | Depends on |
|---|---|---|---|
| Logging and error reporting | M3a | **Done (M3a-4, committed locally, not pushed).** `log` crate; asset failures, skipped renderables, lost surfaces and render errors named once with the cause | D-21 |
| Runtime diagnostics | M3a | **Done (M3a-4, committed locally, not pushed).** `Diagnostics`: per-stage CPU timings, frame delta, fixed-step count, entity count, `RenderStats` and cache counts, printed by the examples | M3a stages |
| Scene and entity inspection | M4 | **Text dump done (M4 Phase 1, local, not pushed).** `dump_hierarchy` and `dump_entity` list runtime ids, names, parent links, registered component names and fields, and unregistered `TypeId`s. No visual inspector | ADR-0014 |
| Asset validation | M5 (textures, atlases), M17 (importers, dependency graph), M21 (bundle-time) | Reports with the asset name: limits, missing or unused references, malformed data | D-16, D-17 |
| Render debugging | M6 (debug draw, depth view, camera frusta), M14 (light and shadow views) | Minimal primitives drawn through the real renderer | M6 |
| On-screen overlay | M10 | Frame time, draw calls and log lines through the UI runtime | M10 |
| Consolidated debug tools, interactive inspector, profiling | M20 | Edit-capable inspector, profiling scopes, collider and skeleton overlays | M19 |
| **Editor** | M22 | See the stability gate under M22 | Gate below |

### Considered now, implemented later

Each row is a capability nobody builds yet whose *shape* constrains work in the next few milestones. The
"consider now" column is a design constraint or a decision, never a task to implement early.

| Future capability | Built in | Consider now, because | Where it is recorded |
|---|---|---|---|
| Scene and asset serialization | M19 | Components must stay plain data (no GPU objects, no cached derived values); asset references must be restorable to a stable identity; entity references need remapping | ARCHITECTURE.md requirement 8; D-16, D-18, D-47 |
| Hot reload | M17 | A reload takes effect at a stage boundary; consumers resolve assets per use instead of caching derived data in components | Requirement 6; D-16 |
| Scripting | M18 | Commands, events and a component registry must exist first; scripts get a small versioned surface, not `hecs` or renderer types | Requirements 7 and 9; D-40 to D-45 |
| Multiple cameras, render targets and a custom projection | M6 | Six views are demonstrated by example-local constructors in `spatial_scene` (2026-10-14). `RenderScene` still holds one view and `Camera` a closed projection enum; do not add features that assume one view, one projection or one ground plane | R-2, R-16; D-08, D-25, D-48; ADR-0004 |
| Blended meshes, particles, skinned characters | M5, M12, M15 | They need a transparency class in the render model before they exist | R-1; D-05 |
| Fixed-step interpolation | M3a | **Done (M3a-5, committed locally, not pushed).** Opt-in, extraction-only, never writes `Transform` or `GlobalTransform` (ADR-0007) | R-3 |
| UI in screen and world space | M10 | Window scale factor, sub-rectangle texture update and an overlay pass; M3b and M5 carry the prerequisites | R-12, KI-24, KI-25; D-24, D-37 to D-39 |
| Three character representations | M8, M11, M12 | One gameplay entity with a replaceable visual subtree; gameplay must not name rendering types | D-28 to D-34 |
| Save games | M18 to M24 | Not a scene file; needs its own versioned format and a state store shaped alongside the script API | D-46, D-47 |
| Public API stability | M9 | Every re-exported type is a promise. M3a-6 stopped adding public `hecs` types; the inventory is still M9 | R-7; D-45 |
| The editor (viewport, inspector, undo, play-in-editor), behind a stability gate | M22 | The runtime stays free of editor state; components must be named, plain, serializable data with field metadata; mutations go through undoable commands; entities and assets need stable ids; the editor's viewport is the real renderer, so it needs camera-to-texture and picking through queries | ARCHITECTURE.md requirement 7 and R-15; D-08, D-10, D-18, D-20 |
| Gamepad, localisation, accessibility | later | Action maps and text and layout decisions can foreclose them if they assume keyboards, ASCII or fixed pixels | CAPABILITIES.md, additional categories |
| Cross-platform determinism | M9 | Replay equality is promised on one platform only; platform trigonometry can differ | R-9 |

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

## M2. Foundation hardening and CI: Substantially done (T1 to T3 done; licence files and CI remain, see TASKS.md)

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
  leaves dangling children (policy from D-03). Done (T3).
- Asset-name collision check on insert (two names hashing to one id is an error, not an overwrite).
  Done (T3), per asset type.
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

**Deferred.** Coverage tooling, multi-platform CI matrix (M23), release automation (M24).

---

## M3. Frame model, input and platform: Complete locally, not pushed

**Objective.** A deterministic, documented frame with ordered stages, safe mutation and events, and an
input and platform layer sufficient for a real game. Delivered in **two slices** so each can be completed
and verified: M3a (the frame model; the immediate milestone) and M3b (input and platform). M3b may start
once M3a's stage and event API is merged; it does not need M3a's interpolation.

**Status.** M3a is complete locally and is not pushed. M3a-3 is `cd802f0`, M3a-4 is `5c4fd97`, M3a-5 is
`523f553`. M3a-6 is the public-API review in this documentation. M3b Phase 1 (keys, mouse release, and
collapsed modifiers) is checkpointed locally as `57dbc4a` and is not pushed. Phase 2 (the fixed-step
latch and `Actions`) is checkpointed locally as `83286a9` and is not pushed. Phase 3 (`Viewport`
physical size, logical size, and display scale) is checkpointed locally as `9c99edf`. Phase 4
(borderless fullscreen, vsync, frame cap, cursor grab, and focus-loss cleanup) is checkpointed
locally as `cfdf358` and is not pushed. The M3b acceptance criteria below are met. Profiling
is not in the code. The rendering regression baseline is commit `6dbabb8`.

**Prerequisites.** M2 (closed for planning). Decisions recorded before implementation code, 2026-10-14:
D-44 ([ADR-0006](docs/adr/0006-simulation-stays-single-threaded.md)), D-01
([ADR-0007](docs/adr/0007-named-stages-on-the-existing-schedule.md)), D-10
([ADR-0008](docs/adr/0008-commands-and-per-frame-events.md)), D-02
([ADR-0009](docs/adr/0009-no-change-detection-until-measured.md), Provisional: none until profiled), D-45
([ADR-0010](docs/adr/0010-three-tier-public-api.md), rule and M3a-6 review), D-47
([ADR-0011](docs/adr/0011-persisted-formats-carry-a-version.md)), D-21 logging
([ADR-0012](docs/adr/0012-logging-uses-the-log-crate.md)). Profiling stays open for M20.

### M3a. Frame model

**Tasks.**
1. **Design note and ADRs** — **done (2026-10-14).** ADR-0006 through ADR-0012. A throwaway command-buffer
   program was run against the `RefCell` resources and then deleted; findings are in ADR-0008 and
   TASKS.md. No engine code was merged.
2. Named stages — **done (M3a-2).** `time`, `loads`, `fixed_update` (0 to 8), `update`, `animate`,
   `post_update`, `input_clear`. Extract and render stay outside `step`. User systems attach to a stage,
   with `before` / `after` by name. Both examples stay in `update`. Fixed systems see the fixed step;
   the frame delta is restored afterward (ADR-0007).
3. Deferred command buffer and typed events — **done (M3a-3).** FIFO commands flush at
   the end of each stage run and between fixed iterations. Typed events become visible at the next
   stage boundary and clear after `input_clear` (ADR-0008). Later consumers: collision (M7), animation
   events (M8, M11), UI (M10), scripts (M18). Those consumers are not built.
4. Logging and diagnostics — **done (M3a-4, committed locally, not pushed).** The `log` crate (ADR-0012). A failed asset
   load, a skipped renderable, and a lost surface or render error each log once with the asset or cause
   named. `Diagnostics` holds per-stage CPU timings, the frame delta, the fixed-step count, and the
   existing render and cache counts. Examples print a few of those numbers. Profiling stays in M20.
5. Fixed-step alpha and opt-in transform interpolation — **done (M3a-5, committed locally, not pushed).**
   `FixedTimestep::alpha` is a finite fraction in `0.0..=1.0`. `Interpolate` stores one previous
   local, copied at the start of each fixed iteration. Extraction blends it toward the current local
   and does not write `Transform` or `GlobalTransform` (R-3, ADR-0007). Entities that do not opt in,
   including both examples, are unchanged.
6. Stop adding public `hecs` types — **done (M3a-6).** The three-tier rule is ADR-0010. The review
   found no new public `hecs` type, no new `hecs` re-export, and no `wgpu` or `winit` type on a new
   public interface. Existing leaks stay. The full list waits for M9.

**Dependencies.** D-44, D-01, D-10, D-02, D-45, D-47, D-21.

**Acceptance criteria.**
- A test registers systems out of order across stages with `before`/`after` and observes the
  documented order; an impossible ordering (cycle) is an error, not a hang.
- Commands queued while iterating a query apply at the stage end (test).
- An event sent in one stage is readable by a system in a later stage of the same frame and gone the next
  frame (test).
- A fixed-step entity moved at 60 Hz with a 144 Hz render rate produces monotonically increasing
  interpolated positions in successive extracted scenes (test through `Game::step` + `extract`), and a
  system reading `Transform` in the fixed stage sees the un-interpolated value. Met by M3a-5
  (`523f553`): `sixty_hertz_motion_sampled_at_one_hundred_forty_four_hertz_stays_behind_and_smooth`.
- A failed asset load produces exactly one log record containing the asset name (test with a capturing
  logger).
- Existing behaviour is unchanged: `scripts/check.sh` passes and the headless screenshots of
  `basic_2_5d` and `spatial_scene` (perspective and `--ortho`) are byte-identical to those from the
  commit before M3a.
- ADRs for D-44, D-01, D-10, D-45, D-47 and D-21 exist, and D-02 is either decided or recorded as
  "deferred to profiling".

**Tests/examples.** Scheduler unit tests, command and event tests, interpolation test, logging test; both
examples ported to the stage API (behaviour unchanged).

### M3b. Input and platform

**Status.** Complete locally, not pushed. The decisions are ADR-0013 (D-49). Phase 1, the wider key map,
`mouse_released`, and collapsed modifiers, is checkpointed locally as `57dbc4a`. Phase 2, `Actions`
and the fixed-step latch, is checkpointed locally as `83286a9`. Phase 3, `Viewport` and DPI, is
checkpointed locally as `9c99edf`. Phase 4, borderless fullscreen, vsync, the frame cap, cursor grab,
and focus-loss cleanup, is checkpointed locally as `cfdf358`. None of M3b is pushed.

**Tasks.**
- Input: full key set (F-keys, punctuation, numpad) — **Phase 1, checkpointed locally.** Cursor
  grab/visibility and relative mouse delta while grabbed are **Phase 4, checkpointed locally.**
  Relative motion is unit-tested. A macOS window confirmed Locked grab; posted motion did not arrive
  (the process was not accessibility-trusted). An action map (named actions and axes bound to keys
  and buttons) that does not assume a keyboard, so a gamepad can bind later, and the fixed-step
  latch, are **Phase 2, checkpointed locally.**
- Platform: forward the window scale factor and its changes; `Viewport` carries physical size, logical
  size and scale (moved here from M10: R-12) — **Phase 3, checkpointed locally as `9c99edf`.**
  Borderless fullscreen, vsync on/off with a capability fallback, and an optional frame cap are
  **Phase 4, checkpointed locally.** Exclusive fullscreen is not in this milestone.

**Dependencies.** D-10 (input events through the M3a channels), D-39 (scaling model, in outline).

**Acceptance criteria.**
- Action map unit tests cover press, release, axis and rebinding.
- A scale-factor change produces a consistent `Viewport` (physical, logical, scale) and pointer
  coordinates in logical units (test on the pure mapping function; a manual check on a HiDPI display).
- `spatial_scene` uses actions instead of raw keys.
- A manual checklist for fullscreen, vsync and cursor grab exists in README or TASKS and was run on at
  least one OS.

**Tests/examples.** Action map tests, mapping tests, updated examples.

**Deferred (both slices).** Parallel system execution, gamepad, touch, text input/IME, rebinding UI,
multi-window.

---

## M4. World and scene management: Complete locally (not pushed)

**Objective.** A scene is a unit that can be built, switched and unloaded without leaking or
orphaning entities, and the world knows its component types by name.

**Prerequisites.** M2 (hierarchy fixes), M3 (stages, commands).

**Tasks.**
- Scene membership (`SceneMember`), `SceneManager` resource with load/unload/replace applied at a
  stage boundary.
- Hierarchical visibility (a hidden parent hides its subtree).
- Name/tag lookup.
- A component registry mapping stable names to component types (no serialization yet); this is the
  groundwork M18, M19 and M20 need. Leave room for a per-component, per-field visibility attribute that
  scripts will use (D-41); no scripting code here.
- Code-defined prefabs (`fn(&mut World, ...) -> Entity` by convention) and one documented pattern. **Met by M4-5** (`2b79806`), not pushed. `spawn_lamp` returns the post. `place_lamp` is the `SceneBuilder` form.
- Read-only scene and entity inspection: a text dump of the hierarchy and of one entity's registered
  components and fields, built on the registry (first of the inspection tools; no editor code).

**Dependencies.** D-03, D-10, D-18 (registry shape, [ADR-0014](docs/adr/0014-scene-model-and-component-registry.md)).

**Phase status.** Phase 1 (registry, built-in registration, text dumps) is commit `cdb5ec3`, local, not
pushed. Phase 2 (hierarchical visibility in extraction) is commit `2c76f71` and is not pushed.
Phase 3 (name and tag lookup) is commit `f9a4aab` and is not pushed. M4-4A (empty instances,
`SceneMember`, and validated lookup) is commit `302622b` and is not pushed. M4-4B (`SceneBuilder` and
synchronous `load_scene`) is commit `128c70e` and is not pushed. M4-4C (`unload_scene` and
`replace_scene`) is commit `e12fb5b` and is not pushed. M4-4D queues those operations on the existing
command queue and is commit `8398e49`. M4-4E switches `spatial_scene` between two code-defined
environments and is commit `d58c66b`. M4-5 documents `spawn_lamp` and makes `create_scene_instance`
and `track_scene_entity` crate-private. M4-5 is `2b79806` and is not pushed. M4 is complete locally
at that commit and is not pushed.
Scene files stay in M19. D-16 stays open.

**Acceptance criteria.**
- Loading then unloading a scene returns entity count and GPU resource counts to baseline (test). **Met by M4-4C** for the entity count. GPU texture and mesh counts return to the camera baseline after unload plus an explicit `AssetManager::remove` of assets that test owns. Unload itself does not remove assets. D-16 stays open.
- A hidden parent hides all descendants in the extracted scene (test). **Met by Phase 2.** Cameras still activate from `Camera.active`.
- Despawning any entity cannot leave a dangling `Parent` (test). **Met** by `World::despawn` subtree removal and the lamp-root despawn test.
- `spatial_scene` switches between two scenes at runtime (test). **Met by M4-4E** (`d58c66b`). The camera and the walker stay. One Tab press queues one replacement.
- The dump of a known world lists each entity's registered components and fields, and a component that is
  not registered is reported as such, not omitted silently (test). **Met by Phase 1.**
- Exact name and tag lookup returns every live match in `Entity` order (test). **Met by Phase 3.**
- A scene entity is found by instance id and scene-local id only when that entity is alive and its `SceneMember` matches (test). **Met by M4-4A.**
- Loading a code-defined scene publishes an instance only when the build returns `Ok`, and a failed build leaves no created entities (test). **Met by M4-4B.**
- Unloading a scene removes its validated members and leaves a non-member child at a TRS approximation of its world transform (test). **Met by M4-4C.**
- A scene operation queued from a system runs at the existing stage flush, and its result stays readable after that flush (test). **Met by M4-4D.**

**Tests/examples.** Scene lifecycle tests; `spatial_scene` scene-switch demo.

**Deferred.** Async/streamed scenes, world partitioning, multiple worlds, serialization.

---

## M5. Materials and textures: In progress

**Objective.** A small but real material and texture model that normal maps, shadows and the asset
pipeline can build on without rewriting the renderer. It serves every camera configuration and visual
style ([ADR-0004](docs/adr/0004-one-spatial-world-many-camera-configurations.md)): nearest and linear
filtering are equal first-class choices, and nothing here is specific to pixel art or to a perspective view.

**Prerequisites.** M2.

**Tasks.**
- Texture: mipmap generation, sampler parameters (filter, wrap, anisotropy), a colour-space flag
  (sRGB vs linear) so data textures are possible.
- Runtime sprite atlas builder (shelf packing) with padding/extrusion. Sprite sheets and their animation
  are M8, not here.
- Update a sub-rectangle of a texture without re-uploading the whole texture (today any change re-uploads
  it all, KI-25). The first consumer is the UI glyph atlas (M10).
- `MaterialAsset` (base colour, albedo, reserved normal-map slot, alpha mode opaque/mask/blend, lit,
  double-sided) referenced by handle from sprites and meshes; batching keyed by pipeline and material.
- Mesh generators: plane, sphere, cylinder.
- Policy for blended meshes (D-05), expressed as a transparency class that does not depend on the
  projection (R-1).
- Asset validation, first slice: texture and atlas loads report size beyond device limits, a missing
  texture and a zero-sized region with the asset name (logging from M3a; see "Incremental developer tooling").

**Dependencies.** D-05 ([ADR-0015](docs/adr/0015-material-alpha-classes.md), settled, not implemented;
blended-mesh depth is the camera-space Z of the world-space bounding-sphere centre), D-27
([ADR-0016](docs/adr/0016-per-sprite-pixels-per-unit.md)). Texture sampling, colour space, partial
updates, and atlas mip limits are
[ADR-0017](docs/adr/0017-texture-sampling-colour-space-and-atlases.md), including the corrected
anisotropy fallback, `write_texture` alignment, split texture and sampler caches, and the provisional
atlas estimate. M5-2A stores the CPU descriptors (`c687d5b`). M5-2B (`11b914f`) connects those
descriptors to the wgpu renderer. M5-2C (`622ef80`) generates mip chains for `Mipmaps::Generate`.
M5-3A (`b51f1fb`) records CPU region updates, validation, and a bounded transition journal. M5-3B,
in the working tree and not committed, uploads a proved rectangle and regenerates the whole allocated
mip chain. It does not pack an atlas and does not regenerate only the affected mip texels.

**Acceptance criteria.**
- GPU test: a minified high-frequency texture approaches its mean colour with mipmaps, and aliases
  without them.
- The mipmap and filtering tests also run under an orthographic camera whose scale changes with zoom.
  Perspective minification is not the only case.
- GPU test: a linear-format texture reaches the shader without sRGB decoding. Tolerances are stated with
  the test. Colour space is per texture. Linear `Color` is not converted a second time.
- Atlas packer unit tests: no overlap, padding and edge extrusion respected, deterministic UVs, a
  zero-sized region named as an error. Packing is in texels and does not set world size. GPU acceptance
  for the atlas covers adjacent regions of strongly contrasting colour, nearest and linear filtering,
  several mip levels, padding and extrusion widths, the returned UV boundaries, and no neighbour sample
  inside the mip range those tests declare supported. The `floor(log2(padding))` note in ADR-0017 is a
  design estimate. It does not reject a pack and it is not a guarantee. Whole-atlas mip generation can
  mix regions at coarse levels because each level halves the padding.
- GPU test: a sub-rectangle update changes only those base-level pixels and uploads only that region's
  base-level bytes, plus the mip levels that rectangle affects. Mip maintenance is required and is not
  counted as a full-image upload. Bytes outside the rectangle stay.
- 1,000 sprites that are compatible render in one instanced draw. Compatible means the same pipeline
  configuration, material identity and revision, texture and sampler bindings, geometry, and instance-data
  layout, and a submission order that the depth and transparency rules do not split. Sharing a material
  alone is not enough. Blended objects are not reordered to form the batch.
- Alpha classes and the blended sort use one policy under a perspective camera and under an orthographic
  camera rotated to an isometric orientation. Each camera has its own expected pixels. The pictures may
  differ because the view direction changes depth order. Neither test may depend on a projection-specific
  branch. These tests do not replace the four screenshot baselines, and they do not retune the three
  coplanar conflicts recorded for M6.
- The four release screenshots against commit `6dbabb8` stay byte-identical. Baselines are not replaced
  to make a test pass.

**Tests/examples.** Above. M5-2's texture tests are specified in TASKS.md and are not written yet.

**Deferred.** PBR, material graphs, custom shader authoring, a shader compiler, texture compression,
texture streaming, normal-map shading (M14), a second renderer, a second world. Pixel snapping stays in
M6 (D-06). Coplanar ordering stays in M6 (D-48).

---

## M6. Renderer stabilization: In progress

**Objective.** Remove renderer limits that would otherwise become baked into later features, make the
unified world demonstrably usable through several camera configurations, and establish a measured performance
baseline. Camera projection stays separate from gameplay orientation and visual style
([ADR-0004](docs/adr/0004-one-spatial-world-many-camera-configurations.md)).

**Prerequisites.** M5.

**Tasks.**
- Custom projections first: let `Camera` hold more than the two enum variants so oblique and custom
  projections are possible without a second code path (D-25).
- Render every active camera in `order`, each with its own viewport and clear/load behaviour (D-08).
- Camera-to-texture render targets.
- **Camera-configuration test scene. Done and verified (2026-10-14).** `spatial_scene` is one world (sprites
  at several depths, overlapping and coplanar decals, cutout foliage, meshes, a ground, a moving sprite)
  viewed through six example-local constructors: perspective, orthographic, isometric (orthographic, yawed and
  pitched), top-down, steep and side. Each returns a `Camera` and a `Transform` and stays editable; isometric
  is not a projection type; the scene prescribes no visual style. Evidence is in TASKS.md. **Not done, and
  not an engine feature:** an engine-level preset API, and the custom projection (D-25).
- Camera configurations decision (D-48), mechanisms only: the owner has decided the principles (ADR-0004,
  2026-10-13: gameplay orientation is data and not derived only from the camera or sprite; spatial depth,
  transparency sorting, render layers or queues and explicit order stay distinct; no occlusion framework until
  a scene needs one). The `+Y` up / `XZ` ground convention is kept as a provisional engine convention. The
  test scene has recorded three coplanar conflicts and the current `Upright` behaviour (SPATIAL_RENDERING.md).
  **Still to decide, and not chosen here:** the coplanar ordering mechanism, and whether `Upright` should keep
  that steep-view behaviour (D-26). Gameplay-orientation components wait for M7 and M8 requirements. Decide
  the first two before M7 and M8 build on them.
- Tilemap chunking, the rendering foundation only: chunk-based extraction, cached render instances,
  dirty-chunk invalidation, and an update that touches the affected chunks. Production tile APIs stay in
  M13, which reuses this chunking and does not build a second one.
- Render debugging, first slice: debug draw (lines, boxes, spheres, rays, camera frusta; depth-tested) and a
  depth-visualisation view, as the minimal debugging primitives for M7.
- Depth convention (D-07). Pixel-art policies (D-06): texture-level filtering, and texel snapping at whichever
  level the chosen technique needs, validated by a prototype before any API is fixed. It is **lowest priority here** and never a default; no other configuration may depend on it.
- Benchmarks (a small timing binary, no new dependency) for extraction and render CPU time with N
  sprites/meshes/tiles; record results and budgets in `docs/PERFORMANCE.md`.

**Dependencies.** D-25, D-08, D-48, D-07; D-06 (opt-in only).

**Acceptance criteria.**
- Two cameras with different viewports render into one target (GPU pixel test).
- A camera rendered to a texture and sampled by a sprite appears correctly (GPU test).
- Changing one tile re-extracts one chunk (test); a 1000x1000 map's per-frame extraction cost is
  recorded and bounded by chunk count rather than tile count. This is the chunking foundation. Flip,
  animation, collision, and layers are M13 acceptance, on top of these chunks.
- `docs/PERFORMANCE.md` contains baseline numbers with the machine they were measured on.
- The configuration test scene renders under each named configuration (perspective, orthographic, isometric,
  top-down, side view, a custom projection) with headless GPU assertions that hold in all of them: a nearer
  mesh occludes a farther sprite and the reverse, a blended sprite interleaves with a mesh by depth, a world
  point projects to the pixel `CameraView::project` reports, and a picking ray through that pixel returns to
  the point. A camera-configuration table in `docs/SPATIAL_RENDERING.md` records what each configuration
  guarantees and what it does not.
- Each preset returns a `Camera` and a `Transform` that can be edited afterwards, and no preset adds an enum
  variant, mode or code path to extraction or the backend.
- Switching configuration changes only the camera's `Transform` and projection. A test or review shows no
  configuration-specific branch in extraction or in the wgpu backend.
- A custom projection supplied by game code renders and picks correctly (GPU test and picking test).

**Met by the test scene, and only that part (2026-10-14).** The six example configurations render one world.
GPU tests hold in all of them for mesh/sprite occlusion both ways, and for projecting a point and picking
back through that pixel. The configuration table is in `docs/SPATIAL_RENDERING.md`. This change does not edit
extraction or the wgpu backend. **Not met:** custom projection; two viewports; camera-to-texture; chunking;
benchmarks; picture-in-picture. The blended-sprite-versus-mesh assertion was not repeated per configuration.
The constructors are example-local, so the engine-preset criterion is not met either.

**Tests/examples.** `spatial_scene` has `--camera <name>`. Picture-in-picture is still to do. The GPU tests
for the scene are `examples/spatial_scene/tests/configurations.rs`.

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
- Colliders attach to the gameplay entity, not to a visual child, so a change of character representation
  (M9) cannot change collision.

**Dependencies.** D-09, D-10.

**Acceptance criteria.**
- Shape-pair overlap tests over a table of cases; raycast tests per shape.
- A character moving at high speed cannot tunnel through a thin wall (test) and slides along it.
- Simulation is deterministic: the same inputs over 1,000 fixed steps give an identical state hash.
- Two colliders with equal X/Y and different Z do not collide.
- Collision and movement do not read the camera: the same inputs give the same state hash under two camera
  configurations, and a constrained 2D-style movement plane (D-48) is expressed as a movement option, not a
  separate controller.
- `spatial_scene`: the walkable figure is blocked by the pillar.

**Tests/examples.** Collision unit/property tests; controller test; example.

**Deferred.** Rigid-body dynamics, joints, continuous collision for dynamic bodies, convex-mesh
colliders, navmesh.

---

## M8. Sprite-sheet animation: Not started

**Objective.** Characters and props animated by selecting frames from sprite sheets or atlases, with
no skeleton, cheap enough for many on screen. This is character representation **A**.

**Prerequisites.** M3 (animate stage, event channel), M5 (atlas builder, sampler control). It does **not** need M7: collision and sprite-sheet animation are independent and can be built
in parallel. D-48 (orientation per view) is decided first, so directions are not written for one camera only.

**What exists today (read, not tested for this purpose).** A sprite can already show part of a
texture: `Sprite.region`, `Sprite.size` and `Sprite.anchor` are per-sprite overrides and `SpriteAsset`
carries uv/pivot/pixels-per-unit. So frame selection needs no renderer change. One trap: the natural
size of a sprite comes from the whole asset, not from `region` (`extract.rs`), so an animator must
write `size` and `anchor` together with `region` or frames are stretched to the sheet's size (KI-21).
There is no sheet, clip, animator, event or direction concept. The existing `AnimationClip` and
`AnimationPlayer` are skeletal (bone tracks) and `animate_system` only visits entities that have a
`SkeletonInstance`; sprite-sheet animation must not reuse or bend them.

**Tasks.**
- `SpriteSheet` asset: a texture plus frames. Two builders: grid (cell size, columns, rows, margin,
  spacing) and irregular (explicit rects with name, per-frame pivot, trim offset and original size, as
  atlas packers produce). Built in code; file formats come with M17.
- `SpriteClip` asset (a new type with a distinct name from the skeletal `AnimationClip`): ordered
  frames, per-frame duration or a fixed rate, loop mode (loop, once, ping-pong), named events attached
  to frames. Named animations: a sheet or set maps a name to a clip.
- `SpriteAnimator` component and one system in the animate stage: advance time with a playback-speed
  multiplier and pause, pick the frame, write `Sprite.region`, `size`, `anchor` and `flip_x` on the same
  entity. No per-frame allocation.
- One-shot playback with a finished signal and optional "then play X".
- Animation events: fired exactly once per frame crossing, including across a loop wrap and when one
  step skips several frames; delivered through the M3 event channel.
- Directional animation sets: a clip per direction (4 or 8) with an optional mirror rule (left from
  right through `flip_x`). The direction is chosen from the entity's facing **relative to the active
  camera** (D-31), because in a spatial world the camera can orbit a billboarded character.
- Simple transitions between named animations: a small table of (from, to, condition on named
  parameters, or on finish). Float, bool and trigger parameters. No blending (frames cannot blend).
- Efficiency: sheets share one texture so a crowd stays one draw call per texture; benchmark an
  animate stage with thousands of animators and record it in `docs/PERFORMANCE.md`.

**Dependencies.** D-10 (events), D-27 (pixels per unit), D-29 (shared control vocabulary), D-30 (sheet
and frame data model), D-31 (directions), D-48 (camera configurations), D-06 (optional pixel snapping affects frame shimmer).

**Acceptance criteria.**
- Grid slicing: frame `i` maps to the exact UV rect for given margin and spacing, including the last
  row (table test). An irregular frame with a trim offset keeps its pivot at the same world position
  across frames (test on extracted quad positions).
- Loop, one-shot and ping-pong produce the expected frame-index sequence for a given sequence of time
  steps (table test); 2x speed halves the period; speed 0 pauses.
- Events fire once per crossing, also across a loop wrap and across a step that skips frames, and never
  after a one-shot has ended (test).
- Stepping at 30 Hz and at 144 Hz shows the same frame at the same simulated time (determinism test).
- A one-shot returns to a named clip on finish, and a parameter-driven transition fires exactly when
  its condition becomes true (test).
- Directional set: a table of (facing, camera yaw) to direction; orbiting the camera around a static
  entity changes the chosen direction (extraction test).
- A character with no `Skeleton` or `SkeletonInstance` animates (test); the sprite animator's system
  never queries them.
- 5,000 animated sprites on one sheet extract to one draw call (extends the existing batching test).
- A sheet-animated walker appears in `spatial_scene`.

**Tests/examples.** The above; example update.

**Deferred.** Aseprite/JSON import (M17), frame blending, normal-map sheets (M14), palette swaps and
shader effects, sprite-sheet packing tools, UI 9-slice.

---

## M9. Character and scene API, and a playable vertical slice: Not started

**Objective.** Freeze the foundational gameplay, scene, and component-integration contracts that UI, the
other character representations, and scripting build on, and prove them with one complete playable game.
"Stable" here means: documented, covered by tests, used by a real consumer, and changed afterwards only
by a recorded decision. The freeze does not forbid later milestones from adding public APIs. An addition
is allowed when an ADR records it. It is not a second copy of a contract M9 already froze.

**Prerequisites.** M3, M4, M7, M8.

**Tasks.**
- Character gameplay layer (D-28): the **gameplay entity** holds `Transform`, the M7 collider and
  controller, and representation-free gameplay state (intent, facing, a named state, parameters). The
  **visual** is a child entity subtree owned by a representation. Gameplay requests animation through the
  shared control vocabulary (D-29) and receives animation events back; it never reads `Sprite`,
  `SkeletonInstance` or `Mesh`. Colliders stay on the gameplay entity, so swapping a representation can
  never change collision.
- Only representation A exists at this point, so the boundary is tested with a second consumer: the
  existing skeletal `basic_2_5d` character is driven through the same gameplay components by a small
  adapter. No new skeletal features are added for this.
- Scene API: scene load, unload and replace (M4), a character prefab convention, scene events.
- API surface list, `docs/API_SURFACE.md`: which types and functions downstream layers (UI bindings,
  scripts, editor) may depend on, each linked to its tests. Everything else is internal.
- `examples/vertical_slice`: one level, a controllable sprite-sheet character, collectibles (triggers),
  simple hazards and enemies, win/lose, restart by scene reload. Text is still drawn from a bitmap font as
  sprites; the UI runtime replaces it in M10.
- Input record/replay harness (small) for headless regression runs.
- Record the audio decision (D-11) and write up the engine gaps found, with a milestone for each, in
  TASKS.md.

**Dependencies.** D-28, D-29, D-11 (decision only), D-24 (informed by the slice's HUD needs).

**Acceptance criteria.**
- The game can be played from start to win and to lose.
- A recorded input log replays headlessly and gives the same final state hash (test).
- A 10-minute scripted soak run completes with no panics and no memory growth beyond a recorded budget.
- The same gameplay entity with its visual swapped between the sprite-sheet character and the skeletal
  `basic_2_5d` rig produces an identical position and collision trace over a scripted input (test).
- Gameplay code in the slice contains no use of `Sprite`, `SkeletonInstance`, `Mesh` or `AnimationPlayer`
  (checked by a test that scans the gameplay module's imports, or by crate boundary).
- `docs/API_SURFACE.md` exists and every listed item links to a test.
- A gap list exists in TASKS.md with each gap assigned to a milestone.

**Tests/examples.** The example, the replay test, the swap-trace test, the soak script.

**Deferred.** Audio (M16), a real UI (M10), skeletal and 3D representations beyond the swap-test adapter
(M11, M12), content beyond one level.
---

## M10. UI runtime: Not started

**Objective.** A dedicated, minimal, code-driven UI runtime as an engine subsystem: screen-space UI
and UI anchored to entities in the spatial world. Layout and interaction are independent of world
rendering; only low-level GPU resources are shared. A visual UI editor is explicitly **later** (it
cannot start before M22 and is not planned until the runtime has proved stable).

**Prerequisites.** M3 (actions, events), M5 (sub-region texture update for a glyph atlas, atlas
packer, samplers), M6 (pass structure and render targets), M9 (stable character and scene API, and a
real HUD to build). The layout, text and rendering work can start after M6 in parallel with M7 to M9;
binding UI to game state waits for M9.

**What exists today (read).** Nothing UI-specific. Input has pointer position, buttons, scroll,
arrows, Enter, Escape and Tab, but no text input, no cursor-left event and no gamepad. The
window scale factor is forwarded, and `Viewport` stores physical size, logical size, and scale
(M3b Phase 3, `9c99edf`). Resolution-independent UI fit modes are not built (D-39, KI-24's
remaining text-input gap). Textures re-upload whole on a revision
change (KI-25). There is no font loading or text rendering. `RenderScene` is world-only.

**Tasks.**
1. Settle D-24 (model), D-37 (text), D-38 (rendering and world UI) and D-39 (input, focus, scaling) in
   a short design note, using small spikes where a decision depends on a measurement.
2. Platform: the scale factor, physical and logical `Viewport`, and physical-to-logical pointer
   conversion are M3b Phase 3. Still to do here: cursor-left, and text input events only if text
   fields are in scope (D-39).
3. Text: font asset (TTF/OTF) loading, glyph rasterisation into a glyph atlas using the M5 sub-region
   update, single- and multi-line layout with wrapping and alignment. Latin text first; no complex
   shaping.
4. Layout: a node tree with anchors, pivots, margins and padding, fixed/percent/fill sizing, and
   row/column stacks with spacing and alignment. Layout is a pure function from (tree, available size)
   to rectangles, with no GPU or world dependency, so it is testable on the CPU.
5. Widgets: panel (colour or 9-slice image), image, label, button (normal, hover, pressed, disabled,
   focused), scroll area with rectangular clipping. Inventories and dialogue boxes are built from
   these in the example; they are not separate subsystems.
6. Interaction: pointer hit-testing with order and clip, hover/press/click events, focus order and
   directional navigation, activate and cancel through the M3 actions, modal layers, and UI consuming
   input before gameplay sees it (D-39).
7. Scaling: reference resolution with fit/expand/fixed/integer modes, plus the device scale factor.
8. Rendering: each frame the tree produces a plain-data draw list (quads, glyph quads, clip rects, in
   order). The backend draws it in an overlay pass after the world: no depth, alpha-blended painter's
   order, scissor clipping, batched by texture. Rectangular clips only; arbitrary masks are deferred.
9. World-anchored UI: a node anchored to an entity (plus an offset) is positioned each frame by
   projecting the entity through the active camera, with a stated policy for behind-camera and
   off-screen (hide or clamp). Health bars, name tags and speech bubbles use this. UI panels that live
   on a world surface (render-to-texture on a quad) are a follow-up decided by D-38.
10. A `Style` struct for colours, fonts and spacing; no theming engine.
11. Example: HUD (score, health), a pause menu with keyboard navigation, a scrolling inventory grid and
    a dialogue box with revealed text, replacing the bitmap-font text in the M9 slice.

**Dependencies.** D-24, D-37, D-38, D-39, D-27 (units), D-16 (font asset ids).

**Acceptance criteria.**
- Layout golden tests: a table of (tree, size) to exact rectangles covering anchors, stretch, stacks,
  padding and wrapping; deterministic; the layout code has no GPU or world dependency.
- Text: layout of a fixture font gives known widths, wrap points and alignment; after the first frame a
  static screen uploads zero new glyphs (counted).
- GPU pixel test: a panel with a label renders at the expected rectangle at scale 1 and scale 2 with
  identical logical layout; a clipped child is cut at the clip edge; scrolling moves content.
- Interaction tests (no GPU): hit-testing respects order and clip; a click needs press and release
  inside the same widget; Tab and arrow navigation visits focusables in the expected order and wraps;
  disabled widgets are skipped; a modal blocks layers below it; a pointer press consumed by UI is not
  visible to gameplay input.
- A name tag follows an entity as the camera moves (test using the camera projection) and is hidden
  behind the camera.
- The same screen at 800x600 and 1600x1200 with the fit mode has proportional rectangles (test).
- Building the draw list for a 500-widget screen fits a recorded budget and issues one draw per
  texture, apart from clip splits (recorded).
- The slice's HUD, menu, inventory and dialogue run through the UI runtime.

**Tests/examples.** Layout, text, interaction and projection tests; GPU pixel tests; `examples/ui_demo`
or the slice.

**Deferred.** Visual UI editor, data-driven UI files (M19 at the earliest), text input fields and IME,
complex shaping and bidirectional text, rich text, theming and skins, UI animation beyond hover states,
localisation, accessibility, arbitrary masks, true 3D UI panels unless D-38 chooses render-to-texture.

---

## M11. Skeletal 2D animation: Not started

**Objective.** Production-usable 2D skeletal animation on the existing `Skeleton`/bone-entity model, with
blending, state machines, IK, constraints, equipment and customisable characters. This is character
representation **B**. Animation evaluation stays separate from GPU rendering.

**Prerequisites.** M3 (animate stage, events), M5 (atlas and materials for parts), M9 (character API).
It shares event and parameter concepts with M8 but not its evaluator. It is independent of M10.

**What exists today (read, plus 4 unit tests).** `Skeleton` (bones with parent index and rest
`Transform`), `Pose`, `spawn_skeleton` (one entity per bone), linear TRS `AnimationClip` sampling, and
`AnimationPlayer`. Evaluation is already GPU-free. But `animate_system` samples a clip and writes the
result straight onto the bone entities' `Transform`; `Pose` is used only by `Skeleton::world_transforms`
and tests. There is nothing between sampling and writing where blending or IK could act (KI-22).

**Tasks.**
- Make evaluation pose-first (D-34): sample clips into a `Pose`, blend, apply constraints and IK, and
  only then write bone entities. The evaluator has no dependency on the world or the renderer, so it is
  testable on plain data.
- Rest poses and structural validation (partly present).
- Crossfade blending; layered or masked blending by bone set if the slice needs it.
- An animation state machine. If M8's transition table has the same shape, extract the shared part
  then and not before (D-29); otherwise keep two.
- Animation events fired exactly once per key, including across a loop wrap.
- Interpolation modes (step, linear, cubic).
- Two-bone IK for limbs and look-at; a small constraint set (rotation limits, look-at) with a defined
  application order after IK. More only if a character needs it.
- Equipment and attachment points: named sockets on bones (bones are already addressable by name through
  `SkeletonInstance::bone`), an attach helper with an offset, and part draw order by real Z offsets
  (already supported) with validation.
- Customisable characters: skin and slot swapping (swap a part's region or material), per-slot part
  variants, per-part tint, and assembly of a character from a definition (skeleton, parts, equipment
  slots).
- File format for skeletons and clips defined together with the importers (M17).

**Dependencies.** D-12, D-29, D-32, D-34.

**Acceptance criteria.**
- Crossfade at 50% equals the lerp/slerp of the two sampled poses (unit test, no world, no GPU).
- Events fire once at their keyed times, and again after a loop (test).
- Sampling is independent of frame rate (determinism test).
- Two-bone IK reaches a reachable target within a tolerance and stretches straight, without NaN, for an
  unreachable one.
- Constraints apply in the documented order (test).
- An attached item follows its bone every frame and is removed with its owner (D-03).
- Swapping a part's region or material changes the extracted scene without changing any bone transform.
- The M9 swap-trace test passes with the slice character on a skeletal visual.

**Tests/examples.** Unit tests above; slice integration with a skeletal character.

**Deferred.** Spine and other importers, 2D mesh deformation (unless D-12 chooses it), ragdoll,
retargeting, physics-driven secondary motion, animation compression.
---

## M12. 3D characters: Not started

**Objective.** Skinned 3D characters from imported models and animations, living in the **same**
world, entity model, transforms and gameplay systems as sprite-sheet and 2D skeletal characters. There
is no second world. This is character representation **C**.

**Prerequisites.** M5 (materials, sampler and colour space), M6 (renderer limits removed, performance
baseline), M9 (character API), M11 (pose evaluation separated from entity writes, blending, IK). A
**minimal glTF importer** is part of this milestone because a skinned character cannot be built or
tested otherwise; the general import pipeline stays in M17.

**What exists today (read).** `Mesh` renders static indexed meshes. `Vertex` is position, normal and
uv only; there is no joint or weight data, no tangent, no loader, and `MeshData` can be built only in
code. Bone entities can carry a `Mesh` or `Sprite` as a rigid child, which works as an attachment but
is not skinning. `Skeleton`, `Pose` and `AnimationClip` already use `Transform` (3D position, `Quat`
with slerp, 3D scale), so they are already shaped for 3D.

**Tasks.**
- Skinned vertex layout (joints and weights; kept apart from the static `Vertex` so static meshes do
  not grow), a skin asset (joint list, inverse bind matrices) and a `SkinnedMesh` component bound to a
  skeleton instance.
- Skinning execution per D-35, with the joint palette carried through the extraction seam as plain data
  in `RenderScene`; the backend still never sees the ECS.
- 3D skeleton representation per D-32 and D-33: reuse `Skeleton`/`Pose`/`AnimationClip` and the M11
  blending where they fit, keep skin binding separate.
- glTF import (D-36): meshes, base-colour materials and textures, node hierarchy, skins, and
  translation/rotation/scale animation clips. glTF is right-handed with +Y up and +Z as the front of a
  model; Dimora is left-handed with +Z deeper. The conversion is a documented, tested rule. Malformed
  files return errors.
- Attachment points (sockets) for equipment, from D-33.
- Two-bone IK on the 3D pose where it fits (for example arms). Foot placement needs ground queries
  from M7 and is optional.
- A 3D character is a gameplay entity (`Transform`, collider, gameplay components) with a visual child
  holding `SkinnedMesh`. The M9 gameplay layer is unchanged.
- Example: a sprite-sheet character, a 2D skeletal character and a 3D skinned character on one stage,
  driven by the same controller and seen by the same camera.

**Dependencies.** D-32, D-33, D-35, D-36, D-12, D-17 (a glTF dependency must be justified), D-07.

**Acceptance criteria.**
- glTF fixtures: a cube loads with correct winding and bounds; a two-joint skinned box with a bend
  animation loads with the expected clip length and channels. An asymmetric fixture shows the
  documented handedness conversion (front faces the way a Dimora character facing +Z does).
- Skinning correctness: a posed fixture matches a CPU reference within tolerance after a GPU readback.
- Blending two imported clips at 50% equals the slerp/lerp of the two sampled poses.
- A skinned character is correctly occluded by and occludes a sprite-sheet character at different Z
  (GPU pixel test): one world, one depth buffer.
- The M9 gameplay trace test passes with the 3D visual swapped in.
- An attached weapon follows its bone.
- N skinned characters fit a recorded budget (N and the budget from the M6 baseline).
- Malformed glTF input returns an error and never panics.

**Tests/examples.** Importer, skinning, handedness and trace tests; the mixed-representation example.

**Deferred.** Morph targets, cloth and physics-driven secondary motion, retargeting, LODs, animation
compression, skinned crowds through vertex-animation textures, glTF extensions, FBX, PBR materials
(M14).

---

## M13. Tilemaps: Not started

**Objective.** Production tilemap APIs on the chunking foundation M6 provides. M13 does not implement a
second chunk extractor.

**Prerequisites.** M6 (chunk extraction, cached instances, dirty chunks), M7 (collision layer hook), M5
(atlas). Independent of the character and UI milestones. It does not start before M6's chunking exists.

**Tasks.**
- Reuse M6 chunks. Do not add another extraction path.
- Tileset asset: atlas reference plus tile metadata and properties. In-code builder.
- Per-tile flip and rotation. Animated tiles. Multiple layers at distinct Z.
- Collision integration with M7.
- Vertical (wall) tilemaps through the same orientation model.

**Dependencies.** D-25 if oblique projection is wanted for tilemaps; D-48.

**Acceptance criteria.**
- The M6 chunk benchmark still holds after the production APIs are added: a 1000x1000 map's per-frame
  cost stays proportional to visible chunks, and `set_tile` still touches one chunk. M13 does not
  replace those tests with a second chunker.
- A tile collision layer blocks the character controller (test).
- Animated tiles advance with time (test).
- One map, placed once as a wall (side view) and once laid on the ground (top-down), extracts with the same
  code and renders correctly through the matching camera configuration (GPU test). A square-grid map under an
  isometric camera needs no isometric tile type.

**Tests/examples.** Tests above; slice level uses the chunked map.

**Deferred.** Tiled importer (M17), auto-tiling, hex and diamond-layout grids (an *isometric camera* over a
square grid is not deferred: it is a camera configuration), infinite/streamed maps.

---

## M14. Unified lighting and shadows: Not started

**Objective.** Sprites and meshes lit and shadowed by the same system, including normal-mapped
sprites, under every supported camera configuration. Lighting and shadow code must not assume one view,
one projection or one light direction relative to the camera; fog and stylised lighting remain optional
scene properties and are not prerequisites.

**Prerequisites.** M5 (material classes, the reserved normal slot, linear textures; ADR-0015 and
ADR-0017), M6 (render targets and multiple passes), D-13. Shadow casters are explicit: an animated
sprite-sheet frame needs M8, a skeletal 2D character needs M11, and a skinned mesh needs M12. The
milestone stays after all three, because its acceptance includes those casters. A representation that
does not exist yet is not given a stand-in shadow test.

**Tasks.**
- Shadow maps for directional and spot lights; alpha-tested sprite casters (using the current sprite-sheet
  frame, M8) and skinned-mesh casters (using the joint palette, M12).
- Normal-mapped sprites (tangent frame from the orientation mode); a normal sheet shares the colour
  sheet's frame regions.
- Light culling beyond 16 lights (D-13).
- Light layers/masks; bias and filtering.

**Dependencies.** D-13, D-14.

**Acceptance criteria.**
- GPU test: an occluder darkens a receiver, and moving the light removes the shadow.
- A sprite casts a visible shadow on the ground, and the shadow follows the animated frame.
- A skinned 3D character's shadow changes with its pose (GPU pixel test).
- GPU test: a normal map changes sprite shading with light angle.
- 64 lights in a scene stay within the recorded frame budget.
- The shadow and normal-map tests above run under a perspective camera, an orthographic isometric camera and
  a top-down orthographic camera (shadow-map fitting follows each projection's frustum, with no
  perspective-only assumption), using the M6 configuration test scene.
- A normal-mapped upright sprite and a billboarded sprite are shaded consistently when the camera changes
  configuration while the light is fixed in the world.

**Tests/examples.** GPU tests; slice lighting pass.

**Deferred.** Global illumination, SSAO, PBR, baked lighting, volumetrics.

---

## M15. Particles: Not started

**Objective.** CPU-simulated particles in the same spatial world.

**Prerequisites.** M3 (stages), M5 (materials), M6 (budgets), M9 (consumer).

**Tasks.** Emitter component (rate, lifetime, velocity, gravity, colour/size curves), seeded
deterministic RNG, particles emitted into the existing instanced sprite path with depth testing,
emitters following hierarchy (including attachment sockets from M11), per-system sort policy (D-15).

**Acceptance criteria.**
- Same seed gives the same particle state frame by frame (test).
- 100,000 live particles extract within the recorded budget.
- GPU test: particles are occluded by a nearer mesh and occlude a farther one.

**Tests/examples.** Above; slice effects.

**Deferred.** GPU particles, ribbons/trails, world collision, sub-emitters.

---

## M16. Audio: Not started

**Objective.** Sound effects and music, with positional sound taken from the entity's real position in
the spatial world. Needed before scripting so that scripts and animation events have audio to call.

**Prerequisites.** M3 (events), M7 (collision events as triggers), M8 (animation events, such as
footsteps), D-11.

**Tasks.**
- Audio backend per D-11, behind a small interface with a null backend so tests and CI need no device.
- `AudioSource` component (asset, volume, pitch, looping, spatial flag) and an `AudioListener` on the
  camera or another entity; distance attenuation and panning computed from the left-handed listener
  basis.
- Buses (master, music, effects) with volumes.
- Decoders for the formats chosen in D-17 (whole-asset decode; streaming is deferred).
- Triggering from animation events (M8, M11) and collision events (M7).

**Dependencies.** D-11, D-17, D-10.

**Acceptance criteria.**
- Attenuation and pan math have table tests, including a listener rotated by yaw.
- With the null backend, spawning a source starts a voice, a one-shot ends and releases it, a looping
  source continues, and despawning the entity stops it.
- Bus gain changes scale output (test on the mixer's output buffer).
- The whole test suite passes on a machine with no audio device.
- Manual: the slice plays footsteps on animation events and a sound on pickup.

**Tests/examples.** Mixer and math unit tests; slice integration.

**Deferred.** Effects and reverb, mixing graphs, HRTF, streamed music with seeking, middleware
integration.

---

## M17. Asset management and importers: Not started

**Objective.** Real content workflows: files on disk, async loading, hot reload, stable ids.

**Prerequisites.** Core identity and loading need M5's texture and material assets and D-16. Individual
importers wait for the content they import: sprite sheets (M8), fonts (M10), skeletons and clips (M11),
the M12 glTF path, tilemaps (M13), and audio (M16). D-17 and D-18 still apply. PNG decoding already
exists and is not written again.

**Tasks.**
- Core, separate from any one content type: stable asset identity and a dependency graph with cascading
  reload and unload (D-16). Worker-pool async loading with main-thread apply. File-watcher hot reload.
- Importers, each only when its content milestone exists, and none as a second copy of a loader that
  milestone already shipped: PNG stays the existing decoder; glTF generalises M12's minimal importer
  (async, dependency tracking, reload) and does not add a second parser beside it; sprite-sheet
  descriptors (grid and Aseprite-style JSON) for M8; fonts for M10; audio for M16; Tiled tilemaps for
  M13; a skeleton and clip format for M11.
- Script files as assets with reload notifications (consumed by M18).
- Loading errors surface as logged errors plus the existing magenta placeholder.

**Dependencies.** D-16, D-17, D-18.

**Acceptance criteria.**
- Examples load their art from files with no procedural fallback.
- Editing a PNG on disk changes the running example without restart (integration test using reload).
- Changing a texture reloads dependent sprites (test).
- 1,000 assets load on workers without a frame-time spike beyond a recorded budget.
- A glTF cube loads with correct winding and bounds, through the async and reload path.
- A sprite sheet loaded from an Aseprite-style JSON plus PNG has the same frames as the code-built sheet
  (test); editing the JSON reloads the running animation.
- Replacing a font file re-renders the running UI text.

**Tests/examples.** Importer unit tests with fixture files; reload integration test.

**Deferred.** Bundles (M21), compression, DCC plugins.

---

## M18. Gameplay scripting: Not started

**Objective.** One embedded scripting runtime for gameplay logic, over a deliberately small, versioned
script API. Scripts are not a way to reach engine internals. Scripting starts only after the core
gameplay APIs have stopped moving (M9), and after the subsystems scripts call exist: events and
commands (M3), the component registry (M4), animation (M8, M11), UI (M10), audio (M16) and hot reload
(M17).

**Prerequisites.** M3, M4, M9, M10, M16, M17, and the decisions D-40 to D-43. At least one of M8 and M11
must exist for the animation API to be real.

**What exists today.** Commands and events exist (M3a-3). The component registry and its inert script flags exist (M4 Phase 1, ADR-0014). There is no scripting and no `serde`. Those gaps are why scripting is scheduled late; building it now would force an API onto code
that is about to change.

**Tasks.**
1. Evaluation spike (throwaway, not shipped): implement the same small set of behaviours (a patrol, a
   trigger-opened door, a UI button callback) with the top two candidates from D-40, and measure
   call overhead, memory, build impact, error output and hot reload. Record the results and decide D-40.
   Exactly one runtime ships; no second language, no custom language.
2. Create the `dimora-script` crate (not before this milestone). The core engine crates never depend
   on it or on the interpreter library.
3. Script API v1, hand-written and versioned: opaque generation-checked entity handles; components
   reachable only by registry name and only if marked script-visible, with read and write flags;
   prefab spawn and despawn; event subscribe and emit; animation intent (play, set parameter);
   audio play; UI binding (set text, bind button); time; seeded deterministic random; input **actions**
   (not raw devices); log; a typed game-state store. Every mutation goes through the M3 command buffer
   and applies at a stage boundary.
4. Script components with callbacks (`on_spawn`, `on_update`, `on_fixed_update`, `on_event`,
   `on_despawn`) in one scripting stage at a documented position in the frame.
5. Errors: compile and run errors carry script name, line and column and a traceback, are logged, and
   disable only the faulting instance (configurable to abort in development). No panic crosses the
   boundary.
6. Sandbox: restricted standard library, an instruction or time budget and a memory cap, no file,
   network or process access.
7. Hot reload at a stage boundary, state policy from D-43; a failed reload keeps the previous version.
8. Convert one slice behaviour (an enemy patrol and a door) to scripts and keep the slice playable.
   Write `docs/SCRIPTING.md` with the API, capabilities and limits.

**Dependencies.** D-40, D-41, D-42, D-43, D-18, D-10.

**Acceptance criteria.**
- A script can access only script-visible components; access to anything else, and access to `os`,
  `io` or file functions, fails with a clear error (sandbox tests).
- An infinite loop is stopped by the budget within a bounded time and the game keeps running (test).
- A runtime error message contains the script name and line (test).
- Editing a script changes behaviour on the next frame with no restart; a syntax error keeps the old
  behaviour and reports the error (test).
- Replaying the slice with a scripted enemy gives the same state hash in two runs (determinism test).
- A mutation requested in a callback is not visible until the stage boundary (test).
- 1,000 scripted entities' `on_update` fits a recorded budget.
- The engine builds and its tests pass without the script crate.
- Every exposed API function has a test.

**Tests/examples.** Sandbox, error, reload, determinism and budget tests; the scripted slice.

**Deferred.** A second language, WebAssembly (only if D-40's criteria for untrusted third-party code
are met), visual scripting, a debugger protocol, networked scripts, mod distribution and signing,
script-defined component layouts, scripts extending rendering.

---

## M19. Scene serialization: Not started

**Objective.** Save and load scenes as versioned, diffable text files.

**Prerequisites.** M4 (component registry), M17 (stable ids), M18 (script components and their state).

**Tasks.** Format and `serde` policy (D-18), component serialization through the registry, entity
reference remapping, hierarchy, asset references by id, version/migration scaffolding, basic prefab
instancing, loading scenes in `Game`. The registry covers the character, sprite-sheet animator, skeletal,
skinned-mesh and script components so a scene can hold all three representations and scripted behaviour.

**Acceptance criteria.**
- Save -> load -> save is byte-identical (deterministic ordering).
- A scene with a hierarchy and every renderable component round-trips and extracts an identical
  `RenderScene`.
- Unknown components and missing assets produce clear errors, not panics.
- A migration test upgrades a stored older-version scene.
- The slice level is a scene file, including its scripted enemy and its sprite-sheet character.

**Tests/examples.** Round-trip, golden-file and migration tests.

**Deferred.** Binary scene format (M21), nested prefab variants, UI tree files (UI stays code-driven
until the runtime has proved stable).

---

## M20. Debugging tools: Not started

**Objective.** See and poke the running engine. Consolidates and extends the tools delivered earlier
(logging and diagnostics M3a, inspection M4, debug draw M6, overlay M10) into one set with an edit-capable
inspector; it is not where tooling starts.

**Prerequisites.** M4 (the component registry and field metadata; M20 does not build another), M6 (debug
draw), M10 (UI runtime for the overlay), M19 (scene serialization). Inspection reads the M4 registry.
Editing a saved scene uses M19. Neither tool re-implements those foundations.

**Tasks.** Overlay (frame graph, counters), entity inspector (read first, edit second), logging and
profiling scopes (D-21), screenshot and replay tooling promoted from M9, asset/hot-reload
notifications. Overlays for what the new subsystems need: colliders (M7), animation state and
skeleton bones (M8, M11, M12, drawn with debug draw), UI layout bounds (M10), and script errors (M18).

**Acceptance criteria.**
- A key toggles an overlay with live frame time and draw calls.
- The inspector lists entities and edits a `Transform` of the running `spatial_scene`.
- A profiling run writes a trace viewable in a standard viewer.

**Tests/examples.** Inspector unit tests over the registry; manual demo.

**Deferred.** Remote debugging, networked tools.

---

## M21. Asset compiler and bundles: Not started

**Objective.** Ship assets and scenes as one deterministic bundle read through `AssetSource`.

**Prerequisites.** M17, M19.

**Tasks.** New `dimora-assets` CLI crate; compile imported assets and scenes; bundle with an id
index, content hashes and optional compression (D-19); `BundleSource`; incremental cache keyed by
content hash; reference validation. Covers sprite sheets, fonts, audio, models and scripts (as source, or
bytecode if the chosen runtime supports it).

**Acceptance criteria.**
- Same inputs produce byte-identical bundles.
- The slice runs from a bundle with no loose files.
- Rebuilding unchanged inputs does nothing.
- A corrupted bundle is detected on load.
- Load time versus loose files is measured and recorded.

**Tests/examples.** Determinism, corruption and incremental-build tests.

**Deferred.** Patching/DLC, streaming, encryption, platform-specific texture formats.

---

## M22. Editor, first usable version: Not started

**Objective.** The first version of Dimora's user-friendly editor: build, arrange and save a spatial scene
visually, on the same scene files and components the code uses. This is the start of the editor, not its
end; it is scheduled here only because it needs scenes, the component registry, serialization and debug
tools underneath. Those prerequisites are designed for the editor from M3 onward, and the editor is not to be
treated as optional or as a late add-on.

**Prerequisites.** M6 (camera-to-texture), M10 (UI runtime, if D-20 chooses to use it), M19, M20, D-20, and
the **stability gate** (ADR-0003): the editor starts only when (1) the scene model and component registry
(M4) are stable, (2) asset references are stable (D-16, M17), (3) scene serialization is versioned with
migrations (M19, D-47), and (4) the runtime API surface is frozen (M9's `docs/API_SURFACE.md`, D-45). The
runtime has no editor-specific state or code paths; the editor is a separate binary using the public API
and the registry. The editor is not moved earlier without a demonstrated dependency or product need,
recorded in an ADR.

**Tasks.** `dimora-editor` binary; viewport with fly camera and translate/rotate/scale gizmos;
hierarchy and inspector panels; asset browser; save/load; play-in-editor on a copy of the world;
minimal undo/redo.

**Acceptance criteria.**
- Open the slice scene, move an entity with a gizmo, save, run the game, see the change.
- Undo restores the previous state.
- Saving is atomic: a crash cannot leave a half-written scene.

**Tests/examples.** Command/undo unit tests; manual walkthrough recorded.

**Deferred.** Visual UI editor, tilemap painter, animation and sprite-sheet editors, script editing,
plugins, custom tool windows.

---

## M23. Performance and stability: Not started

**Objective.** Meet recorded budgets and harden against bad input.

**Prerequisites.** M21, M22 (and baselines from M6).

**Tasks.** Transform dirty tracking and/or parallel propagation; culling acceleration structure;
parallel extraction; memory budgets and leak soak; fuzzing of decoders (PNG, glTF, fonts, audio, scene,
bundle) and of the script API boundary; audit of `unwrap`/`expect` on external-data paths; benchmark regression thresholds in CI;
testing on Windows, macOS and Linux GPUs.

**Acceptance criteria.**
- The budgets in `docs/PERFORMANCE.md` are met on the recorded reference machines.
- A one-hour soak shows no memory growth.
- Fuzz targets run for a fixed time without crashes.
- No panics from malformed external files.

**Tests/examples.** Benchmark suite, fuzz targets, soak script.

**Deferred.** GPU-driven rendering, multi-threaded GPU submission.

---

## M24. Packaging and release: Not started

**Objective.** Ship a desktop game and the engine itself.

**Prerequisites.** M21, M23.

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

These are outside the current plan; each has a decision slot or a reason.

- **Gamepad and touch input.** After M9. The UI focus model (M10) is built on actions so that a gamepad
  can drive it later; the device layer itself is not scheduled.
- **Web and mobile targets.** After M24 (D-23).
- **Networking.** Not planned. M9's replay determinism and M18's deterministic scripting are prerequisites
  for ever adding it.
- **Visual UI editor, data-driven UI files.** After M22 at the earliest.
