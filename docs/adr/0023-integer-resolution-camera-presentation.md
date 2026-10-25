# ADR-0023: A camera may present an integer-scaled internal image

* **Status:** Settled for the current D-06B contract. This does not close D-06.
* **Date:** 2026-10-25
* **Backlog id:** The integer-resolution slice of D-06 (D-06B). MSAA and per-object snapping stay open.
* **Decided by:** Cal, 2026-10-25, accepting the verified behavior of `d491bda` and the acceptance tests in `18257d4`. Not an acceptance of the rest of D-06.
* **Affects:** `Camera.internal_resolution`, `Camera.letterbox_color`, presentation of that camera's image, and window-pixel picking. Not [ADR-0004](0004-one-spatial-world-many-camera-configurations.md)'s one-camera model, and not the [ADR-0022](0022-orthographic-pixel-snap.md) snap.

## Context and problem

D-06A snaps an orthographic view translation onto the image that camera renders. It does not render that image at a chosen low resolution or enlarge it by whole pixels. A private low-resolution image plus one nearest blit can do that without a second renderer, a style component, or a new scene.

`CameraClear::Load` keeps destination pixels a later camera does not draw, and blended fragments composite against that color. An opaque blit of a cleared private image would replace those pixels and would composite blends against the internal clear. Copying the destination down into the internal grid is the wrong resolution: one internal pixel covers many destination pixels that need not share a color, and the stored framebuffer does not retain the source alpha. That is a compositor, not this slice.

## Requirements and constraints

* One spatial camera. `internal_resolution: Option<(u32, u32)>` defaults to `None`, which is the existing direct path. Presets do not set it. `CameraPresentation::Shaded` and `Depth` stay independent.
* The internal size is the whole view. `ViewportRect` is still where the presented image sits in the destination.
* Scale uses physical destination viewport pixels and integer division: `min(destination / internal)`. No crop and no stretch. An odd remainder stays on the right and the bottom.
* A scale of 0 fills the viewport with `letterbox_color` and does not render the scene or allocate a private image.
* A size of 0 or above `RENDER_TARGET_MAX_EXTENT` (8192) is rejected. It is not replaced with the destination size.
* `CameraClear::Load` together with an internal resolution is rejected. It is not drawn as `Color`. Direct `Load` is unchanged.
* The private color and depth images are not `RenderTargetId`s. The public destination receives the presented image, bars included. A later camera samples that destination.
* Orthographic `pixel_snap` uses the internal height when an internal resolution is active. Perspective and custom still ignore the flag. Simulation transforms stay put.
* Picking uses the same view. Letterbox bars belong to that camera and produce no ray. A rejected camera does not own the pixel.
* Sample count stays 1. A later MSAA resolve would have to finish before the blit.
* No render graph.

## Options considered

### A. Render into a private image and blit with integer nearest sampling

* Advantages: the existing scene pass, depth, batching, materials, and lights are unchanged. The destination is a normal color image a later camera can sample. `None` adds no pass and no image.
* Disadvantages: `Load` cannot see the destination colors inside the private image. A viewport smaller than the internal image shows bars and skips the scene.

### B. Change the projection or the sprite samplers and draw directly into the window

* Advantages: no extra image.
* Disadvantages: a low-resolution image and a high-resolution destination are different pixel grids. Drawing directly cannot both shade at 320×180 and cover 1280×720 with whole texels. It also tempts a pixel-art-only path.

### C. Do nothing until MSAA and `Load` can share one compositor

* Advantages: avoids a rejected combination.
* Disadvantages: leaves integer presentation unimplemented, and a compositor is a larger design than this camera field.

## Decision and rationale

Option A, settled for D-06B.

`Camera::with_internal_resolution` stores the size. `without_internal_resolution` clears it. `with_letterbox_color` stores the bar color. The default bar color is `Color::BLACK`. The registry lists `internal_resolution` as `none` or `{width}x{height}`, and `letterbox_color` as a color.

`fit_internal_resolution` checks the size, then `scale = min(Dw / Iw, Dh / Ih)`. Scale 0 is `PresentationFit::Letterbox`. Otherwise the displayed size is the internal size times `scale`, and the origin is `((Dw - displayed) / 2, (Dh - displayed) / 2)` in integers, then shifted by the viewport origin. For 320×180 in 1366×768 the scale is 4, the image is 1280×720, and the origin is (43, 24).

`CameraClear::Load` with any internal size, including scale 0, is `PresentationFit::Rejected`. The camera is omitted from the pass list. `ExtractStats.presentation_skipped` increments. A rate-limited warning names the camera id and the reason. The destination is not written by that camera. An invalid size is the same kind of rejection and is not a silent full-resolution draw. A camera that is the only one, and is rejected, leaves the destination as an empty scene already does.

The wgpu backend keeps one color image and one `Depth32Float` image per camera id (`Entity::id()`), sample count 1, in the renderer format, with `TEXTURE_BINDING | RENDER_ATTACHMENT | COPY_SRC` on the color image. The scene pass clears that image and draws the existing pipelines into it. Depth presentation linearizes that depth, then the blit runs. The blit is a fullscreen triangle, `textureLoad` at `floor((pixel - origin) / scale)`, viewport and scissor on the display rectangle, no lighting, fog, tint, or sampler. Presentation draws are `presentation_draws`, not world draws. A steady frame reuses the images. A size change replaces them and counts `internal_target_reallocs`. A camera that is not `Scaled` this frame drops its images. Letterbox-only and direct frames allocate none. If the device rejects the size, the viewport is filled with the letterbox color and is not promoted to a full-resolution scene.

`CameraView` builds the projection from the internal pixel size. Orthographic snap uses that height. `render_pixel` maps a destination pixel through the display rectangle and the integer scale. The center of an output block is the center of the internal pixel. `ray_from_pixels` is unchanged and takes render pixels. `ray_from_window_pixel` goes through `render_pixel`. Bars return `None` from the ray and still contain the camera, so `top_surface_camera` does not fall through. A rejected camera contains nothing.

## Consequences and follow-up

* `Load` plus an internal resolution is unsupported until a design can keep per-destination-pixel color without downsampling unlike pixels into one internal texel. Direct `Load` stays.
* Scale 0 is a defined letterbox, not a fractional or cropped picture. Those small-window policies are not implemented.
* MSAA remains a renderer setting to add later. The blit must sample a resolved color image. This slice does not allocate a multisample image.
* Per-object snap is not added. Sprite samplers are unchanged.
* The editor is not built. The fields are plain camera data, and the presented image includes the bars, so a later viewport can show both without a second coordinate system. That editor must show a `Load` plus internal-resolution rejection and must not present the skipped camera as a successful composite. The private images are renderer resources keyed by the live entity index. They are not a public target and they are not a stable entity id.
* Native-resolution UI is a different camera or a later UI pass over the same destination. This field does not make UI low-resolution.
* Vulkan, DirectX, OpenGL, and lavapipe are not verified.
* The cache key is the live entity index. Images are cleared every frame and dropped when that camera is absent, so a reused index does not keep another camera's picture. This is not a new stable-id system.

## Evidence and revisit conditions

`d491bda` on `feat/m6-d06b-internal-resolution`, from `3c7ccfb`. Acceptance tests are `18257d4`. CPU coverage is `crates/engine-renderer/tests/internal_resolution.rs` (21 tests). GPU coverage is `crates/engine-renderer-wgpu/tests/internal_resolution.rs` (23 tests), Apple M5, Metal. Fifteen of those GPU tests use `Rgba8Unorm`. Two use `Rgba8UnormSrgb`: an unlit opaque sprite, a half-yellow blend over red, and a lit sprite each match the direct sRGB path within one 8-bit channel, and the center is more than eight channels away from a second sRGB encode. Nearest 2× blocks stay flat and match one IEC encode of the linear color within one channel, including the clear pixel on the next internal texel. The blit is `textureLoad` with no manual gamma. The output-color contract is unchanged. Vulkan, DirectX, OpenGL, and lavapipe are not verified.

The same GPU file checks private-image lifetime. A steady frame reuses the images. A size change reallocates once. Disabling drops them. Despawn drops them. A respawn that reuses the entity index clears the new image and does not show the previous camera's pixels. Sixteen create-and-remove cycles create 16 images and drop 16. Two cameras keep distinct images when their order changes. Moving one camera from the surface to an offscreen target reuses its image, and the public target receives the presented color. A rejected `Load` camera does not write; the next real writer attachment-clears the destination. A direct `Load` camera still composites. `DIMORA_REQUIRE_GPU=1 scripts/check.sh` on 2026-10-25 at 4:16 PM America/Chicago: exit 0, 881 passed, 0 failed, 0 ignored, 76 suites, `rustc 1.99.0`. The four historical 1280×720 screenshot hashes matched at implementation (`d491bda`) and were compared again at this acceptance. Release extract and encode of the 64-sprite depth-view scene are in [../PERFORMANCE.md](../PERFORMANCE.md). That bench was not repeated: the acceptance did not change fit, batching, or the blit. GPU pass timestamps were not measured.

Reopen this record if `Load` must composite through the internal image, if a small viewport should scale fractionally or crop, or if MSAA cannot resolve before the blit without a second color path. Settling it does not close D-06.
