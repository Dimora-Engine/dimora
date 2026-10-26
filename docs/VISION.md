# Product vision

Status: **Confirmed by the owner (2026-10-13):** the definition under "What Dimora is", the editor goal and
its timing (ADR-0003), and that Dimora supports many camera configurations and is not defined by one visual
style (ADR-0004). **Everything else was derived from the existing README, ROADMAP and ARCHITECTURE and from
reading the code (audit of 2026-10-13, commit `4857158`) and awaits owner confirmation;** items marked
*(owner to confirm)* are product choices the repository does not settle. The owner's decisions and open
questions are tracked in [DECISIONS.md](DECISIONS.md#owner-decisions).

Four kinds of statement appear in these documents and are never mixed: **confirmed owner decisions**,
**recommended but unapproved decisions**, **implemented capabilities** (only those with passing tests or a
recorded run), and **future requirements**. This file is intent; what exists is in
[CAPABILITIES.md](CAPABILITIES.md).

> **2.5D is a spatial world, not a fake 2D layer system.** Sprites, tilemaps, Sprite characters, skeletal 2D characters,
> 3D meshes and 3D characters live in one coherent world and share compatible transforms, camera
> behaviour, depth and resource infrastructure.

That does **not** mean every subsystem shares one implementation. A sprite batch, a skinned mesh and a UI
panel need different pipelines. What they share is the *world contract*: a real 3D position, one camera
model, one depth and ordering story, one lifetime model for resources.

## What Dimora is

Dimora is a Rust game engine built around a unified spatial model for 2.5D game development.

Dimora brings 2D sprites, tilemaps, skeletal 2D characters, 2D sprite characters, 3D meshes, and fully 3D characters into a shared world with coherent transforms, cameras, depth, and rendering.

Rather than treating 2.5D as a collection of layered 2D elements or a restricted form of 3D, Dimora makes
spatial interaction between different visual representations a foundational part of the engine.

Its goal is to make hybrid 2D/3D games natural to build through an integrated development experience
combining powerful programming APIs with purpose-built visual tools. Making hybrid 2D/3D games should feel natural and intuitive, rather than forcing developers to choose between 2D and 3D paradigms. The editor is designed to support this vision by providing a seamless interface for working with both 2D and 3D content. The goal is to make the development process as fluid and integrated as possible, allowing creators to focus on their game's design rather than the technical constraints of the engine in a user-friendly environment for developers and designers.

Building a game in Dimora means working with a consistent spatial model, regardless of whether the content is 2D or 3D.

**Typical games:** Isometric and top-down RPGs, side-scrollers, HD-2D adventures, paper-cutout and diorama worlds, 2D characters in 3D environments, and hybrid games combining 2D and 3D elements across orthographic, perspective, and custom camera configurations.

Dimora is not defined by a look or a camera: the same world can be seen through any of these configurations
(see "Camera configurations and visual styles").

**Programming APIs and visual tools are both first-class.** The editor is a product goal, not an optional
extra: a user-friendly editor for building spatial worlds, placing and arranging content, setting up
characters and previewing the result, so that building a game is not only writing code. It is built *after*
the foundations it stands on (scenes, the component registry, commands and undo, serialization, asset
metadata), but those foundations are designed for it from the start (principle 5). What exists today is a
code-driven engine with no editor; see [CAPABILITIES.md](CAPABILITIES.md).

## Problems it intends to solve

1. **Mixed 2D and 3D is a workaround elsewhere.** In general-purpose engines, 2D and 3D content usually
   run through separate renderers, camera types or layer systems, and putting a sprite *between* two
   meshes, or a 3D character in a tile world, takes effort and special cases. (This is our understanding of
   the field; it was not re-verified for this document.) Dimora makes it the default: a sprite at
   `Z = 300` is occluded by a mesh at `Z = 100` because of position, not because of an ordering number.
2. **Integer layers drift from the world.** A z-index cannot express "the camera orbits and the character
   must face the right way", parallax, shadows or collision at a different depth. A real `Vec3` can.
3. **Building a spatial game should not mean choosing between a code-only framework and a general-purpose
   toolchain.** Dimora aims to give developers powerful programming APIs and purpose-built visual tools
   over the same world, so a team can go from prototype to a shipped desktop game: assets in, game out,
   with logic in Rust (and later in a script language) and content arranged visually.
4. **Engines should be verifiable.** Rendering, animation, layout and simulation are tested on plain data
   and by pixel tests, and runs can be replayed deterministically. This is a design goal, partly
   achieved (see CAPABILITIES.md), not a marketing claim.

## Intended users and primary use cases

| User | What they need from Dimora |
|---|---|
| Solo developer or small team building a 2.5D game, writing gameplay in Rust or (later) scripts *(owner to confirm)* | Spatial sprite/mesh rendering, characters, tilemaps, collision, UI, audio, and a build they can ship |
| Artist, level designer or other creator who does not want to work only in code | An editor to build and arrange the spatial world, see it as the game will, and set up characters and content visually (target; M22 onward) |
| 2D-art game developer (pixel art or illustrated) who wants depth, lighting and a choice of camera without leaving 2D art | Sprite sheets and atlases that work under any camera, directional animation relative to the camera, normal-mapped sprites; pixel-exact output as an opt-in policy |
| Developer who wants to mix a few 3D props or characters into a 2D-art world | glTF meshes and skinned characters in the same depth buffer, lights and camera |
| Designer or scripter working with programmers | Scene files, hot reload, the editor, gameplay scripts |

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
* **Many views of one world.** Perspective, orthographic, isometric, top-down, side-scrolling and custom
  cameras are configurations of one camera model, not separate renderers or world representations.
* **Honest engineering.** Plans separate "exists and is tested" from "proposed", with evidence.

## Camera configurations and visual styles

*Confirmed direction (owner, 2026-10-13); mechanism decisions (presets as constructors, style as per-level policy, gameplay orientation as data, coplanar ordering amended) accepted 2026-10-13 and recorded in [ADR-0004](adr/0004-one-spatial-world-many-camera-configurations.md). The test scene demonstrates seven configurations (2026-10-14, plus `custom` in M6-1C). Public camera constructors and an oblique projection builder exist (2026-10-24). An opt-in orthographic view-translation snap is settled for that slice on `Camera.pixel_snap` (2026-10-25, [ADR-0022](adr/0022-orthographic-pixel-snap.md)). It is not integer-resolution output, and perspective and custom projections ignore it. Optional integer-resolution presentation is a separate camera field, settled for that contract in [ADR-0023](adr/0023-integer-resolution-camera-presentation.md) (2026-10-25). It does not close D-06. Optional 4× MSAA is partially implemented for a direct shaded surface camera and is not generally accepted ([ADR-0024](adr/0024-optional-4x-msaa.md), proposed). Per-object snapping is not implemented. Other style policies, and gameplay-orientation data, are not implemented. Coplanar sprite priority is implemented (ADR-0020, 2026-10-23). Tilemap extraction culls 32×32 chunks (2026-10-23); production tile editing, collision, and layers are still future work. Single-frame world-space debug lines exist (2026-10-24). A per-camera presentation of stored depth exists (2026-10-24). Gizmos and collision or lighting visualization are not implemented.*

Dimora's defining principle is the unified spatial world, not a look. It must support these views as
**intended capabilities**, not as a promise to build every one in the next milestone:

| View | How it is expressed | Status today |
|---|---|---|
| Perspective | `Perspective` projection, any camera placement | Implemented and tested |
| Orthographic | `Orthographic` projection, any camera placement | Implemented and tested |
| Isometric | Orthographic camera rotated by its transform; custom axes through a custom projection | **Public constructor** `PlacedCamera::isometric`. Not a projection type. The example still chooses the scene's height and distance |
| Top-down | Camera looking along the world's down axis | **Public constructor** `PlacedCamera::top_down`. `Upright` sprites are edge-on (ADR-0021). `Billboard` stays face-on. Coplanar priority is ADR-0020 |
| Side-scrolling | Camera looking along the depth axis | **Public constructor** `PlacedCamera::side`. Not demonstrated as a 2D-style game |
| Custom | A view-to-clip matrix supplied by game code | **Implemented** as `CameraProjection::Custom` (M6-1B, `a38cb30`, D-25). `oblique_orthographic` and `oblique_perspective` build that matrix (2026-10-24) and do not convert one already stored. The matrix stays fixed across resize |

Three things are kept separate, and none may be implemented as another: **camera projection** (how the world
becomes pixels), **gameplay orientation** (which way is up, where the ground plane is, what "facing" means),
and **visual style** (pixel snapping, fog, palette, lighting mood). An isometric camera does not need its
own world representation. The initial showcase is a reusable test scene (2D sprites, spatial depth and a
simple 3D mesh) viewed through different camera configurations, to demonstrate the shared architecture
without prescribing a visual identity; characters, lighting, shadows and tilemaps join it as their systems
exist. HD-2D-style features (pixel-art styling, fog) are valid per-game choices and are not prioritised above
foundations that every view needs.

## Design principles

When two options are close, these decide, in this order. Each has a test: a question to ask of a change.

1. **Spatial first.** Position in the world decides visibility, occlusion, parallax and collision. *Does
   this change add a layer or order number that a position could express instead?* Explicit ordering is
   allowed only as a narrow, documented override (`RenderOrder` for blended sprites today; deliberate ordering of
   coplanar content is supported as a separate override, never as a stand-in for depth, ADR-0004). *Does this assume one
   camera projection or one view?* If so, it is not spatial-first.
2. **One contract, many implementations.** Subsystems agree on transforms, camera, depth, handles,
   events and lifetimes, not on one code path. *Does this force two different things through one
   abstraction, or force one thing to duplicate a contract?*
3. **Data in, data out at every seam.** The world produces plain data (`RenderScene`, a future UI draw
   list, a pose, an input log); backends and tools consume it. *Can this be tested without a GPU, a
   window or a clock?*
4. **Gameplay does not know its pictures; tools and scripts do not know the renderer.** Dependencies
   point from presentation to simulation, never back. Scripts and the editor see a small, versioned
   surface, not internals.
5. **Built for tools as well as code.** What a programmer can do through the API, an editor must be able to
   do and show: components are plain, named, serializable data; changes go through commands that can be
   undone; assets and entities have stable identities; the editor's viewport is the real renderer. *Does
   this feature have a data shape an editor could inspect and edit, or does it hide state in code?* The
   editor itself is built later; the shapes are decided now.
6. **Foundations before features, and proof before abstraction.** Build the smallest working version of
   a feature end to end, with a test and an example, then generalise. Do not extract a shared
   abstraction until two real implementations show the same shape (D-29).
7. **Decide late, but decide before the data format or public API depends on it.** Data formats, ids and
   public APIs are expensive to change; implementations are not. Prototype to find out, then record
   (see [DECISIONS.md](DECISIONS.md)).
8. **Never silently wrong.** A missing asset, a bad file, a failed upload or a script error is reported
   with context and the engine keeps running where it safely can. Tests that skip because a GPU is
   absent must be able to fail (`DIMORA_REQUIRE_GPU`).
9. **Deterministic by default.** Same inputs, same state: fixed-step simulation, seeded randomness, no
   wall-clock in gameplay. This is what makes replay tests, soak tests and any later networking possible.
10. **Small surface, honest status.** Every public item is a promise. Prefer fewer, better-tested APIs.
   Docs never mark something done because a type for it exists.
11. **Performance is a budget, measured.** No optimisation without a measurement and no feature that
    cannot state its cost (M6 creates the baseline).

## Platforms and developer experience

* **Targets: Windows, macOS and Linux desktop** (wgpu over DX12/Vulkan, Metal, Vulkan; D-23). Web, mobile
  and consoles are out of scope until after the first release (M24). Verified so far: macOS arm64 (Metal)
  and Linux x86_64 (software Vulkan, lavapipe) by hand, and Windows x86_64 (DX12 software adapter), macOS and
  Linux by CI (ADR-0005; TASKS.md "CI results"). **No real-GPU, windowed-mode or DPI verification exists on any
  platform yet.**
* **Toolchain:** stable Rust, pinned in `rust-toolchain.toml`, MSRV 1.90.
* **Developer loop (target, not current):** `cargo run` opens a window within seconds; assets are loaded
  from files and hot-reload; errors name the asset and the problem; `scripts/check.sh` is the one
  command that says whether a checkout is healthy; headless screenshots and replays make rendering and
  gameplay regressions visible without a person looking.
* **Code and visual tools, one world.** Games are Rust crates that depend on the `engine` facade, with
  scripts planned for gameplay. The editor is the intended primary way to build worlds and set up content,
  and it works on the same scene files and components the code uses. Today only the code path exists. The
  runtime must still run without the editor present, and nothing may be possible *only* in the editor.

## What would make Dimora compelling

For the intended audience, Dimora is a better choice than a general engine **only if** it demonstrates,
and keeps demonstrating, that:

1. A mixed scene (sprites, tiles, meshes, a skinned character, lights, shadows, UI) is the *easy* case:
   one world, one camera, no per-feature special handling. `spatial_scene` proves the first half; the
   M12 mixed-representation example is the real test.
2. A 2D-art game gets correct depth, lighting and camera-relative character facing under the camera
   configuration it chooses, with small art budgets (normal maps and shadows for sprites, M14), and can
   change that configuration without rebuilding its content.
3. The path from prototype to a shipped desktop game is complete and documented (assets, scenes, bundles,
   packaging, crash logs), even if each piece is modest, and a user can build a mixed 2D/3D world in a
   friendly editor rather than only in code. Few engines make that visual path spatial-first.
4. It stays small enough to understand: a developer can read the engine's core in days, and failures
   point to causes.

If these are not demonstrably true, a general-purpose engine is the better recommendation and the
project should say so.

## Explicit non-goals

Not pursued without a recorded, compelling reason (an ADR citing a concrete need):

* **A general-purpose 3D engine** (PBR pipelines, GI, large-world streaming, AAA rendering features).
  Real 3D is supported where a 2.5D game needs it.
* **A Unity or Godot clone**: no node-tree or scene-graph-as-API, no asset store, no visual scripting.
  The editor is purpose-built for the spatial model and for the games Dimora targets, not a generic editor
  for every kind of game.
* **Rigid-body physics solver, ragdolls, cloth, vehicles.** Kinematic collision and queries only (M7);
  a physics library would be an ADR.
* **Networking and multiplayer** (though determinism keeps the door open).
* **Mobile, web and console targets** before the first desktop release.
* **A custom scripting language** and a second scripting language.
* **A render graph or custom user shaders** for the initial implementation.
* **A separate renderer, mode or world model per camera configuration**, and defining the engine by one
  visual style (such as HD-2D) or one perspective.
* **Complex-script text shaping and bidirectional text, accessibility tooling, localisation** in the first
  UI runtime (planned later, see CAPABILITIES.md).
* **A visual editor for game UI** before the code-driven UI runtime has proven stable. (This is a sequencing
  rule for one tool, not a non-goal for the editor: the world editor is a goal.)
* **Plugin or native-extension ABI** for third parties: the engine is a Rust library; a game extends it
  by depending on crates. See the scripting section of [CAPABILITIES.md](CAPABILITIES.md).

## Open product questions *(owner to confirm)*

1. **Confirmed direction (2026-10-13):** the editor and a user-friendly experience are goals, so the
   audience is not only programmers, and the full editor stays late behind a stability gate with developer
   tooling arriving incrementally (ADR-0003). Still open (OD-6): how much of a game must be buildable
   without writing Rust.
2. **Settled (2026-10-13):** there is no single showcase look or camera; see "Camera configurations and
   visual styles". Engineering follow-up: D-48 (the `+Y` up / `XZ` ground convention is provisional; coplanar ordering and sprite orientation per view wait for the M6 test scene).
3. **Licence: open (OD-3).** `MIT OR Apache-2.0` is *proposed* and declared in `Cargo.toml`, but the owner
   has not confirmed it and no licence files exist (KI-17; audit in TASKS.md). Publishing to crates.io is
   deferred (D-23).
