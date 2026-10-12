# 2.5D engine

A Rust game engine framework for **spatial 2.5D games**: 2D sprites, skeletal 2D characters, tilemaps and real 3D
meshes living in *one* world, seen through *one* camera and shaded by *one* set of lights.

> **2.5D is a spatial world, not a fake 2D layer system.** Every object has a real `Vec3` position. Depth drives
> projection, parallax, occlusion and lighting. There is no integer z-index.

![Perspective camera](docs/perspective.png)

The scene above is `examples/basic_2_5d`: a tilemap lying on the ground, ~3000 grass tufts, sprite trees and rocks at
different depths, a skeletal player assembled from sprite parts, a 3D brick building and mountains, a spinning 3D
crate, three lights and distance fog. All of it is one world. Only the camera projection differs below:

| Perspective | Orthographic (same framing) |
|---|---|
| ![](docs/perspective.png) | ![](docs/orthographic.png) |

## The spatial proof: `spatial_scene`

![Sprites and meshes at different depths](docs/spatial_scene_occlusion.png)

One world, one camera: four identical figures at Z = 0, 100, 300 and 600, a pillar mesh between figures B and C (B is
in front of it, C is behind it), a cube mesh cutting through a tree sprite, a translucent sprite blended over the floor,
all unlit so only the projection shows. Perspective shrinks the figures with depth; orthographic does not. Moving the
camera produces parallax in perspective and a uniform shift in orthographic, with no per-object parallax value anywhere
(see [`docs/spatial_scene_parallax.png`](docs/spatial_scene_parallax.png)).

```sh
cargo run --release -p spatial_scene
```

| Key | Action |
|---|---|
| `W` / `S` | Move the camera in depth |
| `A` / `D` | Strafe |
| `Space` / `Shift` | Camera up / down |
| Arrow keys | Rotate the camera (yaw / pitch) |
| `P` | Perspective / orthographic |
| `O` | Cycle sprite orientation: billboard, upright, fixed |
| `R` / `Esc` | Reset camera / quit |

The rendering model (world vs render state, depth vs order vs position, orientation, conventions) is written up in
[`docs/SPATIAL_RENDERING.md`](docs/SPATIAL_RENDERING.md).

## Run the larger demo

`basic_2_5d` adds tilemaps, a skeletal character, lights and fog:

```sh
cargo run --release -p basic_2_5d
```

Needs a GPU with Vulkan, Metal or DX12. On a machine without one, a software Vulkan driver works
(`sudo apt install mesa-vulkan-drivers` on Debian/Ubuntu).

| Key | Action |
|---|---|
| `WASD` / arrows | Move the player (X and Z on the ground) |
| `P` | Toggle perspective / orthographic: watch parallax appear and disappear |
| `Q` / `E` | Tilt the camera |
| Mouse wheel | Zoom (camera distance) |
| `L` | Toggle the lantern (point light) |
| `Esc` | Quit |

The window title shows live stats (visible sprites/meshes, lights, draw calls, fps).

Headless screenshot (no window; used for the images in `docs/`):

```sh
cargo run --release -p basic_2_5d -- --screenshot out.png [--ortho] [--size 1280x720] [--distance 10]
```

## What a game looks like

```rust
use engine::prelude::*;

let mut game = Game::new(GameConfig::default().with_title("my game"));

// Assets are addressed by name; components can reference them before they exist.
game.assets().insert("ground", Texture::solid(16, 16, [90, 150, 70, 255]));

game.world.spawn((Transform::from_xyz(0.0, 3.0, -10.0), Camera::perspective(45.0)));
game.world.spawn((Transform::from_xyz(0.0, 0.0, 100.0), Sprite::new("player")));   // real depth: Z = 100
game.world.spawn((Transform::from_xyz(5.0, 0.0, 300.0), Mesh::new("tree")));       // a 3D mesh at Z = 300

game.add_system("gameplay", |world| { /* plain `FnMut(&mut World)` */ });
game.run().unwrap();
```

See `examples/basic_2_5d/src/main.rs` for a complete, working version of this.

## Coordinate system

Left-handed, **+Y up, +Z away from the default camera** (into the screen), +X right. "Z = 800" means far away. A camera
with identity rotation looks down +Z. Sprites are quads whose facing is chosen by `Sprite.orientation` (billboard by
default); a `Fixed` sprite lies in its local XY plane and faces -Z. Full conventions are in
[`docs/SPATIAL_RENDERING.md`](docs/SPATIAL_RENDERING.md).

## Crates

| Crate | Responsibility |
|---|---|
| `engine-math` | `Vec2/3/4`, `Quat`, `Mat3/4`, `Transform`, `Rect`. No dependencies. |
| `engine-core` | `Color`, `Time`, `FixedTimestep`, `Input`. No platform or GPU knowledge. |
| `engine-world` | ECS world (wraps [`hecs`](https://crates.io/crates/hecs)), resources, `Schedule`, transform hierarchy. |
| `engine-assets` | `AssetId`/`AssetKind`/`AssetHandle<T>`, `AssetManager`, sources, texture/sprite/mesh data. No renderer. |
| `engine-animation` | `Skeleton`, bones as hierarchy entities, clips, `AnimationPlayer`. |
| `engine-renderer` | Backend-agnostic render model: projection, camera, components, extraction, culling, sorting, batching. No GPU. |
| `engine-renderer-wgpu` | The [`wgpu`](https://wgpu.rs) backend. Consumes a `RenderScene`; never sees the ECS. |
| `engine-platform` | `winit` window/event loop to `InputEvent`s and frame time. |
| `engine` | Facade: `Game`, schedules, prelude. |

Design rationale, trade-offs (why `hecs`, why wgpu, handedness, ordering rules) and the frame pipeline are in
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Testing

```sh
cargo test --workspace
```

Math, hierarchy, assets, projection/parallax, culling, depth sorting and batching are covered by plain unit tests.
`crates/engine-renderer-wgpu/tests` renders small scenes offscreen and checks pixels (depth interleaving of sprites and
meshes, face culling, world-space lighting, draw-call counts). Those tests print a message and skip if no GPU adapter is
available.

## Status

Dimora is early. The foundation (math, ECS wrapper, transforms and hierarchy, asset manager, platform loop) and
the spatial renderer (perspective and orthographic cameras, depth-sorted sprites and meshes, sprite orientation
modes, forward lighting and fog, camera viewports) exist and are covered by tests. Basic tilemaps and a skeletal animation data model
exist but are limited (see ROADMAP.md). There are no particles, collision, audio, UI, shadows,
scene serialization, asset bundling or editor.

What is done, what is not, and what comes next is tracked with evidence in:

* [`ROADMAP.md`](ROADMAP.md): milestones with prerequisites, acceptance criteria and deferred work.
* [`TASKS.md`](TASKS.md): current milestone, known issues, blockers and the next three tasks.
* [`docs/DECISIONS.md`](docs/DECISIONS.md): open architecture decisions, when each is needed and what it blocks.
* [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md): the architecture as it exists, separated from what is proposed.
