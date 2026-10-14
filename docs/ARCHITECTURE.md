# Architecture

Status: describes the code as of commit `4857158` (re-audited 2026-10-13, after M2 tasks T1 to T3).
Everything outside the sections "Proposed future architecture" and "Proposed boundaries" exists in the
repository today, except that "Cross-system requirements and risks" mixes requirements (what must hold) with
verified current state; each item says which is which. Those two sections are proposals: nothing in them is implemented. Sections that describe
weaknesses are as important as sections that describe strengths; the roadmap
([ROADMAP.md](../ROADMAP.md)) is ordered around them.

> **2.5D is a spatial world, not a fake 2D layer system.**

Every visible thing (sprite, skeleton part, tile, 3D mesh) is an entity with a real 3D `Transform`.
A camera and a projection decide how that world appears on screen. There is no integer z-index in
the model: parallax, occlusion and lighting come from positions. The rendering model is specified
in detail in [SPATIAL_RENDERING.md](SPATIAL_RENDERING.md); this file covers the whole engine. The product
intent is in [VISION.md](VISION.md); what each system does today is in [CAPABILITIES.md](CAPABILITIES.md).

## Crates and responsibilities

About 8,000 lines of Rust in nine library crates and two examples.

| Crate | Responsibility | Depends on (workspace) | External |
|---|---|---|---|
| `engine-math` | `Vec2/3/4`, `Quat`, `Mat3/4`, `Transform`, `Rect`. `f32`, column-major, left-handed | none | none |
| `engine-core` | `Color`, `Time`, `FixedTimestep`, `Key`/`Input` state | math | none |
| `engine-world` | `World` (wraps `hecs` + typed resources), sequential `Schedule`, `Parent`/`Children`, `GlobalTransform`, `propagate_transforms`, `Name`, `Visibility` | math | `hecs` |
| `engine-assets` | `AssetId`, `AssetKind`, `AssetHandle<T>`, `AssetManager`, `AssetSource` (filesystem, memory), `Texture`, `SpriteAsset`, `MeshData` | math | `image` (PNG), `bytemuck` |
| `engine-renderer` | Backend-agnostic render model: `Camera`, projections, frustum, render components (`Sprite`, `Mesh`, `Material`, `TileMap`, `Light`, ...), `extract_scene` -> `RenderScene` | math, core, world, assets | none |
| `engine-renderer-wgpu` | The wgpu backend: pipelines, GPU caches, instance buffers, window/offscreen presentation, WGSL shaders. Consumes `RenderScene` only | math, assets, renderer | `wgpu`, `pollster`, `bytemuck` |
| `engine-animation` | `Skeleton`, bones as entities, `AnimationClip`, `AnimationPlayer`, `animate_system` | math, core, world, assets | none |
| `engine-platform` | winit window and event loop -> `InputEvent`s, frame timing, `App` trait | math, core | `winit` |
| `engine` | Facade: `Game` (world + schedules + frame pipeline), runner, `prelude` | all of the above | none |
| `examples/basic_2_5d` | Tilemap ground, sprite props, skeletal character, lights, fog, 3D meshes | engine | `image` (screenshots) |
| `examples/spatial_scene` | Spatial proof: sprites and meshes at real depths, free camera | engine | `image` |

Dependency direction (arrow = "depends on"). `engine-renderer-wgpu` never depends on `engine-world`
(it sees only a `RenderScene`), and `engine-renderer` never depends on any GPU or window crate:

```text
engine ──► engine-platform ──► engine-core ──► engine-math
   │──► engine-renderer-wgpu ──► engine-renderer ──► engine-world ──► engine-math
   │                       │                    └──► engine-assets ──► engine-math
   │                       └──► engine-assets
   └──► engine-animation ──► engine-world, engine-assets, engine-core
```

## World versus renderer ownership

| Owned by the world (`engine-world`, components in `engine-renderer`/`engine-animation`) | Owned by the renderer (`engine-renderer-wgpu`) |
|---|---|
| Entities, components, resources (`Time`, `Input`, `AssetManager`, `AmbientLight`, `Fog`, `Viewport`, `RenderStats`) | `wgpu` device, queue, surface, pipelines, bind-group layouts, samplers |
| `Transform`, `GlobalTransform`, `Parent`/`Children`, `Visibility` | GPU textures and bind groups (cache keyed by `AssetId`, with the asset revision) |
| `Camera`, `Sprite`, `Mesh`, `Material`, `TileMap`, `Light`, `RenderOrder` | GPU vertex/index buffers for meshes (same cache scheme) |
| Asset *data* in `AssetManager` (pixels, vertices) and `AssetHandle`s | Instance buffers (growable, rewritten each frame), depth texture |

The seam between them is `engine_renderer::extract_scene(&World, &AssetManager, target_size) ->
RenderScene`, a plain-data snapshot (camera view, sorted/batched instance lists, lights, fog). It
holds no ECS or asset-manager references. No component contains a `wgpu` type, and gameplay code
never imports `wgpu`. This boundary is real and enforced by the crate graph.

## Transform and coordinate conventions

Fixed and used everywhere, including shaders (full table in SPATIAL_RENDERING.md):

* Left-handed; `+X` right, `+Y` up, `+Z` forward/deeper. A camera with identity rotation looks
  down `+Z`; positive yaw turns towards `+X`.
* Column-major matrices, column vectors (`clip = proj * view * model * v`); NDC depth `[0, 1]`.
* `Transform { position, rotation: Quat, scale: Vec3 }`; parent/child composition by matrix
  product in `propagate_transforms`, producing `GlobalTransform { matrix }`.
* Sprites carry an anchor (default bottom-centre) and a world size; `pixels_per_unit` converts texels
  to world units. The engine assumes no unit scale.

Limitations: non-uniform scale under a rotated parent produces shear, which `Transform` cannot
represent (`GlobalTransform` keeps the matrix, but `to_transform()` is lossy); `Transform::inverse`
is exact only for uniform scale.

## Camera and projection

* `Camera` component: `projection: CameraProjection`, `viewport: ViewportRect` (normalized sub-rect
  of the target), `clear_color`, `active`, `order`. Position and orientation come from the entity's
  `Transform`.
* `CameraProjection` is a `#[non_exhaustive]` enum with two variants today (`Orthographic`,
  `Perspective`). `CameraView::new(camera, global_transform, target_size)` resolves the view
  matrix (inverse of the camera's world transform), projection matrix, viewport pixels, and basis
  vectors. The same `view_proj` is used for CPU projection, picking rays, culling and the GPU.
* The `Projection` trait (`matrix(aspect)`, default `project()`) is implemented by `Orthographic`,
  `Perspective` and the enum. **`Camera` cannot hold an arbitrary `impl Projection`**, only the enum,
  so adding isometric or oblique projections currently means adding an enum variant (see
  weaknesses).
* Exactly one camera is rendered per frame: the active camera with the lowest `(order, entity id)`.

## Asset ownership

* `AssetManager` (a world resource) owns all loaded asset data, in per-type stores keyed by
  `AssetId` (64-bit FNV-1a of the logical name). Components hold only copyable `AssetHandle<T>`
  values that are created from a name and may point at assets that do not exist yet.
* Each entry tracks `LoadState`, a `revision` (bumped on every replace), dependency ids, and an
  error string. `load()` queues a request and `process_loads()` (called once per frame by
  `Game::step`) reads and decodes synchronously on the main thread.
* `AssetSource` is the byte-level abstraction (`FileSystemSource`, which rejects `..` paths, and
  `MemorySource`).
* Only `Texture` has a decoder (PNG through `image`). `SpriteAsset`, `MeshData`, `Skeleton` and
  `AnimationClip` implement `Asset` without decoders, so every example builds them in code.
* The renderer reads asset data through `&AssetManager` during `render` and caches GPU copies keyed
  by `(AssetId, revision)`; a changed revision re-uploads. A cached copy is evicted when its asset is removed
  (`AssetManager::remove`) or when no frame has used it for a configurable number of frames (D-04, T2).
  Revisions are never reused for an id, even across `remove`, so a stale copy cannot be mistaken for a current one.

## Rendering flow

`Game::step(dt)` (windowed: once per frame; headless/tests: called directly):

```text
Time.advance -> AssetManager.process_loads
  -> fixed_update systems (0..8 times, FixedTimestep at 60 Hz by default)
  -> update systems (user, in insertion order)
  -> animate_system (hard-wired)
  -> propagate_transforms (hard-wired; full pass, no dirty tracking)
  -> Input.end_frame
```

then, separately: `Game::extract()` -> `RenderScene`, and `WindowRenderer::render(&scene, &assets)` (or
`render_to_rgba` offscreen).

Extraction (`extract_scene`): choose the active camera, build `CameraView`, build the frustum, gather
lights (cap 16, directional first, rest nearest to camera), then per renderable: visibility check,
asset resolution (missing assets are skipped and counted), orientation-dependent quad matrix for
sprites, bounding-sphere frustum culling, then sort and batch. Blended sprites are sorted by
`(RenderOrder.layer, view depth far->near, RenderOrder.order)`; cutout sprites, tiles and meshes are
unordered except for grouping by texture/mesh. Sprites and tiles share one instanced quad path.

Backend: one render pass with one depth buffer (Depth32Float): opaque meshes (back-face culled),
cutout sprites/tiles (alpha-tested, depth write), then alpha-blended sprites (depth test, no write).
Lighting is a single forward world-space `shade()` shared by sprites, tiles and meshes: ambient plus
up to 16 directional/point/spot lights, wrap-lit sprites, exponential distance fog. Surface loss and
resize are handled; transient acquire failures skip the frame.

## Current extension points

These exist and work:

* **Components + systems.** New renderable or gameplay data is a component; systems are
  `FnMut(&mut World)` added to `Game::update`/`fixed_update`.
* **`RenderScene` consumers.** Any backend (or test tool) that takes a `RenderScene` works with
  extraction unchanged. `render_to_rgba` already serves headless screenshots and GPU tests.
* **`AssetSource`.** A new byte source (a bundle reader) plugs in without touching components.
* **`Asset::decode`.** Adding a loader for a type is implementing one function.
* **`Projection` trait and `non_exhaustive` `CameraProjection`.** Partial, see above.
* **`Sprite.orientation`** (`#[non_exhaustive]`) and the single function `sprite_placement`.
* **`Material`** is a small struct at the one place a material system would attach.

These are *not* extension points yet, despite existing as ideas in earlier docs: there is no shadow or
normal-map hook of any kind, no render graph or custom pass API, no user shader support, no event
system, no system ordering beyond insertion order, no asset-loading pipeline beyond PNG.

## Known architectural weaknesses

Each is verified by reading the code or by a test run; identifiers refer to the known-issues table in
[TASKS.md](../TASKS.md).

1. **Resource lifetime (KI-4).** Fixed by T2: `AssetManager::remove` exists and the GPU caches evict removed
   and idle entries. Remaining limit: only textures and meshes are cached per asset, instance buffers only
   grow, and nothing yet calls `remove` for you (scene unload is M4).
2. **Hierarchy robustness (KI-7, KI-8).** Fixed by T3: an entity without a `Transform` counts as
   identity and its children are still placed, and `despawn` removes the whole subtree (D-03;
   `detach_children` keeps children as roots at the same world transform). Remaining limit: there is no
   dirty tracking, so the pass is O(entities) every frame.
3. **Scheduling (KI-9, KI-10).** The frame order is hard-coded inside `Game::step`; user systems cannot be
   placed before/after engine stages, and nothing orders systems relative to each other except
   insertion. `FixedTimestep::alpha()` exists but is not exposed, so there is no render
   interpolation for fixed-step motion. Resource access is `RefCell`-based: a double borrow panics
   at runtime. No deferred commands, so spawning while iterating needs manual care. `hecs` has no
   change detection or events.
4. **One camera per frame (KI-5).** `Camera.viewport` is honoured, but only one camera renders, so
   split-screen, minimaps and render-to-texture are not possible, and the editor cannot get a second
   viewport.
5. **Closed projection set (KI-11).** `Camera` takes the enum, not the trait.
6. **Textures and materials (KI-6).** No mipmaps, clamp-to-edge only, every texture uploaded as sRGB
   (so non-colour data such as normal maps cannot be represented), `Material` is
   colour + one albedo + a lit flag, meshes can only be opaque.
7. **Asset pipeline (KI-12).** Synchronous loads, one decoder, no hot-reload watcher, no bundles.
   `AssetId` is a name hash; ids change if an asset is renamed. Two different names that hash to the same
   id for one asset type are now an error (`AssetError::IdCollision`, T3), not an overwrite; the check
   is per asset type.
8. **Skeletal animation is a data model, not a system (KI-13).** Linear TRS keyframes on bone
   entities driving child sprites. No blending, events, IK, skinning, or loading.
9. **Transparency (KI-14).** Blended sprites are sorted per quad centre only; large or intersecting
   blended quads can mis-order; meshes cannot be blended.
10. **Per-frame CPU cost.** Extraction walks all renderables each frame; tilemaps are expanded
    tile-by-tile each frame (an 80x120 map is ~9,600 tile checks per frame); instance buffers are
    rewritten fully. Fine for the demos, unmeasured at scale (KI-19).
11. **Verification gaps (KI-1..3).** Mostly closed by T1: the toolchain is pinned, the MSRV is 1.90, and
    `scripts/check.sh` with `DIMORA_REQUIRE_GPU=1` turns a missing GPU adapter into a failure. Also done: a CI workflow
    runs the check on Windows, macOS and Linux and the MSRV job passes on all three (TASKS.md "CI results"). Still open: it uses
    software adapters only, and plain `cargo test` without the variable still skips GPU tests and reports `ok`.
12. **Frame selection and animation plumbing (KI-21, KI-22).** A sprite's natural size ignores its `region`,
    so sprite-sheet frames need `size` written alongside. `animate_system` writes sampled transforms
    straight onto bone entities, and `AnimationPlayer` only works on entities with a `SkeletonInstance`, so
    there is no pose stage for blending or IK and no way to animate a skeleton-less sprite.
13. **No skin data (KI-23).** `Vertex` has position, normal and uv only; nothing carries joints or weights,
    and there is no model loader.
14. **UI foundations (KI-24, KI-25).** The platform layer does not forward the window scale factor, input
    has no text events, and any texture change re-uploads the whole texture, which a glyph atlas cannot
    afford.

## Cross-system requirements and risks

The systems in [CAPABILITIES.md](CAPABILITIES.md) are only useful if they hold together. This section
states, for each integration question, what is true today (checked in code or by a test), the requirement,
and the gap. It then lists the architectural risks that would be expensive to fix late. Requirements
here are binding on milestone designs; open choices point to a decision in [DECISIONS.md](DECISIONS.md).

### Integration requirements

**1. Sprites and meshes under one camera and one set of conventions.**
*Today:* true and tested. Both are entities with a `GlobalTransform`; extraction projects both with one
`view_proj`; one `Depth32Float` buffer; GPU tests show a nearer mesh occluding a sprite and the reverse
(`nearer_mesh_occludes_sprite_and_farther_mesh_does_not`), blended sprites interleaving with meshes
(`blended_sprites_interleave_with_meshes_by_depth`), and `sprites_and_meshes_share_one_spatial_world`.
*Requirement:* any new renderable (tile, particle, skinned mesh, world-UI quad) enters through the same
transform, camera and depth contract and declares one **transparency class**: opaque, cutout, or blended.
*Gap:* the class is encoded in the `RenderScene` shape rather than as data, see R-1.

**2. Depth, transparency, render layers and occlusion.** The rule to preserve: *depth decides
visibility; a transparency class decides the pass; sort keys exist only for blended content.* Opaque
and cutout content needs no ordering. Blended content is ordered by `(RenderOrder.layer, view depth,
RenderOrder.order)`; `RenderOrder` is a narrow override, not a layer system. Four concepts stay distinct
(ADR-0004, amended 2026-10-13): spatial depth, transparency sorting, render layers or queues, and explicit
render order; an arbitrary sort value never replaces spatial depth, deliberate ordering of coplanar content is a
separate supported override whose mechanism is not yet chosen (D-48), and room is left for sort or occlusion
policies for isometric styles without building an occlusion framework now. Shadows, particles and
skinned meshes must pick a class and may not introduce a second ordering concept. Screen-space UI is
outside this model (painter's order, no depth). Open: blended meshes and intersecting blended quads
(D-05, KI-14).

**3. Character animation, attachment and world transforms.** Bones and sockets are entities in the
world, so an attached sword or particle emitter is an ordinary child and follows `propagate_transforms`.
The order that must hold: gameplay writes intent and state, animation evaluates a pose, the pose is
applied to bone entities, transforms propagate, extraction reads. Two consequences to design now:
(a) any IK that needs world positions (foot placement on ground queries, look-at) needs world transforms
that are valid *after* propagation, so the frame needs a documented point for a second apply and
propagate, or IK works on local data only; (b) camera-relative directional frames (D-31) read the camera
transform from the previous propagation (a one-frame latency to document). Animation evaluation must not
depend on the renderer or the world (D-34), and the visual subtree must never write the gameplay root's
transform (D-28).

**4. Tilemaps, physics and spatial queries.** One coordinate system: a tilemap is an entity with a
`Transform`; tiles, colliders and queries are in world space and 3D, so objects at different Z do not
collide. Colliders belong to the gameplay entity, not to a visual child. Simulation reads `Transform`
(fixed step), never the interpolated render transform. Debug draw is the way to see all of it.
*Gap:* nothing stores a previous or render transform for interpolation (R-3).

**5. Screen-space and world-space UI.** Two subsystems with a thin join: the UI runtime owns layout,
interaction and a plain-data draw list; the backend draws it in an overlay pass. They share the device,
texture cache, samplers, atlases (the glyph atlas is an ordinary texture) and the instanced-quad
machinery. They do not share the ordering model, coordinate space, lighting or input routing.
World-space UI is a UI node anchored to an entity and positioned by projecting through the active camera
(a behind-camera policy is required); render-to-texture panels are a later option (D-38).
*Gaps:* no scale factor (KI-24), no sub-rectangle texture update (KI-25), no overlay pass.

**6. Asset loading, referencing and retention.** *Today:* components hold `AssetHandle<T>`, a copyable
64-bit hash of a name; the manager owns the data; the renderer keeps GPU copies keyed by id and revision;
nothing is reference counted; removal is explicit (`remove`) and the renderer follows it (T2).
*Requirement:* every system that holds an asset resolves it through the manager per use (never caches
derived asset data in a component), unloads are driven by scene unload (M4) and dependency tracking
(M17), and failures are reported with the asset name (D-21). *Gap:* see R-4.

**7. Scripts and editor tools versus the renderer.** (The runtime holds no editor state, see requirement 11.) Neither may reach renderer or wgpu internals. Both
use the same surface: the component registry (names, visibility, field access), commands applied at
stage boundaries, events, and the action layer. An editor viewport needs camera-to-texture (D-08) and
picking through the collision/ray queries, not through GPU readback. The surface is listed in
`docs/API_SURFACE.md` at M9 (not yet written). *Gap:* none of registry, commands, events exists.

**8. Serialization and hot reload shape foundational data.** To be serializable and reloadable,
components must be plain data with no GPU objects, borrowed references or cached derived values; every
asset reference must be restorable to an identity that survives rename (D-16); entity references need
remapping on load (entities are not stable across runs); a field added later needs a default. Hot
reload needs asset revisions (exist), a dependency graph (planned) and a rule that a reload takes effect
at a stage boundary. *Today:* components built so far satisfy the plain-data rule, but `Sprite` and
`Mesh` carry asset handles that hold only a hash, so a serializer cannot recover the name from a handle.
That is R-4.

**9. Which public APIs must stay stable, which stay internal.** See R-7 and D-45. Proposed tiers,
to be fixed in M9's `docs/API_SURFACE.md`:

| Tier | Contents | Change rule |
|---|---|---|
| **Contract** | Gameplay-facing components (`Transform`, `Name`, `Visibility`, `Camera`, `Sprite`, `Mesh`, `Light` as they stabilise), asset handles, actions, events, commands, the script API, every on-disk format | Changed only through a recorded decision and, for formats, a version bump with migration |
| **Backend seam** | `RenderScene`, the future UI draw list, `Pose` | Changed with all consumers in one commit; documented |
| **Internal** | `hecs` types, `World.entities`, schedule internals, `wgpu`/`winit` re-exports, `SceneRenderer` internals, extraction helpers | Free to change; not named in docs or examples as supported |

**10. One world, many camera configurations** ([ADR-0004](adr/0004-one-spatial-world-many-camera-configurations.md);
confirmed owner direction; mechanism decisions accepted or amended 2026-10-13. The test scene demonstrates six
views with example-local constructors; an engine preset API and a coplanar mechanism are not implemented). A camera is a
`Transform` plus a projection; perspective, orthographic, isometric (orthographic, rotated), top-down,
side-scrolling and custom views are configurations of that one model with no per-view renderer, extraction path
or world representation. Presets for common views are constructors that return a `Camera` and a `Transform` and
stay fully editable; isometric is an orthographic preset, not a projection type. Style policies (sprite
orientation, filtering, snapping, fog) live at the object, material, camera, renderer or scene level where each
belongs, with no universal style type. Gameplay orientation is data, never derived only from the camera or the
sprite. Keep three things
separate: *projection* (how the world becomes pixels), *gameplay orientation* (up axis, ground plane,
facing), *visual style* (snapping, fog, mood). *Today:* the camera maths takes any `Transform` and either of
two projections, the tilemap is a quad grid in its entity's local XY plane that is laid flat by rotating the
entity, and sprite orientation modes are tested under a pitched camera. *Demonstrated (2026-10-14):* the same
`spatial_scene` world through example-local perspective, orthographic, isometric, top-down, steep and side
constructors, with GPU tests. Those constructors are not an engine preset API. *Not demonstrated:* a 2D-style
game, or a custom projection; `Camera` holds a closed projection enum (KI-11); one camera renders per frame
(R-2). *Gaps:* R-16, including the three verified coplanar conflicts.

**11. The runtime has no editor state.** Editor and developer tools consume the component registry, commands,
events and diagnostics and live in separate crates or binaries (ADR-0003). The runtime contains no editor
types, no `cfg(editor)` state and no code path that only the editor can reach.

### Risk register

"Cost" is the likely cost of getting it wrong and finding out late. Items are ordered by how many
systems they touch. Each cites evidence from this audit.

| Id | Risk | Evidence | Cost if found late | Response |
|---|---|---|---|---|
| R-1 | **`RenderScene` is partitioned by primitive type** (meshes, cutout sprites, blended sprites), not by transparency class. A blended mesh, particle or skinned character has no place to sort against blended sprites | `scene.rs`; the pass order in `scene_renderer.rs`; meshes have no blend mode | Moderate to high: extraction, backend passes and tests for all renderables | Decide transparency classes in D-05 at the start of M5, before particles or blended meshes exist |
| R-2 | **One camera, one target per frame**; `RenderScene` holds a single view | `extract.rs::active_camera`; KI-5 | High: shadows, editor viewports, render-to-texture and minimaps need a list of views | D-08 in M6 turns the scene into a list of camera passes; do not add features that assume one view |
| R-3 | **No transform timeline.** `Transform` is current state only: no previous state for interpolation, no dirty tracking, a full propagation pass, and a one-frame latency rule for camera-dependent animation | `hierarchy.rs`; KI-9, KI-10, D-02 | Moderate; additive if done early (a `PreviousTransform` or render-transform stage), invasive once gameplay code depends on a single transform | Settle in M3's design note |
| R-4 | **Asset identity and retention.** Handles are name hashes with no recoverable name, no reference counting, ids change on rename, and the collision guard is per type | `handle.rs`, `manager.rs`; D-16 | High once scene files, sheets or bundles exist | Decide D-16 before any persisted format (sheet descriptors in M8 are the first); keep names in the manager so handles can be serialized by name or id |
| R-5 | **Mutation model.** Systems mutate the world directly; resources are `RefCell` (double borrow panics); no commands or events | `resources.rs`; KI-9 | Grows with every system written; scripting and scenes cannot be built safely without it | M3 first; write no new systems before it lands |
| R-6 | **No format versioning policy.** Nothing persisted exists yet, which is the cheap moment to set one | no `serde`, no formats | Very high after the first release | Proposed rule: every persisted format (asset, scene, bundle, save) carries magic or a `version` field from its first line; each version bump ships a migration test; loaders return errors for unknown versions. Record as an ADR before M17 |
| R-7 | **Public API leakage.** The facade re-exports every crate, including `engine_platform::winit` and `engine_renderer_wgpu::wgpu`; `World.entities` is a public `hecs::World`; `Entity` is hecs's | `engine/src/lib.rs`, `world.rs`, `platform/src/lib.rs` | High after M9 and M18 promise stability | D-45: tiers (above) fixed by M9; no new public hecs types |
| R-8 | **Threading model undecided.** `World` is `!Sync` (`RefCell` resources), systems are non-`Send` closures, asset data is `Send + Sync`. Fine for single-threaded simulation, but async loading, parallel extraction and scripts all cross this line | `schedule.rs`, `resources.rs`, `Asset` bound | High if parallel systems are wanted later | D-44: state the rule explicitly (single-threaded simulation, parallel loading and later parallel extraction on `Send` data) so M3's scheduler is designed for it |
| R-9 | **Numeric determinism.** `f32` throughout; trigonometry comes from the platform libm and can differ between operating systems, so state hashes for replay may differ across OSes. World size is small enough for `f32` positions today | `engine-math`; M9 replay criterion | Moderate; surfaces as a flaky cross-OS replay test | Policy at M3: replay equality is required on one platform; cross-platform equality is not promised unless simulation avoids platform trig |
| R-10 | **Animation write path and bone cost.** `animate_system` writes straight onto bone entities; entity-per-bone makes propagation scale with bone count | `clip.rs`; D-33, D-34 | Moderate; contained inside `engine-animation` if the evaluator stays world-free | D-34 first in M11; D-33 by measurement |
| R-11 | **Material and texture format baked in.** `Material` is a component field, textures are always sRGB, no mipmaps, no sub-rect updates, batching keyed by texture only | `components.rs`, `scene_renderer.rs`; KI-6, KI-25 | Moderate: normal maps, glyph atlases and shadows depend on it | M5, before M8/M10/M14 |
| R-12 | **Logical versus physical pixels.** The code uses physical pixels everywhere; no scale factor | KI-24 | Moderate, spread across viewport, pointer, UI and screenshots | Forward scale factor and logical size in M3's platform slice, not at M10 |
| R-13 | **No diagnostics.** No logging; failures are silent or polled | zero logging calls in the workspace | Low now, rising with every call site | Add the facade in M3 (D-21 split) |
| R-14 | **Verification is narrower than it looks.** CI now runs on Windows, macOS and Linux (first run green), but only on software or paravirtual adapters, offscreen; GPU tests skip unless `DIMORA_REQUIRE_GPU=1` (CI sets it); windowed and DPI paths, real hardware and performance are unverified; pixel tests use tolerances that held on three rasterisers | TASKS.md "CI results"; KI-3, KI-24, KI-27 | Rises slowly; regressions on real GPUs or windowed paths found by users | Keep CI green and required; add a windowed smoke test when M3b lands the scale factor; real-GPU timing needs a different runner (ADR-0005 option B) |
| R-15 | **Foundations not yet shaped for the editor.** The editor is a core product goal, but nothing yet carries what it needs: components have no field names, types, ranges or defaults; there are no undoable commands; entities and assets lack stable identities (R-4); no scene format; the renderer draws one view (R-2) so an editor viewport has nowhere to render; `RenderScene` is a one-way snapshot, so picking needs world queries | `components.rs`, `world.rs`, `extract.rs`; CAPABILITIES rows 2, 8, 32, 33, 36 | High: retrofitting field metadata, commands and identities onto many components and systems is the same cost as R-4 and R-5 | Treat editor needs as acceptance criteria of M3 (commands), M4 (registry with field metadata), M6 (camera-to-texture) and M19 (serialization); write no component or system that hides state the editor must edit |
| R-16 | **Single-view assumptions hide in details.** Evidence: `Upright` sprites are edge-on under a straight-down camera and foreshortened in a steep view (observed, not settled, D-26); cutout content at equal depth follows draw order or depth-test rounding, and `RenderOrder` orders only blended sprites (three conflicts verified by `spatial_scene`, D-48); the world is +Y up with an XZ ground (a provisional convention, ADR-0004) while gameplay orientation and movement planes have no design yet; directional animation (D-31) will read the camera; shadow and light code is not yet written and could assume a perspective view. Isometric, top-down and side are now rendered by example-local constructors | `components.rs` (`SpriteOrientation`), `extract.rs`, `scene_renderer.rs`; `examples/spatial_scene/tests/configurations.rs`; ADR-0004 | High if found late: ordering, orientation and light direction are baked into M7, M8, M13 and M14 | D-48 mechanisms still to choose before M7 and M8, from the recorded conflicts, not before (principles decided by the owner, ADR-0004); M14 tests under three camera configurations |

Expensive redesigns, ranked by likelihood times cost, if nothing changes: **R-4 (asset identity)**, **R-1
and R-2 (render model shape)**, **R-5 and R-8 (mutation and threading)**, **R-7 and R-6 (API and format
promises)**, **R-15 (editor-shaped foundations)**, which compounds R-4 and R-5, and **R-16 (single-view
assumptions)**, which compounds R-1 and R-2. Everything else is local to one system.

## Decisions already made

These are settled; revisit only with a concrete problem.

| Decision | Choice | Why / trade-off |
|---|---|---|
| ECS | `hecs` wrapped by `engine_world::World` | Small archetype ECS with no app framework; gameplay goes through our `World`. **Caveat (R-7): the wrapper does not hide `hecs` today** (`World.entities` is public, `query`/`get` return hecs types, `Entity` is hecs's), so swapping storage would be a breaking change unless D-45 closes that. `bevy_ecs` is more capable (scheduling, change detection) but would shape the engine around its conventions. Cost: we must build scheduling, events and change tracking ourselves |
| Graphics | `wgpu` | One API over Vulkan/Metal/DX12 |
| Windowing | `winit` 0.30 (`ApplicationHandler`) | Standard; surfaces work with wgpu |
| Math | Own `f32` crate | Small, deterministic, shaped for `Transform`/`Quat` |
| Handedness | Left-handed, `+Z` deeper | "Z = 800" means far |
| Extraction seam | World -> plain `RenderScene` -> backend | Keeps the world GPU-free; enables headless tests and backends |
| Ordering | Depth buffer for opaque/cutout; sorted blended sprites; `RenderOrder` optional override | Position, not layers, decides visibility |
| Sprite geometry | Unit quad + per-instance matrix; orientation resolved in extraction | One shader path for sprites, tiles, bones |
| Sprite orientation | `Billboard` (default), `Upright`, `Fixed` | See SPATIAL_RENDERING.md |
| Hierarchy | `Parent`/`Children` components + `GlobalTransform` system | Bones and attachments are ordinary entities |
| Assets | Typed handles holding a name hash; manager owns data; sources behind a trait | Components can reference assets before they load |
| Despawn (D-03) | `despawn` removes the whole subtree; `detach_children` keeps children as roots with their world transform | Bones and attachments are owned by their parent, so orphans are never wanted. Cost: a caller who wants children to survive must say so |
| GPU cache lifetime (D-04) | Evict a cached texture or mesh when its asset is removed, and when unused for N frames (configurable) | Removed assets are freed promptly and off-screen assets are not re-uploaded every frame. Cost: an asset unused for N frames is re-uploaded when it returns |

## Proposed future architecture (NOT implemented)

Direction only; none of this exists. Each item is a roadmap milestone with its own acceptance
criteria, and several have open decisions in [DECISIONS.md](DECISIONS.md).

* **Stage-based scheduler** (M3): named engine stages with user systems attachable per stage, fixed-step
  interpolation, deferred commands, typed event channels.
* **Scenes as units** (M4): scene membership, load/unload, a component registry as the base for
  serialization and script exposure.
* **Material assets and sampler/colour-space-aware textures, sub-rectangle texture updates** (M5).
* **A list of camera passes** instead of one `RenderScene` camera, plus render-to-texture (M6).
* **A collision world** as a resource rebuilt in the fixed stage, with spatial 3D queries (M7).
* **Sprite-sheet animation** (M8), **a character gameplay layer with an API surface list** (M9), **a UI
  runtime** (M10), **pose-first skeletal animation** (M11) and **3D skinned characters** (M12), within the
  boundaries below.
* **Audio** (M16) and **gameplay scripting** (M18), within the boundaries below.
* **Importers, stable asset ids, scene files, bundles** (M17, M19, M21), with the asset compiler as the
  first additional binary crate.

The architecture is deliberately not being redesigned to match an ideal diagram: the extraction seam,
the crate graph and the component model are sound and the plan builds on them. No crate is created
for a subsystem until it has code; the boundaries below are rules about what may depend on what, not
a list of new crates.

## Proposed boundaries (NOT implemented)

Each subsection says what exists today, what is proposed, and what is still undecided. The numbered
decisions are in [DECISIONS.md](DECISIONS.md); the milestones are in [ROADMAP.md](../ROADMAP.md).

### 1. Character gameplay and character visuals

**Exists today.** A skeletal character in `examples/basic_2_5d` is a root entity with a `Transform`, a
`SkeletonInstance` and an `AnimationPlayer`, and the example's own movement system chooses the clip by
calling `AnimationPlayer::play`. Sprites are child entities of bone entities. There is no gameplay
character concept; gameplay and one representation are mixed in example code.

**Proposed (D-28).**

```text
gameplay entity            Transform, collider, controller, intent, facing, named state, parameters
 └── visual subtree        owned by exactly one representation; replaceable
      ├─ sprite sheet:     Sprite (+ SpriteAnimator)
      ├─ skeletal 2D:      bone entities with Sprite parts (+ skeletal animator)
      └─ 3D:               SkinnedMesh (+ skeletal animator)
```

* Data flows one way: gameplay writes state and parameters; the visual layer reads them and plays
  animation. The only path back is **events** (a footstep frame, a hit frame, "animation finished").
* Gameplay never reads or writes `Sprite`, `SkeletonInstance`, `Mesh`, `SkinnedMesh` or any animator
  internals. The visual layer never writes gameplay components or the gameplay root's `Transform`. Root
  motion, if it is wanted, is an explicit opt-in channel (open).
* Collision, the controller and the camera target belong to the **gameplay entity**. Swapping the
  representation therefore cannot change collision or movement; M9 tests this with an identical
  position-and-collision trace across representations.
* The only thing the visual layer needs from outside the character is the active camera, to pick
  directional frames (D-31).

**Unresolved.** Convention versus an explicit component (D-28); root motion; who owns facing; how ground
contact is expressed.

### 2. Sprite-sheet animation and skeletal animation

**Exists today.** Skeletal only: `AnimationClip` (bone tracks), `AnimationPlayer` and `animate_system`,
which only visits entities having a `SkeletonInstance`. For sprite sheets there are building blocks only:
`Sprite.region`, `size`, `anchor`, and `SpriteAsset` uv and pivot. A sprite's natural size ignores
`region`, so a frame animator must write `size` with `region` (KI-21).

**Proposed (D-29, D-30).**

* They are **separate evaluators over separate data**. Sprite-sheet animation chooses a frame and writes
  `Sprite` fields. It has no skeleton and never touches bones. Skeletal animation samples poses and writes
  bones. Neither imports the other. The skeletal clip type keeps its name; sprite-sheet clips get their
  own type.
* They share only a **control vocabulary**: clip or state name, playback speed, loop mode (loop, once,
  ping-pong), named float/bool/trigger parameters, a finished signal and named events. Frame durations,
  blend weights and poses are not shared.
* A shared state machine is extracted **only after** both implementations exist and show the same shape
  (M11 after M8). Until then each keeps its own.
* Combinations are made by composition, not by merging: a skeletal part may be a `Sprite` that a sprite
  animator drives (an animated face on a bone).

**Unresolved.** The state-machine extraction (D-29); whether per-frame data lives in the sheet or the
clip, and whether `region` should scale natural size (D-30); directional-set rules (D-31).

### 3. 2D skeletons and 3D skeletons

**Exists today.** `Skeleton`, `Pose` and `AnimationClip` are built on `Transform`: 3D position, `Quat` with
slerp, 3D scale. So the data is already shaped for 3D, even though the demo uses it in 2D. There is no
skin, no joint or weight data in `Vertex`, no model loader.

**Proposed (D-32, D-33, D-34, D-35).**

* **Shared:** the bone hierarchy, rest pose, `Pose`, clip sampling, blending and IK maths, written once
  and operating on plain data.
* **Evaluation first, application second.** Sample to a `Pose`, blend, constrain, solve IK, then write to
  entities (today `animate_system` writes straight to bone entities, KI-22). The evaluator depends on
  neither the world nor the renderer.
* **2D only:** slots and part draw order, skins and part swapping, sprite attachments.
* **3D only:** a skin (joint list, inverse bind matrices), vertex joints and weights, and the joint
  palette that goes to the backend as plain data in `RenderScene` (GPU skinning, D-35).
* A 3D character is a normal entity in the same world, with the same `Transform`, hierarchy,
  collider and gameplay components. There is no second world and no 3D-only gameplay path.
* Attachment points are named sockets. Whether every bone is an entity, or only sockets are, is decided by
  measurement (D-33).

**Unresolved.** One skeleton type or two (D-32); bone storage (D-33); skinning on the GPU or CPU (D-35);
glTF handedness conversion and importer choice (D-36).

### 4. World rendering and UI rendering

**Exists today.** Nothing UI-specific. `RenderScene` is world-only. No font or text rendering; pointer,
scroll and a few keys exist in input; the window scale factor is not forwarded; textures re-upload whole
(KI-24, KI-25).

**Proposed (D-24, D-37, D-38, D-39).**

* **UI is its own runtime.** Its tree, layout and interaction have no dependency on the GPU, the world
  or the renderer, so they are testable on plain data. Layout is a pure function from a tree and an
  available size to rectangles.
* It produces a **plain-data draw list** each frame (quads, glyph quads, clip rectangles, in order), the
  UI counterpart of `RenderScene`. The backend draws it in an **overlay pass after the world**: no depth
  buffer, alpha-blended painter's order, scissor clipping.
* **Shared with the world renderer:** the device and queue, texture cache, samplers, atlases (including
  the glyph atlas as an ordinary texture), and the instanced quad pipeline machinery.
* **Not shared:** the ordering model (painter's order versus depth), the coordinate space (logical pixels,
  top-left origin versus world units), lighting (none), and input routing (UI consumes pointer and keyboard
  events before gameplay sees them).
* **World-space UI** is a UI node anchored to an entity. Each frame the entity's position is projected
  through the active camera to a screen position, with a stated behind-camera policy. UI that lives on a
  world surface (render-to-texture on a quad) is a later option, not the first design.
* The UI reads the world through a narrow binding (entity position, game state). The world does not
  import the UI.
* A visual UI editor comes only after the code-driven runtime has proved stable and not before the editor
  milestone.

**Unresolved.** Retained tree or immediate mode or library (D-24); text stack and font scope (D-37); where
the draw-list type lives and whether to render UI to textures (D-38); input capture, focus model and the
scaling model (D-39).

### 5. Engine runtime and scripting runtime

**Exists today.** No scripting. There is also no component registry, no command buffer, no event channel
and no `serde`, which are prerequisites for a safe script boundary.

**Proposed (D-40, D-42).**

* The scripting runtime is an **optional crate** created when M18 starts. The core engine crates never
  depend on it or on the interpreter library, and the engine builds and passes its tests without it.
* Exactly one language ships, chosen after a measured spike. Candidates are Lua and Rhai; WebAssembly only
  for untrusted third-party code. No custom language.
* Scripts run in one defined stage. The interpreter state lives in a resource owned by the script crate.
  The runtime talks to the engine only through handles, commands and events.
* Script errors never panic the engine: they carry script name, line and column, disable the faulting
  instance by default, and appear in the debug overlay.
* Scripts are bounded (instruction or time budget, memory cap) and have no file, network or process access.
* Determinism: a seeded random source only, with no wall clock, so the M9 replay harness works for
  scripted games.

**Unresolved.** Runtime choice (D-40); execution model and error policy (D-42); hot-reload state policy
(D-43).

### 6. Script-facing APIs and internal engine APIs

**Exists today.** Only internal APIs: `World`, `AssetManager`, `Schedule`, the renderer and `wgpu`
are all public Rust items.

**Proposed (D-41).**

* The script API is a **separate, hand-written, versioned facade**. It is not generated by reflecting
  every component.
* Entities are **opaque, generation-checked handles**. Components are reachable only by registry name and
  only if marked script-visible, with read and write flags per component or field. Other components do not
  exist from a script's point of view.
* Capabilities exposed in v1: gameplay components, events (subscribe, emit), prefab spawn and despawn,
  animation intent (play, set parameter), audio, UI binding, time, seeded random, input **actions** (not
  raw devices), logging and a typed game-state store.
* Every mutation goes through the command buffer and applies at a stage boundary, never mid-callback.
* Not reachable from scripts: `World` as a whole, `AssetManager`, the schedule, the renderer and `wgpu`,
  entities' non-visible components, the filesystem.
* The API surface list from M9 (`docs/API_SURFACE.md`) is the set of internal APIs a script facade, UI
  bindings or the editor may depend on; everything else is internal and free to change.

**Unresolved.** Facade versus reflection and the visibility attribute's shape (D-41), which M4 must leave
room for in the component registry (D-18).

