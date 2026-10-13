# ADR-0001: `despawn` removes the whole subtree

* **Status:** Settled
* **Date:** 2026-10-13
* **Backlog id:** D-03
* **Decided by:** project owner (accepted the proposed default)
* **Affects:** `engine-world` hierarchy, scene unload (M4), animation attachments

## Context and problem

`World::despawn` on a parent left its children alive with a `Parent` pointing at a dead entity, and
`propagate_transforms` then left them at the identity transform forever (KI-8). Bones and attachments
are entities in the hierarchy, so this is the normal case, not an edge case.

## Requirements and constraints

* No dangling `Parent` or `Children` after any despawn.
* Skeleton bones and attached equipment are owned by their parent.
* Scene unload (M4) must be able to remove a whole subtree in one call.
* A caller that wants children to survive must be able to say so.

## Options considered

### A. `despawn` removes the subtree
* Advantages: no orphans by construction; matches ownership of bones and attachments; one call for scene unload.
* Disadvantages: a caller who wants the children must ask for it explicitly.

### B. Children survive and become roots, keeping their world transform
* Advantages: nothing is destroyed by accident.
* Disadvantages: orphaned bones and attachments are never wanted and would leak.

### C. Two calls (`despawn` and `despawn_recursive`), the caller chooses
* Advantages: explicit at every call site.
* Disadvantages: the unsafe-by-default variant stays easy to call; two paths to test.

## Decision and rationale

A, plus an explicit `detach_children(parent)` that implements B on request (children become roots at the
same world transform). `despawn_recursive` was removed. Rationale: ownership in this engine runs from
parent to child, so the default should preserve that.

## Consequences and follow-up

* Implemented in T3 (commits `e1f70b1` and `498f77a`); 8 world tests and 4 propagation tests, with a
  consistency helper checking every `Parent` and `Children` link.
* Scene unload (M4) can rely on subtree despawn.
* `despawn` and `detach_children` do not touch `GlobalTransform`; the next propagation refreshes it.

## Evidence and revisit conditions

Mutation check recorded in TASKS.md (T3): making `despawn` skip children fails 4 tests. Revisit only
if a feature needs shared children between parents, which the one-parent model cannot express anyway.
