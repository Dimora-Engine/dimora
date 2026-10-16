# ADR-0016: Sprite world size stays per sprite

* **Status:** Settled
* **Date:** 2026-10-16
* **Backlog id:** D-27
* **Decided by:** project owner (M5-1 planning approval, confirmed in the M5-1 final review)
* **Affects:** `SpriteAsset`, the M5 atlas packer, M8 sheets, M13 tilemaps. Does not change sizing code.

## Context and problem

World units are unitless. `SpriteAsset::pixels_per_unit` converts that sprite's pixel size into a world size. The default is `SpriteAsset::DEFAULT_PIXELS_PER_UNIT` (16). `Sprite.size`, when set, replaces the natural size. There is no engine-wide pixels-per-unit constant. M5's shelf packer will work in texels. A global constant would fight pixel art, HD-2D art, and illustrated sprites that share one world with meshes.

## Requirements and constraints

* Pixel art, HD-2D artwork, high-resolution illustrated sprites, cutout characters, and sprites next to 3D meshes must be able to size themselves differently.
* The same sprite size must not depend on perspective versus orthographic projection (ADR-0004).
* The current default of 16 stays, so existing sprites do not change size.
* The atlas packer must not write an object's world size.
* D-06 (texel snapping, integer scale) stays deferred to M6 and is not a unit system.

## Options considered

### A. One engine-wide pixels-per-unit constant
* Advantages: one number to document.
* Disadvantages: every sprite and tileset inherits it. Mixed art scales have to fight the constant. It bakes a style into the engine.

### B. Keep the per-sprite value
* Advantages: already implemented. `with_pixels_per_unit` and `Sprite.size` cover the cases above. The packer can ignore world units.
* Disadvantages: two sprites can disagree, and a game that wants one scale must set it on each asset.

### C. A per-tileset value and no sprite value
* Advantages: tilemaps get a local scale.
* Disadvantages: removes the field sprites already use. Tilemap metadata belongs to M13.

## Decision and rationale

Option B.

Texture dimensions are texels. A sprite's world size is `Sprite.size` when set, otherwise `size_px / pixels_per_unit`. The default remains 16. M5 does not add a mandatory global constant. The shelf packer places rectangles in texel space and returns UV rectangles. It does not read or write `pixels_per_unit`.

A game that wants one scale sets the same value on each `SpriteAsset`, or sets `Sprite.size`. That is application data, not an engine mode. Camera projection does not rescale the sprite.

## Consequences and follow-up

* M8 sheets and M13 tilesets reuse this field or an explicit world size. They do not introduce a second unit system.
* M5-2 texture parameters do not change sprite sizing.
* Not decided: texel snapping and integer camera scale (D-06).

## Evidence and revisit conditions

`SpriteAsset::DEFAULT_PIXELS_PER_UNIT` is 16 in `crates/engine-assets/src/sprite.rs`. This ADR does not change that constant and adds no test. The M5-1 final review confirmed the per-sprite policy, the default of 16, and that the atlas packer works in texels and does not set world size. No engine-wide pixels-per-unit constant is introduced.

Reopen only under the rule at the bottom of DECISIONS.md, with a concrete case the per-sprite value cannot express.
