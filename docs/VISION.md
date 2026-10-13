# Product vision

Status: **proposed, derived from the existing README, ROADMAP and ARCHITECTURE, and from reading the code
(audit of 2026-10-13, commit `4857158`). The owner has not yet confirmed it.** Items marked *(owner to
confirm)* are product choices that the repository does not settle. Everything here is intent, not a
statement of what exists; what exists is in [CAPABILITIES.md](CAPABILITIES.md).

> **2.5D is a spatial world, not a fake 2D layer system.** Sprites, tilemaps, skeletal 2D characters,
> 3D meshes and 3D characters live in one coherent world and share compatible transforms, camera
> behaviour, depth and resource infrastructure.

That does **not** mean every subsystem shares one implementation. A sprite batch, a skinned mesh and a UI
panel need different pipelines. What they share is the *world contract*: a real 3D position, one camera
model, one depth and ordering story, one lifetime model for resources.

## What Dimora is

A code-first Rust game engine for **spatial 2.5D games**: games whose look comes from flat art
(sprites, tiles, 2D rigs) standing in a world that also has depth, perspective, parallax, lights and
sometimes real 3D geometry. Typical looks: "HD-2D" style RPGs, paper-cutout and diorama adventures,
2D characters walking through 3D scenery, isometric or tilted-camera action games.

## Problems it intends to solve

1. **Mixed 2D and 3D is a workaround elsewhere.** In general-purpose engines, 2D and 3D content usually
   run through separate renderers, camera types or layer systems, and putting a sprite *between* two
   meshes, or a 3D character in a tile world, takes effort and special cases. (This is our understanding of
   the field; it was not re-verified for this document.) Dimora makes it the default: a sprite at
   `Z = 300` is occluded by a mesh at `Z = 100` because of position, not because of an ordering number.
2. **Integer layers drift from the world.** A z-index cannot express "the camera orbits and the character
   must face the right way", parallax, shadows or collision at a different depth. A real `Vec3` can.
3. **Small teams need a path from prototype to shipped desktop game without learning a general-purpose
   toolchain**: assets in, game out, with logic in Rust (and later in a script language).
4. **Engines should be verifiable.** Rendering, animation, layout and simulation are tested on plain data
   and by pixel tests, and runs can be replayed deterministically. This is a design goal, partly
   achieved (see CAPABILITIES.md), not a marketing claim.

## Intended users and primary use cases

| User | What they need from Dimora |
|---|---|
| Solo developer or very small team building a 2.5D game in Rust *(owner to confirm)* | Spatial sprite/mesh rendering, characters, tilemaps, collision, UI, audio, and a build they can ship |
| Pixel-art or illustrated-art game developer who wants depth, perspective and lighting without leaving 2D art | Pixel-correct sprite sheets, atlases, directional animation relative to the camera, normal-mapped sprites |
| Developer who wants to mix a few 3D props or characters into a 2D-art world | glTF meshes and skinned characters in the same depth buffer, lights and camera |
| Later: a designer or scripter working with the team | Scene files, hot reload, an editor, gameplay scripts |

Primary use cases, in the order the roadmap proves them:

1. A walkable spatial scene with sprites and meshes at different depths (exists: `spatial_scene`).
2. A playable level with a sprite-sheet character, collision, triggers and a HUD (M3 to M10).
3. The same character with a skeletal 2D or a 3D visual, unchanged gameplay (M11, M12).
4. A content pipeline: files on disk, hot reload, scenes, a bundle to ship (M17 to M21).

## What distinguishes Dimora

* **The world is spatial.** Depth is position. Orthographic and perspective cameras look at the same
  world; parallax appears when the projection is perspective and nowhere else.
* **Characters are separate from their pictures.** One gameplay entity (collision, movement, state) with a
  replaceable visual: sprite sheet, 2D skeleton or 3D skinned mesh (D-28). Swapping the picture must not
  change gameplay.
* **The camera is part of the content.** A billboarded sprite chooses its frames from where the camera is
  (D-31). In a flat engine this is not a question.
* **One lighting and depth model** for sprites, tiles and meshes.
* **Honest engineering.** Plans separate "exists and is tested" from "proposed", with evidence.

## Design principles

When two options are close, these decide, in this order. Each has a test: a question to ask of a change.

1. **Spatial first.** Position in the world decides visibility, occlusion, parallax and collision. *Does
   this change add a layer or order number that a position could express instead?* Explicit ordering is
   allowed only as a narrow, documented override (`RenderOrder` for blended sprites).
2. **One contract, many implementations.** Subsystems agree on transforms, camera, depth, handles,
   events and lifetimes, not on one code path. *Does this force two different things through one
   abstraction, or force one thing to duplicate a contract?*
3. **Data in, data out at every seam.** The world produces plain data (`RenderScene`, a future UI draw
   list, a pose, an input log); backends and tools consume it. *Can this be tested without a GPU, a
   window or a clock?*
4. **Gameplay does not know its pictures; tools and scripts do not know the renderer.** Dependencies
   point from presentation to simulation, never back. Scripts and the editor see a small, versioned
   surface, not internals.
5. **Foundations before features, and proof before abstraction.** Build the smallest working version of
   a feature end to end, with a test and an example, then generalise. Do not extract a shared
   abstraction until two real implementations show the same shape (D-29).
6. **Decide late, but decide before the data format or public API depends on it.** Data formats, ids and
   public APIs are expensive to change; implementations are not. Prototype to find out, then record
   (see [DECISIONS.md](DECISIONS.md)).
7. **Never silently wrong.** A missing asset, a bad file, a failed upload or a script error is reported
   with context and the engine keeps running where it safely can. Tests that skip because a GPU is
   absent must be able to fail (`DIMORA_REQUIRE_GPU`).
8. **Deterministic by default.** Same inputs, same state: fixed-step simulation, seeded randomness, no
   wall-clock in gameplay. This is what makes replay tests, soak tests and any later networking possible.
9. **Small surface, honest status.** Every public item is a promise. Prefer fewer, better-tested APIs.
   Docs never mark something done because a type for it exists.
10. **Performance is a budget, measured.** No optimisation without a measurement and no feature that
    cannot state its cost (M6 creates the baseline).

## Platforms and developer experience

* **Targets: Windows, macOS and Linux desktop** (wgpu over DX12/Vulkan, Metal, Vulkan; D-23). Web, mobile
  and consoles are out of scope until after the first release (M24). Verified so far: macOS arm64 (Metal)
  and Linux x86_64 (software Vulkan, lavapipe). **Windows has never been built or run by the project.**
* **Toolchain:** stable Rust, pinned in `rust-toolchain.toml`, MSRV 1.90.
* **Developer loop (target, not current):** `cargo run` opens a window within seconds; assets are loaded
  from files and hot-reload; errors name the asset and the problem; `scripts/check.sh` is the one
  command that says whether a checkout is healthy; headless screenshots and replays make rendering and
  gameplay regressions visible without a person looking.
* **Code first.** Games are Rust crates that depend on the `engine` facade. An editor and a script
  language are planned, but a game must be completable without either.

## What would make Dimora compelling

For the intended audience, Dimora is a better choice than a general engine **only if** it demonstrates,
and keeps demonstrating, that:

1. A mixed scene (sprites, tiles, meshes, a skinned character, lights, shadows, UI) is the *easy* case:
   one world, one camera, no per-feature special handling. `spatial_scene` proves the first half; the
   M12 mixed-representation example is the real test.
2. A 2D-art game gets correct depth, lighting and camera-relative character facing at pixel quality,
   with small art budgets (normal maps and shadows for sprites, M14).
3. The path from prototype to a shipped desktop game is complete and documented (assets, scenes, bundles,
   packaging, crash logs), even if each piece is modest.
4. It stays small enough to understand: a developer can read the engine's core in days, and failures
   point to causes.

If these are not demonstrably true, a general-purpose engine is the better recommendation and the
project should say so.

## Explicit non-goals

Not pursued without a recorded, compelling reason (an ADR citing a concrete need):

* **A general-purpose 3D engine** (PBR pipelines, GI, large-world streaming, AAA rendering features).
  Real 3D is supported where a 2.5D game needs it.
* **A Unity or Godot clone**: no node-tree or scene-graph-as-API, no all-in-one editor suite, no asset
  store, no visual scripting.
* **Rigid-body physics solver, ragdolls, cloth, vehicles.** Kinematic collision and queries only (M7);
  a physics library would be an ADR.
* **Networking and multiplayer** (though determinism keeps the door open).
* **Mobile, web and console targets** before the first desktop release.
* **A custom scripting language** and a second scripting language.
* **A render graph or custom user shaders** for the initial implementation.
* **Complex-script text shaping and bidirectional text, accessibility tooling, localisation** in the first
  UI runtime (planned later, see CAPABILITIES.md).
* **Visual UI editor** before the code-driven UI runtime has proven stable.
* **Plugin or native-extension ABI** for third parties: the engine is a Rust library; a game extends it
  by depending on crates. See the scripting section of [CAPABILITIES.md](CAPABILITIES.md).

## Open product questions *(owner to confirm)*

1. Is "small team in Rust" the audience, or must a non-programmer be able to make a game (which would
   promote the editor and scripting)? The roadmap currently assumes the former with the latter later.
2. Is the first showcase target an "HD-2D" look (pixel sprites, perspective camera, lights and fog), a flat-lit paper look, or an isometric look? The engine supports the first two today; it
   shapes M5/M6/M14 priorities (pixel-art policy D-06, post-processing).
3. Licence and distribution: `MIT OR Apache-2.0` is declared with no licence files (KI-17); publishing to
   crates.io is deferred (D-23).
