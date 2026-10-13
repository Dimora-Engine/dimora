# ADR-0002: GPU caches evict on asset removal and after idle frames

* **Status:** Settled
* **Date:** 2026-10-13
* **Backlog id:** D-04
* **Decided by:** project owner (accepted the proposed default)
* **Affects:** `engine-renderer-wgpu` caches, `AssetManager::remove`, scene unload (M4), hot reload (M17)

## Context and problem

The wgpu caches for textures and meshes were keyed by asset id and never evicted, and `AssetManager` had
no `remove`, so GPU memory only grew (KI-4).

## Requirements and constraints

* A removed asset must free its GPU copy promptly.
* An asset that is merely off-screen should not be re-uploaded every frame.
* A replaced asset must never be drawn from a stale copy.
* The renderer sees only `RenderScene` plus the asset manager; no per-asset reference counts exist.

## Options considered

### A. Evict when the asset is removed from the manager
* Advantages: simple; exact for explicit unloads.
* Disadvantages: a texture no longer used but still loaded stays resident.

### B. Also evict entries unused for N frames
* Advantages: bounds memory for assets that stay loaded but unused.
* Disadvantages: an asset unused for N frames is re-uploaded when it returns.

### C. Explicit `release` only
* Advantages: fully predictable.
* Disadvantages: every caller must remember; nothing calls it for scenes yet.

## Decision and rationale

A and B together: evict when the asset is gone from the manager, and after a configurable number of idle
frames (default 120, minimum 1, `u32::MAX` disables). Revisions are never reused for an id, even across
`remove`, so a stale copy cannot be mistaken for a current one.

## Consequences and follow-up

* Implemented in T2 (commits `884b0ff`, `1d92ce7`, `5d6a199`, `c3c98f8`); counts exposed as `CacheStats`.
* Instance buffers still only grow; nothing calls `remove` automatically until scenes exist (M4).
* wgpu's own allocation counts were not measured (needs wgpu's `counters` feature).

## Evidence and revisit conditions

GPU tests: caches return to `{0, 0}` after removal and recover on re-insert; replaced and re-inserted
assets are re-uploaded; mutation checks fail the tests when eviction is disabled. Revisit if M6
measurements show re-upload hitches from the idle rule, or if M17 streaming needs reference counting.
