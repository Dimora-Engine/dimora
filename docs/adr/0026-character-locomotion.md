# ADR-0026: Character locomotion sits above the kinematic motor

* **Status:** Provisional
* **Date:** 2026-10-28
* **Backlog id:** D-50
* **Decided by:** project owner (approved the 1A foundation and the corrections in this record)
* **Affects:** M7 locomotion components, the fixed-step collision pass, later character animation (M8, M9)

Locomotion-1A, Locomotion-1B, and Locomotion-1C are implemented. 1A is the ground-support foundation. 1B is gravity, jump, fall, and landing. 1C is slope traversal and adhesion. 1D, horizontal acceleration, deceleration, running, and air control, is planned. 1E, the playable locomotion demonstration with integrated performance and verification, is planned. Stairs, autostep, moving platforms, coyote time, jump buffering, variable jump height, double jump, wall jump, and rigid-body physics stay deferred. They are not part of 1D or 1E.

## Context and problem

[ADR-0025](0025-kinematic-collision.md) gives each gameplay body one `CharacterMotor`. `MoveIntent.translation` is a world displacement for one fixed iteration. `MoveResult` records that resolution. It does not say the body is standing on a floor. `engine-world` does not depend on `engine-core`, so the world crate cannot read `Time`.

A character still needs a speed, a wish direction, and a support state. Putting those fields on `CharacterMotor` would make the solver own gameplay policy. A second collision index would disagree with the one the motor just used.

## Requirements and constraints

* `CharacterMotor` stays the displacement solver. Its skin, slide budget, locked axes, and rejection rules stay as ADR-0025 left them.
* One body has one movement authority for an iteration.
* Support uses the index `sync_collision` already rebuilt. No second broadphase, and no public Parry type.
* The fixed delta comes from the existing `Time` resource. `engine-world` does not gain an `engine-core` dependency or a second clock.
* A body with no locomotion component keeps today's `MoveIntent` behavior, including unrestricted XYZ.
* Simulation stays headless. A camera, a sprite child, and a mesh child are not inputs.
* 1B adds gravity along world −Y, one jump speed, and a landing event. 1C projects grounded motion onto a walkable support plane. Neither adds coyote time, jump buffering, variable height, double jump, autostep, or stairs.

## Options considered

### A. Fold speed, wish, and grounded into `CharacterMotor`
* Advantages: one component.
* Disadvantages: the solver starts owning gameplay policy, and a direct XYZ `MoveIntent` has nowhere to live beside it.

### B. A locomotion component that writes `MoveIntent` — chosen
* Advantages: the motor stays a displacement solver. Scripts can disable locomotion and write `MoveIntent` themselves.
* Disadvantages: two components can both be present, so the pass needs an ownership rule.

### C. Do nothing until a full character controller exists
* Advantages: no intermediate airborne body that does not fall.
* Disadvantages: ground detection, authority, and the fixed-delta seam stay unspecified while animation work wants a phase.

## Decision and rationale

Locomotion is a layer above the motor. 1A implements support and flat XZ motion. 1B implements gravity, jumping, falling, ceiling cancellation, and landing. 1C implements tangent-plane walking and adhesion along the support normal. Coyote time and jump buffering stay deferred.

### Components

`CharacterLocomotion` is persistent configuration: `enabled` (default true), `speed` (default 90 world units per second), `max_slope` (default π/4 radians), `stick` (default 0.08 world units), `gravity` (default −400 world units per second squared), `jump_speed` (default 200 world units per second), and `terminal_fall` (default 800 world units per second). On a walkable surface, `speed` is distance along that surface. In the air it is horizontal XZ speed. `stick` is adhesion toward the supporting surface, along the negated support normal. On flat ground that direction is world −Y. A non-finite or negative speed or stick contributes nothing. A `max_slope` outside `0..=π/2` accepts no support. `gravity` is acceleration along world Y. Up is +Y, so the default points down. A positive or non-finite gravity contributes nothing. A non-positive or non-finite `jump_speed` does not jump. `terminal_fall` is a downward speed limit as a positive magnitude. A non-positive or non-finite limit does not clamp. 1C adds no fields.

`CharacterMotion` is runtime state: `support`, `phase`, and `velocity`. `GroundSupport` is `Unknown` before the first positive fixed step, `Supported { entity, normal }` for one walkable solid, `Airborne` when the body is a usable solid and the probe found nothing, and `Unavailable` when the body has no solid indexed collider. `velocity.y` is the jump and fall speed in world units per second. Positive is up. It is the value takeoff and gravity write. It is not `MoveResult.applied / dt`. A grounded tangent may change Y without writing `velocity.y`. The stick is not stored here. `velocity.x` and `velocity.z` are reserved for 1D. 1C leaves them at zero and does not read them. Airborne horizontal motion is still the wish times `speed` times the fixed step.

`LocomotionPhase` is `Idle`, `Walk`, `Jump`, or `Fall`. `Jump` is airborne with `velocity.y > 0`. `Fall` is airborne after gravity was integrated this iteration, or after a ceiling stops the rise. `Walk` is committed traversal longer than `1e-3` that is not jumping or falling, including the step that leaves a ledge before gravity starts. On a walkable slope that length is the part of `MoveResult.applied` lying in the support plane, so adhesion and a recovery along the normal stay `Idle`. Walking uphill is not `Jump`. Walking downhill is not `Fall`. Airborne motion with no vertical velocity still uses the horizontal XZ length. A blocked wish with negligible committed traversal, and a deep overlap that is not support, stay `Idle`.

`LocomotionIntent.wish` is a world XZ direction for one iteration, not a displacement. Y is ignored. A non-finite X or Z is no direction. A horizontal length above 1 is normalized. A shorter direction is kept. `jump` is an edge for this iteration. It is not a held boost, and it is not remembered while the body is airborne. A fixed system copies `Actions::fixed_pressed` into the flag. The latch, not this component, survives a frame with no fixed iteration. `Game::step` clears leftover intents after the loop, including a loop of zero iterations.

`MoveIntent` and `MoveResult` stay the motor's API. An enabled `CharacterLocomotion` owns the displacement: a competing `MoveIntent` is removed and warned once per new conflict, not once per fixed step. The body does not fall back to that intent when its collider is disabled, missing, or invalid. Disabling locomotion, or omitting it, leaves `MoveIntent` in charge. Y is not locked.

### Fixed delta

`sync_collision(world)` is unchanged and does not advance locomotion. `sync_collision_with_step(world, step_secs)` runs the same propagate, rebuild, sweep, and triggers. `Some` of a finite value greater than zero also prepares and settles locomotion. `None`, zero, a negative value, and a non-finite value skip those two calls. The sealed `Game` system reads `Time::delta_secs()` and passes that value. A zero-iteration frame does not call the system. After the fixed loop, including zero iterations, leftover `LocomotionIntent` values are removed, the same way leftover `MoveIntent` values are.

### Support probe

The probe runs on the rebuilt index, before motors, and again at the committed pose, after motors. It does not rebuild the index. It walks the sorted entries. It skips the body itself, sensors, and pairs that fail the motor's mutual layer test. A free `CollisionQuery` is not used for that filter.

The gap is the closest-feature gap. The normal is that feature's wall, from the surface toward the body. A candidate is inside the window when `-(skin + 1e-4) <= gap <= skin + stick`, the normal is finite, `normal.y > 0`, and `normal.y + 1e-4 >= max(cos(max_slope), 0)`. `cos(π/2)` is slightly negative in `f32`. Clamping that cosine to zero would still admit an exactly vertical wall (`y == 0`) at a 90° limit, so `y == 0` is rejected on its own. Equality with the cosine is walkable, so a 45° face passes a 45° limit. The `1e-4` slack is about 0.008° near 45° and covers a few ulps between a rotated box and `cos`. It does not accept a 45.05° face at a 45° limit. A gap below that window is a deep penetration and is not support, even when it is the smallest number in the index. The nearest remaining gap wins. Gaps within `1e-5` keep the lower `Entity`.

The pre-move result decides adhesion. `Supported`, and not jumping or still rising in the takeoff lock, adds `stick` along the negated support normal. `Airborne` and `Unavailable` do not. The post-move result is what `CharacterMotion.support` stores. A rejection that leaves the pose unchanged does not invent a move; the post-move probe can still report the floor.

`Unknown` becomes `Supported` or `Airborne` on the first positive step. Spawning already inside the support window is that classification, not a landing. A gap below the window is not open air. Prepare does not integrate and does not submit a displacement for that body, including when the nearest faces cancel and the feature has no wall. A real walkable candidate elsewhere still wins.

### Gravity, jump, and landing (1B)

Integration is semi-implicit Euler on the fixed step. Read `velocity.y`, add `gravity * dt`, clamp a downward speed to `terminal_fall` when that limit is configured, then the vertical displacement is the resulting `velocity.y * dt`. One `MoveIntent` carries that displacement plus the horizontal wish. The motor resolves it. Settle then corrects `velocity.y` from this iteration's committed result and the post-move probe.

Gravity runs when the pre-move probe is already `Airborne`, or when the takeoff lock is set. A supported body that is not locked sets `velocity.y` to 0 and does not accumulate gravity. The stick is not written into `velocity`. The iteration that walks off a floor still has a supported pre-move probe, so it does not fall. The next iteration does. Removing the floor makes that next probe `Airborne`, so gravity starts on the removal step.

A jump is eligible only when this iteration's pre-move probe is `Supported`, the takeoff lock is clear, and `jump_speed` is finite and positive. Takeoff assigns `velocity.y = jump_speed` and skips gravity for that iteration. A body that spawns on a valid floor can jump on the first positive step. An airborne body, a sensor, a wall, and a ceiling cannot. There is no coyote time, no jump buffer, no double jump, and no variable height. The fixed-step action edge fires once per press. Later iterations of the same frame do not see it. A press while airborne is consumed and does not take off.

The takeoff lock stops the same iteration's post-move probe from grounding a rise that is still inside the support window. It stays set only while `velocity.y` is upward, the move was not rejected, a ceiling did not block the rise, the body moved up, and the post-move probe is still `Supported`. Stored support is `Airborne` for that iteration. The lock drops once the body is geometrically airborne, the vertical speed is no longer upward, a ceiling blocks the rise, or the move is rejected. It does not leave the body suspended at the apex or against a ceiling.

A ceiling is read from this iteration's `MoveResult`, not an older one. The request's Y is upward, applied Y is short of that request, and a contact or recovery normal has `y < -0.5`. Those normals point back toward the mover. Settle sets the blocked upward speed to 0. The ceiling is not ground. Horizontal sliding stays with the motor. The next airborne iteration falls under the rule above.

`LocomotionEvent::Landed { entity }` is sent on the existing world queue when the pre-move probe was `Airborne`, or the stored support was already `Airborne`, and the post-move probe is `Supported`, and the move was not rejected. Spawning on a floor, staying on a floor, a sensor, a wall, a rejected move, and a zero-iteration frame do not send it. The takeoff iteration stores `Airborne` while the lock is suppressing the floor, so that iteration does not land. The event stays pending until the promote after the fixed loop, including when a later iteration of the same frame keeps walking. A fixed system does not see it during that loop.

Ground contact cancels downward speed. Ceiling contact cancels blocked upward speed. A wall does not cancel a fall. A partial displacement and a recovery push are not copied into `velocity`. A rejected move restores the velocity from before that iteration's integration, stores the fresh probe, sets phase `Idle`, drops the lock, and does not land. A permanent rejection does not accumulate gravity.

### Slope traversal and adhesion (1C)

When the pre-move probe is `Supported` and the body is not jumping or still in the takeoff lock, the horizontal step is replaced. For an XZ wish `w` and a unit support normal `n`, `tangent = w - n * dot(w, n)`. The tangent is normalized only when its length is finite and greater than `1e-6`. The displacement is that direction times `|w| * speed * dt`. A zero, non-finite, or downward normal contributes no tangent. On a flat normal `(0, 1, 0)` the result is the previous horizontal step. At 90 units per second and 60 Hz, a 30° plane with normal about `(-0.5, 0.8660254, 0)` asks for about `(1.299, 0.750, 0)` uphill and `(-1.299, -0.750, 0)` downhill before the sweep cancels the stick. Those lengths are 1.5, the along-surface step. A diagonal wish is normalized before that projection, so it does not go faster. A slope yawed about Y uses the same rule.

Adhesion is `-n * stick` in that same intent. The motor resolves the combined displacement in its existing sweep. No second pass and no correction loop were added. Standing on a 30° slope and on a 45° slope for 1,000 steps does not drift, does not store the stick as velocity, and stays `Idle`. Uphill motion gains Y. Downhill motion loses Y, stays on the surface, and does not accumulate gravity.

Takeoff stays world +Y. It does not follow the support normal and does not include adhesion. The takeoff lock still stops the post-move probe from grounding that rise. Landing on a walkable slope is the 1B rule: one `LocomotionEvent::Landed`, downward speed cancelled, then a stable rest. A steep face is not a landing.

A surface above `max_slope` is not ground. Gravity remains. The wish stays horizontal. Grounded adhesion is not applied. The motor may still slide that displacement along the face. On a 60° face, a `+X` wish plus the falling step can leave a positive applied Y while `velocity.y` stays negative and the phase stays `Fall`. That is the existing sweep. 1C does not add a steep-slide speed or a wall-climb.

The same sweep shows up when an uphill tangent meets a vertical wall. The wall removes the horizontal part and the leftover Y separates the body from the slope. Gravity returns it, so the body bounces instead of resting against the wall. The wall never becomes ground, the phase is never `Jump`, and the body does not climb. The motor was not changed.

The support window is still `skin + stick` (0.09 at the defaults). One default step is 1.5 units. On a sharp crest that step can continue the old tangent past the ridge and miss the far face. A shorter step stays inside the window: at speed 15, a 20° crest is climbed and then descended without leaving the ground. Stairs and autostep are not added to bridge the longer step.

### What remains deferred

1D is horizontal acceleration, deceleration, running, and air control. 1E is the playable locomotion demonstration, integrated performance, and verification. Stairs, autostep, moving and rotating platforms, coyote time, jump buffering, variable jump height, double jump, wall jump, and rigid-body physics are deferred. They are not silently assigned to 1D or 1E. Gravity stays world −Y. `spatial_scene --collision` still writes `MoveIntent` directly. The example IJKL bindings are unchanged.

## Consequences and follow-up

* Gameplay can read `CharacterMotion` and `LocomotionEvent::Landed` without reading the motor's contact list.
* A script that wants direct displacement disables locomotion for that body.
* The next locomotion task is 1D, horizontal acceleration, after the owner accepts 1C. It should fill `velocity.x` and `velocity.z`. It should not rename `velocity`.
* Revisit this record if a second character controller needs a different support query, or if the stick window should grow so a default step can cross a sharp crest.

## Evidence and revisit conditions

Headless tests cover the 1A support cases, the 1B fall, Euler step, terminal speed, grounded rest, ledge departure, jump, first-step jump, apex, full arc, ceiling, wall-while-falling, airborne wish, airborne jump rejection, sensor and mask, one-step landing, floor removal, rejected motion, invalid configuration, and authority. 1C adds the tangent numbers, uphill and downhill displacement, diagonal and yawed slopes, 1,000-step rest on 30° and 45°, the angle table through a vertical wall, flat-to-slope and slope-to-flat, a short-step crest, a slope ledge, a steep face that stays airborne, a jump and one landing on a 30° slope, a ceiling above a slope, two supports, a sensor, a mask miss, runtime `max_slope` and `enabled`, and a same-process slope replay. `Game::step` covers the 1B iteration and jump cases and, for 1C, a zero-iteration frame, two iterations against two frames, sprite and mesh children, cameras, and one slope jump landing. The counts and timings are in [../../TASKS.md](../../TASKS.md) and [../PERFORMANCE.md](../PERFORMANCE.md).

This stays provisional until 1D and 1E exist or the owner replaces this split. A measurement that the probe mis-labels a walkable contact, or a second implementation that cannot share this probe, reopens the support window. The ideal continuous apex of the default jump is 50 world units (`jump_speed² / (2 * 400)`). The semi-implicit sum peaks near 51.67. Tests accept that sum within `1e-2` and the continuous value within 2.
