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

## Run it

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
with identity rotation looks down +Z. Sprites lie in their local XY plane and face -Z.

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

The first milestone is complete: a small Rust program can create a spatial 2.5D world and render sprites, tilemaps,
a skeletal character and basic 3D geometry with a camera, lights and fog.

Interfaces exist but are not implemented yet: shadows, normal maps, animation blending / IK / constraints, asset
bundles and asynchronous loading / hot reload (the manager has load states, revisions and a swappable source),
billboarding, particles, collision, UI, hierarchical visibility, mipmaps. See the end of `docs/ARCHITECTURE.md`.
