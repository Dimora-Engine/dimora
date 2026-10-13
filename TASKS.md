# Tasks: current development state

Last audited: 2026-10-12 at commit `6ad872f`. The roadmap was expanded on 2026-10-13 (three character
representations, a UI runtime, gameplay scripting); that was a documentation change and the code and tests
are unchanged since the audit, so the verification record below still describes the code. Nothing is marked
done here without evidence from the repository or a recorded test run. Long-range plan: [ROADMAP.md](ROADMAP.md). Architecture:
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). Open decisions: [docs/DECISIONS.md](docs/DECISIONS.md).

## Current milestone

**M2. Foundation hardening and CI** (not started). Priority 1 in the ordering rules: fix the defects
the audit found in the existing foundation, and make the build reproducible, before adding features.

## Completed work

| Milestone | Evidence |
|---|---|
| M0 Foundation | Commits `6b0d016`..`278de15`: math, core, world, assets, renderer, wgpu backend, animation, platform, facade, `basic_2_5d`. All crates build; tests pass (below) |
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
they are exercised by `--screenshot` runs and manually.

## Known issues

"Verified" means reproduced by running something; "read" means found by reading the code and not
yet reproduced.

| ID | Issue | Evidence | Milestone |
|---|---|---|---|
| KI-1 | No `rust-version` or toolchain pin; the workspace does not build on rustc 1.83 | Verified | M2 |
| KI-2 | GPU tests skip silently when no adapter exists and still report `ok` | Verified | M2 |
| KI-3 | No CI of any kind | Repository listing | M2 |
| KI-4 | GPU texture and mesh caches are never evicted; `AssetManager` has no `remove` | Read (`scene_renderer.rs`, `manager.rs`) | M2 |
| KI-5 | Only one camera renders per frame, so `ViewportRect` cannot give split-screen/minimap; no render-to-texture | Read (`extract.rs::active_camera`) | M6 |
| KI-6 | Textures: no mipmaps, clamp only, always sRGB; `Material` is colour + albedo + `lit`; meshes cannot be transparent | Read | M5 |
| KI-7 | A child of an entity without a `Transform` stays at the origin | Verified with a scratch test (child expected at x = 5, got 0) | M2 |
| KI-8 | `World::despawn(parent)` leaves children with a dangling `Parent`; they stay at identity forever | Verified (same scratch test) | M2 |
| KI-9 | Resource access is `RefCell`: double borrows panic at runtime; no deferred commands; `hecs` has no events or change detection | Read | M3 |
| KI-10 | Frame order is hard-wired in `Game::step`; user systems cannot be ordered relative to engine stages; `FixedTimestep::alpha()` is not exposed, so no render interpolation | Read | M3 |
| KI-11 | `Camera` stores the closed `CameraProjection` enum, not an arbitrary `Projection` | Read | M6 |
| KI-12 | Assets: synchronous loading; PNG is the only decoder (sprites, meshes, skeletons, clips are code-built); no hot-reload watcher; `AssetId` is a name hash with no collision check and changes on rename | Read | M17 (collision check in M2) |
| KI-13 | Skeletal animation has no blending, events, IK, skinning or file loading | Read | M11 |
| KI-14 | Blended sprites sort by quad centre only; large or intersecting blended quads can mis-order | Read | M5/M14 |
| KI-15 | Input has 47 keys, no gamepad/touch/text/actions/cursor grab, no F-keys | Read | M3 (text and scale factor for UI: M10) |
| KI-16 | Two rustdoc warnings: unresolved `World` and `Schedule` links in `crates/engine/src/lib.rs` | Verified | M2 |
| KI-17 | `Cargo.toml` declares `MIT OR Apache-2.0` but no licence files exist | Repository listing | M2 |
| KI-18 | `Visibility::Hidden` applies to the entity only; it does not hide children | Read (`extract.rs`) | M4 |
| KI-19 | Performance is unmeasured: extraction is O(renderables) every frame, tilemaps expand per tile per frame, instance buffers are fully rewritten; the only timings available come from a software rasterizer | Read | M6 |
| KI-20 | Platform: single window, `ControlFlow::Poll`, default present mode; no fullscreen/vsync configuration | Read | M3 |
| KI-21 | A sprite's natural size comes from the whole `SpriteAsset`, not from `Sprite.region`, so showing a sheet frame without also setting `size` stretches it to the asset's size | Read (`extract.rs`: `sprite.size.unwrap_or_else(|| asset.world_size())`) | M8 |
| KI-22 | `animate_system` writes sampled transforms directly onto bone entities and only visits entities that have a `SkeletonInstance`; `Pose` is not in the path, so there is no stage for blending or IK, and a skeleton-less sprite cannot be animated by it | Read (`clip.rs`) | M11 (sprite sheets: M8) |
| KI-23 | `Vertex` is position, normal and uv; no joints, weights or tangents; no model loader; meshes are code-built | Read (`mesh.rs`) | M12 |
| KI-24 | The platform layer does not forward the window scale factor (no use of it anywhere in the code), `Viewport` is one pixel size, and there are no text-input events, so resolution-independent UI has no foundation | Read (search for `scale_factor` finds nothing) | M10 |
| KI-25 | Any change to a texture re-uploads the whole texture (cache keyed by `AssetId` and revision); there is no sub-rectangle update, which a glyph atlas needs | Read (`scene_renderer.rs`) | M5 |

## Blockers

| ID | Blocker | Effect |
|---|---|---|
| B-1 | Code host and CI provider are undecided. The repository lives on Origin; a GitHub push to the `Dimora-Engine` organisation could not be done from this environment (no GitHub credentials) | Blocks only the CI config file in M2. Everything else, including `scripts/check.sh`, proceeds |
| B-2 | GPU tests need a Vulkan adapter; fresh VMs do not have one (`sudo apt install mesa-vulkan-drivers`) | Not blocking once documented (task T1 documents it and makes the absence loud) |
| B-3 | D-03 (despawn semantics) and D-04 (GPU cache lifetime) are unmade. Defaults are proposed in DECISIONS.md | Blocks T2 and T3 until accepted or changed; low risk |

## Next three tasks

All three are inside M2, ordered by dependency and by how much they unblock. **The single next implementation task is T1.** After M2 the roadmap continues with M3 (input and
scheduling); sprite-sheet animation (M8) cannot start before M3 and M5.

### T1. Reproducible toolchain and one verification command

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

### T2. GPU resource lifetime and `AssetManager::remove` (KI-4)

**Dependencies:** T1 (verification through `scripts/check.sh`); D-04 accepted or changed (B-3).

**Acceptance criteria:**

1. `AssetManager::remove::<T>(handle)` removes the entry and its dependency links; `get`/`state` then
   report unknown.
2. `SceneRenderer` evicts a cached texture or mesh when its asset is gone, and (per D-04) when it has
   not been used for a configurable number of frames. It exposes counts of cached textures/meshes.
3. A headless test renders a scene, removes the assets and renders again, and shows the cache counts
   return to the empty baseline; re-inserting an asset re-uploads it correctly (pixel check).
4. A test shows a replaced asset (revision bump) is re-uploaded without leaking the old GPU copy.
5. Both examples still render, and `spatial_scene` shows no growth in cache counts over 1,000 frames.

### T3. Hierarchy robustness and asset-id collision guard (KI-7, KI-8, KI-12 part)

**Dependencies:** T1; D-03 accepted or changed (B-3). Independent of T2: either order works.

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
