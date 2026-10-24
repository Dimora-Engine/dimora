# Spatial rendering

> **Everything exists in spatial coordinates first. Rendering is the projection of that spatial
> world into an image.**

This document describes how a thing in the Dimora world becomes pixels, and which concept is
responsible for what. [ARCHITECTURE.md](ARCHITECTURE.md) covers the crate layout; this file is
about the rendering model. The runnable proof is `examples/spatial_scene`.

## Why 2.5D is a spatial problem

A common way to build 2.5D is a stack of 2D layers: each sprite gets an integer layer, layers are
sorted, and "parallax" is a per-layer multiplier on camera movement. It works for a fixed camera
and falls apart as soon as the camera moves in depth, a 3D object enters the scene, or a sprite has
to be in front of one thing and behind another. Every one of those needs a special case, because
the model never knew where anything *was*.

Dimora does the opposite. Every renderable is an entity with a real 3D `Transform`. A camera has a
`Transform` too. Sprites, 3D meshes, tilemaps and skeleton parts all go through the same chain:

```text
World                entities, components (no GPU types)
  │  Transform / GlobalTransform   (+ Parent/Children hierarchy)
  ▼
Spatial Transform    where each object is: position, rotation, scale in world space
  │  CameraView::new(Camera, GlobalTransform, target size)
  ▼
Camera               view = inverse of the camera's world transform
  │  Projection::matrix(aspect)
  ▼
Projection           orthographic or perspective: view space -> clip space
  │  extract_scene(world, assets, target) -> RenderScene
  ▼
Render extraction    visibility, frustum culling, sprite quad generation, ordering, batching
  │  SceneRenderer::render(&RenderScene)
  ▼
Renderer             GPU resources, pipelines, instance buffers, draw calls
  │  vertex shader: clip = view_proj * world_position, then perspective divide
  ▼
GPU                  NDC -> viewport rectangle -> depth test -> pixels
```

Nothing after the first arrow is told "this sprite is on layer 3". It is told *where the sprite
is*. Parallax, perspective scaling, occlusion between a sprite and a mesh, and (later) lighting and
shadows fall out of the same positions. The tests in `crates/engine-renderer/tests/camera.rs` check
those claims numerically: with a perspective camera the screen shift of an object when the camera
moves is inversely proportional to its distance, and with an orthographic camera it is identical
for every object.

## Conventions

These are fixed and used everywhere, including shaders. `+Y` up with an `XZ` ground plane is the current engine
convention (provisional, [ADR-0004](adr/0004-one-spatial-world-many-camera-configurations.md)); it is not
configurable per game, and a movement plane is a gameplay choice, not the ground plane.

| Item | Convention |
|---|---|
| Handedness | **Left-handed** |
| Axes | `+X` right, `+Y` up, `+Z` forward / deeper into the scene |
| World units | Unitless; the engine assumes no scale (`basic_2_5d` uses metres, `spatial_scene` uses roughly centimetres). Sprites carry `pixels_per_unit` to convert from texture size |
| Camera forward | The camera entity's local `+Z`. Identity rotation looks down `+Z`; positive yaw (rotation about `+Y`) turns it towards `+X`; positive pitch (about `+X`) tilts it down |
| Rotations | Right-hand rule about the given axis (`Quat::from_rotation_x/y/z`) |
| View space | Camera at the origin, `+X` right, `+Y` up, `+Z` into the screen. View depth is the `Z` of a point in this space |
| Clip / NDC | wgpu/D3D style: x and y in `[-1, 1]` (`+Y` up), depth in `[0, 1]` (`1` near plane, `0` far plane). D-07 settled in ADR-0019. Compare `Greater`, clear 0. A stored custom matrix is not converted |
| Screen pixels | Origin at the **top-left** of the camera's viewport, `+X` right, `+Y` down |
| Matrices | Column-major, column vectors: `clip = proj * view * model * v` |
| Mesh winding | Triangles are wound so `(c - a) x (b - a)` is the outward normal; the pipeline uses `FrontFace::Ccw` + back-face culling |
| Sprite anchor | Normalized within the sprite rectangle: `(0, 0)` bottom-left, `(1, 1)` top-right. The anchor sits exactly at the entity's position. Default `(0.5, 0)` = bottom-centre (feet) |
| Sprite facing | A sprite quad faces local `-Z` (towards a default camera); the quad's own `+X`/`+Y` are its right/up |

## World state vs render state

The boundary is the `RenderScene`:

```text
World state (engine-world, engine-renderer components)   Render state (engine-renderer-wgpu)
────────────────────────────────────────────────────     ──────────────────────────────────
Transform, GlobalTransform, Parent, Children             wgpu Device / Queue / Surface
Camera { projection, viewport, ... }                     Pipelines, bind group layouts
Sprite, Mesh, Material, TileMap, RenderOrder             GPU textures + bind groups (cache)
AssetHandle<T> (a name hash)                             GPU vertex/index buffers (cache)
                                                         Instance buffers, depth texture
          │                                                         ▲
          └──── extract_scene ──► RenderScene (plain data) ─────────┘
```

* ECS components hold **only** plain data and `AssetHandle`s. There is no `wgpu` type in
  `engine-world`, `engine-assets`, `engine-renderer` or in any component. A gameplay system never
  needs the `wgpu` crate.
* `extract_scene` reads the world and produces a `RenderScene`: the camera view, instance lists and
  batches. It contains no ECS or asset-manager references.
* `engine-renderer-wgpu` consumes a `RenderScene` and owns every GPU object. Its texture and mesh
  caches are keyed by `AssetId` and remember the asset revision, so a reloaded asset is re-uploaded.
  `Mipmaps::Generate` is filled there, by a private blit, when the image is created or its pixels change.
  An entry is evicted when its asset is removed from the `AssetManager` or when no frame has used it for a
  configurable number of frames (120 by default); `SceneRenderer::cache_stats()` reports the counts.
* The world never issues a `RenderPass` call. The renderer decides how to draw what exists.

Because the seam is a plain struct, a second backend, a headless recorder or a replay tool needs
only a `RenderScene`.

### GPU resource boundaries

| Concept | World-side (data) | Renderer-side (GPU) |
|---|---|---|
| Texture | `Texture` asset (pixels), `AssetHandle<Texture>` | `wgpu::Texture` + bind group, cached by id + revision |
| Mesh | `MeshData` asset (vertices, indices), `Mesh` component | Vertex/index buffers, cached by id + revision |
| Material | Inline `Material`, or an optional `MaterialAsset` handle on `Sprite` and `Mesh` | Resolved into per-instance color, cutoff, and a pipeline chosen by class and cull mode |
| Sprite | `Sprite` (+ `SpriteAsset`: texture, UV rect, pivot) | One unit quad shared by all sprites; per-instance data |
| Pipeline | none | Legacy sprite cutout and blend, plus opaque, mask, and culled blend variants; mesh solid and mask, culled or double-sided. Created once, not per material |
| Buffer | none | Growable instance buffers, uploaded per frame |

An unassigned mesh still uses the inline `Material`. An assigned handle resolves to `MaterialAsset` during extraction. Unassigned sprites keep tint, `SpriteBlend`, and their own lit flag.

## Spatial renderables

Renderables are plain components next to `Transform`, not a trait hierarchy:

```text
Sprite     texture region placed in the world      (batched, instanced)
Mesh       indexed triangles + Material            (batched by mesh + texture)
TileMap    a grid of cutout quads                  (same instanced path as sprites)
(Skeleton parts are Sprites in a bone hierarchy; particles are future)
```

`TileMap` is an entity with a real `Transform`. Cells live in its local XY plane. Rotating the entity
lays the map on the ground, or on a wall, with the same camera as every other renderable. Extraction
splits the grid into 32×32 chunks. Each chunk's bounds are the sphere of its four corners after the
tilemap's global transform, so a rotated or scaled map is not treated as an axis-aligned world box.
A camera rejects a chunk before reading its cells. Accepted chunks still emit cutout sprites in
row-major order on the shared sprite path, one batch per texture, with real depth. Tiles do not read
`CoplanarPriority`. A direct write to `tiles` is visible to the next extract of any camera that can
see the cell. `set` dirties one chunk's occupancy count. `invalidate` dirties them all. That count is
internal. It is not authored scene data, and a future serializer must not be required to store it.
Editor painting should call `set`, or `invalidate` after replacing the tile vector, so a rejected
chunk's cull count stays honest. Collision and shadow casters are not built. When they are, they
should reuse `TileMap::CHUNK_EDGE` and this grid rather than a second extractor. Dynamic lights
already shade a lit tile in world space. A future shadow pass would see the same depth as the color
pass. Nothing here adds a tilemap camera or a second render pass.

An enum or trait would force dynamic dispatch or one giant match on the hot path. Components keep
each kind in its own archetype and its own tight extraction loop, while *all* of them share the same
spatial inputs: `GlobalTransform`, `Visibility`, the active `CameraView`, and the frustum.

## Camera

A camera entity keeps its pose on `Transform` and its image policy on `Camera`:

```text
Transform     where it is and where it looks (the view is the inverse of its world matrix)
Camera.projection  Orthographic | Perspective | Custom(Mat4)
Camera.viewport    ViewportRect: the part of the render target it draws into
Camera.clear       Color (default) or Load
Camera.output      Surface (default) or Target(RenderTargetId)
Camera.order       draw order among active cameras
Camera.pixel_snap  orthographic view-translation snap; default false; ignored by perspective and custom
```

Every active camera with a drawable destination is a pass. Passes sort by ascending `(order, entity id)`
across surface and offscreen outputs. Order is not rearranged to satisfy a sample. A later pass can
overlay an earlier one where they share a destination and their viewports overlap. `Visibility` does not
drop a camera. Each pass has its own view, projection, frustum, sprite orientation, and transparent
order. The aspect and the viewport rectangle use that destination's pixels. A `Custom` matrix ignores
the destination aspect. Texture, sampler, and mesh geometry caches are shared.

`RenderTargetStore` lives on the world and stores plain descriptions. Ids start at 1 and are not reused.
The renderer allocates the color image and a private depth image, and it replaces both when `resize`
changes the size. New pixels are undefined until a camera writes the target. A sprite with
`render_target` set samples that full image in `[0, 1]` instead of its `SpriteAsset` texture and
instead of `MaterialAsset.albedo`. The sample is available only after an earlier camera in the same
frame has written the id, and not while this camera is writing it. Anything else binds the magenta
placeholder. The previous frame's image is not used. Several cameras may write one id.

`CameraView` resolves them for a given render target and carries everything downstream needs:
`view`, `proj`, `view_proj`, position, forward/right/up, and the viewport rectangle in pixels. The
aspect ratio comes from the viewport rectangle, not the window, so a camera in half the window is
not stretched. The same `view_proj` drives CPU projection (`CameraView::project`, picking
rays, culling) and that camera's GPU uniform, so the two cannot disagree. `project` and
`ray_from_pixels` are viewport-local. A window pixel subtracts `viewport_origin` first.
`top_surface_camera` picks the last drawn surface camera whose rectangle contains the window pixel.
An offscreen camera is not selected that way. `camera_view_of` still resolves it in target-local pixels.

The first camera that writes a destination clears that whole image, whether the policy is `Color` or
`Load`. A later `Color` pass on that destination fills only its rectangle and does not write depth. A
later `Load` pass does not. Every pass clears that destination's depth to 0.0 and discards it. The
renderer submits each camera before writing the next camera's globals or instance data. One
full-surface camera does not add a fill draw. An offscreen target does not receive the surface clear.

`CameraProjection::Custom` stores that view-to-clip matrix. `matrix(aspect)` returns it unchanged, so a
viewport resize changes pixel mapping and does not rebuild the matrix. Perspective and orthographic still
recompute from the viewport aspect. `with_clip_range` edits only those two descriptors. The matrix must use
the engine convention: left-handed, +Y up, +Z forward, column vectors, NDC xy in `[-1, 1]` with +Y up, NDC
depth `[0, 1]` (1 near, 0 far), viewport origin top-left, depth compare greater-than, clear 0. OpenGL depth
and a 0-near matrix are not converted. Rebuild a 0-near matrix with `perspective_lh` or `orthographic_lh`.
A finite singular matrix is accepted; rendering does not need an inverse, and
`CameraView::ray_from_pixels` returns `None` when the inverse fails. Transparent objects still sort by
camera-space depth from the view matrix, then `RenderOrder`, then extraction order. An oblique matrix can
shear the picture while `Billboard` and `Upright` keep using the camera basis (ADR-0021). A coplanar
tie stays a tie unless the sprite opts into `CoplanarPriority` (ADR-0020). Frustum planes come from the same `view_proj` formulas. A plane whose normal is
negligible beside that plane's own coefficients is dropped and does not cull. Scaling the matrix by a
negative constant can leave divided NDC and those CPU planes unchanged, because both the clip numerator
and `w` change sign and the frustum corrects the inward direction. The GPU clips in homogeneous space
before that divide, so the same scale is not a guarantee of the same rendered image.

### Orthographic

Parallel projection. X and Y map linearly to the screen, Z only affects depth (what is in front)
and clipping. Two objects at different Z with the same X/Y land on the same pixel, and objects keep
their apparent size at any depth. Moving the camera shifts every object by the same amount: no
parallax, because there is no per-object parallax value to apply.

`Camera.pixel_snap` (default `false`, proposed [ADR-0022](adr/0022-orthographic-pixel-snap.md)) can quantize that shift. The scale is `viewport_height_px / orthographic_height`, which is both axes of the symmetric orthographic window. View-space X and Y translation are rounded to an integer number of those pixels with `f32::round` (a tie rounds away from zero) and the eye is moved in the right/up plane. View-space Z and the rotation stay. The projection is the existing reverse-Z matrix. An already-integer translation does not move. There is no extra half-pixel bias: an odd viewport height puts the optical axis on a pixel center, and an even height puts it on a pixel edge. The correction is at most half a pixel. Crossing the halfway value jumps a whole pixel even when the camera moved by less than one pixel. A translation that stays inside one rounding bin does not change the view.

The grid is that camera's viewport from `ViewportRect::to_pixels`, not the window. A split origin, a resize, and an offscreen target each use their own height. Side, top-down, and isometric placements are orthographic, so they snap along the view axes when the flag is set. Roll does too. Perspective, `CameraProjection::Custom`, and the oblique builders do not snap. The flag remains editable. Presets leave it off.

Sprites, tilemaps, meshes, and debug lines of that camera share the snapped view, so they occlude one another as before. The snap does not pixelate mesh shading and does not allocate a low-resolution image. Nearest and linear filtering stay on the texture. It also does not freeze independent sprite motion, a sprite animation, camera rotation, a fractional zoom, a sheared custom matrix, a non-integer texel-to-pixel ratio, a rotated or perspective-projected face, or mixed-resolution art. An integer-aligned nearest sprite often keeps the same covered pixels for any camera shift inside the same half-pixel coverage bin as the snapped pose. A sub-pixel pose is therefore not a promise of a different framebuffer, and it is not a promise of an unchanged one once an edge crosses a pixel center.

### Perspective

Pinhole projection. Apparent size is proportional to `1 / view_depth`, and a camera move shifts
nearer objects more than farther ones. With the `spatial_scene` home camera (50 degree vertical field of view, 215 units up, 420 units
back, pitched 0.3 rad down), a 96-unit figure at Z = 0, 100, 300 and 600 sits at view depths of
465, 560, 751 and 1038 and covers about 159, 132, 99 and 71 pixels on a 720p frame.

## Depth, order and position are different things

| Concept | What it is | Who owns it |
|---|---|---|
| **World position** | Where an object is in the world (`Transform`, `GlobalTransform`) | Gameplay / the world |
| **Camera-space (view) depth** | Distance in front of the camera along its forward axis; the `Z` of a point after the view matrix | Derived from position + camera. Never stored |
| **GPU depth** | The depth-buffer value after projection (`0..1`, non-linear for perspective) | The GPU, per fragment |
| **Screen position** | Pixels in the camera's viewport | Derived by projecting a world position |
| **Transparent-object ordering** | The order alpha-blended things are drawn in, so blending composes correctly | The extractor: back-to-front by view depth |
| **Render layer or queue** | Which pass, group or camera content belongs to. **Not implemented** beyond the opaque, cutout and blended pass split; not the same thing as `RenderOrder.layer` | The renderer |
| **Explicit render order** | `RenderOrder` orders blended sprites and blended meshes. It does not order opaque or cutout draws | The game, for transparent ties |
| **Coplanar priority** | Opt-in `CoplanarPriority` on a sprite. Depth-only pull of `(1 + level) * thickness` toward the camera. Not a z-index and not a blend sort (ADR-0020) | The game, on the sprites that share a surface |
| **Camera projection** | How view space maps to the image | `Camera.projection` |

Rules that follow:

1. **Opaque geometry is resolved by the GPU depth buffer.** Meshes, tilemap tiles and `Cutout`
   sprites (alpha-tested, write depth) can be submitted in any order and intersect correctly, with
   each other too. A cube can cut through a tree sprite; a sprite can be in front of, behind or
   beside a mesh purely from where they are. There is no sprite-versus-mesh special case.
2. **Alpha-blended sprites cannot use the depth buffer alone**, because blending is order
   dependent. They are drawn last, depth-tested but without depth writes, sorted back-to-front by
   *view-space* depth of the quad centre. This is a secondary mechanism for transparency, not the
   visibility architecture; it is not a global `sort_by_z()`: it applies only to the blended
   subset and uses camera-space depth, so it is correct for any camera orientation.
3. **`RenderOrder { layer, order }` is optional and never positional.** It is an additional
   component for blended sprites. `layer` dominates depth, `order` breaks ties at equal depth.
   Entities without it are `(0, 0)`, which leaves depth in charge. It does not move anything, does
   not affect opaque geometry, and does not affect projection.

These four ideas (spatial depth, transparency sorting, render layers or queues, explicit order) are kept
distinct on purpose ([ADR-0004](adr/0004-one-spatial-world-many-camera-configurations.md), decision 4). Today
`RenderOrder` reaches blended sprites and blended meshes. It does not reach opaque or mask draws. The
sprite depth point is the quad centre. A blended mesh uses the camera-space Z of its world-space
bounding-sphere centre.

**Debug lines.** `DebugDraw` segments are world positions. They use the camera's existing
`view_proj`, including perspective, orthographic, a rotated or isometric pose, a custom matrix, a
split viewport, and an offscreen target. There is no debug camera and no screen-space path. The
backend draws them as a `LineList` after the transparent sequence. wgpu 30 has no portable line
width, so each segment is one pixel. The depth test is reverse-Z greater-than, and the draw does
not write depth. Opaque, mask, and cutout geometry, which did write depth, hide a line that is
farther from the camera. A blended sprite or mesh does not write depth, so a line can paint over
those pixels even when the blend is nearer. That is the current transparency limit. Lines are not
lit, fogged, or shadowed, and they do not change sprite or mesh order. An empty list adds no draw.
Segments are not culled on the CPU; the GPU clips them. A line exactly on a surface can z-fight,
because there is no depth bias.

**Depth presentation.** `CameraPresentation::Depth` replaces that camera's viewport with the depth
buffer that camera just wrote. `Shaded` is the default and runs no extra pass. Extraction copies the
field. It does not build a second scene, and it does not change materials, projection, or gameplay
orientation. Switching back is the same field.

The mapping is reverse-Z view depth (ADR-0019). Stored 1 is the near plane and stored 0 is the clear.
A fragment at exactly 0 does not pass `Greater`, so a stored 0 is unwritten. Unwritten pixels are
purple, linear RGBA `(0.35, 0.02, 0.45)`. Written pixels are gray. Nearer is brighter. The gray is
`(z_far - z) / (z_far - z_near)` after unprojecting the stored value through the inverse of that
camera's projection at the pixel's NDC. NDC +Y is up. Perspective is linearized. Orthographic is the
same formula, and there the raw NDC already matches the linear gray. A singular matrix, a non-finite
value, or a span that is not a positive view-space range from NDC 1 to NDC 0 fills the viewport with
rust, `(0.62, 0.22, 0.08)`, including the clear pixels. A 0-near custom matrix is that case. A mild
oblique reverse-Z matrix that stays invertible is linearized. An arbitrary custom matrix is not
claimed to linearize.

The image is the depth the production passes store. Opaque meshes, mask and cutout sprites, and
tiles write where the fragment survives. A discarded mask or cutout texel keeps the depth behind it.
Blended sprites and meshes, and debug lines, do not write depth, so they do not change the image.
`CoplanarPriority` is the stored depth-only pull. The fallback thickness `1/256` is below one 8-bit
level across a range of about 20. The view does not draw a special coplanar color. Meshes and tiles
do not read the component. Fog and lighting stay in the color pass and are replaced with this image.

Each camera has its own depth clear. The visualization pass runs in that camera's encoder, before
submit, so a later camera's clear does not replace an earlier camera's picture. Two separate
viewports each show their own camera. A shaded viewport beside a depth viewport stays color. An
offscreen camera can present depth into its target. A later surface sprite still samples that
target's color. Overlapping viewports follow the existing rule that the later camera paints its
rectangle. The default perspective clip range `0.1..1000` compresses a nearby scene toward white.
Other GPU backends are not verified. Apple M5, Metal is the verified backend.

**Assigned materials (ADR-0015).** Opaque, Mask, and Blend are material classes. An assigned
sprite uses the class instead of `SpriteBlend`. Opaque and Mask write depth and do not blend. Mask
discards below the material cutoff. Blend sprites and blend meshes share one back-to-front list.
The GPU submits that list in order. Depth test stays on and depth write stays off. `RenderOrder`
applies inside that list only. A missing component is `(0, 0)`. Sprites are visited before meshes,
so a tie draws the sprite first. The mesh centre is an object-level approximation: intersecting
transparent meshes, cyclic overlaps, an overlap whose centre misses the overlapping region, a mesh
that spans another object's depth, and a transparent mesh passing through a sprite plane are outside
what that point decides. Coplanar priority is a separate opt-in (ADR-0020), described below. The three conflicts
that follow are content that has not opted in. `spatial_scene` does not attach `CoplanarPriority`, so those
tests still describe that scene. Configurable sort or occlusion policies for isometric styles are left room
for and not built.

### Coplanar priority (ADR-0020)

`CoplanarPriority(level)` on a sprite opts that sprite into a depth-only pull of `(1 + level) * thickness`
world units toward the camera. `level` is `0..=7`. Level 0 is a real step. Leaving the component off is
ordinary spatial depth. `CoplanarSettings { thickness }` is one world resource. When it is absent the
thickness is `1/256`. `CoplanarPriority::new` and `CoplanarSettings::new` refuse an out-of-range level and a
non-positive or non-finite thickness. Extraction does not clamp a value written into the public field: a bad
level is ignored, and a bad explicit thickness disables the pull.

The sprite shader keeps `clip.xy` and `clip.w`. UVs, lighting, and the quad stay on the real vertex. Meshes
are not moved. A tile map does not read the component. Two blended sprites still sort by camera-space Z and
`RenderOrder`. A nearer mesh stays visible when its separation along the view ray exceeds the pull. At the
fallback thickness, level 7 pulls 0.03125 world units. `1/1024` and `1/64` passed the same inside and outside
checks; `1/64` was not selected because that pull is 0.125. A crossing mesh remains visible outside the band.
A pull large enough to cover the crossing will hide the mesh, which is why thickness is not increased until a
picture looks right.

Priority is not a batch key. Sprites that share a material can use different levels and still draw together.
The registry field is `level: u8`. `CoplanarSettings` is a resource, like `Fog`, and is not a component. A
later editor should show both, refuse values `new` refuses, and describe the pull in world units. It should
not present the level as a layer that replaces position. No editor is built here. Mesh decals, per-tile
priority, and a skeletal-attachment API are out of scope. A sprite on a bone is still a sprite: the component
on that entity already applies.

Verified on Apple M5, Metal, with reverse-Z. Other backends are not verified.

### Coplanar conflicts the test scene verified (2026-10-14)

`spatial_scene` lays flat `Fixed` decals on the `XZ` ground and samples their overlap under every
configuration that is not edge-on to them (`side` is excluded: a flat decal has no area). These three are
verified limitations of content that has not opted in. The scene was not given `CoplanarPriority` when
ADR-0020 landed, so the tests still pin this behaviour. They should be updated in the same change that opts
those decals in. None of them is a chosen design.

| Conflict | What happens | Test |
|---|---|---|
| Two cutout decals at the same height, overlapping in `XZ` | The one spawned first keeps the overlap in every configuration that can see the ground. Swapping the tints makes the first entity's new colour win, so nothing spatial decided it | `equal_height_cutout_decals_are_decided_by_draw_order_not_by_space` |
| A cutout decal in the same plane as the slab tops (`Y = 0`) | Shown under some cameras and hidden under others. The test requires at least one of the five downward configurations to show it (≥ 90% of samples) and at least one to hide it (≤ 10%). On Apple M5 (Metal) that day, orthographic showed it and perspective, isometric, top-down and steep hid it. That named split is one GPU's rounding, not a portable rule | `known_conflict_a_decal_coplanar_with_a_mesh_is_hidden_or_shown_by_rounding` |
| A blended decal at the same height as a cutout one | Visible under some cameras and hidden under others. The test requires the blended colour over the cutout in ≥ 80% of samples under at least one configuration and in < 20% under at least one other. On the same machine, isometric showed it (about 99%) and the other four hid it. Same caveat: the names are that run, the split is the limit | `known_conflict_a_blended_decal_at_the_same_height_as_a_cutout_one_depends_on_the_camera` |

Cutout decals one unit apart in height, and a blended decal one unit above a cutout one, resolve by depth
in all five downward configurations (`decals_a_little_apart_in_height_resolve_by_depth_in_every_configuration`).
`RenderOrder.layer` decides overlapping blended sprites whichever way the camera looks.
`RenderOrder.order` decides only when the view depths actually tie: it does under perspective, orthographic
and side, and it does not under isometric, where the yaw separates the view depths
(`render_order_order_only_breaks_ties_at_equal_view_depth`).

Sorting uses the sprite quad's centre, so intersecting blended quads or large blended quads that
span a wide depth range can mis-order. The usual fix is to make those sprites `Cutout`.

## Sprites

```rust
Sprite {
    asset,         // SpriteAsset: texture + UV rect + natural size + pivot
    region,        // optional sub-rectangle of the asset (sprite-sheet frames)
    size,          // world size; None = asset's natural size
    anchor,        // pivot inside the quad; None = asset's pivot
    flip_x, flip_y,
    tint,          // Color with alpha
    orientation,   // Billboard | Upright | Fixed
    blend,         // Cutout | Alpha
    lit,
}
```

A sprite stores no screen or world position. The renderer takes `Transform + Sprite + Camera` and
produces world-space quad geometry. When any entity has a stored previous local, extraction
substitutes the composed render matrix from ADR-0007 for `GlobalTransform` before placement, culling,
and depth sorting. Those components are not written. With no stored previous, extraction still uses
`GlobalTransform`.

```text
GlobalTransform ──┐
Sprite.orientation ┼─► sprite_placement ─► model matrix = placement * anchor * size ─► SpriteInstance
CameraView ───────┘                                                                      │
                                                          shader: clip = view_proj * model * quad corner
```

### Orientation

| Mode | Quad axes | Use |
|---|---|---|
| `Billboard` (default) | The camera's right/up; faces the view plane | Particles-like sprites, effects, anything that should always read flat to the screen |
| `Upright` | Horizontal right, world `+Y` up; faces the camera's heading | Characters and props standing on ground, under a tilted camera |
| `Fixed` | The entity's own matrix | Decals, signs, anything lying in the world; the quad is edge-on when seen edge-on |

Billboards align to the camera's **view plane**, not to the camera's position. Every billboard
therefore has the same orientation in a frame, so a perspective camera does not rotate or skew
individual sprites (important for pixel art); they only change position and apparent size.

For `Billboard` and `Upright`, the entity's position and scale are used. Its twist about its own
Z axis (roll) is kept, so a bone can still swing a billboarded arm; its pitch and yaw are ignored
because the camera decides them. A negative scale mirrors the sprite.

The quad is generated on the CPU in extraction (one matrix per sprite). That keeps the shader
identical for every orientation mode and makes new modes (axis-constrained, custom) a change to one
function, `sprite_placement`. Orientation is a rendering decision, so it lives in `Sprite`, not in
`Transform`: the entity's `Transform` always says *where* the sprite is.

### What `Upright` does in the test scene

This extraction rule is the design ([ADR-0021](adr/0021-sprite-orientation.md)). A straight-down edge and steep foreshortening are expected.
`Upright` builds a vertical quad and turns it about world `+Y` to face the camera's heading. A pitched
camera therefore foreshortens it.

| View | Measured against a billboard of the same sprite | Test |
|---|---|---|
| Orthographic, pitch 0.3 rad; isometric; side (pitch 0) | On-screen height is `cos(pitch)` times the billboard. Exact, because the projection is orthographic | `an_upright_sprite_is_foreshortened_by_the_cosine_of_the_pitch_in_orthographic_views` |
| Top-down (looking along −Y) | Edge-on: under half a pixel, against more than 5 px for the billboard. Looking straight along `+Y` has no heading, so extraction falls back to the camera's right axis and the quad stays vertical | `an_upright_sprite_collapses_to_a_line_when_the_camera_looks_straight_down` |
| Steep (perspective, pitch 1.05 rad, about 60°) | Projected height between 0.4 and 0.65 of the billboard. Not exactly `cos(pitch)`, because the projection is perspective | same test, the steep assertion |

`orientation_modes_under_each_configuration` counts pixels: a billboard has area under every configuration,
including top-down; `Upright` and `Fixed` have none when the camera looks straight down. A billboard standing
on a mesh pad keeps its pixels under the shallower views and loses about half of them top-down, where the
quad lies in the ground plane and the pad cuts it
(`a_billboard_standing_on_a_mesh_is_cut_by_it_when_the_view_is_steep_enough`).

The shader's world-space normal for lighting is derived from the final quad, so billboards and
fixed quads are shaded consistently.

## Meshes

`Mesh { asset, material, material_asset }` plus the entity's `Transform` is all a mesh needs. With
`material_asset` unset, `Material` is a colour, an optional texture and a `lit` flag, and the mesh is
opaque and back-face culled. With a handle, `MaterialAsset` supplies the colour, albedo, alpha class,
lit flag, and `double_sided`. Albedo `None` is untextured. Opaque and Mask write depth. Blend joins
the same transparent sequence as blended sprites, tests depth, and does not write it. The fragment
keeps the sampled alpha. Meshes use the same `GlobalTransform`, the same `CameraView` and the same
depth buffer as sprites. Depth-writing instances that share a mesh, a texture, a draw class,
and a material become one draw. Blended meshes merge only while they stay contiguous in the sorted sequence.
`MeshData::plane`, `cube`, `cuboid`, `sphere`, and `cylinder` (ADR-0018, M5-6 `92286ce`) are the same kind of asset. M5 is accepted locally.
`cube(size)` matches `cuboid` with that size on every axis. A plane is the horizontal XZ ground,
normal +Y. Boxes are centred, with 24 flat-shaded vertices and 36 indices. A sphere is a UV sphere
with radial normals. A cylinder is centred on Y; `cylinder` includes caps, and
`cylinder_with_caps(..., false)` leaves them off. None of these selects a pipeline of its own. A
large blended box still sorts by the centre of its bounding sphere.

## Batching

* Sprites become `SpriteInstance` records (model matrix, UV rect, tint, flags). Consecutive
  instances that share a batch key become one instanced draw. A sprite never issues its own GPU
  commands, and batches never reorder instances, so correctness never depends on batching.
* Cutout sprites and tiles are grouped by texture to minimise batches; blended sprites keep their
  depth order.
* Meshes are sorted by `(mesh, texture)` and merged into instanced draws; per-instance colour keeps
  that from splitting batches.
* In `spatial_scene`, hundreds of floor-tile meshes are a handful of draw calls. In
  `basic_2_5d`, 3000 grass tufts plus tiles are a single sprite draw call.
* Culling (a bounding sphere against the camera frustum) runs before instances are created. That test does not skip `prepare_sprites`: every sprite that ancestor visibility leaves visible is prepared once per extract, including sprites the camera then rejects. The release numbers are in [PERFORMANCE.md](PERFORMANCE.md).
* Sprites that share one atlas texture batch as one texture. `AtlasBuilder` fills those UV rects
  (M5-4A, `bf4d9bb`); GPU samples of that path are M5-4B (`1666695`). A material albedo uses those
  UVs as stored and does not repack the atlas. A sprite run merges when the pipeline, material id,
  material revision, and sampled image match. Without `Sprite.render_target`, that image is the
  material albedo when it is set, otherwise the sprite texture. With `render_target` set, it is that
  offscreen image, or the missing-texture placeholder when the sample is not available this frame.
  Two different targets do not merge. The placeholder samples do. Tint, UV, and `lit` stay on the instance. The cutoff copied there is the one value on the shared material.
  Unassigned sprites have no material id. Opaque and mask sprites
  are grouped by that key. Blended sprites keep sort order and merge only with an already
  neighbouring match. Depth-writing meshes merge on mesh, texture, draw class, and material.
  Blended meshes do the same only inside a contiguous run. One thousand compatible opaque sprites
  are one instanced draw. GPU-driven instancing and culling beyond the frustum test are still open.

## Camera configurations

Each row is one function in `examples/spatial_scene/src/cameras.rs`. The function calls `PlacedCamera` or
`CameraProjection::oblique_orthographic`, then applies this scene's framing: the focus, the height or field
of view, clip `1..4000`, and the sky clear. Isometric is an orthographic camera, not a projection variant.
`custom` stores the public oblique matrix as `CameraProjection::Custom`. Switching among the seven functions
changes only the camera and the transform. Extraction and the wgpu backend have no branch on the name.
A preset does not set gameplay orientation: which way a character faces is not read from the camera.
D-06A (`4a87bb0`) can snap an orthographic view translation. These rows leave `pixel_snap` off.
`perspective`, `steep`, and `custom` would ignore the flag if it were set. Integer-resolution scaling and MSAA are not part of this table.

| Name | Projection | Placement | Holds in the tests | Does not |
|---|---|---|---|---|
| `perspective` | Perspective, 50° vertical field of view | (0, 215, −420), yaw 0, pitch 0.3 rad | Occlusion both ways; a projected point picks back to itself | |
| `orthographic` | Orthographic, height 400 | The same transform | The same, with no parallax. `Upright` height is `cos(0.3)` of a billboard | |
| `isometric` | Orthographic, height 900 | Yaw 45°, pitch `atan(1/√2)`, looking at (0, 60, 300) | The same. It is a rotated orthographic camera | `RenderOrder.order` does not break the equal-world-depth ghost pair: the yaw separates their view depths |
| `top-down` | Orthographic, height 1000 | Pitch 90°, looking along −Y, +Z toward the top of the image | Billboards face the camera. Flat decals are visible | `Upright` and vertical `Fixed` quads are edge-on |
| `steep` | Perspective, 50° | Pitch 1.05 rad (about 60°), same focus | The same occlusion and picking | `Upright` is roughly half a billboard; that ratio is not a settled design |
| `side` | Orthographic, height 400 | Level, looking along +Z | The same occlusion and picking | Flat ground decals are edge-on. Not a 2D-style game |
| `custom` | `CameraProjection::oblique_orthographic`: height 400, near 1, far 4000, aspect 16/9, shear `(0.14, 0.04)`, stored as `Custom` | The same home transform as `perspective` and `orthographic` | Occlusion both ways; project-then-pick. At 320×180, figure C moves about 48 px from its orthographic pixel. The pillar's GPU pixel is the projected centre and is not the orthographic pixel. A cutout figure and a blended ghost stay visible. The frustum keeps those scene objects and culls a point at x = 8000. Resize keeps the matrix. View z is unchanged, so reverse-Z still holds | Aspect is baked. `with_clip_range` does not edit the matrix. `Upright` does not follow the shear (ADR-0021). This scene does not opt into `CoplanarPriority` (ADR-0020). A negative scale of a custom matrix is not a claim of the same GPU image. A stored 0-near matrix is not converted. The builder does not rewrite a matrix already passed to `custom` |

## The example: `spatial_scene`

```text
cargo run --release -p spatial_scene
cargo run --release -p spatial_scene -- --screenshot out.png [--camera NAME | --ortho]
    [--environment a|b] [--cam x,y,z] [--yaw deg] [--pitch deg] [--size WxH]
    [--orientation billboard|upright|fixed] [--frames N]
```

`NAME` is `perspective`, `orthographic`, `isometric`, `top-down`, `steep`, `side`, or `custom`. An unknown
name is an error. `--ortho` is the orthographic configuration. `--frames N` renders N frames and exits with an error if the GPU cache grew.
The default environment is A. `--environment b` publishes the yard before the first step.

Environment A is a checkerboard of mesh slabs on the `XZ` plane (`+Y` up), four cutout figures at Z = 0,
100, 300 and 600, overlapping cutout figures, cutout trees, blended ghosts (one pair with `RenderOrder`),
a pillar, a spinning cube and a pyramid, and the flat decal bench at the left. It is unlit. Environment B
is a smaller yard in the same view: four floor tiles, a lit cube, a pillar, a figure, a tree, and one
point light. Both are `SceneBuilder` functions. Replacing one does not remove assets.

The camera and the walker are not members of either environment. The walker is moved by a `Transform`
write and keeps that entity across a switch. Tab queues one deferred replacement; holding it does not
queue another. The world stays on the current environment until the stage flush, and the tracked instance
id changes only after that replacement reports success. A failure leaves the current environment in place
and is logged when the status becomes `Failed`. An unexpected load stays published, and an unexpected
unload is left as the engine applied it: the switch logs that it does not own that instance and does
not unload or rebuild it.

`spawn_lamp` in `examples/spatial_scene/src/prefab.rs` is the code-defined prefab. It is a function
that receives `&mut World` and returns the lamp-post entity. The light is a child. `place_lamp` is the
same object built through `SceneBuilder`, for a managed scene. Neither is spawned by environment A or B,
so the screenshot baselines stay the original scene. There is no prefab asset and no prefab editor.

Controls: `W/S` move in depth, `A/D` strafe, `Space/Shift` move up/down, arrows rotate, `C` selects the next
configuration (`custom` is last, then `perspective`), `P` toggles perspective/orthographic and leaves a
custom matrix unchanged, `O` cycles the figures' sprite orientation, `R` returns to the current configuration,
`Tab` switches environment, `Esc` quits. The window title names the active configuration and, for the
projection, `perspective`, `orthographic`, or `custom`. It does not infer that from `is_perspective`.

<img src="spatial_scene_m6_perspective.png" alt="The same scene in perspective">

<img src="spatial_scene_m6_orthographic.png" alt="The same scene in orthographic">

<img src="spatial_scene_m6_isometric.png" alt="The same scene through the isometric orthographic camera">

<img src="spatial_scene_m6_top_down.png" alt="The same scene looking straight down">

<img src="spatial_scene_m6_steep.png" alt="The same scene from about 60 degrees">

<img src="spatial_scene_m6_side.png" alt="The same scene from the side, looking along +Z">

Perspective, orthographic, isometric, top-down, steep and side, in that order. Top-down, the standing sprites
are edge-on except the billboards, which lie flat on the ground. Side-on, the flat decals disappear.

<img src="spatial_scene_m6_decals_perspective.png" alt="The decal bench from a low perspective camera">

The decal bench, perspective, camera moved onto it. Nearest large pair: two cutout decals at equal height;
red was spawned first and keeps the overlap. The yellow and blue pair is one unit apart in height, and blue
wins. Farther up, one orange/cyan pair is the blended decal at the cutout's height and the other is the
blended decal a unit above it. The torn red fragment at the bottom edge is the decal coplanar with the slabs.

<img src="spatial_scene_m6_decals_top_down.png" alt="The same bench looking straight down">

The same bench looking straight down. The equal-height red/green pair still gives the overlap to red. The
decal that shares the slab plane is absent. Both pictures are illustrations of the 2026-10-14 Apple M5 run;
the tests, not these pixels, are the verification.

### Earlier pictures (M1 scene)

These two are the scene before the walker, the ghost pairs and the decal bench. They are kept because they
are the recorded evidence of occlusion and parallax from M1. `--screenshot` does not reproduce them.

<img src="spatial_scene_occlusion.png" alt="M1 scene: figure B in front of the pillar, figure C behind it, a cube cutting through a tree sprite">

M1 scene. Figure B (Z = 100) in front of the pillar (Z = 200), figure C (Z = 300) behind it, the cube cutting
through the tree: resolved by the depth buffer from real positions.

<img src="spatial_scene_parallax.png" alt="M1 scene: top row perspective, bottom row orthographic; right column moves the camera 200 units right">

M1 scene. Top row perspective, bottom row orthographic. The right column moved the camera 200 units along
+X. In perspective the near figure slides off the left edge while the distant one barely moves; in
orthographic everything shifts together.

## What this milestone deliberately does not include

Shadows, new lighting features, skeletal-animation changes, physics, particles, post-processing,
editor and scripting (both planned; see VISION.md). (The lighting and skeleton code from the previous milestone is untouched; the
spatial example simply opts out of lighting with unlit sprites and materials so that it shows the
projection and nothing else.)
