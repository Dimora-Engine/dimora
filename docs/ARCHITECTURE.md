# Architecture

> **2.5D is a spatial world, not a fake 2D layer system.**

Everything that can be seen (sprites, skeleton parts, tiles, 3D meshes) is an entity with a real
3D `Transform`. A camera plus a projection decides how that world appears on screen. There is no
integer z-index anywhere in the model; depth comes from position, and parallax, occlusion and
lighting fall out of it.

## Coordinate system

Left-handed, Y up, **+Z points away from the default camera** (into the screen):

* `+X` right, `+Y` up, `+Z` deeper into the scene.
* A camera with identity rotation looks down `+Z`.
* A sprite lies in its local XY plane and faces `-Z` (towards the default camera).
* Quaternion rotations are the usual right-hand-rule rotations about the given axis.
* Clip space follows wgpu/D3D: NDC z in `[0, 1]`.
* Mesh triangles are wound so `(c - a) x (b - a)` points along the outward normal (checked by tests); the GPU
  pipeline declares them `FrontFace::Ccw` with back-face culling. Sprites are two-sided.

## Crates

```
engine-math            Vec2/3/4, Quat, Mat3/4, Transform, Rect. No dependencies.
engine-core            Color, Time + FixedTimestep, Input state. No platform/GPU dependencies.
engine-world           ECS world (wraps `hecs`), resources, systems, Transform hierarchy.
engine-assets          AssetId/Kind/Handle<T>, AssetManager, sources, Texture/Sprite/Mesh data.
engine-animation       Skeleton definition, bones as hierarchy entities, clips, sampling.
engine-renderer        Backend-agnostic render model: components, projection, camera, lights,
                       extraction (world -> RenderScene), culling, sorting, batching. No GPU.
engine-renderer-wgpu   The wgpu backend. Consumes a RenderScene; knows nothing about the ECS.
engine-platform        winit window + event loop -> engine-core Input/Time. No rendering.
engine                 Facade: `Game`, schedules, prelude. Wires the crates together.
examples/basic_2_5d    Demo scene.
```

Dependency direction (arrows = "depends on"):

```
engine ──► platform ──► core ──► math
   │──► renderer-wgpu ──► renderer ──► world ──► math
   │                         │──► assets ──► math
   └──► animation ──► world, assets
```

`engine-renderer` is the important seam: it turns *world state* into a plain-data `RenderScene`
(`extract`), and everything after that (culling, sorting, batching, GPU submission) works on that
snapshot. The world never holds GPU resources and the GPU backend never touches the ECS, so a
second backend, a headless test renderer or a replay tool only needs a `RenderScene`.

## Pipeline per frame

```
input -> systems (gameplay, animation) -> propagate_transforms
      -> extract (World + Assets -> RenderScene: cull, sort, batch)
      -> backend.render(RenderScene)
```

* **Projection.** A `Camera` component holds a `CameraProjection` (Orthographic or Perspective
  today; the `Projection` trait is the extension point for isometric / oblique / custom). The
  camera's *Transform* is the view; the projection supplies the view->clip matrix. The same
  `view_proj` drives CPU-side `project()` (picking, tests, culling) and the GPU, so they cannot
  disagree. Parallax is therefore not a feature: with a perspective camera, objects at Z = 0, 100,
  300, 800 move at different apparent rates when the camera translates, with no per-sprite
  multiplier.
* **Ordering.** The depth buffer resolves opaque geometry (meshes, tilemaps, cutout sprites).
  Blended sprites are sorted back-to-front by view-space depth. `(layer, order)` is an *explicit*
  override applied in addition to depth, default 0, for cases that need deterministic ordering.
* **Batching.** Sprites become instances (model matrix, UV rect, tint, flags) and consecutive
  sprites with the same texture become one instanced draw. A sprite never issues its own GPU
  command.
* **Sprite blend modes.** `Cutout` sprites (and tilemap tiles) are alpha-tested, write depth and need no
  sorting, so they intersect correctly with 3D geometry and with each other. `Alpha` sprites are sorted
  back-to-front and drawn last with depth testing. Parts of a skeletal character are `Cutout` sprites separated by
  small real Z offsets (far arm slightly behind the torso, eyes slightly in front of the face); the depth buffer
  resolves the overlap, so a character needs no per-part ordering metadata.
* **Fog.** A `Fog` resource blends shaded color toward a fog color by true camera distance, which is a depth cue
  and hides the world's far edge.
* **Lighting.** Lights are spatial (`Light` + `Transform`). Sprites and meshes are shaded by the
  same per-pixel shader in world space (sprites have a world-space normal from their orientation),
  so a point light's falloff includes depth. Directional, point and spot lights plus ambient are
  implemented; shadows and normal maps are interface-only for now.

## Decisions and trade-offs

| Decision | Choice | Why / trade-off |
|---|---|---|
| ECS | `hecs` (wrapped by `engine_world::World`) | Mature, tiny archetype ECS with no scheduler or app framework imposed. `bevy_ecs` is more powerful (parallel scheduling, change detection) but drags in a lot of surface and would shape the engine around its conventions. We keep a thin `World` + sequential `Schedule` and can swap the storage later because gameplay code goes through our `World`. |
| Graphics | `wgpu` | One API for Metal/DX12/Vulkan. No platform-specific code. Raw Vulkan buys control we do not need yet. |
| Windowing | `winit` | De-facto standard, wgpu surfaces accept its windows directly. |
| Math | Own crate | Small surface, deterministic, `f32`, and lets Transform/Quat be shaped for the engine (no large abstraction dependency). |
| Handedness | Left-handed, +Z deeper | Makes "Z = 800" mean "far away", matching how 2.5D designers think about depth. |
| Hierarchy | `Parent` + `Children` components, `GlobalTransform` computed by a system | Bones and sprite attachments are plain entities in the same hierarchy as everything else. |
| Assets | Typed `AssetHandle<T>` holding a stable name hash | Components can be built before anything is loaded (`Sprite::new("player")`); the renderer resolves lazily and falls back to a placeholder. Sources (filesystem now, bundle later) sit behind `AssetSource`. |
| Sprite geometry | Unit quad + per-instance model matrix | Size, pivot and orientation are baked into one matrix, so sprites, tiles and skeleton parts all go through the same instanced path. |

## Verification

* `engine-math`, `engine-world`, `engine-assets`, `engine-animation`: unit tests.
* `engine-renderer/tests/spatial.rs`: parallax emerges from perspective and vanishes under orthographic; sprites
  sort by real depth; `(layer, order)` overrides; batching never reorders; culling; hierarchy reaches the scene.
* `engine-renderer-wgpu/tests/headless.rs`: renders offscreen and checks pixels (mesh/sprite depth interleaving,
  interior of meshes culled, point-light falloff depends on depth distance, 5000 sprites = one draw call).
* The example runs windowed (winit + wgpu surface) and headless (`--screenshot`).

## Not built yet (deliberately)

Shadows, normal maps, IK/blending/mixer, asset bundler and async loading/hot reload (interfaces
exist: `AssetSource`, revisions, `LoadState`), billboarding, particles, collision, UI, hierarchical
visibility, mipmaps, frustum-culling acceleration structures, GPU-driven culling.
