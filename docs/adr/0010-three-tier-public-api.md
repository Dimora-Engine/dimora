# ADR-0010: Three-tier public API

* **Status:** Settled
* **Date:** 2026-10-14
* **Backlog id:** D-45
* **Decided by:** project owner (approved the proposed rule)
* **Affects:** `engine` prelude, `engine-world` re-exports, M9 `docs/API_SURFACE.md`, scripts (M18), editor (M22)

## Context and problem

The facade re-exports every crate. `engine_platform` re-exports `winit` and `engine_renderer_wgpu`
re-exports `wgpu`. `World.entities` is a public `hecs::World`, and `Entity` is hecs's (R-7). Nothing
in force said which of those a game, a script, or the editor may depend on. Wrapping `hecs` now would
be a new ECS surface in the middle of the frame-model work.

## Requirements and constraints

* M3a adds no new public `hecs` type.
* The rule exists before M9 writes the list. The list is still M9's job.
* Existing examples keep compiling. Removing `winit` or `wgpu` from the facade is not part of M3a.

## Options considered

### A. Three tiers, written down now; stop adding public `hecs` types; wrap nothing yet
* Advantages: later code knows what it may depend on; M9 still owns the inventory.
* Disadvantages: the leak stays until M9. The rule does not close it by itself.

### B. Wrap `hecs` now (our own `Entity` and query types)
* Advantages: storage could change behind the wrapper.
* Disadvantages: a second object model, and every call site moves, before a reason to swap storage exists.

### C. Document that the engine exposes `hecs`, `winit` and `wgpu` as supported
* Advantages: honest about today.
* Disadvantages: those crates become a compatibility promise. That is the opposite of what M9 is for.

## Decision and rationale

A. The tiers already drafted in `ARCHITECTURE.md` are the rule:

| Tier | What a dependent may rely on | How it changes |
|---|---|---|
| Contract | Gameplay-facing components as they stabilise (`Transform`, `Name`, `Visibility`, `Camera`, `Sprite`, `Mesh`, `Light`), asset handles, and, once they exist, actions, events, commands, the script API, and every on-disk format | A recorded decision. Formats also follow ADR-0011 |
| Backend seam | `RenderScene`, and later the UI draw list and `Pose` | One commit updates every consumer, and the change is documented |
| Internal | `hecs` types, `World.entities`, schedule internals, the `wgpu` and `winit` re-exports, `SceneRenderer` internals, extraction helpers | Free to change. Not a supported dependency. Docs and examples do not teach them as the API |

M3a adds no public type that is a `hecs` type and no new `hecs` re-export. The existing ones stay
until M9 lists them and either wraps or explicitly retires each one. Storage swappability is not a
goal of M3a.

## Consequences and follow-up

* M3a-6 is this rule, plus a review that the M3a commits did not add a public `hecs` type. The review
  happens with the code, so this ADR is the M3a-1 half of that task.
* M9 writes `docs/API_SURFACE.md` and decides, item by item, what happens to the leaks.
* Commands and events (ADR-0008), when they land, are contract types. Their fields are plain data.

## Evidence and revisit conditions

The re-exports are in `crates/engine-world/src/lib.rs`, `crates/engine-platform/src/lib.rs` and
`crates/engine-renderer-wgpu/src/lib.rs`. Reopen only if M9's inventory finds a contract that cannot
be expressed without a public `hecs` type, and names that type.
