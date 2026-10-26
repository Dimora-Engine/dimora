# ADR-0024: Optional renderer-wide 4× MSAA

* **Status:** Proposed. Stage 1 is implemented on `feat/m6-d06c-msaa` and is not accepted. This does not close D-06, and it does not complete M6.
* **Date:** 2026-10-26
* **Backlog id:** The MSAA slice of D-06 (D-06C). Per-object snapping stays open. Stage 2 is not started.
* **Decided by:** Proposed for review. Cal approved the core design for this implementation on 2026-10-26. That approval is not an acceptance of 4× for every camera or output.
* **Affects:** The renderer's sample count, `Game::set_msaa`, and direct shaded surface cameras that clear color. Not a camera field and not a component. Not [ADR-0004](0004-one-spatial-world-many-camera-configurations.md)'s one-camera model, not the [ADR-0022](0022-orthographic-pixel-snap.md) snap, and not the [ADR-0023](0023-integer-resolution-camera-presentation.md) blit. The blit still requires a resolved one-sample image. Stage 1 refuses internal resolution while 4× is selected.

## Context and problem

D-06 treats MSAA as an optional technique, not a foundation and not a pixel-art mode. The renderer draws one sample. Edges of triangles are binary. A multisample color image can hold intermediate coverage on those edges.

`wgpu` 30's `RenderPassColorAttachment.resolve_target` stores the resolved image for the whole attachment. A viewport or scissor on the scene pass does not limit that store. Dimora cameras share one destination and often own only a rectangle of it, so a hardware resolve would replace pixels outside the camera. `Depth32Float` is guaranteed `MULTISAMPLE_X4` and is not guaranteed `MULTISAMPLE_RESOLVE`. A render pass uses one sample count for every attachment, so color and depth move together. The shader resolve does not need the resolve flag.

A failed 4× request must not be reported as success and must not keep drawing at one sample under that name. `wgpu` 30's `create_texture` does not return a `Result`. The recoverable check already in the renderer is `validate_texture_dimensions`. Device loss stays the existing unhandled path. This record does not add an out-of-memory contract.

## Requirements and constraints

* `MsaaSamples::One` and `MsaaSamples::Four` only. `One` is the default. The count is renderer-wide. There is no per-camera count and no per-material count.
* The public game API does not expose `wgpu` types. The setting is not a camera field and is not in the component registry. A later editor can read the selected count and whether 4× is available.
* Four is stored only when the adapter reports 4 samples for the renderer's color format and for `Depth32Float`. The color formats in use are `Rgba8Unorm` and `Rgba8UnormSrgb`. A rejection leaves the previous count in place. The renderer never reports 4× while the frame is one sample.
* The one-sample path stays the historical path: same pipelines, no 4× images, no resolve draw. Historical screenshots stay on that default.
* The resolve reads the four color samples, averages them in the numerical space `textureLoad` already returns, and writes only the camera viewport. An sRGB attachment encodes on store. The shader does not apply a second gamma. Material shaders are unchanged.
* Reverse-Z stays `Greater` with a depth clear of 0. `alpha_to_coverage` stays off. Sampler defaults stay. MSAA changes geometric coverage. A mask or cutout still discards a whole fragment. Texture filtering, orthographic pixel snap, and integer presentation stay independent.
* Stage 1 draws 4× only for a direct shaded surface camera that clears color. `CameraClear::Load`, `CameraPresentation::Depth`, an offscreen target, and an internal resolution are skipped with a diagnostic. They are not drawn at one sample, and they are not treated as `Color`.
* One is always usable.

## Options considered

### A. One renderer-wide count, with a viewport-scoped shader resolve

* Advantages: one sample count matches the pass. The resolve can be scissored. Depth does not need a hardware resolve. The one-sample path allocates nothing extra. The destination stays a one-sample image, which is what a later camera, a blit, and a screenshot already sample.
* Disadvantages: 4× color and depth are large. Stage 1 cannot yet expand `Load`, show depth, resolve before the integer blit, or resolve an offscreen target. Those combinations are rejected rather than approximated.

### B. Hardware `resolve_target` on the scene pass

* Advantages: the backend averages the samples.
* Disadvantages: the store covers the whole attachment. A split viewport would replace the other camera's pixels. Depth still has no resolve target, so depth presentation would show one sample or nothing.

### C. A sample count on each camera or material

* Advantages: a pixel-art camera could stay at one sample beside a smoothed camera.
* Disadvantages: one pass has one sample count. Two counts in one frame means two attachment sets and a resolve between them, which is a compositor. D-06 does not ask for that, and a per-material count would change batching.

### D. Leave MSAA unstarted

* Advantages: no extra images and no rejected combinations.
* Disadvantages: geometric edges stay binary, and the approved design would stay a review.

## Decision and rationale

Option A, proposed, and only for the stage-1 contract below. It is not settled.

`MsaaSamples` lives in `engine-renderer`. `select_msaa` accepts `Four` only when both format checks pass. `Err` means the caller keeps the previous count. `msaa4_camera_skip` returns a skip for `Four` unless the camera is direct, `CameraOutput::Surface`, `CameraPresentation::Shaded`, and `CameraClear::Color`. The check order is presentation, output, fit, then clear. `One` never skips.

`SceneRenderer::set_msaa` asks the adapter with `sample_count_supported(4)` and then calls `select_msaa`. The first successful `Four` builds the 4× pipelines once. A second `Four` does not rebuild them. Returning to `One` drops the 4× images and keeps the pipelines. `Game::set_msaa` stores a request. `Game::msaa` reports the headless renderer's active count, or `One` when no renderer exists, including when a four-sample request is only stored. `Game::msaa4_available` is `None` until that renderer exists. `render_to_rgba` applies the request before drawing and returns `RenderError::Device` on rejection, without drawing. Window startup applies it before the frame loop; a rejection logs, stores the error, and does not keep a one-sample renderer under a four-sample request.

The 4× pipelines are the existing scene pipelines at sample count 4: seven sprite, six mesh, the debug line, and the viewport clear. `alpha_to_coverage_enabled` is false. Letterbox fill, integer presentation, and depth visualization stay one sample and are not built again for 4×. The default renderer does not build the 4× set.

The 4× color image uses the renderer format, sample count 4, and `RENDER_ATTACHMENT | TEXTURE_BINDING`. The 4× depth image is `Depth32Float`, sample count 4, `RENDER_ATTACHMENT` only. The scene pass does not set `resolve_target`. The first camera in a frame clears that color image. A later `Color` camera loads it and draws the 4× viewport clear. Depth is cleared to 0 and discarded. A second pass, sample count 1, binds a different view of the multisample color and averages samples 0 through 3. The first writer clears the one-sample destination, then the resolve is viewport- and scissor-limited. A later camera loads the destination and uses the same scissor, so pixels outside that rectangle stay. `resolve_draws` counts that pass. It is not a world draw. One sample records zero.

While `Four` is selected the one-sample surface depth is not retained. A size the dimension check rejects does not allocate, does not change the count, does not draw that camera at one sample, and does not drop a previously valid image of another size. If every requested surface camera was skipped, the destination is cleared to opaque black. That black is not the skipped camera's clear color. A later supported camera in the same frame is the first writer and is not preceded by that clear. Headless readback allocates a new destination texture each call, so the black defines otherwise uninitialized texels.

`MsaaSkip` names `Load`, `DepthPresentation`, `Offscreen`, `InternalResolution`, and `ImagesUnavailable`. The warning is remembered per renderer, per camera id and reason. A repeated frame does not log the same skip again.

## Consequences and follow-up

* Stage 1 is not a generally accepted MSAA feature. Direct `Load`, depth presentation, offscreen targets, and internal resolution at 4× are explicit failures. Ordinary scenes that use those combinations must not silently look right.
* Stage 2 still has to do five things, and this slice does not start them. Direct `Load` must expand destination color, including alpha, into the multisample viewport before the scene, and must not replace that color with a clear. Depth presentation must reduce to the nearest reverse-Z sample and keep the existing diagnostic colors. An internal-resolution camera must resolve into its one-sample private image before the integer blit. An offscreen target must be that resolved image before a later camera samples it. Overlapping, reordered, and camera-to-texture views still need their own tests. Two non-overlapping direct `Color` viewports are in stage 1.
* The stage-1 images are one surface-sized pair. Stage 2 needs multisample images at each destination's render size. A 320×180 private image cannot resolve a 1280×720 multisample texture. The resolve pass already targets whatever one-sample view it is given, so that part does not need a second world.
* Pipelines stay after a return to one sample. Images do not. The next `Four` frame allocates images again and does not rebuild pipelines.
* Mask and cutout edges do not gain alpha-to-coverage smoothing. Fully covered interiors can match one sample. Geometric edges of opaque triangles can carry intermediate coverage.
* Vulkan, DirectX, OpenGL, and lavapipe are not verified.
* Per-object snap is not added. Sprite samplers are unchanged.
* The editor is not built. `Game::msaa` and `Game::msaa4_available` are the inspection points, once a renderer exists.

## Evidence and revisit conditions

Stage 1 is `255863c89bc467799bfe98513db9e7eb3e97fc46` on `feat/m6-d06c-msaa`, from `143146b`. CPU coverage is `crates/engine-renderer/tests/msaa.rs` (5 tests) and `crates/engine/tests/msaa.rs` (2 tests). GPU coverage is `crates/engine-renderer-wgpu/tests/msaa.rs` (14 tests: one capability test with no adapter, and 13 Apple M5 Metal pixel tests). `guaranteed_format_features` accepts 4× for `Rgba8Unorm`, `Rgba8UnormSrgb`, and `Depth32Float` without requiring `MULTISAMPLE_RESOLVE` on depth. Empty flags reject 4×. A diagonal triangle is binary at one sample and has intermediate coverage at four. A covered blend center matches one sample within one channel. A mask of alpha 0 stays the clear color, with no partial-coverage gray. An sRGB unlit color matches one IEC encode within one channel and is more than eight away from a second encode. A half viewport leaves the other half on the clear. Two non-overlapping surface clears stay red and blue. A later frame replaces the previous clear. Resize reallocates once. Switching counts keeps `msaa_pipeline_creates` at 1, and a covered one-sample center matches a renderer that never selected four. Unsupported cameras increment `msaa_skipped` and do not change the count. `DIMORA_REQUIRE_GPU=1 scripts/check.sh` on 2026-10-26 at 11:42 AM America/Chicago: exit 0, 902 passed, 0 failed, 0 ignored, 79 suites, `rustc 1.99.0`. The screenshot hashes are in [../../TASKS.md](../../TASKS.md). Release encode and completion of the 64-sprite scene are in [../PERFORMANCE.md](../PERFORMANCE.md). GPU pass timestamps were not measured.

Settle this record only after stage 2 verifies `Load` expansion, depth reduction, resolve-before-blit, offscreen sampling, and overlapping or reordered views, on at least the machines those tests name. A measured cost that makes 4× the wrong default sample count would reopen the count, not the one-sample path. Proposing this record does not close D-06.
