# ADR-0017: Texture colour space, samplers, partial updates, and atlas mip limits

* **Status:** Settled as the M5 contract. M5-2A (`c687d5b`) and M5-2B (`11b914f`) implement the CPU descriptors, GPU formats, and sampler cache. M5-2C (`622ef80`) generates mip chains on the GPU. M5-3A records CPU region updates and does not upload them. GPU partial uploads and the atlas packer are not implemented.
* **Date:** 2026-10-16
* **Backlog id:** M5 texture tasks (KI-6, KI-25). D-06 stays open for texel snapping.
* **Decided by:** project owner (M5-1 planning approval, corrected in the M5-1 final review)
* **Affects:** `Texture`, the wgpu upload path, the sampler cache, the later atlas packer and sub-rectangle update. Public enums stay in engine crates and do not expose `wgpu` types.

## Context and problem

`Texture` is RGBA8 with `TextureFilter::{Nearest, Linear}`. The backend (`wgpu` 30.0.1) uploads every texture with `Queue::write_texture` as `Rgba8UnormSrgb`, mip count 1, and a tight row stride (`4 * width` in `upload_texture`). It binds one of two samplers: nearest or linear, both `ClampToEdge`. The device is created with `DeviceDescriptor`'s default limits and features. There is no `generate_mipmaps` helper on the device.

`SamplerDescriptor::anisotropy_clamp` is a `u16`. The type comment requires at least 1, and requires every filter mode to be linear when the value is not 1. wgpu-core 30.0.1 `Device::create_sampler` enforces that: a clamp below 1 is `InvalidAnisotropy`; a clamp other than 1 requires linear minification, linear magnification, and linear mip filtering. That validation has no power-of-two check. The wgpu-hal sampler descriptor documents the clamp as the range [1, 16].

`COPY_BYTES_PER_ROW_ALIGNMENT` is 256. `write_texture` calls `validate_texture_buffer_copy` with alignment not required for the offset or the bytes per row. The function then copies into a staging buffer and aligns those staging rows itself. The 256-byte check applies when a copy is validated as aligned, including `copy_buffer_to_texture` and `copy_texture_to_buffer`. The existing screenshot readback uses `copy_texture_to_buffer` and pads. That path is not the texture upload.

`Color` is linear. sRGB upload plus hardware decode is why a `Color` multiplied in the shader is not decoded a second time. A data texture cannot opt out of that decode (KI-6). Any revision change re-uploads the whole image (KI-25). The GPU cache keys a texture by asset id and revision and evicts on `AssetManager::remove` or after idle frames (ADR-0002). The sampler is chosen inside the bind group at upload, so a filter change today uploads the pixels again.

ADR-0004 already says filtering and wrap belong on the texture or sampler, not on a style mode. D-06 still owns texel snapping and is not decided here.

## Requirements and constraints

* Defaults must match today's pictures: sRGB, nearest min and mag, clamp, no mips, anisotropy off. The four screenshots against commit `6dbabb8` stay the regression bar for any phase that touches the GPU.
* Colour space is per texture, not a renderer switch.
* sRGB colour images use sRGB sampling. Linear data (a mask, a normal map in M14) must not be decoded as sRGB.
* Material `Color` is already linear and must not be converted again.
* GPU image storage and sampler objects are separate caches. A different sampler does not allocate a second copy of the same pixels.
* Partial updates are specified here and implemented in a later M5 phase, not in M5-2. The transfer follows the alignment of the GPU operation it calls. Affected mip levels must stay correct. Mip maintenance is not a full-image upload.
* The atlas packer is a later M5 phase. It works in texels (ADR-0016). How much minification its padding protects is decided by GPU tests. A formula written before those tests is an estimate.
* D-16 stays open. Unload does not remove textures.
* M5-2 keeps `Texture::from_rgba8`, PNG decoding, `TextureFilter`, `with_filter`, the sRGB default, nearest filtering, clamp-to-edge addressing, no mipmaps by default, existing texture handles, and existing sprite and mesh rendering. It does not add public `wgpu` types.

## Options considered

### A. Leave every texture sRGB, clamp, and mip-less
* Advantages: no GPU change.
* Disadvantages: M5's acceptance tests cannot be written. Normal maps and a glyph atlas stay blocked.

### B. Per-texture descriptor, defaults equal to today
* Advantages: existing assets keep their sampling. New fields are opt-in. The backend can cache samplers separately from image storage.
* Disadvantages: the renderer must tell a pixel change from a sampler change. Mip generation is new GPU work, written by the backend because wgpu 30 has no mip helper.

### C. A global renderer colour space
* Advantages: one switch.
* Disadvantages: a data texture and an albedo cannot differ. It contradicts the per-texture rule.

## Decision and rationale

Option B.

**Colour space.** `ColorSpace::{Srgb, Linear}`, default `Srgb`. The GPU format is `Rgba8UnormSrgb` or `Rgba8Unorm`. The asset chooses. Linear bytes are stored and sampled as linear. The engine does not run `srgb_to_linear` on them. `Color` and sprite tint stay linear and are multiplied after sampling, which is the current shader.

**Sampler descriptor, engine-owned.** Min filter, mag filter, mip filter, address mode per axis, anisotropy level. Defaults: nearest min, nearest mag, no mip sampling, clamp on each axis, anisotropy 1. `Texture::with_filter` continues to set min and mag together so existing call sites stay valid. Mipmaps are off unless the texture asks for generation. Address modes are clamp, repeat, and mirror. Clamp-to-border is not in M5. The engine does not impose a power-of-two anisotropy rule. wgpu 30.0.1 does not require one, and no engine policy chooses one.

**CPU configuration.** `ColorSpace`, `AddressMode`, `MipFilter`, `Mipmaps`, and `SamplerDesc` live on `Texture` in `engine-assets`. `from_rgba8`, `solid`, `from_fn`, and PNG decode still produce sRGB, nearest, clamp, no mips, and anisotropy 1. `sampler` is the configuration the GPU reads, including a min filter that differs from the mag filter. `Texture::filter` is private. `filter()` reports linear only when both min and mag are linear. `with_filter` sets that projection and both filters, and leaves address, mip filter, and colour space alone. `with_sampler` stores the normalized descriptor and sets the projection the same way. A write to the private field, which other crates cannot do, does not update `sampler` and does not change the draw.

**Mip levels and the mip filter are separate.** `Mipmaps` asks which levels exist. `mip_filter` says how to sample levels that exist. Setting a mip filter does not select `Mipmaps::Generate`. `Mipmaps::None` with a mip filter is valid and allocates one level.

`Mipmaps::Generate` asks for `floor(log2(max(width, height))) + 1` levels. The renderer uploads the base level and fills the rest with a private blit in `engine-renderer-wgpu`. Each pass reads one mip view and writes the next. The source view is a single level, and min and mag on the blit sampler are both linear, so the sample is the GPU's linear filter of the previous level. Even sizes land on a 2×2 average. Odd sizes use that same filter; it is not a separate full-texel box. Alpha is averaged as stored. It is straight, not premultiplied, and normals are not renormalized.

An sRGB texture is `Rgba8UnormSrgb`. Sampling decodes to linear, the blit averages those linear values, and the sRGB attachment encodes the result. A linear texture is `Rgba8Unorm` and is not encoded. On Apple M5 Metal, a 2×2 black/white checker read back as `[188, 188, 188, 255]` from the sRGB 1×1 mip and `[128, 128, 128, 255]` from the linear 1×1 mip. Sampling either result into a linear render target is about half intensity. Byte 188 is the sRGB encoding of linear 0.5, not a second average of the encoded bytes.

The sampler LOD clamp is part of the cache key. A one-level view, including `Mipmaps::None` and a generated chain whose sampler sets no mip filter, uses LOD `0..32`. The view contains only the base level. A clamp of `0..0`, or a tiny upper bound, makes minification use the magnification filter on this Metal device even when the mip filter is linear. When a mip filter is set and more than one level is allocated, the view includes the chain and the upper LOD is the last allocated level. Nearest and linear mip filters are different cache entries. A missing mip filter does not generate levels.

**Anisotropy, normalized before a sampler exists.** The CPU descriptor is validated and normalized, and the GPU sampler is created once from that result.

* Anisotropy below 1 is invalid. `SamplerDesc::normalized` and `Texture::with_sampler` return `SamplerConfigError::AnisotropyBelowOne`. They do not raise 0 to 1. If a caller then writes the public `anisotropy` field to 0, the GPU path logs once, uses 1, keeps the requested filters, and does not pass 0 to `create_sampler`.
* Anisotropy above 1 requires linear magnification, linear minification, and linear mip filtering. If any of those is not linear, the normalized descriptor sets anisotropy to 1 and keeps the requested filters. Nearest-neighbour filtering stays nearest.
* Anisotropy above 16 is clamped to 16 when the filters are linear. That cap is the wgpu-hal interface range. wgpu-core applies `anisotropy_clamp.min(16)` when `DownlevelFlags::ANISOTROPIC_FILTERING` is set. It is not a power-of-two sequence and not a maximum read from a live adapter.
* The sampler is created once. `Device::create_sampler` in wgpu 30.0.1 returns the sampler, not a `Result`. Known validation failures are removed before that call. A later HAL failure is reported by wgpu on the device error sink. The engine does not retry the create at anisotropy 1, and it does not treat that failure as a recoverable asset-load miss.

**Downlevel.** If `DownlevelFlags::ANISOTROPIC_FILTERING` is absent, wgpu-core sets the HAL clamp to 1 and still creates the sampler. The renderer reads `adapter.get_downlevel_capabilities().flags` once when it is created. Anisotropy above 1 then becomes 1, the other sampler settings stay, one log line is written, and the sampler is created once. The engine does not guess a maximum above 16.

**Capability query.** M5-2B GPU tests on this machine queried that flag. The adapter was Apple M5, Metal, `IntegratedGpu`, and `DownlevelFlags::ANISOTROPIC_FILTERING` was present. No live maximum above 16 was read. The absent-flag path is covered by a CPU test of the same normalization; it was not executed on a device that lacks the flag.

**Capability uncertainties.**

* The flag's comment in wgpu-types 30.0.1 says every backend and device supports anisotropic filtering and that an implementation may ignore the clamp. The device path still forces 1 when the flag is absent. The live query above found the flag set. Both statements stay on record.
* Vulkan sets the flag from `sampler_anisotropy`. Metal's sampler path comments that anisotropy is supported up to 16. No device maximum other than that interface cap was measured.
* A backend that rejects a non-power-of-two clamp is not in the wgpu 30.0.1 validation that was read. It is not an engine rule. If one appears, this ADR is reopened with that backend named.

**Non-power-of-two mips.** The renderer reads `DownlevelFlags::NON_POWER_OF_TWO_MIPMAPPED_TEXTURES` once. If the flag is absent and either side is not a power of two, the base level is kept, mip generation is skipped, and one log line names the asset. A later draw of that name does not log again. The texture is not refused. A format that cannot be linearly filtered and written as a render attachment takes the same one-level path and logs the format once. This flag is separate from anisotropy. M5-2C queried it on Apple M5 Metal and the flag was present, so that run generated non-power-of-two chains. The one-level decision is covered by a CPU test. It was not executed as a hardware fallback on this adapter.

**Two caches.** GPU image storage and samplers are separate. M5-2B implements that split.

Texture storage is identified by the asset id, the pixel bytes, the GPU format (the colour space), the allocated mip count, and the dimensions. A sampler change does not allocate a second copy of those pixels, does not upload them again, and does not rebuild a mip chain. A pixel change rewrites the base level and rebuilds the chain when one is allocated. A change of dimensions, format, or allocated mip count replaces the image. Mip generation runs on that create or pixel change, not on an unchanged revision.

`AssetManager` keeps one revision per asset. `insert` bumps it on every replace, including a replace whose pixels match, and a revision is never reused (ADR-0002). The revision tells the renderer to look again. It does not by itself mean the pixels changed. The renderer hashes the pixel bytes when the revision differs from the cached revision, and not on a frame whose revision is unchanged. This decision does not change `AssetManager`.

Samplers are cached by the effective GPU descriptor: min and mag filters, whether the mip filter is linear, both address modes, anisotropy after the device fallback, and the LOD clamp. Identical effective descriptors share one sampler. An entity does not get its own.

A bind group pairs one texture view with one sampler. Rebuilding it when the sampler changes does not copy the image.

Eviction of a texture stays "removed from the manager, or idle for N frames" (D-04, ADR-0002). A sampler stays while a cached image or a placeholder still uses it. Scene unload does not call `remove` (D-16).

**CPU region update (M5-3A, implemented).** `Texture::update_region(x, y, width, height, rgba)` replaces one texel rectangle. Coordinates are integers. `(0, 0)` is the top-left texel. `rgba` is the rectangle only, four bytes per texel, rows of `width * 4`. Destination rows stay `texture.width * 4` apart. The call does not change dimensions, colour space, sampler, mipmap preference, or asset identity, and it does not convert bytes between sRGB and linear.

Validation happens before any write. A failure returns `TextureRegionError` and leaves pixels and the edit record unchanged. Rejected inputs are a zero width, a zero height, a rectangle past either edge, `x + width` or `y + height` overflowing `u32`, a byte length that overflows, fewer bytes than the rectangle, more bytes than the rectangle, and a texture buffer shorter than its own dimensions claim. The error does not contain the asset name. The caller that has an `AssetManager` attaches `name_of`. `AssetHandle` stores the name hash, not the name.

**Dirty rectangles (M5-3A, implemented).** The record lives on the `Texture` value, so it survives the clone-and-`insert` path. There is no `AssetManager::get_mut`. A game edits a clone and inserts it under the same name. `insert` still bumps the revision on every replace, including a sampler-only replace and a replace whose pixels match (ADR-0002). A revision change is not itself a full pixel change.

`TexturePixelEdits::Clean` means this value has had no successful region update. Constructors, PNG decode, `with_sampler`, `with_filter`, `with_color_space`, and `with_mipmaps` stay `Clean`. Assigning the public `rgba` field does not update the record. `Journal` stores at most `MAX_TRACKED_REGIONS` (8) transitions, oldest first. Each transition is the fingerprint before the update, the fingerprint after it, and the rectangle. A ninth transition drops the oldest. The newest eight stay. Passing the cap does not force every later edit to a full upload.

`TexturePixelEdits::base_upload` is the rule for the GPU step.

1. No cached GPU image: full base upload.
2. The current `rgba_fingerprint` equals the cached fingerprint: pixels did not change. Do not upload and do not rebuild mips. This covers a sampler-only replace after the pixels were already uploaded, and it stops a second frame from replaying an update.
3. The newest contiguous chain whose first fingerprint is the GPU image and whose last fingerprint is the current bytes: upload those rectangles in order from the current bytes. Several edits before the next render are one chain. An edit on the next frame, after the GPU holds the previous fingerprint, is only the new transition. A GPU fingerprint in the middle of the journal selects the suffix. Overlapping and repeated rectangles stay in order. Disjoint rectangles stay separate.
4. Anything else is a full base upload. That includes a GPU image older than the retained window, a `Clean` value whose bytes differ (a whole-texture replace), a direct `rgba` write, and a chain that does not connect. The renderer does not skip a real pixel change, and it does not clear the CPU record. The `AssetManager` stores an `Arc`, so the renderer cannot edit it.

**GPU partial upload (M5-3B, not implemented).** The renderer still writes the whole base level on a pixel change. M5-3B uploads one base-level rectangle with `Queue::write_texture`: `origin` is `(x, y, 0)` at mip 0, `size` is the rectangle, and the source is tightly packed (`bytes_per_row = 4 * width`, `rows_per_image = height`). `write_texture` does not require 256-byte row alignment. The engine does not pad that source. A later `copy_buffer_to_texture` still respects `COPY_BYTES_PER_ROW_ALIGNMENT` when the copy is validated as aligned. The screenshot readback already pads for `copy_texture_to_buffer`.

Counters for that phase count base-level upload calls, the source bytes submitted to those calls, full uploads and partial uploads separately, mip-chain regeneration calls, and the texel size of each partial mip region. A full mip rebuild is not a partial mip update. The counters are logical sizes, not measured GPU memory bandwidth.

**Mip invalidation (CPU rule now, GPU rebuild later).** A successful region update sets `generated_mips_may_be_stale`. M5-3A does not build mip pixels on the CPU or the GPU. A sampler-only change does not set the flag. `affected_mip_rect` in `engine-renderer-wgpu` names the destination texels whose `mip.wgsl` footprint intersects the dirty source rectangle. Destination pixel `(x, y)` samples `((x + 0.5) / dest_width, (y + 0.5) / dest_height)`. The texel coordinate is `u * src_size - 0.5`. The linear sample reads `floor(p)` and, when the fraction is not zero, the next texel, then clamps. Even sizes land on a 2×2. Odd sizes do not: a 5-wide image reduced to 2 leaves the middle texel in neither footprint, and that level can stay untouched. The next level uses the previous level's affected rectangle, and a miss ends the chain. The rectangle is the tight bounds, not half of every side. Hardware may differ by about one destination texel at an odd edge. M5-3B may expand the rectangle by one texel or rebuild that level. Rebuilding the whole chain is a correct fallback. Untouched mip texels stay. A partial rebuild must match a full rebuild within the tolerance already used for mip readback.

**Device limits (CPU check now, GPU call later).** `validate_texture_dimensions` takes a width, a height, and injected maximums. It rejects the width first when both axes are past the limit. It does not allocate and it does not read a device. M5-3B calls it with `max_texture_dimension_2d` (8192 on wgpu 30.0.1's default limits) before `create_texture`. The error has no asset name. `name_of` supplies the name when an entry exists. When it does not, the log uses the id. A test must not allocate a texture large enough to trip the real device limit.

**Missing textures (current behaviour, naming gap).** Extraction logs a missing renderable once per id, up to 64 ids, and counts every miss in `missing_assets`. The log includes `name_of` when the manager has an entry. It otherwise prints the id. It does not invent a name. A sprite resolves the `SpriteAsset`. A missing texture inside a present sprite is drawn with the magenta placeholder and is not logged today. A tilemap resolves its texture. A mesh resolves the mesh asset. `Material.base_texture == None` draws the white placeholder. A `Some` handle with no GPU image also draws white, and that miss is not logged. Placeholders stay as they are. The smallest later correction is to report the texture id through the same once-per-id log, with `name_of` when it returns `Some`, without changing the placeholder and without storing a second name on `AssetHandle`. There is no atlas asset to miss. The packer is not implemented.

**Atlas and mips (later phase).** Not implemented. Shelf packing is deterministic, in texels, with padding and edge extrusion, no overlap, and UV rectangles inside the atlas. A zero-sized region is an error named with the asset. The packer does not set world size (ADR-0016).

A design estimate, checked against no GPU: padding of P texels, P > 0, suggests nearest mip sampling might stay inside a region through level `floor(log2(P))`, and linear mip sampling one level fewer, because each level halves and a linear mip sample can read two levels. The estimate does not reject a pack. It does not define the supported mip range. It is not a universal guarantee.

Whole-atlas mip generation has limits the estimate does not remove. Each coarser level halves the padding. A filter can reach a neighbour once the remaining padding is smaller than the kernel. Extruding a region's own edge colours does not keep regions apart at every level. A coarse level can mix regions that the base level separated. Pixel-art atlases that want nearest sampling and no mips remain the supported tight case. Sprite-sheet animation stays in M8.

The M5 atlas acceptance tests, when that phase is implemented, establish the supported range. They include adjacent regions of strongly contrasting colour, nearest and linear filtering, several mip levels, more than one padding width and more than one extrusion width, the UV boundaries the packer returns, and no neighbour sample inside the mip range the test declares supported.

## Consequences and follow-up

* M5-2 implements the descriptor, formats, mip generation, the split caches, and the GPU tests. It does not implement the packer, partial upload, or blended meshes.
* M5-2A adds the CPU types, the backward-compatible constructors, CPU validation, and unit tests. It does not generate mipmaps.
* M5-2B (`11b914f`) selects `Rgba8UnormSrgb` or `Rgba8Unorm` from `ColorSpace`, translates `SamplerDesc` into the wgpu sampler, and caches samplers separately from image storage. Its one-level LOD range is `0..32`.
* M5-2C (`622ef80`) generates the `Mipmaps::Generate` chain with the private blit above.
* M5-3A adds `Texture::update_region`, the dirty-rectangle record, `base_upload`, and `validate_texture_dimensions`. It does not upload a rectangle, regenerate a mip on the GPU, or pack an atlas. M5 stays in progress.
* M5-2 preserves the constructors, defaults, handles, and sprite and mesh rendering listed above. The four screenshots against `6dbabb8` stay byte-identical unless a separately reviewed correctness issue requires a change.
* M14 is the first consumer of `ColorSpace::Linear` for normals. The slot exists only as storage until then (ADR-0015).
* D-06 remains open.
* Not decided: texture compression, streaming, 3D textures, clamp-to-border, a live anisotropy maximum above the HAL cap of 16.

## Evidence and revisit conditions

Checked against `crates/engine-assets/src/texture.rs`, `crates/engine-assets/src/manager.rs` (one revision per replace), `crates/engine-renderer-wgpu/src/scene_renderer.rs` (`write_texture` with stride `4 * width`; readback pads for `copy_texture_to_buffer`), `crates/engine-renderer-wgpu/src/mip.rs`, `crates/engine-renderer-wgpu/src/gpu.rs`, and wgpu 30.0.1: `SamplerDescriptor`, `Device::create_sampler` (returns `Sampler`; HAL errors go to the device sink), `Queue::write_texture` (alignment not required; staging aligns internally), the aligned-copy check in `command/transfer.rs`, and `DownlevelFlags::NON_POWER_OF_TWO_MIPMAPPED_TEXTURES`. M5-2B and M5-2C queried Apple M5, Metal, `IntegratedGpu`. Anisotropic filtering and non-power-of-two mipmapped textures were both present. No live anisotropy maximum above 16 was read. The absent non-power-of-two flag was not the hardware path on this adapter.

Reopen if a required adapter cannot express the defaults, if a named backend rejects a non-power-of-two anisotropy clamp, if the downlevel flag and the device path are reconciled by a later wgpu, or if the atlas GPU tests disagree with the padding estimate.
