# ADR-0017: Texture colour space, samplers, partial updates, and atlas mip limits

* **Status:** Settled as the M5 contract. Not implemented. M5-2 is the first code. M5-2A is CPU descriptors only and is specified in TASKS.md.
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

**Anisotropy, normalized before a sampler exists.** The CPU descriptor is validated and normalized, and the GPU sampler is created once from that result.

* Anisotropy below 1 is invalid. wgpu returns `CreateSamplerError::InvalidAnisotropy`. The engine returns that as a configuration error. It does not raise 0 to 1.
* Anisotropy above 1 requires linear magnification, linear minification, and linear mip filtering. If any of those is not linear, the normalized descriptor sets anisotropy to 1 and keeps the requested filters. Nearest-neighbour filtering stays nearest.
* Anisotropy above 16 is clamped to 16 when the filters are linear. That cap is the wgpu-hal interface range. wgpu-core applies `anisotropy_clamp.min(16)` when `DownlevelFlags::ANISOTROPIC_FILTERING` is set. It is not a power-of-two sequence and not a maximum read from a live adapter.
* The sampler is created once. A failed `create_sampler` after that normalization is a device error. wgpu reports it through `handle_hal_error_with_nonfatal_oom`. The engine surfaces it as the existing device error. It does not retry the create at anisotropy 1, and it does not treat the failure as a recoverable asset-load miss.

**Downlevel.** If `DownlevelFlags::ANISOTROPIC_FILTERING` is absent, wgpu-core sets the HAL clamp to 1 and still creates the sampler. The engine applies that normalization itself when it reads the flag: anisotropy becomes 1, the filters stay, one log line is written, and the sampler is created once. The GPU phase does this. M5-2A has no device, so it does not.

**Capability uncertainties.**

* The flag's comment in wgpu-types 30.0.1 says every backend and device supports anisotropic filtering and that an implementation may ignore the clamp. The device path still forces 1 when the flag is absent. This review did not query a live adapter, so both statements stay on record.
* Vulkan sets the flag from `sampler_anisotropy`. Metal's sampler path comments that anisotropy is supported up to 16. No device maximum other than that interface cap was measured.
* A backend that rejects a non-power-of-two clamp is not in the wgpu 30.0.1 validation that was read. It is not an engine rule. If one appears, this ADR is reopened with that backend named.

**Non-power-of-two mips.** If the adapter lacks `DownlevelFlags::NON_POWER_OF_TWO_MIPMAPPED_TEXTURES` and the image is not power-of-two on both axes, upload mip count 1, sample the base level, and log once. Do not refuse the texture. This is a mipmap-size flag. It is separate from anisotropy.

**Two caches.** GPU image storage and samplers are separate.

Texture storage is identified by the image asset id, the pixel bytes, the GPU format (the colour space), the mip configuration, and the dimensions. A sampler change does not allocate a second copy of those pixels, and it does not upload them again.

`AssetManager` keeps one revision per asset. `insert` bumps it on every replace, including a replace whose pixels match, and a revision is never reused (ADR-0002). `Texture` will hold the sampler beside the pixels, so a descriptor-only replace still bumps the revision. The revision tells the renderer to look again. It does not by itself mean the pixels changed. Before uploading, the renderer compares the storage fields, or a hash of the pixel bytes. This decision does not change `AssetManager`.

Samplers are cached by the normalized descriptor.

A bind group pairs one texture view with one sampler. It may be cached by that pair. Creating it does not copy the image.

Eviction of a texture stays "removed from the manager, or idle for N frames" (ADR-0002). Scene unload does not call `remove`.

**Partial update (later phase, contract now).** Not implemented in M5-1 or M5-2. The transfer uses the alignment of the operation it calls.

`Queue::write_texture` is the upload used today. The caller passes the rectangle's tight texel stride. wgpu aligns the staging copy. The engine does not pad those rows to 256 bytes.

A later path that calls `copy_buffer_to_texture` follows that call's rule: `bytes_per_row` respects `COPY_BYTES_PER_ROW_ALIGNMENT` when the copy is validated as aligned. The screenshot readback already does this for `copy_texture_to_buffer`. That requirement stays on the buffer-copy path.

One rectangle, when the later phase exists:

1. CPU storage changes only those pixels.
2. The base-level transfer covers that rectangle. On `write_texture`, the row stride is the tight texel stride of the rectangle.
3. Mip levels that the rectangle affects are rebuilt for that region. That work is required. It is separate from uploading the rest of the base image.
4. Pixels outside the rectangle stay. One rectangle is enough. Merging many dirty rects is not an M5 requirement. The M10 glyph atlas is the consumer.

**Atlas and mips (later phase).** Not implemented. Shelf packing is deterministic, in texels, with padding and edge extrusion, no overlap, and UV rectangles inside the atlas. A zero-sized region is an error named with the asset. The packer does not set world size (ADR-0016).

A design estimate, checked against no GPU: padding of P texels, P > 0, suggests nearest mip sampling might stay inside a region through level `floor(log2(P))`, and linear mip sampling one level fewer, because each level halves and a linear mip sample can read two levels. The estimate does not reject a pack. It does not define the supported mip range. It is not a universal guarantee.

Whole-atlas mip generation has limits the estimate does not remove. Each coarser level halves the padding. A filter can reach a neighbour once the remaining padding is smaller than the kernel. Extruding a region's own edge colours does not keep regions apart at every level. A coarse level can mix regions that the base level separated. Pixel-art atlases that want nearest sampling and no mips remain the supported tight case. Sprite-sheet animation stays in M8.

The M5 atlas acceptance tests, when that phase is implemented, establish the supported range. They include adjacent regions of strongly contrasting colour, nearest and linear filtering, several mip levels, more than one padding width and more than one extrusion width, the UV boundaries the packer returns, and no neighbour sample inside the mip range the test declares supported.

## Consequences and follow-up

* M5-2 implements the descriptor, formats, mip generation, the split caches, and the GPU tests. It does not implement the packer, partial upload, or blended meshes.
* M5-2A, specified in TASKS.md, adds the CPU types, the backward-compatible constructors, CPU validation, and unit tests. It does not create a GPU sampler and does not generate mipmaps.
* M5-2 preserves the constructors, defaults, handles, and sprite and mesh rendering listed above. The four screenshots against `6dbabb8` stay byte-identical unless a separately reviewed correctness issue requires a change.
* M14 is the first consumer of `ColorSpace::Linear` for normals. The slot exists only as storage until then (ADR-0015).
* D-06 remains open.
* Not decided: texture compression, streaming, 3D textures, clamp-to-border, a live anisotropy maximum above the HAL cap of 16.

## Evidence and revisit conditions

Checked against `crates/engine-assets/src/texture.rs`, `crates/engine-assets/src/manager.rs` (one revision per replace), `crates/engine-renderer-wgpu/src/scene_renderer.rs` (`write_texture` with stride `4 * width`; readback pads for `copy_texture_to_buffer`), `crates/engine-renderer-wgpu/src/gpu.rs`, and wgpu 30.0.1: `SamplerDescriptor`, `Device::create_sampler` in `device/resource.rs`, `Queue::write_texture` in `device/queue.rs` (alignment not required; staging aligns internally), and the aligned-copy check in `command/transfer.rs`. No GPU test was run for this ADR. No live adapter maximum was read.

Reopen if a required adapter cannot express the defaults, if a named backend rejects a non-power-of-two anisotropy clamp, if the downlevel flag and the device path are reconciled by a later wgpu, or if the atlas GPU tests disagree with the padding estimate.
