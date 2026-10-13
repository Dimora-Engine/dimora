# ADR-0005: Continuous integration on GitHub Actions across Windows, macOS and Linux

* **Status:** Provisional
* **Date:** 2026-10-13
* **Backlog id:** D-22; owner decision OD-4 in [../DECISIONS.md](../DECISIONS.md)
* **Decided by:** project owner (direction: use GitHub Actions if the repository is hosted on GitHub; validate on all three desktop platforms, Windows early). That the repository is hosted on GitHub is an observation, not a recorded owner statement.
* **Affects:** `.github/workflows/ci.yml`, `scripts/check.sh`, README, TASKS.md (KI-3, KI-27, B-1)

## Context and problem

There was no CI (KI-3), no code host decision (D-22), Windows had never been built (KI-27), and the MSRV
(1.90) had only been verified on macOS and, in the audit, Linux. Regressions on untested platforms would be
found by users.

## Requirements and constraints

* Run the existing `scripts/check.sh` (fmt, clippy `-D warnings`, tests with `DIMORA_REQUIRE_GPU=1`, rustdoc
  `-D warnings`) on Windows, macOS and Linux, plus example builds.
* Investigate platform-specific failures; do not suppress or skip them to get green.
* GPU tests need an adapter; hosted runners are virtual machines, so what each offers is empirical.

## Options considered

### A. GitHub Actions, hosted runners (`ubuntu-latest`, `macos-latest`, `windows-latest`)
* Advantages: the repository is on GitHub; all three desktop platforms; no infrastructure to run.
* Disadvantages: runners may lack a GPU (software adapters only); minutes are metered; third-party actions need pinning.

### B. Self-hosted runners with real GPUs
* Advantages: real hardware, GPU timing.
* Disadvantages: infrastructure, security and cost; not needed for correctness tests yet.

### C. Defer CI
* Advantages: none that outweigh the risk.
* Disadvantages: Windows stays unverified.

## Decision and rationale

A. One workflow runs, per OS: the pinned toolchain's `scripts/check.sh`, both examples as headless screenshot
smoke runs, and a separate MSRV job (`cargo +1.90.0 test --workspace --locked`). Linux installs
`mesa-vulkan-drivers` (lavapipe). Results of the first runs are recorded in TASKS.md.

## Consequences and follow-up

* KI-3 and KI-27 are closed by the first green run; B-1 is resolved in practice (owner confirmation of GitHub as host is still outstanding). CI protects `main` once the workflow is merged.
* Real-GPU timing (M6, M23) is not covered and would need option B.
* The cache and artifact actions are third-party or GitHub-owned and pinned by major tag; pin by commit SHA before the first release.
* Does not decide a release pipeline (M24) or dependency licence checks (OD-3 follow-up).

## Evidence and revisit conditions

First run (2026-10-13, [run 38475229601](https://github.com/Dimora-Engine/dimora/actions/runs/38475229601)): all six jobs green, 138 tests and no skipped GPU test on Linux (llvmpipe), macOS (Metal, paravirtual) and Windows (DX12, WARP), MSRV 1.90 green on all three, no platform-specific failure. Details and limits are in TASKS.md ("CI results"). Revisit if a runner type loses its software adapter, if
minutes become a problem, or when GPU-timed performance gates are needed.
