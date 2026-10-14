# ADR-0012: Logging uses the `log` crate

* **Status:** Settled
* **Date:** 2026-10-14
* **Backlog id:** D-21
* **Decided by:** project owner (approved the `log` crate)
* **Affects:** asset loads, extraction, the window runner, M18 script errors, M20 diagnostics

## Context and problem

Library code does not log. A failed asset load is visible only to whoever polls the asset. A missing
renderable is a counter on `ExtractStats`. A lost surface becomes a `RenderError` and the runner
exits. Every new error path would have to be revisited once a facade existed (R-13). Profiling is a
different problem and stays in M20.

## Requirements and constraints

* One new dependency, named, with its licence. No engine-owned logging facade crate.
* Tests can capture records without a process-wide logger leaking between tests.
* "Once" means once per failure, not once per frame while the failure still exists.
* M3a-1 does not add the dependency. M3a-4 does.

## Options considered

### A. The `log` crate, and a tiny stderr backend we own
* Advantages: the API is the standard one; tests implement `log::Log`; one dependency.
* Disadvantages: no spans. Profiling will add its own tool later.

### B. `tracing`
* Advantages: spans are there when M20 wants them.
* Disadvantages: a heavier crate, and a second style, for a milestone that only needs records.

### C. Our own facade trait
* Advantages: no dependency.
* Disadvantages: every crate learns our trait, and a game cannot point its existing logger at it.

## Decision and rationale

A. M3a-4 adds the `log` crate (version chosen then, licence `MIT OR Apache-2.0`, no default features
that pull a backend). The engine does not add `tracing` and does not add `env_logger`. A few dozen
lines write records to stderr when a logger has not been installed. Tests install a capturing
`log::Log` and assert on it.

Three call sites, each one record per occurrence:

* A load that **becomes** failed logs once, and the record names the asset. A later frame that still sees it failed does not log again. A new failure of that asset logs again.
* Extraction that skips a renderable because its asset is missing logs once per missing asset. When that asset is available again, a later miss logs again.
* The runner logs an error when window initialization fails or a frame render returns an error, and the record contains the error's cause. It still exits, as it does now.
* A lost or outdated window surface logs one warning and the renderer reconfigures. The frame loop continues, which is what that path already did. A later successful acquire clears the mark, so the next loss logs again. Timeout and occlusion skip the frame and are not logged.

A load that is still in flight is not a failure and is not logged. Extraction logs a skipped sprite, mesh, or tilemap once per asset id until that asset is loaded again. It remembers at most 64 ids and does not rotate the memory, so a full buffer does not warn again every frame. Further distinct misses stay on `ExtractStats.missing_assets` until a remembered asset loads and frees a slot.

Command failures stay on `command_failures`. `Game::step` logs each new `CommandFailure` once at the end of the step. The next step clears the list before systems run.

`Game::run` installs a stderr logger at Info only when `log::set_logger` succeeds. An application logger, and the filter it set, are left in place.

`Diagnostics` (stage durations, frame `dt`, fixed-step count, entity count, and the existing render and cache counts) is a resource M3a-4 adds so a system can read those numbers. `frame` is the simulation delta, not CPU time. Stage durations are CPU time measured with `Instant`. The fixed-update duration is the whole fixed loop. `render` is the previous successful render until the runner or `render_to_rgba` writes a new one. It is not a profiler and it is not the log.

## Consequences and follow-up

* M3a-4 is the implementation. M20 may add `tracing` or another profiler without replacing these records.
* Script errors (M18) use the same crate.
* The dependency is justified here so M3a-4 does not reopen D-17 for this one crate.

## Evidence and revisit conditions

A search of the workspace at `6dbabb8` finds no `log`, `tracing`, or `println!` in the library crates.
Reopen the crate choice if a call site needs a span to name the cause, and a record with fields cannot.
