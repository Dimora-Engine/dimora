# ADR-0018: Procedural mesh generators

* **Status:** Settled
* **Date:** 2026-10-21
* **Backlog id:** No new decision. The M5 task list already required plane, sphere, and cylinder generators. This record adds the cube/cuboid contract and the validation limits.
* **Decided by:** M5-6 implementation task
* **Affects:** `MeshData` in `engine-assets`. Rendering, materials, and batching are unchanged.

## Context and problem

`MeshData::cuboid` and `MeshData::cone` already build CPU meshes. M5 still needed a plane, a sphere, and a cylinder, and the box helper did not reject a zero or non-finite size. A second mesh type, or a primitive ECS component, would give those shapes a private renderer.

## Requirements and constraints

Generated geometry is ordinary `MeshData`. It uses the existing asset store, `Mesh` component, extractor, GPU upload, material, and batch key. Y is up, X is width, Z is depth, and the world is left-handed. Front faces match the cuboid winding the renderer already culls with `FrontFace::Ccw`. Generators do not create transforms, entities, collision shapes, or GPU objects. Invalid input returns `MeshError` instead of a degenerate mesh.

## Options considered

### A. Functions on `MeshData`
* Advantages: one vertex format, one upload path, one batch key. `cube` can call `cuboid`.
* Disadvantages: `cuboid` changes from an infallible function to `Result`.

### B. A primitive component or a second mesh format
* Advantages: the component could remember the generator arguments.
* Disadvantages: a second representation, and a renderer branch the rest of M5 does not need.

## Decision and rationale

Option A.

* `plane(width, depth)` is a 4-vertex XZ quad, normal +Y, centred on the origin. That is the horizontal ground.
* `cuboid(size)` is the existing 24-vertex, 36-index box. X, Y, and Z are width, height, and depth. The box is centred. Each face has four vertices, one outward normal, and a `0..1` UV square. Normals are not averaged across corners.
* `cube(size)` is `cuboid(Vec3::splat(size))`. The vertices match.
* `sphere(radius, segments, rings)` is a UV sphere. `segments` is the count around Y (minimum 3). `rings` is the number of triangle bands from pole to pole (minimum 2). Poles are one vertex per sector, so a pole triangle is not a zero-area quad. Each middle ring repeats its first vertex at `u = 1`. Normals are radial. Counts are `2 * segments + (rings - 1) * (segments + 1)` vertices and `6 * segments * (rings - 1)` indices.
* `cylinder(radius, height, segments)` is centred on Y and includes caps. `cylinder_with_caps(..., false)` omits them. The side uses radial normals and a seam vertex. Caps use axial normals and their own vertices. Minimum 3 segments.
* `cone` stays the earlier helper. Its base is on `y = 0`, and it still asserts.
* A length must be finite and strictly positive, and half of it must stay finite and non-zero. A face product must be finite and at least `MeshData::MIN_FACE_PRODUCT` (`1e-12`). More than `MeshData::MAX_GENERATED_VERTICES` (`65_536`) vertices is rejected before allocation, as is a count that overflows.

A generated mesh is not a collider and has no per-face material. One asset uses one `Mesh` material binding. A blended mesh still sorts by the bounding-sphere centre, which is only an approximation when a large transparent box overlaps other transparent geometry. That limit belongs to ADR-0015, not to the generator.

## Consequences and follow-up

* M5-6 is the working tree. M5 is not accepted as a milestone. Localized mip regeneration is still absent. M6 and D-48 are unchanged.
* Call sites of `cuboid` handle `Result`. The vertex order of a valid box is the order the previous function built.
* A later loader or skinned mesh uses `MeshData`. It does not add a primitive pipeline.

## Evidence and revisit conditions

CPU tests in `engine-assets` check counts, winding, UVs, bounds, and rejected input. GPU readback in `engine-renderer-wgpu` `primitives.rs` checks visibility, culling, lighting, materials, and batching. Reopen this only if a generator must share one draw across different geometry, which the current batch key does not do.
