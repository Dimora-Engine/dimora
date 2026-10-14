# ADR-0011: Persisted formats carry a version from the first byte

* **Status:** Settled
* **Date:** 2026-10-14
* **Backlog id:** D-47
* **Decided by:** project owner (approved the proposed policy)
* **Affects:** asset descriptors, compiled assets, scenes, bundles, saves, settings (M17, M19, M21, M24)

## Context and problem

No persisted format exists yet, and there is no `serde` dependency (R-6). The first format that ships
without a version is the expensive one to repair. M3a creates no format. The policy has to be settled
before one does.

## Requirements and constraints

* A loader can reject a file it does not understand without guessing.
* Entity indices are not identities. Saved references use the stable ids from D-16, which is still open and is not decided here.
* Pre-1.0 breaks are allowed only as version bumps, not as silent changes.

## Options considered

### A. Every format carries a version from its first line; unknown versions are errors
* Advantages: one rule for assets, scenes, bundles, saves and settings; a test can pin it the day the first format exists.
* Disadvantages: the first encoder has to write a version even when there is only one.

### B. Versions start at the first breaking change
* Advantages: the first file is shorter to specify.
* Disadvantages: the first file has no version field to put the number in. That is the break this rule exists to avoid.

### C. Decide per format, at that format's milestone
* Advantages: each milestone is free.
* Disadvantages: one of them will forget, and R-6 is that forgetfulness.

## Decision and rationale

A. Every persisted format (asset descriptors, compiled assets, scenes, bundles, saves, settings)
carries a magic number or a `version` from its first line. A loader returns an error for an unknown
version. It does not panic and it does not invent a migration. A version bump ships either a
migration or a documented break, and a test that loads a stored older fixture. Ids in files are the
stable asset and entity ids from D-16, never `hecs` entity indices and never a slot in a `Vec`.

This ADR does not choose encoding, `serde`, or the id scheme.

## Consequences and follow-up

* No format code in M3a.
* M17, M19 and M21 cite this rule in the format's own decision. D-46 (saves) uses it too.
* D-16 still has to decide what the stable id is. Until then, no format is written.

## Evidence and revisit conditions

There is no format to measure. Reopen if a format cannot put a version in its first record, and the
reason is the medium rather than convenience.
