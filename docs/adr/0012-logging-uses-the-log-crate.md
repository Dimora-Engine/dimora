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
* The runner logs once when a surface is lost or a frame render returns an error, and the record contains the error's cause. It still exits, as it does now.

`Diagnostics` (stage durations, frame `dt`, and the existing render and cache counts) is a resource
M3a-4 adds so a system can read those numbers. It is not a profiler and it is not the log.

## Consequences and follow-up

* M3a-4 is the implementation. M20 may add `tracing` or another profiler without replacing these records.
* Script errors (M18) use the same crate.
* The dependency is justified here so M3a-4 does not reopen D-17 for this one crate.

## Evidence and revisit conditions

A search of the workspace at `6dbabb8` finds no `log`, `tracing`, or `println!` in the library crates.
Reopen the crate choice if a call site needs a span to name the cause, and a record with fields cannot.
