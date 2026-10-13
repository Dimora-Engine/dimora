# ADR-0004: One spatial world, many camera configurations

* **Status:** Settled as a direction (owner, 2026-10-13); the mechanisms are Provisional and listed below
* **Date:** 2026-10-13
* **Backlog id:** D-25, D-08, D-06, D-48 (new); owner decision OD-2 in [../DECISIONS.md](../DECISIONS.md)
* **Decided by:** project owner (direction); engineering recommendations are marked as such
* **Affects:** `Camera` and `Projection`, `RenderScene`, sprite orientation, tilemaps, lighting and shadows (M14), directional animation (M8), collision (M7), examples, the roadmap for M5, M6, M14

## Context and problem

The earlier plan implied one showcase, a perspective, HD-2D-style look (pixel sprites, fog, lights). Dimora's
defining idea is not a look but a unified spatial world in which sprites, tilemaps, 2D rigs, meshes and 3D
characters coexist under coherent transforms and compatible rendering. Games built on it should be able to
choose orthographic, perspective, isometric, top-down, side-scrolling or custom views without a different
renderer or world model for each.

## Requirements and constraints

* One world representation, one `Transform` model, one depth model, one resource model for every view.
* Camera projection stays distinct from gameplay orientation (which way is "up", where the ground plane is) and
  from visual style (pixel snapping, fog, palette).
* The supported views are intended capabilities, not an obligation to build each in the next milestone.
* No prioritising HD-2D-specific features (pixel-art policy, fog) over foundations other views need.
* Do not add a separate renderer or incompatible world model per view.

## Options considered

### A. A mode or renderer per perspective (2D mode, isometric mode, 3D mode)
* Advantages: each mode can be tuned.
* Disadvantages: breaks the central principle; duplicates extraction, sorting and lighting; mixed scenes become special cases.

### B. One world; a camera is a transform plus a projection; views are configurations
* Advantages: matches the code (`Camera` is a component, its position and orientation come from the entity's
  `Transform`, `CameraView` is built from it); isometric, top-down and side views are camera placements and
  orthographic settings; style stays in per-camera or per-asset policies.
* Disadvantages: some 2D conveniences need explicit design (below); the closed projection enum must open (KI-11).

### C. Choose one flagship view and add others later
* Advantages: smallest first showcase.
* Disadvantages: bakes in that view's assumptions (ordering, sprite orientation, light direction) that are costly to remove (R-16).

## Decision and rationale

B (owner direction). Views are configurations of the same world:

| View | How, with one world | Status today |
|---|---|---|
| Perspective | `CameraProjection::Perspective`, any placement | Working, tested |
| Orthographic | `CameraProjection::Orthographic`, any placement | Working, tested |
| Isometric | Orthographic camera yawed and pitched by its transform; oblique or custom axes through a custom projection (D-25) | Camera maths supports a rotated orthographic camera; **no isometric scene or test exists** |
| Top-down | Camera looking along the world's down axis, orthographic or perspective; ground content laid flat | **Untested**; upright sprites are edge-on from above, so top-down needs billboard or fixed orientation (R-16) |
| Side-scrolling | Camera looking along the depth axis, orthographic or perspective | The default camera orientation; **not demonstrated as a 2D-style game** |
| Custom | A `Projection` implementation held by `Camera` | Trait exists; `Camera` holds a closed enum (KI-11) |

**Recommended, not owner-approved** (Provisional until accepted; each has a backlog entry):

1. The reusable **camera-configuration test scene** is `spatial_scene`: 2D sprites at several depths, a simple
   mesh and a flat ground, viewed through named configurations by one code path. Animated characters, lights,
   shadows and tilemaps join it as their systems land. It must not prescribe a visual identity.
2. **Camera presets are constructors, not modes.** A preset returns a `Camera` plus a `Transform`; the engine has
   no "isometric renderer".
3. **Style is policy, not architecture.** Texel snapping and integer scaling are per-camera, opt-in (D-06);
   fog and lighting are scene properties. Neither is a default or a foundation.
4. **Gameplay orientation is configuration** (D-48): the ground plane and up axis used by movement, collision
   and directional animation are data, not read from the camera.
5. Coplanar content in flat views must have a defined ordering that stays spatial (D-48, R-16).

## Consequences and follow-up

* M5, M6, M14 acceptance criteria are rewritten so that tests run under several camera configurations
  (ROADMAP.md). M7, M8 and M13 gain one criterion each.
* New risk R-16 and decision D-48; D-25 moves up in priority within M6; D-06 moves down.
* The roadmap still supports the multi-perspective architecture; where it does not yet *prove* it is stated in CAPABILITIES.md row 8.
* Does not choose a projection API shape (D-25), a render-target model (D-08) or any pixel-art policy (D-06).

## Evidence and revisit conditions

Evidence that exists: camera maths and sprite-orientation tests under a pitched camera
(`crates/engine-renderer/tests/camera.rs`), GPU tests with both projections, tilemap laid on the ground by
rotating its entity. Evidence missing: a rendered isometric, top-down or side view. Revisit the provisional
items when M6's configuration test scene exists, or if a view cannot be expressed as a camera
configuration without a second code path.
