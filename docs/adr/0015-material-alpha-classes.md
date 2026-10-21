# ADR-0015: Material alpha classes and one blended sort

* **Status:** Settled. The three alpha classes, the shared blended sort key, the sprite depth point, and the blended-mesh representative depth are accepted. M5-5A (`e0c29ef`) stores the CPU `MaterialAsset`. M5-5B (`3b3cd4a`) resolves handles and draws opaque, mask, and blended sprites plus opaque and mask meshes. M5-5C (`ab1b6f7`) draws blended meshes in that same sequence. M5-5D (working tree, 2026-10-20) batches by pipeline, material identity and revision, and texture. One thousand compatible opaque sprites are one instanced draw.
* **Date:** 2026-10-16. CPU asset 2026-10-19. Handle rendering 2026-10-20. Blended meshes 2026-10-20.
* **Backlog id:** D-05
* **Decided by:** project owner (M5-1 planning approval, confirmed in the M5-1 final review)
* **Affects:** `MaterialAsset`, extraction order, `RenderScene`, sprite and mesh pipelines, M5, M14, M15. Unassigned renderables keep the previous path.

## Context and problem

`Material` is a component field: a linear `Color`, one optional albedo, and a lit flag. Meshes are always opaque. Sprites choose `SpriteBlend::Alpha` or `Cutout` on the sprite. Extraction puts meshes, cutout sprites, and blended sprites in separate lists (R-1, KI-6, KI-14). A blended mesh has nowhere to sort against a blended sprite.

ADR-0004 keeps spatial depth, transparency sorting, render layers or queues, and explicit order distinct. It does not choose a coplanar mechanism (D-48). Three coplanar conflicts are pinned by tests in `examples/spatial_scene/tests/configurations.rs` and described in `docs/SPATIAL_RENDERING.md`. This decision does not remove those conflicts.

M5-1 recorded the policy without changing the renderer. M5-5A adds the CPU asset. M5-5B reads a handle during extraction and selects a pipeline. M5-5C puts blended meshes on the same sorted submission list as blended sprites. Unassigned sprites, meshes, and tilemaps are unchanged.

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

**Sort.** Blended sprites and blended meshes share one list. They are not drawn as "all meshes, then all sprites". The key is:

1. `RenderOrder.layer` ascending.
2. Camera-space depth descending (larger view Z is farther; the view is left-handed, +Z deeper).
3. `RenderOrder.order` ascending.
4. Visit order. Sprites are extracted before meshes, and each blended item takes the next sequence number. Equal sprite keys keep the order they had when the sort was stable over sprites alone. A sprite and a mesh that tie draw the sprite first. A per-type index is not the shared key.

A missing `RenderOrder` is `(0, 0)`, so depth decides. `layer` dominating depth is the existing blended-sprite rule (`render_order_layer_always_decides_between_overlapping_blended_sprites`). It is an explicit-order override inside the blended class. It is not a world z-index, and it does not apply to Opaque or Mask. ADR-0004's rule that an arbitrary order value does not replace spatial depth still holds for opaque and cutout visibility. The GPU walk follows this list. Switching a sprite pipeline for a mesh pipeline does not reorder it.

**Sprite depth point (implemented, unchanged).** The depth is the camera-space Z of the sprite quad centre.

**Mesh depth point (implemented).** The representative is the camera-space Z of the mesh's world-space bounding-sphere centre, the point `extract.rs` already transforms for mesh culling. Parent translation, rotation, and nonuniform scale are in that matrix. The local origin and the entity translation are not substitutes when they differ from the centre. Blended meshes use the same comparison direction as blended sprites. This is an object-level approximation. It does not order intersecting transparent meshes, cyclic transparency, a large overlap whose centre misses the overlapping region, a mesh that spans a depth range another object sits inside, or a transparent mesh passing through a sprite plane. Those limits are recorded, not solved. There is no order-independent transparency, per-triangle sort, depth peeling, or weighted blended transparency.

**What a camera may change.** Perspective and an orthographic camera yawed and pitched into an isometric view use the same class rules and the same key. They may produce different visible order because the view direction changes the depth values. Tests state the expected pixels for each camera. They do not require the two pictures to match.

**Object-level limits, accepted as limits.** Whole-object sorting does not correctly order intersecting transparent meshes, overlapping transparent surfaces whose centres disagree with the overlap, cyclic overlaps, or one object that spans a depth range another object sits inside. Those cases are not solved in M5. Cutout, or a later technique outside M5, is the escape. This is the same limit KI-14 already records for large blended sprites.

**Coplanar conflicts.** D-48 is unchanged. Equal-height cutout decals, a cutout decal coplanar with a mesh, and a blended decal at the same height as a cutout one stay known conflicts. This ADR does not extend `RenderOrder` to cutout or opaque content, and it does not add a depth bias.

**MaterialAsset.** A CPU asset in `engine-assets`, no `wgpu` type. Fields: linear `Color` base colour, optional albedo `AssetHandle<Texture>`, optional normal-map `AssetHandle<Texture>` that M5 does not sample, `AlphaMode`, lit flag, double-sided flag. `AlphaMode::Mask` stores an `AlphaCutoff` that must be finite and in `0.0..=1.0`. The default cutoff is `0.5`. `MaterialAsset::default` is linear white, no textures, `Opaque`, `lit: true`, `double_sided: false`, matching the inline mesh `Material` plus the opaque class meshes already use. It does not reproduce `SpriteBlend::Alpha`. GPU pipelines and bind groups stay in `engine-renderer-wgpu`. `Sprite.material` and `Mesh.material_asset` are optional handles. `TileMap` has none.

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

## What M5-5B renders

M5-5A registers `MaterialAsset` through the existing `AssetManager`. A replacement bumps that store's revision. Texture handles are not reference-counted. `validate` rejects a non-finite base colour and allows finite components outside `0.0..=1.0`.

**No handle.** A sprite keeps its tint, `SpriteBlend`, `lit` flag, and `SpriteAsset` texture, and it is not culled. A mesh keeps the inline `Material`, stays opaque, and keeps back-face culling. A tilemap stays on the cutout path, which discards below 0.5.

**Handle set.** The asset supplies alpha class, cutoff, lit, double-sided, base colour, and albedo. Sprite tint still multiplies, including alpha. `SpriteBlend` and the inline mesh `Material` are not consulted for that renderable. The product is linear and is not clamped on the CPU. Albedo `Some` selects that texture. Albedo `None` uses `SpriteAsset.texture` for a sprite and leaves a mesh untextured. Sprite UVs, size, pivot, and pixels per unit stay on the sprite asset. An overriding albedo is sampled with those UVs and is not remapped onto another atlas. A missing material skips the renderable and logs once, using a known name or the id, and does not fall back to the legacy fields. A pending load does not log. A missing albedo uses the existing placeholder, magenta for a sprite and white for a mesh. `double_sided: false` culls back faces on both sprites and meshes. `true` disables that cull. Assigning `MaterialAsset::default` to a sprite therefore hides the back face and treats transparent texels as opaque. A sprite author who wants the back face sets `double_sided`. One who wants transparent texels uses Blend or Mask.

**Classes that draw.** Opaque and Mask sprites and meshes write depth and do not blend. Mask discards when the combined alpha is below the material cutoff. That comparison is `effective_alpha < cutoff`. It is not a hardcoded 0.5 for an assigned material. Legacy cutout still uses 0.5. Blend sprites and blend meshes share `RenderScene.transparent`. The mesh blend fragment keeps sampled alpha. The legacy mesh fragment still writes alpha 1. Lighting shades rgb and does not change alpha. The blend is straight alpha, the same state blended sprites already use.

**Revisions.** Material identity is the `AssetId`. The revision is the asset-store revision. Both are copied onto the instance and participate in the batch key. A base-colour, lit, cutoff, class, or culling change is instance data or a pipeline selection. It does not allocate a texture image. An albedo change rebinds through the existing texture cache. The normal handle is not bound. There is no material GPU cache.

**Batching.** A sprite run merges when the pipeline, material id, material revision, and texture handle match. The pipeline is the class plus `double_sided`. Tint, UV, the model, and `lit` stay on the instance. The cutoff stored beside them is a copy of that material's single `AlphaCutoff`. `Sprite` and `Mesh` have no cutoff field, so two renderables that share one material revision discard at the same threshold. A different cutoff is a different material, and that identity already splits the draw. The texture handle is the sampled view and the sampler. An unassigned sprite or mesh has no material id and is not given one. Opaque and mask sprites are grouped by that key. Depth-writing meshes use the same material pair plus the mesh asset and `MeshDraw`. Blended items merge only when they are already neighbours in the sorted sequence and share a kind and that key. A mesh between two sprites splits those sprites. Extraction does not reorder blended objects to enlarge a batch. One thousand opaque sprites that share one material revision, one texture, one sampler, the sprite quad, and one pipeline were one instanced draw: 1,000 instances submitted, one `draw`. A second frame did not allocate another image, sampler, bind group, or instance buffer. Two overlapping blended sprites in one draw matched the same pair submitted as two ordered draws, so contiguous blend runs stay merged.

## Consequences and follow-up

* M5-5A (`e0c29ef`) stores `MaterialAsset`. M5-5B (`3b3cd4a`) binds it for sprites and for opaque and mask meshes. M5-5C (`ab1b6f7`) draws blended meshes in the shared sequence. M5-5D, in the working tree, is the material batch key and the 1,000-sprite draw. Mesh generators remain.
* The blended-mesh depth point is the camera-space Z of the world-space bounding-sphere centre. GPU readback covers ordinary non-intersecting order. The approximation limits above stay limits.
* M14 samples the normal slot. M15 particles pick one of the three classes.
* Screenshot baselines against `6dbabb8` stay in force. This ADR does not move them.
* Not decided: PBR, material graphs, custom shaders, alpha-to-coverage, order-independent transparency, a coplanar mechanism (D-48), reverse-Z (D-07).

## Evidence and revisit conditions

Today's sprite key and quad-centre depth are in `extract.rs` and the `RenderOrder` tests. Mesh culling transforms the bounding-sphere centre, and blended meshes reuse that point. The three coplanar tests still describe current pixels. The 2026-10-16 review accepted the mesh depth point and left the three coplanar conflicts with M6 and D-48. M5-5A (`e0c29ef`) adds the CPU asset. M5-5B (`3b3cd4a`) reads handles in extraction. M5-5C (`ab1b6f7`) submits blended meshes in the same GPU order as blended sprites. M5-5D batches by that material key without reordering the blend list. It does not change the coplanar tests.

Reopen the settled class, sort key, or mesh depth point only under the rule at the bottom of DECISIONS.md.
