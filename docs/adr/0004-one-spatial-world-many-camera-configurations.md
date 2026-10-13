# ADR-0004: One spatial world, many camera configurations

* **Status:** Direction Settled (owner, 2026-10-13). Mechanism decisions 1 to 3 **Accepted with clarifications** and
  decision 4 **Amended** (owner, 2026-10-13). Nothing in decisions 1 to 4 is implemented yet; the questions they
  leave open are listed under "Open questions" and are not decided
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

### Mechanism decisions (owner, 2026-10-13)

The engineering recommendations that followed the direction were answered one by one. 1 to 3 are accepted with
the clarifications the owner gave; 4 is amended. The wording below is the decision; where the first draft said
something different, the difference is stated.

| # | Decision | Owner response |
|---|---|---|
| 1 | Camera presets are constructors, not modes | **Accepted** |
| 2 | Style is policy, not architecture | **Accepted with clarification** |
| 3 | Gameplay orientation is data | **Accepted** |
| 4 | Spatial ordering for coplanar content | **Amended** |

**Test scene (still a recommendation).** The reusable camera-configuration test scene is `spatial_scene`: 2D
sprites at several depths, a simple mesh and a flat ground, viewed through named configurations by one code path.
Animated characters, lights, shadows and tilemaps join it as their systems land. It must not prescribe a visual
identity. This was part of the first draft, was not among the four items the owner answered, and stays
Recommended.

#### 1. Camera presets are constructors, not modes: accepted

* Provide constructors or presets for common configurations: orthographic, perspective, isometric, top-down and
  side-scrolling, where appropriate. A preset returns the same `Camera` component and a `Transform`; it uses
  the same camera model, extraction, `RenderScene` and wgpu backend as any hand-built camera. There is no
  isometric renderer, no top-down mode and no per-view code path.
* A preset supplies sensible defaults and nothing more. Position, orientation, projection parameters,
  viewport, clear colour, order and every other supported camera setting stay editable on the result.
  A preset is never a closed type that prevents that.
* Not every gameplay style is a projection type. Isometric is an orthographic camera with an appropriate
  orientation, so it is a preset over `Orthographic`, not a new projection. Top-down and side-scrolling are
  placements and orientations of an orthographic or perspective camera. A projection is added only when a view
  cannot be expressed by existing ones (oblique or custom: D-25).
* A preset configures the camera only. It does not set gameplay orientation (decision 3) or visual style
  (decision 2); a top-down preset does not by itself fix upright sprites being edge-on from above, which is an
  object-level orientation choice (D-48, D-26).

#### 2. Style is policy, not architecture: accepted with clarification

* Visual style and presentation behaviour are configurable, not hardcoded through the engine. Policies are
  supported at the level where each one logically belongs: **object** (for example sprite orientation:
  billboard, world-oriented or fixed), **material or texture** (for example filtering), **camera** (for
  example texel snapping and integer scaling, D-06), **renderer** (for example anti-aliasing) and, where it
  describes the scene, **scene** (fog, lighting).
* There is **no universal style abstraction** and no single "style" type that gathers unrelated settings.
  A policy lives next to the data it changes and is read where it takes effect. If one setting would need
  to be replicated at several levels to work, that is a sign it belongs at one of them. Do not add a level, a
  policy type or a registry until a real setting needs it.
* Pixel-art features and fog stay optional policies, never defaults or foundations (unchanged from the first draft).
* *Today:* sprite orientation is per `Sprite`; texture filtering is per `Texture` (`TextureFilter`); no
  camera-level or renderer-level style policy exists yet.

#### 3. Gameplay orientation is data: accepted

* Gameplay orientation (the up axis and ground plane that movement and collision use, and which way a
  character faces for gameplay) is kept conceptually separate from world transforms and from visual
  presentation.
* A character's gameplay direction is never derived **only** from the camera's orientation or from its sprite's
  orientation. It may correspond to the entity's world orientation or to the visual orientation when that
  is the right meaning, and need not be identical to them.
* No redundant state: if one value can be reliably derived from another it is derived, not stored twice. An
  independent value is stored only where the meaning or behaviour really differs (for example a character that
  moves one way and faces another, or a four-direction sprite that has no rotation to read). Which values
  those are is not yet decided (see Open questions and D-48, D-31).

#### 4. Spatial ordering for coplanar content: amended

First draft: "Coplanar content in flat views must have a defined ordering that stays spatial", with a spatial
offset helper and a depth bias to be tried before any layer concept. The owner's amendment keeps the
principle that spatial depth is primary but states that explicit ordering is legitimate and must be a
separate, named concept, not something smuggled in as fake depth.

* **Four concepts, kept distinct** in code, data and documentation:

  | Concept | Question it answers | Today |
  |---|---|---|
  | **Spatial depth** | What is in front of what in the world? From transforms, camera-space depth and the depth test | Implemented: depth buffer for meshes, tiles and cutout sprites |
  | **Transparency sorting** | In what order must blended things be drawn so blending composes? | Implemented for blended sprites only: back to front by view depth of the quad centre (KI-14) |
  | **Render layers or queues** | Which pass, group or camera does something belong to? | Not implemented as such. The pass split (opaque, cutout, blended) exists; nothing user-facing |
  | **Explicit render order** | What did the game deliberately decide, where depth cannot? | `RenderOrder { layer, order }` on blended sprites only |

* Spatial visibility is decided by transforms, camera-space depth and depth testing. An arbitrary sort value
  never replaces real spatial depth. A feature that can be expressed by position is expressed by position.
* Deliberate ordering for **coplanar content** (tiles, ground decals, flat sprites at equal depth) and other
  intentional overrides **is supported**. It is an explicit, documented override and is not presented as depth.
  Today `RenderOrder` orders blended sprites only, and coplanar cutout sprites and tiles z-fight (R-16); how
  an override reaches opaque and cutout content is an open question, not decided here.
* Leave room for configurable sorting or occlusion policies for isometric and other specialised styles
  where object-origin depth alone does not give the intended picture. The room is a constraint on new code,
  not a feature: the point used to measure an object's depth, and the rule that orders objects, must not
  become the only possibility baked into extraction. **No generalised occlusion framework is built now.**
  Further policies are added when an actual scene demonstrates the need.
* The foundation to establish is the distinctions above in the vocabulary and documentation of the renderer.
  This ADR does not change code.

## Consequences and follow-up

* M5, M6, M14 acceptance criteria are rewritten so that tests run under several camera configurations
  (ROADMAP.md). M7, M8 and M13 gain one criterion each.
* New risk R-16 and decision D-48; D-25 moves up in priority within M6; D-06 moves down.
* The roadmap still supports the multi-perspective architecture; where it does not yet *prove* it is stated in CAPABILITIES.md row 8.
* Does not choose a projection API shape (D-25), a render-target model (D-08) or any pixel-art policy (D-06).
* Decisions 1 to 4 are recorded in DECISIONS.md (OD-2 and D-48), ROADMAP.md (M6), ARCHITECTURE.md (items 2 and
  10, R-16), SPATIAL_RENDERING.md and CAPABILITIES.md. None is implemented: presets, per-level style policies,
  gameplay-orientation data and any coplanar-ordering mechanism are planned work in M6 to M8, not existing
  capabilities.
* Decision 4 changes the wording that said ordering "stays spatial" and that no layer concept comes
  first. The existing `RenderOrder` rule (never positional, blended only) is unchanged until a mechanism is chosen.

## Open questions

Not decided by this ADR. Each needs evidence (a scene, a test or a prototype) first and belongs to D-48
unless stated.

1. **Preset shape.** Free functions, `Camera` associated functions returning `(Camera, Transform)`, or a
   bundle; what each takes (extent, distance, angles); the default isometric angles (true isometric, 2:1
   dimetric or others); how a preset gets an oblique or custom projection once D-25 lands.
2. **Where each style policy sits.** Which level owns pixel-art filtering and texel snapping in practice
   (texture, material or camera) and what stays at the renderer level (D-06, D-05).
3. **Gameplay orientation representation.** Which components exist, whether facing is stored or derived for
   each kind of character, how it feeds directional animation (D-31) and movement-plane constraints (M7), and
   the default up axis and ground plane (+Y and XZ today, as engine default; per-game configuration is
   intended but unspecified).
4. **Coplanar ordering mechanism.** A spatial offset helper, a depth bias, extending `RenderOrder` to
   opaque and cutout content, or something else; and whether a render layer or queue concept is needed at
   all beyond the pass split. Today `RenderOrder.layer` dominates depth for blended sprites, which is explicit
   order and not a render layer in the sense of the table above; whether it should be renamed is open.
5. **Isometric sorting and occlusion policy.** Whether depth plus explicit order is enough for isometric
   scenes, or a sort or occlusion policy is needed (for example a configurable depth sample point per object).
   Decide only after the M6 test scene shows a concrete failure.
6. **Sprite orientation per view.** Whether `Billboard`, `Upright` and `Fixed` are enough for steep views or
   another orientation is needed (D-26).

## Evidence and revisit conditions

Evidence that exists: camera maths and sprite-orientation tests under a pitched camera
(`crates/engine-renderer/tests/camera.rs`), GPU tests with both projections, tilemap laid on the ground by
rotating its entity. Evidence missing: a rendered isometric, top-down or side view; any preset; any
implementation of decisions 2 to 4. Revisit the decisions when M6's configuration test scene exists, or if a view
cannot be expressed as a camera configuration without a second code path, or if a preset needs a
view-specific code path in extraction or the backend.
