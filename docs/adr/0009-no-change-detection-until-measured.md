# ADR-0009: No change detection until a measurement asks for it

* **Status:** Provisional
* **Date:** 2026-10-14
* **Backlog id:** D-02
* **Decided by:** project owner (approved "none until profiled")
* **Affects:** `propagate_transforms`, M4 dirty transforms, M23

## Context and problem

`propagate_transforms` visits every entity on every `step`. Nothing can ask what changed (KI-9,
KI-19). A dirty bit or a new ECS would bake a mutation API into every system before any profile
has shown the full pass is the cost that matters.

## Requirements and constraints

* M3a must not add a write path that gameplay has to use instead of setting `Transform` fields.
* The hierarchy invariant and the full propagation pass stay correct (ADR-0001, ADR-0007).
* A later optimisation has to be able to replace the full pass without changing what a system writes.

## Options considered

### A. No change detection
* Advantages: systems keep writing `Transform`; nothing new to get wrong; M23 can still add a dirty pass later.
* Disadvantages: propagation stays O(entities) per frame.

### B. A dirty flag set by a mutation API
* Advantages: propagation can skip still subtrees.
* Disadvantages: every write has to go through the API, or the flag is a lie.

### C. Replace the ECS with one that tracks changes
* Advantages: the feature comes with the crate.
* Disadvantages: replaces `World` for a feature nobody has measured.

## Decision and rationale

A. `propagate_transforms` keeps walking every entity. Components stay plain data. There is no
changed-tick, no dirty bit, and no mutation wrapper.

## Consequences and follow-up

* M3a does not implement change detection. M4 does not get a dirty-transform pass from this decision.
* Interpolation (ADR-0007) stores a previous local for entities that opt in. That store is not change detection.

## Evidence and revisit conditions

Revisit when a profile, on a scene whose size is recorded, shows `propagate_transforms` or another
full pass is a material part of the frame, and names the budget it misses. KI-19 is that profile's
home. A guess about a large scene is not the measurement.
