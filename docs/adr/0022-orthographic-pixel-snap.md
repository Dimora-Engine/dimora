# ADR-0022: Orthographic camera views can snap their translation to pixels

* **Status:** Settled for D-06A only.
* **Follow-up (2026-10-26).** D-06 is closed for its approved policy. Option B, per-object snapping, stays deferred. The camera-translation contract in this record is unchanged.
* **Date:** 2026-10-25
* **Backlog id:** The view-snapping slice of D-06 (D-06A). Integer-resolution output, letterboxing, nearest-neighbor upscaling, MSAA, and per-object snapping stay open.
* **Decided by:** Cal, 2026-10-25, accepting the verified behavior of the D-06A prototype (`4a87bb0`, recorded 2026-10-24). Not an acceptance of the rest of D-06.
* **Affects:** `Camera.pixel_snap` and `CameraView::new`. Not shaders, not `Transform`, not texture filtering, not [ADR-0004](0004-one-spatial-world-many-camera-configurations.md)'s policy that snapping is opt-in and not a foundation.

## Context and problem

[ADR-0004](0004-one-spatial-world-many-camera-configurations.md) accepts that a pixel-art look may need several settings, that snapping belongs at the level its technique needs, and that no style type groups them. Filtering already lives on the texture ([ADR-0017](0017-texture-sampling-colour-space-and-atlases.md)). There was no camera snap, so a fractional orthographic pose could move nearest-filtered art across texel edges.

D-06 also names integer scaling and MSAA. Those are different mechanisms. This record is only the camera-translation prototype ADR-0004 asked for before an API was treated as fixed.

## Requirements and constraints

* One camera model. No pixel-art camera type, no style component, no new projection, no extra pass.
* The entity `Transform`, `GlobalTransform`, and interpolation state stay as gameplay wrote them.
* Sprites, tilemaps, meshes, and debug lines of one camera share one view. Bones and attachments are not quantized one by one. M11 is not implemented here; that constraint is the one it must meet.
* Perspective and `CameraProjection::Custom`, including the oblique builders, do not snap. The flag stays so a later projection change does not drop it. A custom matrix is not classified by inspecting its coefficients.
* The grid is that camera's physical viewport, including a split origin, a resize, and an offscreen target. It is not the window.
* Reverse-Z ([ADR-0019](0019-reverse-z.md)) stays. Presets do not enable the flag ([ADR-0004](0004-one-spatial-world-many-camera-configurations.md)).
* Picking, culling, and drawing use the same matrices. There is no second camera.

## Options considered

### A. Quantize the orthographic view translation

* Advantages: one change moves every object that camera draws. Gameplay positions stay put. A rotated camera snaps along the screen axes, so a skeleton that shares the view stays together.
* Disadvantages: it does not make texels integer-sized, and it does not freeze a sprite that moves on its own. A move shorter than one pixel can still cross the rounding boundary and jump one pixel.

### B. Snap each sprite, bone, or tile

* Advantages: a moving sprite could sit on pixels while a mesh in the same view stays smooth.
* Disadvantages: a second policy per object, and depth would compare a snapped sprite with an unsnapped neighbor. Not required by a scene that exists.

### C. Do nothing until integer-resolution output exists

* Advantages: avoids a camera flag that does not, by itself, make a pixel-perfect frame.
* Disadvantages: leaves the fractional-camera shimmer ADR-0004 named, and blocks no measurement of the cheaper technique.

## Decision and rationale

Option A, settled for D-06A. `Camera.pixel_snap` defaults to `false`. `Camera::with_pixel_snap` stores it. The component registry lists the field as a bool.

When the flag is set and the projection is orthographic, with a finite height greater than zero, `CameraView::new` measures `viewport_height_px / orthographic_height`. That scale is both axes of the symmetric `orthographic_lh` window whose aspect is the viewport's. The view-space X and Y translation, on the same right/up basis as `look_to_lh`, is `f32::round` in pixel units and converted back. A tie rounds away from zero. The eye moves in that plane and one `look_to_lh` is built. View-space Z and the rotation are unchanged. `CameraView.position` is the adjusted eye, so lights use it. `forward`, `up`, and `right` stay the entity basis, which billboards use.

There is no extra half-pixel bias. An integer translation stays. An odd viewport height already puts the optical axis on a pixel center (`n + 0.5`). An even height puts it on a pixel edge. The correction is at most half a pixel. The result depends only on the current pose, projection, and viewport.

Perspective, custom, oblique, a non-positive height, a non-finite scale, and a forward axis parallel to up skip the shift. Disabled and skipped cameras match the unsnapped `look_to_lh`.

`camera_view_of`, `top_surface_camera`, extraction, the debug frustum, and `ray_from_pixels` build this view. A ray taken from the raw `Transform`, without `CameraView::new`, can disagree by up to half a pixel. That call site is the discrepancy. The public view is the coherent one. No hidden camera was added.

## Consequences and follow-up

* Orthographic side, top-down, and isometric cameras snap when the flag is set, along view X and Y. Steep is perspective, so it does not. Camera roll swaps which world axis lands on a screen axis. Rotated edges still alias.
* Independent sprite motion, sprite animation, camera rotation, fractional zoom, perspective, custom shear, a non-integer texel-to-pixel ratio, a rotated or perspective-projected face, and mixed-resolution assets can still shimmer. Nearest and linear filtering stay per texture. The snap does not pixelate mesh shading and does not allocate a low-resolution framebuffer.
* M8 and M11 must not quantize gameplay positions, bones, or attachments, and must not snap inside interpolation. Sharing this view is enough to keep one character's parts together.
* An editor can show the bool later. Undo and a scene file are not built. Two cameras can disagree.
* Still open inside D-06: integer-resolution output, letterboxing, nearest-neighbor upscaling, and MSAA as its own renderer setting, defaulting off. Per-object snap only if a real scene needs a smooth mesh and a pixel-locked sprite in one depth buffer.

## Evidence and revisit conditions

`4a87bb0` on `feat/m6-d06a-pixel-snap`. CPU coverage is `crates/engine-renderer/tests/pixel_snap.rs` (19 tests). GPU coverage is `crates/engine-renderer-wgpu/tests/pixel_snap.rs` (12 tests), Apple M5, Metal, `Rgba8Unorm`. `DIMORA_REQUIRE_GPU=1 scripts/check.sh` on 2026-10-24 at 6:44 PM America/Chicago: exit 0, 837 passed, 0 failed, 0 ignored, 73 suites, `rustc 1.99.0`. The four historical 1280×720 screenshot hashes matched, with `depth_view_draws` 0. Release extract of 10,000 sprites is in [../PERFORMANCE.md](../PERFORMANCE.md). Allocation traffic and batch counts match the unsnapped rows. A later off/on/on/off order on the same scene does not show a consistent extract-time difference. GPU pass time was not measured. Vulkan, DirectX, OpenGL, and lavapipe are not verified.

Reopen D-06A if a scene needs per-object snap, if the half-away-from-zero tie is the wrong pixel-center rule, or if integer-resolution output replaces view snapping. Per-object snap remains option B and was left deferred when D-06 closed on 2026-10-26.
