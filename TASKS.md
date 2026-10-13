# Tasks: current development state

Last audited: 2026-10-12 at commit `6ad872f` (Linux). Re-verified on macOS on 2026-10-13 from `a0b4662`
(baseline) and again after task T1 (see "Verification record (macOS)"). The roadmap was expanded on
2026-10-13 (three character representations, a UI runtime, gameplay scripting); that was a documentation
change. Nothing is marked
done here without evidence from the repository or a recorded test run. Long-range plan: [ROADMAP.md](ROADMAP.md). Architecture:
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). Open decisions: [docs/DECISIONS.md](docs/DECISIONS.md).

## Current milestone

**M2. Foundation hardening and CI** (in progress: T1, T2 and T3 done). Priority 1
in the ordering rules: fix the defects the audit found in the existing foundation, and make the build
reproducible, before adding features.

## Completed work

| Milestone | Evidence |
|---|---|
| M0 Foundation | Commits `6b0d016`..`278de15`: math, core, world, assets, renderer, wgpu backend, animation, platform, facade, `basic_2_5d`. All crates build; tests pass (below) |
| M2 / T3 Hierarchy robustness and asset-id collision guard | Commits `498f77a`, `e1f70b1`, `8abee2e`, `a4f9e77`; evidence under "Task T3 results" below. Verified on macOS only |
| M2 / T2 GPU resource lifetime and `AssetManager::remove` | Commits `884b0ff`, `1d92ce7`, `5d6a199`, `c3c98f8`; evidence under "Task T2 results" below. Verified on macOS only |
| M2 / T1 Reproducible toolchain and one verification command | Commits `e08033e`, `3a7a95c`, `7fb2c4d`, `3d467b6`, `89b1fa5`; evidence under "Task T1 results" below. Verified on macOS only |
| M1 Core spatial renderer | Commits `83d9883`..`6ad872f`: sprite orientation, `RenderOrder`, sprite `region`, camera `ViewportRect`, `spatial_scene`, `docs/SPATIAL_RENDERING.md`. `engine-renderer/tests/camera.rs` (27 tests), GPU tests `camera_viewport_confines_the_image_to_its_rectangle` and `billboards_face_the_camera_but_fixed_quads_go_edge_on` pass with a Vulkan adapter. `spatial_scene` run windowed under Xvfb: figure heights shrink with depth in perspective and stay equal in orthographic |

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

## Known issues

"Verified" means reproduced by running something; "read" means found by reading the code and not
yet reproduced.

| ID | Issue | Evidence | Milestone |
|---|---|---|---|
| KI-1 | ~~No `rust-version` or toolchain pin; the workspace does not build on rustc 1.83~~ **Fixed by T1**: `rust-toolchain.toml` pins 1.99.0, `rust-version = "1.90"` verified on 1.90.0 (macOS only, see "Task T1 results") | Verified | M2 |
| KI-2 | ~~GPU tests skip silently when no adapter exists and still report `ok`~~ **Fixed by T1** for runs that set `DIMORA_REQUIRE_GPU=1` (`scripts/check.sh` does). Plain `cargo test` still skips and reports `ok` by design | Verified (forced no-adapter run on macOS) | M2 |
| KI-3 | No CI of any kind | Repository listing | M2 |
| KI-4 | ~~GPU texture and mesh caches are never evicted; `AssetManager` has no `remove`~~ **Fixed by T2** (commits `884b0ff`, `1d92ce7`, `5d6a199`, `c3c98f8`; evidence under "Task T2 results"). Remaining: instance buffers only grow; nothing calls `remove` automatically until scenes exist (M4) | Verified | M2 |
| KI-5 | Only one camera renders per frame, so `ViewportRect` cannot give split-screen/minimap; no render-to-texture | Read (`extract.rs::active_camera`) | M6 |
| KI-6 | Textures: no mipmaps, clamp only, always sRGB; `Material` is colour + albedo + `lit`; meshes cannot be transparent | Read | M5 |
| KI-7 | ~~A child of an entity without a `Transform` stays at the origin~~ **Fixed by T3** (commit `498f77a`): such an entity counts as identity, at any depth | Verified (test failed before, passes after) | M2 |
| KI-8 | ~~`World::despawn(parent)` leaves children with a dangling `Parent`~~ **Fixed by T3** (commit `e1f70b1`, D-03): `despawn` removes the whole subtree; `detach_children` keeps children as roots. `despawn_recursive` is gone | Verified | M2 |
| KI-9 | Resource access is `RefCell`: double borrows panic at runtime; no deferred commands; `hecs` has no events or change detection | Read | M3 |
| KI-10 | Frame order is hard-wired in `Game::step`; user systems cannot be ordered relative to engine stages; `FixedTimestep::alpha()` is not exposed, so no render interpolation | Read | M3 |
| KI-11 | `Camera` stores the closed `CameraProjection` enum, not an arbitrary `Projection` | Read | M6 |
| KI-12 | Assets: synchronous loading; PNG is the only decoder (sprites, meshes, skeletons, clips are code-built); no hot-reload watcher; `AssetId` is a name hash and changes on rename. The collision check is **done by T3** (commit `8abee2e`): `insert`, `load` and `reload` return `AssetError::IdCollision` and never overwrite. Remaining limit: the check is per asset type (see "Task T3 results") | Read | M17 |
| KI-13 | Skeletal animation has no blending, events, IK, skinning or file loading | Read | M11 |
| KI-14 | Blended sprites sort by quad centre only; large or intersecting blended quads can mis-order | Read | M5/M14 |
| KI-15 | Input has 47 keys, no gamepad/touch/text/actions/cursor grab, no F-keys | Read | M3 (text and scale factor for UI: M10) |
| KI-16 | ~~Two rustdoc warnings: unresolved `World` and `Schedule` links in `crates/engine/src/lib.rs`~~ **Fixed by T1** (commit `e08033e`); `cargo doc` with `-D warnings` is clean | Verified | M2 |
| KI-17 | `Cargo.toml` declares `MIT OR Apache-2.0` but no licence files exist | Repository listing | M2 |
| KI-18 | `Visibility::Hidden` applies to the entity only; it does not hide children | Read (`extract.rs`) | M4 |
| KI-19 | Performance is unmeasured: extraction is O(renderables) every frame, tilemaps expand per tile per frame, instance buffers are fully rewritten; the only timings available come from a software rasterizer | Read | M6 |
| KI-20 | Platform: single window, `ControlFlow::Poll`, default present mode; no fullscreen/vsync configuration | Read | M3 |
| KI-21 | A sprite's natural size comes from the whole `SpriteAsset`, not from `Sprite.region`, so showing a sheet frame without also setting `size` stretches it to the asset's size | Read (`extract.rs`: `sprite.size.unwrap_or_else(|| asset.world_size())`) | M8 |
| KI-22 | `animate_system` writes sampled transforms directly onto bone entities and only visits entities that have a `SkeletonInstance`; `Pose` is not in the path, so there is no stage for blending or IK, and a skeleton-less sprite cannot be animated by it | Read (`clip.rs`) | M11 (sprite sheets: M8) |
| KI-23 | `Vertex` is position, normal and uv; no joints, weights or tangents; no model loader; meshes are code-built | Read (`mesh.rs`) | M12 |
| KI-24 | The platform layer does not forward the window scale factor (no use of it anywhere in the code), `Viewport` is one pixel size, and there are no text-input events, so resolution-independent UI has no foundation | Read (search for `scale_factor` finds nothing) | M10 |
| KI-25 | Any change to a texture re-uploads the whole texture (cache keyed by `AssetId` and revision); there is no sub-rectangle update, which a glyph atlas needs | Read (`scene_renderer.rs`) | M5 |
| KI-26 | macOS dev machine (Apple M5, macOS 27.0): rustc **older than 1.99** cannot link anything, not even `fn main(){}`, with `ld: tapi error: malformed file ... libSystem.B.tbd ... unknown architecture arm64e.x1-macos`. Cause seen: `xcode-select -p` points at Xcode 26.4.1 (`ld-1266.8`) but `xcrun --show-sdk-path` returns the macOS 27.0 SDK from the Command Line Tools, which that linker cannot read; rustc 1.99.0 links fine (not investigated why). Host setup, not engine code; nothing was changed system-wide. Workaround used for the 1.89.0 and 1.90.0 runs: `SDKROOT=/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk`. Proper fix (user's decision): select a matching Xcode or CLT with `xcode-select`, or update Xcode | Verified (hello-world reproduction with 1.89.0 and 1.90.0 fails, with the `SDKROOT` above succeeds, 1.99.0 succeeds without it) | Environment |
| KI-27 | The MSRV (1.90) has only been built and tested on macOS arm64. On Linux and Windows the floor comes from `ordered-float 5.5.0` in the locked graph (cargo 1.89 refuses it), but a build and test on 1.90.0 there has not been run. The MSRV holds only with the committed `Cargo.lock` | Verified on macOS; Linux/Windows read from cargo metadata | M2 (CI) |

## Blockers

| ID | Blocker | Effect |
|---|---|---|
| B-1 | Code host and CI provider are undecided. The repository lives on Origin; a GitHub push to the `Dimora-Engine` organisation could not be done from this environment (no GitHub credentials) | Blocks only the CI config file in M2. Everything else, including `scripts/check.sh`, proceeds |
| B-2 | GPU tests need an adapter: Metal on macOS (always present on a Mac), a Vulkan driver on Linux; fresh Linux VMs do not have one (`sudo apt install mesa-vulkan-drivers`) | Resolved by T1: documented in README, and `DIMORA_REQUIRE_GPU=1` (set by `scripts/check.sh`) makes the absence a failure |
| B-3 | ~~D-03 (despawn semantics) and D-04 (GPU cache lifetime) are unmade~~ **Resolved 2026-10-13**: the owner accepted the proposed defaults for both (recorded in DECISIONS.md and ARCHITECTURE.md) | None |

## Next three tasks

All three are inside M2, ordered by dependency and by how much they unblock. **T1, T2 and T3 are done
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

## Remaining work in M2 (after T1-T3)

- Licence files matching `Cargo.toml` (KI-17).
- Choose the code host and add CI configuration that runs `scripts/check.sh` (B-1, D-22).
- Re-run the audit table in ROADMAP.md and update this file when M2 is complete.

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
| Gameplay scripting | M18 | Not started; prerequisites (registry, commands, events) missing |
| Audio | M16 | Not started |

## Not in scope now

Everything from M3 onward. In particular: no new rendering features, no collision, no editor work, and no
work on UI, scripting or any of the three character representations until M2 is finished and the plan is
re-read. These systems are deliberately not being started together: each has its own milestone,
prerequisites and decisions.
