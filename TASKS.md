# Tasks: current development state

Last audited: 2026-10-13 at commit `4857158` (Linux, `scripts/check.sh` exit 0, 138 tests, no GPU test
skipped; macOS results from T1 to T3 are recorded below; Linux, macOS and Windows CI results are under "CI results"). Nothing is marked done here without evidence from
the repository or a recorded test run. This file holds the **current milestone, task acceptance criteria,
known issues and evidence**. Where things sit in the whole plan: [ROADMAP.md](ROADMAP.md). What each system
does today: [docs/CAPABILITIES.md](docs/CAPABILITIES.md). Product intent: [docs/VISION.md](docs/VISION.md).
Architecture and risks: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). Open decisions:
[docs/DECISIONS.md](docs/DECISIONS.md).

## Current milestone

**M2. Foundation hardening and CI: closed for planning.** T1, T2 and T3 are done. Two items are carried
because they wait on the owner, not on code: licence files (KI-17) and CI (D-22, B-1). Neither blocks M3a.

**M6 is in progress for one task.** The camera-configuration test scene was requested and is verified (below). The rest of M6 is not started. Milestone order is unchanged.

**M3a, the frame model, is complete locally and is not pushed.** Full scope and acceptance criteria:
[ROADMAP.md](ROADMAP.md#m3-frame-model-input-and-platform-in-progress). M3a-2 is commit `312a05f`.
Lifecycle anchors are `ecf4fa1`. M3a-3 is `cd802f0`. M3a-4 is `5c4fd97`. M3a-5 is `523f553`. M3a-6
is the public-API review recorded below. M3b Phases 1–3 are checkpointed locally (`57dbc4a`,
`83286a9`, `9c99edf`). Phase 4 is checkpointed locally as `cfdf358`. None of M3b is pushed. The rendering regression baseline is commit `6dbabb8`.

**M4 is in progress.** Phase 1, the component registry and text dumps, is commit `cdb5ec3` and is not
pushed (ADR-0014). Phase 2, hierarchical visibility, is commit `2c76f71` and is not pushed. Phase 3,
name and tag lookup, is commit `f9a4aab` and is not pushed. M4-4A, scene instance tracking, is
commit `302622b` and is not pushed. M4-4B, synchronous scene load, is implemented locally and is not
committed. Unload, replace, and the `spatial_scene` switch are not started. M4 is not complete.
Evidence is under "M4 Phase 1 results", "M4 Phase 2 results", "M4 Phase 3 results", "M4-4A results",
and "M4-4B results".

## Completed work

| Milestone | Evidence |
|---|---|
| M0 Foundation | Commits `6b0d016`..`278de15`: math, core, world, assets, renderer, wgpu backend, animation, platform, facade, `basic_2_5d`. All crates build; tests pass (below) |
| M2 / T3 Hierarchy robustness and asset-id collision guard | Commits `498f77a`, `e1f70b1`, `8abee2e`, `a4f9e77`; evidence under "Task T3 results" below. Verified on macOS only |
| M2 / T2 GPU resource lifetime and `AssetManager::remove` | Commits `884b0ff`, `1d92ce7`, `5d6a199`, `c3c98f8`; evidence under "Task T2 results" below. Verified on macOS only |
| M2 / T1 Reproducible toolchain and one verification command | Commits `e08033e`, `3a7a95c`, `7fb2c4d`, `3d467b6`, `89b1fa5`; evidence under "Task T1 results" below. Verified on macOS only |
| M1 Core spatial renderer | Commits `83d9883`..`6ad872f`: sprite orientation, `RenderOrder`, sprite `region`, camera `ViewportRect`, `spatial_scene`, `docs/SPATIAL_RENDERING.md`. `engine-renderer/tests/camera.rs` (27 tests), GPU tests `camera_viewport_confines_the_image_to_its_rectangle` and `billboards_face_the_camera_but_fixed_quads_go_edge_on` pass with a Vulkan adapter. `spatial_scene` run windowed under Xvfb: figure heights shrink with depth in perspective and stay equal in orthographic |
| M6 camera-configuration test scene (one task; M6 itself is not done) | Commit `6dbabb8` (`feat(examples): add M6 spatial scene and camera validation`), same as `origin/main` when M3a-1 started. Evidence under "M6 test scene" below. Verified on macOS (Apple M5, Metal) only. This commit is the rendering regression baseline for M3a |

## Verification record (2026-10-12)

Environment: Linux, `rustc 1.99.0` stable (installed with `rustup toolchain install stable`; the VM's
default was 1.83.0), lavapipe software Vulkan from `mesa-vulkan-drivers`.

| Command | Result |
|---|---|
| `cargo fmt --check` | Pass |
| `cargo clippy --workspace --all-targets` | Pass, no warnings |
| `cargo test --workspace` | Pass: **99 tests, 0 failed** |
| `cargo doc --workspace --no-deps` | 2 warnings (KI-16) |
| `cargo test` with rustc 1.83.0 | **Fails** before compiling: `wayland-protocols 0.32.13` requires `edition2024` (KI-1) |
| `cargo test -p engine-renderer-wgpu --test headless` with no adapter | 7 tests "pass" but each only prints `skipping GPU test` (KI-2). With lavapipe installed all 7 really run and pass |

Test counts by crate: `engine-math` 22, `engine-core` 5, `engine-world` 7, `engine-assets` 8 (+1
doctest), `engine-renderer` 2 unit + 27 (`camera.rs`) + 13 (`spatial.rs`), `engine-renderer-wgpu` 1 unit
+ 7 GPU, `engine-animation` 4, `engine-platform` 1, `engine` 1 doctest. The examples have no tests;
they are exercised by `--screenshot` runs and manually. (This is the 2026-10-12 count of 99. After T1,
`engine-renderer-wgpu` has 12 in `tests/headless.rs`: 7 GPU plus 5 for `DIMORA_REQUIRE_GPU`, 104 in total. After T2:
`engine-assets` 16 unit tests, `headless.rs` 18, 118 in total. After T3: `engine-world` 19, `engine-assets` 24 unit tests, 138 in total.)

## Verification record (macOS, 2026-10-13)

Environment: MacBook Pro, **Apple M5, arm64** (`uname -m` = `arm64`), macOS 27.0 (build 26A428), Xcode
26.4.1 selected (`xcode-select -p` = `/Applications/Xcode.app/Contents/Developer`) with Command Line Tools
27.0 installed alongside (see KI-26). `rustc 1.99.0 (b940084d7 2026-09-28)`, `cargo 1.99.0
(5f94df478 2026-08-27)`, `clippy 0.1.99`, `rustfmt 1.10.0-stable`, rustup 1.29.1. GPU: wgpu picked
**Apple M5 (Metal, IntegratedGpu)** (printed by the GPU tests with `--nocapture`). No software driver, no Xvfb.

**Baseline, before any change (from `a0b4662`, matches the Linux audit exactly):**

| Command | macOS result | Difference from Linux audit |
|---|---|---|
| `cargo fmt --check` | Pass | none |
| `cargo clippy --workspace --all-targets` | Pass, 0 warnings | none |
| `cargo test --workspace` | Pass: 99 tests, 0 failed (97 + 2 doctests); the 7 GPU tests rendered on Metal and none printed `skipping GPU test` | none: no pixel differences, no Metal-specific failures |
| `cargo doc --workspace --no-deps` | 2 warnings (KI-16), same two links | none |

**After T1 (commit `89b1fa5` and this file):** `scripts/check.sh` exits 0 on macOS: fmt, clippy `-D warnings`,
`cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (**104 tests, 0 failed**: the 99 above plus 5 new
unit tests of the skip-or-fail decision), and `cargo doc` with `RUSTDOCFLAGS="-D warnings"` (0 warnings).

**Not verified on this Mac:** anything on Linux or Windows (lavapipe, `VK_ICD_FILENAMES=/nonexistent`,
Vulkan), and any run with an Intel Mac.

## Task T1 results (2026-10-13, macOS arm64)

| Criterion | Evidence |
|---|---|
| 1. `rust-toolchain.toml` and a verified `rust-version` | `rust-toolchain.toml` pins `1.99.0` (+ rustfmt, clippy). `rust-version = "1.90"`, inherited by all 11 crates. **Verified MSRV: 1.90.0**: `cargo +1.90.0 test --workspace --locked` on exactly `rustc 1.90.0 (1159e78c4 2025-09-14)`, macOS arm64: 99 passed, 0 failed (GPU tests included, on Metal). Why not lower: `cargo +1.89.0 check --workspace --locked` is refused (`ordered-float@5.5.0 requires rustc 1.90`, reached through `wgpu-hal` on Linux and Windows; it is not in the macOS graph). On macOS alone 1.89.0 does build, but the workspace MSRV must hold on every target, and cargo 1.89 rejects the Linux graph (`--target x86_64-unknown-linux-gnu`). **Not done: an actual build and test on 1.90.0 on Linux or Windows**; the floor there is what cargo's metadata enforces. The MSRV holds only with the committed `Cargo.lock` (`--locked`) |
| 2. `scripts/check.sh` | Runs the four steps in order with the required flags. Exit 0 on macOS (above), also when started from another directory. Failure paths run: with the GPU removed (`WGPU_BACKEND=vulkan`) it stops at "FAILED at step 3/4: cargo test --workspace (DIMORA_REQUIRE_GPU=1) (exit status 101)" and does not run step 4; with an unformatted file it stops at step 1/4, exit 1. Not run on Linux with lavapipe (the criterion's literal wording) |
| 3. `DIMORA_REQUIRE_GPU` | Decision is the pure function `on_gpu_unavailable` in `crates/engine-renderer-wgpu/tests/headless.rs`, covered by 5 unit tests (unset skips; only the exact value `1` requires; required and missing fails with "no GPU adapter was found"; the Linux message suggests `mesa-vulkan-drivers`, the macOS one does not; an unusable adapter also fails). **End-to-end on this Mac as well**, not only by unit test: `WGPU_BACKEND=vulkan` removes the adapter (Vulkan is not compiled in on macOS). Unset: `skipping GPU test: no suitable GPU adapter: ...`, test `ok`. With `DIMORA_REQUIRE_GPU=1`: all 7 GPU tests panic with `DIMORA_REQUIRE_GPU=1 but no GPU adapter was found: ...`. The Linux method `VK_ICD_FILENAMES=/nonexistent` was **not** run |
| 4. rustdoc warnings fixed | `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`: 0 warnings (commit `e08033e`) |
| 5. README | Required toolchain, per-platform GPU table (Metal on macOS, Vulkan driver and the apt command for lavapipe on Linux), `scripts/check.sh`, `DIMORA_REQUIRE_GPU` and how to force the no-adapter path |
| 6. No new dependencies; existing tests pass | `Cargo.lock` unchanged by T1 (`git diff a0b4662 -- Cargo.lock` is empty); all 99 earlier tests still pass |

## Task T2 results (2026-10-13, macOS arm64, Apple M5 / Metal, rustc 1.99.0)

Decision D-04 (accepted by the owner): evict when the asset is removed, and when unused for N frames.

| Criterion | Evidence |
|---|---|
| 1. `AssetManager::remove` | `remove::<T>(handle) -> bool` forgets the entry (value, state, error, name, revision), its own dependency links, links other assets recorded to it (unless an asset of another type still has that id; links are untyped ids), and any pending load for it. 8 unit tests in `engine-assets`: `get` is `None`, `state` is `Unknown`, `revision` is 0; failed loads; both link directions; same-id-different-type; a cancelled pending load does not resurrect the asset; other pending loads still run |
| 2. Eviction and counts | `SceneRenderer` counts frames, marks each cached texture/mesh used when a frame draws with it, and after the frame's uploads and before drawing evicts entries whose asset is gone or that went `cache_max_idle_frames` consecutive frames unused (default `DEFAULT_CACHE_MAX_IDLE_FRAMES` = 120, `set_cache_max_idle_frames`, minimum 1, `u32::MAX` disables idle eviction). `cache_stats()` returns `CacheStats { textures, meshes }` (placeholders not counted). `RenderStats.cache` carries it to the facade and window titles |
| 3. Baseline and re-insert | `removing_assets_returns_the_gpu_caches_to_the_empty_baseline`: fresh renderer `{0, 0}`; after a frame `{2 textures, 1 mesh}`; remove all assets and render: `{0, 0}`, centre pixel is the clear colour; re-insert and render: `{2, 1}` again and the sprite and cube pixels equal the first picture. `a_removed_texture_is_drawn_as_the_placeholder_never_from_a_stale_copy`: white texture removed while its sprite asset remains: the pixel goes from white to the magenta placeholder |
| 4. Replacement | `a_replaced_asset_is_reuploaded_without_keeping_the_old_copy`: three in-place replacements each draw the new colour and the cache stays at 1 texture. `remove_and_reinsert_between_frames_is_seen_as_a_change`: exposed a real hazard, revisions restarted at 1 after a removal and would have matched a stale cache entry; revisions now come from a per-store counter and are never reused. **Limit:** "no leak" is shown as one cache entry per asset (the old Rust-side GPU objects are dropped on replacement); wgpu's own allocation counts were not measured (that needs wgpu's `counters` feature, which was not enabled) |
| 5. Examples and 1,000 frames | Headless `--screenshot` of `basic_2_5d`, `spatial_scene` and `spatial_scene --ortho` are **byte-identical** (`cmp`) to the same screenshots taken from the unmodified engine (`f8186d2`), and I looked at the images. `spatial_scene --screenshot out.png --frames 1000` (release): `gpu cache after frame 1: { textures: 3, meshes: 5 }; after frame 1000: { textures: 3, meshes: 5 }`, exit 0 (it exits non-zero if the counts grow). **Not done:** the examples were not run in a window, and the 1,000-frame run has a static camera |
| Mutation checks | The tests fail when the feature is broken: eviction call removed: 4 GPU tests fail; re-upload on revision change disabled: 1 fails; per-entry revisions (reused after remove): 1 fails |

`scripts/check.sh`: exit 0, **118 tests, 0 failed** (104 before T2, +8 asset tests, +6 GPU tests), no skipped GPU test.
`cargo +1.90.0 test --workspace --locked` (MSRV, with the KI-26 `SDKROOT` workaround): 118 passed, 0 failed.

## Task T3 results (2026-10-13, macOS arm64, Apple M5 / Metal, rustc 1.99.0)

Decision D-03 (accepted by the owner): `despawn` removes the whole subtree; keeping the children is an explicit `detach_children` call that leaves them as roots at the same world transform.

| Criterion | Evidence |
|---|---|
| 1. Child of an entity with no `Transform` | Commit `498f77a`. `propagate_transforms` now also starts from entities that have `Children` but no `Transform` and no `Parent`, and passes the parent's matrix through any transform-less entity (identity for its own contribution). Four new tests in `hierarchy.rs`: `a_child_of_an_entity_without_a_transform_treats_the_parent_as_identity` (the KI-7 case, x = 5 expected), a transform-less entity in the middle of a chain, a transform-less root group above a whole subtree, and a transform-less parent under a rotated ancestor keeps that rotation. They failed on the unmodified code (child stayed at the origin) and pass now |
| 2. `despawn` follows D-03 | Commit `e1f70b1`. `World::despawn` returns `Err(NoSuchEntity)` for a missing entity without changing anything, otherwise unlinks it from its parent and despawns its whole subtree. `World::detach_children(parent)` removes `Children`, then for each child removes `Parent` and rewrites its local `Transform` to its old world transform (the parent's world matrix is rebuilt from the local transforms along its ancestor chain, so it does not depend on a fresh `GlobalTransform`). A transform-less child that has children of its own gets a `Transform` so they stay in place. `despawn_recursive` is removed. 8 new tests in `world.rs` use a three-level chain and a consistency helper that checks every `Parent` points at a live entity that lists it in `Children` and the reverse: despawn leaf, middle and root; missing entity; transform-less group; `detach_children` keeps children alive at the same world matrix (rotated, scaled parent; grandchild too); through a transform-less child; on a leaf and on a missing entity |
| 3. Collision guard | Commit `8abee2e`. `AssetManager::insert`, `load` and `reload` now return `Result<AssetHandle<T>, AssetError>`. A different name that hashes to an id already owned by an entry **of the same asset type** returns the new `AssetError::IdCollision { id, existing, new }` (message names both assets) and changes nothing: value, name, revision and state of the existing asset are untouched, and `load` queues no job. The same name still replaces. Entries that are loading or failed also own their id. After `remove` the id is free again. 8 tests; they use private `insert_with_id` / `load_with_id` to pick the id (finding two real names that collide would take about 2^32 tries): reject-and-keep, same name replaces, other type is no collision, remove frees the id, `load` rejected, colliding with a load in flight, colliding with a failed load, 2,000 distinct names do not collide |
| 4. Existing hierarchy tests | The tests in `hierarchy.rs` that existed before are unchanged (`git diff` of that file removes only non-test lines) and pass. One older test, `world::tests::hierarchy_bookkeeping`, changed by one line: it called `despawn_recursive`, which D-03 removes, so it now calls `despawn(a).unwrap()` and asserts the same thing |
| Mutation checks | `despawn` changed to not visit children: 4 tests fail. Collision checks removed from `insert_with_id` and `load_with_id`: 4 tests fail |
| Examples | Headless `--screenshot` of `basic_2_5d`, `spatial_scene` and `spatial_scene --ortho` are byte-identical (`cmp`) to the same screenshots from the engine before T3 (`f8186d2`). The examples' call sites now use `.expect("example asset names are distinct")` |

Limits: the guard is per asset type, as the criterion says. The same name hashing to one id for two different types is not an error, and dependency links are untyped ids, so such a pair would still make links ambiguous. The `reload` collision path is covered only through the shared check, not by its own test (it has no id-choosing hook). `detach_children` and `despawn` do not touch `GlobalTransform`; it is refreshed by the next `propagate_transforms`. There is still no dirty tracking (O(entities) every frame).

`scripts/check.sh`: exit 0, **138 tests, 0 failed** (118 before T3: +4 propagation, +8 despawn/detach, +8 collision), no skipped GPU test. `cargo +1.90.0 test --workspace --locked` (MSRV, with the KI-26 `SDKROOT` workaround): 138 passed, 0 failed. A first `scripts/check.sh` run stopped at step 2/4 on a clippy error in a new test constant (`unusual_byte_groupings`); fixed in `a4f9e77` and re-run clean.

## CI results (2026-10-13)

First run of `.github/workflows/ci.yml` ([run 38475229601](https://github.com/Dimora-Engine/dimora/actions/runs/38475229601)), on commit `464d03b` (pull request #3). All six
jobs passed. Nothing was suppressed, retried or skipped to get there.

| Job | Platform and adapter the GPU tests used | Result | Time |
|---|---|---|---|
| `check` Linux | ubuntu-latest, x86_64; `llvmpipe (LLVM 20.1.2)`, Vulkan, CPU (lavapipe installed by the workflow) | `scripts/check.sh` 4 of 4 steps; 138 passed, 0 failed, none skipped; 3 example screenshots written | about 14 min |
| `check` macOS | macos-26-arm64; `Apple Paravirtual device`, Metal | Same: 138 passed, 0 failed, none skipped; screenshots written | about 1.5 min |
| `check` Windows | windows-2025-vs2026; `Microsoft Basic Render Driver`, DX12, CPU (WARP) | Same: 138 passed, 0 failed, none skipped; screenshots written | about 21 min |
| `msrv 1.90` x3 | rustc 1.90.0 on x86_64 Linux, aarch64 macOS, x86_64 Windows | `cargo +1.90.0 test --workspace --locked`: 138 passed, 0 failed on each | 1 to 2 min each |

What this establishes: the engine builds, passes clippy with warnings denied, passes all 138 tests including the
offscreen pixel tests, builds rustdoc cleanly and renders both examples headless on Windows, macOS and Linux, and
the MSRV claim now holds on all three (KI-27). It is the first time the project has been built on Windows.
**No platform-specific failure appeared**, so there was nothing to investigate; the pixel assertions pass on
three unrelated rasterisers (lavapipe, Apple's paravirtual Metal device, WARP).

What it does not establish:

* **Real GPUs.** Every hosted adapter is a software or paravirtual device. Hardware drivers, performance and GPU
  timing are unverified.
* **The windowed path.** CI renders offscreen and headless only; window creation, DPI and input on Windows are not
  exercised (nothing in the code uses the scale factor yet, KI-24).
* **Example output is not compared.** The smoke runs check that a non-empty PNG is written, not that it equals the
  previous output; the screenshots are uploaded as artifacts for a person to look at.
* **Speed.** Windows takes about 21 minutes and Linux about 14, mostly compiling at the dev profile's `opt-level = 3`
  for dependencies and running GPU tests on a CPU rasteriser. The Windows time is dominated by test step 3 (10 min)
  and the release build of the examples (5 min). The "record which adapter" step recompiles the tests under a
  different command line and adds about 2 minutes on Linux; it is a diagnostic, and can be dropped if minutes matter.
* **One run.** Flakiness is unmeasured. An earlier run of the same workflow on this pull request was cancelled by a
  newer push (the `concurrency` group), not by a failure.
* The workflow runs on a pull request and on pushes to `main`; it does not exist on `main` until this branch merges.

## Known issues

"Verified" means reproduced by running something; "read" means found by reading the code and not
yet reproduced.

| ID | Issue | Evidence | Milestone |
|---|---|---|---|
| KI-1 | ~~No `rust-version` or toolchain pin; the workspace does not build on rustc 1.83~~ **Fixed by T1**: `rust-toolchain.toml` pins 1.99.0, `rust-version = "1.90"` verified on 1.90.0 (macOS only, see "Task T1 results") | Verified | M2 |
| KI-2 | ~~GPU tests skip silently when no adapter exists and still report `ok`~~ **Fixed by T1** for runs that set `DIMORA_REQUIRE_GPU=1` (`scripts/check.sh` does). Plain `cargo test` still skips and reports `ok` by design | Verified (forced no-adapter run on macOS) | M2 |
| KI-3 | ~~No CI of any kind~~ **Fixed on the PR branch** (`.github/workflows/ci.yml`, ADR-0005): the first run passed on Linux, macOS and Windows (see "CI results"). It protects `main` only once the workflow is merged | Verified (run 38475229601) | M2 |
| KI-4 | ~~GPU texture and mesh caches are never evicted; `AssetManager` has no `remove`~~ **Fixed by T2** (commits `884b0ff`, `1d92ce7`, `5d6a199`, `c3c98f8`; evidence under "Task T2 results"). Remaining: instance buffers only grow; nothing calls `remove` automatically until scenes exist (M4) | Verified | M2 |
| KI-5 | Only one camera renders per frame, so `ViewportRect` cannot give split-screen/minimap; no render-to-texture | Read (`extract.rs::active_camera`) | M6 |
| KI-6 | Textures: no mipmaps, clamp only, always sRGB; `Material` is colour + albedo + `lit`; meshes cannot be transparent | Read | M5 |
| KI-7 | ~~A child of an entity without a `Transform` stays at the origin~~ **Fixed by T3** (commit `498f77a`): such an entity counts as identity, at any depth | Verified (test failed before, passes after) | M2 |
| KI-8 | ~~`World::despawn(parent)` leaves children with a dangling `Parent`~~ **Fixed by T3** (commit `e1f70b1`, D-03): `despawn` removes the whole subtree; `detach_children` keeps children as roots. `despawn_recursive` is gone | Verified | M2 |
| KI-9 | Resource access is `RefCell`: a double borrow panics at runtime. Deferred commands and typed events are implemented (M3a-3). Change detection is still absent (ADR-0009) | Partial (the `RefCell` panic and change detection remain) | M3a-3 done; change detection stays deferred |
| KI-10 | ~~Frame order is hard-wired in `Game::step`~~ Named stages and `before`/`after` are implemented (M3a-2). ~~`FixedTimestep::alpha()` is not exposed~~ Opt-in extract interpolation is implemented (M3a-5). `Game::step` still runs the stages in the documented order | Verified | M3a-5 |
| KI-11 | `Camera` stores the closed `CameraProjection` enum, not an arbitrary `Projection` | Read | M6 |
| KI-12 | Assets: synchronous loading; PNG is the only decoder (sprites, meshes, skeletons, clips are code-built); no hot-reload watcher; `AssetId` is a name hash and changes on rename. The collision check is **done by T3** (commit `8abee2e`): `insert`, `load` and `reload` return `AssetError::IdCollision` and never overwrite. Remaining limit: the check is per asset type (see "Task T3 results") | Read | M17 |
| KI-13 | Skeletal animation has no blending, events, IK, skinning or file loading | Read | M11 |
| KI-14 | Blended sprites sort by quad centre only; large or intersecting blended quads can mis-order | Read | M5/M14 |
| KI-15 | ~~Input has 47 keys and no F-keys~~ Phase 1 maps F1–F12, punctuation, navigation, and the keypad, and mouse release is a one-frame edge. Phase 2 adds `Actions` and the fixed-step latch. Phase 4 adds cursor visibility, grab (locked, then confined), and focus-loss release of held input. Still open: gamepad, touch, and text. Device motion is winit's units, not unaccelerated HID. Relative motion and OS focus-loss were not observed on a window; the unit tests cover them | Done for M3b (gamepad, touch, and text stay deferred) | M3b (text and UI scale policy: M10, D-39) |
| KI-16 | ~~Two rustdoc warnings: unresolved `World` and `Schedule` links in `crates/engine/src/lib.rs`~~ **Fixed by T1** (commit `e08033e`); `cargo doc` with `-D warnings` is clean | Verified | M2 |
| KI-17 | `Cargo.toml` declares `MIT OR Apache-2.0` but no licence files exist. The licence is **proposed, not owner-confirmed** (OD-3); copyright holder unknown. Audit and the list of files that would change: "Licence audit" below | Verified (repository listing, `cargo metadata --locked`) | Owner decision, then M24 notices |
| KI-18 | ~~`Visibility::Hidden` applies to the entity only; it does not hide children~~ Extraction omits a sprite, mesh, tilemap, or light when it or any ancestor is `Hidden`. Local components are unchanged. Cameras still use `Camera.active` | Done (M4 Phase 2 checkpoint) | M4 |
| KI-19 | Performance is unmeasured: extraction is O(renderables) every frame, tilemaps expand per tile per frame, instance buffers are fully rewritten; the only timings available come from a software rasterizer | Read | M6 |
| KI-20 | ~~Platform: single window, `ControlFlow::Poll`, default present mode; no fullscreen/vsync configuration~~ Phase 4 adds borderless fullscreen, a vsync request with a surface-capability fallback, and an optional `WaitUntil` frame cap. Still one window. Uncapped frames still poll. Exclusive fullscreen and a frame pacer are not built. macOS showed Fifo with vsync on and Immediate with vsync off | Done for M3b | M3 |
| KI-21 | A sprite's natural size comes from the whole `SpriteAsset`, not from `Sprite.region`, so showing a sheet frame without also setting `size` stretches it to the asset's size | Read (`extract.rs`: `sprite.size.unwrap_or_else(|| asset.world_size())`) | M8 |
| KI-22 | `animate_system` writes sampled transforms directly onto bone entities and only visits entities that have a `SkeletonInstance`; `Pose` is not in the path, so there is no stage for blending or IK, and a skeleton-less sprite cannot be animated by it | Read (`clip.rs`) | M11 (sprite sheets: M8) |
| KI-23 | `Vertex` is position, normal and uv; no joints, weights or tangents; no model loader; meshes are code-built | Read (`mesh.rs`) | M12 |
| KI-24 | ~~The platform layer does not forward the window scale factor, and `Viewport` is one pixel size~~ M3b Phase 3 stores physical size, logical size, and a validated scale, and converts the physical pointer. Remaining: no text-input events, so UI text still has no input path, and reference-resolution fit modes are not built (D-39) | Partial (checkpointed locally as `9c99edf`) | M10 |
| KI-25 | Any change to a texture re-uploads the whole texture (cache keyed by `AssetId` and revision); there is no sub-rectangle update, which a glyph atlas needs | Read (`scene_renderer.rs`) | M5 |
| KI-26 | macOS dev machine (Apple M5, macOS 27.0): rustc **older than 1.99** cannot link anything, not even `fn main(){}`, with `ld: tapi error: malformed file ... libSystem.B.tbd ... unknown architecture arm64e.x1-macos`. Cause seen: `xcode-select -p` points at Xcode 26.4.1 (`ld-1266.8`) but `xcrun --show-sdk-path` returns the macOS 27.0 SDK from the Command Line Tools, which that linker cannot read; rustc 1.99.0 links fine (not investigated why). Host setup, not engine code; nothing was changed system-wide. Workaround used for the 1.89.0 and 1.90.0 runs: `SDKROOT=/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk`. Proper fix (user's decision): select a matching Xcode or CLT with `xcode-select`, or update Xcode | Verified (hello-world reproduction with 1.89.0 and 1.90.0 fails, with the `SDKROOT` above succeeds, 1.99.0 succeeds without it) | Environment |
| KI-27 | ~~The MSRV (1.90) was only built on macOS arm64~~ **Verified on all three systems** by the CI `msrv` job: `cargo +1.90.0 test --workspace --locked`, 138 passed, 0 failed on x86_64 Linux, aarch64 macOS and x86_64 Windows (MSVC). The MSRV holds only with the committed `Cargo.lock` | Verified (run 38475229601) | Done |
| KI-28 | Six views are now rendered by example-local constructors in `spatial_scene` (evidence below). Still true: no engine preset API, no 2D-style game, no custom projection (KI-11). `Upright` is edge-on from straight above and foreshortened in a steep view; that is observed, not accepted as the design (D-26). Coplanar cutout content is ordered by draw order or by depth-test rounding, and `RenderOrder` still orders only blended sprites (D-48, three verified conflicts). The ground plane stays `XZ` (provisional, ADR-0004) | Verified for the demonstration and the conflicts (Apple M5, 2026-10-14). The remaining gaps are unread as designs | M6 |
| KI-29 | Logging and `Diagnostics` exist (M3a-4). M4 Phase 1 adds a text hierarchy dump and an entity dump. Still absent: asset validation, a depth or overdraw view, and a visual inspector | Partial (text dump implemented locally, not pushed) | M5, M6, M20 |

## M6 test scene (2026-10-14)

One task of M6, committed as `6dbabb8` and pushed to `origin/main` before M3a-1. That revision is the
rendering regression baseline: later M3a screenshot checks compare `basic_2_5d` and `spatial_scene`
(perspective and orthographic) to it. No engine crate changed for the scene. Constructors stay in
`examples/spatial_scene` (`cameras.rs`): `perspective`, `orthographic`, `isometric`, `top-down`, `steep`,
`side`. Each returns a `Camera` and a `Transform`. There is no engine preset API.

Environment: the macOS machine in the 2026-10-13 record (Apple M5, Metal, rustc 1.99.0). `scripts/check.sh`
sets `DIMORA_REQUIRE_GPU=1`.

| Check | Result |
|---|---|
| `scripts/check.sh` after the documentation pass (2026-10-14) | Exit 0. All 4 steps: fmt, clippy `-D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (**158 passed, 0 failed**, including 18 GPU tests in `headless.rs` and 16 in `configurations.rs`; none skipped), rustdoc `-D warnings` |
| `cargo test -p spatial_scene` | 4 tests in `cameras.rs`, 16 in `tests/configurations.rs`, 0 failed |
| Headless screenshots | `--camera` for all six names, 1280×720, exit 0. Images: `docs/spatial_scene_m6_*.png` |
| `--frames 30` (isometric, release) | GPU cache 4 textures and 5 meshes after frame 1 and after frame 30, exit 0 |
| Windowed run | **Not done** |
| Other machines | **Not done.** The two coplanar splits that name a camera were read on this GPU only; the tests lock the split, not the camera's name |

What the 16 tests cover: one placed world under every configuration; project then pick; the walker moves on
its line and is drawn there; `Upright` height is `cos(pitch)` in the orthographic views and collapses
top-down; a nearer mesh occludes a farther sprite and the reverse, in every configuration; orientation modes;
decals a unit apart resolve by depth; the three conflicts named in `docs/SPATIAL_RENDERING.md`.

Not claimed: blended sprites interleaving with meshes under every configuration (the older single-camera GPU
test still covers one camera); two viewports; camera-to-texture; a custom projection; an engine preset.

## Licence audit (2026-10-13)

**Status: the licence is proposed, not confirmed.** `MIT OR Apache-2.0` is what `Cargo.toml` declares; the
owner has not approved it (OD-3 in [docs/DECISIONS.md](docs/DECISIONS.md#owner-decisions)). Nothing below
adds or changes a licence file. Findings come from reading the repository and from `cargo metadata --locked`
on the committed `Cargo.lock` (all targets, so it includes Windows, macOS and Linux-only crates).

**Declarations that exist.** One: `license = "MIT OR Apache-2.0"` in `[workspace.package]` of the root
`Cargo.toml`, inherited by all 11 members with `license.workspace = true`. `publish = false` is set. There
is no `authors`, `repository` or `homepage` field. No SPDX or copyright header appears in any source file,
shader or document, and no README, ROADMAP, TASKS or docs file states a licence. The only prose mention is
KI-17 itself.

**Files missing.** `LICENSE-MIT`, `LICENSE-APACHE` (or a single `LICENSE`), any `NOTICE`, and any
third-party-licence file. No licence file exists anywhere in the tree.

**Copyright holder.** Unknown. The history has three author names from the same person plus the Cursor
agent (`Cursor Agent`, 23 commits). Who holds copyright, and whether agent-written code needs a statement or
a contributor policy (OD-5), is an owner question; the MIT licence text needs a named holder.

**Third-party code.** None is vendored: every dependency comes from the crates.io registry (no git or path
sources outside the workspace). No fonts, audio, models or third-party images are in the repository; the five
PNGs in `docs/` are screenshots the project's own examples generate, and the example art is drawn in code
(`art.rs`).

**Dependency licences (271 external packages in the locked graph; none without a declared licence; the rows below do not sum to 271 because the remaining packages use other OR sets of MIT, Apache-2.0, BSD, Zlib, 0BSD and Unlicense).**

| Licence expression (SPDX, as declared) | Packages |
|---|---|
| `MIT OR Apache-2.0` (either order, `/` spelling too) | 156 |
| MIT only | 61 |
| Apache-2.0 only | 10: `ab_glyph`, `ab_glyph_rasterizer`, `owned_ttf_parser`, `codespan-reporting`, `gethostname`, `gl_generator`, `glutin_wgl_sys`, `khronos_api`, `spirv`, `winit` |
| Zlib only | `foldhash`, `slotmap`, `zlib-rs` (Zlib also appears in several OR sets with MIT and Apache-2.0) |
| BSD-3-Clause only | `tiny-skia`, `tiny-skia-path` (reached through winit's Linux window decorations) |
| BSD-2-Clause only | `arrayref` |
| ISC | `libloading` |
| Unlicense or MIT | `byteorder-lite`, `memchr`, `termcolor`, `winapi-util` |
| `Apache-2.0 AND MIT` | `dpi` (both apply) |
| `(MIT OR Apache-2.0) AND Unicode-3.0` | `unicode-ident` (Unicode data notice applies) |
| `MIT OR Apache-2.0 OR LGPL-2.1-or-later` | `r-efi` (two versions; the permissive options are available, so LGPL need not apply) |

**Concerns.** None found that conflicts with `MIT OR Apache-2.0`: there is no GPL, AGPL or LGPL-only
dependency, no unlicensed crate and no non-registry source. Things that need handling **before a binary is
distributed**, not before the licence files exist: (1) MIT, BSD, ISC, Zlib and Unicode-3.0 require their
copyright and licence text to accompany binary distributions, and Apache-2.0 requires NOTICE files to be
carried, so a third-party notices file (generated, for example, with `cargo-about`) is needed for M24;
(2) `cargo-deny` or an equivalent licence allow-list in CI would keep the finding true; neither is added;
(3) the Apache-2.0-only crates are compatible with distributing under either option, but they mean the
combined binary is effectively under Apache-2.0 terms for that code; (4) runtime graphics drivers (Mesa
lavapipe, system Vulkan) are not bundled and are out of scope; (5) the dependency list is only as complete
as the lockfile and the SPDX fields declared by the crates, which were not independently verified.

**Files that would change to make the repository consistent, once the owner approves a licence:**

| File | Change |
|---|---|
| `LICENSE-MIT` (new) | MIT text with the confirmed copyright holder and year |
| `LICENSE-APACHE` (new) | Apache-2.0 text |
| `README.md` | A "Licence" section: the dual licence and the contribution clause |
| `Cargo.toml` | Optionally `authors`, `repository`; `license` stays or changes to the approved expression; add `license-file` only if a custom licence is chosen |
| `THIRD_PARTY_NOTICES` (new, before the first release) | Generated notices for dependencies (M24) |
| `docs/VISION.md`, `docs/CAPABILITIES.md`, `docs/DECISIONS.md` | Replace "proposed" with "confirmed" in OD-3 and the open questions |
| `TASKS.md` | Close KI-17 |
| `.github/workflows/ci.yml` | Optionally add a licence allow-list check (`cargo-deny`) |
| Contributor policy (new, if OD-5 says so) | A DCO or CLA statement in `CONTRIBUTING.md` |

## Blockers

| ID | Blocker | Effect |
|---|---|---|
| B-1 | ~~Code host and CI provider are undecided~~ The repository and its pull requests are on GitHub (`Dimora-Engine/dimora`) and CI runs on GitHub Actions (ADR-0005, Provisional). The owner has not stated GitHub as the long-term host | Resolved in practice; owner confirmation outstanding |
| B-2 | GPU tests need an adapter: Metal on macOS (always present on a Mac), a Vulkan driver on Linux; fresh Linux VMs do not have one (`sudo apt install mesa-vulkan-drivers`) | Resolved by T1: documented in README, and `DIMORA_REQUIRE_GPU=1` (set by `scripts/check.sh`) makes the absence a failure |
| B-3 | ~~D-03 (despawn semantics) and D-04 (GPU cache lifetime) are unmade~~ **Resolved 2026-10-13**: the owner accepted the proposed defaults for both (recorded in DECISIONS.md and ARCHITECTURE.md) | None |

## M2 tasks T1 to T3 (done)

All three were inside M2, ordered by dependency and by how much they unblock. **T1, T2 and T3 are done
(2026-10-13, macOS arm64; evidence above).** D-03 and D-04 were accepted by the owner on 2026-10-13. No numbered
task is left in M2; what remains is under "Remaining work in M2" (licence files, CI). After M2 the roadmap continues with M3 (input and scheduling); sprite-sheet
animation (M8) cannot start before M3 and M5.

### T1. Reproducible toolchain and one verification command: DONE

Done on macOS arm64 with the deviations listed in "Task T1 results" (no Linux/lavapipe run; the no-adapter
path was verified with `WGPU_BACKEND=vulkan` instead of `VK_ICD_FILENAMES`). The criteria below are the original text.

**Dependencies:** none. D-23 (MSRV policy) is decided in this task by building, not in advance.

Why first: every later task is verified through it, and KI-1/KI-2 mean today's "tests pass" is not
fully trustworthy on a fresh machine.

**Acceptance criteria (all testable):**

1. `rust-toolchain.toml` pins a channel, and `[workspace.package] rust-version` holds an MSRV that has
   been verified by building and running `cargo test --workspace` on exactly that version. The
   verified version is written in this file.
2. `scripts/check.sh` runs, in order, `cargo fmt --check`, `cargo clippy --workspace --all-targets --
   -D warnings`, `cargo test --workspace` and `cargo doc --workspace --no-deps` with
   `RUSTDOCFLAGS="-D warnings"`. It stops with a non-zero exit code on the first failure and prints
   which step failed. It exits 0 on a clean checkout of `main` on a machine with lavapipe.
3. The GPU test setup reads `DIMORA_REQUIRE_GPU`. When it is `1` and no adapter exists,
   `cargo test -p engine-renderer-wgpu` fails with a message that names the missing driver. When it is
   unset, behaviour is unchanged (skip message). `scripts/check.sh` sets it. Verified by running with
   `VK_ICD_FILENAMES=/nonexistent` (no adapter): fails with the variable, skips without it.
4. The two rustdoc warnings in `crates/engine/src/lib.rs` are fixed, and `cargo doc` produces none.
5. `README.md` states the required toolchain and how to install a software Vulkan driver, and points to
   `scripts/check.sh`.
6. No new dependencies. All 99 existing tests still pass.

### T2. GPU resource lifetime and `AssetManager::remove` (KI-4): DONE

Done on macOS arm64 (2026-10-13); see "Task T2 results" for the evidence and its limits.

**Dependencies:** T1 (verification through `scripts/check.sh`); D-04 accepted (2026-10-13).

**Acceptance criteria:**

1. `AssetManager::remove::<T>(handle)` removes the entry and its dependency links; `get`/`state` then
   report unknown.
2. `SceneRenderer` evicts a cached texture or mesh when its asset is gone, and (per D-04) when it has
   not been used for a configurable number of frames. It exposes counts of cached textures/meshes.
3. A headless test renders a scene, removes the assets and renders again, and shows the cache counts
   return to the empty baseline; re-inserting an asset re-uploads it correctly (pixel check).
4. A test shows a replaced asset (revision bump) is re-uploaded without leaking the old GPU copy.
5. Both examples still render, and `spatial_scene` shows no growth in cache counts over 1,000 frames.

### T3. Hierarchy robustness and asset-id collision guard (KI-7, KI-8, KI-12 part): DONE

Done on macOS arm64 (2026-10-13); see "Task T3 results" for the evidence and its limits. The criteria below are the original text.

**Dependencies:** T1; D-03 accepted (2026-10-13). Independent of T2.

**Acceptance criteria:**

1. A test fails on the current code and passes after the change: a child of an entity with no
   `Transform` has the correct `GlobalTransform` (the parent counts as identity).
2. `World::despawn` on an entity with children follows D-03 (no child retains a `Parent` pointing at a
   dead entity), covered by tests for the leaf, middle and root of a three-level chain.
3. `AssetManager::insert`/`load` returns an error (and never overwrites) when a different name hashes
   to an existing id for the same asset type; a test forces a collision using two names and a test-only
   hook, or `AssetId` construction with a chosen value.
4. Existing hierarchy tests are unchanged and pass.

## Carried from M2 (owner input needed)

- Licence files matching `Cargo.toml` (KI-17): the licence is **proposed, not confirmed**; the owner must
  approve it and name the copyright holder (OD-3, OD-5) before any licence file is added. See "Licence audit".
- CI: the GitHub Actions workflow exists (ADR-0005, D-22 Provisional); results are in "CI results". The
  owner still has to confirm GitHub as the long-term host.
- The capability audit table now lives in [docs/CAPABILITIES.md](docs/CAPABILITIES.md); update its rows in
  the same commit as any change that moves a system's status.

## Next tasks: M3a, the frame model

Ordered by dependency. Each task is small enough to verify on its own. The milestone-level acceptance
criteria (ordering test, command and event tests, interpolation test, logging test, byte-identical example
screenshots against `6dbabb8`, ADRs) are in ROADMAP.md under M3a and are not repeated here. M3a is
complete locally and is not pushed. M3b is complete locally and is not pushed.

| Task | Content | Prerequisite | Done when |
|---|---|---|---|
| M3a-1 | **Design note, prototype and ADRs** for D-44, D-01, D-10, D-45, D-47, D-21, and D-02 ("deferred to profiling" is acceptable). A throwaway prototype of stages plus a command buffer is run against the `RefCell` resources (KI-9). No engine code is merged | Owner approval | **Done 2026-10-14.** Evidence under "M3a-1 results". The prototype is not in the tree |
| M3a-2 | Named stages and `before`/`after` ordering; `Game::step` stops hard-wiring engine stages (KI-10) | M3a-1 | **Done.** Commit `312a05f`, plus the lifecycle-anchor seal. Evidence under "M3a-2 results" |
| M3a-3 | `Commands` flushed at stage boundaries; typed event channels cleared per frame (KI-9) | M3a-2 | **Done.** Committed locally, not pushed. Evidence under "M3a-3 results" |
| M3a-4 | Logging facade; asset failure, skipped renderable and surface loss each log once with the cause named (R-13). `Diagnostics` resource | M3a-1 | **Done.** Committed locally, not pushed. Evidence under "M3a-4 results" |
| M3a-5 | Fixed-step alpha exposed and opt-in transform interpolation (R-3) | M3a-2 | **Done, committed locally, not pushed.** 60 Hz fixed versus 144 Hz render test; simulation still reads the un-interpolated `Transform`. Evidence under "M3a-5 results" |
| M3a-6 | No new public `hecs` types. Review the `engine` re-exports against ADR-0010 | M3a-1 | **Done, committed locally with this review, not pushed.** No new public `hecs` type, no new `hecs` re-export, no `wgpu` or `winit` type on a new public interface. Evidence under "M3a-6 results" |

M3b phases are listed under "M3b". M4, M5 and M6 are the next
milestones; M7 (collision) and M8 (sprite sheets) follow M3a, M4, M5 and M6, then M9 (stable character and
scene API with a playable slice).

## M3b. Input and platform

Decisions: [ADR-0013](docs/adr/0013-input-actions-and-window-controls.md) (D-49). D-39 stays open for UI
capture, focus, and text. `basic_2_5d` stays on raw `Input`. `spatial_scene` moves to actions in Phase 2.

| Phase | Content | Done when |
|---|---|---|
| 1 | Wider physical key map and mouse just-released, on the existing frame clock | **Checkpointed locally.** Evidence under "M3b Phase 1 results" below |
| 2 | `Actions`, aggregate edges, axes, rebinding, and the fixed-step latch | **Checkpointed locally.** Evidence under "M3b Phase 2 results" below |
| 3 | `Viewport` physical size, logical size, and scale; `ScaleFactorChanged` | **Checkpointed locally as `9c99edf`.** Evidence under "M3b Phase 3 results" below |
| 4 | Borderless fullscreen, vsync fallback, event-loop frame cap, cursor grab, focus-loss release | **Checkpointed locally as `cfdf358`.** Evidence under "M3b Phase 4 results" and "M3b Phase 4 review" below |

## M3a-1 results (2026-10-14)

Owner approval of the seven decisions and of the two implementation directions (fixed-step delta, opt-in
extract-only interpolation). Docs and ADRs only. No engine crate changed. Committed locally as `9876f7d`
and not pushed.

**Baseline.** `6dbabb88f10779301115d04720ac1f52000ed03a` (`6dbabb8`, "feat(examples): add M6 spatial scene
and camera validation") is the parent of that commit and matches `origin/main`. Engine and example code
at `9876f7d` are identical to `6dbabb8`. Later screenshot checks compare against that revision.

**ADRs.** ADR-0006 (D-44, Settled), ADR-0007 (D-01, Settled), ADR-0008 (D-10, Settled), ADR-0009 (D-02,
Provisional), ADR-0010 (D-45, Settled), ADR-0011 (D-47, Settled), ADR-0012 (D-21 logging, Settled).
Command order, event lifetime and the unsupported interpolation cases are in ADR-0007 and ADR-0008.

**Throwaway experiment.** A program outside the repository, linked against `engine-world` and
`engine-math`, then deleted. Findings, also in ADR-0008:

* A `query` borrow and `resource_mut` on a queue resource succeed together. One command was appended during iteration.
* `world.spawn` in that same loop does not compile (`E0502`). The flush has to run after the query is dropped.
* A FIFO flush through the real `despawn`, `insert` and `set_parent` kept `Parent` / `Children` consistent: last write wins; a command after despawn fails; a command queued before despawn applies, then the entity is gone; a child command after the parent's despawn fails; a `set_parent` cycle fails and the old parent remains.
* Two overlapping `resource_mut` borrows of the same queue still panic. The implementation must not let that `RefMut` escape.

**Not a blocker for M3a-2.** Stages can be built on the existing `Schedule`. Commands wait for M3a-3.
The `log` dependency waits for M3a-4. Interpolation waits for M3a-5.

**Repository check.** `scripts/check.sh` on 2026-10-14, exit 0. All 4 steps: fmt, clippy `-D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (**158 passed, 0 failed**; the 18 tests in `headless.rs` and the 16 in `configurations.rs` ran, none skipped), rustdoc `-D warnings`. Docs only; this does not re-verify a frame-model change, because none was made.

## M3a-2 results (2026-10-14)

Named stages and `before` / `after` on the existing `Schedule`. No commands, events, interpolation,
logging, or parallel execution. Committed as `312a05f`. The lifecycle-anchor seal is the change on top of that commit.

**API.** `Stage` is `time`, `loads`, `fixed_update`, `update`, `animate`, `post_update`, `input_clear`.
`Game::add_system` and `Game::add_fixed_system` keep their signatures and append to `update` and
`fixed_update`. `add_to_stage`, `add_to_stage_before` and `add_to_stage_after` return `ScheduleError`.
`Schedule::add_system` now returns `Result`. A duplicate name, a missing target, a self-dependency or a
cycle is rejected and the schedule is left unchanged. Unconstrained systems keep registration order.
When several systems are ready, the earlier registration runs next. During each fixed iteration
`Time::delta` is the fixed step; it is restored before `update`. `elapsed` and `frame` advance once
per `step`. Input edges last until `input_clear`. Extract and render stay outside `step`.

Built-ins, and where an unconstrained system sits relative to them: `advance_time`, `process_loads` and
`animate` run first in their stages; `propagate_transforms` and `end_input_frame` run last.
`advance_time`, `process_loads`, `propagate_transforms` and `end_input_frame` are sealed there: an
explicit `before` / `after` on the closed side returns `MustRunAfter` or `MustRunBefore` and does not
change the schedule. `animate` stays open, so a system may be placed before or after `animate_system`.
The frame-delta restore sits in `Game::step` between the fixed loop and `update`, so system order
cannot move it.

**Screenshots.** Byte-identical to images rendered from the engine and example code at `6dbabb8` (the M3a-1 commit `9876f7d` does not change those files). Same commands, 1280×720, debug: `basic_2_5d`, `basic_2_5d --ortho`, `spatial_scene --camera perspective`, `spatial_scene --camera orthographic`. `cmp` reported all four identical. SHA-256: basic `c3c5874a…`, ortho `f2aedc0c…`, perspective `ec5f6252…`, orthographic `3551d84f…`.

**Repository check.** `scripts/check.sh` on 2026-10-14, exit 0. All 4 steps: fmt, clippy `-D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (**174 passed, 0 failed**; 18 in `headless.rs` and 16 in `configurations.rs` ran, none skipped), rustdoc `-D warnings`. The new tests are 9 schedule tests in `engine-world` and 7 in `crates/engine/tests/frame.rs`, including `lifecycle_builtins_stay_at_the_ends_of_their_stages`. Examples `basic_2_5d` and `spatial_scene` compiled and their existing tests passed. Extract and render were not changed, so the screenshot comparison above still stands.

## M3a-3 results (2026-10-14)

Deferred commands and typed events on the existing `World` and `Schedule`. No parallel scheduling, no
replacement ECS, no change detection, no physics, no animation-event subsystem, no logging, no
interpolation. Committed locally with this change. Not pushed.

**Commands.** `World::commands` returns a `CommandWriter` that can append during a query. The writer
does not hold the queue borrow; each call borrows it only for the append, so a second writer in the
same system is safe. `try_commands` returns `None` when that borrow would panic. A missing queue is
still a panic. `spawn` reserves an id immediately and materializes it at flush. The other operations
are `despawn`, `insert`, `remove`, `set_parent` and `detach_children`. Flush order is call order.
Each failure is a `CommandFailure` (the `CommandError` and the stage name when `Game::step` flushed
it) on `command_failures`, and the queue continues. A failed command writes nothing. The list stays
readable after `step` returns and is cleared at the start of the next step. Flush points are the end
of every stage run and each fixed iteration. `Parent` and `Children` cannot be inserted as ordinary
components; `remove::<Parent>` unlinks without rewriting the local transform, and `remove::<Children>`
detaches children at their world transform.

**Events.** `World::send::<T>` appends. `World::events::<T>` returns a copy of the promoted events in
send order (`T: Clone` to read, `T: Send` to send). A send is invisible for the rest of that stage,
including later systems in it. It becomes visible to later stages once the stage run finishes.
`fixed_update` promotes once, after the whole loop, so iterations do not see each other's events.
`Game::step` clears every queue after `input_clear`. A send in `input_clear` is not delivered later
in the step or on the next step. `discarded_events` counts events that were still pending at that
clear, until the next step starts. Promoted events are not counted. `input_clear` stays a stage
systems can join. Extract and render do not read events.

**Screenshots.** Re-rendered after the review edits and compared with `cmp -s` against `/tmp/m3a-baseline` (the images rendered from the engine at `6dbabb8`, SHA-256 recorded under M3a-2). Debug, 1280×720: `basic_2_5d`, `basic_2_5d --ortho`, `spatial_scene --camera perspective`, `spatial_scene --camera orthographic`. All four identical. SHA-256 unchanged: basic `c3c5874adeea119c98255c6c1d08208c0244224d930d86e8736c280ef88b38fe`, ortho `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f`, perspective `ec5f6252a88f642b454ae768f9eec8cdd5fba88a5aeb7a424fcefe5fda12275e`, orthographic `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde`.

**Repository check.** `scripts/check.sh` on 2026-10-14, after the review edits, exit 0. All 4 steps: fmt, clippy `-D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (**191 passed, 0 failed**; 18 in `headless.rs` and 16 in `configurations.rs` ran, none skipped), rustdoc `-D warnings`. `cargo test --workspace -- --list` reports 191 tests. New tests against M3a-2: 10 command tests and 1 event test in `engine-world`, and 6 in `crates/engine/tests/deferred.rs`. The review added the two-writer, query-plus-writer, `try_commands`, panic, discarded-event and end-of-step failure tests. Examples `basic_2_5d` and `spatial_scene` compiled and their existing tests passed.

## M3a-4 results (2026-10-14)

Logging and a `Diagnostics` resource. No profiler, no sample history, no per-system timings, no GPU
timestamps, no overlay, no interpolation. Committed locally, not pushed.

**Logging.** Workspace dependency `log` 0.4. `Game::run` installs a stderr logger at Info only when
`log::set_logger` succeeds, and does not change an existing logger's filter. A failed asset load logs
one warning naming the asset at the transition to `Failed`. A later `process_loads` does not repeat it.
A new failing load does. A load that is still `Loading` does not log. Extraction logs a skipped sprite,
mesh, or tilemap once per asset id when the asset is missing, failed, or invalid, and remembers at most
64 ids without rotating. A later successful extract forgets the id. A lost or outdated surface logs one
warning and the renderer reconfigures; timeout and occlusion are not logged. Window initialization and
render errors log the cause and the runner still exits. Each `CommandFailure` is logged once at the end
of `step`. The list stays readable. The next step clears it before systems run, so an old failure is not
logged again. Discarded events are counted and not logged.

**Diagnostics.** `Diagnostics` and `StageDurations` live in the `engine` crate. `frame` is the simulation
`dt`. `stages` is CPU time from `Instant` around each world stage. `stages.fixed_update` is the whole
fixed loop, not the fixed timestep. `fixed_steps` is the iteration count. `render` is the latest
successful `RenderStats` and is not written by `step`. Examples print entity count, fixed steps, and
CPU step time in the window title they already update.

**Screenshots.** Re-rendered after this change and compared with `cmp -s` against `/tmp/m3a-baseline` (the images rendered from the engine at `6dbabb8`). Debug, 1280×720: `basic_2_5d`, `basic_2_5d --ortho`, `spatial_scene --camera perspective`, `spatial_scene --camera orthographic`. All four identical. SHA-256 unchanged: basic `c3c5874adeea119c98255c6c1d08208c0244224d930d86e8736c280ef88b38fe`, ortho `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f`, perspective `ec5f6252a88f642b454ae768f9eec8cdd5fba88a5aeb7a424fcefe5fda12275e`, orthographic `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde`.

**Repository check.** `scripts/check.sh` on 2026-10-14, exit 0. All 4 steps: fmt, clippy `-D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (**204 passed, 0 failed**; 18 in `headless.rs` and 16 in `configurations.rs` ran, none skipped), rustdoc `-D warnings`. `cargo test --workspace -- --list` reports 204 tests. New tests against M3a-3: failed-load warning, bounded missing-asset memory, three extract warning tests, surface-loss warning, window and render error logs, application-logger retention, default stderr install, command-failure log, discarded-event silence, stage timings, and render statistics after `render_to_rgba`.

## M3a-5 results (2026-10-14)

Opt-in fixed-step interpolation at extraction. Commit `523f553`, local, not pushed.

**API.** `Interpolate` on an entity stores `previous: Option<Transform>`. `Interpolate::snap` sets that previous local to the transform the caller passes. `snapshot_interpolation` copies the current local into `previous` for every entity that has both. `Game::step` calls it at the start of each fixed iteration, before fixed-update systems. It is not a schedulable system. `FixedTimestep::alpha` is the accumulator divided by the step, clamped to a finite value in `0.0..=1.0`. A zero step and the eight-step stall leftover are `0.0`.

**History.** One previous local per marked entity. No world copy and no animation timeline. Several fixed iterations in one frame leave `previous` as the local from before the last iteration. With no stored previous anywhere, extraction keeps using `GlobalTransform`.

**Math.** `Transform::lerp`: linear translation, linear scale, `Quat::slerp` on the shortest arc. A scale sign change passes through zero. A rotation of 180 degrees is whatever `Quat::slerp` returns and is not special-cased. A non-finite previous, current, alpha, or blend result renders the current local and does not change the simulation.

**Hierarchy.** The render matrix is the product of render-locals up the parent chain, matching `propagate_transforms`. An unmarked child follows a marked parent's blended local. A transform-less node stays identity. `set_parent` keeps the child's local and history, so extraction is the new parent composed with the local blend, and the entity pops. Removing `Parent` leaves both samples as the old local numbers. `detach_children` rewrites each direct child's local into world space and sets that child's `previous` to the rewritten local. Descendants are not rewritten. Despawning a parent removes the interpolated subtree. `snap` is still the explicit teleport.

**Lifecycle.** A new entity with no previous renders at its current local. A spawn flushed after the snapshot waits until the next fixed iteration for history. Despawn removes the component, including when the despawned entity is the parent of an interpolated child. Adding the marker with `previous: None` changes nothing until a snapshot. Removing it restores the `GlobalTransform` path. A variable-step write of a marked local is included in the blend. That mixing is not supported. `update` and `fixed_update` still read `Transform`.

**Cameras.** The same matrix feeds sprites, meshes, tilemaps, lights, and the active camera, for billboard, upright, and fixed sprites and for perspective, orthographic, and isometric placements. A camera interpolates only when it or an ancestor has a stored previous local. The examples do not mark one. Depth sorting is unchanged.

**Screenshots.** Re-rendered after this change and compared with `cmp -s` against `/tmp/m3a-baseline` (the images rendered from the engine at `6dbabb8`). Debug, 1280×720: `basic_2_5d`, `basic_2_5d --ortho`, `spatial_scene --camera perspective`, `spatial_scene --camera orthographic`. All four identical. SHA-256 unchanged: basic `c3c5874adeea119c98255c6c1d08208c0244224d930d86e8736c280ef88b38fe`, ortho `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f`, perspective `ec5f6252a88f642b454ae768f9eec8cdd5fba88a5aeb7a424fcefe5fda12275e`, orthographic `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde`. The examples do not add `Interpolate`.

**Repository check.** `scripts/check.sh` on 2026-10-14, after the hierarchy review, exit 0. All 4 steps: fmt, clippy `-D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (the interpolation GPU test ran, none skipped), rustdoc `-D warnings`. `cargo test --workspace -- --list` reports **237 tests**. New tests against M3a-4: alpha across zero, one, and many steps and the stall cap; translation, quaternion, and scale blends, including a short arc near 180 degrees, a scale sign change, and a non-finite history; parent-child composition; teleport via `snap`; `detach_children` snapping the rewritten child and leaving the grandchild's blend; `set_parent` and removing `Parent` keeping both locals; reparenting an interpolated ancestor; despawn of a parent; spawn, despawn, and gaining or losing the marker; fixed-update and variable-update reads; the 60 Hz motion sampled at 144 Hz; and a GPU frame where an interpolated sprite and mesh move while an unmarked sprite does not.

**GPU.** `interpolated_sprites_and_meshes_move_on_the_gpu_between_fixed_steps` rendered 64×64 frames. At alpha 0 the marked sprite and mesh sit on the previous pose. After a half step with no new fixed update they move toward the simulation pose and stay short of an unmarked sprite that was already there. A perspective camera on the same world keeps that separation. Extraction of the mesh matrix was checked in the same test, and the simulation transforms were unchanged after `render_to_rgba`.

## M3a-6 results (2026-10-14)

Public-API review against ADR-0010. No signature was removed or renamed. `docs/API_SURFACE.md` was not written.

**Contract.** `Interpolate`, `FixedTimestep::alpha`, `CommandWriter`, `CommandError`, `CommandFailure`, caller-defined events (`World::send`, `World::events`), `Diagnostics` and `StageDurations`. These are plain data or methods on plain data. The ones a game imports are in `engine::prelude`.

**Not in the prelude.** `snapshot_interpolation` is public on `engine-world` because `Game::step` lives in another crate. The render-pose walk (`RenderPoses`) is `pub(crate)` in `engine-renderer`.

**Internal, unchanged.** `pub use hecs::{Entity, EntityRef, Query, QueryBorrow}` and `World.entities`. `Entity` stays in the prelude. `CommandWriter::spawn` and `insert` take `hecs::DynamicBundle`, the same trait `World::spawn` already takes. `pub use wgpu` and `pub use winit` stay, and the facade still re-exports those crates. None of the new M3a types names `wgpu` or `winit`.

**Acceptance, re-checked against the suite that `scripts/check.sh` runs.** Ordering and cycles: `stages_run_in_order_and_builtins_stay_at_their_anchors`, `a_cycle_is_rejected_and_run_keeps_the_previous_order`. Commands: `a_queued_spawn_stays_invisible_until_the_stage_ends`. Events: `events_cross_stage_boundaries_and_not_fixed_iterations_or_the_next_frame`. Interpolation: the 60 Hz / 144 Hz test named in ROADMAP, plus the hierarchy tests in M3a-5. Logging: `a_failed_load_warns_once_per_occurrence_and_names_the_asset`. Screenshots: the four images under M3a-5, compared with `cmp -s` to `6dbabb8` after the hierarchy snap. This review did not change extraction, so those images were not re-rendered. ADRs 0006 through 0012 exist. D-02 stays deferred to profiling (ADR-0009).

**Repository check.** `scripts/check.sh` on 2026-10-14, after these comments, exit 0. All 4 steps passed, including rustdoc `-D warnings`. `cargo test --workspace -- --list` still reports 237 tests. The GPU tests were not skipped.

## M3b Phase 1 results (2026-10-15)

Wider physical keys and a mouse release edge. Checkpointed locally after the modifier correction below. Phases 2–4 are not started in that commit. The fixed-step latch is specified in ADR-0013 and is not built there.

**Keys.** `Key` adds F1–F12, US punctuation positions, Backspace, Delete, Insert, Home, End, PageUp, PageDown, and the numeric keypad. `NumpadEnter` is not `Enter`. Left and right Shift, Control, and Alt stay one variant. `map_key` still uses physical `KeyCode`s. F13 and unidentified codes stay unmapped. OS key repeat is still dropped before `Input`. A second `press` while a key is down still does not set another edge.

**Collapsed modifiers.** Holding both physical Shift keys and releasing one used to emit `Key::Shift` released while the other side was still down. The platform now counts physical downs for Shift, Control, and Alt. The logical key goes down on the first side and up on the last. A side that does not change the logical key emits nothing. Other keys still emit every non-repeat event. This is a behavior change from the earlier Phase 1 working tree, recorded in ADR-0013.

**Mouse.** `mouse_released` is set only when a held button transitions up, and `end_frame` clears it with the other frame edges. `mouse_pressed` and `key_pressed` are unchanged. A press and release before `end_frame` leave both key edges set and the key up.

**Examples.** `basic_2_5d` and `spatial_scene` still read raw `Input`. Their keys were already mapped. Extraction is unchanged.

**Repository check.** `scripts/check.sh` on 2026-10-15, before the modifier correction, exit 0: 239 tests, GPU tests ran. The four screenshots against `6dbabb8` were byte-identical. After the correction, `scripts/check.sh` on 2026-10-15 exited 0 again (fmt, clippy `-D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1`, rustdoc `-D warnings`). Two tests were added: `collapsed_modifier_stays_down_until_both_sides_release` and `collapsed_modifiers_are_independent_and_other_keys_pass_through`, so the suite is 241 tests. The correction does not change a frame with no input.

## M3b Phase 2 results (2026-10-15)

Fixed-step latch and `Actions`. Checkpointed locally as `83286a9`. Phase 3 is checkpointed locally as `9c99edf`. The review before the Phase 2 checkpoint added `zero_steps_keep_the_latch_and_rebinding_waits_until_publish`. No latch behavior changed.

**Latch.** A real press or release sets the frame edge and stores a fixed-step bit. `end_frame` clears only the frame edge. `Game::step` publishes the stored bits at the start of the step. After the first fixed iteration's systems and command flush it conceals them, so later iterations read false. A step with zero fixed iterations does not conceal. A transition after conceal stays stored until the next publish, and queries keep reading the published set, so they do not change meaning by stage. A read does not clear a bit. Several transitions before publish collapse to one press bit and one release bit. `key_down` and `mouse_down` stay live.

**Actions.** `Actions` and `ButtonSource` live in `engine-core` and in `engine::prelude`. Names are `&'static str`. `bind_button` / `set_button` cover keys and mouse buttons. `bind_axis` / `set_axis` build 1D axes; `axis2` is two of those. `down`, `pressed`, and `released` are the frame view. `fixed_pressed` and `fixed_released` use the same publish and conceal calls as raw input. `Game::send_input` syncs the aggregate after each device event. The first binding down presses once. Another binding while it is held does not. Releasing one of several held bindings does not release. The last one does. A tap in one frame keeps both edges. Rebinding compares the aggregate with the held snapshot from the last sync. Opposing axis sides read as `0`. A second `press` of a key that is already down does not press the action again.

**Examples.** `spatial_scene` binds its existing camera and gameplay keys to actions and still reads scroll from `Input`. `basic_2_5d` still reads raw `Input`.

**Public API.** New: `Input::fixed_key_pressed`, `fixed_key_released`, `fixed_mouse_pressed`, `fixed_mouse_released`, `publish_fixed_edges`, `conceal_fixed_edges`; `Actions`, `ButtonSource`, and their bind, query, sync, `end_frame`, publish, and conceal methods. `Game::new` inserts `Actions`. `Game::send_input` syncs it. Frame-edge methods are unchanged.

**Repository check.** `scripts/check.sh` on 2026-10-15, exit 0. All 4 steps: fmt, clippy `-D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (GPU tests ran, none skipped), rustdoc `-D warnings`. `cargo test --workspace -- --list` reports **255 tests**, including `zero_steps_keep_the_latch_and_rebinding_waits_until_publish`.

**Screenshots.** Release renders, compared with `cmp -s` to the same commands at `6dbabb8`. All four were byte-identical: `basic_2_5d` (746071 bytes), `basic_2_5d --ortho` (462130 bytes), `spatial_scene --camera perspective` (136991 bytes), `spatial_scene --camera orthographic` (34046 bytes). `basic_2_5d`'s screenshot path holds D and W for 75 steps; `spatial_scene` steps three times with no keys.

## M3b Phase 3 results (2026-10-15)

Physical and logical `Viewport`, display scale, and pointer conversion. Checkpointed locally as `9c99edf`. Not pushed.

**Viewport.** `Viewport` stores physical width and height (`u32`) and a scale factor (`f64`, finite and greater than zero). Logical size is `physical / scale` (`f32`). `Viewport::new` rejects a non-finite or non-positive scale. `from_logical_pixels` is scale 1, used before a window exists and by `render_to_rgba`. `apply_scale_factor` does not change the physical size. `apply_physical_size` ignores a zero width or height and keeps the last positive framebuffer. `physical_to_logical` and `logical_to_physical` do not clamp and do not flip Y. Both pointer spaces are top-left, +X right, +Y down. World space stays Y-up.

**Window.** winit 0.30.13 `ScaleFactorChanged` updates the scale and does not call `inner_size_writer.request_inner_size`. The following `Resized` stores the physical size, so the scale is not applied twice. `WindowedGame::init` reads `window.scale_factor()` and builds the surface from `window.inner_size()`. The platform then calls `resize` with that size. `WindowConfig` width and height stay logical pixels and only size the window at creation. An invalid scale is logged and ignored. A zero resize skips both the viewport update and `WindowRenderer::resize`.

**Rendering and input.** Extraction and the surface use `physical_size_pixels()`. The same physical size keeps the same camera projection at scale 2 and at 1.5. `Input::mouse_position` stays physical. `spatial_scene`'s HUD projects with the physical size. Sprite and mesh transforms do not read `Viewport`.

**Public API.** `Viewport` and `ViewportError` on `engine` and `engine::prelude`. No new `winit` or `wgpu` type. Methods: `physical_width`, `physical_height`, `physical_size`, `physical_size_pixels`, `logical_size`, `scale_factor`, `physical_to_logical`, `logical_to_physical`, `apply_physical_size`, `apply_scale_factor`, `new`, `from_logical_pixels`.

**Not in this phase.** Fullscreen, vsync, frame cap, cursor grab, controllers, UI layout, UI capture, world picking, camera raycasting, multi-window.

**Repository check.** `scripts/check.sh` on 2026-10-15, exit 0. All 4 steps: fmt, clippy `-D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (GPU tests ran, none skipped), rustdoc `-D warnings`. The run reported **261 passed, 0 failed**. New tests: `logical_size_is_physical_divided_by_scale`, `a_non_integer_scale_round_trips_including_points_outside_the_window`, `invalid_scales_are_rejected_and_zero_resizes_keep_the_last_framebuffer`, `resize_and_scale_events_do_not_apply_the_scale_twice`, `extraction_uses_the_physical_size_at_every_scale`, `mouse_position_stays_physical_and_logical_conversion_does_not_change_input`.

**Screenshots.** Release, 1280×720, rendered from this working tree and again from commit `6dbabb8` (`6dbabb88f10779301115d04720ac1f52000ed03a`), then compared with `cmp -s`. All four byte-identical: `basic_2_5d` (746071 bytes), `basic_2_5d --ortho` (462130 bytes), `spatial_scene --camera perspective` (136991 bytes), `spatial_scene --camera orthographic` (34046 bytes). `basic_2_5d`'s screenshot path holds D and W; `spatial_scene` does not send keys.

**Manual window (macOS only, 2026-10-15).** A throwaway program outside the repo opened a winit window. It is not a committed test. Windows and Linux were not run. Moving the window to a second monitor was not run; the only monitor scale observed was 2.

`Game::run` (the real `WindowedGame`), requested logical size 800×600: physical viewport 1600×1200, scale 2, logical size 800×600. `window.inner_size()` was 1600×1200 and `window.scale_factor()` was 2. `WindowRenderer::size()` was 1600×1200. A physical point (100, 40) converted to logical (50, 20) and back to (100, 40). The pointer had not moved, so `mouse_position` was (0, 0). The first run logged `ignored window scale factor 2` for a duplicate scale event after init had already stored 2. That warning was wrong: the scale was already applied, and the framebuffer was not scaled again. The runner now warns only when the scale is rejected. The rerun did not log it, and the viewport stayed 1600×1200 at scale 2.

A second process used `engine_platform::run` and the same viewport and `WindowRenderer::resize` calls. Initial size, surface, and scale matched. A logical resize to 900×500 became physical 1800×1000 at scale 2, and the surface matched. Two rapid logical resizes (640×360, then 1100×400) settled on physical 2200×800, logical 1100×400, scale still 2, surface matching. `set_minimized(true)` did not emit a zero size on this Mac; the viewport stayed 2200×800. Zero-size handling is covered by `invalid_scales_are_rejected_and_zero_resizes_keep_the_last_framebuffer`, not by this window.

## M3b Phase 4 results (2026-10-15)

Borderless fullscreen, vsync, an optional frame cap, cursor visibility and grab, and focus-loss cleanup. Checkpointed locally as `cfdf358`. Not pushed.

**Fullscreen.** `WindowConfig.fullscreen` creates the window borderless on the current monitor. `WindowControl.fullscreen` switches later. Entering stores the windowed logical size. Leaving requests that size back. A request that already matches does not call `set_fullscreen`. A resize that still has the fullscreen logical size does not replace the saved windowed size. `WindowRenderer::resize` skips a zero size and an unchanged physical size. Exclusive fullscreen and display-mode selection are not implemented.

**VSync.** `WindowControl.vsync` is the request. `WindowState.present_mode` is the effective mode (`Fifo`, `Immediate`, or `Mailbox`) after the surface is configured, and `None` before that, including every headless `Game`. VSync on prefers Fifo (`Fifo`, then `AutoVsync`, then `FifoRelaxed`). VSync off prefers Immediate, then Mailbox, then Fifo, using the modes the surface lists. The wgpu surface is reconfigured only when its present mode changes. A result other than the preferred mode is logged once per request.

**Frame cap.** `None` or `Some(0)` keeps `ControlFlow::Poll` and a redraw from every `about_to_wait`. A positive cap schedules `ControlFlow::WaitUntil` from the start of that frame, so input is not blocked by a sleep. `Game::step` still receives the time since the previous redraw, clamped as before. The fixed timestep is unchanged. A cap is a scheduling target, not a promise that the display presents at that rate. VSync can still hold a frame for one refresh.

**Cursor and pointer delta.** `cursor_visible` and `cursor_grabbed` are requests. Grab tries locked, then confined. If both are rejected, the request is cleared, `WindowState.grab_failed` stays set until a later attempt succeeds, and the runner does not panic. `WindowState.cursor_grab` is `Free`, `Locked`, or `Confined`. Release asks for grab none and reapplies the requested visibility. While the grab is locked or confined, `mouse_delta` comes only from device motion. While it is free, `mouse_delta` comes only from cursor-position changes. One position event after a grab change adds no delta. Device-motion units are winit's. This is not unaccelerated HID input. winit 0.30.13 rejects confined grab on macOS and locked grab on X11.

**Focus loss.** Every held key and mouse button is released. Collapsed Shift, Control, and Alt drop both physical sides, so the logical key is not left down. The release remains a frame edge and a fixed-step latch edge. `Actions` sync from the same event. `mouse_delta` is cleared. The grab request is cleared and is not set again when focus returns. The next cursor position after focus returns does not add a jump.

**Public API.** `PresentMode`, `CursorGrab`, and `WindowState` on `engine` and `engine::prelude`. `GameConfig::with_fullscreen`, `with_vsync`, `with_max_frame_rate`. `WindowControl` fields `fullscreen`, `vsync`, `max_frame_rate`, `cursor_visible`, `cursor_grabbed`. `WindowConfig` fields `fullscreen`, `vsync`, `max_frame_rate`. `InputEvent::MousePosition`, `MouseDelta`, `FocusLost`, `FocusGained`. `Input::place_mouse`, `add_mouse_delta`, `release_focus`, `regain_focus`. `engine_platform::frame_deadline` and `Flow::WaitUntil`. `SurfacePresent` and the present-mode helpers on `engine-renderer-wgpu`. No new public signature names a winit or wgpu type.

**Repository check.** `scripts/check.sh` on 2026-10-15, exit 0. All 4 steps: fmt, clippy `-D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (GPU tests ran, none skipped), rustdoc `-D warnings`. The run reported **277 passed, 0 failed**. The closing review added `a_second_focus_loss_does_not_repeat_the_release` and reran the check at **278 passed, 0 failed** (see "M3b closing check"). New tests: `vsync_on_selects_fifo_and_off_falls_through_immediate_then_mailbox`, `wgpu_modes_collapse_and_map_back_without_assuming_immediate_exists`, `fullscreen_enter_is_once_and_exit_restores_the_windowed_size`, `a_fullscreen_sized_resize_after_exit_does_not_replace_the_windowed_size`, `reentering_before_the_windowed_resize_keeps_the_saved_size`, `logical_window_size_rounds_and_rejects_a_bad_scale`, `grab_tries_locked_then_confined_and_a_failure_does_not_stick_the_request`, `a_failed_grab_stays_visible_until_the_next_attempt`, `pointer_events_use_one_delta_source_and_a_transition_drops_the_warp`, `a_frame_cap_waits_one_period_and_zero_disables_it`, `focus_loss_releases_held_input_and_focus_gain_drops_the_next_jump`, `a_concealed_focus_release_waits_for_the_next_publish`, `focus_loss_clears_both_sides_of_a_collapsed_modifier`, `focus_loss_clears_a_grab_request_and_focus_gain_does_not_restore_it`, `config_seeds_the_request_and_leaves_the_effective_mode_unset`, `focus_loss_releases_actions_and_the_fixed_latch_waits_for_the_next_step`.

**Screenshots.** Release, 1280×720, rendered from this working tree and again from commit `6dbabb8` (`6dbabb88f10779301115d04720ac1f52000ed03a`), then compared with `cmp -s`. All four byte-identical: `basic_2_5d` (746071 bytes), `basic_2_5d --ortho` (462130 bytes), `spatial_scene --camera perspective` (136991 bytes), `spatial_scene --camera orthographic` (34046 bytes).

**Manual window (macOS 27.0, build 26A428, 2026-10-15).** A throwaway program outside the repo called `Game::run`. It is not a committed test. Windows and Linux were not run. The process was not accessibility-trusted.

Requested logical 800×600, scale 2: physical 1600×1200, `present_mode` Fifo while vsync was requested. Borderless fullscreen became physical 3024×1898, logical 1512×949, scale still 2, `WindowState.fullscreen` true. Leaving it restored physical 1600×1200, logical 800×600, scale 2. A second toggle reached the same fullscreen size and restored 800×600 logical again. A second process created the window already borderless (requested 640×480): physical 3024×1898, logical 1512×949, scale 2, fullscreen true, then restored physical 1280×960, logical 640×480, scale 2.

VSync off selected `Immediate` on this Mac, so Mailbox and Fifo were not needed as fallbacks. Those fallbacks are the unit tests above. With vsync off and a cap of 20, eight frame deltas were min 0.0501 s, mean 0.0514 s, max 0.0522 s. With the cap cleared on the same `Immediate` mode, four later deltas averaged 0.0002 s.

`WindowState.cursor_visible` became false, then true. Grab succeeded as `Locked` with `grab_failed` false. Confined was not requested after locked succeeded. winit rejects confined on macOS; the fallback test covers that without a window.

Posted mouse moves did not change `mouse_delta` while grabbed or while free (the sums stayed 0). A posted A key never became `key_down`. The AppleScript that asked System Events to focus the probe, and a later request to resize the window to 900×500, did not return; a 20 s watchdog ended that process. Focus-loss release, the fixed latch, collapsed modifiers, delta-source selection, and grab failure remain the unit tests. The viewport scale stayed 2 through every fullscreen enter and restore that did complete.

## M3b Phase 4 review (2026-10-15)

The scheduling, grab, and focus paths were re-read against ADR-0013 and winit 0.30.13. No scheduling or grab code changed. One test was added.

**Frame cap.** On macOS, winit dispatches a queued `RedrawRequested` and then `about_to_wait` in the same turn (`app_state.rs` `cleared`). `frame` stores the deadline before that `about_to_wait`. While `now` is before the deadline, `about_to_wait` sets `ControlFlow::WaitUntil` and does not call `request_redraw`. An input or `MouseMotion` wake is `WaitCancelled`: the event is applied immediately, and the same wait is armed again. `Game::step` still runs only from `RedrawRequested`, with `dt` measured from the previous redraw. `None` and `0` set `next_frame` to `None`, which restores `Poll` plus a redraw on every `about_to_wait`. The cap is copied onto `WindowState` in the same frame, before the deadline is chosen. VSync blocks inside that redraw. If the blanking interval already passed the deadline, the next wait is immediate. This is the whole pacer.

**Cursor grab.** `WindowControl.cursor_grabbed` is the request. `WindowState.cursor_grab` is the mode `set_cursor_grab` accepted. Locked is tried first, then Confined. Both failing leaves the grab `Free`, sets `grab_failed`, clears the request, and does not panic. Visibility stays the requested `cursor_visible`; a grab does not itself hide the cursor. `mouse_delta` follows `WindowState.cursor_grab`, not the request. An application grab change suppresses one following position delta. Focus loss does not also set that suppress, because focus gain ignores the next position delta, and setting both would drop two moves.

**Focus.** `release` and `release_mouse` emit an edge only when the key or button was down, so a second `FocusLost` after `end_frame` does not emit another edge. `Actions::sync` records a release only when the aggregate goes from down to up. `a_second_focus_loss_does_not_repeat_the_release` and the extra asserts in `focus_loss_releases_actions_and_the_fixed_latch_waits_for_the_next_step` cover that. Collapsed Shift, Control, and Alt counts are cleared before the logical release. The grab request is cleared and focus gain does not set it.

**Still not observed on a window.** Relative `mouse_delta` while grabbed, the unlock sample, focus loss with a key held down, and focus return. A second macOS probe grabbed as `Locked` and released back to `Free`. Posted moves left `mouse_delta` at 0; the helper is not accessibility-trusted. Unit tests cover the unobserved paths. Windows and Linux were not run.

**Manual steps, when a developer can sit at the window.** Run a `Game::run` with an `update` system that sets `WindowControl.cursor_grabbed` and logs `WindowState.cursor_grab` and `Input::mouse_delta`.

1. Set `cursor_grabbed` true and move the mouse. On macOS the grab should read `Locked`, and `mouse_delta` should be non-zero. Confined is rejected on macOS.
2. Set `cursor_grabbed` false. The grab should read `Free`, and the unlock should not leave one huge delta as the only sample.
3. Hold a letter key, activate another application, then return. The key should be up. The position jump on return should not be added to `mouse_delta`. Moving again should produce a small delta.

## M3b acceptance (2026-10-15)

Checked against ADR-0013, ROADMAP.md, and the phase results above. M3a frame semantics, the fixed step, commands, typed events, interpolation, and logging were not changed by M3b. `basic_2_5d` still reads raw `Input`. `spatial_scene` uses `Actions` for keys and still reads scroll from `Input`.

| Requirement | Result |
|---|---|
| Wider keys, mouse press and release, collapsed modifiers | Met. Phase 1, `57dbc4a`. Unit tests |
| Frame edges and the fixed-step latch | Met. Phases 1–2. Unit tests |
| Actions, several bindings, rebinding, 1D and 2D axes, aggregate edges | Met. Phase 2, `83286a9`. Unit tests. `spatial_scene` uses actions |
| Raw `Input` still available | Met. `basic_2_5d` |
| Focus-loss cleanup, including a repeated loss | Met in tests. Not observed as an OS focus change |
| Physical and logical viewport, DPI, resize, pointer conversion, physical render size | Met. Phase 3, `9c99edf`. Unit tests, macOS window, byte-identical screenshots |
| Borderless fullscreen and windowed restore | Met. Unit tests and macOS window |
| VSync request versus effective mode | Met. Unit tests. macOS: Fifo on, Immediate off |
| Optional frame cap | Met. Deadline test and macOS 20 Hz measurement. Uncapped poll restored on that window |
| Cursor visibility, grab, release, delta source | Grab and visibility observed on macOS (`Locked`). Release, confined fallback, and delta source are unit-tested. Relative motion was not observed on the window |
| M3a compatibility and screenshot baseline | Met when the closing `scripts/check.sh` and `cmp -s` against `6dbabb8` pass. Recorded in the closing check below |

Deferred, and not acceptance blockers: gamepad, touch, text and IME, UI capture, action stacks, exclusive fullscreen, multi-window, a frame pacer, unaccelerated HID.

## M3b closing check (2026-10-15)

`scripts/check.sh` exit 0. `cargo fmt --check` passed. `cargo clippy --workspace --all-targets -- -D warnings` passed. `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` reported **278 passed, 0 failed, 0 ignored**. The 18 tests in `engine-renderer-wgpu` `tests/headless.rs` and the 16 in `spatial_scene` `tests/configurations.rs` ran; none were skipped. `cargo doc --workspace --no-deps` with `-D warnings` passed.

Release screenshots at 1280×720 were rendered again and compared with `cmp -s` to the renders of commit `6dbabb8` (`6dbabb88f10779301115d04720ac1f52000ed03a`). All four were byte-identical: `basic_2_5d` 746071 bytes, `basic_2_5d --ortho` 462130, `spatial_scene --camera perspective` 136991, `spatial_scene --camera orthographic` 34046.

## M4 Phase 1 results (2026-10-15)

Implemented locally. Not pushed. M4 is not complete.

**What this change is.** `ComponentRegistry` is a resource inserted empty by `World::new`. Registration is explicit: stable name, `TypeId`, hand-written `FieldDesc` list, optional value formatter, `Persistence`, and inert script flags. `register` rejects duplicate names, a second registration of the same type, conflicting metadata, and an empty or repeated field name. `register_builtin_components` is idempotent when the existing built-in metadata matches, and it still returns those conflicts. `Game::new` calls it once. Text dumps are `World::dump_hierarchy` and `World::dump_entity`. Unregistered components are counted and their `TypeId` debug text is listed. A formatter whose length disagrees with the field list is reported and is not paired onto the wrong fields. ADR-0014 records the settled scene model and the open follow-ups.

**Built-ins.** World: `Name`, `Transform`, `Visibility` (`Persist`); `Parent`, `Children`, `GlobalTransform`, `Interpolate` (`Runtime`). Renderer: `Sprite`, `RenderOrder`, `Mesh`, `TileMap`, `Camera`, `Light` (`Persist`). Animation: `Bone`, `AnimationPlayer`, `SkeletonInstance` (`Runtime`). `Fog` and `AmbientLight` are resources and are not registered. `Material` is a field of `Mesh`.

**Not in this change.** Scene membership, `SceneManager`, hierarchical visibility, name/tag lookup, code-defined prefabs, the spawn-rollback spike, asset removal on unload, and any scene file. No `serde`, RON, or JSON dependency. `extract_scene` is unchanged. Rendering behavior is unchanged.

**Still open (ADR-0014).** Asset ownership: scene membership does not own assets, and `SceneManager` will not delete from a retain count. Spawn rollback: a constrained builder and a feasibility spike come before Phase 4 code; full atomicity is not claimed. Persistent ids: spawn-order `u32`s are a future runtime map only. M19 is not required to store them. Direct despawn outside `SceneManager` is specified for Phase 4 (alive entity plus matching `SceneMember`) and is not implemented. `dump_entity` already returns `DumpError::NoSuchEntity` for a despawned id. D-16 and D-41 stay open.

**Repository check.** `scripts/check.sh` on 2026-10-15, macOS arm64, `rustc 1.99.0`, exit 0, after the registration review. All 4 steps: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` (includes `basic_2_5d` and `spatial_scene`), `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (**294 passed, 0 failed, 0 ignored**), `cargo doc --workspace --no-deps` with `-D warnings`. The 18 tests in `engine-renderer-wgpu` `tests/headless.rs` and the 16 in `spatial_scene` `tests/configurations.rs` ran. New tests: 14 in `engine-world` `dump::tests`, 2 in `engine` `tests/registry.rs`. Existing hierarchy and world tests still pass. No screenshot compare was run; extraction was not modified.

## M4 Phase 2 results (2026-10-15)

Checkpointed locally as `2c76f71`. M4 is not complete. Phase 1 remains commit `cdb5ec3`.

**Behavior.** `extract_scene` reads `Visibility` and `Parent`. A sprite, mesh, tilemap, or light is omitted when that entity or any ancestor has `Visibility::Hidden`. An entity with no `Visibility` is locally visible. The components are not written, and no inherited visibility component is stored. Transforms, parent links, depth testing, and `RenderOrder` are unchanged. The walk records entity ids it has already seen and stops, so a cycle ends. A parent id with no further `Parent`, including a despawned id, ends the walk. Gameplay systems are not told to skip hidden entities.

**Cameras and lights.** `Camera.active` and `order` still choose the camera. `Visibility` on a camera does not turn it off. A renderable parented under a hidden camera is omitted with the other renderables. A light uses the same ancestor rule as a sprite, including a light's own `Visibility::Hidden`. `AmbientLight` and `Fog` stay resources and are unaffected.

**Tests.** Eight extraction tests in `engine-renderer` `tests/visibility.rs`: hidden parent, nested ancestors, a hidden child under a visible parent, a transform-less hidden parent, a missing `Visibility`, restore by removing `Hidden`, reparent and detach, a stale parent, a cycle, meshes, tilemaps, lights, and a hidden active camera. One GPU test, `a_hidden_parent_keeps_its_sprite_off_the_framebuffer`, renders the clear color while the parent is hidden and the red child after `Hidden` is removed. Adapter: Apple M5, Metal.

**Repository check.** `scripts/check.sh` on 2026-10-15, macOS arm64, `rustc 1.99.0`, exit 0. All 4 steps passed. `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` reported **303 passed, 0 failed, 0 ignored**. `headless.rs` ran 19 tests. `spatial_scene` `configurations.rs` ran 16. Existing hierarchy tests still pass.

**Screenshots.** Release, 1280×720, rendered from this working tree and from commit `6dbabb8` (`6dbabb88f10779301115d04720ac1f52000ed03a`), then compared with `cmp -s`. All four byte-identical: `basic_2_5d` 746071 bytes, `basic_2_5d --ortho` 462130, `spatial_scene --camera perspective` 136991, `spatial_scene --camera orthographic` 34046. KI-18 is closed by the extraction tests and this comparison.

## M4 Phase 3 results (2026-10-15)

Checkpointed locally with this record. Not pushed. M4 is not complete. Phase 1 is `cdb5ec3`. Phase 2 is `2c76f71`.

**Behavior.** `Tags` is a component: a list of plain strings. `insert` rejects `""` and a tag already stored, and returns whether the list changed. A duplicate keeps its first position. `from_tags` skips empty strings and duplicates. Tags do not copy onto children, do not mark scene membership, and do not change `Transform`, `Visibility`, or extraction. `World::find_by_name` and `World::find_by_tag` scan live entities through a query and return every exact, case-sensitive match. Results are sorted by `Entity`'s `Ord` (index, then generation). That order is deterministic for an unchanged world. It is not spawn order and not a persistent id. A missing name or tag returns an empty list. `find_by_name("")` matches only a `Name` whose string is empty. `find_by_tag("")` matches nothing, because an empty tag is not stored. Neither scan follows `Parent`, and neither keeps an index.

**Registry.** `Tags` is registered with the other built-ins: name `"Tags"`, field `values: Vec<String>`, `Persistence::Persist`, script flags false, and a formatter. `dump_entity` prints the stored tags in insertion order, each with `Debug` quotes. An empty list prints `(none)`. No serializer and no script reads the component.

**Not in this change.** `SceneManager`, `SceneMember`, scene definitions, instantiation, unload, scene-aware lookup, asset-ownership changes, component serialization, prefabs, a visual editor, and any name or tag index. Caching, dirty visibility, and a second visibility component were not added.

**Tests.** Five in `engine-world` `lookup`: one match, duplicate names, a missing name, an empty name, rename, despawn and id reuse, tag insert and remove, duplicate and empty tags, several tags on one entity, one tag on several entities, case, no inheritance, lookup across two hierarchies, registry metadata, and dump text. One extraction test, `names_and_tags_do_not_change_extraction`: adding and removing tags leaves the sprite extracted and the transform unchanged.

**Repository check.** `scripts/check.sh` on 2026-10-15, macOS 27.0 arm64, `rustc 1.99.0`, exit 0. All 4 steps passed. `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` reported **309 passed, 0 failed, 0 ignored**. `headless.rs` ran 19 tests. `spatial_scene` `configurations.rs` ran 16. Clippy `--all-targets` compiled `basic_2_5d` and `spatial_scene`.

**Screenshots.** Release, 1280×720, rendered from this working tree and compared with `cmp -s` to the four PNGs previously rendered from commit `6dbabb8` (`6dbabb88f10779301115d04720ac1f52000ed03a`) and recorded in the Phase 2 results. All four byte-identical: `basic_2_5d` 746071 bytes, `basic_2_5d --ortho` 462130, `spatial_scene --camera perspective` 136991, `spatial_scene --camera orthographic` 34046.

## M4-4A results (2026-10-15)

Commit `302622b`. Not pushed. M4 is not complete. Phase 1 is `cdb5ec3`. Phase 2 is `2c76f71`. Phase 3 is `f9a4aab`.

**Behavior.** `World::new` inserts an internal scene manager. `create_scene_instance` opens an empty instance and returns a new `SceneInstanceId`. Ids start at 0 and increase. An issued id is not issued again. `track_scene_entity` records one live entity that does not already have `SceneMember`, inserts that component, and assigns the next scene-local `u32` in that instance, starting at 0. It does not spawn and does not edit any other entity. The same local id may exist in another instance. `scene_entity` returns the recorded entity only when it is alive and its `SceneMember` names that instance and local id. Otherwise it returns `Ok(None)`. A stale row stays until `sweep_scene_mappings`, which drops dead or mismatched rows and does not reuse the local id. Empty instances stay. `World::despawn` is unchanged.

**Errors.** A scene method takes the manager out and inserts the same value before returning `Ok` or `Err`. `NoSuchEntity`, `NoSuchInstance`, and `AlreadyMember` leave the manager in the world. If the manager is absent, the call returns `SceneError::SceneManagerUnavailable` and does not insert a new one. A panic while the manager is out is not restored. There is no `unsafe`.

**Registry.** `SceneMember` is registered with the built-ins: fields `instance: SceneInstanceId` and `local: u32`, `Persistence::Runtime`, script flags false. `dump_entity` prints both numbers.

**Not in this change.** Scene definitions, the constrained builder, spawn, rollback, unload, replace, deferred scene commands, asset removal, serialization, prefabs, and the `spatial_scene` switch. The `SceneOpId` result for a deferred command is recommended in ADR-0014 and is not accepted or implemented.

**Tests.** Nine in `engine-world` `scene::tests`: distinct ids and issuance order, repeated local ids on distinct entities, unknown instance and local id, an entity with no membership, despawn plus sweep plus a reused slot, a removed or replaced `SceneMember`, ordinary errors, a missing manager, and registry plus dump text. `engine` `tests/registry.rs` now also requires `"SceneMember"` from `Game::new`.

**Repository check.** `scripts/check.sh` on 2026-10-15, macOS 27.0 arm64, `rustc 1.99.0`, exit 0. All 4 steps passed. `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` reported **318 passed, 0 failed, 0 ignored**. `headless.rs` ran 19 tests. `spatial_scene` `configurations.rs` ran 16. Clippy `--all-targets` compiled `basic_2_5d` and `spatial_scene`.

**Screenshots.** Not re-rendered. `extract_scene` queries sprites, meshes, tilemaps, lights, cameras, `Visibility`, and `Parent`. It does not read `SceneMember` or the scene manager. This change does not alter extraction or the examples.

## M4-4B results (2026-10-15)

Implemented locally. Not committed. M4 is not complete. M4-4A is `302622b`.

**Behavior.** `World::load_scene` takes the scene manager out, issues a `SceneInstanceId`, and runs `SceneBuilder`. `spawn` records a new entity. `spawn_child` parents it only under an entity this build created. The closure does not receive `&mut World`. On `Ok`, each created entity gets `SceneMember` in spawn order and the instance is inserted. On `Err`, those entities are despawned, an empty `Children` component the build introduced on a surviving entity is removed, and the instance is not inserted. The id is not reused. A nested load while the manager is out returns `SceneManagerUnavailable` and does not insert a replacement. The original manager is put back on `Ok` and on `Err`. A panic is not restored. `create_scene_instance` and `track_scene_entity` stay public and can still build membership outside a load.

**Rendering.** Extraction is unchanged. A sprite created by `load_scene` produces the same `SpriteInstance` as the same sprite spawned directly (`engine-renderer` `tests/scene_load.rs`).

**Not in this change.** Unload, replace, deferred scene commands, `SceneOpId`, scene files, prefab assets, asset deletion, and the `spatial_scene` switch.

**Tests.** Nine new tests in `engine-world` `scene::tests`: empty load, one entity, a parent with children plus a deep chain, two instances, failed loads before and after spawns, an outside parent and a `Parent` in a bundle, empty `Children` cleanup, a nested load, and a missing manager. One extraction test compares a scene sprite with a spawned sprite.

**Repository check.** `scripts/check.sh` on 2026-10-15, macOS 27.0 arm64, `rustc 1.99.0`, exit 0. All 4 steps passed. `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` reported **328 passed, 0 failed, 0 ignored**. `headless.rs` ran 19 tests. `spatial_scene` `configurations.rs` ran 16.

**Screenshots.** Release, 1280×720, rendered from this working tree and from commit `6dbabb8` (`6dbabb88f10779301115d04720ac1f52000ed03a`), then compared with `cmp -s`. All four byte-identical: `basic_2_5d` 746071 bytes, `basic_2_5d --ortho` 462130, `spatial_scene --camera perspective` 136991, `spatial_scene --camera orthographic` 34046.

## Planned systems and their state

None of these exists. They are in the roadmap so that earlier work does not block them; they are not
in the current scope.

| System | Milestone | State |
|---|---|---|
| Sprite-sheet characters (A) | M8 | Not started. Building blocks only (`Sprite.region/size/anchor`, KI-21) |
| Skeletal 2D characters (B) | M11 | Data model and a single-clip player exist (4 tests); no blending, IK, events (KI-22) |
| 3D skinned characters (C) | M12 | Not started (KI-23) |
| Character gameplay layer and API surface | M9 | Not started |
| UI runtime | M10 | Not started (KI-24, KI-25) |
| Gameplay scripting | M18 | Not started. Commands and events exist (M3a-3). The component registry and inert script flags exist (M4 Phase 1). No script runtime |
| Audio | M16 | Not started |

## Not in scope now

M4-4C, phase 5, and everything after M4. In particular: no scene format, no scene unload or replace, no new rendering features, no collision, no
editor work, and no work on UI, scripting or any of the three character representations. M4 Phase 1 is commit `cdb5ec3`. Phase 2 is commit `2c76f71`. Phase 3 is commit `f9a4aab`. M4-4A is commit `302622b`. M4-4B (synchronous load) is implemented locally and is not committed. None of M4 is pushed. M4 is not complete. M3b is complete locally and is not pushed. These systems are deliberately
not being started together: each has its own milestone, prerequisites and decisions. Decisions that data
formats or public APIs will bake in are made before their first consumer (see
[docs/DECISIONS.md](docs/DECISIONS.md#what-to-resolve-first)).
