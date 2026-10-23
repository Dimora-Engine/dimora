# ADR-0021: Sprite orientation is three render policies

* **Status:** Settled
* **Date:** 2026-10-23
* **Backlog id:** D-26. Gameplay-facing components stay with M7 and M8.
* **Decided by:** Cal, 2026-10-23, after the tests below. The three-policy model was approved before those tests. The tests confirmed the existing placement, so extraction was not changed.
* **Affects:** the meaning of `SpriteOrientation`. Extraction, shaders, and batching are unchanged. Not [ADR-0004](0004-one-spatial-world-many-camera-configurations.md), [ADR-0019](0019-reverse-z.md), or [ADR-0020](0020-coplanar-priority.md).

## Context and problem

`SpriteOrientation` already has three modes, resolved in `sprite_placement` (`crates/engine-renderer/src/extract.rs`). The quad is world geometry. The shared camera then projects it. Placement reads `GlobalTransform` and `CameraView`. It does not write `Transform`.

[ADR-0004](0004-one-spatial-world-many-camera-configurations.md) kept `Billboard`, `Upright`, and `Fixed` as a candidate set and left steep, top-down, and oblique `Upright` undecided. The M6 scene has since measured that behaviour (`docs/SPATIAL_RENDERING.md`, `examples/spatial_scene/tests/configurations.rs`):

* Orthographic height of `Upright` is `cos(pitch)` times a billboard.
* A straight-down view makes a vertical quad edge-on. `Billboard` still has area.
* A perspective view at about 60° leaves `Upright` roughly half as tall. Perspective, so not exactly `cos(pitch)`.
* An oblique custom matrix shears the picture. Placement still uses the camera transform's basis.

Those measurements match the code. This decision accepts them, and it adds no mode.

## Requirements and constraints

* One spatial world. Orientation is visual policy. Gameplay facing is never derived only from the camera or the sprite (ADR-0004).
* A camera change must not mutate the authoritative world transform. Two cameras in one frame, including an offscreen target, may orient the same entity differently.
* Hierarchy, depth, reverse-Z (ADR-0019), and coplanar priority (ADR-0020) stay as they are.
* `Upright` stays world-vertical. It does not tilt to stay readable, and it does not scale up to cancel foreshortening.
* Behaviour is a function of the current `GlobalTransform` and `CameraView`. No previous frame. No pop threshold.
* The public choice is a field an editor can show. The caller does not build a GPU matrix.
* Skeletal 2D is not implemented here. The rule below is the constraint M11 must meet.

## What the code does

`CameraView` takes `forward`, `up`, and `right` from the camera entity's transform. Scale on that entity is ignored. `CameraProjection::Custom` does not change that basis.

`sprite_placement` keeps the global translation. Scale is the column lengths. A negative determinant mirrors local X. Then:

| Mode | Axes | Entity rotation that survives |
|---|---|---|
| `Fixed` | The global matrix, unchanged | All of it. Local XY, facing local −Z |
| `Billboard` (default) | `(view.right, view.up, view.forward)` | Twist about the entity's own Z, then applied in the view plane. Pitch and yaw are dropped |
| `Upright` | `right = normalize(Y × view.forward)` when that cross product's length squared is greater than `1e-8`, otherwise `view.right`. `up = +Y`. `forward = right × Y` | The same Z twist, applied in the vertical plane |

Every `Billboard` in one camera pass shares that basis. The quad is parallel to the view plane. It is not aimed at the camera position, so perspective does not skew one sprite against another.

`Upright` yaws about world `+Y` toward the camera heading. Pitch foreshortens it. The `1e-8` test is a zero-vector guard, not a blend range. `|Y × forward|` is the sine of the angle between the view direction and world up. The guard trips only when that sine is about `1e-4` or smaller.

The sprite normal is `-cross` of the placed X and Y axes (`sprite.wgsl`). UVs come from the unit quad and the UV rect. Coplanar priority, when present, runs after `view_proj * world` and changes depth only. Orientation is not a batch key (`SpriteInstance::shares_batch`).

## Options considered

### A. Keep the three modes and specify the pole

* Advantages: matches extraction, the pitched-camera tests, and the M6 measurements. No shader, batch, or depth change. `Fixed` plus a transform already lays a quad in a plane, which is how tilemaps lie on `XZ`.
* Disadvantages: a world-vertical sprite is edge-on from straight above. Crossing exact vertical reverses its yaw. A readable overhead character must use `Billboard`.

### B. Add a ground-aligned mode

* Advantages: the name would say "floor" in an editor.
* Disadvantages: it is `Fixed` after a rotation about local X. A second enum value would not change the matrix. Tiles already use the entity matrix.

### C. Make `Upright` compensate foreshortening, or blend into `Billboard` near the pole

* Advantages: a standing sprite would stay large, and a top-down view would show it.
* Disadvantages: un-foreshortening changes the world size of a real vertical quad. A blend tilts `Upright`, which this decision forbids. Any cutoff that switches mode is a pop. A continuous tilt is a different policy, and `Billboard` already is that policy.

### D. Unwrap heading with the previous frame

* Advantages: a camera path through exact vertical could keep a continuous yaw.
* Disadvantages: the result depends on history. Two cameras, a seek, or a respawn would disagree. The underlying map from view direction to a vertical frame has no continuous value on every path through `±Y`.

### E. Per-bone billboards for a 2D skeleton

* Advantages: each part would face the camera with no extra component.
* Disadvantages: placement replaces each sprite's pitch and yaw and keeps only that sprite's Z twist. Joint positions would survive. The pose's out-of-plane rotations would not. Children do not inherit a parent's billboard frame today; they read `GlobalTransform`.

## Decision and rationale

Adopt option A. Do not add a mode.

`Fixed` is world-aligned. A ground decal, floor mark, or tile is `Fixed` (or a tilemap) on an entity rotated into that plane. The camera does not turn it.

`Upright` is a cylindrical billboard about world `+Y`. It yaws to `normalize(Y × view.forward)` and does not pitch. Orthographic foreshortening is `cos(pitch)`. Perspective foreshortening is whatever the shared projection does to that vertical quad. Exact alignment with `+Y` or `-Y` has no heading: the quad stays vertical, uses `view.right` (including when that camera is rolled), and is edge-on. That edge is the geometry of a vertical plane seen along world up, not a missing branch. The two limits on either side of that alignment are opposite headings. A path through it reverses yaw. A path that only approaches it spins faster as the sine shrinks, and it stays continuous. There is no blend and no memory of the previous frame. Away from the pole, camera roll does not change `view.forward`, so it does not change `Upright`. The sprite stays world-vertical and looks rolled in the viewport.

`Billboard` is the camera-facing policy, including steep and exact top-down. Its up is the camera's up, so an unrolled camera already keeps the sprite parallel to the projection of world up, and a rolled camera keeps it parallel to the screen. Gameplay `Transform` is unchanged. Use this when the sprite must stay readable from above.

No separate policy preserves both world-vertical and a face-on top-down image. Those constraints contradict. Projected uprightness under roll is `Billboard`. World uprightness is `Upright`.

Oblique and custom projections do not grow a rule. Shear changes pixels of the placed quad. It does not change the placement basis. `Upright` stays vertical in world space.

Degenerate inputs stay deterministic. A zero cross product uses `view.right`. A zero scale uses `f32::EPSILON` only inside the twist reconstruction; the placed axis length is still zero and the quad collapses. A camera basis that is itself degenerate is a camera-transform failure. The sprite function does not repair it and does not remember a previous heading.

Multiple cameras already call placement once per `CameraView`. That stays the multi-camera rule. There is no per-camera override component.

Skeletal 2D, when M11 builds it, has one requirement from this decision: apply visual orientation once, at the character's rendered visual root, so each bone keeps its local pose and attachments. Part sprites stay `Fixed` in bone space and inherit that one rendered root frame. Do not put `Billboard` or `Upright` on every bone. Per-bone billboards would drop each part's pitch and yaw and keep only its Z twist. Do not add an orientation-group type until a second character representation shows the same shape. Gameplay orientation stays on the simulation root and is a separate concept. The renderer must not write it. A child does not inherit a parent's rendered basis today: a `Fixed` child under a billboarded parent keeps the simulation `GlobalTransform`. That is tested, and this checkpoint does not change it. M11 implements the visual-root frame.

Lighting, materials, UVs, reverse-Z, and coplanar priority see the placed quad. Shadows, when built, should sample that same placed geometry. They do not get a second orientation path.

Batching is unchanged. The mode becomes the instance matrix before `shares_batch`. Sprites that share pipeline, material, and sampled image stay one run. Different modes do not split that run.

### Editor-facing descriptions

| Policy | What to show | Use |
|---|---|---|
| `Billboard` | Faces the screen, including from above. The entity does not turn when the camera moves. | Effects, markers, overhead characters and props that must stay readable |
| `Upright` | Stands vertically and turns to the camera's heading. A camera looking straight down sees it edge-on. | Standing characters and props when foreshortening should be real |
| `Fixed` | Uses the entity's rotation, including lying on the ground. The camera does not turn it. | Signs, decals, floor art |

Supported cameras are the one camera model: perspective, orthographic, isometric, steep, side, top-down, and custom. The mode does not branch on projection kind.

## Consequences and follow-up

* D-26 is settled. `sprite_placement` is unchanged. The tests below lock the math that was already there.
* M11 owns the visual-root frame. Until that frame exists, bone sprites that use `Billboard` or `Upright` are oriented one sprite at a time, and a `Fixed` child does not follow a parent's rendered basis. D-31 still owns directional frame selection. Gameplay-facing components stay deferred. A per-game up axis stays out of scope (ADR-0004).
* `SpriteOrientation` remains `#[non_exhaustive]` so a later measured need can add a variant. This decision is that no such need exists for ground alignment, pole blending, or screen-upright as a fourth name.
* The next M6 work is tilemap chunking. Debug draw, benchmarks, and D-06 stay in M6 and are not started here. Gameplay orientation still waits for M7 and M8.

## Evidence and revisit conditions

Apple M5, Metal, 2026-10-23. Extraction was not modified. `DIMORA_REQUIRE_GPU=1`.

* `crates/engine-renderer/tests/orientation.rs`: global rotation for `Fixed`; world up for `Upright`; both exact vertical views, repeated; opposite near-vertical headings with roll, each computed alone; camera yaw; camera roll; the exact-pole fallback to the rolled camera right; zero scale; parent translation and parent yaw; a `Fixed` child under a billboarded parent; ground art in `XZ`; a sheared custom matrix leaving the model unchanged; two cameras leaving `Transform` unchanged; three policies, one batch.
* `crates/engine-renderer/tests/camera.rs` and `tests/multi_camera.rs`: the earlier pitched-camera and per-camera billboard cases.
* `crates/engine-renderer-wgpu/tests/orientation.rs`: opposite near-overhead pixels follow `Y × forward`; three policies are one batch and three colours; a side viewport, an overhead viewport, and an offscreen `+X` camera orient one upright sprite independently; `CoplanarPriority` 7 does not paint a face on an edge-on upright quad.
* `crates/engine-renderer-wgpu/tests/coplanar.rs`: the higher priority wins for a top-down billboard, a front `Fixed` quad, a front `Upright` quad, a perspective `Upright` quad, and a side `Upright` quad, in addition to the views already listed in ADR-0020.
* `examples/spatial_scene/tests/configurations.rs`: cosine foreshortening, top-down collapse, steep perspective, billboard area in every configuration including top-down, and mesh occlusion both ways. The top-down edge is expected geometry.

Screenshots, release, 1280×720, 2026-10-23 4:14 PM America/Chicago. SHA-256 matches the reverse-Z references in ADR-0019 and ADR-0020. The `6dbabb8` hashes were not replaced. `basic_2_5d` `196faee1b7eb39d7d0f706b20b50597867c92b72e73189e507b7e43b8098f7f0` (746060 bytes). `basic_2_5d --ortho` `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f` (462130). `spatial_scene --camera perspective` `cad5f528af127de4e94c16ecbcedea1e238e0ba6e1b72187ddfc19e13b24182b` (137001). `spatial_scene --camera orthographic` `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde` (34046). Draw counts match the D-07 capture. `DIMORA_REQUIRE_GPU=1 scripts/check.sh` at 4:19 PM America/Chicago exited 0: 712 passed, 0 failed, 0 ignored.

Reopen if a real scene needs a quad that is neither the entity matrix, nor world-vertical yaw, nor the view plane. Reopen the M11 sentence if the first skeletal character cannot express its pose with one root policy and `Fixed` parts. Do not reopen in order to hide the top-down edge.
