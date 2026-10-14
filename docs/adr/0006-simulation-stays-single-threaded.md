# ADR-0006: Simulation stays single-threaded

* **Status:** Settled
* **Date:** 2026-10-14
* **Backlog id:** D-44
* **Decided by:** project owner (approved the proposed direction)
* **Affects:** `Game::step`, `Schedule`, asset loading (M17), scripting (M18), parallel extraction (M23)

## Context and problem

`World` holds `RefCell` resources and systems are `FnMut(&mut World)`, so the simulation is
single-threaded by construction (R-8, KI-9). Asset data is `Send + Sync`. M17 plans worker-pool
loading, M23 plans parallel extraction, and M18 may host a runtime that is not thread-safe. The rule
was only a leaning, so a scheduler built in M3 could have assumed the wrong thing.

## Requirements and constraints

* `Game::step` stays callable from tests with a chosen `Duration`, on one thread, with no lock.
* Later parallelism must not require systems to be `Send` or resources to be `Sync`.
* The spatial world is one `World`. There is no second world for a worker thread.

## Options considered

### A. Simulation on the calling thread; other threads see only `Send` snapshots
* Advantages: matches the code that exists; asset decode and a future extract snapshot fit it; no access declarations.
* Disadvantages: parallel gameplay systems are a later, explicit change.

### B. Design the scheduler now for parallel systems with declared access
* Advantages: nothing to undo if parallel gameplay is wanted.
* Disadvantages: `RefCell` resources cannot answer "who reads what"; it would be a second scheduler (D-01).

### C. Leave the rule implicit
* Advantages: no document.
* Disadvantages: M17 and M23 would guess.

## Decision and rationale

A. The thread that calls `Game::step` owns the `World` for the whole step. Systems stay
`FnMut(&mut World)` and are not required to be `Send`. `World` stays `!Sync`. Work that leaves that
thread, now or later, moves plain `Send` data (bytes to decode, a `RenderScene` to draw) and does not
borrow the `World`.

## Consequences and follow-up

* M3a-2 builds stages on the current `Schedule`, not a parallel one.
* M17 loading and M23 extraction may use threads only on those snapshots.
* This decision does not choose a thread pool, an async runtime, or a script threading model (D-42).

## Evidence and revisit conditions

The types are the evidence: `Resources` uses `RefCell`, and `Schedule` stores `Box<dyn FnMut(&mut World)>`.
Reopen only if a measured gameplay workload cannot meet its budget on one thread and the measurement
names the systems that would have to run together.
