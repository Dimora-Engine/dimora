# ADR-0026: Character locomotion sits above the kinematic motor

* **Status:** Provisional
* **Date:** 2026-10-28
* **Backlog id:** D-50
* **Decided by:** project owner (approved the 1A foundation and the corrections in this record)
* **Affects:** M7 locomotion components, the fixed-step collision pass, later character animation (M8, M9)

Locomotion-1A is implemented. Gravity, jumping, slope projection, stairs, and moving platforms are not.

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
* This record does not add gravity, a jump, a coyote timer, slope tangent projection, autostep, or stairs.

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

Locomotion is a layer above the motor. 1A implements the part a later jump and a later walk animation can read. It does not implement those features.

### Components

`CharacterLocomotion` is persistent configuration: `enabled` (default true), `speed` (default 90 world units per second), `max_slope` (default π/4 radians), `stick` (default 0.08 world units). A non-finite or negative speed or stick contributes nothing. A `max_slope` outside `0..=π/2` accepts no support.

`CharacterMotion` is runtime state: `support` and `phase`. `GroundSupport` is `Unknown` before the first positive fixed step, `Supported { entity, normal }` for one walkable solid, `Airborne` when the body is a usable solid and the probe found nothing, and `Unavailable` when the body has no solid indexed collider. `LocomotionPhase` in 1A is `Idle` or `Walk`. `Walk` means this iteration's locomotion committed a horizontal displacement longer than `1e-3`. A wish held into a wall, and a vertical stick that the skin blocks, stay `Idle`.

`LocomotionIntent.wish` is a world XZ direction for one iteration, not a displacement. Y is ignored. A non-finite X or Z is no direction. A horizontal length above 1 is normalized. A shorter direction is kept.

`MoveIntent` and `MoveResult` stay the motor's API. An enabled `CharacterLocomotion` owns the displacement: a competing `MoveIntent` is removed and warned once per new conflict, not once per fixed step. The body does not fall back to that intent when its collider is disabled, missing, or invalid. Disabling locomotion, or omitting it, leaves `MoveIntent` in charge. Y is not locked.

### Fixed delta

`sync_collision(world)` is unchanged and does not advance locomotion. `sync_collision_with_step(world, step_secs)` runs the same propagate, rebuild, sweep, and triggers. `Some` of a finite value greater than zero also prepares and settles locomotion. `None`, zero, a negative value, and a non-finite value skip those two calls. The sealed `Game` system reads `Time::delta_secs()` and passes that value. A zero-iteration frame does not call the system. After the fixed loop, including zero iterations, leftover `LocomotionIntent` values are removed, the same way leftover `MoveIntent` values are.

### Support probe

The probe runs on the rebuilt index, before motors, and again at the committed pose, after motors. It does not rebuild the index. It walks the sorted entries. It skips the body itself, sensors, and pairs that fail the motor's mutual layer test. A free `CollisionQuery` is not used for that filter.

The gap is the closest-feature gap. The normal is that feature's wall, from the surface toward the body. A candidate is inside the window when `-(skin + 1e-4) <= gap <= skin + stick`, the normal is finite, and `normal.y >= cos(max_slope)`. A gap below that window is a deep penetration and is not support, even when it is the smallest number in the index. The nearest remaining gap wins. Gaps within `1e-5` keep the lower `Entity`.

The pre-move result decides the stick. `Supported` adds `stick` along world −Y. `Airborne` and `Unavailable` do not. The post-move result is what `CharacterMotion.support` stores. A rejection that leaves the pose unchanged does not invent a move; the post-move probe can still report the floor.

`Unknown` becomes `Supported` or `Airborne` on the first positive step. That transition is not a landing. `Supported` to `Airborne` is losing the floor. `Airborne` to `Supported` is gaining it, and 1A emits no landing event. There is no gravity, so a body that walks off a ledge stays at the height where it lost support.

### What 1A does not do

1B owns gravity, jump, coyote time, jump buffering, and landing. 1C owns slope projection and the slope limit as a movement rule. 1D owns stairs and step-up. 1E owns moving and rotating platforms. None of those are implemented. `spatial_scene --collision` still writes `MoveIntent` directly.

## Consequences and follow-up

* Gameplay can read `CharacterMotion` without reading the motor's contact list.
* A script that wants direct displacement disables locomotion for that body.
* The next locomotion task is 1B. It should consume the pre-move support result instead of adding another probe style.
* Revisit this record if a second character controller needs a different support query, or when 1B shows the stick and the reach window fight gravity.

## Evidence and revisit conditions

Headless tests cover the first supported step, a 1,000-step rest, flat and diagonal walks, a blocked wish staying `Idle`, a ledge, floor removal, sensors, masks, walls, ceilings, tie-breaks, deep penetration, an invalid collider, an indexed descendant, authority, a disabled component, a body with no locomotion component, `sync_collision` without a step, invalid deltas, repeated steps, a zero-iteration frame, sprite and mesh children, cameras, and an unlocked Y axis. The counts and the support-query timings are in [../../TASKS.md](../../TASKS.md) and [../PERFORMANCE.md](../PERFORMANCE.md).

This stays provisional until 1B through 1E exist or the owner replaces this split. A measurement that the downward probe mis-labels a walkable contact, or a second implementation that cannot share this probe, reopens the support window.
