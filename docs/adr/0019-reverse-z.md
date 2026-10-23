# ADR-0019: Finite reverse-Z depth

* **Status:** Settled
* **Date:** 2026-10-23
* **Backlog id:** D-07. This record closes D-07.
* **Decided by:** Cal, 2026-10-23, after the compatibility and regression checks in this record.
* **Affects:** `Mat4::perspective_lh`, `Mat4::orthographic_lh`, `CameraView::ray_from_pixels`, frustum plane identity, the wgpu depth compare and clear, custom projection authoring.

This file is ADR-0019. The coplanar draft on `spike/m6-3b-d48-coplanar` (`a176502`) used the same number and is not on this branch. That draft must be copied as ADR-0020. It must not overwrite this file.

## Context and problem

Production depth was standard Z: NDC 0 at the near plane, 1 at the far plane, `CompareFunction::Less`, clear 1. `Depth32Float` then spends its small ulps near the camera. At a perspective near of 0.1 and far of 1000, a 1/1024 world-unit gap at view z = 200 is smaller than one ulp of the 0-near value, so the two depths are the same `f32`.

The M6-3B precision checkpoint on `spike/m6-3b-d48-coplanar` (`a176502`), Apple M5, Metal, measured a real reverse-Z matrix. Depth-only and full-position displacement both passed 20/20 at thicknesses 1/1024, 1/256, and 1/64 for perspective 45° and orthographic height 10, at distances 5, 10, 40, 100, and 200. The standard-Z misses were perspective at distance 40 and beyond. A shader complement `clip.w - clip.z` is the same algebra and is not the same `f32`; that run was discarded. This ADR uses the matrix result only.

D-48 stays a separate decision. This record does not add `CoplanarPriority`.

## Requirements and constraints

* One depth convention for sprites, meshes, every camera, and every destination. No second renderer and no style-specific depth.
* User-facing near and far stay view-space distances. World transforms do not change.
* The mapping is the projection. It is not a shader rewrite of `clip.z`.
* A stored custom matrix is not silently reinterpreted. Oblique and off-centre matrices still go through `CameraProjection::Custom`.
* Transparent sort stays camera-space Z from the view matrix (ADR-0015).
* ADR-0004 is unchanged: one camera model. Reverse-Z is that model's depth mapping, not a new projection kind.

## Options considered

### A. Keep standard Z
* Advantages: existing screenshots and any hand-built 0-near matrix stay as they are.
* Disadvantages: the measured perspective gap at distance 200 does not survive `f32`.

### B. Shader complement (`clip.w - clip.z`) with greater-than
* Advantages: the projection helpers could stay.
* Disadvantages: the checkpoint showed the complement loses the gap that a real reverse-Z matrix keeps. Rejected.

### C. Finite reverse-Z in the two math helpers, one compare, one clear
* Advantages: every caller of `perspective_lh` and `orthographic_lh`, including the example oblique camera, follows one convention. No second public constructor.
* Disadvantages: a custom matrix built for 0-near depth occludes backwards until the author rebuilds it. Exact coplanar ties can change winner.

### D. A depth-convention enum, or automatic conversion of `Custom`
* Advantages: old matrices could keep working beside new ones.
* Disadvantages: a permanent dual path, and a converter cannot tell which convention a stored matrix already used. Not added.

## Decision and rationale

Adopt option C. This is the accepted depth convention.

* Perspective, left-handed, `span = far - near`, `f = 1 / tan(fov_y / 2)`: column 2 is `(0, 0, -near/span, 1)`, column 3 is `(0, 0, near*far/span, 0)`. `clip.w = z`. NDC z is 1 at `near` and 0 at `far`.
* Orthographic: the x/y window is unchanged. Column 2 z is `-1/(far-near)`, column 3 z is `far/(far-near)`, column 3 w is 1. `near` may be 0.
* The depth attachment stays `Depth32Float`. The compare is `Greater`. Each camera pass clears depth to 0 and discards it. The viewport depth range stays `0..1`, which writes NDC as-is.
* Sprite and mesh pipelines share that compare. Opaque and mask write depth. Blend tests and does not write. There is no clip-z rewrite in `sprite.wgsl` or `mesh.wgsl`.
* `CameraView::ray_from_pixels` unprojects NDC z = 1 as the ray origin and NDC z = 0 as the far point.
* Frustum rows are unchanged as formulas. Under this mapping, row 2 (NDC z = 0) is the far plane and `r3 - r2` (NDC z = w) is the near plane. The visible interval is still `0 <= clip.z <= clip.w`.
* `CameraProjection::Custom` stores the matrix it is given. OpenGL depth and a 0-near matrix are not converted. Rebuild those with `perspective_lh` or `orthographic_lh`. The example oblique camera multiplies a shear by `orthographic_lh`, so it follows the helper without a second path.

## Consequences and follow-up

* Nearer opaque and cutout fragments win because they write a larger depth. A second camera still cannot reject the first: every pass clears depth to 0.
* Authors of stored custom matrices must rebuild them. Until they do, farther fragments win the greater-than test. That is the migration, and `a_stored_standard_z_matrix_is_not_reinterpreted` locks it.
* `ScreenPosition.depth` is 1 at the near plane and 0 at the far plane. `is_visible` still requires that value in `[0, 1]`.
* A bit-identical depth is still a tie: `Greater` and `Less` both reject equals. When two surfaces are coplanar but their interpolated depths differ by ulps, the compare picks the opposite winner. That is the screenshot difference below. It is not a moved vertex and not a change to `clip.x`, `clip.y`, or `clip.w`. The perspective x and y rows do not use z.
* Historical screenshot hashes from `6dbabb8` stay the pre-reverse-Z record. The reverse-Z hashes below are an additional reference. Image files were not stored in git either way.
* D-48 is not implemented. The form that preserved UVs, lighting, and near-plane coverage on the spike was a depth-only change of `clip.z`, leaving `clip.xy` and `clip.w` alone, by `(1 + level) * thickness`. The proposed API, still not in the engine, is `CoplanarPriority(pub u8)` for levels 0..=7 and `CoplanarSettings { thickness: f32 }`. Absence means ordinary depth. No production thickness is chosen. 1/256 is a candidate for the D-48 tests. 1/1024 passed the spike's reverse-Z cells on this machine and is not a default.
* This branch does not add a depth-convention enum, a second scene type, or a per-sprite depth flag.
* M14 shadow maps can use the same mapping if this ADR is accepted. They are not built here.

## Evidence and revisit conditions

Apple M5, Metal, integrated GPU, 2026-10-23. No Vulkan, DirectX, OpenGL, or lavapipe run. wgpu on this machine is Metal. Do not read these results as those backends.

CPU: `projection_depth_is_one_at_near_and_zero_at_far` and `reverse_z_separates_a_far_perspective_gap_that_standard_z_cannot` in `engine-math`. The historical 0-near formula lives only in that second test. At perspective z = 200, near 0.1, far 1000, gap 1/1024, reverse-Z produces two `f32` depths and the historical formula produces one. Orthographic reverse-Z also separates that gap; the precision win over standard Z is the perspective case, because orthographic depth is linear in either mapping. Camera tests expect a larger depth for nearer positions, and picking rays still pass through the projected point. The degenerate-far-plane test drops NDC z = 0 and keeps the near plane.

GPU, `engine-renderer-wgpu` `tests/reverse_z.rs`, not ignored: perspective, orthographic, and a side-looking orthographic camera; sprite/mesh occlusion both ways; mask keep and discard, cutout, blend-over, and opaque-over-blend; the z = 200 gap of 1/1024 for perspective and orthographic, one instanced draw and two instances; near and far clipping; an oblique custom matrix; a later `Load` camera drawing where the first camera's depth would have rejected it; an offscreen target with the same nearer-wins result and one world draw. Existing `custom_projection`, `multi_camera`, `render_target`, `material`, `blend`, and `configurations` tests were run with this mapping. The seven example cameras still pass, including the three coplanar conflict tests. Those tests still allow a tie to go either way. They did not need a new expected winner.

Screenshots, release, 1280×720, 2026-10-23 1:01 PM America/Chicago. The `6dbabb8` hashes stay. These are the reverse-Z references:

| Frame | `6dbabb8` | Reverse-Z |
|---|---|---|
| `basic_2_5d` | `c3c5874adeea119c98255c6c1d08208c0244224d930d86e8736c280ef88b38fe` | `196faee1b7eb39d7d0f706b20b50597867c92b72e73189e507b7e43b8098f7f0` (746060 bytes) |
| `basic_2_5d --ortho` | `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f` | same, byte-identical |
| `spatial_scene --camera perspective` | `ec5f6252a88f642b454ae768f9eec8cdd5fba88a5aeb7a424fcefe5fda12275e` | `cad5f528af127de4e94c16ecbcedea1e238e0ba6e1b72187ddfc19e13b24182b` (137001 bytes) |
| `spatial_scene --camera orthographic` | `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde` | same, byte-identical |

Perspective differs in 84 of 921600 pixels (0.0091%). 77 are the sRGB colours of coplanar decals "decal equal a" (`[255, 89, 89]`, tint `(1, 0.1, 0.1)`) and "decal equal b" (`[89, 231, 89]`, tint `(0.1, 0.8, 0.1)`), a 65×9 strip from `(3, 365)` to `(68, 374)` plus `(89, 356)`. The overlap-interior test still requires the first decal. The other seven pixels each lie on a shared edge of a light floor slab (`Color::rgb(0.80, 0.83, 0.86)`) and a dark one (`Color::rgb(0.52, 0.57, 0.64)`). Both winning texels are the panel's outer bevel, byte 150. Encoded alone those are `[135, 138, 140]` and `[111, 116, 122]`, which is what six of the pixels swap. Pixel `(290, 278)` is the same edge behind two sprites that do not themselves change: "ghost ordered a" (figure body `[225, 225, 230]` tinted `(0.2, 0.3, 1, 0.5)`, farther) then "ghost pair b" (white head tinted `(1, 0.2, 0.2, 0.5)`). Two 0.5 blends in the sRGB framebuffer leave a quarter of the floor. That composite is exactly `[203, 129, 162]` over the light bevel and `[200, 124, 159]` over the dark bevel, delta `(3, 5, 3)`. "tree 0" and "ghost pair a" cover the ray and discard. Hiding "ghost pair b" or "ghost ordered a" changes the pixel; hiding the tree does not.

`basic_2_5d` differs in 16 pixels (0.0017%), 11 clusters, the largest 3 pixels. They are single-pixel boundaries between neighboring fogged colours, including foliage greens and one crown-green `(78, 170, 80)` against trunk-brown `(96, 61, 62)`. Those 16 were not each traced to a named entity. The orthographic frame of the same scene is byte-identical, so the projection's x and y mapping did not move the picture.

Draw calls, camera passes, instance counts, and GPU cache counts on those four frames match the pre-change capture. Reverse-Z did not add a pass or split a sprite batch. No timestamp query was enabled, so this is not a claim that the GPU got faster.

Verified on Apple M5, Metal, only. Vulkan, DirectX, OpenGL, and lavapipe are not verified. A disagreement on one of those backends reopens the compare and clear, not the decision to keep one convention. A stored 0-near matrix is still not converted. D-48 is still open; the coplanar record, when written, is ADR-0020.
