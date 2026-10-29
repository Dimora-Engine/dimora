# ADR-0026: Character locomotion sits above the kinematic motor

* **Status:** Provisional
* **Date:** 2026-10-28
* **Backlog id:** D-50
* **Decided by:** project owner (approved the 1A foundation and the corrections in this record)
* **Affects:** M7 locomotion components, the fixed-step collision pass, later character animation (M8, M9)

Locomotion-1A and Locomotion-1B are implemented. Coyote time, jump buffering, variable jump height, double jump, slope projection, stairs, and moving platforms are not.

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
* 1B adds gravity along world −Y, one jump speed, and a landing event. It does not add coyote time, jump buffering, variable height, double jump, slope tangent projection, autostep, or stairs.

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

Locomotion is a layer above the motor. 1A implements support and flat XZ motion. 1B implements gravity, jumping, falling, ceiling cancellation, and landing. Coyote time, jump buffering, and slopes stay later checkpoints.

### Components

`CharacterLocomotion` is persistent configuration: `enabled` (default true), `speed` (default 90 world units per second), `max_slope` (default π/4 radians), `stick` (default 0.08 world units), `gravity` (default −400 world units per second squared), `jump_speed` (default 200 world units per second), and `terminal_fall` (default 800 world units per second). A non-finite or negative speed or stick contributes nothing. A `max_slope` outside `0..=π/2` accepts no support. `gravity` is acceleration along world Y. Up is +Y, so the default points down. A positive or non-finite gravity contributes nothing. A non-positive or non-finite `jump_speed` does not jump. `terminal_fall` is a downward speed limit as a positive magnitude. A non-positive or non-finite limit does not clamp.

`CharacterMotion` is runtime state: `support`, `phase`, and `velocity`. `GroundSupport` is `Unknown` before the first positive fixed step, `Supported { entity, normal }` for one walkable solid, `Airborne` when the body is a usable solid and the probe found nothing, and `Unavailable` when the body has no solid indexed collider. `velocity.y` is the jump and fall speed in world units per second. Positive is up. It is the value takeoff and gravity write. It is not `MoveResult.applied / dt`. `velocity.x` and `velocity.z` are reserved for a later acceleration checkpoint. 1B leaves them at zero and does not read them. Horizontal motion, on the ground and in the air, is still the wish times `speed` times the fixed step.

`LocomotionPhase` is `Idle`, `Walk`, `Jump`, or `Fall`. `Jump` is airborne with `velocity.y > 0`. `Fall` is airborne after gravity was integrated this iteration, or after a ceiling stops the rise. `Walk` is a committed horizontal displacement longer than `1e-3` that is not jumping or falling, including the step that leaves a ledge before gravity starts. A wish held into a wall, a vertical stick that the skin blocks, and a deep overlap that is not support stay `Idle`.

`LocomotionIntent.wish` is a world XZ direction for one iteration, not a displacement. Y is ignored. A non-finite X or Z is no direction. A horizontal length above 1 is normalized. A shorter direction is kept. `jump` is an edge for this iteration. It is not a held boost, and it is not remembered while the body is airborne. A fixed system copies `Actions::fixed_pressed` into the flag. The latch, not this component, survives a frame with no fixed iteration. `Game::step` clears leftover intents after the loop, including a loop of zero iterations.

`MoveIntent` and `MoveResult` stay the motor's API. An enabled `CharacterLocomotion` owns the displacement: a competing `MoveIntent` is removed and warned once per new conflict, not once per fixed step. The body does not fall back to that intent when its collider is disabled, missing, or invalid. Disabling locomotion, or omitting it, leaves `MoveIntent` in charge. Y is not locked.

### Fixed delta

`sync_collision(world)` is unchanged and does not advance locomotion. `sync_collision_with_step(world, step_secs)` runs the same propagate, rebuild, sweep, and triggers. `Some` of a finite value greater than zero also prepares and settles locomotion. `None`, zero, a negative value, and a non-finite value skip those two calls. The sealed `Game` system reads `Time::delta_secs()` and passes that value. A zero-iteration frame does not call the system. After the fixed loop, including zero iterations, leftover `LocomotionIntent` values are removed, the same way leftover `MoveIntent` values are.

### Support probe

The probe runs on the rebuilt index, before motors, and again at the committed pose, after motors. It does not rebuild the index. It walks the sorted entries. It skips the body itself, sensors, and pairs that fail the motor's mutual layer test. A free `CollisionQuery` is not used for that filter.

The gap is the closest-feature gap. The normal is that feature's wall, from the surface toward the body. A candidate is inside the window when `-(skin + 1e-4) <= gap <= skin + stick`, the normal is finite, and `normal.y >= cos(max_slope)`. A gap below that window is a deep penetration and is not support, even when it is the smallest number in the index. The nearest remaining gap wins. Gaps within `1e-5` keep the lower `Entity`.

The pre-move result decides the stick. `Supported` adds `stick` along world −Y. `Airborne` and `Unavailable` do not. The post-move result is what `CharacterMotion.support` stores. A rejection that leaves the pose unchanged does not invent a move; the post-move probe can still report the floor.

`Unknown` becomes `Supported` or `Airborne` on the first positive step. Spawning already inside the support window is that classification, not a landing. A gap below the window is not open air. Prepare does not integrate and does not submit a displacement for that body, including when the nearest faces cancel and the feature has no wall. A real walkable candidate elsewhere still wins.

### Gravity, jump, and landing (1B)

Integration is semi-implicit Euler on the fixed step. Read `velocity.y`, add `gravity * dt`, clamp a downward speed to `terminal_fall` when that limit is configured, then the vertical displacement is the resulting `velocity.y * dt`. One `MoveIntent` carries that displacement plus the horizontal wish. The motor resolves it. Settle then corrects `velocity.y` from this iteration's committed result and the post-move probe.

Gravity runs when the pre-move probe is already `Airborne`, or when the takeoff lock is set. A supported body that is not locked sets `velocity.y` to 0 and does not accumulate gravity. The stick is not written into `velocity`. The iteration that walks off a floor still has a supported pre-move probe, so it does not fall. The next iteration does. Removing the floor makes that next probe `Airborne`, so gravity starts on the removal step.

A jump is eligible only when this iteration's pre-move probe is `Supported`, the takeoff lock is clear, and `jump_speed` is finite and positive. Takeoff assigns `velocity.y = jump_speed` and skips gravity for that iteration. A body that spawns on a valid floor can jump on the first positive step. An airborne body, a sensor, a wall, and a ceiling cannot. There is no coyote time, no jump buffer, no double jump, and no variable height. The fixed-step action edge fires once per press. Later iterations of the same frame do not see it. A press while airborne is consumed and does not take off.

The takeoff lock stops the same iteration's post-move probe from grounding a rise that is still inside the support window. It stays set only while `velocity.y` is upward, the move was not rejected, a ceiling did not block the rise, the body moved up, and the post-move probe is still `Supported`. Stored support is `Airborne` for that iteration. The lock drops once the body is geometrically airborne, the vertical speed is no longer upward, a ceiling blocks the rise, or the move is rejected. It does not leave the body suspended at the apex or against a ceiling.

A ceiling is read from this iteration's `MoveResult`, not an older one. The request's Y is upward, applied Y is short of that request, and a contact or recovery normal has `y < -0.5`. Those normals point back toward the mover. Settle sets the blocked upward speed to 0. The ceiling is not ground. Horizontal sliding stays with the motor. The next airborne iteration falls under the rule above.

`LocomotionEvent::Landed { entity }` is sent on the existing world queue when the pre-move probe was `Airborne`, or the stored support was already `Airborne`, and the post-move probe is `Supported`, and the move was not rejected. Spawning on a floor, staying on a floor, a sensor, a wall, a rejected move, and a zero-iteration frame do not send it. The takeoff iteration stores `Airborne` while the lock is suppressing the floor, so that iteration does not land. The event stays pending until the promote after the fixed loop, including when a later iteration of the same frame keeps walking. A fixed system does not see it during that loop.

Ground contact cancels downward speed. Ceiling contact cancels blocked upward speed. A wall does not cancel a fall. A partial displacement and a recovery push are not copied into `velocity`. A rejected move restores the velocity from before that iteration's integration, stores the fresh probe, sets phase `Idle`, drops the lock, and does not land. A permanent rejection does not accumulate gravity.

### What remains deferred

1C owns slope projection and the slope limit as a movement rule. 1D owns stairs and step-up. 1E owns moving and rotating platforms. Coyote time, jump buffering, variable jump height, double jump, wall jump, horizontal acceleration, running, and an arbitrary gravity direction are not part of those checkpoints unless a later record adds them. `spatial_scene --collision` still writes `MoveIntent` directly. The example IJKL bindings are unchanged.

## Consequences and follow-up

* Gameplay can read `CharacterMotion` and `LocomotionEvent::Landed` without reading the motor's contact list.
* A script that wants direct displacement disables locomotion for that body.
* The next locomotion task is 1C, slope projection, after the owner accepts 1B. It should accelerate or project horizontal motion without renaming `velocity`.
* Revisit this record if a second character controller needs a different support query, or if the stick and the reach window fight a later slope rule.

## Evidence and revisit conditions

Headless tests cover the 1A support cases and the 1B fall, Euler step, terminal speed, grounded rest, ledge departure, jump, first-step jump, apex, full arc, ceiling, wall-while-falling, airborne wish, airborne jump rejection, sensor and mask, one-step landing, floor removal, rejected motion, invalid configuration, authority, and a same-process replay. `Game::step` covers one, two, and eight iterations, the eight-step cap, a zero-iteration frame that keeps the jump edge, a held jump, a landing that stays visible to `update` and `animate`, an in-frame apex, camera independence, and sprite and mesh children. The counts and timings are in [../../TASKS.md](../../TASKS.md) and [../PERFORMANCE.md](../PERFORMANCE.md).

This stays provisional until 1C through 1E exist or the owner replaces this split. A measurement that the downward probe mis-labels a walkable contact, or a second implementation that cannot share this probe, reopens the support window. The ideal continuous apex of the default jump is 50 world units (`jump_speed² / (2 * 400)`). The semi-implicit sum peaks near 51.67. Tests accept that sum within `1e-2` and the continuous value within 2.
