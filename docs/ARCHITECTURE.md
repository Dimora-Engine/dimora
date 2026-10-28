# Architecture

Status: describes the working tree. The M6 spatial scene is commit `6dbabb8` (2026-10-14); that commit is
the screenshot baseline for M3a. M3a-2 adds named stages on that code (ADR-0007). M3a-3 adds deferred
commands and typed events (ADR-0008). M3a-4 adds the `log` crate and a `Diagnostics` resource
(ADR-0012). M3a-5 adds opt-in extract-only interpolation (ADR-0007, `523f553`). M3a-6 reviews the
public re-exports (ADR-0010) and adds no new `hecs`, `wgpu` or `winit` type. M3a is complete locally
and is not pushed. M3b Phase 1 widens `Key`, adds a mouse release edge, and counts collapsed
modifiers (ADR-0013, `57dbc4a`). Phase 2 adds the fixed-step input latch and `Actions` and is
checkpointed locally as `83286a9`. Phase 3, physical and logical `Viewport` plus the display scale, is
checkpointed locally as `9c99edf`. Phase 4, borderless fullscreen, vsync, an optional frame cap,
cursor grab, and focus-loss cleanup, is checkpointed locally as `cfdf358`. M3b is complete
locally and is not pushed. M4 Phase 1, the component registry and text dumps, is commit `cdb5ec3`
(ADR-0014) and is not pushed. M4 Phase 2, hierarchical visibility, is commit `2c76f71` and is not
pushed. M4 Phase 3, `find_by_name` and `Tags` / `find_by_tag`, is commit `f9a4aab` and is not
pushed. M4-4A, scene instance tracking, is commit `302622b` and is not pushed. M4-4B,
synchronous `load_scene`, is commit `128c70e` and is not pushed. M4-4C, unload and replace, is
commit `e12fb5b` and is not pushed. M4-4D, deferred scene operations, is commit `8398e49` and is
not pushed. M4-4E, the `spatial_scene` switch, is commit `d58c66b` and is not pushed. M4-5,
the lamp prefab and crate-private low-level scene methods, is commit `2b79806` and is not pushed. M4 is
complete locally at `2b79806` and is not pushed. M5 decisions are ADR-0015, ADR-0016, and ADR-0017,
corrected in the M5-1 final review. M5-2A CPU descriptors are commit `c687d5b`. M5-2B (`11b914f`)
selects the GPU format from `ColorSpace` and caches samplers separately from image storage. M5-2C
(`622ef80`) generates mip chains in the wgpu backend. M5-3A (`b51f1fb`) records bounded CPU
region transitions. M5-3B (`84d5ab1`) uploads those rectangles and
rebuilds an allocated mip chain. M5-4A (`bf4d9bb`) packs RGBA8 images on the CPU. M5-4B (`1666695`) samples those atlases on the GPU. M5-5A (`e0c29ef`) stores `MaterialAsset`. M5-5B (`3b3cd4a`), M5-5C (`ab1b6f7`), and M5-5D (`a7efd6c`) draw and batch assigned materials. M5-6 (`92286ce`) adds procedural `MeshData`. M5 is complete locally and accepted. The acceptance record is documentation after that checkpoint. Localized mip-texel regeneration is deferred and is not implemented.
Profiling is not in the code. The M2 audit
date was 2026-10-13 at `4857158`.
Everything outside the sections "Proposed future architecture" and "Proposed boundaries" exists in the
repository today, except that "Cross-system requirements and risks" mixes requirements (what must hold) with
verified current state; each item says which is which. Those two sections are not implemented, except the
M4 Phase 1 registry and text dumps, and the M4 Phase 2 visibility rule, named in the scene bullet. A decision
recorded there is still not code. Sections that describe
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
| `engine-core` | `Color`, `Time`, `FixedTimestep`, `Key`/`Input`, `Actions` | math | none |
| `engine-world` | `World` (wraps `hecs` + typed resources), sequential `Schedule`, `Parent`/`Children`, `GlobalTransform`, `propagate_transforms`, `Interpolate`, `Name`, `Visibility`, `Collider`, `CollisionWorld` | math | `hecs`, private `parry3d` 0.31.1 (`dim3`, `f32`, `std`) |
| `engine-assets` | `AssetId`, `AssetKind`, `AssetHandle<T>`, `AssetManager`, `AssetSource` (filesystem, memory), `Texture`, `SpriteAsset`, `MeshData` | math | `image` (PNG), `bytemuck` |
| `engine-renderer` | Backend-agnostic render model: `Camera`, projections, frustum, render components (`Sprite`, `Mesh`, `Material`, `TileMap`, `Light`, ...), single-frame `DebugDraw` segments, `extract_scene` -> `RenderScene` | math, core, world, assets | none |
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
| Entities, components, resources (`Time`, `Input`, `Actions`, `AssetManager`, `AmbientLight`, `Fog`, `Viewport`, `RenderStats`) | `wgpu` device, queue, surface, pipelines, bind-group layouts, samplers |
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
* Column-major matrices, column vectors (`clip = proj * view * model * v`); NDC depth `[0, 1]`, 1 at the near plane and 0 at the far plane (D-07, ADR-0019, settled). Compare `Greater`, clear 0. Verified on Apple M5, Metal.
* `Transform { position, rotation: Quat, scale: Vec3 }`; parent/child composition by matrix
  product in `propagate_transforms`, producing `GlobalTransform { matrix }`.
* Sprites carry an anchor (default bottom-centre) and a world size; `pixels_per_unit` converts texels
  to world units. The engine assumes no unit scale.

Limitations: non-uniform scale under a rotated parent produces shear, which `Transform` cannot
represent (`GlobalTransform` keeps the matrix, but `to_transform()` is lossy); `Transform::inverse`
is exact only for uniform scale.

## Camera and projection

* `Camera` component: `projection: CameraProjection`, `viewport: ViewportRect` (normalized sub-rect
  of the destination), `clear_color`, `clear: CameraClear`, `output: CameraOutput`, `active`, `order`.
  Position and orientation come from the entity's `Transform`. `CameraClear::Color` is the default.
  `Load` keeps the color already in a later camera's viewport on that same destination.
  `CameraOutput::Surface` is the default. `CameraOutput::Target` names a `RenderTargetId`.
  `pixel_snap` defaults to `false` and is a registered camera field.
  `internal_resolution` defaults to `None`. `letterbox_color` defaults to black. Both are registered.
  `None` draws directly into the destination viewport. `Some` renders that whole view at the given
  size and presents it with integer nearest scaling ([ADR-0023](adr/0023-integer-resolution-camera-presentation.md),
  settled for D-06B only). The private color and depth images are not `RenderTargetId`s. They are keyed by the
  live entity index, cleared when that index is reused for a scaled camera, and dropped when the camera is not
  scaled. `CameraClear::Load` with an internal resolution is rejected. A future editor must show that rejection
  and must not present the skipped camera as a successful composite.
* `CameraProjection` is a `#[non_exhaustive]` enum: `Orthographic`, `Perspective`, and
  `Custom(Mat4)` (D-25, M6-1B). `CameraView::new(camera, global_transform, target_size)` resolves the
  view matrix (inverse of the camera's world transform), projection matrix, viewport pixels, and basis
  vectors. The same `view_proj` is used for CPU projection, picking rays, culling and the GPU.
  When `pixel_snap` is set and the projection is orthographic, that constructor shifts the eye in the
  camera's right/up plane so the view-space X and Y translation land on the render-image pixel grid, then
  builds one `look_to_lh` ([ADR-0022](adr/0022-orthographic-pixel-snap.md), settled for D-06A only). `CameraView.position`
  is that eye. `forward`, `up`, and `right` stay the entity basis. The projection, including reverse-Z,
  is not snapped, and the entity transform is not written. Perspective and `Custom` skip the shift and
  keep the flag. Frustum culling, sprites, tiles, meshes, debug lines, depth presentation, and
  `ray_from_pixels` use this view. A ray built from the raw transform, without `CameraView::new`, can
  disagree by at most half a pixel. With an internal resolution the grid and the projection aspect
  are that internal size. `render_pixel` maps a destination pixel into the displayed image.
  Letterbox bars contain the camera and return no ray. A rejected presentation, including `Load`
  with an internal resolution, contains no pixel. There is no second camera and no shader snap.
* Renderer sample count ([ADR-0024](adr/0024-optional-4x-msaa.md), settled for the 1×/4× contract). `MsaaSamples::One`
  is the default and builds no 4× pipelines. `Four` is renderer-wide. It is stored only when the adapter
  supports 4 samples for the color format and for `Depth32Float`. A rejection keeps the previous count.
  `Game::msaa` reports the active count, not a request stored before a renderer exists. The scene pass
  writes a 4× color image and a 4× `Depth32Float` image, reverse-Z greater-than, depth clear 0. A later
  pass averages the four color samples into that camera's viewport only. `resolve_target` is not used.
  `alpha_to_coverage` stays off. A later direct `Load` copies the resolved destination into the four
  samples of its viewport before the scene. Depth presentation shows the maximum stored sample. An
  offscreen target and an internal-resolution camera keep a private pair at their render size and resolve
  into the existing one-sample image. The integer blit samples that image. A one-sample window draws
  into the swapchain, which is configured as `RENDER_ATTACHMENT` only. A four-sample window resolves
  surface cameras into a private single-sample image that is not a `RenderTargetId`, then one fullscreen
  copy writes the swapchain. Returning to one sample drops the 4× images and that frame image and keeps
  the pipelines. D-06 is closed for the approved policy (2026-10-26). Per-object snapping, alpha-to-coverage, and sample counts other than one and four stay deferred. M6 is not complete.
* The `Projection` trait (`matrix(aspect)`, default `project()`) is implemented by `Orthographic`,
  `Perspective` and the enum. A caller-supplied matrix is `CameraProjection::Custom`, not a trait
  object.
* Every active camera renders, in ascending `(order, entity id)` (D-08, implemented locally). `Visibility`
  does not hide a camera. `RenderScene` stores `passes: Vec<CameraPass>`, the live `RenderTargetDesc`
  list, and shared ambient and fog. There is no single `view` that stands for the frame. `sole_pass`
  is the one-camera accessor and panics otherwise. A frame with no drawable camera returns `None`.
  An offscreen camera uses its target's size. A missing output target is skipped.

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

`Game::step(dt)` (windowed: once per frame; headless/tests: called directly) runs named stages.
Each stage is a `Schedule`. Inside a stage, `before` / `after` order named systems; systems with no
constraint keep registration order, and a ready-set tie goes to the earlier registration. A cycle,
a duplicate name, a missing target or a self-dependency is rejected and the schedule is left unchanged
(ADR-0007). `advance_time` and `process_loads` stay first in their stages. `sync_collision`,
`propagate_transforms`, and `end_input_frame` stay last. A constraint on the closed side is rejected. `animate` may have systems
on either side when the constraint is explicit. `Game::step` restores the frame delta after the fixed
loop; that restore is not a system.

Deferred commands flush after each stage run, and after each fixed iteration. A command applies
through the existing `World` methods, in queue order. A failure is recorded and the queue continues.
Events sent in a stage become visible to later stages when that run finishes. `fixed_update` is one
stage, so its events stay invisible across the remaining iterations and become visible to `update`.
The queues are cleared after `input_clear`. A send in that stage is not delivered later in the step
or on the next step. `discarded_events` counts those pending events until the next step. Extract and
render do not read events. `command_failures` is the only command-dispatch record. Each entry names
the stage that flushed it. The list stays readable after `step` returns. `step` logs each failure
once at the end; the next step clears the list before systems run. A deferred scene load, unload, or
replace uses that same queue. Its outcome is a `SceneOpStatus` on the scene manager, not a
`CommandFailure`. A failure is logged once, through the `log` crate, when that status is stored.
`step` does not clear scene-operation results, and it does not log the same failure again.

`Diagnostics` is overwritten at the end of `step`. `frame` is the `dt` argument (simulation time).
`stages` is CPU time around each world stage, measured with `Instant`. `stages.fixed_update` is the
CPU time of every fixed iteration in the step, not the fixed timestep. `fixed_steps` is the
iteration count. `render` is not written by `step`; it stays the most recent successful render
until the runner or `render_to_rgba` copies new `RenderStats`. Extract and render stay outside
`step`.

Logging uses the `log` crate (ADR-0012). A failed load logs once when it becomes failed. A pending
load does not. Extraction logs a missing, failed, or invalid renderable asset once per id, remembering
at most 64 ids. A lost surface logs once and the renderer reconfigures. A window-init or render error
logs once and the runner exits. Discarded events are counted and not logged.

```text
time (advance Time, accumulate the fixed step)
  -> loads (AssetManager::process_loads)
  -> fixed_update (0 to 8 times; each iteration copies marked locals, then runs systems,
     then sync_collision; Time::delta is the fixed step, then restored; the first iteration
     then conceals the fixed-step input latch)
  -> update (user systems; both examples stay here; they see the frame delta)
  -> animate (animate_system, frame delta)
  -> post_update (propagate_transforms; full pass, no dirty tracking)
  -> input_clear (Input::end_frame and Actions::end_frame; frame edges only)
```

`sync_collision` is the last system of each fixed iteration
([ADR-0025](adr/0025-kinematic-collision.md)). It is the only fixed-step caller of
`propagate_transforms`, once per iteration, and then rebuilds `CollisionWorld`. A step with no fixed
iteration does not call it and does not flush the fixed-update command queue from that loop. `time`
and `loads` still flush before the loop, so a command queued before `step` is already applied when
the first iteration rebuilds. A command queued by a fixed system flushes after `sync_collision` in
that iteration: the entity is visible to `update`, and the snapshot omits it until the next
iteration that rebuilds. A following step with zero fixed iterations leaves that snapshot in place.
A direct `World` write in a fixed system, before `sync_collision`, is part of that iteration's
snapshot. A `Transform`, `Parent`, or `Collider` write after the system, including one in `update`,
is absent until the next iteration. `post_update` still propagates after animation and does not
rebuild the index. Interpolation snapshots locals at the start of each iteration and does not read
`CollisionWorld`; a zero-iteration step does not take that snapshot. Collision wires are copied into
`DebugDraw` only when `CollisionDebug` is enabled. It defaults to off. Indexing and queries do not
read that flag. After the rebuild, each `CharacterMotor` that has a `MoveIntent` is swept in
ascending `Entity` order. The intent is a world-space displacement for that iteration, not a
velocity and not a camera direction. World axes stay world axes: +X right, +Y up, +Z forward into
the scene, and XZ is the ground plane. A camera rotation does not rotate them. Locked axes are
cleared on the request, on each slide, and on the one recovery push. The sweep tests solid
colliders in a bounds volume that covers the start and the end of the remaining displacement, plus
skin. A gap inside `(-1e-4, skin + 1e-4]` is classified with the closest-feature wall: motion into
that wall blocks at time zero, and tangent or separating motion drops that candidate while the other
candidates are still tested. A wider gap keeps the shape-cast time of impact and slides on the
feature wall at the impact pose. Deeper overlap stays the one recovery push. The traveled part is
applied, and only the unconsumed remainder is projected along the wall. The motor writes `Transform.position` through the parent inverse when there is a
parent, and does not write `GlobalTransform`, rotation, or scale. It then patches that collider's
center, pose, and bounds so the next motor in the same iteration sees the new pose. `MoveResult`
records that resolution. It is not a claim that the character is standing on a surface. After the
fixed loop, including a loop of zero iterations, every leftover `MoveIntent` is removed. An intent
written during `update` waits for the next simulating step. Trigger events, gravity, jumping, and a
grounded flag are not implemented.

`elapsed` and `frame` advance once per `step`, not once per fixed iteration. Extract and render are
not stages: `Game::extract()` then `WindowRenderer::render` (or `render_to_rgba` offscreen).

`step` publishes the fixed-step input latch before `time`. The first fixed iteration sees those
edges; conceal then drops them, and a later iteration does not. A step with no fixed iteration does
not conceal. Frame edges still clear only in `input_clear`. `Actions` is published and concealed
with `Input` (ADR-0013). `spatial_scene` reads actions for its keys. `basic_2_5d` reads raw `Input`.
Scroll stays on `Input`.

`Viewport` is the window's physical framebuffer (`u32` pixels), a finite positive display scale
(`f64`), and a logical size of `physical / scale` (`f32`). Both pointer spaces are top-left, +X
right, +Y down. `Input::mouse_position` stays physical. `physical_to_logical` and
`logical_to_physical` do not clamp and do not flip Y. A scale event does not resize the
framebuffer; the following resize stores the physical size the OS chose. A zero resize keeps the
last positive framebuffer. Camera extraction and the GPU surface use the physical size. World
transforms stay Y-up and do not move when the scale changes (ADR-0013).

`Interpolate` is an opt-in component (ADR-0007). The fixed loop copies that entity's current local
`Transform` into `Interpolate::previous` before user fixed systems. `FixedTimestep::alpha` is a
finite fraction in `0.0..=1.0` of the accumulator; the eight-step stall drops the leftover and alpha
is then 0. Extraction blends `previous` toward the current local with `Transform::lerp` (translation
and scale lerp, `Quat::slerp` on the shortest arc) and composes those locals up the parent chain.
The result is a temporary render matrix. It is not stored, and it is not written to `Transform` or
`GlobalTransform`. Fixed-update and variable-update systems therefore keep reading the simulation.
An unmarked child of a marked parent uses its current local on the parent's blended local, so it
follows the smoothed parent. A non-finite history, factor, or result uses the current local.
`Interpolate::snap` sets the stored previous local to a transform the caller passes, which is how a
teleport avoids sliding. `detach_children` does the same for each direct child whose local it
rewrites into world space, and leaves descendants' history alone. `set_parent` and removing
`Parent` leave both samples unchanged. A variable-step write of a marked local is included in the
blend and is not a supported motion mode. With no stored previous anywhere, extraction reads
`GlobalTransform` as before. Sprites, meshes, tilemaps, lights, and the active camera share that
one matrix, for every sprite orientation and every camera placement. An unmarked camera matches
`GlobalTransform`. Depth sorting is unchanged.

Extraction (`extract_scene`): collect every active camera in `(order, entity id)` order. Materials,
textures, and mesh assets are resolved once for the frame. Each camera then builds its `CameraView`,
including the orthographic pixel snap when that camera's flag is set,
frustum, and visible instances. Billboard and upright quads use that camera's basis. Lights above 16
keep directionals and the positional lights nearest to that camera. Blended sprites and blended meshes
sort per camera by `(RenderOrder.layer, view depth far->near, RenderOrder.order, extraction order)`.
Cutout sprites, tiles and opaque meshes are unordered except for grouping by texture, material, and
mesh. Sprites and tiles share one instanced quad path. A missing asset is counted once on the scene,
not once per camera. `DebugDraw`, when the world has one, is copied onto `RenderScene::debug_lines`
and is not removed from the world. The copy is one list for every camera. It is not frustum-culled.
A missing resource is an empty list. `Game::step` clears the resource before systems, so a later
extract of the same step still sees what those systems submitted, and the next step drops it.

Backend: each destination has a `Depth32Float` image, cleared to 0.0. A shaded camera discards that
depth. A camera whose `presentation` is `Depth` stores it. A second pass in the same encoder then
loads color, binds no depth attachment, and samples a second view of that image inside the same
viewport and scissor. The sample happens before this camera is submitted, because the next camera
clears the whole depth image. The sample bind group is created the first time that image is presented
as depth and is kept until the image is replaced. A shaded frame does not create it and does not run
the pass. Depth images are created with `RENDER_ATTACHMENT | TEXTURE_BINDING`, so the toggle does not
allocate a second copy. The surface depth is not reused for an offscreen target. The first camera that writes a
destination clears that whole color image. Later cameras on it load the color. A later
`CameraClear::Color` fills only its viewport. Opaque meshes (back-face culled), mask and cutout
sprites and tiles write depth. Blended sprites and meshes test depth and do not write it. When
`debug_lines` is non-empty, one line-list draw follows that sequence: the same camera matrices,
greater-than depth test, no depth write, unlit vertex color, alpha blend. The vertices are uploaded
once per frame and drawn by each camera. An empty list does not upload and does not draw. Each camera
is uploaded and submitted before the next camera writes the shared globals or instance buffers.
Texture, sampler, and mesh-geometry caches stay shared. Offscreen color images stay in a renderer
cache keyed by `RenderTargetId` and are replaced when the description's generation changes. A sprite
samples one through the existing texture bind group. Lighting is a single forward world-space
`shade()` shared by sprites, tiles and meshes: ambient plus up to 16 directional, point, or spot
lights, wrap-lit sprites, exponential distance fog. Surface loss and resize are handled; transient
acquire failures skip the frame. There is no render graph.

## Current extension points

These exist and work:

* **Components + systems.** New renderable or gameplay data is a component; systems are
  `FnMut(&mut World)` added to a stage (`Game::add_system` is the `update` stage, `add_fixed_system`
  is `fixed_update`, `add_to_stage` is any stage) and ordered with `before` / `after` inside that stage.
* **`RenderScene` consumers.** Any backend (or test tool) that takes a `RenderScene` works with
  extraction unchanged. `render_to_rgba` already serves headless screenshots and GPU tests.
* **`AssetSource`.** A new byte source (a bundle reader) plugs in without touching components.
* **`Asset::decode`.** Adding a loader for a type is implementing one function.
* **`Projection` trait and `non_exhaustive` `CameraProjection`.** Partial, see above.
* **`Sprite.orientation`** (`#[non_exhaustive]`) and the single function `sprite_placement`.
* **`Material`** is the inline mesh field. CPU `MaterialAsset` is the shared asset. Extraction does not read it yet.
* **Commands and events.** `World::commands` queues spawn, despawn, insert, remove, `set_parent`
  and `detach_children` until the stage flush. `World::send` / `World::events` are typed queues
  with no callbacks. A new event type needs no registration.

These are *not* extension points yet, despite existing as ideas in earlier docs: there is no shadow or
normal-map hook of any kind, no render graph or custom pass API, no user shader support, no ordering
across stages except the fixed stage list, no asset-loading pipeline beyond PNG.

## Known architectural weaknesses

Each is verified by reading the code or by a test run; identifiers refer to the known-issues table in
[TASKS.md](../TASKS.md).

1. **Resource lifetime (KI-4).** Fixed by T2: `AssetManager::remove` exists and the GPU caches evict removed
   and idle entries. Remaining limit: only textures and meshes are cached per asset, instance buffers only
   grow, and nothing calls `remove` for you. Scene unload will not either (ADR-0014): membership does not own assets.
2. **Hierarchy robustness (KI-7, KI-8).** Fixed by T3: an entity without a `Transform` counts as
   identity and its children are still placed, and `despawn` removes the whole subtree (D-03;
   `detach_children` keeps children as roots at the same world transform). Remaining limit: there is no
   dirty tracking, so the pass is O(entities) every frame.
3. **Scheduling (KI-9, KI-10).** Named stages and `before` / `after` are in `Game::step` (ADR-0007, M3a-2).
   Deferred commands and typed events are implemented (ADR-0008, M3a-3). Still absent: change detection (ADR-0009)
   and exposure of `FixedTimestep::alpha()` (M3a-5). Resource access is still `RefCell`: a second borrow
   of the same resource panics, including a second `World::commands` while a writer is alive. Direct
   `spawn` during a query still does not compile; the command queue is the path that does.
4. **One camera per frame (KI-5).** Fixed locally. M6-2B renders every active surface camera. M6-2C
   adds offscreen targets and lets a later sprite sample one. The editor viewport UI remains M22.
5. **Custom projection (KI-11).** Fixed in M6-1B: `CameraProjection::Custom(Mat4)` is ordinary data on the
   same `Camera`. Each camera pass still receives one `view_proj`. Projection kind is not a pipeline.
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
10. **Per-frame CPU cost.** Extraction walks sprites and meshes each frame. A tilemap is divided
    into 32×32 cull squares. A camera tests every square, then reads tiles only inside squares that
    meet the frustum. Rejected squares add a stored occupancy count and do not build matrices.
    A direct write to `tiles` is still read on the next extract; only that occupancy count goes
    stale until `invalidate`. Instances are not cached. A camera that sees the whole map still
    reads every cell. `set` rebuilds one square's count. The release numbers, and the reading of
    the allocation column as traffic rather than resident memory, are in
    [PERFORMANCE.md](PERFORMANCE.md). Sprite, mesh, and mixed-scene extraction, and the CPU cost of
    recording and submitting a frame, are in that file (`82972c2`). A frustum rejection does not skip
    the sprite prepare walk. GPU pass timestamps are still not in that record (KI-19). The
    full-view tile instance traffic is KI-31 and stays deferred. The scale harness calls
    `extract_scene` and `SceneRenderer::render`. It is not a second extractor.
    `instance_buffer_capacity_bytes` reports reserved instance-buffer bytes and does not change a draw.
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
14. **UI foundations (KI-24, KI-25).** The window scale factor and physical versus logical `Viewport`
    exist (M3b Phase 3, `9c99edf`). Input still has no text events, and any texture change re-uploads the whole
    texture, which a glyph atlas cannot afford. Reference-resolution UI scaling is still D-39.

## Cross-system requirements and risks

The systems in [CAPABILITIES.md](CAPABILITIES.md) are only useful if they hold together. This section
states, for each integration question, what is true today (checked in code or by a test), the requirement,
and the gap. It then lists the architectural risks that would be expensive to fix late. Requirements
here are binding on milestone designs; open choices point to a decision in [DECISIONS.md](DECISIONS.md).

### Integration requirements

**1. Sprites and meshes under one camera and one set of conventions.**
*Today:* true and tested. Both are entities with a `GlobalTransform`; extraction projects both with one
`view_proj`; one `Depth32Float` buffer; a depth presentation reads that same buffer after the camera's
scene pass and does not extract a second scene; GPU tests show a nearer mesh occluding a sprite and the reverse
(`nearer_mesh_occludes_sprite_and_farther_mesh_does_not`), blended sprites interleaving with meshes
(`blended_sprites_interleave_with_meshes_by_depth` on one camera, and
`a_blended_sprite_interleaves_with_meshes_by_depth_in_every_configuration` on the seven example cameras),
and `sprites_and_meshes_share_one_spatial_world`.
Blended meshes and blended sprites share `RenderScene.transparent` and one GPU submission order.
*Requirement:* any new renderable (tile, particle, skinned mesh, world-UI quad) enters through the same
transform, camera and depth contract and declares one **transparency class**: opaque, cutout, or blended.
*Gap:* particles and skinned characters are not in that sequence yet. See R-1.

**2. Depth, transparency, render layers and occlusion.** The rule to preserve: *depth decides
visibility; a transparency class decides the pass; sort keys exist only for blended content.* Opaque
and cutout content needs no ordering. Blended content is ordered by `(RenderOrder.layer, view depth,
RenderOrder.order)`; `RenderOrder` is a narrow override, not a layer system. Four concepts stay distinct
(ADR-0004, amended 2026-10-13): spatial depth, transparency sorting, render layers or queues, and explicit
render order; an arbitrary sort value never replaces spatial depth, deliberate ordering of coplanar sprites is
`CoplanarPriority` (ADR-0020: depth-only, opt-in, not a batch key, not a blend sort), and room is left for sort or occlusion
policies for isometric styles without building an occlusion framework now. Shadows, particles and
skinned meshes must pick a class and may not introduce a second ordering concept. Screen-space UI is
outside this model (painter's order, no depth). Blended meshes use the bounding-sphere centre.
Intersecting blended objects stay an accepted limit (D-05, KI-14), not a second ordering policy.
The depth presentation displays the values those rules stored. It does not add a sort, a layer, or a
second depth convention. An internal-resolution camera stores those values in its private depth
image, linearizes that image, and then nearest-blits the color. The blit does not light, fog, or tint.

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
(fixed step), never the interpolated render transform. The opt-in previous local for that render
transform is `Interpolate` (ADR-0007, M3a-5). Debug draw is the way to see all of it.

**5. Screen-space and world-space UI.** Two subsystems with a thin join: the UI runtime owns layout,
interaction and a plain-data draw list; the backend draws it in an overlay pass. They share the device,
texture cache, samplers, atlases (the glyph atlas is an ordinary texture) and the instanced-quad
machinery. They do not share the ordering model, coordinate space, lighting or input routing.
World-space UI is a UI node anchored to an entity and positioned by projecting through the active camera
(a behind-camera policy is required); render-to-texture panels are a later option (D-38).
*Gaps:* device scale and logical pointer conversion exist (M3b Phase 3, `9c99edf`); proved rectangle upload exists (M5-3B, `84d5ab1`); a CPU shelf packer is commit `bf4d9bb` (M5-4A); GPU atlas samples are commit `1666695` (M5-4B). No UI layout and no overlay pass. The glyph atlas consumer is still M10. Reference-resolution modes stay in D-39.

**6. Asset loading, referencing and retention.** *Today:* components hold `AssetHandle<T>`, a copyable
64-bit hash of a name; the manager owns the data; the renderer keeps GPU copies keyed by id and revision;
nothing is reference counted; removal is explicit (`remove`) and the renderer follows it (T2).
*Requirement:* every system that holds an asset resolves it through the manager per use (never caches
derived asset data in a component), and failures are reported with the asset name (D-21). Scene
membership does not own assets (ADR-0014): unloading a scene must not remove an asset another entity
or the application still needs. Exclusive release is unresolved. Dependency tracking stays M17. *Gap:* see R-4. D-16 stays open.

**7. Scripts and editor tools versus the renderer.** (The runtime holds no editor state, see requirement 11.) Neither may reach renderer or wgpu internals. Both
use the same surface: the component registry (names, visibility, field access), commands applied at
stage boundaries, events, and the action layer. An editor viewport needs camera-to-texture (D-08) and
picking through the collision/ray queries, not through GPU readback. The surface is listed in
`docs/API_SURFACE.md` at M9 (not yet written). *Today:* commands, events, and actions exist (M3a-3, M3b, ADR-0008, ADR-0013). The component
registry's names, field metadata, and text dumps exist (M4 Phase 1, ADR-0014). Scene membership,
script execution, and serialization do not.

**8. Serialization and hot reload shape foundational data.** To be serializable and reloadable,
components must be plain data with no GPU objects, borrowed references or cached derived values; every
asset reference must be restorable to an identity that survives rename (D-16); entity references need
remapping on load (entities are not stable across runs); a field added later needs a default. Hot
reload needs asset revisions (exist), a dependency graph (planned) and a rule that a reload takes effect
at a stage boundary. *Today:* components built so far satisfy the plain-data rule, but `Sprite` and
`Mesh` carry asset handles that hold only a hash, so a serializer cannot recover the name from a handle.
That is R-4.

**9. Which public APIs must stay stable, which stay internal.** The three tiers are the rule
([ADR-0010](adr/0010-three-tier-public-api.md), D-45). M3a-6 reviewed the re-exports after M3a-5
(`523f553`). The item-by-item inventory is still M9's `docs/API_SURFACE.md`, which is not written.
M3a adds no public `hecs` type, no new `hecs` re-export, and no `wgpu` or `winit` type on a new
public interface. It does not wrap `hecs`. Existing `winit` and `wgpu` re-exports stay.

Contract types M3a added, all plain data: `Interpolate`; `FixedTimestep::alpha`; `CommandWriter`,
`CommandError` and `CommandFailure`; caller-defined events through `World::send` and `World::events`;
`Diagnostics` and `StageDurations`. `snapshot_interpolation` is the copy the frame loop runs. It is
public on `engine-world` because `Game` is another crate, and it is not in the prelude.
`CommandWriter::spawn` and `insert` take `hecs::DynamicBundle`, the same trait `World::spawn` already
takes. That is the existing storage leak, not a new `hecs` type. The render-pose walk is `pub(crate)`.

| Tier | Contents | Change rule |
|---|---|---|
| **Contract** | Gameplay-facing components (`Transform`, `Name`, `Visibility`, `Camera`, `Sprite`, `Mesh`, `Light`, `Interpolate`), asset handles, commands, events, `Diagnostics` / `StageDurations`, `FixedTimestep::alpha`, and, once they exist, actions, the script API, and every on-disk format | Changed only through a recorded decision and, for formats, a version bump with migration |
| **Backend seam** | `RenderScene`, the future UI draw list, `Pose` | Changed with all consumers in one commit; documented |
| **Internal** | `hecs` types (`Entity`, `EntityRef`, `Query`, `QueryBorrow`, `World.entities`, `DynamicBundle` on spawn and command insert), schedule internals, `wgpu`/`winit` re-exports, `SceneRenderer` internals, extraction helpers including the render-pose walk | Free to change; not named in docs or examples as supported |

**10. One world, many camera configurations** ([ADR-0004](adr/0004-one-spatial-world-many-camera-configurations.md);
confirmed owner direction; mechanism decisions accepted or amended 2026-10-13. The test scene demonstrates seven
views. `spatial_scene` calls `PlacedCamera` and `CameraProjection::oblique_orthographic`; the example keeps its framing. Those constructors return an ordinary camera, transform, or `Custom` matrix. Coplanar sprite priority is implemented (ADR-0020). Tilemap extraction culls 32×32 chunks). A camera is a
`Transform` plus a projection; perspective, orthographic, isometric (orthographic, rotated), top-down,
side-scrolling and custom views are configurations of that one model with no per-view renderer, extraction path
or world representation. Presets for common views are constructors that return a `Camera` and a `Transform` and
stay fully editable; isometric is an orthographic preset, not a projection type. Style policies (sprite
orientation, filtering, snapping, fog) live at the object, material, camera, renderer or scene level where each
belongs, with no universal style type. Gameplay orientation is data, never derived only from the camera or the
sprite. Keep three things
separate: *projection* (how the world becomes pixels), *gameplay orientation* (up axis, ground plane,
facing), *visual style* (snapping, fog, mood). *Today:* the camera maths takes any `Transform` and a perspective, orthographic, or custom view-to-clip
matrix. The tilemap is a quad grid in its entity's local XY plane that is laid flat by rotating the
entity. Extraction culls that grid in 32×32 chunks with the same frustum and does not add a tilemap
camera or a second pass. Sprite orientation modes are tested under a pitched camera. *Demonstrated (2026-10-14):* the same
`spatial_scene` world through example-local perspective, orthographic, isometric, top-down, steep, side, and
custom constructors, with GPU tests. Those functions call the public constructors and keep the scene framing. *Custom projection (M6-1B, `a38cb30`):*
`CameraProjection::Custom(Mat4)` renders and picks through the same extraction and `view_proj` uniform.
`CameraProjection::oblique_orthographic` and `oblique_perspective` (`16ed495`) build that matrix from a reverse-Z helper and a view shear. They do not convert a matrix already stored with `custom`, and a resize does not rebuild it. The example `custom` constructor calls the orthographic builder. The pose stays on `Transform`. There is no hidden aspect field beside the matrix.
*Not demonstrated:* a 2D-style game. Several surface cameras and an offscreen target sampled by a sprite render in one frame (D-08). *Gaps:* R-16, including the three
verified coplanar conflicts (D-48). Sprite orientation is [ADR-0021](adr/0021-sprite-orientation.md): oblique shear keeps the camera basis, and a child does not inherit a rendered billboard frame.

**11. The runtime has no editor state.** Editor and developer tools consume the component registry, commands,
events and diagnostics and live in separate crates or binaries (ADR-0003). The runtime contains no editor
types, no `cfg(editor)` state and no code path that only the editor can reach. `CameraPresentation`
is a camera field. A later editor viewport sets it. It is not an editor type.

### Risk register

"Cost" is the likely cost of getting it wrong and finding out late. Items are ordered by how many
systems they touch. Each cites evidence from this audit.

| Id | Risk | Evidence | Cost if found late | Response |
|---|---|---|---|---|
| R-1 | **Later transparent primitives still need the shared sequence.** Blended sprites and blended meshes sort and submit together. Particles and skinned characters are not in that list yet. Opaque and mask content stays in the depth-writing passes | `scene.rs` `transparent`; the walk in `scene_renderer.rs` | Moderate for the next transparent primitive if it grows a private list | D-05 is implemented for sprites and meshes (ADR-0015, M5-5C `ab1b6f7`, M5-5D `a7efd6c`). A new blended primitive joins `transparent` instead of a second order |
| R-2 | **Targets are explicit, not a graph.** `RenderScene.passes` is every drawable camera (D-08). A camera writes the surface or one `RenderTargetId`. A sprite samples a target only after an earlier camera in that frame has written it. There is no scheduler and no previous-frame fallback. The depth presentation is a second pass in the encoder of the camera that owns the destination. An internal-resolution blit is another pass in that same submission, after the scene and any depth view, and later cameras sample the public destination | `render_target.rs`; `extract.rs` `sprite_sample`; `scene_renderer.rs` `sync_render_targets`, `draw_depth_view`, `record_present` | High if a later effect assumes feedback or automatic ordering | D-08 is implemented locally. The depth view and the integer blit do not add a renderer or a graph. Do not add one for shadows or the editor viewport |
| R-3 | **No transform timeline beyond one previous local.** `Transform` is current simulation state. `Interpolate` stores one previous local for opt-in extract interpolation. There is still no dirty tracking, propagation is still a full pass, and camera-dependent animation still has a one-frame latency rule | `interpolate.rs`, `hierarchy.rs`; KI-9, KI-10, D-02 | Moderate if a later system starts reading the render matrix as simulation | M3a-5 stores the previous local and blends only at extract. Dirty tracking stays out (ADR-0009) until a profile |
| R-4 | **Asset identity and retention.** Handles are name hashes with no recoverable name, no reference counting, ids change on rename, and the collision guard is per type | `handle.rs`, `manager.rs`; D-16 | High once scene files, sheets or bundles exist | Decide D-16 before any persisted format (sheet descriptors in M8 are the first); keep names in the manager so handles can be serialized by name or id |
| R-5 | **Mutation model.** Systems may still mutate the world directly. Resources are `RefCell` (a double borrow panics). Commands and events exist and follow ADR-0008 | `commands.rs`, `events.rs`, `resources.rs`; KI-9 | A second mutation path, or holding the command writer across a flush, reintroduces the alias | Built in M3a-3. Direct `spawn` remains for code that holds no query. No observers |
| R-6 | **No format versioning policy.** Nothing persisted exists yet, which is the cheap moment to set one | no `serde`, no formats | Very high after the first release | Settled in ADR-0011. The first encoder writes a version; loaders return errors for unknown versions. No format in M3a |
| R-7 | **Public API leakage.** The facade re-exports every crate, including `engine_platform::winit` and `engine_renderer_wgpu::wgpu`; `World.entities` is a public `hecs::World`; `Entity` is hecs's | `engine/src/lib.rs`, `world.rs`, `platform/src/lib.rs` | High after M9 and M18 promise stability | Rule settled in ADR-0010. The leak stays until M9's list. M3a adds no public `hecs` type and does not wrap `hecs` |
| R-8 | **Threading model.** `World` is `!Sync` (`RefCell` resources), systems are non-`Send` closures, asset data is `Send + Sync`. The simulation is single-threaded; async loading, parallel extraction and scripts cross this line only as `Send` data | `schedule.rs`, `resources.rs`, `Asset` bound | High if parallel systems are wanted later | Settled in ADR-0006. The calling thread owns `World`. M3a's scheduler must not assume parallel systems |
| R-9 | **Numeric determinism.** `f32` throughout; trigonometry comes from the platform libm and can differ between operating systems, so state hashes for replay may differ across OSes. World size is small enough for `f32` positions today | `engine-math`; M9 replay criterion | Moderate; surfaces as a flaky cross-OS replay test | Policy at M3: replay equality is required on one platform; cross-platform equality is not promised unless simulation avoids platform trig |
| R-10 | **Animation write path and bone cost.** `animate_system` writes straight onto bone entities; entity-per-bone makes propagation scale with bone count | `clip.rs`; D-33, D-34 | Moderate; contained inside `engine-animation` if the evaluator stays world-free | D-34 first in M11; D-33 by measurement |
| R-11 | **Material and texture format still incomplete.** Assigned sprites and meshes read `MaterialAsset`. Unassigned meshes still read the inline `Material`. Batching is keyed by pipeline, material id and revision, and texture. One thousand compatible opaque sprites are one draw (M5-5D, `a7efd6c`). Proved region journals upload rectangles (M5-3B, `84d5ab1`). Mip generation and per-texture colour space exist. A CPU shelf packer is commit `bf4d9bb`. GPU atlas samples are commit `1666695` and isolate regions only for the layouts in ADR-0017. Localized mip regeneration does not. Plane, cube, cuboid, sphere, and cylinder generators exist as ordinary `MeshData` (ADR-0018, M5-6 `92286ce`). M5 is accepted locally. There is no mesh loader. Localized mip-texel regeneration is not implemented | `components.rs`, `material.rs`, `mesh.rs`, `scene_renderer.rs`, `atlas.rs`; KI-6, KI-25 | Moderate: normal maps, glyph atlases and shadows depend on the rest | Localized mips, then a loader before M12 |
| R-12 | **Logical versus physical pixels.** `Viewport` stores physical pixels, logical pixels, and the device scale. Rendering and extraction use physical pixels. UI reference-resolution modes are still open | KI-24 (DPI part done in M3b Phase 3, `9c99edf`); D-39 | Lower for the coordinate split. UI fit modes can still spread if they invent a third size | Device scale is M3b Phase 3. Fit/expand/integer modes stay with D-39 at M10 |
| R-13 | **Diagnostics are counters, not a profile.** Logging is the `log` crate. `Diagnostics` holds one step of CPU stage times, the frame delta, and the latest render stats. There is no history, no per-system timing, and no GPU timestamp | `logging.rs`, `diagnostics.rs`, `missing.rs`; ADR-0012 | Low for the current call sites. A profiler bolted on later would duplicate them | M3a-4 implemented the records and the resource. M20 may add `tracing` or another profiler. The on-screen overlay stays in M10 |
| R-14 | **Verification is narrower than it looks.** CI now runs on Windows, macOS and Linux (first run green), but only on software or paravirtual adapters, offscreen; GPU tests skip unless `DIMORA_REQUIRE_GPU=1` (CI sets it). A macOS window was checked for DPI (Phase 3). Phase 4's macOS window check is the one recorded in TASKS.md; Windows and Linux windowed paths are unverified. Performance is unmeasured. Pixel tests use tolerances that held on three rasterisers | TASKS.md "CI results" and "M3b Phase 4 results"; KI-3, KI-24, KI-27 | Rises slowly; regressions on real GPUs or untested windowed paths found by users | Keep CI green and required. Real-GPU timing needs a different runner (ADR-0005 option B) |
| R-15 | **Foundations not yet shaped for the editor.** Registered components now have hand-written field names and a persistence class (M4 Phase 1, ADR-0014). Those lists are not derived from the structs. Unregistered components still have none. There is no undo stack, entities and assets still lack stable file ids (R-4), there is no scene format, and the renderer draws one view (R-2) | `registry.rs`, `components.rs`, `world.rs`, `extract.rs`; CAPABILITIES rows 2, 8, 32, 33, 36 | High for the parts still missing: commands exist, but editor undo, identities, and a viewport do not | M4 still owes scene membership. M6 owes camera-to-texture. M19 owes serialization. Write no component that hides state the editor must edit |
| R-16 | **Single-view assumptions hide in details.** Evidence: `Upright` sprites are edge-on under a straight-down camera and foreshortened in a steep view (settled, ADR-0021); cutout content that has not opted in still follows draw order or depth-test rounding (`spatial_scene` does not attach `CoplanarPriority`; ADR-0020 is the opt-in); the world is +Y up with an XZ ground (a provisional convention, ADR-0004). `CharacterMotor.locked_axes` is an explicit world-axis lock, and the gameplay-orientation component is still open; directional animation (D-31) will read the camera; shadow and light code is not yet written and could assume a perspective view. Isometric, top-down and side are ordinary cameras built by `PlacedCamera` | `components.rs` (`SpriteOrientation`), `extract.rs`, `coplanar.rs`, `scene_renderer.rs`; `examples/spatial_scene/tests/configurations.rs`; ADR-0004; ADR-0020 | High if found late: orientation and light direction are baked into M7, M8, M13 and M14 | D-26 is settled (ADR-0021). Gameplay orientation is still open before M7 and M8. Coplanar sprite priority is settled (ADR-0020). M14 tests under three camera configurations |

Expensive redesigns, ranked by likelihood times cost, if nothing changes: **R-4 (asset identity)**, **R-1
and R-2 (render model shape)**, **R-5 and R-8 (mutation and threading)**, **R-7 and R-6 (API and format
promises)**, **R-15 (editor-shaped foundations)**, which compounds R-4 and R-5, and **R-16 (single-view
assumptions)**, which compounds R-1 and R-2. Everything else is local to one system.

## Decisions already made

These are settled; revisit only with a concrete problem.

| Decision | Choice | Why / trade-off |
|---|---|---|
| ECS | `hecs` wrapped by `engine_world::World` | Small archetype ECS with no app framework; gameplay goes through our `World`. **Caveat (R-7): the wrapper does not hide `hecs` today** (`World.entities` is public, `query`/`get` return hecs types, `Entity` is hecs's). ADR-0010 does not wrap `hecs` in M3a; it stops new public `hecs` types and leaves the existing leak until M9. `bevy_ecs` is more capable (scheduling, change detection) but would shape the engine around its conventions. Cost: we build scheduling and events ourselves; change tracking waits on a profile (ADR-0009) |
| Simulation threading (D-44) | The calling thread owns `World` for the whole step; other threads see `Send` snapshots only | ADR-0006. Already how the code works. Parallel gameplay systems are a later decision |
| Change detection (D-02) | None until a recorded profile | ADR-0009, Provisional. The full propagation pass stays. The interpolation previous-local is not a dirty bit |
| Scheduling (D-01) | Named stages on the existing `Schedule`, `before` / `after` by system name, fixed-step delta restored after `fixed_update`, opt-in extract interpolation | ADR-0007. Stages in M3a-2. Interpolation in M3a-5 (committed locally, not pushed). One thread (ADR-0006) |
| Commands and events (D-10) | One FIFO command queue flushed per stage run, including each fixed iteration. Typed events become visible at the next stage boundary and clear after `input_clear` | ADR-0008, implemented in M3a-3. No observers. A fixed iteration does not see events from the same stage |
| Logging (D-21) | The `log` crate, a stderr logger only when none is installed, and a `Diagnostics` resource | ADR-0012, implemented in M3a-4. Not a profiler. Profiling stays in M20 |
| Graphics | `wgpu` | One API over Vulkan/Metal/DX12 |
| Windowing | `winit` 0.30 (`ApplicationHandler`) | Standard; surfaces work with wgpu |
| Math | Own `f32` crate | Small, deterministic, shaped for `Transform`/`Quat` |
| Handedness | Left-handed, `+Z` deeper | "Z = 800" means far |
| Extraction seam | World -> plain `RenderScene` -> backend | Keeps the world GPU-free; enables headless tests and backends |
| Ordering | Depth buffer for opaque and mask; one sorted list for blended sprites and blended meshes; `RenderOrder` optional inside that list. `CoplanarPriority` pulls an opted-in sprite's depth only (ADR-0020) | Position, not layers, decides visibility. The pull is bounded by `(1 + level) * thickness` |
| Sprite geometry | Unit quad + per-instance matrix; orientation resolved in extraction | One shader path for sprites, tiles, bones |
| Sprite orientation | `Billboard` (default), `Upright`, `Fixed` | [ADR-0021](adr/0021-sprite-orientation.md). See SPATIAL_RENDERING.md |
| Hierarchy | `Parent`/`Children` components + `GlobalTransform` system | Bones and attachments are ordinary entities |
| Assets | Typed handles holding a name hash; manager owns data; sources behind a trait | Components can reference assets before they load |
| Despawn (D-03) | `despawn` removes the whole subtree; `detach_children` keeps children as roots with their world transform | Bones and attachments are owned by their parent, so orphans are never wanted. Cost: a caller who wants children to survive must say so |
| GPU cache lifetime (D-04) | Evict a cached texture or mesh when its asset is removed, and when unused for N frames (configurable) | Removed assets are freed promptly and off-screen assets are not re-uploaded every frame. Cost: an asset unused for N frames is re-uploaded when it returns |
| Scene model and registry (D-18) | One world; internal `SceneManager`; `SceneBuilder` without `&mut World`; load publishes only on success; unload lifts non-members and does not remove assets; deferred scene ops use the command queue and store `SceneOpStatus`; `spatial_scene` replaces one environment at the stage flush; a prefab is `fn(&mut World) -> Entity` | ADR-0014. Phase 1 is `cdb5ec3`. Phase 2 is `2c76f71`. Phase 3 is `f9a4aab`. M4-4A is `302622b`. M4-4B is `128c70e`. M4-4C is `e12fb5b`. M4-4D is `8398e49`. M4-4E is `d58c66b`. M4-5 is `2b79806`. Scene files stay in M19. D-16 stays open |
| Material alpha classes (D-05) | Opaque, Mask, and Blend. Blended sprites and meshes share one sort and one GPU submission list. A blended mesh uses the camera-space Z of its world-space bounding-sphere centre. `RenderOrder` applies to that list only. Opt-in coplanar priority is ADR-0020 and does not reorder that list. Content that does not opt in still ties | ADR-0015. M5-5A is `e0c29ef`. M5-5B is `3b3cd4a`. M5-5C is `ab1b6f7`. M5-5D (`a7efd6c`) batches by material identity and revision. One thousand compatible opaque sprites are one draw. Procedural meshes are ADR-0018 (M5-6, `92286ce`). M5 is accepted locally |
| Sprite units (D-27) | Per-sprite `pixels_per_unit`, default 16. No global constant. Atlases are in texels | ADR-0016. The field already exists. M5-1 does not change it |
| Texture sampling (M5) | Per-texture sRGB or linear. GPU image storage and samplers are separate caches. `write_texture` keeps a tight row stride. A CPU shelf packer places RGBA8 images in insertion order. GPU atlas isolation is the measured layouts in ADR-0017, not `floor(log2(padding))` | ADR-0017. M5-2B (`11b914f`) reads `Texture.sampler` and selects the format from `ColorSpace`. M5-2C (`622ef80`) generates `Mipmaps::Generate` in the wgpu backend. M5-3A (`b51f1fb`) records CPU region transitions. M5-3B (`84d5ab1`) uploads proved rectangles and rebuilds the whole allocated chain. M5-4A (`bf4d9bb`) adds `AtlasBuilder`. M5-4B (`1666695`) samples those atlases and adds `Atlas::replace_source`. Localized mip regeneration is not implemented |

## Proposed future architecture (NOT implemented)

Direction only. A bullet names a checkpoint when that part is already in the code. Where an ADR is
cited, the decision is settled. Each item is a roadmap milestone with its own acceptance
criteria. Open decisions stay in [DECISIONS.md](DECISIONS.md).

* **Scenes as units** (M4): the registry and text dumps exist (Phase 1, `cdb5ec3`, ADR-0014). Extraction
  omits a sprite, mesh, tilemap, or light whose ancestor chain contains `Visibility::Hidden` (Phase 2,
  `2c76f71`). `find_by_name` and `find_by_tag` scan live entities (Phase 3, `f9a4aab`). M4-4A records
  empty instances and `SceneMember` (`302622b`). M4-4B loads a code-defined scene through `SceneBuilder`
  (`128c70e`). M4-4C unloads and replaces an instance without removing assets (`e12fb5b`). M4-4D queues
  those operations on the command queue (`8398e49`). M4-4E uses that queue to switch `spatial_scene`
  between two code-defined environments (`d58c66b`). The camera and the walker are not scene members.
  M4-5 adds `spawn_lamp` as the code-defined prefab and makes the empty-instance helpers crate-private.
  That change is commit `2b79806` and is not pushed. Serialization stays in M19.
* **Material-driven rendering** (rest of M5). Sampler parameters, colour space, mip generation, rectangle
  uploads, and GPU atlas sampling are implemented. Assigned opaque, mask, and blend sprites and meshes
  draw through the existing renderer. M5-5D (`a7efd6c`) batches 1,000 compatible opaque
  sprites into one instanced draw. M5-6 (`92286ce`) adds plane, cube, cuboid, sphere,
  and cylinder meshes as ordinary `MeshData` (ADR-0018). M5 is accepted locally. Localized mip-texel regeneration is not implemented.
* **A list of camera passes** instead of one `RenderScene` camera, plus render-to-texture (M6).
* **Kinematic movement** on the collision world (rest of M7). M7-1A rebuilds `CollisionWorld` in the
  fixed stage and answers overlap and ray queries. M7-1B-1 sweeps `CharacterMotor` in that same
  system and writes the local position. The contract is
  [ADR-0025](adr/0025-kinematic-collision.md): private `parry3d` 0.31.1 queries, ECS-owned
  transforms, a linear AABB list, and no rigid-body pipeline. A nonblocking zero-time skin contact
  is dropped and the rest of that remainder is still tested. Trigger events, gravity, jumping,
  grounded support, and the `spatial_scene` pillar block are not written.
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
* **2D only:** slots and part draw order, skins and part swapping, sprite attachments. Visual orientation is
  applied once at the rendered visual root (ADR-0021). Bone-local poses and attachments stay in the
  simulation hierarchy. Part sprites are `Fixed` in that root frame. Per-bone `Billboard` or `Upright`
  is not the character policy. That root frame is not implemented. Gameplay facing stays a separate value.
* **3D only:** a skin (joint list, inverse bind matrices), vertex joints and weights, and the joint
  palette that goes to the backend as plain data in `RenderScene` (GPU skinning, D-35).
* A 3D character is a normal entity in the same world, with the same `Transform`, hierarchy,
  collider and gameplay components. There is no second world and no 3D-only gameplay path.
* Attachment points are named sockets. Whether every bone is an entity, or only sockets are, is decided by
  measurement (D-33).

**Unresolved.** One skeleton type or two (D-32); bone storage (D-33); skinning on the GPU or CPU (D-35);
glTF handedness conversion and importer choice (D-36).

### 4. World rendering and UI rendering

**Exists today.** Nothing UI-specific. `RenderScene` is world-only. No font or text rendering. Pointer,
scroll, and keys exist in input. The window scale factor is forwarded, and `Viewport` stores physical
size, logical size, and scale (M3b Phase 3, `9c99edf`). A proved texture rectangle can upload without
rewriting the whole image (M5-3B, `84d5ab1`). A CPU shelf packer is commit `bf4d9bb` (M5-4A). GPU
atlas samples are commit `1666695` (M5-4B). The glyph atlas that would use both is still absent (M10). Reference-resolution
UI scaling is not built (D-39).

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

**Exists today.** No scripting and no `serde`. Commands, events, and the component registry exist
(ADR-0008, ADR-0014). The registry stores script-visibility flags and no script reads them.

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

**Exists today.** Internal APIs: `World`, `AssetManager`, `Schedule`, the renderer and `wgpu`
are public Rust items. The component registry stores inert script-visible and script-writable flags (M4 Phase 1, ADR-0014). No script reads them.

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

**Unresolved.** Facade versus reflection (D-41). The registry already stores the flags as inert metadata (ADR-0014). D-41 still decides whether a script facade reads them.

