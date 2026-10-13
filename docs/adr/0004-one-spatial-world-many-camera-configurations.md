# ADR-0004: One spatial world, many camera configurations

* **Status:** Direction Settled (owner, 2026-10-13). Mechanism decisions 1 to 4 **Accepted with clarifications** or
  **Amended** (owner, 2026-10-13). The six open questions were answered by owner direction on 2026-10-13; the
  answers are classified under "Resolution of the open questions" as accepted principles, provisional conventions,
  deferred API decisions and questions that need prototype evidence. **Nothing in this ADR is implemented.**
  Mechanisms and APIs remain open where that section says so.
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
  orthographic settings; style stays in policies at the level each belongs to (decision 2).
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
  those are is not yet decided (see "Resolution of the open questions", 3, and D-48, D-31).

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
* The six open questions are resolved as classified above. Settled: the presets, style-policy, orientation
  and ordering principles, the `+Y` up and `XZ` ground convention (provisional), and the candidate orientation
  modes. Still open and owned by D-48, D-06, D-25 and D-26: preset signatures and defaults, style API ownership,
  the gameplay-orientation component, the coplanar mechanism, any isometric policy, `Upright` semantics.
* Decision 4 changes the wording that said ordering "stays spatial" and that no layer concept comes
  first. The existing `RenderOrder` rule (never positional, blended only) is unchanged until a mechanism is chosen.

## Resolution of the open questions

Owner direction of 2026-10-13 on the six questions this ADR left open. Four kinds of statement are kept apart:

* **Accepted principle**: settled; code and later designs must respect it.
* **Provisional convention**: the current engine behaviour, retained and documented; changing it needs a recorded
  reason, not a prototype.
* **Deferred API decision**: the principle is settled but the names, signatures, defaults or ownership are not,
  and they are decided when the owning design is written (the milestone shown).
* **Needs prototype evidence**: the mechanism is chosen only after the M6 test scene or another prototype
  shows the requirement.

| # | Question | Accepted principle | Provisional convention | Deferred API decision | Needs prototype evidence |
|---|---|---|---|---|---|
| 1 | Camera presets | Presets produce the same `Camera` and `Transform` types and stay fully configurable; projection type, camera orientation and gameplay configuration stay distinct | none | Constructor signatures, default isometric angles, custom-projection integration (M6 camera design, D-25) | none beyond the camera design |
| 2 | Pixel-art filtering and texel snapping | Sampling configuration lives at texture or sampler level where appropriate; pixel-art behaviour may need coordinated settings across materials, sprites, cameras and renderer; snapping is implemented at the level its technique needs; no universal style abstraction; presets may group settings without removing independent control | Today filtering is per `Texture`; there is no camera or renderer style setting | API ownership and defaults (D-06, D-05) | A pixel-art rendering prototype validating the requirements |
| 3 | Coordinate conventions and gameplay orientation | World transform, movement direction, gameplay-facing direction and visual orientation are distinct concepts; derive where meanings align, store only where behaviour requires | `+Y` up, `XZ` ground plane, left-handed, `+Z` forward (below) | The gameplay-orientation component design, until M7 movement and M8 directional-animation requirements are specified (D-48, D-31) | Which characters need independently stored facing (from M7 and M8 requirements) |
| 4 | Coplanar ordering | Spatial depth, transparency sorting, render layers or queues and explicit render order are distinct; an arbitrary order value never replaces spatial depth | The existing `RenderOrder` rule (blended sprites only) stands until a mechanism is chosen | Mechanism, naming, whether a layer or queue concept is needed (D-48) | **Yes.** The M6 test scene identifies the real coplanar and depth-conflict cases first |
| 5 | Isometric sorting and occlusion | No general-purpose occlusion framework is built speculatively; room is left for specialised policies | none | Any policy | **Yes.** Only after the M6 test scene shows a concrete limitation. A possible future need, recorded here and nowhere promised |
| 6 | Sprite orientation | Orientation design is independent of camera projection and extensible | `Billboard`, `Upright` and `Fixed` are the candidate initial modes, not an exhaustive or permanent set | `Upright` semantics for oblique and steep views, clarified during implementation (D-26) | Behaviour of each mode under the M6 steep, top-down and isometric configurations |

### 1. Camera presets

Settled in decision 1; this answer adds only what is deferred. The exact constructor signatures (free functions,
associated functions, a bundle), the default isometric angles (true isometric, 2:1 dimetric or another art
convention), and how a preset takes a custom projection are decided when the camera implementation is designed
(M6, with D-25), not now. The three things a preset must never merge stay separate: the **projection type**
(`Orthographic`, `Perspective`, later custom), the **camera orientation** (the entity's `Transform`), and the
**gameplay configuration** (decision 3).

### 2. Pixel-art filtering and texel snapping

Architectural guidance, extending decision 2:

* Filtering and sampling (nearest or linear, wrap, anisotropy, mipmaps) are properties of the texture or its
  sampler wherever the GPU model makes them so. They do not depend on a style setting.
* A pixel-art look can need several coordinated settings at different levels: nearest filtering (texture or
  sampler), a material or sprite choice about how a region is sampled and padded, a camera choice about aligning
  the view to the texel grid or scaling to integer factors, and a renderer choice about anti-aliasing. These are
  separate settings that happen to be used together.
* **Texel snapping belongs at the level of the technique chosen.** Snapping the view translation is a camera
  concern; snapping per sprite or in a shader is an object or renderer concern. Which technique, and so which
  level, is not chosen here.
* Avoid a universal "style" abstraction that couples unrelated settings. A **convenience preset** may configure
  several settings together (for example "pixel-art camera and sampler"), provided each setting can still be
  changed on its own afterwards.
* Exact API ownership and defaults wait for a rendering prototype that validates the requirements (for example a
  pixel-art scene under fractional camera movement, with and without rotation and perspective). D-06 stays Open and
  below the foundations other views need.

### 3. Coordinate conventions and gameplay orientation

**Current engine convention (Provisional).** Left-handed, `+Y` up, `+X` right, `+Z` forward, so the ground plane
is `XZ` and height is `Y` (the table in [../SPATIAL_RENDERING.md](../SPATIAL_RENDERING.md) is the specification).
Inspection of the implementation found no material incompatibility with any of the supported views, so it is
retained. What the code actually does with it today:

* The only place the renderer reads `+Y` as the world's up is `Upright` sprite extraction (`extract.rs`: the
  vertical axis and the turn axis). The math helpers take the up vector as an argument (`Transform::looking_at`,
  `Quat::look_rotation`), and a transform's local axes are its own. No component, resource or API stores an up
  axis or a ground plane, so none is configurable. The shaders contain no world-up assumption; the examples (for example
  `basic_2_5d`, which moves the player on `XZ`) and the tests do assume the convention.
* Sprites and tilemaps are authored in their local `XY` plane (a quad faces local `-Z`) and are laid on the ground
  by rotating the owning entity; a `TileMap` has no notion of "ground".
* The earlier draft's phrase "per-game configuration is intended" is **withdrawn**. A per-game up axis or ground
  plane is not part of this decision, is not provided, and would be a deliberate later decision touching at least
  `Upright`, the examples and any gameplay code that assumes the convention.

Implications, which are the reason it is recorded:

| Area | Consequence of `+Y` up and `XZ` ground |
|---|---|
| Transforms | Yaw is rotation about `+Y`, so "turn left or right" on the ground is yaw. Ground objects differ in `X` and `Z`; `Y` is height. Rotating an entity about `X` lays a local-`XY` quad or tilemap flat on the ground |
| Camera configuration | Default camera orientation looks along `+Z`, which is the side-scrolling view. Top-down looks along `-Y`. Isometric is an orthographic camera yawed about `Y` and pitched about `X`. A camera never redefines "up" for the world |
| Movement planes | A **movement plane is a gameplay constraint, not the ground plane.** Top-down and role-playing games move on `XZ`. A side-scroller moves in the vertical `XY` plane (or on `XZ` with locked depth) and gravity is `-Y`. The convention fixes up; which plane or axis a game constrains movement to is chosen by the game (M7 expresses it as a movement option) |
| Spatial queries | Queries (rays, overlaps, ground checks) are three-dimensional in world space. A "ground" query casts along `-Y`. A 2D-style game uses the same queries with a constrained axis; there is no separate 2D query world |

**Four concepts, kept conceptually distinct:**

| Concept | Meaning | Source today or later |
|---|---|---|
| **World transform** | Where an entity is and how it is oriented in world space | `Transform` and `GlobalTransform`. Exists |
| **Movement direction** | The direction the entity is travelling, from velocity or position change | Gameplay state. Not a transform property. Does not exist yet (M7) |
| **Gameplay-facing direction** | The way the entity is facing for gameplay (aiming, interacting, choosing a directional animation) | Derived from the world transform's heading or from movement direction where those match. Stored only where behaviour requires |
| **Visual orientation** | How the picture is presented: sprite orientation mode, mesh rotation of the visual subtree, camera-relative frame selection | Derived from the world transform, the orientation mode and the camera. The visual subtree never writes the gameplay root (D-28) |

Rules: do not require an entity to store four separate values. Derive a value wherever the semantics align and
store independent state only where behaviour requires it (a character that strafes, aims at something other than
its heading, or is drawn from a four-direction sprite sheet that has no rotation to read). The exact
gameplay-orientation component design is deferred until the M7 character-movement and M8 directional-animation
requirements are written down.

### 4. Coplanar ordering

The mechanism stays unresolved. The vocabulary is accepted (decision 4): spatial depth, transparency sorting,
render layers or queues, and explicit render order. Arbitrary order values are not a replacement for spatial
depth. The M6 test scene is the instrument: before any mechanism is chosen it must record the actual cases, for
example ground tiles under decals, flat sprites at equal depth, cutout against cutout, a blended sprite over
cutout ground, and a character's feet meeting the ground plane, and for each whether the depth test, the
blended sort or an explicit override resolves it. The candidates named earlier (a spatial offset helper, a
depth bias, extending `RenderOrder` to opaque and cutout content, a layer or queue) are options to evaluate, not
a shortlist.

### 5. Isometric sorting and occlusion

Specialised isometric sorting or occlusion policies are deferred until the M6 test scene demonstrates a concrete
limitation of depth plus explicit order. No general-purpose occlusion framework is built on speculation. The
potential need is recorded as a future consideration, and the constraint on current code stays: the point at
which an object's depth is measured and the rule that orders objects must not become the only possibility baked
into extraction.

### 6. Sprite orientation

`Billboard`, `Upright` and `Fixed` stay as the candidate initial orientation modes. They are not exhaustive or
permanent: `SpriteOrientation` is already `#[non_exhaustive]`, and further modes (D-26) can be added where a real
scene needs them. Orientation is already independent of projection in the code: extraction reads only the view
basis (right, up, forward) from the camera, not whether it is orthographic or perspective, and the design must stay
that way.

`Upright` needs its semantics clarified for oblique and steep views during implementation. Known today, from the
code: the quad is vertical and turns about world `+Y`, so a pitched camera foreshortens it (by the cosine of the
pitch); looking straight along `Y` has no heading, the code falls back to the camera's right axis, and the quad is
edge-on; behaviour under an oblique or custom projection is unspecified. Whether `Upright` should compensate for
foreshortening, whether steep views need another mode or a per-camera choice, and what an oblique projection means
for a vertical quad are questions for the M6 test scene and the implementation, not decided here.

## Evidence and revisit conditions

Evidence the open items wait for, all from the M6 test scene unless stated: coplanar and depth-conflict cases
(question 4); a concrete isometric sorting or occlusion failure (5); mode behaviour under steep, top-down and
isometric views (6); a pixel-art prototype (2); M7 and M8 character requirements (3). The provisional convention
(3) is revisited only if a supported view or the gameplay-orientation design shows a material incompatibility.

Evidence that exists: camera maths and sprite-orientation tests under a pitched camera
(`crates/engine-renderer/tests/camera.rs`), GPU tests with both projections, tilemap laid on the ground by
rotating its entity. Evidence missing: a rendered isometric, top-down or side view; any preset; any
implementation of decisions 2 to 4. Revisit the decisions when M6's configuration test scene exists, or if a view
cannot be expressed as a camera configuration without a second code path, or if a preset needs a
view-specific code path in extraction or the backend.
