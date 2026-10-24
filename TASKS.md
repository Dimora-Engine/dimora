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

**M6 is in progress.** The camera-configuration test scene is verified (below). M6-1B, custom projection matrices (D-25), is commit `a38cb30` and is not pushed. M6-1C, the example's seventh camera, is committed locally and is not pushed. D-07 reverse-Z is settled (ADR-0019). Coplanar sprite priority is settled (ADR-0020) and is not pushed. D-26 is settled (ADR-0021). Tilemap extraction culls 32×32 chunks (`4cc0b08` baseline, `c752395` chunks). The baseline, the comparison, and the allocation-traffic reading are in [docs/PERFORMANCE.md](docs/PERFORMANCE.md). A direct write to `tiles` does not hide a visible tile; `invalidate` refreshes the rejected-chunk cull count (`a_direct_write_stays_visible_when_the_camera_moves_and_only_the_cull_count_goes_stale`). Debug drawing is implemented on `feat/m6-debug-draw` (`b7ec684`): single-frame world-space lines, rays, wire boxes, wire spheres, and camera frusta. Sprite, mesh, mixed-scene, lighting, and multi-camera CPU benchmarks, plus CPU encode and submit, are recorded on `feat/m6-render-scale` (`82972c2`). GPU pass timestamps are not measured. The depth-visualisation view, gameplay orientation, and D-06 have not started. M6 is not complete. Milestone order is unchanged.

**M3a, the frame model, is complete locally and is not pushed.** Full scope and acceptance criteria:
[ROADMAP.md](ROADMAP.md#m3-frame-model-input-and-platform-in-progress). M3a-2 is commit `312a05f`.
Lifecycle anchors are `ecf4fa1`. M3a-3 is `cd802f0`. M3a-4 is `5c4fd97`. M3a-5 is `523f553`. M3a-6
is the public-API review recorded below. M3b Phases 1–3 are checkpointed locally (`57dbc4a`,
`83286a9`, `9c99edf`). Phase 4 is checkpointed locally as `cfdf358`. None of M3b is pushed. The rendering regression baseline is commit `6dbabb8`.

**M4 is complete locally at `2b79806` and is not pushed.** Phase 1, the component registry and text dumps, is commit `cdb5ec3` and is not
pushed (ADR-0014). Phase 2, hierarchical visibility, is commit `2c76f71` and is not pushed. Phase 3,
name and tag lookup, is commit `f9a4aab` and is not pushed. M4-4A, scene instance tracking, is
commit `302622b` and is not pushed. M4-4B, synchronous scene load, is commit `128c70e` and is not
pushed. M4-4C, unload and replace, is commit `e12fb5b` and is not pushed. M4-4D, deferred scene
operations, is commit `8398e49` and is not pushed. M4-4E, the `spatial_scene` switch, is commit
`d58c66b` and is not pushed. M4-5, the lamp prefab and crate-private low-level scene methods, is
commit `2b79806` and is not pushed. Evidence is under "M4 Phase 1 results", "M4 Phase 2 results", "M4 Phase 3 results",
"M4-4A results", "M4-4B results", "M4-4C results", "M4-4D results", "M4-4E results", and "M4-5 results".

**M5 is complete locally and accepted, and is not pushed.** Formal acceptance was approved on 2026-10-22. The implementation checkpoint is `92286ce0546a4d94bb0365581ae7469fe2ad3260`. The acceptance record is the documentation section below. M5-1 records ADR-0015, ADR-0016, and ADR-0017. M5-2A, the
CPU texture descriptors, is committed locally as `c687d5b` and is not pushed. M5-2B, GPU formats and
the sampler cache, is commit `11b914f` and is not pushed. M5-2C, GPU mip generation, is commit
`622ef80` and is not pushed. M5-3A, bounded CPU region transitions, is commit `b51f1fb` and is not
pushed. M5-3B, GPU rectangle uploads, is commit `84d5ab1` and is not pushed. M5-4A, the CPU shelf
packer, is commit `bf4d9bb` and is not pushed. M5-4B, GPU atlas sampling, is commit `1666695` and is
not pushed. M5-5A, the CPU `MaterialAsset`, is commit `e0c29ef` and is not pushed. M5-5B, material
handles and material-driven sprite and mesh rendering, is commit `3b3cd4a` and is not pushed.
M5-5C, unified blended sprite and mesh rendering, is commit `ab1b6f7` and is not pushed.
M5-5D, material-aware batching, is commit `a7efd6c` and is not pushed. M5-6, procedural mesh
generators, is commit `92286ce` and is not pushed. Localized mip-texel regeneration is deferred and has no milestone.
M6 continued after that acceptance: reverse-Z (ADR-0019) and coplanar sprite priority (ADR-0020) are later local work. D-26 is settled (ADR-0021). Tilemap chunks are in extraction (`4cc0b08`, `c752395`). The baseline, comparison, and allocation-traffic reading are in [docs/PERFORMANCE.md](docs/PERFORMANCE.md). Debug drawing is implemented on `feat/m6-debug-draw` (`b7ec684`). The sprite, mesh, and mixed-scene benchmark is recorded on `feat/m6-render-scale` (`82972c2`). GPU pass timestamps are not measured. The depth-visualisation view, gameplay orientation, and D-06 have not started.
The manual window note under M4-4E and M4-5 is unchanged: the framebuffer and the keyboard were not observed.

## Completed work

| Milestone | Evidence |
|---|---|
| M0 Foundation | Commits `6b0d016`..`278de15`: math, core, world, assets, renderer, wgpu backend, animation, platform, facade, `basic_2_5d`. All crates build; tests pass (below) |
| M2 / T3 Hierarchy robustness and asset-id collision guard | Commits `498f77a`, `e1f70b1`, `8abee2e`, `a4f9e77`; evidence under "Task T3 results" below. Verified on macOS only |
| M2 / T2 GPU resource lifetime and `AssetManager::remove` | Commits `884b0ff`, `1d92ce7`, `5d6a199`, `c3c98f8`; evidence under "Task T2 results" below. Verified on macOS only |
| M2 / T1 Reproducible toolchain and one verification command | Commits `e08033e`, `3a7a95c`, `7fb2c4d`, `3d467b6`, `89b1fa5`; evidence under "Task T1 results" below. Verified on macOS only |
| M1 Core spatial renderer | Commits `83d9883`..`6ad872f`: sprite orientation, `RenderOrder`, sprite `region`, camera `ViewportRect`, `spatial_scene`, `docs/SPATIAL_RENDERING.md`. `engine-renderer/tests/camera.rs` (27 tests), GPU tests `camera_viewport_confines_the_image_to_its_rectangle` and `billboards_face_the_camera_but_fixed_quads_go_edge_on` pass with a Vulkan adapter. `spatial_scene` run windowed under Xvfb: figure heights shrink with depth in perspective and stay equal in orthographic |
| M5 Materials and textures | Formally accepted 2026-10-22. Implementation checkpoint `92286ce0546a4d94bb0365581ae7469fe2ad3260`. Evidence under "M5 acceptance audit" and "M5 formal acceptance". Not pushed. M6 is not part of this row |

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
| KI-5 | ~~Only one camera renders per frame~~ **Fixed locally.** M6-2B (`6a47882`) renders every active surface camera. M6-2C lets a camera write an offscreen target and a later sprite sample that image. The editor viewport UI remains M22 | Verified (CPU and GPU tests, Apple M5 / Metal). M6-2C is committed locally and not pushed | M6 |
| KI-6 | ~~Textures: no mipmaps, clamp only, always sRGB; `Material` is colour + albedo + `lit`; meshes cannot be transparent~~ M5 implements per-texture colour space, samplers, mip generation, `MaterialAsset` (Opaque, Mask, and Blend), and blended meshes. Still open, and not closed by acceptance: localized mip texels (KI-25), normal-map shading (M14), the `from_rgba8` assert (KI-30), and `cone` still asserts | Verified for the implemented slice (M5 acceptance, `92286ce`) | M5 accepted; remaining items stay open |
| KI-7 | ~~A child of an entity without a `Transform` stays at the origin~~ **Fixed by T3** (commit `498f77a`): such an entity counts as identity, at any depth | Verified (test failed before, passes after) | M2 |
| KI-8 | ~~`World::despawn(parent)` leaves children with a dangling `Parent`~~ **Fixed by T3** (commit `e1f70b1`, D-03): `despawn` removes the whole subtree; `detach_children` keeps children as roots. `despawn_recursive` is gone | Verified | M2 |
| KI-9 | Resource access is `RefCell`: a double borrow panics at runtime. Deferred commands and typed events are implemented (M3a-3). Change detection is still absent (ADR-0009) | Partial (the `RefCell` panic and change detection remain) | M3a-3 done; change detection stays deferred |
| KI-10 | ~~Frame order is hard-wired in `Game::step`~~ Named stages and `before`/`after` are implemented (M3a-2). ~~`FixedTimestep::alpha()` is not exposed~~ Opt-in extract interpolation is implemented (M3a-5). `Game::step` still runs the stages in the documented order | Verified | M3a-5 |
| KI-11 | ~~`Camera` stores the closed `CameraProjection` enum, not an arbitrary `Projection`~~ **Fixed in M6-1B** (`a38cb30`, not pushed): `CameraProjection::Custom(Mat4)` via `CameraProjection::custom`. The matrix is fixed across resize. Singular matrices render; picking returns `None` when the inverse fails. No trait object | Verified (CPU tests in `tests/camera.rs` and `frustum.rs`; GPU tests in `custom_projection.rs`, Apple M5 / Metal) | M6 |
| KI-12 | Assets: synchronous loading; PNG is the only decoder (sprites, meshes, skeletons, clips are code-built); no hot-reload watcher; `AssetId` is a name hash and changes on rename. The collision check is **done by T3** (commit `8abee2e`): `insert`, `load` and `reload` return `AssetError::IdCollision` and never overwrite. Remaining limit: the check is per asset type (see "Task T3 results") | Read | M17 |
| KI-13 | Skeletal animation has no blending, events, IK, skinning or file loading | Read | M11 |
| KI-14 | Blended sprites sort by quad centre. Blended meshes sort by the camera-space Z of the world-space bounding-sphere centre (ADR-0015). Large or intersecting blended objects can still mis-order. M5 accepted that object-level approximation. It is not solved | Read | M14 for intersections. D-48 coplanar conflicts stay M6 |
| KI-15 | ~~Input has 47 keys and no F-keys~~ Phase 1 maps F1–F12, punctuation, navigation, and the keypad, and mouse release is a one-frame edge. Phase 2 adds `Actions` and the fixed-step latch. Phase 4 adds cursor visibility, grab (locked, then confined), and focus-loss release of held input. Still open: gamepad, touch, and text. Device motion is winit's units, not unaccelerated HID. Relative motion and OS focus-loss were not observed on a window; the unit tests cover them | Done for M3b (gamepad, touch, and text stay deferred) | M3b (text and UI scale policy: M10, D-39) |
| KI-16 | ~~Two rustdoc warnings: unresolved `World` and `Schedule` links in `crates/engine/src/lib.rs`~~ **Fixed by T1** (commit `e08033e`); `cargo doc` with `-D warnings` is clean | Verified | M2 |
| KI-17 | `Cargo.toml` declares `MIT OR Apache-2.0` but no licence files exist. The licence is **proposed, not owner-confirmed** (OD-3); copyright holder unknown. Audit and the list of files that would change: "Licence audit" below | Verified (repository listing, `cargo metadata --locked`) | Owner decision, then M24 notices |
| KI-18 | ~~`Visibility::Hidden` applies to the entity only; it does not hide children~~ Extraction omits a sprite, mesh, tilemap, or light when it or any ancestor is `Hidden`. Local components are unchanged. Cameras still use `Camera.active` | Done (M4 Phase 2 checkpoint) | M4 |
| KI-19 | Tilemap CPU extraction is measured on Apple M5, release, 2026-10-23 ([docs/PERFORMANCE.md](docs/PERFORMANCE.md)). A partial view visits accepted 32×32 chunks. A fully visible map still visits every cell and builds one `SpriteInstance` per occupied tile per camera. `alloc_bytes` is cumulative allocator traffic, not peak resident memory. Debug-line CPU submission and extraction are measured on the same machine, release, 2026-10-24. Sprite, mesh, mixed-lane, lighting, and multi-camera CPU extraction, plus CPU encode and submit, are measured on the same machine, release, 2026-10-24 (`82972c2`, "Render scale"). GPU pass timestamps are still open. A driver completion wait is recorded and is not that pass time. Instance buffers are still rewritten every frame: steady renders upload and do not grow | Partial. Harnesses `tilemap_extract_bench`, `debug_draw_bench`, `render_scale_bench`, and `render_scale_gpu`. GPU pass time not measured | M6 |
| KI-31 | A dense 1000×1000 tilemap that is fully visible builds a new cutout instance vector per camera every extract. One orthographic camera records 394,769,896 bytes of allocator traffic and keeps 152 MiB of vector capacity (1,048,576 × 152). Seven such cameras record 2,763,382,296 bytes of traffic and keep 1,064 MiB until the scene drops (PERFORMANCE.md, 2026-10-23, Apple M5). Chunk culling does not reduce this, because every tile is visible. Debug drawing does not change this path. The 2026-10-24 sprite scale uses the same per-camera instance-vector shape at a smaller size: 10,000 opaque sprites keep 2.49 MB and extract in about 1.7 ms, and seven cameras keep exactly 7× that capacity. That does not pull this forward. **Deferred.** Do not add a second renderer or a retained tile world unless a later measurement says the full-view tile reuse pays for itself. Acceptance: same harness, same machine class, release profile; the seven-camera orthographic full view's `alloc_bytes` and retained instance capacity are re-measured; partial-view chunk rejection, row-major order, and one cutout batch per camera per texture stay as they are | Deferred. Reproducible with `DIMORA_BENCH_SIZES=1000 cargo run --release -p engine-renderer --bin tilemap_extract_bench` | M6. Sprite scale does not start it |
| KI-32 | `AncestorVisibility::hides` allocates a `Vec` on every call, including a parentless visible entity (`extract.rs`). `render_scale_bench` records about 10,030 allocations for 10,000 parentless opaque sprites, and about 10,020 when a tight camera keeps 20 of them. Extract stays about 1.54 ms in that tight case against about 1.67 ms when all 10,000 are visible. The allocation's share of that millisecond was not timed on its own. **Not started.** Can wait past M6. Acceptance: release `render_scale_bench` on the same machine class; `sprite-opaque-10k` or the later unlit 10,000-sprite row drops `alloc_count` from about 10,030 to a small constant; visible, culled, and batch counts stay as they are | Read. Not required to close M6 | M6, after the depth view |
| KI-20 | ~~Platform: single window, `ControlFlow::Poll`, default present mode; no fullscreen/vsync configuration~~ Phase 4 adds borderless fullscreen, a vsync request with a surface-capability fallback, and an optional `WaitUntil` frame cap. Still one window. Uncapped frames still poll. Exclusive fullscreen and a frame pacer are not built. macOS showed Fifo with vsync on and Immediate with vsync off | Done for M3b | M3 |
| KI-21 | A sprite's natural size comes from the whole `SpriteAsset`, not from `Sprite.region`, so showing a sheet frame without also setting `size` stretches it to the asset's size | Read (`extract.rs`: `sprite.size.unwrap_or_else(|| asset.world_size())`) | M8 |
| KI-22 | `animate_system` writes sampled transforms directly onto bone entities and only visits entities that have a `SkeletonInstance`; `Pose` is not in the path, so there is no stage for blending or IK, and a skeleton-less sprite cannot be animated by it | Read (`clip.rs`) | M11 (sprite sheets: M8) |
| KI-23 | `Vertex` is position, normal and uv; no joints, weights or tangents; no model loader; meshes are code-built | Read (`mesh.rs`) | M12 |
| KI-24 | ~~The platform layer does not forward the window scale factor, and `Viewport` is one pixel size~~ M3b Phase 3 stores physical size, logical size, and a validated scale, and converts the physical pointer. Remaining: no text-input events, so UI text still has no input path, and reference-resolution fit modes are not built (D-39) | Partial (checkpointed locally as `9c99edf`) | M10 |
| KI-25 | Proved region journals upload rectangles (M5-3B, `84d5ab1`). A CPU shelf packer is commit `bf4d9bb`. GPU atlas samples are commit `1666695` and isolate regions only for the layouts in ADR-0017. Formal acceptance keeps whole-chain mip regeneration. Still open: localized mip-texel regeneration (DEFERRED, milestone unassigned; `affected_mip_rect` is unused) and the M10 glyph atlas. A journal older than eight transitions, or bytes written outside `update_region`, still uploads the whole base level. A visible-only atlas write leaves the extruded gutter stale; `Atlas::replace_source` refreshes it | M5-3B, M5-4A, M5-4B; acceptance did not close the open items | M10 for the glyph atlas. Localized mips are unassigned |
| KI-26 | macOS dev machine (Apple M5, macOS 27.0): rustc **older than 1.99** cannot link anything, not even `fn main(){}`, with `ld: tapi error: malformed file ... libSystem.B.tbd ... unknown architecture arm64e.x1-macos`. Cause seen: `xcode-select -p` points at Xcode 26.4.1 (`ld-1266.8`) but `xcrun --show-sdk-path` returns the macOS 27.0 SDK from the Command Line Tools, which that linker cannot read; rustc 1.99.0 links fine (not investigated why). Host setup, not engine code; nothing was changed system-wide. Workaround used for the 1.89.0 and 1.90.0 runs: `SDKROOT=/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk`. Proper fix (user's decision): select a matching Xcode or CLT with `xcode-select`, or update Xcode | Verified (hello-world reproduction with 1.89.0 and 1.90.0 fails, with the `SDKROOT` above succeeds, 1.99.0 succeeds without it) | Environment |
| KI-27 | ~~The MSRV (1.90) was only built on macOS arm64~~ **Verified on all three systems** by the CI `msrv` job: `cargo +1.90.0 test --workspace --locked`, 138 passed, 0 failed on x86_64 Linux, aarch64 macOS and x86_64 Windows (MSVC). The MSRV holds only with the committed `Cargo.lock` | Verified (run 38475229601) | Done |
| KI-28 | Seven views are rendered by example-local constructors in `spatial_scene` (evidence below). Still true: no engine preset API, no oblique builder, no 2D-style game. `custom` stores an example matrix in `CameraProjection::Custom` (KI-11, M6-1B `a38cb30`). `Upright` is edge-on from straight above and foreshortened in a steep view; an oblique matrix does not change that (ADR-0021). Coplanar cutout content that has not opted in is ordered by draw order or by depth-test rounding, and `RenderOrder` still orders only blended sprites (ADR-0020; `spatial_scene` does not opt in). The ground plane stays `XZ` (provisional, ADR-0004) | Verified for the demonstration and the conflicts (Apple M5, 2026-10-14). Custom matrices verified 2026-10-22. The seventh constructor is M6-1C, committed locally and not pushed. D-26 is settled. Gameplay orientation remains open | M6 |
| KI-29 | Logging and `Diagnostics` exist (M3a-4). M4 Phase 1 adds a text hierarchy dump and an entity dump. M5's first validation slice names an oversized texture, a missing texture, and a zero-sized atlas region. Still absent: a depth or overdraw view, and a visual inspector | Partial | M6, M20 |
| KI-30 | `Texture::from_rgba8` asserts when the RGBA8 byte count is not `width * height * 4`. PNG decode and `Texture::update_region` return errors instead. The constructor signature stays. A later ergonomic change could return `Result` for the same success path. That change is not scheduled | Read (`texture.rs`) | Unassigned |

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

Commit `128c70e`. Not pushed. M4 is not complete. M4-4A is `302622b`.

**Behavior.** `World::load_scene` takes the scene manager out, issues a `SceneInstanceId`, and runs `SceneBuilder`. `spawn` records a new entity. `spawn_child` parents it only under an entity this build created. The closure does not receive `&mut World`. On `Ok`, each created entity gets `SceneMember` in spawn order and the instance is inserted. On `Err`, those entities are despawned, an empty `Children` component the build introduced on a surviving entity is removed, and the instance is not inserted. The id is not reused. A nested load while the manager is out returns `SceneManagerUnavailable` and does not insert a replacement. The original manager is put back on `Ok` and on `Err`. A panic is not restored. `create_scene_instance` and `track_scene_entity` stay public and can still build membership outside a load.

**Rendering.** Extraction is unchanged. A sprite created by `load_scene` produces the same `SpriteInstance` as the same sprite spawned directly (`engine-renderer` `tests/scene_load.rs`).

**Not in this change.** Unload, replace, deferred scene commands, `SceneOpId`, scene files, prefab assets, asset deletion, and the `spatial_scene` switch.

**Tests.** Nine new tests in `engine-world` `scene::tests`: empty load, one entity, a parent with children plus a deep chain, two instances, failed loads before and after spawns, an outside parent and a `Parent` in a bundle, empty `Children` cleanup, a nested load, and a missing manager. One extraction test compares a scene sprite with a spawned sprite.

**Repository check.** `scripts/check.sh` on 2026-10-15, macOS 27.0 arm64, `rustc 1.99.0`, exit 0. All 4 steps passed. `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` reported **328 passed, 0 failed, 0 ignored**. `headless.rs` ran 19 tests. `spatial_scene` `configurations.rs` ran 16.

**Screenshots.** Release, 1280×720, rendered from this working tree and from commit `6dbabb8` (`6dbabb88f10779301115d04720ac1f52000ed03a`), then compared with `cmp -s`. All four byte-identical: `basic_2_5d` 746071 bytes, `basic_2_5d --ortho` 462130, `spatial_scene --camera perspective` 136991, `spatial_scene --camera orthographic` 34046.

## M4-4C results (2026-10-16)

Commit `e12fb5b`. Not pushed. M4 is not complete. M4-4A is `302622b`. M4-4B is `128c70e`.

**Unload.** `unload_scene` preflights, then lifts every non-member whose parent is a validated member, then removes those members with `despawn_one`. `World::despawn` is unchanged and is not used. The instance record is removed. Other instances and runtime entities stay. Assets are not removed. An unknown id returns `NoSuchInstance` and changes nothing. An empty instance succeeds.

**Lift.** The world matrix is the parent chain. `Transform::from_matrix` stores a TRS triple and drops shear. Uniform scale matches that matrix. A zero scale axis that decomposes to a finite TRS is stored, including scale 0. A non-finite result refuses the unload before any removal. The lifted entity's interpolation previous local becomes the new local. Descendants and bystander `GlobalTransform` values are left as they were. No `GlobalTransform` is inserted.

**Preflight.** Stale rows are not members. A forged `SceneMember` for the instance is `InconsistentMembership` and nothing is removed. A broken `Parent`/`Children` pair or a parent cycle is `InvalidHierarchy`. A missing manager is `SceneManagerUnavailable` and is not replaced. These `Result` paths put the manager back. A panic after mutation has started is not undone.

**Replace.** `replace_scene` preflights the old instance, builds the new one with the existing `load_scene` path, and unloads the old one only after that build returns `Ok`. A failed build leaves the old instance published. The new id is consumed either way and is not reused. A bundled `SceneMember` for the old instance fails validation before publish, so the recovery branch that unloads a published replacement is not reached through the public builder. A successful replace that must lift a runtime child keeps the new instance and the lifted entity.

**Low-level methods.** `create_scene_instance` and `track_scene_entity` stay public. Nothing outside `engine-world` tests calls them. The supported lifecycle is `load_scene`, `unload_scene`, and `replace_scene`. They should become `pub(crate)` before a stable public release. This checkpoint does not change their visibility.

**GPU.** `unloading_a_scene_and_removing_its_assets_returns_the_gpu_cache_to_baseline` on Apple M5, Metal. A camera-only world caches 0 textures and 0 meshes. After the scene loads and renders, the cache is 2 textures and 1 mesh. After unload, explicit `AssetManager::remove` of the test-owned assets, and another frame, the cache and the entity count are back to that camera baseline. Renderer pipelines are not counted. Shared assets are covered by `unloading_a_scene_leaves_shared_and_application_assets_in_place`, which does not call `remove`.

**Repository check.** `scripts/check.sh` on 2026-10-15, macOS 27.0 arm64, `rustc 1.99.0`, exit 0. All 4 steps passed: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (**341 passed, 0 failed, 0 ignored**), `cargo doc --workspace --no-deps` with `-D warnings`. `headless.rs` ran 20 tests. `spatial_scene` `configurations.rs` ran 16.

**Screenshots.** Release, 1280×720, rendered from this working tree and compared with `cmp -s` to the four PNGs rendered from commit `6dbabb8` (`6dbabb88f10779301115d04720ac1f52000ed03a`). All four byte-identical: `basic_2_5d` 746071 bytes, `basic_2_5d --ortho` 462130, `spatial_scene --camera perspective` 136991, `spatial_scene --camera orthographic` 34046. SHA-256: basic `c3c5874adeea119c98255c6c1d08208c0244224d930d86e8736c280ef88b38fe`, ortho `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f`, perspective `ec5f6252a88f642b454ae768f9eec8cdd5fba88a5aeb7a424fcefe5fda12275e`, orthographic `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde`.

## M4-4D results (2026-10-16)

Commit `8398e49`. Not pushed. M4 is not complete. M4-4C is `e12fb5b`.

**Queue.** `CommandWriter::load_scene`, `unload_scene`, and `replace_scene` append to the existing FIFO command queue. Each accepted call returns a `SceneOpId` and stores `SceneOpStatus::Pending`. The flush calls the synchronous lifecycle. The closure does not enqueue another command. `Commands` stays removed for the duration of the flush, which is the existing rule.

**Result.** `scene_operation` copies the status and leaves it stored. A completed status is `Loaded`, `Unloaded`, `Replaced`, or `Failed`. `dismiss_scene_operation` removes one completed status. A pending id returns `OperationPending`. `clear_scene_operations` removes every completed status and leaves pending ones. `Game::step` does not clear them. An unknown id is `NoSuchOperation`.

**Failures.** A scene `Err` is stored on the operation. The command returns `Ok`, so later commands still run and `command_failures` stays empty unless some other command failed. The failure is logged once, with the `log` crate, when the status becomes `Failed`. A later step does not log that record again. A failed load rolls its entities back. A failed replacement build leaves the original instance published. A failed unload leaves other instances in place. A panic is not undone.

**Stages.** A request made in `update` is still pending for a later system in that stage. `post_update` sees the completed status. A request made in one fixed iteration is visible to the next fixed iteration. No stage was added.

**Repository check.** `scripts/check.sh` on 2026-10-16, macOS 27.0 arm64, `rustc 1.99.0`, exit 0. All 4 steps passed: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (**348 passed, 0 failed, 0 ignored**), `cargo doc --workspace --no-deps` with `-D warnings`. `headless.rs` ran 20 tests. `spatial_scene` `configurations.rs` ran 16.

**Screenshots.** Release, 1280×720, rendered from this working tree and compared with `cmp -s` to the four PNGs rendered from commit `6dbabb8` (`6dbabb88f10779301115d04720ac1f52000ed03a`). All four byte-identical: `basic_2_5d` 746071 bytes, `basic_2_5d --ortho` 462130, `spatial_scene --camera perspective` 136991, `spatial_scene --camera orthographic` 34046. SHA-256: basic `c3c5874adeea119c98255c6c1d08208c0244224d930d86e8736c280ef88b38fe`, ortho `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f`, perspective `ec5f6252a88f642b454ae768f9eec8cdd5fba88a5aeb7a424fcefe5fda12275e`, orthographic `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde`.

## M4-4E results (2026-10-16)

Commit `d58c66b`. Not pushed. M4-4D is `8398e49`.

**Environments.** `build_environment_a` is the original `spatial_scene` content, built with `SceneBuilder`. `build_environment_b` is a yard of four floor tiles, a lit cube, a pillar, a figure, a tree, and one point light. Mesh assets are inserted once in `build_world`. Sprite and panel assets stay in `art::register`. Neither build spawns the camera or the walker, and neither inserts `SceneMember` by hand.

**Persistent entities.** The camera and the walker are ordinary `World::spawn` calls. They are not members of either `SceneInstanceId`. Replacement leaves their `Entity` values and transforms in place. The camera pose and projection stay on the example `Scene` resource. There is no new player controller.

**Switch.** Tab is bound to `switch_environment`. `request_environment_switch` runs in `update` and queues `CommandWriter::replace_scene` on the frame-scoped press. A held key does not queue again, and a press while one operation is pending does not queue another. `apply_environment_switch` runs in `post_update`. `Replaced` updates the active id and dismisses the record. `Failed` dismisses it and leaves the active id unchanged. The engine logs that failure once when the status is stored. `Loaded` and `Unloaded` are logged with the instance id. The example does not own those instances: an unexpected load stays published, an unexpected unload is left as the engine applied it, and the tracked id stays unchanged. `Pending` stays stored.

**Launch.** `cargo run -p spatial_scene` opens environment A. `--environment b` publishes B before the first step. The default screenshot path stays on A.

**Repository check.** `scripts/check.sh` on 2026-10-16, macOS 27.0 arm64, `rustc 1.99.0`, exit 0. All 4 steps passed: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (**359 passed, 0 failed, 0 ignored**), `cargo doc --workspace --no-deps` with `-D warnings`. `headless.rs` ran 20 tests on Apple M5, Metal. `spatial_scene` `configurations.rs` ran 16. `switching.rs` ran 12. The 359 total above was counted before the unexpected-unload test; the passing check for this commit included that twelfth test.

**Screenshots.** Release, 1280×720, rendered from this working tree and compared with `cmp -s` to the four PNGs rendered from commit `6dbabb8` (`6dbabb88f10779301115d04720ac1f52000ed03a`). All four byte-identical: `basic_2_5d` 746071 bytes, `basic_2_5d --ortho` 462130, `spatial_scene --camera perspective` 136991, `spatial_scene --camera orthographic` 34046. SHA-256: basic `c3c5874adeea119c98255c6c1d08208c0244224d930d86e8736c280ef88b38fe`, ortho `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f`, perspective `ec5f6252a88f642b454ae768f9eec8cdd5fba88a5aeb7a424fcefe5fda12275e`, orthographic `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde`. Environment B, same size, is a new render and does not replace those baselines: perspective 22179 bytes, SHA-256 `a3da6f1d3ce9543fc3cd04c7b677ad9498fbe7cde65857de157db69fedbcd9b8`; orthographic 21934 bytes, SHA-256 `b71ffbb52aa52e0a96d31dfc670752af45d379ebcb3da813800684c234dfb093`.

**Window.** `cargo run -p spatial_scene` stayed running, which is the windowed loop, and was then closed from this session. The framebuffer was not inspected. Keyboard switching, walker motion, and the absence of render errors were not observed in that window.

## M4-5 results (2026-10-16)

Commit `2b79806`. Not pushed. M4-4E is `d58c66b`. M4 is complete locally and is not pushed.

**Prefab.** `spatial_scene::prefab::spawn_lamp` takes `&mut World` and a position and returns the post `Entity`. The post is a lit pillar. A point light is its child. Each call creates a new hierarchy. `World::despawn` on the root removes the light. `place_lamp` builds the same parts through `SceneBuilder`, so a failed scene closure rolls them back and a successful load records `SceneMember`. `SceneBuilder` does not expose `&mut World`. Environment A and environment B do not spawn a lamp.

**API.** `World::create_scene_instance` and `World::track_scene_entity` are `pub(crate)`. No example and no crate outside `engine-world` called them. `scene_entity`, `scene_entities`, `scene_instances`, `scene_membership`, and the operation-result methods stay public. `SceneManager` stays internal.

**Lifecycle.** No accepted scene rule was redesigned. Ids still start at 0 and are not reused. A failed build still rolls entities back. Unload still lifts a non-member to a TRS approximation and drops shear. Deferred operations still flush with the existing command queue. Shared assets are still not removed. D-16 stays open.

**Manual check.** Launch with `cargo run -p spatial_scene`. Environment A is the original yard of figures and the checkerboard. Press Tab once: environment B replaces it, and the walker and camera stay. Hold Tab: the scene does not switch again. Move with W/A/S/D and the arrows. Press Tab again: environment A returns. On 2026-10-16 the process stayed in the windowed loop. CoreGraphics listed an on-screen window owned by `spatial_scene`, 1280×752 at (116, 83). `screencapture` could not read that window, and System Events refused assistive access, so the framebuffer, Tab, the walker, and the camera were not observed. A Tab event posted to the process is not evidence of a switch. The automated tests cover the sequence except the pixels on a real display.

**Repository check.** `scripts/check.sh` on 2026-10-16, macOS 27.0 arm64, `rustc 1.99.0`, exit 0. All 4 steps passed: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (**365 passed, 0 failed, 0 ignored**), `cargo doc --workspace --no-deps` with `-D warnings`. `headless.rs` ran 20 tests on Apple M5, Metal. `spatial_scene` `configurations.rs` ran 16. `switching.rs` ran 12. `prefab.rs` ran 5.

**Screenshots.** Release, 1280×720, rendered from this working tree and compared with `cmp -s` to the four PNGs rendered from commit `6dbabb8` (`6dbabb88f10779301115d04720ac1f52000ed03a`). All four byte-identical: `basic_2_5d` 746071 bytes, `basic_2_5d --ortho` 462130, `spatial_scene --camera perspective` 136991, `spatial_scene --camera orthographic` 34046. SHA-256: basic `c3c5874adeea119c98255c6c1d08208c0244224d930d86e8736c280ef88b38fe`, ortho `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f`, perspective `ec5f6252a88f642b454ae768f9eec8cdd5fba88a5aeb7a424fcefe5fda12275e`, orthographic `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde`. Environment B, same size, matches the M4-4E renders and does not replace those baselines: perspective 22179 bytes, SHA-256 `a3da6f1d3ce9543fc3cd04c7b677ad9498fbe7cde65857de157db69fedbcd9b8`; orthographic 21934 bytes, SHA-256 `b71ffbb52aa52e0a96d31dfc670752af45d379ebcb3da813800684c234dfb093`. Neither environment spawns the lamp.

## M5-1 results (2026-10-16)

Documentation checkpoint. Committed locally and not pushed. No renderer, texture, material, sampler, shader, or atlas code changed. No M5-2 code was added. M4 behaviour is unchanged. M5 stays not started.

**Decisions.** D-05 is ADR-0015, including the approved blended-mesh depth: the camera-space Z of the world-space bounding-sphere centre, an object-level approximation. D-27 is ADR-0016: per-sprite `pixels_per_unit`, default 16, no engine-wide constant. Texture colour space, samplers, partial updates, and atlas mip limits are ADR-0017. The final review corrected anisotropy (no power-of-two rule; normalize before create; no synchronous retry), upload alignment (`write_texture` keeps a tight row stride; 256-byte alignment is the buffer-copy path), the atlas mip formula (a provisional estimate, not a guarantee), and the split between GPU image storage and sampler objects. `AssetManager`'s single revision is unchanged.

**Roadmap.** The introduction no longer describes the 2026-10-13 engine as current. M5 acceptance states separate pixel expectations for perspective and isometric orthographic cameras, the batching compatibility list, mip-correct sub-rectangle updates, and atlas tests that establish the supported mip range. M6 owns chunk extraction. M13 reuses it. M9's freeze allows later public APIs through an ADR. M14 names M8, M11, and M12 as caster prerequisites. M17 splits core loading from content importers, including audio (M16), and generalises M12's glTF importer instead of adding a second parser. M20 uses the M4 registry and M19 serialization.

**Repository check.** `scripts/check.sh` on 2026-10-16, macOS 27.0 arm64, `rustc 1.99.0`, exit 0. All 4 steps passed: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (**365 passed, 0 failed, 0 ignored**, 41 suites), `cargo doc --workspace --no-deps` with `-D warnings`. `headless.rs` ran 20 tests. `spatial_scene` `configurations.rs` ran 16, `switching.rs` 12, `prefab.rs` 5. The log did not print the adapter name. `DIMORA_REQUIRE_GPU=1` was set, and those GPU tests passed, so a usable adapter was present. Screenshot baselines were not re-rendered: no renderer, texture, or shader file changed. The four images against `6dbabb8` remain the standing bar for later GPU work. The M4-5 window note is unchanged.

## M5-2 specification

M5-2A is committed locally as `c687d5b` and is not pushed. M5-2B is commit `11b914f` and is not pushed. M5-2C is commit `622ef80` and is not pushed. M5-3A is commit `b51f1fb` and is not pushed. M5-3B is commit `84d5ab1` and is not pushed. M5-4A is commit `bf4d9bb` and is not pushed. M5-4B is commit `1666695` and is not pushed. The paragraphs below are the M5-2 specification, not the current milestone boundary.

Texture parameters and mipmaps only. No `MaterialAsset`, no atlas packer, no sub-rectangle upload, no blended meshes, no shader rewrite beyond what a colour-space or mip sample requires. Defaults must keep the four screenshots against `6dbabb8` byte-identical unless a separately reviewed correctness issue requires a change.

**Preserved.** `Texture::from_rgba8`, PNG decoding, `TextureFilter`, `with_filter`, default sRGB, nearest-neighbour default filtering, clamp-to-edge default addressing, no mipmaps by default, existing texture asset handles, and existing sprite and mesh rendering. No new public `wgpu` type.

**Public API, in `engine-assets` unless noted.** Keep `TextureFilter::{Nearest, Linear}` and `Texture::with_filter`, which sets min and mag together. Add engine-owned types, not `wgpu` types:

- `ColorSpace::{Srgb, Linear}`, default `Srgb`.
- `AddressMode::{Clamp, Repeat, Mirror}`, default `Clamp`.
- `MipFilter::{Nearest, Linear}`.
- `Mipmaps::{None, Generate}`, default `None`.
- `SamplerDesc { min_filter, mag_filter, mip_filter: Option<MipFilter>, address_u, address_v, anisotropy: u8 }`. `anisotropy` 1 means off. `mip_filter: None` samples the base level.
- `Texture` gains `color_space`, `mips`, and `sampler: SamplerDesc`. `filter` stays. `with_filter` writes it and both min and mag. A texture built with `from_rgba8` or `decode` matches today's sampling: sRGB, nearest, clamp, no mips, anisotropy 1.

`Sprite.size` and `pixels_per_unit` are not touched (ADR-0016).

**GPU (`engine-renderer-wgpu`), after M5-2A.** `wgpu` 30.0.1. The device is created with default limits (`gpu.rs`). Upload format is `Rgba8UnormSrgb` or `Rgba8Unorm` from `ColorSpace`. The upload operation stays `Queue::write_texture` with a tight row stride unless a later path explicitly uses a buffer copy, in which case that path respects `COPY_BYTES_PER_ROW_ALIGNMENT`. Mip count is 1 unless `Mipmaps::Generate`. There is no `generate_mipmaps` on the device; the backend generates levels with a private blit. Normalize `SamplerDesc` on the CPU before creating a sampler (ADR-0017): reject anisotropy below 1; if anisotropy is above 1 and min, mag, or mip is not linear, set anisotropy to 1 and keep the filters; if the filters are linear and anisotropy is above 16, clamp to 16. If `DownlevelFlags::ANISOTROPIC_FILTERING` is absent, set anisotropy to 1, keep the filters, and log once. Create the sampler once. A HAL failure is a device error, not a retry. If the adapter lacks non-power-of-two mipmaps and the image is not power-of-two, upload one level and log once. GPU image storage is keyed by asset id, pixel storage, format, and mip configuration. Samplers are keyed by the normalized descriptor. A bind group pairs the two. A revision bump means look again; re-upload only when storage differs. `AssetManager` is not redesigned. Eviction stays ADR-0002. Scene unload does not call `remove`.

**Shaders.** Sampling stays `textureSample`. An sRGB format decodes in hardware. A linear format does not. Tint and `Color` stay linear and are not converted again. No new material buffer.

**Tests.** CPU: the M5-2A list below. GPU, `DIMORA_REQUIRE_GPU=1`, in the later checkpoints: a high-frequency texture minified with mips approaches the image mean, and without mips does not; the same pair under an orthographic camera whose scale changes; a linear texture's sample matches the stored channel within a stated tolerance and is not the sRGB curve; repeat versus clamp at an out-of-range UV; anisotropy above 1 with a nearest filter is submitted as anisotropy 1 with nearest filters. Screenshot regression for any GPU checkpoint: the four release 1280×720 images against `6dbabb8`, compared with `cmp -s`, baselines not replaced.

**Later checkpoints, after M5-2A.**

1. **Formats and sampler cache.** `scene_renderer.rs` upload and `GpuContext` limits. The backend reads `Texture.sampler` after `SamplerDesc::normalized`, and stops choosing the draw sampler from `Texture.filter`. A direct write to `filter` is not the sampled configuration. `ColorSpace` selects `Rgba8UnormSrgb` or `Rgba8Unorm`. `Mipmaps::None` stays one level. The sampler LOD range stays `0..32` so minification uses `min_filter`; the one-level view keeps the sample on that level. A `0..0` clamp is not used. Downlevel anisotropy forces 1 when the flag is absent, logs once, and creates the sampler once. A HAL failure is a device error. Re-upload pixels only when storage differs. GPU tests for linear sampling, clamp versus repeat, independent min and mag filters, and the anisotropy normalization. Screenshots compared. Done when defaults still match `6dbabb8` and the linear sample is not sRGB-decoded.
2. **Mip generation.** Private blit, mip count `floor(log2(max(width, height))) + 1`. `Mipmaps::None` stays one level. A mip filter does not allocate levels. When levels exist and no mip filter is set, the sampling view is the base level and LOD stays `0..32`. When a mip filter is set, LOD max is the last allocated level. GPU mean-versus-alias test in perspective and under orthographic zoom. NPOT fallback logged once, not a hard failure. Screenshots compared again. Done when both cameras show the mean-with-mips and alias-without-mips results, and the four baselines are unchanged.

Partial upload and the atlas packer are later M5 phases (ADR-0017). They are not part of these checkpoints.

## M5-2A specification

CPU texture descriptors and defaults. No GPU sampler creation, no mipmap generation, no `MaterialAsset`, no atlas packer, no partial upload, and no change to `engine-renderer`, `engine-renderer-wgpu`, or any shader.

**Crates and files.**

- `crates/engine-assets/src/texture.rs`: the types, constructors, normalization, and unit tests.
- `crates/engine-assets/src/lib.rs`: re-export the new public types next to `Texture` and `TextureFilter`.
- `crates/engine/src/lib.rs` prelude: re-export the same types next to `TextureFilter`. The prelude already exports `Texture` and `TextureFilter`. No `wgpu` type is added.
- Rustdoc on the new types and on `SamplerDesc::normalized`.
- `TASKS.md`: a results note when the checkpoint is done. ADR-0017 stays the contract. If a type name in this specification changes during implementation, update ADR-0017 in the same commit.
- `docs/CAPABILITIES.md`: the texture "Now" row still describes GPU behaviour. Add one sentence that the CPU descriptor types exist and that the GPU path still reads `Texture.filter`. Do not mark M5 or M5-2 implemented.

Do not edit `crates/engine-renderer-wgpu/src/scene_renderer.rs`, `gpu.rs`, `sprite.wgsl`, `mesh.wgsl`, or `common.wgsl`. Do not edit `AssetManager`.

**Types and methods.** All new enums and `SamplerDesc` derive `Copy`, `Clone`, `Debug`, `PartialEq`, `Eq`, `Hash`, and `Default` where a default exists. Add `Hash` to `TextureFilter` so `SamplerDesc` can derive `Hash`.

- `ColorSpace::{Srgb, Linear}`, default `Srgb`.
- `AddressMode::{Clamp, Repeat, Mirror}`, default `Clamp`.
- `MipFilter::{Nearest, Linear}`. No default of its own; absence is `Option::None`.
- `Mipmaps::{None, Generate}`, default `None`.
- `SamplerDesc { min_filter: TextureFilter, mag_filter: TextureFilter, mip_filter: Option<MipFilter>, address_u: AddressMode, address_v: AddressMode, anisotropy: u8 }`. Default: nearest, nearest, `None`, clamp, clamp, anisotropy 1.
- `SamplerConfigError::AnisotropyBelowOne`.
- `SamplerDesc::normalized(self) -> Result<Self, SamplerConfigError>`.

`Texture` keeps `width`, `height`, `rgba`, and `filter`, and gains `color_space: ColorSpace`, `mips: Mipmaps`, and `sampler: SamplerDesc`.

- `from_rgba8`, `solid`, `from_fn`, and `Asset::decode` keep their signatures. They set sRGB, `Mipmaps::None`, nearest `filter`, and the default `SamplerDesc`.
- `with_filter(self, TextureFilter) -> Self` sets `filter` and both `sampler.min_filter` and `sampler.mag_filter`. Address, mip filter, colour space, and mipmap generation stay. If anisotropy is above 1 and the resulting filters are not all linear, anisotropy becomes 1.
- `with_color_space(self, ColorSpace) -> Self`.
- `with_mipmaps(self, Mipmaps) -> Self`.
- `with_sampler(self, SamplerDesc) -> Result<Self, SamplerConfigError>` stores `desc.normalized()?`. It sets `filter` to `Linear` when both min and mag are `Linear`, and to `Nearest` otherwise. The current backend still reads `filter` until the GPU checkpoint, so a split min and mag is stored on `sampler` and is not visible in a draw during M5-2A.

**Defaults.** A value from `from_rgba8` or PNG decode has `color_space == Srgb`, `mips == Mipmaps::None`, `filter == Nearest`, `sampler.min_filter == Nearest`, `sampler.mag_filter == Nearest`, `sampler.mip_filter == None`, `sampler.address_u == Clamp`, `sampler.address_v == Clamp`, and `sampler.anisotropy == 1`.

**Validation, CPU only.** `normalized` applies these rules and no others:

- Anisotropy 0 returns `Err(SamplerConfigError::AnisotropyBelowOne)`.
- Anisotropy above 1 with nearest min, nearest mag, `mip_filter: None`, or `mip_filter: Some(Nearest)` returns anisotropy 1 and keeps those filters.
- Anisotropy from 2 through 16 with linear min, linear mag, and `Some(Linear)` is unchanged.
- Anisotropy above 16 with those three linear filters becomes 16.
- Anisotropy above 16 with any nearest filter becomes 1, not 16.
- Values 3, 5, 6, and 7 are accepted when the three filters are linear. There is no power-of-two rejection.
- Downlevel flags and a failed GPU `create_sampler` are not represented here. ADR-0017 assigns them to the GPU checkpoint.

**Unit tests, in `texture.rs`.** `from_rgba8_defaults_match_todays_sampling`; `png_decode_uses_the_same_defaults` (a tiny in-memory PNG, or `image` writing one into a buffer); `with_filter_sets_min_and_mag`; `anisotropy_zero_is_rejected`; `anisotropy_with_nearest_keeps_nearest`; `anisotropy_without_linear_mips_drops_to_one`; `linear_anisotropy_above_16_clamps_to_16`; `non_power_of_two_anisotropy_is_kept`; `with_sampler_stores_the_normalized_descriptor`.

**Completion.** Existing constructors compile. Default sampling fields equal today's behaviour. The CPU tests pass. `scene_renderer.rs` still compiles against `Texture.filter` without an edit. `scripts/check.sh` passes. Screenshots are not required for this checkpoint because the upload path is unchanged; do not claim a new byte comparison unless it is run. M5 stays not complete. M5-2B has not started. No GPU sampler, mip chain, packer, or partial update exists at the end of M5-2A.

## M5-2A results (2026-10-18)

Committed locally and not pushed.

**API.** `engine-assets` adds `ColorSpace`, `AddressMode`, `MipFilter`, `Mipmaps`, `SamplerDesc`, and `SamplerConfigError`. They are re-exported from `engine-assets` and the `engine` prelude. `Texture` keeps `width`, `height`, `rgba`, and `filter`, and gains `color_space`, `mips`, and `sampler`. `from_rgba8`, `solid`, `from_fn`, and PNG `decode` keep their signatures and the previous sampling defaults. No caller outside `texture.rs` built a `Texture` literal or called `with_filter`. The only reader of `Texture.filter` is `scene_renderer.rs`, which was not edited.

**Legacy filter.** `sampler` holds the full descriptor, including distinct min and mag filters. `filter` is the summary the current backend reads. `with_sampler` sets it to linear only when both min and mag are linear. A direct write to the public `filter` field does not update `sampler`. `with_filter` updates both and reduces anisotropy when the new filters cannot keep a value above 1.

**Mipmaps.** `Mipmaps` and `mip_filter` are stored independently. A one-level texture with a mip filter is valid. wgpu 30.0.1 does not reject that sampler; sampling clamps to the levels that exist. M5-2B clamps LOD to the base level in that case. No mip chain is generated here.

**Repository check.** `scripts/check.sh` on 2026-10-18, macOS 27.0 arm64, `rustc 1.99.0`, exit 0. All 4 steps passed: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (**387 passed, 0 failed, 0 ignored**, 41 suites), `cargo doc --workspace --no-deps` with `-D warnings`. `headless.rs` ran 20 tests. `spatial_scene` `configurations.rs` ran 16, `switching.rs` 12, `prefab.rs` 5. The log did not print the adapter name. `DIMORA_REQUIRE_GPU=1` was set, and those GPU tests passed, so a usable adapter was present. Screenshots were not re-rendered. The four baselines against `6dbabb8` were not compared in this checkpoint.

## M5-2B results (2026-10-19)

Implemented in this checkpoint. Not pushed.

**Legacy filter.** `Texture.sampler` is the configuration the GPU reads. `filter` is private. `filter()` is linear only when min and mag are both linear. `with_filter` still sets both filters. `with_sampler` keeps them independent. No crate outside `texture.rs` assigned the field. The renderer no longer reads it.

**GPU format.** `ColorSpace::Srgb` uploads `Rgba8UnormSrgb`. `ColorSpace::Linear` uploads `Rgba8Unorm`. The RGBA8 bytes are not converted on the CPU. Shaders, lighting, and transparency are unchanged.

**Samplers.** `SamplerDesc::normalized` runs before `Device::create_sampler`, including after a direct write to the public descriptor fields. Anisotropy 0 is logged once and becomes 1. The renderer queries `adapter.get_downlevel_capabilities().flags` once. If `DownlevelFlags::ANISOTROPIC_FILTERING` is absent, anisotropy above 1 becomes 1, the other settings stay, and that fallback is logged once. The engine maximum stays 16. Non-power-of-two values are kept. `create_sampler` is not retried. The cache key is the effective descriptor: min, mag, linear mip, address modes, anisotropy, and LOD `0..32`. A `0..0` clamp made one-level minification use the magnification filter, so the upper bound stays at wgpu's default. The one-level view is what keeps the sample on the base level. `Mipmaps::None` and nearest mip filtering share one entry. Every texture allocates one mip level. `Mipmaps::Generate` is stored and is not built.

**Storage.** An unchanged revision does not hash pixels. A new revision hashes once and re-uploads only when dimensions, format, allocated mip count, or the pixel fingerprint differ. A sampler-only change rebuilds the bind group and keeps the image. Image eviction is unchanged (D-04). Shared samplers stay while a cached image or a placeholder uses them. `AssetManager` is unchanged. D-16 is unchanged.

**Adapter.** GPU tests that printed the adapter reported Apple M5, Metal, `IntegratedGpu`, with `DownlevelFlags::ANISOTROPIC_FILTERING` present. No maximum above 16 was queried. The absent-flag path was unit-tested and was not run on a device that lacks the flag. `scripts/check.sh` hides output from passing tests, so that log does not repeat the adapter line.

**Single-level filtering.** A focused GPU test renders one mip level with contrasting adjacent texels. Magnified samples follow `mag_filter`. Minified samples, 32 texels across 8 pixels, follow `min_filter`: linear min mixes the edge and nearest min stays on one texel. The reverse pair does the opposite. The allocated view stays one level.

**Repository check.** `scripts/check.sh` on 2026-10-19, after the LOD correction, macOS 27.0 arm64, `rustc 1.99.0`, exit 0. All 4 steps passed: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (**399 passed, 0 failed, 0 ignored**, 41 suites), `cargo doc --workspace --no-deps` with `-D warnings`. `headless.rs` ran 26 tests. `spatial_scene` `configurations.rs` ran 16, `switching.rs` 12, `prefab.rs` 5.

**Screenshots.** Release 1280×720 images rendered on 2026-10-19 from this tree and from `6dbabb8`, compared with `cmp -s`. All four matched after the LOD correction. Baselines were not replaced. `basic_2_5d` 746071 bytes, SHA-256 `c3c5874adeea119c98255c6c1d08208c0244224d930d86e8736c280ef88b38fe`. `basic_2_5d --ortho` 462130 bytes, `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f`. `spatial_scene --camera perspective` 136991 bytes, `ec5f6252a88f642b454ae768f9eec8cdd5fba88a5aeb7a424fcefe5fda12275e`. `spatial_scene --camera orthographic` 34046 bytes, `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde`.

## M5-2C results (2026-10-19)

Commit `622ef80`. Not pushed. M5-2B is commit `11b914f`.

**Allocation.** `Mipmaps::None` is one level and is not blitted. `Mipmaps::Generate` is `floor(log2(max(width, height))) + 1`, including 1×1 (one level), power-of-two, non-power-of-two, non-square, 1×N, and N×1. The count is `Mipmaps::level_count`. A mip filter does not change it.

**Generation.** `engine-renderer-wgpu` owns a private blit. Each level is a separate pass: a view of the previous mip is sampled, and a view of the next mip is the color attachment. The blit pipeline is one per RGBA8 format, not one per asset. The blit sampler is created once. sRGB formats decode on sample and encode on write. Linear formats do not. Alpha is a straight average. On Apple M5 Metal the 1×1 mip of a 2×2 black/white checker was `[188, 188, 188, 255]` for `Rgba8UnormSrgb` and `[128, 128, 128, 255]` for `Rgba8Unorm`.

**LOD.** A sampling view that contains only the base level keeps LOD `0..32`, including a generated chain whose sampler has no mip filter. When a mip filter is set, the view includes the allocated chain and the LOD maximum is the last level. `0..0` is not used: on this device it selects the magnification filter.

**Device.** `DownlevelFlags::NON_POWER_OF_TWO_MIPMAPPED_TEXTURES` is queried once. Apple M5, Metal, `IntegratedGpu` reported the flag, so non-power-of-two chains were generated. The one-level decision for a missing flag is a CPU test. It was not the hardware path. A format that cannot be filtered and written as a render attachment also stays at one level.

**Caches.** Sampler-only changes do not upload pixels or rebuild mips. Pixel changes rewrite the base level and rebuild the chain. A mip-count or colour-space change replaces the image. Removal still follows D-04. `mip_generations` counts chain builds. The blit sampler counts as one sampler creation.

**Repository check.** `scripts/check.sh` on 2026-10-19, macOS 27.0 arm64, `rustc 1.99.0`, exit 0. All 4 steps passed: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (**413 passed, 0 failed, 0 ignored**, 41 suites), `cargo doc --workspace --no-deps` with `-D warnings`. `headless.rs` ran 32 tests. `spatial_scene` `configurations.rs` ran 16, `switching.rs` 12, `prefab.rs` 5. The passing-test log hides adapter lines. A direct GPU run printed Apple M5, Metal, `IntegratedGpu`, with both `ANISOTROPIC_FILTERING` and `NON_POWER_OF_TWO_MIPMAPPED_TEXTURES` present.

**Screenshots.** Release 1280×720 images rendered on 2026-10-19 from this tree and from `6dbabb8`, compared with `cmp -s`. All four matched. Baselines were not replaced. `basic_2_5d` 746071 bytes, SHA-256 `c3c5874adeea119c98255c6c1d08208c0244224d930d86e8736c280ef88b38fe`. `basic_2_5d --ortho` 462130 bytes, `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f`. `spatial_scene --camera perspective` 136991 bytes, `ec5f6252a88f642b454ae768f9eec8cdd5fba88a5aeb7a424fcefe5fda12275e`. `spatial_scene --camera orthographic` 34046 bytes, `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde`.

## M5-3A results (2026-10-19)

Commit `b51f1fb`. Not pushed. M5-2C is commit `622ef80`. This commit records CPU transitions and does not upload them.

**API.** `Texture::update_region(x, y, width, height, rgba)` writes one tightly packed RGBA8 rectangle into the existing buffer. Source rows are `width * 4` bytes. Destination rows stay `width_of_texture * 4` apart. Dimensions, colour space, sampler, mip preference, and asset id stay. Bytes are not converted between sRGB and linear. `AssetManager` is unchanged: the game clones the texture, edits the clone, and `insert`s it under the same name. That bumps the revision.

**Validation.** `TextureRegionError` is returned before any write. Zero width, zero height, either edge past the texture, `u32` coordinate overflow, byte-length overflow, too few bytes, too many bytes, and a buffer shorter than the claimed dimensions are rejected. Pixels and `Texture::pixel_edits` stay as they were. The error does not store an asset name. `AssetManager::name_of` is how a caller adds one. `AssetHandle` still stores only the name hash.

**Dirty regions.** `TexturePixelEdits::Clean` is a texture with no successful region update, including a whole-texture replace. `Journal` stores at most 8 transitions, oldest first. Each transition is the fingerprint before that update, the fingerprint after it, and the rectangle. A ninth transition drops the oldest and keeps the newest eight. Sampler, colour-space, and mip-preference changes do not mark pixels dirty. Writing `rgba` directly does not update the record. `base_upload` returns `Unchanged` when the current fingerprint matches the cached GPU image. It returns `Partial` for the newest contiguous chain from that GPU fingerprint to the current bytes, including a later frame whose GPU image is a middle entry. It returns `Full` when the GPU image is older than the retained window, the value is a whole-texture replace, or the bytes were written outside `update_region`. Several edits before the next `insert` stay on that one value. The renderer does not see intermediate revisions it was never given.

**Mips.** A successful region update reports `generated_mips_may_be_stale`. No mip is built on the CPU. `affected_mip_rect` names destination texels from the `mip.wgsl` linear footprint, including odd sizes where a middle texel can miss every destination. A miss ends the chain. M5-3B may expand by one texel or rebuild the level. A full chain rebuild is a correct fallback and is not a partial mip update.

**Not in this checkpoint.** `Queue::write_texture` for a sub-rectangle, GPU mip regeneration, device-limit checks before `create_texture`, and a new missing-texture log. `validate_texture_dimensions` is CPU-only and takes injected limits. Extraction already logs a missing renderable once per id and includes `name_of` when an entry exists. A sprite texture miss inside a present sprite, and a mesh `base_texture` miss, still draw the existing placeholder (magenta for sprites, white for meshes) and are not separately logged. The handle cannot recover a name that was never stored. Do not invent one.

**Repository check.** `scripts/check.sh` on 2026-10-19, macOS 27.0 arm64, `rustc 1.99.0`, exit 0, after the transition journal. All 4 steps passed: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (**438 passed, 0 failed, 0 ignored**, 41 suites), `cargo doc --workspace --no-deps` with `-D warnings`. `headless.rs` ran 32 tests. `spatial_scene` `configurations.rs` ran 16, `switching.rs` 12, `prefab.rs` 5. The passing-test log hides adapter lines. GPU tests passed with `DIMORA_REQUIRE_GPU=1`. GPU partial uploads are not in this commit.

**Screenshots.** GPU sampling and the upload path are unchanged in this commit, so the four release screenshots were not regenerated for M5-3A. The M5-2C comparison against `6dbabb8` remains the baseline for that commit.

## M5-3B results (2026-10-19)

Committed locally as `84d5ab1`. Not pushed. M5-3A is commit `b51f1fb`. The checks below are the ones recorded for that commit.

**Upload selection.** A new asset revision hashes pixels once. An unchanged revision does not. Matching dimensions, format, and mip allocation keep the GPU image. `base_upload` then returns `Unchanged` (no pixel write, no mip rebuild), `Partial` (each proved rectangle, from the current CPU bytes), or `Full`. A partial or full base upload regenerates the allocated chain with the M5-2C blit. `Mipmaps::None` does not blit. Sampler-only changes do not upload pixels or rebuild mips. Dimensions, colour space, or mip allocation still replace the image. Removal and idle eviction are unchanged.

**Rectangles.** Each partial write uses mip 0, origin `(x, y, 0)`, the rectangle extent, and tightly packed RGBA8 (`bytes_per_row = 4 * width`). Rows are not padded to 256 bytes. Several rectangles, including overlaps and a skipped revision, are written before the mip rebuild. The GPU image then matches the latest CPU bytes. A journal that cannot prove that difference, including a GPU fingerprint older than the eight retained transitions, uses one full base upload. Later edits can be partial again. A failed dimension or byte check does not update the cached revision or fingerprint.

**Counters.** `full_base_uploads` counts whole base writes, including the two placeholders. `partial_base_uploads` counts one `write_texture` per rectangle. `base_upload_bytes` is logical source bytes, not measured bandwidth. `mip_generations` counts a whole-chain rebuild and is not a partial mip update. Texture, sampler, bind-group, and fingerprint counters are unchanged in meaning. A partial pixel update does not recreate the texture, view, sampler, or bind group unless sampling actually changes.

**Limits.** `create_texture` is preceded by `validate_texture_dimensions` against `max_texture_dimension_2d`. `set_texture_dimension_limits` can only tighten that limit. A rejection logs the asset name when `name_of` has one, otherwise the id, and does not allocate.

**Missing textures.** A present sprite with an absent texture still draws and samples the magenta placeholder. A mesh with `base_texture: Some` and no image still samples the white placeholder. `None` is untextured white and is not a warning. The miss logs once per texture id. A registered name is included. A handle that was never registered, or whose entry was removed, has no name on `AssetHandle`; the log uses the id and the parent sprite or mesh name when that entry exists. It does not invent a texture name. A pending load does not warn. The warning memory holds 64 ids, does not grow, and does not rotate. A distinct miss after that is still counted in `missing_assets` and is not logged individually. The first dropped id logs one aggregate line. Later drops stay silent until a remembered asset loads and frees a slot. Worlds without `MissingAssetWarnings` log every miss.

**Not in this checkpoint.** Localized mip regeneration, shelf atlas packing, sprite-sheet animation, `MaterialAsset`, blended meshes, texture compression, and streaming. M5 stays in progress. Shelf packing is M5-4A, below.

**Repository check.** `scripts/check.sh` on 2026-10-19, macOS 27.0 arm64, `rustc 1.99.0`, exit 0, including the 64-id aggregate warning. All 4 steps passed: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (**449 passed, 0 failed, 0 ignored**, 41 suites), `cargo doc --workspace --no-deps` with `-D warnings`. `headless.rs` ran 39 tests. `spatial_scene` `configurations.rs` ran 16, `switching.rs` 12, `prefab.rs` 5. A direct GPU run printed Apple M5, Metal, `IntegratedGpu`, with `ANISOTROPIC_FILTERING` and `NON_POWER_OF_TWO_MIPMAPPED_TEXTURES` both present. This paragraph was written after that check.

**Screenshots.** Release 1280×720 images rendered on 2026-10-19 from this working tree and compared with `cmp -s` to the `6dbabb8` baselines. All four matched. Baselines were not replaced. `basic_2_5d` 746071 bytes, SHA-256 `c3c5874adeea119c98255c6c1d08208c0244224d930d86e8736c280ef88b38fe`. `basic_2_5d --ortho` 462130 bytes, `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f`. `spatial_scene --camera perspective` 136991 bytes, `ec5f6252a88f642b454ae768f9eec8cdd5fba88a5aeb7a424fcefe5fda12275e`. `spatial_scene --camera orthographic` 34046 bytes, `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde`.

## M5-4A results (2026-10-19)

Committed locally as `bf4d9bb` and not pushed. M5-3B is commit `84d5ab1`. GPU atlas acceptance is M5-4B, below, and is not part of this commit. M5 stays in progress.

**API.** `AtlasBuilder`, `AtlasConfig`, `Atlas`, `AtlasRegion`, and `AtlasError` live in `engine-assets` and are re-exported from the engine prelude. The builder stores copies of named RGBA8 images. `build` returns one `Texture` and one visible region per image. It does not hold a world or an `AssetManager`, does not create a GPU texture, and does not make regions into entities. Registration is the caller's `insert` of `Atlas::texture`. `Atlas::sprite` calls `SpriteAsset::from_atlas_rect` on the visible rectangle.

**Order.** Placement follows `add` order. Names and heights are not sorted. `HashMap` iteration is not used. The same images, order, and config produce the same placements and the same atlas bytes.

**Shelves.** The cursor starts at the top-left. Each image's allocated rectangle is `(width + 2 * padding)` by `(height + 2 * padding)`. Shelves fill left to right. A rectangle that would pass the atlas width starts a new shelf below the tallest allocated height on the current shelf. Allocated rectangles do not overlap and may touch. The visible origin is the allocated origin plus `padding`. The visible size is the source size. The configured width and height are the atlas size. The builder does not grow it.

**Padding and extrusion.** Padding is the gutter inside the allocated rectangle. Extrusion copies the source edge, including corners, outward into that gutter, all four channels, with no premultiplication. Extrusion wider than padding is an error. The outer `padding - extrusion` texels stay transparent black. Extrusion stays inside its own allocated rectangle. Source bytes passed to `add` are not written back.

**Regions.** `AtlasRegion` stores the name, visible origin and size, source size, and a UV rectangle. UV uses the atlas size. `(0, 0)` is the top-left texel. There is no Y flip and no half-texel bias. `pixels_per_unit` stays 16. Atlas position does not change world size. `Texture::update_region` can replace a visible rectangle later. That does not refresh extrusion or move regions. The supported gutter refresh is `Atlas::replace_source` in M5-4B, not in `bf4d9bb`. A source whose dimensions change needs a new build, and old metadata must not be reused.

**Texture configuration.** One atlas has one colour space, one sampler, and one mip request. Bytes are not converted between sRGB and linear. The default is sRGB, nearest, clamp, anisotropy 1, and `Mipmaps::None`. `Mipmaps::None` is the recommended atlas sampling policy. `Mipmaps::Generate` stores the request only. Whole-atlas mip generation can mix neighbours at coarse levels. `floor(log2(padding))` is still an estimate, not a guarantee, and the builder does not enforce it. Renderer LOD is unchanged.

**Errors.** `AtlasError` names the image when the caller supplied a name. Empty input, an empty name, duplicates, a zero side, truncated or excess RGBA8, a source larger than the atlas, padding that makes a fitting source too large, a shelf with no room, extrusion wider than padding, `u32` overflow, an atlas byte length that does not fit in `usize`, a zero atlas side, and anisotropy below 1 are errors. `build` returns `Err` and does not yield an atlas. The packer does not log.

**Not in this checkpoint.** The GPU atlas tests and `Atlas::replace_source` are M5-4B and are not in `bf4d9bb`. Sprite-sheet animation, `SpriteClip`, `SpriteAnimator`, `MaterialAsset`, blended meshes, a new upload path, incremental repacking, an atlas manager, streaming, compression, and localized mip regeneration are not in this commit. M5 stays in progress.

**Repository check.** `scripts/check.sh` on 2026-10-19, macOS 27.0 arm64, `rustc 1.99.0`, exit 0, after the CPU shelf packer. All 4 steps passed: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (**470 passed, 0 failed, 0 ignored**, 41 suites), `cargo doc --workspace --no-deps` with `-D warnings`. `headless.rs` ran 39 tests. `spatial_scene` `configurations.rs` ran 16, `switching.rs` 12, `prefab.rs` 5. A direct GPU run of `srgb_and_linear_textures_select_different_gpu_formats` printed `GPU test adapter: Apple M5 (Metal, IntegratedGpu); DownlevelFlags::ANISOTROPIC_FILTERING=true`. This paragraph was written after that check.

**Screenshots.** No renderer, shader, or scene-extraction file changed, so the four release screenshots were not regenerated. The M5-3B `cmp -s` against `6dbabb8` remains the baseline. This checkpoint does not repeat that comparison.

## M5-4B results (2026-10-19)

Committed locally as `1666695`. Not pushed. M5-4A is commit `bf4d9bb`. M5 stays in progress.

**Integration.** `crates/engine-renderer-wgpu/tests/atlas.rs` packs known RGBA8 images, registers the texture, and builds each `SpriteAsset` with `Atlas::sprite`. The UV rectangles are the ones the packer returned. The existing extract and GPU renderer draw the sprites. Readback, not the CPU buffer alone, checks the pixels. An 8×4 nearest atlas keeps each region's texels, including a non-square image, a second shelf, and a region on the texture boundary. The image is not flipped. Padding is not part of `size_px` or the world size. `pixels_per_unit` stays 16. The default pivot stays bottom-centre.

**Filtering.** Linear magnification of a full-screen region reaches about half a texel past the visible rectangle. Padding 0 mixes the neighbour. Padding without extrusion mixes the clear gutter and darkens over black. Extrusion of 1 keeps that footprint on the copied edge. Extrusion of 2 matches that magnified edge and is visible as a second gutter texel on the base mip. These are the tested footprints. Linear filtering is not universally safe for an arbitrary tightly packed atlas.

**Mips.** Whole-atlas chains use the existing blit. `floor(log2(padding))` is not the pass condition. On Apple M5 / Metal, linear colour space: a padding-0 1×2 pair mixes on level 1; a padding-0 6×4 pair whose boundary sits between 2×2 groups stays pure on level 1 and mixes on level 2; padding 1 with extrusion 0 keeps neighbours apart through level 2 while mixing the clear gutter into the region; padding 1 with extrusion 1 keeps that layout opaque and apart through level 2 and mixes on level 3; padding 2 with extrusion 2 on 4×4 regions in a 16×16 atlas stays apart through level 3 and mixes on level 4. A red region from that last atlas, at about mip 1, stayed red with nearest and linear mip filters. Coarser levels outside a measured layout may blend. Per-region mip generation is not implemented.

**Colour and alpha.** Atlas bytes are unchanged by construction and by the base upload. A linear render target decodes sRGB and leaves linear bytes alone. Generated mips follow M5-2C. Straight alpha is unchanged. A clear edge without a gutter picks up the neighbour under linear magnification. An extruded clear edge does not. Partial alpha over black is ordinary coverage. A mip of opaque red and green at alpha 128 averages stored channels and does not premultiply.

**Cameras.** One sprite component was sampled under perspective, orthographic, and orthographic yawed 45° and pitched by `atan(1/sqrt(2))`. Perspective depth and orthographic zoom minify the sprite and keep the region colour. No projection branch was added.

**Updates.** A visible-only `update_region` leaves the GPU gutter stale. `Atlas::replace_source` writes the visible rectangle expanded by the stored extrusion in one `Texture::update_region`. Coordinates, layout, and other regions stay. The GPU path is the existing partial upload: one rectangle, the expanded byte count, the same image, sampler, and bind group. An allocated chain is rebuilt once. Several sprites of one atlas allocate one image and do not upload it again per sprite. Removing the texture follows the existing eviction rule.

**Validation.** CPU atlas errors are unchanged. An 8×4 atlas against an injected 4×4 limit allocates nothing, logs the registered name and "exceeds" once, and the sprite draws magenta. A never-registered texture handle logs the id and the sprite name. It does not log an invented texture name.

**Not in this checkpoint.** No atlas demo example. The four historical scenes were not modified. No `SpriteSheet`, `SpriteClip`, `SpriteAnimator`, `MaterialAsset`, blended meshes, new alpha mode, material-batching change, automatic repack, atlas manager, localized mip regeneration, streaming, compression, or custom shaders. The 1,000-sprite batch stays with M5-5.

**Repository check.** `scripts/check.sh` on 2026-10-19, macOS 27.0 arm64, `rustc 1.99.0`, exit 0, with M5-4B in the working tree. All 4 steps passed: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (**481 passed, 0 failed, 0 ignored**, 42 suites), `cargo doc --workspace --no-deps` with `-D warnings`. `atlas.rs` ran 9 tests. `headless.rs` ran 39. `spatial_scene` `configurations.rs` ran 16, `switching.rs` 12, `prefab.rs` 5. A direct GPU run printed `GPU test adapter: Apple M5 (Metal, IntegratedGpu); DownlevelFlags::ANISOTROPIC_FILTERING=true`. This paragraph was written after that check.

**Screenshots.** Release 1280×720 images rendered on 2026-10-19 from this working tree and from commit `6dbabb8`, compared with `cmp -s`. All four matched. Baselines were not replaced. `basic_2_5d` 746071 bytes, SHA-256 `c3c5874adeea119c98255c6c1d08208c0244224d930d86e8736c280ef88b38fe`. `basic_2_5d --ortho` 462130 bytes, `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f`. `spatial_scene --camera perspective` 136991 bytes, `ec5f6252a88f642b454ae768f9eec8cdd5fba88a5aeb7a424fcefe5fda12275e`. `spatial_scene --camera orthographic` 34046 bytes, `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde`. The commit-time re-run of that comparison matched the same four hashes.

## M5-5A results (2026-10-19)

Committed locally as `e0c29ef` on 2026-10-20. Not pushed. M5-4B is commit `1666695`. The check below ran on the same tree before that commit. M5 stays in progress. M5-5B consumes this asset and is recorded in the next section.

**API.** `MaterialAsset`, `AlphaMode`, `AlphaCutoff`, and `MaterialError` live in `engine-assets` and are re-exported from the engine prelude. The asset is plain data: linear `Color`, optional albedo `AssetHandle<Texture>`, optional normal `AssetHandle<Texture>`, alpha mode, `lit`, and `double_sided`. It has no GPU type, shader, bind group, pipeline, entity, or world. `engine-assets` depends on `engine-core` for `Color`. That edge does not cycle. The inline mesh `Material` keeps its name and stays the component field.

**Defaults.** `MaterialAsset::default` is linear white, no textures, `AlphaMode::Opaque`, `lit: true`, `double_sided: false`. `lit: true` matches `Material::default` and `Sprite::from_asset`. Opaque matches a mesh, which is always opaque today. It does not match `SpriteBlend::Alpha`. Unassigned sprites and meshes keep that legacy path. `AlphaCutoff::DEFAULT` is `0.5`.

**Validation.** `AlphaCutoff::new` and `AlphaMode::mask` reject NaN, infinities, negatives, and values above 1. `0.0` and `1.0` are accepted. The cutoff cannot be stored except through that check. `MaterialAsset::validate` rejects a non-finite base colour and allows finite components outside `0.0..=1.0`, including negative and HDR values. It does not resolve texture handles.

**Ownership.** `AssetManager::insert` stores a material. A same-name replacement changes the material-store revision. Several materials can hold one `AssetHandle<Texture>`. There is no material reference count and no scene ownership of the asset. The normal handle is stored and not shaded.

**Precedence, not implemented.** No handle preserves today's sprite tint, `SpriteBlend`, sprite lighting, `SpriteAsset` texture, inline mesh material, and tilemap cutout. A set handle supplies class, cutoff, lit, double-sided, base colour, and albedo, and sprite tint still multiplies. `SpriteBlend` is then ignored. Four clarifications need approval before M5-5B: albedo `None` falls back to the sprite texture and leaves a mesh untextured; a missing material skips the renderable instead of using the legacy fields; the assigned colour is sample times base colour times tint; `double_sided: false` back-face culls an assigned sprite, while an unassigned sprite stays uncullled. Details are in ADR-0015.

**Handles for M5-5B.** Add `Option<AssetHandle<MaterialAsset>>` to `Sprite` and `Mesh`, default `None` in the existing constructors. Do not add it to `TileMap` then. Do not remove the inline `Material`. This checkpoint does not add the field, so current construction is unchanged.

**GPU and batching, not implemented.** Identity is the asset id. Revision is the store revision. A colour, lit, or cutoff change is uniform or instance data. A class or `double_sided` change selects a pipeline. An albedo change rebinds through the existing texture cache. The normal slot is not bound. There is no material GPU cache. Today's sprite batches are consecutive equal textures. Meshes batch on mesh plus texture. M5-5D adds material id, revision, pipeline class, and sampler. Sharing a material is not enough to merge a draw. Blended runs stay in sorted order.

**Not in this checkpoint.** GPU binding, new pipelines, material fields on renderables, blended meshes, unified transparent extraction, material-aware batching, lighting changes, normal-map shading, sprite sheets, atlas repacking, localized mips, a material graph, and asset reference counts.

**Repository check.** `scripts/check.sh` on 2026-10-19, macOS 27.0 arm64, `rustc 1.99.0`, exit 0, with M5-5A in the working tree. All 4 steps passed: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (**488 passed, 0 failed, 0 ignored**, 42 suites), `cargo doc --workspace --no-deps` with `-D warnings`. `engine-assets` ran 99 lib tests. `atlas.rs` ran 9. `headless.rs` ran 39. `spatial_scene` `configurations.rs` ran 16, `switching.rs` 12, `prefab.rs` 5. The passing-test log does not print the adapter. The GPU tests ran because `DIMORA_REQUIRE_GPU=1` was set. This paragraph was written after that check.

**Screenshots.** Renderer, shaders, extraction, and the four historical scenes are unchanged, so M5-5A does not render them again. The M5-4B `cmp -s` against `6dbabb8` remains the baseline.

## M5-5B results (2026-10-20)

Committed locally as `3b3cd4a` on 2026-10-20. Not pushed. M5-5A is commit `e0c29ef`. M5 stays in progress. The check below ran on the same tree before that commit.

**Handles.** `Sprite.material` is `Option<AssetHandle<MaterialAsset>>`, default `None`, set with `with_material`. `Mesh.material` remains the inline `Material`. `Mesh.material_asset` is the optional handle, default `None`, set with `with_material_asset`. `TileMap` has no material handle. Existing constructors still build an unassigned renderable.

**Resolution.** Extraction resolves the handle on the calling thread and copies colour, class, cutoff, lit, culling, texture id, material id, and material revision into `RenderScene`. The snapshot holds no world or asset-manager borrow. A missing or failed material skips that renderable and logs once, with `name_of` when the entry exists and the id otherwise. A load that is still pending does not log. A present material with a missing albedo keeps the existing placeholder: magenta for a sprite, white for a mesh. A mesh whose material is `AlphaMode::Blend` is not submitted. That logs once per material id and does not increment `missing_assets`.

**Appearance.** An unassigned sprite keeps tint, `SpriteBlend`, `lit`, and the sprite texture. An unassigned mesh keeps the inline material and back-face culling. An assigned sprite's colour is the sample times `base_color` times `tint`, including alpha, in linear `Color`, without a CPU clamp. An assigned mesh is the sample times `base_color`. White is the sample when the mesh is untextured. Albedo `Some` selects that texture. Albedo `None` uses `SpriteAsset.texture` for a sprite and leaves a mesh untextured. Sprite UVs, size, pivot, and pixels per unit stay on the sprite asset. An overriding albedo is not remapped onto a different atlas. `AlphaMode` replaces `SpriteBlend` for that sprite. Opaque writes depth, does not blend, and does not discard. Mask writes depth and discards when the combined alpha is below the material cutoff. Blend sprites join the existing blended list: depth test, no depth write, the same `RenderOrder` then depth then order sort. `lit` and `double_sided` come from the material. `double_sided: false` culls back faces for both sprites and meshes. Unassigned sprites stay uncullled. `MaterialAsset::default` on a sprite is therefore opaque and single-sided.

**Drawing.** Material opaque sprites, then material mask sprites, are drawn after meshes and before legacy cutout and tiles. Material blend sprites are in the blended list with legacy blends. New fragment entry points are `fs_opaque` and `fs_mask`. Legacy `fs_cutout` still discards below 0.5, and the legacy mesh fragment still writes alpha 1. The mask cutoff is packed in an existing instance flag. Pipelines differ by class and cull mode. They are not compiled per material. Texture and sampler caches are the existing ones.

**Batching baseline, not the M5 criterion.** Thirty-two opaque sprites that share one material, one texture, and one cull mode are one instanced draw (`compatible_material_sprites_share_a_draw_and_blends_are_not_reordered_to_batch`). Opaque and mask sprites are grouped by texture and pipeline. Blended sprites are not reordered to enlarge a batch. Meshes merge on mesh, texture, and draw class. Material id and revision are carried for M5-5D and are not yet part of that key. This is not the 1,000-sprite acceptance test.

**Not in this checkpoint.** Blended mesh drawing, a unified sprite/mesh transparent list, material-aware batching of 1,000 sprites, normal-map shading, a material graph, custom shaders, sprite sheets, atlas repacking, localized mips, and asset reference counts.

**Repository check.** `scripts/check.sh` on 2026-10-20, macOS 27.0 arm64, `rustc 1.99.0`, exit 0, with M5-5B in the working tree. All 4 steps passed: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (**514 passed, 0 failed, 0 ignored**, 44 suites), `cargo doc --workspace --no-deps` with `-D warnings`. A direct GPU run of `material.rs` printed `GPU test adapter: Apple M5 (Metal, IntegratedGpu); DownlevelFlags::ANISOTROPIC_FILTERING=true`. The check log captures test output, so that line is not in the check transcript. The GPU tests ran because `DIMORA_REQUIRE_GPU=1` was set. This paragraph was written after that check.

**Screenshots.** Release 1280×720 images rendered on 2026-10-20 from this working tree, compared with `cmp -s` to the `6dbabb8` images recorded under M5-4B. All four matched. Baselines were not replaced. `basic_2_5d` 746071 bytes, SHA-256 `c3c5874adeea119c98255c6c1d08208c0244224d930d86e8736c280ef88b38fe`. `basic_2_5d --ortho` 462130 bytes, `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f`. `spatial_scene --camera perspective` 136991 bytes, `ec5f6252a88f642b454ae768f9eec8cdd5fba88a5aeb7a424fcefe5fda12275e`. `spatial_scene --camera orthographic` 34046 bytes, `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde`.

## M5-5C results (2026-10-20)

Committed locally as `ab1b6f7`. Not pushed. M5-5B is commit `3b3cd4a`. M5 stays in progress. The check below ran on this tree before that commit.

**Sequence.** Blended sprites and blended meshes are one list, `RenderScene.transparent`. The backend walks that list after the depth-writing passes. A pipeline, bind group, or vertex-buffer change does not pull every sprite ahead of every mesh, and the list is not sorted again for pipeline locality. The key is `RenderOrder.layer` ascending, camera-space depth descending, `RenderOrder.order` ascending, then visit order. Sprites are visited first, then meshes, so a tie draws the sprite first. Equal sprite keys keep the relative order they had before this checkpoint. A missing `RenderOrder` is `(0, 0)`. Opaque and mask meshes stay in `meshes` and ignore `RenderOrder`.

**Mesh depth.** The representative is the camera-space Z of the world-space bounding-sphere centre, transformed by the same matrix mesh culling uses. Parent transforms, rotation, and nonuniform scale are in that matrix. Sprite depth stays the quad centre. Both values use view Z, and a larger Z is farther. Projection does not choose the class. Turning the camera can reverse which object is farther.

**Drawing.** `MeshDraw::Blend` depth-tests, does not write depth, and uses the same straight-alpha state as blended sprites. `mesh.wgsl` `fs_blend` returns the sampled alpha. Lighting changes rgb only. `fs_main` still forces alpha 1, and unassigned meshes stay on that path. `double_sided` selects back-face culling or none. Two blend pipelines cover that pair. They are not compiled per material. A class change on the next extraction selects the pipeline. It does not allocate a texture image. Texture and sampler caches stay the existing ones. A removed albedo is evicted by the existing rule and the draw uses the white mesh placeholder.

**Batching at this checkpoint.** A transparent run grew only while the next item was the same kind and the same draw key, so the submission order stayed the sort order. Four compatible blended meshes were one draw. One sprite between two meshes was three draws. An alternating sprite and mesh sequence of four items was four draws. One blended mesh was one draw. The existing 32 compatible opaque material sprites remained one draw. Material id and revision were stored and were not yet part of the key. M5-5D adds them. It must not reorder this sequence to enlarge a blend batch.

**Limits.** The centre is an object-level approximation. It does not order intersecting transparent meshes, cyclic overlaps, an overlap the centre misses, a mesh that spans another object's depth, or a transparent mesh passing through a sprite plane. Those cases are demonstrated as sort results where a test is useful, and they are not claimed as solved pictures. There is no order-independent transparency, per-triangle sort, depth peeling, or weighted blended transparency. The three D-48 coplanar conflicts are unchanged.

**Not in this checkpoint.** The 1,000-sprite batch, mesh generators, tilemap materials, sprite sheets, normal-map shading, PBR, material graphs, custom shaders, asset reference counts, and the M6 coplanar fixes.

**Repository check.** `scripts/check.sh` on 2026-10-20, macOS 27.0 arm64, `rustc 1.99.0`, exit 0, with M5-5C in the working tree. All 4 steps passed: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (**529 passed, 0 failed, 0 ignored**, 46 suites), `cargo doc --workspace --no-deps` with `-D warnings`. A direct GPU run of `blend.rs` printed `GPU test adapter: Apple M5 (Metal, IntegratedGpu)`. The check log captures test output, so that line is not in the check transcript. The GPU tests ran because `DIMORA_REQUIRE_GPU=1` was set. This paragraph was written after that check.

**Screenshots.** Release 1280×720 images rendered on 2026-10-20 from this working tree, compared with `cmp -s` to the `6dbabb8` images recorded under M5-4B. All four matched. Baselines were not replaced. `basic_2_5d` 746071 bytes, SHA-256 `c3c5874adeea119c98255c6c1d08208c0244224d930d86e8736c280ef88b38fe`. `basic_2_5d --ortho` 462130 bytes, `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f`. `spatial_scene --camera perspective` 136991 bytes, `ec5f6252a88f642b454ae768f9eec8cdd5fba88a5aeb7a424fcefe5fda12275e`. `spatial_scene --camera orthographic` 34046 bytes, `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde`. The commit-time re-run of that comparison matched the same four hashes.

## M5-5D results (2026-10-20)

Committed locally as `a7efd6c`. Not pushed. M5-5C is commit `ab1b6f7`. M5 stays in progress.

**Batch key.** A sprite run shares one instanced draw when the pipeline, material id, material revision, and texture handle match. The pipeline is the alpha class plus `double_sided`. Tint, UV, the model, and `lit` are instance data. The mask cutoff copied onto the instance is the one `AlphaCutoff` on that `MaterialAsset`. `Sprite` and `Mesh` have no cutoff field, so two renderables that share one material discard at the same threshold. A different cutoff is a different material. That copy is not a pipeline and not a batch key. The texture handle is the view and the sampler, because a texture asset has one sampler descriptor. An unassigned sprite has `material_id: None` and revision 0. That is not a fabricated material, so legacy sprites still batch only when the texture and pipeline match. A mesh run adds the mesh asset and `MeshDraw`. A sprite and a mesh never share a draw. Opaque and mask sprites are grouped by this key. Blended items are not sorted by it. A blended run grows only when the next item in the existing depth order is the same kind and `shares_batch` accepts it.

**Acceptance measurement.** One thousand opaque sprites, one material revision, one atlas texture, one sampler, the shared quad, and one pipeline: CPU batches 1, GPU draw calls 1, `submitted_instances` 1000, `largest_draw_instances` 1000. Those counters are incremented at `draw` / `draw_indexed`, not from the batch-object count alone. The frame also read back three pixels: the left atlas texel, the right atlas texel, and a darker tint on the left texel. The first frame created 1 texture image, 1 base upload, 0 new samplers, 1 bind group, and grew the opaque instance buffer once (the 4096-byte buffer created with the renderer is not counted). The second frame added none of those. No GPU timings were measured.

**Splits that stayed split.** Different material ids, a different revision in `SpriteLayer::from_ordered`, alpha class, `double_sided`, texture, sampler descriptor, and mesh geometry. A transparent sprite run interrupted by a mesh, and a transparent mesh run interrupted by another material, stayed in sort order. Two blended sprites in one draw matched the same pair submitted as two ordered draws on this GPU, so contiguous blend runs are merged. Per-instance tint did not split a batch.

**Revisions.** Replacing a material through `AssetManager` updates the next extraction. Colour, albedo, cutoff, class, lit, and `double_sided` were each visible on the following frame. Compatible sprites stayed one batch. A colour or class change did not allocate a texture image. Pointing the material at an already cached albedo allocated nothing. Sampler objects stay in the existing cache.

**Not in this checkpoint.** Mesh generators, tilemap materials, sprite sheets, normal-map shading, PBR, material graphs, custom shaders, order-independent transparency, per-triangle sorting, asset reference counts, and the M6 coplanar fixes. The three D-48 conflicts are unchanged.

**Repository check.** `scripts/check.sh` on 2026-10-21, macOS 27.0 arm64, `rustc 1.99.0`, exit 0, with M5-5D in the working tree, repeated after the cutoff clarification. All 4 steps passed: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (**551 passed, 0 failed, 0 ignored**, 48 suites), `cargo doc --workspace --no-deps` with `-D warnings`. `engine-renderer` `batch.rs` ran 13 tests. `engine-renderer-wgpu` `batch.rs` ran 9, including `one_thousand_compatible_opaque_sprites_are_one_draw`. A direct run of that test printed `GPU test adapter: Apple M5 (Metal, IntegratedGpu)` and `cpu batches=1 gpu draws=1 instances=1000`. The check log captures test output, so the adapter line is not in the check transcript. The GPU tests ran because `DIMORA_REQUIRE_GPU=1` was set. This paragraph was written after that check.

**Screenshots.** Release `1280x720` PNGs on 2026-10-21 compared with `cmp -s` against images rendered from commit `6dbabb8` (`/tmp/dimora-m55c-baseline`). All four matched byte-for-byte: `basic_2_5d`, `basic_2_5d --ortho`, `spatial_scene --camera perspective`, `spatial_scene --camera orthographic`. SHA-256: basic `c3c5874adeea119c98255c6c1d08208c0244224d930d86e8736c280ef88b38fe`, basic-ortho `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f`, perspective `ec5f6252a88f642b454ae768f9eec8cdd5fba88a5aeb7a424fcefe5fda12275e`, orthographic `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde`. Historical baselines were not replaced.

## M5-6 results (2026-10-21)

Committed locally as `92286ce0546a4d94bb0365581ae7469fe2ad3260`. Not pushed. The paragraphs below were written against the working tree and the acceptance check repeated them after the commit. M5-5D is commit `a7efd6c`. This checkpoint did not accept the milestone. Formal acceptance is the 2026-10-22 section. M6 has not started. D-48 is unchanged.

**API.** `MeshData::plane(width, depth)`, `cube(size)`, `cuboid(size: Vec3)`, `sphere(radius, segments, rings)`, `cylinder(radius, height, segments)`, and `cylinder_with_caps(..., caps)`. `cube` calls `cuboid(Vec3::splat(size))`. `cylinder` includes caps. Y is up, X is width, Z is depth. The plane is the XZ ground with normal +Y. Boxes, the sphere, and the cylinder are centred. `cone` is unchanged and still asserts. Invalid sizes and subdivision counts return `MeshError`. A face product below `1e-12`, or more than 65,536 vertices, is rejected before allocation. ADR-0018.

**Counts.** Plane: 4 vertices, 6 indices. Cube and cuboid: 24 and 36. Sphere: `2 * segments + (rings - 1) * (segments + 1)` vertices and `6 * segments * (rings - 1)` indices. A 4-by-2 sphere is 13 and 24. A capped 4-segment cylinder is 20 and 48. An open one is 10 and 24. Each box face has its own outward normal and a `0..1` UV square. Sphere normals are radial. Cylinder caps do not share side normals. The longitude seam copies the first vertex and sets `u` to 1.

**Rendering.** The meshes register through `AssetManager` and draw with the existing mesh pipeline. GPU readback on Apple M5 / Metal showed the plane from above, the cube from four sides, six cuboid faces sampling a two-texel stripe, a thin wall, a slab, a rotated beam, a scaled cube, a culled box, a sphere, and a cylinder side plus both caps. An open cylinder's top view is empty at the centre. A lit +X face is bright and a lit +Z face is dark under a light travelling toward -X. Mask cutoff 0.4 keeps alpha 0.5 and 0.75 discards it. A blended cuboid in front of a blended sprite composites to about `(128, 0, 128)`. Two cuboids sharing one asset and material are one draw and two submitted instances. Adding a sphere makes two draws and three instances. Replacing the material colour updates both instances and does not allocate a texture. Perspective, orthographic, and a rotated orthographic view each show the same unlit cube, sphere, and cylinder. A large blended box still uses the bounding-sphere-centre sort.

**Not in this checkpoint.** Localized mip regeneration, a mesh loader, tangents, colliders, per-face materials, normal-map shading, and M6. This checkpoint does not accept the milestone. Formal acceptance is the 2026-10-22 section.

**Repository check.** `scripts/check.sh` on 2026-10-21, macOS 27.0 arm64, `rustc 1.99.0`, exit 0, with M5-6 in the working tree. All 4 steps passed: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (**573 passed, 0 failed, 0 ignored**, 50 suites), `cargo doc --workspace --no-deps` with `-D warnings`. `engine-assets` mesh tests ran 11. `engine-renderer` `primitives.rs` ran 5. `engine-renderer-wgpu` `primitives.rs` ran 8. A direct rerun of `one_thousand_compatible_opaque_sprites_are_one_draw` printed `GPU test adapter: Apple M5 (Metal, IntegratedGpu)` and `cpu batches=1 gpu draws=1 instances=1000`. The check log captures test output, so that adapter line is not in the check transcript. The GPU tests ran because `DIMORA_REQUIRE_GPU=1` was set. This paragraph was written after that check.

**Screenshots.** Release `1280x720` PNGs on 2026-10-21 compared with `cmp -s` against images rendered from commit `6dbabb8` (`/tmp/dimora-m55c-baseline`). All four matched byte-for-byte: `basic_2_5d`, `basic_2_5d --ortho`, `spatial_scene --camera perspective`, `spatial_scene --camera orthographic`. SHA-256: basic `c3c5874adeea119c98255c6c1d08208c0244224d930d86e8736c280ef88b38fe`, basic-ortho `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f`, perspective `ec5f6252a88f642b454ae768f9eec8cdd5fba88a5aeb7a424fcefe5fda12275e`, orthographic `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde`. Historical baselines were not replaced.

## M5 acceptance audit (2026-10-21)

Review only, as of 2026-10-21. This section does not itself close the milestone. The 2026-10-22 decision approves the recommendation. See "M5 formal acceptance". Audited commit `92286ce0546a4d94bb0365581ae7469fe2ad3260` (`feat(assets): add procedural mesh generators`), which is local and not pushed. M5-5D is `a7efd6c45451b7aa769e98b481429324c82074e2`.

**Check.** `scripts/check.sh` on 2026-10-21, macOS 27.0 arm64, `rustc 1.99.0 (b940084d7 2026-09-28)`, exit 0, on the tree that became `92286ce`. All 4 steps passed: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (**573 passed, 0 failed, 0 ignored**, 50 suites, including doc-tests), `cargo doc --workspace --no-deps` with `-D warnings`. A direct rerun of `one_thousand_compatible_opaque_sprites_are_one_draw` printed `GPU test adapter: Apple M5 (Metal, IntegratedGpu)` and `1000-sprite batch: cpu batches=1 gpu draws=1 instances=1000 texture_creates=1 uploads=1 samplers=0 bind_groups=1 buffer_grows=1`. The check log captures test output, so the adapter line is not in the check transcript.

**Screenshots.** Release 1280×720 PNGs rendered from that same tree into `/tmp/dimora-m5-accept` and compared with `cmp -s` to `/tmp/dimora-m55c-baseline` (renders of `6dbabb8`). All four matched. Baselines were not replaced. SHA-256 of the new files: `basic_2_5d` `c3c5874adeea119c98255c6c1d08208c0244224d930d86e8736c280ef88b38fe`; `basic_2_5d --ortho` `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f`; `spatial_scene --camera perspective` `ec5f6252a88f642b454ae768f9eec8cdd5fba88a5aeb7a424fcefe5fda12275e`; `spatial_scene --camera orthographic` `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde`.

**How to read the matrix.** A is an explicit M5 task or acceptance bullet in ROADMAP.md "M5. Materials and textures". B is an accepted ADR. C is an implementation choice. D is a deferred optimization named by an accepted document. E is a later-milestone limit. A passing test covers the behavior it asserts. `DEFERRED` names where the work goes. `UNVERIFIED` is not a pass. This audit does not change the acceptance bullets.

**Localized mip ruling.** Localized regeneration of only the affected mip texels is not a binding M5 acceptance requirement. It is a later optimization with no assigned milestone. Formal acceptance on 2026-10-22 approves this reading. The classification is DEFERRED, a performance optimization, milestone unassigned.

The ROADMAP acceptance bullet requires a sub-rectangle update that changes only those base-level pixels, uploads only that region's base-level bytes, performs mip maintenance, and does not count that maintenance as a full-image upload. ADR-0017, which the same section cites for partial updates, settles the M5-3B policy as a full-chain rebuild and says the renderer does not regenerate only the affected mip texels. `affected_mip_rect` is unused and is the note for a later localized rebuild. The Dependencies paragraph of the same ROADMAP section says the same thing. CAPABILITIES section 12 still lists localized mip regeneration under Required as the remainder of ADR-0017, and KI-25 leaves it open next to the M10 glyph atlas. Those texts disagree if "mip levels that rectangle affects" is read as a dirty mip-texel upload. This audit does not delete that phrase. It rules that the binding clauses are the partial base upload, unchanged texels outside the rectangle, required mip maintenance, and separate upload accounting. Whole-chain regeneration meets those clauses. Texel-local regeneration stays deferred and unassigned. Formal acceptance should record that ruling beside the bullet so the two readings are not left open.

| Requirement | Source | Implementation | Evidence | Result | Remaining limitation | Blocks |
|---|---|---|---|---|---|---|
| Minified checker approaches its mean with mips and aliases without | ROADMAP M5 acceptance | `engine-renderer-wgpu` mip blit | `generated_mips_average_a_minified_checker_and_alias_without_them` | PASS | None in this bullet | No |
| The same mip and filter behavior under orthographic zoom | ROADMAP M5 acceptance | Existing camera; no projection branch | `perspective_and_orthographic_zoom_both_select_mip_levels` | PASS | None in this bullet | No |
| A linear texture reaches the shader without an sRGB decode | ROADMAP M5 acceptance; ADR-0017 | `Rgba8Unorm` for `ColorSpace::Linear`, `Rgba8UnormSrgb` for sRGB | `srgb_and_linear_textures_select_different_gpu_formats`; `srgb_and_linear_mip_samples_are_about_half_linear_intensity` | PASS | M14 is the first shaded consumer of linear data | No |
| CPU atlas: no overlap, padding, extrusion, deterministic UVs, zero-sized region named as an error | ROADMAP M5 acceptance and task list | `AtlasBuilder` in `engine-assets` | `atlas.rs` unit tests, including `AtlasError::ZeroWidth { name }` / `ZeroHeight` | PASS | The packer returns the error and does not log (ADR-0017). Packing does not set world size | No |
| GPU atlas isolation inside the tested mip range; coarse mixing allowed | ROADMAP M5 acceptance; ADR-0017 measured layouts | One image, existing sampler and mip blit | `atlas.rs`: nearest, `linear_magnification_reaches_about_half_a_texel_past_the_region`, `generated_mips_isolate_regions_only_for_specific_layouts`, sRGB/linear, transparent edges, three cameras | PASS | Whole-atlas mips can mix regions outside a measured layout. `floor(log2(padding))` is not the range | No |
| Sub-rectangle base upload, mip maintenance, not counted as a full-image upload | ROADMAP M5 acceptance; ADR-0017 M5-3B | `Texture::update_region`, `Queue::write_texture`, whole-chain blit | `a_region_update_uploads_only_that_rectangle`; `successive_frames_upload_each_new_region_and_keep_the_earlier_ones`; `skipped_revisions_upload_the_retained_suffix_and_old_history_falls_back_to_full`; `a_partial_base_upload_regenerates_the_mip_chain_from_the_new_base` | PASS | See the localized-mip row. `base_upload_bytes` is logical submitted bytes, not measured bandwidth | No |
| Regenerating only the mip texels a rectangle affects | CAPABILITIES §12; KI-25. Not the ADR-0017 M5-3B policy | `affected_mip_rect` exists and is not called | ADR-0017 states the helper is unused | DEFERRED | Performance optimization, milestone unassigned. Not M6 and not the M10 glyph atlas by itself | No |
| 1,000 compatible sprites are one instanced draw | ROADMAP M5 acceptance | M5-5D batch key in `scene.rs` / `scene_renderer.rs` | `one_thousand_compatible_opaque_sprites_are_one_draw`: 1,000 extracted, 1 CPU batch, 1 GPU draw, 1,000 submitted instances, three read-back pixels | PASS | Blended objects are not reordered to form a batch | No |
| One alpha policy under perspective and rotated orthographic; D-48 unchanged | ROADMAP M5 acceptance; ADR-0015 | Class comes from the material. Blend depth is camera-space Z of the world-space bounding-sphere centre | `material_class_is_stable_across_perspective_orthographic_and_isometric_views`; `projection_does_not_change_blend_class_and_a_turn_reverses_pixels`; `blended_mesh_sprite_mesh_order_is_the_submitted_order`; the three `configurations.rs` conflict tests | PASS | Object-level sort does not order intersections (ADR-0015). The three D-48 conflicts stay M6 | No |
| Four release screenshots stay byte-identical to `6dbabb8` | ROADMAP M5 acceptance | No baseline replacement | `cmp -s` and the four SHA-256 values above | PASS | None | No |
| Texture descriptors: sRGB, linear, clamp/repeat/mirror, independent min and mag, mips, anisotropy normalization, RGBA8, PNG | ROADMAP M5 tasks; ADR-0017 | `texture.rs` `SamplerDesc`, `ColorSpace`, `Mipmaps` | CPU tests in `texture.rs`; GPU `filtering_and_addressing_follow_the_sampler_descriptor`, `sampler_cache_reuses_descriptors_without_copying_pixels`, `anisotropy_follows_device_support_and_keeps_nearest` | PASS | `from_rgba8` still asserts on a wrong byte length. PNG decode and `update_region` return errors. Defaults stay sRGB, nearest, clamp, no mips, anisotropy 1 | No |
| GPU formats, split sampler cache, sampler-only changes do not reupload pixels | ADR-0017; M5-2B | `scene_renderer.rs` | `srgb_and_linear_textures_select_different_gpu_formats`; `sampler_cache_reuses_descriptors_without_copying_pixels`; `asset_revision_reuploads_storage_only_when_it_changes` | PASS | Apple M5 had anisotropic filtering. An adapter without that flag was not the hardware path | No |
| Mip chain allocation, non-square, odd, 1×1, sRGB and linear averages, regenerate on pixel change, not on sampler change | ADR-0017; M5-2C | Private blit. sRGB decodes, averages in linear light, and encodes | `mip::tests`, including `generated_srgb_and_linear_levels_average_black_and_white_in_linear_light`, `none_is_one_level_and_generate_follows_the_larger_side`, `an_odd_width_can_leave_the_middle_texel_out_of_every_footprint`; `mip_generation_follows_pixels_allocation_and_eviction` | PASS | Odd reductions can leave a middle texel out of every footprint. That is documented, not a second average | No |
| NPOT mip chain when the device lacks the downlevel flag | ADR-0017 fallback | One-level fallback, logged once | CPU `npot_without_the_downlevel_flag_keeps_one_level`. GPU `npot_mip_allocation_follows_the_device_flag` ran on Apple M5, where ADR-0017 recorded anisotropic filtering and non-power-of-two mips present | UNVERIFIED | UNVERIFIED HARDWARE COVERAGE. The missing-flag device path was not executed. The CPU decision was. This is not a multi-adapter claim | No |
| Partial CPU updates validate before writing; the journal stays bounded; direct `rgba` writes cannot prove a partial upload | ADR-0017; M5-3A | `Texture::update_region`, eight transitions | `texture.rs` region tests | PASS | A journal older than eight transitions, or a direct `rgba` write, uploads the whole base level | No |
| Failed uploads do not mark rejected pixels uploaded; dimension limits run before allocation | ADR-0017; ROADMAP asset-validation task | `validate_texture_dimensions` | `cache_policy_selects_full_partial_or_no_upload_and_rejects_oversized_textures`; `an_oversized_atlas_is_refused_and_a_missing_texture_stays_magenta` | PASS | The log uses `name_of` when an entry exists | No |
| Missing texture and zero-sized region carry a name | ROADMAP M5 task; ADR-0017 | Extraction warnings; `AtlasError` | `a_failed_texture_load_is_named_and_a_pending_one_is_silent` includes `tiles.png`. A never-registered handle logs the id and the parent name and does not invent a texture name. Zero size is `ZeroWidth` / `ZeroHeight` with the image name | PASS | An unregistered handle cannot recover a name that was never stored. The packer does not log | No |
| `Atlas::replace_source` refreshes the same-size source and extruded gutter through one partial upload | ADR-0017; M5-4B | `atlas.rs` `replace_source` | `replace_source_clears_a_stale_gutter_with_one_partial_upload`; CPU `replace_source_rejects_unknown_names_and_the_wrong_bytes` | PASS | A size change needs a new build. No incremental repack | No |
| `MaterialAsset`: Opaque, Mask, Blend, validated cutoff, linear colour, shared textures, reserved normal, no GPU objects | ROADMAP M5 task; ADR-0015 | `engine-assets` `material.rs` | M5-5A tests; one cutoff per asset, copied into instance data and not a batch key | PASS | The normal handle is stored and not shaded (M14) | No |
| Material appearance: legacy path, texture precedence, tint multiply, depth and blend, lit and culling, missing material skipped, missing albedo placeholder, next-frame replacement | ADR-0015; M5-5B | `extract.rs`, mesh and sprite shaders | `material.rs` GPU tests, including `material_color_and_sprite_tint_multiply_in_linear_space`, `masked_sprites_use_the_material_cutoff_and_write_depth`, `a_missing_material_is_not_drawn_and_a_missing_albedo_uses_the_placeholder`, `material_replacement_updates_class_colour_cutoff_light_and_culling` | PASS | Instance cutoff storage is not a public per-sprite override | No |
| Blended sprites and meshes share one sequence; mesh depth is the bounding-sphere centre; explicit order is the tie-breaker | ADR-0015; ROADMAP blended-mesh task | `extract.rs` transparent list | `blended_mesh_sprite_mesh_order_is_the_submitted_order`; `blended_meshes_depth_test_without_writing_depth`; `render_order_and_stable_ties_are_visible_on_the_gpu` | PASS | Large intersecting transparent meshes stay an object-level approximation | No |
| Material-aware batching, contiguous transparent runs only, resource caches reused | ROADMAP M5 acceptance; M5-5D | Batch key: pipeline, material id and revision, texture view and sampler; meshes also need the mesh asset and draw | `batch.rs` GPU tests, including the 1,000-sprite test and `mixed_sprite_and_mesh_runs_merge_only_at_kind_boundaries` | PASS | A sprite and a mesh never share a draw | No |
| Plane, cube, cuboid, sphere, cylinder, optional caps | ROADMAP M5 task; ADR-0018 | `MeshData` in `engine-assets/src/mesh.rs`. `cube` calls `cuboid` | CPU `mesh::tests` (11, including the pre-existing cone test); GPU `primitives.rs` (8), including lighting, materials, three cameras, and `two_cuboids_share_one_draw_and_a_sphere_does_not` | PASS | Cone is outside this list. See below | No |
| Generator limits: 65,536 vertices, subdivision minimums, checked counts, face product `1e-12` | ADR-0018 | `MeshError` before allocation | `plane_rejects_bad_dimensions`, `cuboid_rejects_each_bad_dimension`, `sphere_rejects_bad_arguments`, `cylinder_rejects_bad_arguments` | PASS | Ordinary finite sizes above the product floor are accepted. `1e-20` and non-finite products are not. `MeshData::new` still asserts index integrity after that validation. `cone` still asserts | No |
| `cone` uses the same `Result` validation | Not in the ROADMAP generator list | Pre-M5 helper, base on `y = 0` | `cone_is_well_formed` still passes. Screenshots still use it | DEFERRED | Technical debt. Not rewritten in this audit. A later mesh pass can convert it without an M5 obligation | No |
| Shared textures, materials, and meshes; scene removal does not destroy global assets; no new reference count | ADR-0002, ADR-0014, D-16 open | Existing `AssetManager` | `a_material_can_reference_an_atlas_without_repacking_it`; `unloading_a_scene_and_removing_its_assets_returns_the_gpu_cache_to_baseline`; `assets_unused_for_the_idle_limit_are_evicted_and_come_back_when_needed` | PASS | D-16 stays open. Handles still store a name hash | No |
| GPU images, samplers, bind groups, mesh cache, and instance buffers are reused in the tested workflows | ADR-0002 | Existing caches. Counters in `ResourceStats` | 1,000-sprite first frame: 1 texture create, 1 base upload, 0 new samplers, 1 bind group, 1 opaque buffer grow. Second frame added none. Atlas and material tests assert the same caches | PASS | No timings and no byte-accurate GPU memory totals were measured | No |
| Legacy sprites, meshes, `SpriteBlend`, inline mesh `Material`, tilemaps, camera placement, `RenderOrder`, hierarchy, scenes | ADR-0015 unassigned path | Unassigned renderables keep the previous classification | `legacy_sprites_and_meshes_keep_their_batches`; `a_blended_mesh_draws_and_an_unassigned_mesh_stays_opaque`; screenshot match; `configurations.rs` 16 tests | PASS | `MeshData::cuboid` now returns `Result`. Examples and tests were updated. That is an intentional break. Cone was not given the same signature | No |
| Perspective, orthographic, and isometric configuration do not add a projection-specific material class | ADR-0004; ADR-0015; ROADMAP M5 acceptance | One extract path | Material, blend, atlas, and primitive camera tests; D-48 tests still pass | PASS | Coplanar ordering stays M6 | No |

**Other accepted ADRs that govern M5.** ADR-0002 (GPU cache lifetime) and ADR-0004 (one world, many cameras) still hold. ADR-0016 is the per-sprite pixels-per-unit decision and was not changed by the texture or mesh work; atlas packing stays in texels. ADR-0014 keeps scene membership from owning assets. No new decision was recorded here.

**Public API.** The break verified in this audit is `MeshData::cuboid(Vec3) -> MeshData` becoming `Result<MeshData, MeshError>`. `plane`, `cube`, `sphere`, `cylinder`, and `cylinder_with_caps` are new and return `Result`. `cone` is unchanged. `MaterialAsset`, atlas types, and sampler descriptors are additions. This audit does not claim that every M5 signature is source-compatible with pre-M5 callers.

**Blocking defects.** None found in the binding M5 tasks or acceptance bullets under the localized-mip ruling above.

**Accepted limitations.** Atlas mip bleed outside the measured layouts. Object-level transparent sorting. The three D-48 conflicts, assigned to M6. Normal-map shading assigned to M14. A journal older than eight transitions uploads the whole base level. `from_rgba8` asserts on a short or long buffer. An unregistered texture handle cannot invent a name. `base_upload_bytes` is not measured GPU bandwidth.

**Deferred, and where it goes.** Localized mip texels: unassigned remainder of ADR-0017 / KI-25. Cone validation: technical debt, no milestone. Downlevel NPOT and anisotropy fallbacks on a device that lacks the flag: untested hardware, CPU decision covered. Pixel snapping and coplanar ordering: M6. Normal maps: M14. Glyph atlas consumer of partial uploads: M10. Mesh loader, tangents, skin: M12. PBR, material graphs, custom shaders, compression, and streaming: ROADMAP M5 deferred list, no earlier milestone.

**Recommendation.** READY FOR ACCEPTANCE, as of this 2026-10-21 review. The binding M5 tasks and acceptance bullets are implemented and were re-checked on Apple M5 / Metal. The 2026-10-22 decision approves that recommendation. The formal record is the next section. It does not start M6, push, or tag.

## M5 formal acceptance (2026-10-22)

Approved. M5 is complete locally and accepted. Not pushed. This section is documentation. It does not change renderer code. The implementation checkpoint remains `92286ce0546a4d94bb0365581ae7469fe2ad3260`. The commit that adds this section is separate from that checkpoint.

**Verification carried into acceptance.** The 2026-10-21 audit check is the screenshot evidence: four release renders matched `6dbabb8`, and those hashes are in the audit section. Rendering code did not change for this documentation commit, so those comparisons were not repeated. The closing `scripts/check.sh` on 2026-10-22, macOS 27.0 arm64, `rustc 1.99.0`, exit 0, repeated fmt, clippy `-D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (573 passed, 0 failed, 0 ignored, 50 suites), and rustdoc `-D warnings`. A direct rerun of `one_thousand_compatible_opaque_sprites_are_one_draw` printed `GPU test adapter: Apple M5 (Metal, IntegratedGpu)` and `cpu batches=1 gpu draws=1 instances=1000`.

**Approved mip reading.** Sub-rectangle base uploads are implemented. Untouched base-level GPU pixels stay. When base pixels change, the GPU regenerates the entire existing mip chain. Sampler-only changes do not rebuild mipmaps. The rebuilt chain must be correct. Localized mip-texel regeneration is not required. It is DEFERRED, a performance optimization, milestone unassigned. KI-25 stays open. `affected_mip_rect` stays unused. ADR-0017's note for a later localized rebuild stays.

**Limitations that acceptance does not close.** Coarse atlas mips can mix regions outside the measured layouts. Partial uploads rebuild the whole chain. The edit journal keeps eight transitions, and older history uploads the whole base level. Blended meshes sort as objects by the bounding-sphere centre. The three D-48 conflicts stay on M6. Normal-map shading stays on M14. The glyph-atlas consumer stays on M10. A texture handle that was never registered cannot recover a name. `MeshData::cone` still asserts and keeps its base on `y = 0`. Generators reject more than 65,536 vertices and a face product below `1e-12`. Scene membership still does not own shared assets, and there is no new reference count. `Texture::from_rgba8` asserts on a wrong byte length (KI-30). `update_region` is the path that returns a structured error and leaves pixels and history unchanged on failure. The missing downlevel-mip hardware path is UNVERIFIED HARDWARE COVERAGE. Apple M5 has the anisotropic-filtering and non-power-of-two mip capabilities recorded in ADR-0017. CPU injection tests for the one-level fallback remain.

**Not implemented.** Localized mip-texel regeneration. Normal-map shading. Order-independent transparency. Sprite-sheet animation. The glyph atlas. M6 coplanar stabilization.

**Public API.** `MeshData::cuboid` returns `Result`. The new generators return `Result`. `cone` does not. `from_rgba8` still returns `Self` and asserts. Acceptance does not claim source compatibility with every pre-M5 call site.

## M6-1B results (2026-10-22)

Custom view-to-clip matrices, D-25. Commit `a38cb30d476755fb0da96b8ac12b92ab5ff45235`. Not pushed. M6 is not complete. D-48 and D-26 are unchanged.

`CameraProjection::Custom(Mat4)` is constructed with `CameraProjection::custom`. NaN and infinity return `ProjectionError::NonFinite`. The stored matrix is not rebuilt from aspect and is not edited by `with_clip_range`. Perspective and orthographic still track the viewport aspect. `view_proj` is still `proj * view`. The GPU uniform is unchanged. `is_perspective` is true only for the `Perspective` variant. The `spatial_scene` projection key still swaps the two built-in descriptors; the window title names `Custom` separately. Singular matrices extract and render; `ray_from_pixels` returns `None` when `Mat4::inverse` does.

Frustum planes use the same six clip-cube formulas. A plane is dropped when its normal length is at most `1e-6` of that plane's own largest coefficient, or when a coefficient is non-finite. A dropped plane does not cull. The ratio is unchanged when the whole matrix is scaled, including by a negative factor: a negative homogeneous scale flips clip `w`, and the planes are negated when the NDC-cube centre reprojects with negative `w`. No replacement plane is created. That agreement is CPU culling. Divided NDC can match while GPU homogeneous clipping does not, because the hardware clips before the divide. A negative-scaled matrix is not claimed to produce the same GPU image as its positive equivalent.

**CPU.** `engine-renderer` `tests/camera.rs` and `frustum::tests`, plus `existing_camera_presets_remain_compatible` in `spatial_scene`. The original six example constructors stay perspective or orthographic.

**GPU.** `engine-renderer-wgpu` `tests/custom_projection.rs`, Apple M5, Metal, integrated GPU, `DIMORA_REQUIRE_GPU=1`. Perspective-equivalent and orthographic-equivalent images match the built-in cameras. An oblique matrix hits the pixel `CameraView::project` reports and misses the plain orthographic pixel. Opaque occlusion, a kept mask, a discarded mask, blend-without-depth-write, and the mesh/sprite/mesh blend composite match the accepted colours. Translation, yaw, and a parented camera behave as the view matrix. A resize keeps the custom matrix and still moves pixels; perspective still changes with aspect. 1,000 compatible sprites are one instanced draw. An off-centre box and an oblique frustum each keep one sprite and cull one. A dropped far plane still draws a sprite past the old far distance; the built-in far plane culls that sprite.

**Screenshots.** Release, 1280×720, SHA-256 matches commit `6dbabb8`: `basic_2_5d` `c3c5874adeea119c98255c6c1d08208c0244224d930d86e8736c280ef88b38fe`, `basic_2_5d --ortho` `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f`, `spatial_scene --camera perspective` `ec5f6252a88f642b454ae768f9eec8cdd5fba88a5aeb7a424fcefe5fda12275e`, `spatial_scene --camera orthographic` `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde`.

**Repository check.** `scripts/check.sh` on 2026-10-22, macOS arm64, Apple M5, Metal, exit 0. All 4 steps: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (611 passed, 0 failed, 0 ignored, 51 suites), `cargo doc --workspace --no-deps` with `-D warnings`.

## M6-1C results (2026-10-22)

Seventh example camera in `spatial_scene`. Committed locally. Not pushed. M6 is not complete. D-08, D-26, and D-48 are not started and are not changed.

`custom` is appended to `NAMED` after the original six. It returns `(Camera, Transform)`. The transform is the home pose shared with perspective and orthographic. The projection is `CameraProjection::Custom` filled by a local helper: view-space shear `(0.14, 0.04)` then `Mat4::orthographic_lh` at height 400, near 1, far 4000, aspect 16/9. The pose is not in the matrix. `--camera custom` selects it. An unknown name is still an error. `C` cycles in `NAMED` order and wraps to perspective. `P` still swaps the two built-in projections and leaves `Custom` unchanged. The window title uses the configuration name and the `Custom` label, not `is_perspective`.

There is no engine preset and no public oblique builder. The matrix stays fixed when the viewport aspect changes. A negative scale of a custom matrix can match divided NDC and the corrected CPU frustum without matching GPU homogeneous clipping. That limit is unchanged from M6-1B.

**CPU.** `the_custom_configuration_shears_a_known_point_and_keeps_pose_on_the_transform`: at 320×180, figure C projects to about (221.7, 52.8) under `custom` and (175.3, 66.0) under orthographic, a shift of about 48.3 px. The ray through the custom pixel misses the point by less than 0.5. Figure A is nearer in view depth and in NDC depth than figure D, both in `[0, 1]`. Moving the transform changes the view matrix and not the projection. A 400×400 viewport keeps the same matrix and the same NDC. The custom frustum contains figure A, figure C, the pillar, and the cube, and rejects a sphere at x = 8000. `cycling_appends_custom_and_wraps_to_perspective` and `cli_names_keep_their_indices_and_an_unknown_name_is_rejected` cover `C` and `--camera`.

**GPU.** `the_custom_camera_renders_sheared_scene_content`, Apple M5, Metal. The pillar centre is pixel (199, 46), colour `[241, 191, 158, 255]`. The orthographic projection of that point is (160, 57) and is not the pillar colour. Figure A’s cutout body is visible around its projected point. An isolated blended ghost changes the sky clear colour and stays blue. The existing configuration GPU tests, including both occlusion directions, orientation, decal depth, and the three D-48 conflicts, ran with `custom` included where those loops already walk `NAMED`. Custom does not remove the conflicts.

**Screenshots.** Release, 1280×720, SHA-256 matches commit `6dbabb8`: `basic_2_5d` `c3c5874adeea119c98255c6c1d08208c0244224d930d86e8736c280ef88b38fe`, `basic_2_5d --ortho` `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f`, `spatial_scene --camera perspective` `ec5f6252a88f642b454ae768f9eec8cdd5fba88a5aeb7a424fcefe5fda12275e`, `spatial_scene --camera orthographic` `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde`.

**Repository check.** `scripts/check.sh` on 2026-10-22, macOS arm64, Apple M5, Metal, exit 0. All 4 steps: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (615 passed, 0 failed, 0 ignored, 51 suites), `cargo doc --workspace --no-deps` with `-D warnings`. `one_thousand_compatible_opaque_sprites_are_one_draw` printed `cpu batches=1 gpu draws=1 instances=1000`.

## M6-2B results (2026-10-22)

Surface cameras for D-08. Commit `6a4788290b8bd69c56255f12b7a557a78e5583c6`. Not pushed. M6 is not complete. D-48, D-26, D-07, and D-06 are unchanged. Offscreen targets are the M6-2C results below.

Every `Camera.active` camera is a `CameraPass` in `RenderScene.passes`, ordered by ascending `(Camera.order, entity id)`. There is no `RenderScene.view`. `sole_pass` reads the only pass and panics otherwise. `CameraClear` is `Color` (default) or `Load`. Constructors and struct-update syntax are unchanged. The first pass clears the whole color attachment to that camera's clear color, including when the policy is `Load`. A later `Color` camera draws a viewport-sized triangle from `viewport_clear.wgsl` with blending off and depth writes off. A later `Load` camera does not. Every pass clears `Depth32Float` to 1.0 and discards it. Opaque and Mask write depth. Blend tests and does not write. Each camera is uploaded and `queue.submit`ted before the next camera writes globals or instance buffers. Texture, sampler, and mesh geometry caches stay shared. `top_surface_camera` returns the last drawn camera whose viewport contains the window pixel. `camera_view_of` resolves one camera by id, including an inactive camera. `window_to_viewport` subtracts the origin before `ray_from_pixels`. `FrameStats.draw_calls` includes viewport fills. `world_draw_calls` and `submitted_instances` do not. `pass_stats` is per camera.

**Not implemented in M6-2B.** Offscreen render targets, camera-to-texture, `RenderTargetStore`, `RenderTargetId`, `CameraOutput`, target dependencies, target resize, and sampling a camera from a sprite. M6-2C, recorded below, adds those. No render graph. No second renderer.

**CPU.** `engine-renderer` `tests/multi_camera.rs`: zero cameras, one pass, `(order, entity id)` with an inactive camera excluded, a hidden camera entity still rendering, per-camera projection and aspect, a custom matrix fixed across resize, the one-pixel minimum and the clamped oversize viewport, per-camera frustum and billboard axes, per-camera transparent order, shared material revision, owned instance matrices, and surface plus explicit picking including a custom ray through a projected point.

**GPU.** `engine-renderer-wgpu` `tests/multi_camera.rs`, Apple M5, Metal, `Rgba8Unorm` readback. Split viewports keep each sprite in its rectangle and leave the gap on the first clear. Perpendicular billboards fail if either camera reads the other's view or instances. Picture-in-picture `Color` with no objects fills the inset; `Load` with no objects keeps the previous color; an opaque inset replaces it; a 0.5 yellow blend over red is about `[255, 128, 0, 255]`. A near red sprite in camera A does not reject camera B: a kept mask stays white, and a discarded mask lets yellow blend over green to about `[128, 255, 0, 255]`. Equal order paints the higher entity id. The first camera's `Load` matches `Color` and adds no fill draw. A viewport past the target edge is clamped. Perspective, orthographic, and custom projections each hit `CameraView::project` in their own third of a 192×128 target, and the custom centre stays clear. Front and back cameras composite the same blend stack to `[128, 64, 64, 255]` and `[0, 0, 255, 255]`. One mesh asset stays one cache entry while two cameras submit it. Two cameras each drawing 1,000 compatible sprites: `world_draws=2 clear_draws=1 total_draws=3 instances=2000 texture_creates=1 buffer_grows=1`.

The one-camera 1,000-sprite test printed `cpu batches=1 gpu draws=1 instances=1000` and asserted `world_draw_calls=1`, `clear_draw_calls=0`. The two-camera run printed `world_draws=2 clear_draws=1 total_draws=3 instances=2000 texture_creates=1 buffer_grows=1`.

**Screenshots.** Release, 1280×720, SHA-256 matches commit `6dbabb8`. Baselines were not replaced. `basic_2_5d` `c3c5874adeea119c98255c6c1d08208c0244224d930d86e8736c280ef88b38fe`. `basic_2_5d --ortho` `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f`. `spatial_scene --camera perspective` `ec5f6252a88f642b454ae768f9eec8cdd5fba88a5aeb7a424fcefe5fda12275e`. `spatial_scene --camera orthographic` `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde`. Each of those frames had `camera_passes=1` and `clear_draw_calls=0`.

**Repository check.** `scripts/check.sh` on 2026-10-22, macOS arm64, Apple M5, Metal, exit 0. All 4 steps: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (638 passed, 0 failed, 0 ignored, 53 suites), `cargo doc --workspace --no-deps` with `-D warnings`.

M6-2C, offscreen targets and camera-to-texture, is committed locally and is not pushed. Its results follow.

## M6-2C results (2026-10-22)

Offscreen targets and camera-to-texture for D-08. Committed locally. Not pushed. M6 is not complete. D-08 is implemented locally. D-48, D-26, D-07, and D-06 are unchanged. There is no render graph, no automatic dependency reorder, and no previous-frame feedback.

`RenderTargetId` is a `u64` newtype starting at 1. Removed ids are not reused. It is not an `AssetId`. `RenderTargetStore` is a world resource of plain `RenderTargetDesc` values (`id`, `width`, `height`, `generation`). `Game::new` inserts an empty store. The store holds no wgpu objects. `check_render_target_size` rejects a zero axis, an axis above 8192, and a `width * height * 4` overflow, and returns `RenderTargetError`. `resize` keeps the id. An equal size keeps `generation`. A new size increments it. Pixels in the replacement image are undefined until a camera writes that target in the frame. `CameraOutput` is `Surface` (the default from `Camera::new`) or `Target(RenderTargetId)`. `Camera` stays `Copy`. `Sprite.render_target` is an optional source. When it is set, the sample is that color image in full `[0, 1]` UV space, with flips applied, instead of the `SpriteAsset` texture and instead of `MaterialAsset.albedo`. Size, pivot, tint, `base_color`, alpha class, lighting, and material revision stay where they were. The instance still records the asset texture handle. It is not a fabricated target id.

A sprite may sample a target only after an earlier camera in the same extract has written it, and not while its own camera is writing that target. Camera order is not rearranged. A missing, removed, not-yet-written, or self-sampled target binds the magenta missing-texture placeholder. The diagnostic names the target id and the consuming camera, and the same id is logged once until the 64-id buffer is full. The next frame starts with an empty written set, so a GPU image left from the previous frame is not a fallback. Several cameras may write one id. A consumer between them sees the writers that have already run. A camera whose output target is missing is skipped and is not sent to the surface. Removing a camera does not remove a target. Removing a target drops its GPU image on the next frame that no longer lists it.

The color image uses the renderer's RGBA8 format (`Rgba8UnormSrgb` on an sRGB surface, `Rgba8Unorm` when the renderer was created linear), with `RENDER_ATTACHMENT | TEXTURE_BINDING | COPY_SRC`. Depth is a private `Depth32Float` image and is not sampleable. The first camera that writes a destination clears that whole image, including when its policy is `Load`. Later cameras on that destination load color and apply viewport-local `Color` or `Load`. Every pass clears that destination's depth to 1.0. Surface depth is not reused for a differently sized target. An offscreen target does not receive the surface clear. The target sampler is the renderer's default nearest, clamp sampler. A resize replaces the color image, the depth image, the view, and the bind group before the passes. Unchanged targets are kept. Allocation counts are `render_target_creates` and `render_target_reallocs`. They are not texture uploads. `FrameStats` adds `surface_passes` and `offscreen_passes`.

Attachment clears store alpha 1, and the existing blend state keeps framebuffer alpha at 1 over that clear. Camera-to-texture does not produce a transparent-background layer. A mask consumer therefore does not discard texels of a target produced this way. The mask shader still discards a sampled alpha below its cutoff, which the alpha-0 texture test covers. There is no target-specific shader. Sampling uses the renderer's default nearest, clamp sampler. The extent cap is 8192 on each axis, and the device limit still applies. Depth images are not sampleable. GPU tests for this slice ran on Apple M5, Metal.

**CPU.** `engine-renderer` `tests/render_target.rs` covers store identity, rejected sizes, resize and generation, removal, camera defaults, target aspect and viewport, a fixed custom matrix, mixed output order, a missing output, per-camera extraction, producer-before-consumer, a forward reference, self-sampling after an earlier writer, missing and removed ids, an A-B-C chain, a cycle, two writers with a consumer between them and after them, availability resetting on the next extract, albedo and unassigned precedence, full-target UVs against an atlas rect, size and pivot, alpha class, and target-local picking that `top_surface_camera` does not select.

**GPU.** `engine-renderer-wgpu` `tests/render_target.rs`, Apple M5, Metal, `Rgba8Unorm`. `camera_to_texture_displays_the_offscreen_image_on_the_surface`: a 64×64 target holds a red sprite on a blue clear, and the surface sprite shows that image rather than its white atlas or the green surface clear. One target allocation, one texture upload, one sampler, and a second identical frame adds none. `resize_replaces_the_sampled_view_and_leaves_the_other_target`: UV 0.4 changes from red to blue when the target becomes 128×32, the other target stays 32×32 yellow, and reallocs increase by 1. Perspective, orthographic, and custom projections sample through `CameraView::project` on the target image. Custom stays on its matrix after a resize. Legacy tint, opaque base-color times tint with a rejected farther blend, a mask that keeps target alpha 1 and discards an alpha-0 texture, and a blend sample that does not write depth are covered. Forward, self, missing, and removed targets are magenta. A cycle shows magenta on the earlier target and that image on the later one. A frame with the producer removed shows magenta on the surface while the retained GPU image is still the previous red. Two writers cover half viewports, an overlap, `Color` then `Load`, a per-camera depth reset, a consumer between the writers, and a consumer after both. Two sprites on one target are one world draw. Two targets are two draws.

**Screenshots.** Release, 1280×720, SHA-256 matches commit `6dbabb8`. Baselines were not replaced. `basic_2_5d` `c3c5874adeea119c98255c6c1d08208c0244224d930d86e8736c280ef88b38fe` (746071 bytes). `basic_2_5d --ortho` `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f` (462130). `spatial_scene --camera perspective` `ec5f6252a88f642b454ae768f9eec8cdd5fba88a5aeb7a424fcefe5fda12275e` (136991). `spatial_scene --camera orthographic` `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde` (34046). Each frame had `camera_passes=1`, `surface_passes=1`, `offscreen_passes=0`, and `clear_draw_calls=0`.

**Repository check.** `scripts/check.sh` on 2026-10-22, macOS arm64, exit 0. All 4 steps: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (665 passed, 0 failed, 0 ignored, 55 suites), `cargo doc --workspace --no-deps` with `-D warnings`. The check log does not print the adapter name. A direct run of `engine-renderer-wgpu` `tests/render_target.rs` printed Apple M5, Metal, `IntegratedGpu`.

## D-07 reverse-Z accepted (2026-10-23)

Finite reverse-Z on `feat/d07-reverse-z`, from `main` at `e9906fe`. [ADR-0019](docs/adr/0019-reverse-z.md) is Settled and closes D-07. The coplanar draft on `spike/m6-3b-d48-coplanar` (`a176502`) must become ADR-0020 and must not overwrite ADR-0019. D-48 is not implemented. `CoplanarPriority` and `CoplanarSettings` are not in the engine. M6 is not complete. Depth compare is `Greater` and every camera pass clears depth to 0.0. The spike was not merged. Vulkan, DirectX, OpenGL, and lavapipe are not verified. Apple M5, Metal is the verified backend.

`Mat4::perspective_lh` and `Mat4::orthographic_lh` map the near plane to NDC 1 and the far plane to NDC 0. Near and far stay view-space distances. There is no shader `clip.w - clip.z` rewrite. Sprites and meshes share one `Depth32Float` compare. Every camera pass clears its own depth to 0. The viewport depth range stays `0..1`. `ray_from_pixels` unprojects NDC 1 as the origin. Frustum row 2 is the far plane and `r3 - r2` is the near plane. The visible interval is still `0 <= clip.z <= clip.w`. Blend sort is still camera-space Z. `CameraProjection::Custom` is stored unchanged. A 0-near matrix is not converted and occludes backwards until it is rebuilt with those helpers. The example oblique camera follows `orthographic_lh`, so it did not need a second matrix.

**CPU.** `engine-math`: near is 1 and far is 0 for perspective and orthographic, including an orthographic near of 0. The historical 0-near perspective formula, local to the test, quantizes a 1/1024 gap at z = 200 to one `f32`; reverse-Z does not. `engine-renderer` camera tests expect a larger depth for a nearer sprite or mesh, and picking rays still hit the projected point. The degenerate far plane drops NDC z = 0 and still culls a side sphere. `spatial_scene` expects figure A to have a larger NDC depth than figure D under the custom camera.

**GPU.** `engine-renderer-wgpu` `tests/reverse_z.rs`, Apple M5, Metal, `DIMORA_REQUIRE_GPU=1`, not ignored. Perspective, orthographic, and a side-looking orthographic camera: the nearer cutout wins, one world draw, two instances, one pass. A nearer mesh occludes a sprite and the reverse, under perspective and orthographic. A kept mask occludes a farther cutout; a discarded mask does not; a nearer blend composites to about `[255, 127, 127, 255]`; a nearer opaque hides a farther blend. Perspective and orthographic both keep a 1/1024 gap at z = 200 (near 0.1, far 1000) as a red pixel, one draw, two instances. A sprite in front of the near plane and a sprite past the far plane are culled; the sprite between them is drawn. A hand-built 0-near orthographic matrix lets the farther sprite win; the same window rebuilt with `orthographic_lh` lets the nearer sprite win. An oblique custom matrix keeps view-depth order. A later `Load` camera draws its own sprite at the same pixel, so the first camera's depth was cleared. An offscreen target shows the nearer sprite, with `offscreen_passes=1`, `world_draw_calls=1`, `submitted_instances=2`. `custom_projection` (9) and `configurations` (19) passed, including the three coplanar conflict tests. Those tests were not rewritten to name a new tie winner.

**Screenshots.** Release, 1280×720, 2026-10-23 1:01 PM America/Chicago. The `6dbabb8` hashes stay the historical baselines. The reverse-Z references, recorded only after the pixels below were identified, are `basic_2_5d` `196faee1b7eb39d7d0f706b20b50597867c92b72e73189e507b7e43b8098f7f0` (746060 bytes) and `spatial_scene --camera perspective` `cad5f528af127de4e94c16ecbcedea1e238e0ba6e1b72187ddfc19e13b24182b` (137001 bytes). `basic_2_5d --ortho` and `spatial_scene --camera orthographic` are byte-identical to `6dbabb8` (`f2aedc0c…` and `3551d84f…`). Perspective differs in 84 pixels (0.0091%). 77 swap the sRGB colours of coplanar decals "decal equal a" and "decal equal b"; the overlap-interior test still requires the first decal. The other seven are one pixel each on a shared edge of a light and a dark floor slab. Six swap the outer bevel colours `[135, 138, 140]` and `[111, 116, 122]`. `(290, 278)` is that edge under "ghost ordered a" then "ghost pair b", which composites to exactly `[203, 129, 162]` on the light bevel and `[200, 124, 159]` on the dark bevel. A bit-identical depth would not flip. `basic_2_5d` differs in 16 pixels, 11 clusters, largest 3, including one crown-green against trunk-brown; those 16 were not each named to an entity. The cause is written in ADR-0019.

**Performance.** The four frames kept the pre-change counts. `basic_2_5d`: sprites 8848/3783, meshes 6/0, draws 10, passes 1, instances 8848+6, largest draw 7012, submitted 8854, textures 8, meshes 4. Ortho: sprites 809/11822, meshes 5/1, draws 10, instances 809+5, largest 432, submitted 814. Perspective scene: sprites 28/1, meshes 349/282, draws 10, instances 28+349, largest 342, submitted 377, textures 4, meshes 5. Orthographic scene: sprites 20/9, meshes 151/480, draws 8, instances 20+151, largest 144, submitted 171, textures 3, meshes 5. Each frame had `camera_passes=1`, `surface_passes=1`, `offscreen_passes=0`, `clear_draw_calls=0`. No extra pass and no split batch. Timestamp queries were not enabled. This is not a claim that the GPU is faster.

**Repository check.** `DIMORA_REQUIRE_GPU=1 scripts/check.sh` on 2026-10-23, macOS arm64, Apple M5, Metal, exit 0. All 4 steps: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (675 passed, 0 failed, 0 ignored), `cargo doc --workspace --no-deps` with `-D warnings`. The 675 count is every `test result` line in that log, including doc-tests. Ten of them are new: one `engine-math` comparison and nine tests in `engine-renderer-wgpu` `tests/reverse_z.rs`. The M6-2C log on `e9906fe` was 665. The acceptance edit re-ran the same check at 1:37 PM America/Chicago and again recorded 675 passed, 0 failed, 0 ignored.

**Not claimed.** Vulkan, DirectX, OpenGL, or lavapipe. A production thickness. `CoplanarPriority`. M6 is not complete.

## D-48 coplanar priority (2026-10-23)

Opt-in sprite priority on `feat/d07-reverse-z`, after D-07 acceptance `7462163`. [ADR-0020](docs/adr/0020-coplanar-priority.md) is Settled for the coplanar mechanism. Gameplay orientation and D-26 stay open. M6 is not complete. The spike branch `spike/m6-3b-d48-coplanar` (`a176502`) was not merged. Vulkan, DirectX, OpenGL, and lavapipe are not verified. Apple M5, Metal is the verified backend.

`CoplanarPriority(u8)` is `0..=7`. No component is ordinary spatial depth. Level 0 pulls one thickness toward the camera. `CoplanarSettings.thickness` is one world-unit step for the whole world. A missing resource uses `1/256` (`CANDIDATE_COPLANAR_THICKNESS`). `new` returns an error for an out-of-range level and for a non-finite or non-positive thickness. Extraction does not clamp: level 9 is logged and stored as none, and an explicit thickness of −1 is logged and becomes 0 so the sprite keeps its real depth. The sprite shader writes `clip.z` only. `clip.xy` and `clip.w` stay. Reverse-Z compare `Greater` and clear 0 stay. A vertex inside `0 <= clip.z <= clip.w` stays inside that interval. Meshes are not displaced. Tile maps do not read the component. Priority is not a batch key and does not order blend against blend. `spatial_scene` does not opt in, so its three conflict tests still describe that scene.

**Thickness.** Level 7 pulls `8 * thickness`: 0.0078125 at `1/1024`, 0.03125 at `1/256`, 0.125 at `1/64`. `the_candidate_thickness_overrides_only_inside_its_documented_pull` passed all three on Apple M5, Metal: a mesh at `0.35 * pull` is overridden and a mesh at `2 * pull` stays visible. `1/64` was not selected. Its level-7 pull hides four times as much nearer geometry. `1/1024` passed the same check and remains a legal `CoplanarSettings` value. `1/256` is the fallback because it passed and is the value published when a world sets nothing. One thickness. The spike's standard-Z perspective misses at distance 40 and beyond are not the production path.

**CPU.** `engine-renderer` `src/coplanar.rs` unit tests. `tests/coplanar.rs`: priority is not a batch key; levels 0, 4, and 7 share one opaque material batch; a missing component and `CoplanarPriority(9)` extract as none; an explicit `1/64` is used; thickness −1 disables the pull and keeps the level; a tile map with the component still extracts `coplanar_level: None`. `engine` `tests/registry.rs` lists the component and dumps `level: u8 script_visible=false script_writable=false = 3`.

**GPU.** `engine-renderer-wgpu` `tests/coplanar.rs`, 11 tests, not ignored, `DIMORA_REQUIRE_GPU=1`, Apple M5, Metal, `Rgba8Unorm`. Coincident cutouts stay red (the higher level) for perspective, orthographic, side, steep, isometric, top-down with a fixed quad in the XZ plane, and a sheared custom matrix. The same winner holds at perspective distances 5, 40, and 200 and at orthographic cameras `z = -6, -30, -80`. The three-thickness pull table above. A coplanar cutout with level 1 beats a slab, and a plain sprite does not cross a slab one thickness closer. Two blends follow `RenderOrder` rather than priority. A blend 0.5 behind a cutout stays hidden at the `1/256` pull. A coplanar blend of alpha 0.5 with level 4 composites over the cutout. Two sprites of one material are one world draw and two instances; raising the lower sprite to level 7 at runtime swaps the winner. A nearer mesh occludes two sprites that did not opt in, in one cutout batch. A sprite at `z = 0.12` with a 0.125 pull stays visible; a sprite at `z = 0.02` stays clipped (near 0.1). Two viewports and a 64×64 offscreen target each show the higher priority, with `offscreen_passes = 1`. An intersection (wall rotated 0.7 rad about Y, level 7, `1/256`) keeps both colours and does not move a corner pixel outside the quad. 64 cutout sprites with priorities `i % 8` are one batch, one world draw, and 64 instances.

**Screenshots.** Release, 1280×720, 2026-10-23. The examples do not opt in. SHA-256 matches the reverse-Z references. The `6dbabb8` hashes were not replaced. `basic_2_5d` `196faee1b7eb39d7d0f706b20b50597867c92b72e73189e507b7e43b8098f7f0` (746060 bytes). `basic_2_5d --ortho` `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f` (462130). `spatial_scene --camera perspective` `cad5f528af127de4e94c16ecbcedea1e238e0ba6e1b72187ddfc19e13b24182b` (137001). `spatial_scene --camera orthographic` `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde` (34046).

**Performance.** Those four frames match the D-07 counts. Basic: sprites 8848/3783, meshes 6/0, draws 10, instances 8848+6, largest 7012, submitted 8854, textures 8, meshes 4. Ortho: sprites 809/11822, meshes 5/1, draws 10, instances 809+5, largest 432, submitted 814. Perspective scene: sprites 28/1, meshes 349/282, draws 10, instances 28+349, largest 342, submitted 377, textures 4, meshes 5. Orthographic scene: sprites 20/9, meshes 151/480, draws 8, instances 20+151, largest 144, submitted 171, textures 3, meshes 5. Each frame had `camera_passes=1`, `surface_passes=1`, `offscreen_passes=0`, `clear_draw_calls=0`. The sprite instance stays 112 bytes. The camera uniform gains `inv_view_proj` (64 bytes) and a `coplanar` vec4 (16 bytes). No extra pass and no split batch. Timestamp queries were not enabled. This is not a claim that the GPU is faster.

**Repository check.** `DIMORA_REQUIRE_GPU=1 scripts/check.sh` on 2026-10-23, 2:08 PM America/Chicago, macOS arm64, Apple M5, Metal, exit 0. All 4 steps: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (694 passed, 0 failed, 0 ignored, 58 suites), `cargo doc --workspace --no-deps` with `-D warnings`. The 694 count is every `test result` line in that log, including doc-tests. Nineteen are new relative to the D-07 check (675): 3 unit tests in `coplanar.rs`, 5 tests in `engine-renderer` `tests/coplanar.rs`, and 11 tests in `engine-renderer-wgpu` `tests/coplanar.rs`. Those 11 were also run on their own before the suite and passed. The registry test was extended, not added.

**Not claimed.** Vulkan, DirectX, OpenGL, or lavapipe. Mesh or tile opt-in. A skeletal-attachment API. That `1/256` is the right step for every game: a world inserts `CoplanarSettings` to replace it. M6 is not complete.

## D-26 sprite orientation (2026-10-23)

[ADR-0021](docs/adr/0021-sprite-orientation.md) is Settled. `Billboard`, `Upright`, and `Fixed` stay the only policies. `sprite_placement` was not changed. A camera does not write `Transform`. A `Fixed` child does not inherit a parent's rendered billboard basis. Exact `±Y` views keep `Upright` vertical and edge-on, using the camera's right axis, including when that camera is rolled. Opposite near-vertical headings reverse. There is no smoothing. Ground art is `Fixed` with a quarter-turn about X. A sheared projection does not change the world matrix. M11 must orient a 2D skeleton at the visual root; that frame is not implemented. Per-bone billboards are not the character policy.

**CPU.** `engine-renderer` `tests/orientation.rs`, 14 tests: global rotation, world up, both exact vertical views, opposite near-vertical headings, yaw, roll, the rolled pole fallback, zero scale, parent translation and yaw, a fixed child under a billboarded parent, ground art in `XZ`, custom shear, two cameras leaving `Transform` unchanged, and one batch for three policies.

**GPU.** `engine-renderer-wgpu` `tests/orientation.rs`, 4 tests, Apple M5, Metal, `DIMORA_REQUIRE_GPU=1`, not ignored. Opposite near-overhead cameras paint opposite sides of an upright quad along `Y × forward`. Three policies of one material are one batch and three colours. A side viewport sees that upright sprite, an overhead viewport sees it edge-on and sees a billboard, and an offscreen camera from `+X` sees the upright face. `CoplanarPriority` 7 does not paint a face on the edge-on quad. `tests/coplanar.rs` also keeps the higher priority for a top-down billboard, a front fixed quad, a front upright quad, a perspective upright quad, and a side upright quad. `spatial_scene` `configurations.rs` already covers top-down billboard area, the upright edge, steep foreshortening, and mesh occlusion. Those tests were not duplicated.

**Screenshots.** Release, 1280×720, 2026-10-23 4:14 PM America/Chicago. SHA-256 matches the reverse-Z references. The `6dbabb8` hashes were not replaced. `basic_2_5d` `196faee1b7eb39d7d0f706b20b50597867c92b72e73189e507b7e43b8098f7f0` (746060 bytes). `basic_2_5d --ortho` `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f` (462130). `spatial_scene --camera perspective` `cad5f528af127de4e94c16ecbcedea1e238e0ba6e1b72187ddfc19e13b24182b` (137001). `spatial_scene --camera orthographic` `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde` (34046). Draw counts match the D-07 capture: basic sprites 8848/3783, meshes 6/0, draws 10, submitted 8854; perspective scene sprites 28/1, meshes 349/282, draws 10, submitted 377.

**Repository check.** `DIMORA_REQUIRE_GPU=1 scripts/check.sh` on 2026-10-23, 4:19 PM America/Chicago, macOS arm64, Apple M5, Metal, exit 0. All 4 steps: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` with `DIMORA_REQUIRE_GPU=1` (712 passed, 0 failed, 0 ignored, 60 suites), `cargo doc --workspace --no-deps` with `-D warnings`. The 712 count is the sum of every `test result` passed count, including doc-tests. Eighteen are new relative to the D-48 check (694): 14 in `engine-renderer` `tests/orientation.rs` and 4 in `engine-renderer-wgpu` `tests/orientation.rs`. The coplanar configuration list was extended inside the existing test, not added as a new test. Those GPU tests were also run on their own before the suite and passed.

**Not claimed.** A change to placement, shaders, or batch keys. Vulkan, DirectX, OpenGL, or lavapipe. The M11 visual-root frame. Smoothing across the vertical singularity: the reversal is the cylindrical heading, and it stays. M6 is not complete.

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

## M6 debug drawing (2026-10-24)

Single-frame world-space debug lines on `feat/m6-debug-draw` (`b7ec684`), from `abb707c`. Not pushed. M6 is not complete. No new ADR. D-07, D-26, and D-48 are unchanged. `DECISIONS.md` was not edited.

**API.** `DebugDraw` is a world resource, inserted by `Game::new` and reinserted if a step finds it missing. `line`, `ray`, `wire_box`, `wire_box_matrix`, `wire_sphere`, `wire_sphere_segments` (default 16 edges per great circle, clamped at 128), `frustum`, and `axes` append `DebugSegment` values. Axes are three lines, X red, Y green, Z blue, with no arrowheads. A zero length, a negative extent, a non-finite value, a direction that cannot be normalized, or a projection that does not unproject to eight finite corners adds nothing. Alpha 0 is kept. A zero extent on one axis drops only the collapsed edges.

**Lifetime.** `Game::step` clears the resource before any stage. Fixed-update, update, and post-update submissions are still present when the step returns. `extract_scene` copies them and does not clear them. A second extract sees the same list. The next step drops it. Two cameras share that one copy. `ExtractStats::debug_segments` counts it once.

**GPU.** One `LineList` pipeline, group 0 only, unlit vertex color, alpha blend, reverse-Z greater-than test, no depth write. The draw is after the transparent sequence. Vertices upload once per frame. Each camera draws that buffer. Empty input does not upload and does not call `draw`. The pipeline and a 4,096-byte buffer are created with `SceneRenderer`. wgpu 30 has no line width, so the contract is one pixel. Lines are not lit, fogged, or shadowed. Opaque, mask, and cutout writes hide a farther line. A blended draw does not write depth, so a line can paint over it. Segments are not CPU-culled. There is no depth bias.

**CPU tests.** `engine-renderer` `debug::tests` (lines, rays, box corners and edges, sphere chords, reverse-Z ortho and perspective frusta, a custom matrix, a top-down frustum, invalid input, axes). `tests/debug_draw.rs` (missing and empty resources, two cameras, a copy that survives a later clear). `engine` `tests/debug_draw.rs` (clear at the start of the step, fixed and update and post-update, two fixed iterations, a removed resource). Doctest on `DebugDraw`.

**GPU tests.** `engine-renderer-wgpu` `tests/debug_draw.rs`, 11 tests, Apple M5, Metal, `DIMORA_REQUIRE_GPU=1`, `Rgba8Unorm`. A world line, a wire box and sphere, a line hidden behind an unlit cube, a nearer line drawn over that cube, one batch for several primitives and one extra draw beside a mesh, perspective, orthographic, and a custom matrix, an oblique `looking_at` camera, two viewports, an offscreen target, empty debug leaving sprite and mesh pixels and draw counts unchanged, and a cutout that hides a line while a blend does not, with the surface sprite still sampling the target center.

**Performance.** [docs/PERFORMANCE.md](docs/PERFORMANCE.md), "Debug lines". Release, Apple M5, 2026-10-24 12:08 PM America/Chicago. Ten thousand segments: generation about 41 µs, extraction about 5.1 µs, one 400,000-byte copy. A second camera adds 656 bytes, not a second copy. Empty and absent match. GPU time was not measured.

**Repository check.** `DIMORA_REQUIRE_GPU=1 scripts/check.sh` on 2026-10-24 at 12:13 PM America/Chicago, macOS arm64, Apple M5, Metal, `rustc 1.99.0`. Exit 0. `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (758 passed, 0 failed, 0 ignored, 67 suites), and `cargo doc --workspace --no-deps` with `-D warnings`.

**Screenshots.** Release, 1280×720, 2026-10-24. `debug_segments` is 0. Draw counts are unchanged: `basic_2_5d` 10 draws, SHA-256 `196faee1b7eb39d7d0f706b20b50597867c92b72e73189e507b7e43b8098f7f0` (746,060 bytes); `basic_2_5d --ortho` 10 draws, `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f` (462,130 bytes); `spatial_scene --camera perspective` 10 draws, `cad5f528af127de4e94c16ecbcedea1e238e0ba6e1b72187ddfc19e13b24182b` (137,001 bytes); `spatial_scene --camera orthographic` 8 draws, `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde` (34,046 bytes). These match the hashes recorded after tilemap chunking. No new example image was added.

**Not implemented.** The depth-visualisation view. Screen-space overlay and X-ray. Line thickness. Persistent shapes. CPU frustum culling of segments. Depth bias. A spot-light cone. Editor selection, picking, gizmos, and undo. Collision, lighting, and skeleton callers. Those callers use this resource later: a bone is `line`, a joint or a visual root is `axes`, a point-light range is `wire_sphere`, a direction is `ray`, a shadow camera is `frustum`, a collider is `wire_box` or `wire_box_matrix`. None of those systems is changed here. KI-31 is unchanged.

**Next, as of this section.** The render-scale benchmark below followed this checkpoint. KI-31 stays deferred. The depth view stays an open M6 slice and needs its own color-mapping choice before code. D-06 stays last. Gameplay orientation waits for M7 and M8. M6 is not complete.

## M6 render scale (2026-10-24)

CPU extraction and CPU encode/submit for sprites, meshes, a mixed lane, lights, and one to seven cameras. Branch `feat/m6-render-scale`. Harness commit `82972c2`. Rustdoc links `b26dd3a`. Not pushed. M6 is not complete. No new ADR. No renderer optimization. `DECISIONS.md` and the accepted ADRs were not edited. D-02 stays provisional: propagating 10,000 sprite entities is about 103 µs, and extracting them is about 1.67 ms.

**Harness.** `engine_renderer::bench_scene` builds the worlds. `render_scale_bench` times extraction, propagation, allocator traffic, and retained scene capacity. `examples/render_scale_gpu.rs` times `SceneRenderer::render` and the following driver wait. Both refuse a debug build. `SceneRenderer::instance_buffer_capacity_bytes` reports reserved GPU instance-buffer bytes and does not change a draw. The only production change is that counter.

**CPU tests.** `bench_scene::tests`, 14 tests: unique labels, sphere index count above the cuboid, one opaque batch across coplanar levels, eight opaque materials, three orientations in one batch, mask and blend and legacy cutout layers, interleaved blended materials splitting, a tight camera culling a sparse grid, the 16-light cap keeping a directional, two cameras sharing debug segments once, hierarchy children and repeatable builds, distinct meshes drawing once per asset, the lane mixing tiles and sprite classes and meshes, and offscreen versus surface pairs keeping two passes.

**Commands.**

```
cargo run --release -p engine-renderer --bin render_scale_bench
cargo run --release -p engine-renderer-wgpu --example render_scale_gpu
```

**Evidence.** [docs/PERFORMANCE.md](docs/PERFORMANCE.md), "Render scale". Apple M5, Metal, 2026-10-24, 1:00 PM America/Chicago (CPU) and 1:02 PM (GPU). Ten thousand opaque sprites extract in about 1.67 ms and encode in 159 µs, one batch. One hundred thousand opaque sprites extract in 19.06 ms. A tight camera that keeps 20 of 10,000 sprites still extracts in 1.54 ms. Seven cameras take about 2.4× one camera's extract and exactly 7× the retained instance capacity. Sixteen lights do not move extract or encode. Adding 22 meshes to the lane is a few microseconds. One thousand interleaved blended materials become 1,000 draws and move encode from 44 µs to 281 µs. GPU pass timestamps were not measured. The completion wait is not that time.

**Follow-ups.** KI-31 stays deferred. The million-tile full view is still the case that would justify instance reuse. KI-32 records the per-renderable `hides` allocation and can wait. Do not regroup blended sprites by material. Do not add a retained render world, mesh instancing, or GPU-driven culling from these numbers.

**Repository check.** `DIMORA_REQUIRE_GPU=1 scripts/check.sh` on 2026-10-24 at 1:17 PM America/Chicago, macOS arm64, Apple M5, Metal, `rustc 1.99.0`. Exit 0. `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (772 passed, 0 failed, 0 ignored, 68 suites), and `cargo doc --workspace --no-deps` with `-D warnings`. The 772 count is 14 more than the debug-draw check (758): the 14 `bench_scene` tests. The 68th suite is the `render_scale_bench` binary, which has no tests of its own.

**Screenshots.** Release, 1280×720, 2026-10-24 at 1:18 PM America/Chicago. `debug_segments` is 0. Draw counts and SHA-256 match the debug-draw baselines. `basic_2_5d` 10 draws, `196faee1b7eb39d7d0f706b20b50597867c92b72e73189e507b7e43b8098f7f0` (746,060 bytes). `basic_2_5d --ortho` 10 draws, `f2aedc0c4a65e13a33187f391d2a778d073b3befbf1ce207bc6484d961ac980f` (462,130 bytes). `spatial_scene --camera perspective` 10 draws, `cad5f528af127de4e94c16ecbcedea1e238e0ba6e1b72187ddfc19e13b24182b` (137,001 bytes). `spatial_scene --camera orthographic` 8 draws, `3551d84fe2ec1c14fc0729c7a86e2031c068285a6551597ee32d100f76805cde` (34,046 bytes).

**Not claimed.** GPU shader time. Vulkan or another backend. Texture bandwidth. Mesh complexity past 100 spheres. Sprite sheets, skeletal poses, skinned meshes, shadows, or an editor viewport. A change to batching, depth, orientation, or lighting. M6 is not complete.

**Next.** The depth-visualisation view. It needs a color-mapping choice before code. D-06 stays last. Engine camera presets and the public oblique builder stay open. Gameplay orientation waits for M7 and M8. KI-31 and KI-32 wait. M6 is not complete.

## Not in scope now

M5-2A, CPU texture descriptors, is committed locally as `c687d5b` and is not pushed. M5-2B is commit `11b914f` and is not pushed. M5-2C is commit `622ef80` and is not pushed. M5-3A, bounded region transitions, is commit `b51f1fb` and is not pushed. M5-3B, GPU rectangle uploads, is commit `84d5ab1` and is not pushed. M5-4A, the CPU shelf packer, is commit `bf4d9bb` and is not pushed. M5-4B, GPU atlas sampling, is commit `1666695` and is not pushed. M5-5A, the CPU `MaterialAsset`, is commit `e0c29ef` and is not pushed. M5-5B, material-driven rendering, is commit `3b3cd4a` and is not pushed. M5-5C, unified blended rendering, is commit `ab1b6f7` and is not pushed. M5-5D, material-aware batching, is commit `a7efd6c` and is not pushed. M5-6, procedural mesh generators, is commit `92286ce` and is not pushed. M5 is complete locally and accepted. Localized mip-texel regeneration is deferred and is not implemented. M6-1B, custom projection matrices, is commit `a38cb30` and is not pushed. M6-1C, the example custom camera, is committed locally and is not pushed. M6-2B, surface cameras, is commit `6a4788290b8bd69c56255f12b7a557a78e5583c6` and is not pushed. M6-2C, camera-to-texture, is committed locally and is not pushed. D-08 is implemented locally. Coplanar sprite priority is ADR-0020. D-26 is settled (ADR-0021). Tilemap chunks are in extraction. The baseline and comparison are in [docs/PERFORMANCE.md](docs/PERFORMANCE.md). Debug drawing is implemented on `feat/m6-debug-draw` (`b7ec684`). The sprite, mesh, and mixed-scene benchmark is recorded on `feat/m6-render-scale` (`82972c2`). The depth view and gameplay orientation have not started. In particular: no scene format beyond M4, no new prefab work, no collision, no
editor work, and no work on UI, scripting or any of the three character representations. M4 Phase 1 is commit `cdb5ec3`. Phase 2 is commit `2c76f71`. Phase 3 is commit `f9a4aab`. M4-4A is commit `302622b`. M4-4B is commit `128c70e`. M4-4C is commit `e12fb5b`. M4-4D is commit `8398e49`. M4-4E is commit `d58c66b`. M4-5 is commit `2b79806`. None of M4 is pushed. M4 is complete locally. M3b is complete locally and is not pushed. These systems are deliberately
not being started together: each has its own milestone, prerequisites and decisions. Decisions that data
formats or public APIs will bake in are made before their first consumer (see
[docs/DECISIONS.md](docs/DECISIONS.md#what-to-resolve-first)).
