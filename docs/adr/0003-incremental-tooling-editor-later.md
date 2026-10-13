# ADR-0003: Incremental developer tooling now; the full editor after its foundations are stable

* **Status:** Settled (owner direction, 2026-10-13)
* **Date:** 2026-10-13
* **Backlog id:** D-20 (technology still open), owner decision OD-1 in [../DECISIONS.md](../DECISIONS.md)
* **Decided by:** project owner
* **Affects:** roadmap order (M3a, M4, M6, M10, M17, M20, M22), `engine-world` registry, runtime diagnostics, ARCHITECTURE.md requirement 7

## Context and problem

The editor is a core product goal ([../VISION.md](../VISION.md)), but a full visual editor needs a stable scene
model, a component registry, serialization, asset references and runtime APIs, none of which exists yet
(CAPABILITIES.md rows 32, 33, 27, 36). Two failure modes follow: building the editor early, so it is rewritten
as the foundations move; or deferring all tooling to the editor milestone, so developers work blind for most
of the project and the runtime grows no hooks for inspection.

## Requirements and constraints

* The full editor is not reordered ahead of its prerequisites without a demonstrated dependency or product need.
* Useful tooling arrives in the milestone where it first pays off.
* The runtime must not depend on editor-specific state or behaviour, and must run with no editor present.
* Anything an editor will need (registry, commands, stable ids, diagnostics) is shaped by the earlier tooling,
  so the editor reuses it instead of adding parallel paths.

## Options considered

### A. Editor early, around M4 to M6
* Advantages: users get visual authoring sooner.
* Disadvantages: built on an unsettled scene model, registry, ids and API; high rework; delays the foundations.

### B. All tooling at M20 and M22
* Advantages: simplest roadmap.
* Disadvantages: no diagnostics while the engine is built; runtime hooks are designed late, with the editor as the
  first consumer (R-15).

### C. Incremental tooling now, full editor at M22 behind a stability gate
* Advantages: each tool is small and useful alone; each exercises the contracts the editor needs; the editor
  starts only when its foundations hold.
* Disadvantages: several small deliverables to schedule and keep honest.

## Decision and rationale

C. Tooling is delivered incrementally (table "Incremental developer tooling" in [../../ROADMAP.md](../../ROADMAP.md)):
logging and runtime diagnostics in M3a, read-only entity and scene inspection in M4, render debugging in M6,
asset validation in M5 and M17, an on-screen overlay in M10, consolidation in M20. The full editor stays at
M22. It starts only when the **stability gate** holds: scene model and component registry (M4), asset
references (D-16, M17), serialization with a versioned format (M19) and the runtime API surface (M9) are stable
enough that editor work would not be redone. The runtime exposes data and commands (registry, events, commands,
diagnostics); it contains no editor types, no `cfg(editor)` state and no editor-only code path.

## Consequences and follow-up

* M3a to M6 gain small, named tooling deliverables with acceptance criteria; none builds editor UI.
* The tools read the same registry and diagnostics the editor will use, so they de-risk R-15.
* The editor can still be pulled earlier only through an ADR that names the dependency or product need.
* Does not decide the editor's technology (D-20).

## Evidence and revisit conditions

Settled. Reopen if a milestone's tooling cannot be built without editor-specific runtime state (that would show
the boundary is wrong), or if the owner records a product need for an earlier editor.
