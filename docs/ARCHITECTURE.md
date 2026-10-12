# Architecture

Status: describes the code as of commit `6ad872f` (audited 2026-10-12). Everything outside the
section "Proposed future architecture" exists in the repository today. Sections that describe
weaknesses are as important as sections that describe strengths; the roadmap
([ROADMAP.md](../ROADMAP.md)) is ordered around them.

> **2.5D is a spatial world, not a fake 2D layer system.**

Every visible thing (sprite, skeleton part, tile, 3D mesh) is an entity with a real 3D `Transform`.
A camera and a projection decide how that world appears on screen. There is no integer z-index in
the model: parallax, occlusion and lighting come from positions. The rendering model is specified
in detail in [SPATIAL_RENDERING.md](SPATIAL_RENDERING.md); this file covers the whole engine.

## Crates and responsibilities

About 7,500 lines of Rust in nine library crates and two examples.

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
  by `(AssetId, revision)`; a changed revision re-uploads. **Nothing is ever evicted.**

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

1. **Resource lifetime (KI-4).** GPU texture and mesh caches never evict; `AssetManager` has no
   `remove`. Long sessions and scene switching leak.
2. **Hierarchy robustness (KI-7, KI-8).** `propagate_transforms` only walks from entities that have a
   `Transform` and no `Parent`. A child whose parent lacks a `Transform`, or whose parent was
   `despawn`ed (children are not touched), keeps `GlobalTransform` at identity forever. Verified
   with a scratch test. There is also no dirty tracking: the pass is O(entities) every frame.
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
   `AssetId` is a name hash with no collision check; ids change if an asset is renamed.
8. **Skeletal animation is a data model, not a system (KI-13).** Linear TRS keyframes on bone
   entities driving child sprites. No blending, events, IK, skinning, or loading.
9. **Transparency (KI-14).** Blended sprites are sorted per quad centre only; large or intersecting
   blended quads can mis-order; meshes cannot be blended.
10. **Per-frame CPU cost.** Extraction walks all renderables each frame; tilemaps are expanded
    tile-by-tile each frame (an 80x120 map is ~9,600 tile checks per frame); instance buffers are
    rewritten fully. Fine for the demos, unmeasured at scale (KI-19).
11. **Verification gaps (KI-1..3).** No toolchain pin or MSRV (the build fails on rustc 1.83), no CI,
    and GPU tests silently skip, still reporting `ok`, when no adapter exists.

## Decisions already made

These are settled; revisit only with a concrete problem.

| Decision | Choice | Why / trade-off |
|---|---|---|
| ECS | `hecs` wrapped by `engine_world::World` | Small archetype ECS with no app framework; gameplay goes through our `World`, so storage is swappable. `bevy_ecs` is more capable (scheduling, change detection) but would shape the engine around its conventions. Cost: we must build scheduling, events and change tracking ourselves |
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

## Proposed future architecture (NOT implemented)

Direction only; none of this exists. Each item is a roadmap milestone with its own acceptance
criteria, and several have open decisions in [DECISIONS.md](DECISIONS.md).

* **Stage-based scheduler** (M3): named engine stages with user systems attachable per stage, fixed-step
  interpolation, deferred commands.
* **Scenes as units** (M4): scene membership, load/unload, a component registry as the base for
  serialization.
* **Material assets and sampler/colour-space-aware textures** (M5).
* **A list of camera passes** instead of one `RenderScene` camera, plus render-to-texture (M6).
* **A collision world** as a resource rebuilt in the fixed stage, with spatial 3D queries (M7).
* **Importers, stable asset ids, bundles, scene files** (M13-M15), with the asset compiler as the
  first additional binary crate.

The architecture is deliberately not being redesigned to match an ideal diagram: the extraction seam,
the crate graph and the component model are sound and the plan builds on them.
