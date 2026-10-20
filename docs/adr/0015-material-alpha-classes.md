# ADR-0015: Material alpha classes and one blended sort

* **Status:** Settled. The three alpha classes, the shared blended sort key, the sprite depth point, and the blended-mesh representative depth are accepted. M5-5A (working tree, not committed, 2026-10-19) adds the CPU `MaterialAsset` and `AlphaMode`. Extraction, pipelines, blended meshes, and material batching are not implemented.
* **Date:** 2026-10-16. CPU asset note 2026-10-19.
* **Backlog id:** D-05
* **Decided by:** project owner (M5-1 planning approval, confirmed in the M5-1 final review)
* **Affects:** `MaterialAsset` (not yet implemented), extraction order, `RenderScene`, sprite and mesh pipelines, M5, M14, M15. Does not change today's renderer.

## Context and problem

`Material` is a component field: a linear `Color`, one optional albedo, and a lit flag. Meshes are always opaque. Sprites choose `SpriteBlend::Alpha` or `Cutout` on the sprite. Extraction puts meshes, cutout sprites, and blended sprites in separate lists (R-1, KI-6, KI-14). A blended mesh has nowhere to sort against a blended sprite.

ADR-0004 keeps spatial depth, transparency sorting, render layers or queues, and explicit order distinct. It does not choose a coplanar mechanism (D-48). Three coplanar conflicts are pinned by tests in `examples/spatial_scene/tests/configurations.rs` and described in `docs/SPATIAL_RENDERING.md`. This decision does not remove those conflicts.

M5-1 recorded the policy without changing the renderer. M5-5A adds the CPU asset only. Renderer and shader code are unchanged.

## Requirements and constraints

* One spatial world and one backend (ADR-0004). No isometric renderer and no second world.
* Alpha class is a property of the material, not of perspective versus orthographic projection, and not of primitive type.
* Camera position and orientation may change which object is nearer. That is spatial order, not a second policy.
* Opaque and mask content keep using the depth buffer. Blended content is sorted as whole objects.
* No alpha-to-coverage and no order-independent transparency in M5.
* Normal-map shading stays in M14. The normal slot may be stored earlier and must not be shaded.
* Scene membership does not own assets (ADR-0014). D-16 stays open.
* `Color` stays linear. Texture colour space is ADR-0017.

## Options considered

### A. Keep the split by primitive type
* Advantages: no extraction change; today's tests stay valid without a migration.
* Disadvantages: a blended mesh cannot interleave with a blended sprite. R-1 stays. M14 and M15 would invent another path.

### B. Three material classes, one blended sort
* Advantages: sprites, meshes, and later tiles and particles name the same class. Projection does not pick the class. The existing sprite sort key can be reused.
* Disadvantages: `RenderScene` must grow a blended list that is not "sprites only". Batching can no longer assume every mesh is opaque.

### C. Order-independent transparency or alpha-to-coverage
* Advantages: intersecting surfaces can look better.
* Disadvantages: a second technique, extra targets or samples, and it is outside M5.

## Decision and rationale

Option B.

**Classes.** Each material is one of:

| Class | Depth test | Depth write | Blending |
|---|---|---|---|
| Opaque | on | on | off |
| Mask | on | on | off; fragments below the material's alpha cutoff are discarded |
| Blend | on | off | on; the object is in the back-to-front sort |

Mask's conventional blend state is off. The cutoff is data on the material. The sprite cutout shader's current threshold is 0.5; a material that does not set a cutoff uses 0.5 so that behaviour can be reproduced. That default is a compatibility rule, not a new shader in this change.

**Sort.** Blended sprites and blended meshes share one list. They are not drawn as "all meshes, then all sprites". The key, already implemented for blended sprites in `extract.rs`, is:

1. `RenderOrder.layer` ascending.
2. Camera-space depth descending (larger view Z is farther; the view is left-handed, +Z deeper).
3. `RenderOrder.order` ascending.
4. A full tie keeps extraction order, which follows spawn order for a given world. The sort is stable.

A missing `RenderOrder` is `(0, 0)`, so depth decides. `layer` dominating depth is the existing blended-sprite rule (`render_order_layer_always_decides_between_overlapping_blended_sprites`). It is an explicit-order override inside the blended class. It is not a world z-index, and it does not apply to Opaque or Mask. ADR-0004's rule that an arbitrary order value does not replace spatial depth still holds for opaque and cutout visibility.

**Sprite depth point (implemented, unchanged).** The depth is the camera-space Z of the sprite quad centre.

**Mesh depth point (settled, not implemented).** The representative is the camera-space Z of the mesh's world-space bounding-sphere centre, the point `extract.rs` already uses for mesh culling. Blended meshes use the same sort key as blended sprites. This is an object-level approximation. It does not order intersecting transparent meshes, cyclic transparency, a large overlap whose centre misses the overlapping region, or a mesh that spans several depth ranges another object sits inside. Those cases stay outside M5. GPU tests are required in the phase that implements blended meshes. M5-1 and M5-2 do not implement them.

**What a camera may change.** Perspective and an orthographic camera yawed and pitched into an isometric view use the same class rules and the same key. They may produce different visible order because the view direction changes the depth values. Tests state the expected pixels for each camera. They do not require the two pictures to match.

**Object-level limits, accepted as limits.** Whole-object sorting does not correctly order intersecting transparent meshes, overlapping transparent surfaces whose centres disagree with the overlap, cyclic overlaps, or one object that spans a depth range another object sits inside. Those cases are not solved in M5. Cutout, or a later technique outside M5, is the escape. This is the same limit KI-14 already records for large blended sprites.

**Coplanar conflicts.** D-48 is unchanged. Equal-height cutout decals, a cutout decal coplanar with a mesh, and a blended decal at the same height as a cutout one stay known conflicts. This ADR does not extend `RenderOrder` to cutout or opaque content, and it does not add a depth bias.

**MaterialAsset.** A CPU asset in `engine-assets`, no `wgpu` type. Fields: linear `Color` base colour, optional albedo `AssetHandle<Texture>`, optional normal-map `AssetHandle<Texture>` that M5 does not sample, `AlphaMode`, lit flag, double-sided flag. `AlphaMode::Mask` stores an `AlphaCutoff` that must be finite and in `0.0..=1.0`. The default cutoff is `0.5`. `MaterialAsset::default` is linear white, no textures, `Opaque`, `lit: true`, `double_sided: false`, matching the inline mesh `Material` plus the opaque class meshes already use. It does not reproduce `SpriteBlend::Alpha`. GPU pipelines and bind groups stay in `engine-renderer-wgpu`. Sprites and meshes do not hold a material handle yet.

**Compatibility, so examples need not migrate in the texture phases.** A sprite or mesh with no material handle keeps today's fields: sprite tint, `SpriteBlend`, and `lit`; inline mesh `Material` as opaque colour, one texture, and `lit`. When a handle is set, the asset supplies alpha class, cutoff, lit, double-sided, base colour, and albedo. Sprite tint still multiplies. `SpriteBlend` is not consulted for that sprite, so the handle and the enum cannot disagree. Tilemaps stay on the cutout path, which is the Mask class with cutoff 0.5, until a later change gives them a handle. They do not get a second renderer.

**Batching.** Sharing a material is not enough to share a draw. A run may merge only when all of these match:

* pipeline (class, double-sided, and the shader variant already chosen for that class);
* material identity and the asset revision that supplied it;
* texture bindings and sampler descriptor;
* geometry (the same mesh, or the shared sprite quad);
* instance-data layout;
* a position in the submission order that does not cross a different key.

Opaque and Mask runs may be grouped by that key. Blend runs may merge only where the sorted order is already a contiguous run. Extraction must not reorder blended objects to enlarge a batch. One thousand sprites are one instanced draw when they share one material revision, one texture, one sampler, the sprite quad, one instance layout, and one class, and no other blended key is interleaved. That is true for a set of only those sprites whether they are Mask (grouped) or Blend (the sort never splits one key).

**Ownership.** Materials and textures may be shared by entities, scene instances, and application code. Unload does not remove them. No new reference count and no second `AssetManager`.

## CPU asset and the next checkpoint

M5-5A registers `MaterialAsset` through the existing `AssetManager`. A replacement bumps that store's revision. Texture handles are not resolved and are not reference-counted. A handle may name an asset that is not loaded. `validate` rejects a non-finite base colour and allows finite components outside `0.0..=1.0`.

**No handle, accepted and still the renderer.** A sprite keeps its tint, `SpriteBlend`, `lit` flag, and `SpriteAsset` texture. A mesh keeps the inline `Material` and stays opaque. A tilemap stays on the cutout path, Mask with cutoff 0.5, until a later change gives it a handle.

**Handle set, accepted, not implemented.** The asset supplies alpha class, cutoff, lit, double-sided, base colour, and albedo. Sprite tint still multiplies. `SpriteBlend` is not consulted, so the handle and the enum cannot disagree. Inline mesh `Material` is not consulted for that mesh.

**Clarifications that need approval before M5-5B implements them.**

* Albedo `None` on a sprite falls back to `SpriteAsset.texture`. Albedo `Some` replaces that texture binding. The sprite asset still supplies UVs, size, and pivot. A mesh with albedo `None` stays untextured, as `base_texture: None` does today.
* A material handle whose asset is missing skips the renderable and logs once, using `name_of` or the id. It does not fall back to the legacy fields and it does not invent a name. A present material whose albedo is missing keeps the existing missing-texture path: the draw uses the placeholder, and the log names the texture only when the manager has that name.
* The assigned sprite colour is the sample multiplied by `base_color` and then by `sprite.tint`, per channel, including alpha.
* `double_sided: false` on an assigned sprite selects back-face culling. Unassigned sprites stay uncullled. The default is `false` because meshes cull back faces. Assigning the default material would hide a sprite's back face.

**M5-5B handles.** Add `material: Option<AssetHandle<MaterialAsset>>` to `Sprite` and `Mesh`, default `None` in the existing constructors. Do not add it to `TileMap` in that checkpoint. Do not remove the inline `Material`.

**Revisions, not implemented.** Material identity is the `AssetId`. The revision is the asset-store revision. A base-colour, lit, or cutoff change updates instance or uniform data and does not allocate a pipeline, image, or sampler. A class or `double_sided` change selects a pipeline. An albedo id change rebinds through the existing texture cache. The normal handle is not bound. There is no material GPU cache in M5-5A, and M5-5B should not add a second texture cache.

**Batching, not implemented.** Sharing a `MaterialAsset` is not a batch. The current renderer merges consecutive sprite instances with the same texture and does not reorder them. Cutout sprites are grouped by texture. Blended sprites keep the sort, so they batch only when neighbours already share a texture. Meshes merge on `(mesh, texture)`. M5-5D adds material id, revision, pipeline class, and sampler to that key. Opaque and mask runs may be grouped. Blend runs merge only when the sorted order is already contiguous. One thousand sprites are one instanced draw when they share one material revision, one texture, one sampler, the sprite quad, one instance layout, and one class, and no other key is interleaved.

## Consequences and follow-up

* M5-5A stores `MaterialAsset`. M5-5B is the optional handle and the GPU binding, after the clarifications above. Blended meshes stay a later M5 slice. The 1,000-sprite measurement is M5-5D. M5-2 did not add the asset.
* The blended-mesh depth point is settled as the camera-space Z of the world-space bounding-sphere centre. Implementing it requires GPU tests of the approximation limits above.
* M14 samples the normal slot. M15 particles pick one of the three classes.
* Screenshot baselines against `6dbabb8` stay in force. This ADR does not move them.
* Not decided: PBR, material graphs, custom shaders, alpha-to-coverage, order-independent transparency, a coplanar mechanism (D-48), reverse-Z (D-07).

## Evidence and revisit conditions

Today's sprite key and quad-centre depth are in `extract.rs` and the `RenderOrder` tests. Mesh culling already transforms the bounding-sphere centre. The three coplanar tests still describe current pixels. The 2026-10-16 review accepted the mesh depth point and left the three coplanar conflicts with M6 and D-48. M5-5A adds CPU tests of `MaterialAsset` only. It does not change extraction or the renderer.

Reopen the settled class, sort key, or mesh depth point only under the rule at the bottom of DECISIONS.md. The phase that implements blended meshes adds the GPU tests named above before the approximation is treated as verified behaviour.
