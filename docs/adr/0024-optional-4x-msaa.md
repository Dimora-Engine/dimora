# ADR-0024: Optional renderer-wide 4× MSAA

* **Status:** Proposed. Stage 2 is implemented on `feat/m6-d06c-msaa` (`2bc97fa`) and is not accepted. This does not close D-06, and it does not complete M6.
* **Date:** 2026-10-26
* **Backlog id:** The MSAA slice of D-06 (D-06C). Per-object snapping stays open.
* **Decided by:** Proposed for review. Cal approved the core design for this implementation on 2026-10-26. That approval is not an acceptance of the record.
* **Affects:** The renderer's sample count and `Game::set_msaa`. Not a camera field and not a component. Not [ADR-0004](0004-one-spatial-world-many-camera-configurations.md)'s one-camera model, not the [ADR-0022](0022-orthographic-pixel-snap.md) snap, and not the [ADR-0023](0023-integer-resolution-camera-presentation.md) blit. The blit still samples a resolved one-sample image. `CameraClear::Load` with an internal resolution stays rejected by that record.

## Context and problem

D-06 treats MSAA as an optional technique, not a foundation and not a pixel-art mode. The renderer draws one sample. Edges of triangles are binary. A multisample color image can hold intermediate coverage on those edges.

`wgpu` 30's `RenderPassColorAttachment.resolve_target` stores the resolved image for the whole attachment. A viewport or scissor on the scene pass does not limit that store. Dimora cameras share one destination and often own only a rectangle of it, so a hardware resolve would replace pixels outside the camera. `Depth32Float` is guaranteed `MULTISAMPLE_X4` and is not guaranteed `MULTISAMPLE_RESOLVE`. A render pass uses one sample count for every attachment, so color and depth move together. The shader resolve does not need the resolve flag.

A failed 4× request must not be reported as success and must not keep drawing at one sample under that name. `wgpu` 30's `create_texture` does not return a `Result`. The recoverable check already in the renderer is `validate_texture_dimensions`. Device loss stays the existing unhandled path. This record does not add an out-of-memory contract.

## Requirements and constraints

* `MsaaSamples::One` and `MsaaSamples::Four` only. `One` is the default. The count is renderer-wide. There is no per-camera count and no per-material count.
* The public game API does not expose `wgpu` types. The setting is not a camera field and is not in the component registry. A later editor can read the selected count and whether 4× is available.
* Four is stored only when the adapter reports 4 samples for the renderer's color format and for `Depth32Float`. The color formats in use are `Rgba8Unorm` and `Rgba8UnormSrgb`. A rejection leaves the previous count in place. The renderer never reports 4× while the frame is one sample. A request stored before a renderer exists is not the active count. `Game::msaa` reports the active count, or `One` when no renderer exists.
* The one-sample path stays the historical path: same pipelines, no 4× images, no resolve draw, no load-expansion draw. Historical screenshots stay on that default.
* The resolve reads the four color samples, averages them in the numerical space `textureLoad` already returns, and writes only the camera viewport. An sRGB attachment encodes on store. The shader does not apply a second gamma. Material shaders are unchanged.
* Reverse-Z stays `Greater` with a depth clear of 0. `alpha_to_coverage` stays off. Sampler defaults stay. MSAA changes geometric coverage. A mask or cutout still discards a whole fragment. Texture filtering, orthographic pixel snap, and integer presentation stay independent.
* A later direct `CameraClear::Load` copies the already-resolved destination into all four color samples of its viewport, then draws, then resolves that viewport only. The first writer of a destination still clears the whole image. Depth still clears to 0 for that camera. `Load` is not drawn as `Color`.
* Depth presentation at 4× displays the nearest reverse-Z sample, the maximum of the four stored samples. It does not average them and it does not report a coverage fraction. The one-sample depth view is unchanged. Near stays 1, far stays 0, and the empty and unsupported colors stay.
* A public render target stays one sample. Its 4× color and depth are private. A later camera samples the resolved target.
* An internal-resolution camera renders at the internal pixel size, resolves into the existing one-sample internal image, then uses the existing nearest blit. The multisample images are the internal size. `CameraClear::Load` with an internal resolution stays rejected. Scale 0 still skips the scene and allocates no private image.
* One is always usable.

## Options considered

### A. One renderer-wide count, with a viewport-scoped shader resolve

* Advantages: one sample count matches the pass. The resolve can be scissored. Depth does not need a hardware resolve. The one-sample path allocates nothing extra. The destination stays a one-sample image, which is what a later camera, a blit, and a screenshot already sample.
* Disadvantages: 4× color and depth are large. Each destination that draws at 4× keeps its own pair. A later `Load` needs a pass that copies the resolved color into the samples before the scene.

### B. Hardware `resolve_target` on the scene pass

* Advantages: the backend averages the samples.
* Disadvantages: the store covers the whole attachment. A split viewport would replace the other camera's pixels. Depth still has no resolve target, so depth presentation would show one sample or nothing.

### C. A sample count on each camera or material

* Advantages: a pixel-art camera could stay at one sample beside a smoothed camera.
* Disadvantages: one pass has one sample count. Two counts in one frame means two attachment sets and a resolve between them, which is a compositor. D-06 does not ask for that, and a per-material count would change batching.

### D. Leave MSAA unstarted

* Advantages: no extra images.
* Disadvantages: geometric edges stay binary, and the approved design would stay a review.

## Decision and rationale

Option A, proposed. It is not settled.

`MsaaSamples` lives in `engine-renderer`. `select_msaa` accepts `Four` only when both format checks pass. `Err` means the caller keeps the previous count. `msaa4_camera_skip` does not reject a camera combination. `Load` with an internal resolution is still omitted by presentation fit, before a pass exists. An image that `validate_texture_dimensions` rejects is `MsaaSkip::ImagesUnavailable` at draw time. The count stays `Four`. That camera is not drawn at one sample.

`SceneRenderer::set_msaa` asks the adapter with `sample_count_supported(4)` and then calls `select_msaa`. The first successful `Four` builds the 4× pipelines once, including the load-expansion pipeline and the multisample depth-view pipeline. A second `Four` does not rebuild them. Returning to `One` drops every 4× image and keeps the pipelines. `Game::set_msaa` stores a request. `Game::msaa` reports the headless renderer's active count, or `One` when no renderer exists, including when a four-sample request is only stored. `Game::msaa4_available` is `None` until that renderer exists. `render_to_rgba` applies the request before drawing and returns `RenderError::Device` on rejection, without drawing. Window startup applies it before the frame loop; a rejection logs, stores the error, and does not keep a one-sample renderer under a four-sample request.

The 4× scene pipelines are the existing sprite, mesh, debug-line, and viewport-clear pipelines at sample count 4. `alpha_to_coverage_enabled` is false. Letterbox fill and the integer presentation blit stay one sample. The default renderer does not build the 4× set.

Each destination that a 4× camera actually draws owns a color image and a depth image at that camera's render size. A direct surface camera uses the surface size. An offscreen target uses the target's size. An internal-resolution camera uses the internal pixel size. Two cameras that write the same destination share that pair for the frame, one after the other. A letterbox with scale 0 allocates neither image. The color image is the renderer format, sample count 4, `RENDER_ATTACHMENT | TEXTURE_BINDING`. The depth image is `Depth32Float`, sample count 4, `RENDER_ATTACHMENT | TEXTURE_BINDING`. Bytes reported by `MsaaImageUsage` are `width * height * 4 * 4` per image, summed across retained pairs. `width` and `height` describe the surface pair, and are zero when only a private pair is retained. This is a size calculation, not a driver allocation measurement.

The scene pass does not set `resolve_target`. The first shaded writer clears the 4× color image. A later `Color` camera loads it and draws the 4× viewport clear. A later shaded `Load` camera first runs a sample-count-4 pass with no depth attachment: it samples the resolved one-sample destination and writes that color into every sample of the viewport. The scene pass then loads those samples. Depth is cleared to 0 for every camera. A shaded camera then averages samples 0 through 3 into its viewport only. The first writer clears the one-sample destination before that resolve. A later camera loads the destination and uses the same scissor, so pixels outside that rectangle stay. `resolve_draws` and `expand_draws` count those passes. They are not world draws. One sample records zero.

An sRGB `textureLoad` returns linear values. Writing the multisample attachment encodes. The resolve decodes, averages four identical samples for a fully covered pixel, and encodes once more. There is no manual gamma. A fully covered pixel can differ from one sample by the 8-bit rounding of that round trip.

Depth presentation does not resolve color. It writes the one-sample destination from the multisample depth, using the maximum stored sample, then the existing linearization. The empty color and the unsupported-projection color are unchanged. An internal depth view writes the private one-sample image, and the existing blit presents that image. The blit does not bind a multisample texture.

While `Four` is selected the one-sample surface depth is not retained. A size the dimension check rejects does not allocate, does not change the count, does not draw that camera at one sample, and does not drop a previously valid image of another size. A direct surface `Color` camera that fails this way, with no `Load` surface camera also skipped and with nothing else written, still clears the destination to opaque black. A skipped `Load` does not replace an existing destination with that black. A camera that does not draw does not become the first writer. Headless readback allocates a new destination texture each call, so the black defines otherwise uninitialized texels.

The window surface usage includes `TEXTURE_BINDING` so a later `Load` can sample the resolved swapchain. The scene pass does not bind that texture while it is writing it. Expansion and resolve are separate passes. Offscreen and internal color images already include `TEXTURE_BINDING`. An expansion bind group is created for that submit and is not a retained image.

## Consequences and follow-up

* This implementation is not an accepted MSAA feature until this record is settled. D-06 stays open because per-object snap is still open. M6 stays open.
* Mask and cutout edges do not gain alpha-to-coverage smoothing. Fully covered interiors can match one sample within a channel. Geometric edges of opaque triangles can carry intermediate coverage.
* The depth diagnostic is the nearest covered sample. A pixel with one covered sample and three clear samples shows that surface, not the empty color and not a blend toward it.
* `CameraClear::Load` with an internal resolution stays rejected. This record does not change that ADR-0023 rule.
* Pipelines stay after a return to one sample. Images do not. The next `Four` frame allocates images again and does not rebuild pipelines. A resize replaces the pair for that destination. Removing a target or an internal camera drops its pair. Steady frames do not create images.
* Vulkan, DirectX, OpenGL, and lavapipe are not verified. The GPU evidence is Apple M5, Metal.
* The headless `Load` path is pixel-tested. The window flag is set and was not given its own swapchain readback.
* Per-object snap is not added. Sprite samplers are unchanged. There is no 2× or 8× count.
* The editor is not built. `Game::msaa` and `Game::msaa4_available` are the inspection points, once a renderer exists.
* GPU pass timestamps were not measured. `msaa_bench` completion waits for the device queue. That is not a pass time.

## Evidence and revisit conditions

Stage 1 is `255863c89bc467799bfe98513db9e7eb3e97fc46`. Stage 2 is `2bc97fa1a12793c61266cf1ab2a6c24d10aab816` on `feat/m6-d06c-msaa`, from `143146b`. CPU coverage is `crates/engine-renderer/tests/msaa.rs` (5 tests) and `crates/engine/tests/msaa.rs` (2 tests). GPU coverage is `crates/engine-renderer-wgpu/tests/msaa.rs` (14 tests: one capability test with no adapter, and 13 Apple M5 Metal pixel tests) and `tests/msaa_stage2.rs` (25 Apple M5 Metal pixel tests). `guaranteed_format_features` accepts 4× for `Rgba8Unorm`, `Rgba8UnormSrgb`, and `Depth32Float` without requiring `MULTISAMPLE_RESOLVE` on depth.

Stage 2 pixels, on that machine: a later `Load` keeps an uncovered red pixel and replaces a covered pixel with opaque blue; a translucent sprite matches one sample within one channel and keeps destination alpha 255; an sRGB covered pixel and the expanded clear match one sample within one channel; a small `Load` viewport and a later camera leave the rest; overlapping viewports keep the pixels outside the later one. Depth shows a partial-coverage edge as the covered sample, orders a near card above a far card under reverse-Z, keeps the empty and unsupported colors, and gives a later viewport its own clear. An offscreen mesh and sprite resolve, and a later camera samples that resolved image, including intermediate coverage. Two targets do not alias. An internal 320×180 image resolves before a 4× nearest blit into 1280×720, and 1366×768 keeps the letterbox bars. Scale 0 allocates no multisample image. Switching 1× → 4× → 1× matches a renderer that never left one sample. A skipped `Load` beside a failed `Color` camera does not log the opaque-black clear.

`DIMORA_REQUIRE_GPU=1 scripts/check.sh` on 2026-10-26 at 1:13 PM America/Chicago: exit 0, 927 passed, 0 failed, 0 ignored, 80 suites, `rustc 1.99.0`. The screenshot hashes are in [../../TASKS.md](../../TASKS.md). Release encode, extraction, and calculated image bytes are in [../PERFORMANCE.md](../PERFORMANCE.md). GPU pass timestamps were not measured.

Settle this record only after review of that evidence. A measured cost that makes 4× the wrong default sample count would reopen the count, not the one-sample path. Proposing this record does not close D-06.
