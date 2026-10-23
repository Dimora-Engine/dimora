# ADR-0020: Opt-in coplanar priority for sprites

* **Status:** Settled
* **Date:** 2026-10-23
* **Backlog id:** The coplanar-ordering part of D-48. Gameplay orientation and D-26 stay open. This file is ADR-0020. [ADR-0019](0019-reverse-z.md) is reverse-Z and is not overwritten.
* **Decided by:** Cal, 2026-10-23, after the production checks below.
* **Affects:** sprite extraction, the sprite vertex shader, one camera-uniform field, the component registry. Not mesh drawing, not tile drawing, not blend sort.

## Context and problem

Two cutout sprites on the same plane, or a cutout sprite on a mesh surface, do not have a stable winner. A bit-identical depth fails both `Less` and `Greater`. Depths that differ by a few ulps pick a winner that can flip when the projection changes. [ADR-0019](0019-reverse-z.md) kept that tie and recorded the screenshot pixels it moves. `RenderOrder` orders blended sprites and blended meshes. It does not order opaque or cutout draws.

The owner decision in [ADR-0004](0004-one-spatial-world-many-camera-configurations.md) keeps four ideas distinct: spatial depth, transparency sorting, render layers or queues, and an explicit override for coplanar content. The override is not a second depth, and it is not a layer index.

The spike on `spike/m6-3b-d48-coplanar` (`a176502`) measured overlay passes, hardware depth bias, NDC and ulp shifts, slope-scaled shifts, full-position moves, and depth-only moves. That record is `docs/spikes/m6-3b-coplanar-ordering.md` on that branch. It is not merged. The production path below is the depth-only shader only. The experiment enum and the ignored harness are not in this tree.

`spatial_scene` still does not attach this component. Its three conflict tests still describe content that has not opted in.

## Requirements and constraints

* Ordinary sprites and meshes keep real spatial depth. No component means no pull.
* The pull is opt-in, for overlapping coplanar artwork and for a sprite on a surface. It does not replace depth sorting, transparent sorting, or spatial occlusion.
* Preserve `clip.xy` and `clip.w`. Change depth only. UVs, lighting, shading, and the visible quad stay on the real vertex.
* Keep reverse-Z: compare `Greater`, clear 0, no shader complement of `clip.z`.
* A vertex that was inside `0 <= clip.z <= clip.w` stays inside that interval. A sprite in front of the near plane stays clipped.
* One project thickness. Priority is not a batch key. Sprites that share a material keep independent levels.
* Priority does not order blend against blend. That stays `RenderOrder` and camera-space Z.
* Meshes are not displaced. A tile map does not read the component.
* Do not add a second renderer or a pass.
* Invalid levels and thicknesses are errors. They are not clamped and not replaced with the fallback.
* The component is plain data the registry can list. The editor is not built in this checkpoint (ADR-0003).

## Options considered

### A. Leave coplanar ties as ties
* Advantages: no shader change, no uniform growth, the three scene conflicts stay exactly as measured.
* Disadvantages: overlapping decals and a sprite on a floor stay camera-dependent. That is the bug D-48's coplanar part exists to close.

### B. Overlay pass or hardware depth bias
* Advantages: the bias is a pipeline state, so the vertex shader stays simple.
* Disadvantages: the spike's overlay variants split one material into one draw per level (interleaved scene 3 draws to 12). Bias large enough to beat oblique rounding hid genuinely nearer objects. Depth-write and no-depth-write each failed a different blend case. Rejected.

### C. NDC, ulp, or slope displacement
* Advantages: no inverse matrix.
* Disadvantages: a shift large enough for a grazing orthographic floor was larger than a real perspective gap, and a slope term hid nearer objects at every measured separation below 10. Rejected.

### D. Move the vertex in world space and reproject
* Advantages: one matrix multiply, and the new point is a real position.
* Disadvantages: the spike's full-position path dropped a quad that had been pulled across the near plane, and a lit ground ramp changed thousands of pixels. UVs and shading would follow a point the game did not place. Rejected.

### E. Depth-only pull along the view ray
* Advantages: `clip.xy` and `clip.w` stay. The spike's reverse-Z grid passed 20/20 cells at `1/1024`, `1/256`, and `1/64`. Same-material draws stayed one draw. A nearer object beyond the pull stayed visible.
* Disadvantages: the camera uniform carries `inv_view_proj` (64 bytes) and a thickness (16 bytes). A singular projection cannot build the ray. The pull is a band, not a solution for a long intersection.

## Decision and rationale

Option E, on the settled reverse-Z path.

```rust
pub struct CoplanarPriority(pub u8);
pub struct CoplanarSettings { pub thickness: f32 }
```

`CoplanarPriority::new` and `CoplanarSettings::new` return `Result`. Levels are `0..=7` (`MAX_COPLANAR_PRIORITY`). Thickness must be finite and greater than 0. The public fields still exist, so a caller can write `CoplanarPriority(9)` or a negative thickness. Extraction does not clamp those. An out-of-range level is logged and stored as "no priority". An explicit thickness that fails the check is logged and becomes `0`, which the shader treats as "do not move". The missing-resource fallback is not substituted for a bad explicit value.

No `CoplanarPriority` is ordinary spatial depth. Level 0 is a real step, not a sentinel. The pull toward the camera is `(1 + level) * thickness` world units along the view ray through that vertex. Only `clip.z` is written. `flags.z` is 1 when the sprite opted in and `flags.w` is the level. Those lanes were already reserved zeros, so the sprite instance stays 112 bytes. A tile instance and a mesh instance keep flags `[lit, cutoff, 0, 0]` or the mesh equivalent `[lit, cutoff, 0, 0]`.

`CoplanarSettings` is one world resource, like `Fog`. There is no per-material or per-camera thickness. When the resource is absent, extraction uses `CANDIDATE_COPLANAR_THICKNESS`, which is `1/256`.

The inverse view-projection is uploaded per camera. A singular matrix uploads zeros, and the shader leaves `clip` unchanged when either unprojected `w` is below `1e-6`. `clip.w <= 0` and a non-positive amount also leave `clip` unchanged. A vertex whose original `clip.z` is inside `[0, clip.w]` has the new `z` clamped to that interval.

Blend sort is still camera-space Z, then `RenderOrder`, then extraction order. Displacement can change whether an opted-in blend passes the depth test against an opaque or a cutout. It does not reorder two blends.

## Thickness

Level 7 pulls `8 * thickness`.

| Thickness | Level-7 pull (world units) | Mesh at `0.35 * pull` | Mesh at `2 * pull` |
|---|---|---|---|
| `1/1024` | 0.0078125 | sprite wins | mesh stays |
| `1/256` | 0.03125 | sprite wins | mesh stays |
| `1/64` | 0.125 | sprite wins | mesh stays |

The production check is `the_candidate_thickness_overrides_only_inside_its_documented_pull` in `engine-renderer-wgpu` `tests/coplanar.rs`. Orthographic height 6, camera at `z = -8`, sprite at the origin, slab half-depth 0.001, level 7, Apple M5, Metal, reverse-Z. All three thicknesses passed. The gap is the slab center. The front face is 0.001 closer than that center.

`1/64` was not selected. Its level-7 pull is 0.125, four times `1/256`, so it overrides more geometry that is actually nearer. `1/1024` also passed this check and the spike's 20/20 reverse-Z cells. It is a legal `CoplanarSettings` value. It is not the fallback, because `1/256` passed the same contract and is the value this checkpoint publishes when a world sets nothing. A game whose art scale needs a shorter or longer step inserts `CoplanarSettings`. The measurements did not show a need for a second setting.

The rule, from the spike and held by this test: a nearer surface stays visible when its separation along the view ray exceeds `(1 + level) * thickness`. The test does not claim a winner at exactly one pull. Slab thickness and depth quantization sit on that boundary.

The spike's standard-Z perspective cells still failed at distance 40 and beyond for the smaller thicknesses. Production does not use that mapping. [ADR-0019](0019-reverse-z.md) is the depth convention this pull assumes.

## Consequences and follow-up

**How to use it.** Spawn `CoplanarPriority` on the sprite entity. Level 3 is in front of level 0 when they share a surface, by four thicknesses (`(1+3)` versus `(1+0)`). Insert `CoplanarSettings` to replace `1/256`. Constructors refuse an out-of-range level and a non-positive thickness.

**What the level means.** It selects a multiple of the project thickness. It is not a z-index, not a sort key, and not a batch key. Level 0 already moves the sprite. Leaving the component off is the only way to keep the real depth.

**What thickness controls.** The world-unit length of one step, along the view ray, for every opted-in sprite in that world. It does not change `clip.xy`, UVs, or lights.

**Why spatial depth stays authoritative.** Anything that is not opted in is unchanged, including every mesh and every tile. An opted-in sprite loses to geometry whose view-ray separation is greater than its pull. The `2 * pull` column above is that case at level 7. Camera-space sort of blended objects is a separate list and ignores the level.

**Intersections.** The pull is a band around the sprite plane. A mesh that crosses the sprite stays visible outside that band. The production intersection test (orthographic, wall rotated 0.7 rad about Y, level 7, thickness `1/256`) keeps both colours, leaves a corner pixel outside the quad unchanged, and does not give pixels back to the mesh. The spike's 2-unit pull (thickness 0.25, level 7) removed the mesh side of a −40° crossing. That pull is not the fallback. Do not raise thickness to paint over an intersection.

**Blend versus blend.** Two blended sprites at the same view Z still follow `RenderOrder`. A higher priority does not put a blend in front of another blend. A blend farther than the pull stays hidden behind a cutout. A coplanar blend that opts in can pass the depth test and composite over a cutout. That is the depth test, not a new sort.

**Cameras.** The same higher-priority winner was read back for perspective, orthographic, a side view, a steep view, isometric, top-down with a fixed quad laid in the XZ plane, and a sheared custom orthographic matrix. Distances 5, 40, and 200, and orthographic cameras at `z` of −6, −30, and −80, kept the winner. Two viewports and an offscreen target each keep their own depth. `Upright` from straight above is still edge-on (D-26). This component does not change that.

**Backends.** Apple M5, Metal, integrated GPU, 2026-10-23. Vulkan, DirectX, OpenGL, and lavapipe were not run. A disagreement on one of those reopens the shader's inverse and the compare, not the decision that the pull is opt-in and depth-only.

**Editor.** `CoplanarPriority` is registered as Persist. The field is `level: u8`. The dump prints that integer. `script_visible` and `script_writable` stay false, as they do for the other built-in components, until a scripting milestone turns them on. `CoplanarSettings` is a world resource, not a component, same as `Fog` and `AmbientLight`. A later editor should show `level` on the sprite and one thickness with the other world resources, and it should refuse values that `new` refuses. The label should say the sprite moves `(1 + level) * thickness` world units toward the camera for the depth test. It should not be presented as a layer that replaces position. No editor UI is built here.

**Out of scope.** Mesh decals and any mesh displacement. Tile maps reading the component, including a per-tile level. A skeletal-attachment API: a sprite parented to a bone is still a sprite, and this component on that entity already applies. No socket-specific priority was added. Camera-sized foreground overlays. A second thickness. Changing `RenderOrder`. Opting the `spatial_scene` decals in. Those three conflict tests stay, because that scene still has no `CoplanarPriority`.

**Uniforms and draws.** `GpuGlobals` gains `inv_view_proj` and a `coplanar` vec4: 80 bytes on the camera uniform. The sprite instance layout is unchanged. Meshes share the uniform and do not read the new fields. The four release frames below match the reverse-Z draw counts: no extra pass and no split batch. Timestamp queries are not enabled. This is not a claim that the GPU is faster.

## Evidence and revisit conditions

CPU, not ignored: `engine-renderer` `src/coplanar.rs` unit tests; `tests/coplanar.rs` (priority is not a batch key, three levels share one material batch, a missing component and level 9 extract as none, an explicit `1/64` is used, a thickness of −1 disables the pull and keeps the level, a tile map ignores the component). `engine` `tests/registry.rs` lists `CoplanarPriority` and dumps `level: u8 = 3`.

GPU, not ignored, `DIMORA_REQUIRE_GPU=1`, `engine-renderer-wgpu` `tests/coplanar.rs`, Apple M5, Metal: coincident cutouts in the configurations named above; the same winner across distance and camera movement; the three-thickness pull table; a coplanar cutout against a mesh, and a plain sprite that does not cross a mesh one thickness closer; blend order ignoring priority, a blend hidden 0.5 behind a cutout, and a coplanar blend compositing over a cutout; one draw for two priorities of one material, then a runtime change of the lower sprite to level 7; an unmodified sprite and mesh; near-plane clamp and a sprite at `z = 0.02` still clipped; two cameras and an offscreen target; an intersection that keeps both colours; 64 sprites of mixed priority in one draw.

Screenshots, release, 1280×720, 2026-10-23. The scene does not opt in. SHA-256 matches the reverse-Z references in ADR-0019, and the `6dbabb8` hashes were not replaced. `basic_2_5d` `196faee1b7eb39d7d0f706b20b50597867c92b72e73189e507b7e43b8098f7f0` (746060 bytes). `basic_2_5d --ortho` `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f` (462130). `spatial_scene --camera perspective` `cad5f528af127de4e94c16ecbcedea1e238e0ba6e1b72187ddfc19e13b24182b` (137001). `spatial_scene --camera orthographic` `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde` (34046). Draw counts match the D-07 capture: basic sprites 8848/3783, meshes 6/0, draws 10, submitted 8854; perspective scene sprites 28/1, meshes 349/282, draws 10, submitted 377. Each frame had one camera pass, one surface pass, no offscreen pass, and no clear draw.

Reopen this decision only under the rule at the bottom of [DECISIONS.md](../DECISIONS.md), and only for a cited failure. A scene where level 7 at `1/256` (0.03125 world units) hides geometry that must stay visible, and where `1/1024` keeps both the coplanar winner and that geometry, is such a failure. A backend that disagrees on the inverse or the compare is such a failure. Mesh or tile opt-in is a new record, not a silent change to this one.
