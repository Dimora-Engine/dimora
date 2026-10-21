//! CPU-side mesh data and procedural generators.
//!
//! Generators build ordinary [`MeshData`] in local space. They do not create entities, transforms,
//! materials, or GPU objects. [`MeshData::cube`] is [`MeshData::cuboid`] with three equal sizes.
//! [`MeshData::cone`] is the earlier helper and still asserts; the generators below return
//! [`MeshError`] instead.
//!
//! # Space
//!
//! Y is up. X is width. Z is depth, and +Z points away from the default camera. The world is
//! left-handed. A generated surface is centred on the local origin except the cone, whose base
//! stays on `y = 0`. The plane lies in the horizontal XZ plane with its normal along +Y, so it is
//! a ground surface.
//!
//! Front faces use the same winding as the existing cuboid: the cross `(c - a) × (b - a)` points
//! the same way as the outward normal. That is the winding the renderer keeps with back-face
//! culling. Each cuboid face has its own four vertices, one constant outward normal, and a `0..1`
//! UV rectangle. U is 0 at the first corner of that face and V is 0 toward +Y on a side face.
//! Sphere and cylinder sides use smooth outward normals. Cylinder caps keep a separate axial
//! normal and are not welded to the side.
//!
//! # Limits
//!
//! A length must be finite and strictly positive, and half of it must stay finite and non-zero.
//! Every product of two lengths that form a face must be finite and at least
//! [`MeshData::MIN_FACE_PRODUCT`]. A sphere asks for at least 3 longitudinal segments and 2
//! latitudinal rings. A cylinder asks for at least 3 radial segments. [`MeshData::cylinder`]
//! includes both caps. [`MeshData::cylinder_with_caps`] can omit them. Vertex counts above
//! [`MeshData::MAX_GENERATED_VERTICES`], and index counts that do not fit in `u32`, are rejected
//! before the mesh is allocated.

use core::f32::consts::TAU;

use bytemuck::{Pod, Zeroable};
use engine_math::Vec3;

use crate::{Asset, AssetKind};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

impl Vertex {
    pub fn new(position: Vec3, normal: Vec3, uv: [f32; 2]) -> Self {
        Self { position: position.to_array(), normal: normal.to_array(), uv }
    }
}

/// Why a procedural mesh generator refused its arguments.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeshError {
    /// The length was NaN or infinite.
    DimensionNotFinite { axis: &'static str },
    /// The length was finite and not strictly positive.
    DimensionNotPositive { axis: &'static str, value: f32 },
    /// Half of the length is zero or not finite, so the vertices would collapse.
    DimensionNotRepresentable { axis: &'static str, value: f32 },
    /// The product of two face edges is below [`MeshData::MIN_FACE_PRODUCT`] or is not finite.
    FaceNotRepresentable { first: &'static str, second: &'static str },
    /// A radial or longitudinal division count is below the minimum that can form a surface.
    TooFewSegments { segments: u32, minimum: u32 },
    /// A sphere has fewer than two latitudinal bands.
    TooFewRings { rings: u32, minimum: u32 },
    /// The vertex count would exceed [`MeshData::MAX_GENERATED_VERTICES`].
    TooManyVertices { vertices: u64 },
    /// The index count does not fit in `u32`.
    IndexCountOverflow,
    /// A vertex or index count overflowed before the limit check.
    CountOverflow,
}

impl core::fmt::Display for MeshError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MeshError::DimensionNotFinite { axis } => write!(f, "{axis} must be finite"),
            MeshError::DimensionNotPositive { axis, value } => {
                write!(f, "{axis} must be greater than 0, got {value}")
            }
            MeshError::DimensionNotRepresentable { axis, value } => {
                write!(f, "{axis} {value} is too small or too large to place vertices")
            }
            MeshError::FaceNotRepresentable { first, second } => {
                write!(f, "the {first} by {second} face is not representable")
            }
            MeshError::TooFewSegments { segments, minimum } => {
                write!(f, "{segments} segments are below the minimum of {minimum}")
            }
            MeshError::TooFewRings { rings, minimum } => {
                write!(f, "{rings} rings are below the minimum of {minimum}")
            }
            MeshError::TooManyVertices { vertices } => {
                write!(f, "{vertices} vertices exceeds the generator limit of {}", MeshData::MAX_GENERATED_VERTICES)
            }
            MeshError::IndexCountOverflow => write!(f, "index count does not fit in u32"),
            MeshError::CountOverflow => write!(f, "vertex or index count overflowed"),
        }
    }
}

impl std::error::Error for MeshError {}

/// Indexed triangle list with a precomputed bounding sphere (used for culling).
#[derive(Clone, Debug)]
pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    bounds_center: Vec3,
    bounds_radius: f32,
}

impl MeshData {
    /// Largest vertex count a generator will allocate.
    pub const MAX_GENERATED_VERTICES: u32 = 65_536;
    /// Smallest finite product of two face edges a generator will accept.
    pub const MIN_FACE_PRODUCT: f32 = 1.0e-12;
    const MIN_SEGMENTS: u32 = 3;
    const MIN_RINGS: u32 = 2;

    pub fn new(vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        assert!(indices.len().is_multiple_of(3), "indices must form triangles");
        assert!(indices.iter().all(|&i| (i as usize) < vertices.len()), "index out of range");
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for v in &vertices {
            lo = lo.min(v.position.into());
            hi = hi.max(v.position.into());
        }
        let center = if vertices.is_empty() { Vec3::ZERO } else { (lo + hi) * 0.5 };
        let radius = vertices.iter().map(|v| Vec3::from(v.position).distance(center)).fold(0.0, f32::max);
        Self { vertices, indices, bounds_center: center, bounds_radius: radius }
    }

    /// Bounding sphere in local space as `(center, radius)`.
    pub fn bounding_sphere(&self) -> (Vec3, f32) {
        (self.bounds_center, self.bounds_radius)
    }

    /// Horizontal rectangle in the XZ plane, normal +Y, centred on the origin.
    ///
    /// `width` is the X size and `depth` is the Z size. The mesh is 4 vertices and 6 indices.
    /// Its UV rectangle matches the top face of [`Self::cuboid`].
    pub fn plane(width: f32, depth: f32) -> Result<Self, MeshError> {
        require_length("width", width)?;
        require_length("depth", depth)?;
        require_face("width", "depth", width, depth)?;
        let (hx, hz) = (width * 0.5, depth * 0.5);
        let n = Vec3::Y;
        let vertices = vec![
            Vertex::new(Vec3::new(-hx, 0.0, hz), n, [0.0, 1.0]),
            Vertex::new(Vec3::new(hx, 0.0, hz), n, [1.0, 1.0]),
            Vertex::new(Vec3::new(hx, 0.0, -hz), n, [1.0, 0.0]),
            Vertex::new(Vec3::new(-hx, 0.0, -hz), n, [0.0, 0.0]),
        ];
        Ok(Self::new(vertices, vec![0, 2, 1, 0, 3, 2]))
    }

    /// Cube centred on the origin. This is [`Self::cuboid`] with `size` on every axis, so the
    /// vertices, normals, UVs, and winding match `cuboid(Vec3::splat(size))`.
    pub fn cube(size: f32) -> Result<Self, MeshError> {
        Self::cuboid(Vec3::splat(size))
    }

    /// Box centred on the origin. X is width, Y is height, and Z is depth.
    ///
    /// Six faces, four vertices each (24), two triangles each (36 indices). Normals are flat and
    /// point outward. Each face has its own `0..1` UV square. A non-positive, non-finite, or
    /// non-representable component is an error.
    pub fn cuboid(size: Vec3) -> Result<Self, MeshError> {
        require_length("width", size.x)?;
        require_length("height", size.y)?;
        require_length("depth", size.z)?;
        require_face("width", "height", size.x, size.y)?;
        require_face("height", "depth", size.y, size.z)?;
        require_face("width", "depth", size.x, size.z)?;
        Ok(Self::cuboid_geometry(size))
    }

    fn cuboid_geometry(size: Vec3) -> Self {
        let h = size * 0.5;
        // (normal, tangent u, tangent v)
        let faces = [
            (Vec3::X, Vec3::Z * -1.0, Vec3::Y),
            (-Vec3::X, Vec3::Z, Vec3::Y),
            (Vec3::Y, Vec3::X, Vec3::Z * -1.0),
            (-Vec3::Y, Vec3::X, Vec3::Z),
            (Vec3::Z, Vec3::X, Vec3::Y),
            (-Vec3::Z, Vec3::X * -1.0, Vec3::Y),
        ];
        let (mut vertices, mut indices) = (Vec::new(), Vec::new());
        for (n, u, v) in faces {
            let base = vertices.len() as u32;
            for (su, sv, uv) in
                [(-1.0, -1.0, [0.0, 1.0]), (1.0, -1.0, [1.0, 1.0]), (1.0, 1.0, [1.0, 0.0]), (-1.0, 1.0, [0.0, 0.0])]
            {
                let p = (n + u * su + v * sv) * h;
                vertices.push(Vertex::new(p, n, uv));
            }
            // Wound so the front face is clockwise when seen from outside (left-handed).
            indices.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
        }
        Self::new(vertices, indices)
    }

    /// UV sphere centred on the origin.
    ///
    /// `segments` is the number of divisions around Y and must be at least 3. `rings` is the
    /// number of triangle bands from the north pole to the south pole and must be at least 2.
    /// Poles are one vertex per sector, so a pole triangle is not a zero-area quad. Each
    /// intermediate ring duplicates its first vertex at `u = 1` for the longitude seam. Normals
    /// are the radial direction. Counts are `2 * segments + (rings - 1) * (segments + 1)` vertices
    /// and `6 * segments * (rings - 1)` indices.
    pub fn sphere(radius: f32, segments: u32, rings: u32) -> Result<Self, MeshError> {
        require_length("radius", radius)?;
        require_face("radius", "radius", radius, radius)?;
        require_segments(segments)?;
        if rings < Self::MIN_RINGS {
            return Err(MeshError::TooFewRings { rings, minimum: Self::MIN_RINGS });
        }
        let (vertex_count, index_count) = sphere_counts(segments, rings)?;
        let mut vertices = Vec::with_capacity(vertex_count as usize);
        let mut indices = Vec::with_capacity(index_count as usize);

        for i in 0..segments {
            let u = i as f32 / segments as f32;
            vertices.push(Vertex::new(Vec3::new(0.0, radius, 0.0), Vec3::Y, [u, 0.0]));
        }

        let mut row_start = Vec::with_capacity(rings as usize - 1);
        for row in 1..rings {
            row_start.push(vertices.len() as u32);
            let v = row as f32 / rings as f32;
            let phi = core::f32::consts::PI * v;
            let (sin_phi, cos_phi) = phi.sin_cos();
            let row = vertices.len();
            for i in 0..=segments {
                let u = i as f32 / segments as f32;
                if i == segments {
                    let mut seam = vertices[row];
                    seam.uv[0] = 1.0;
                    vertices.push(seam);
                    continue;
                }
                let (sin_theta, cos_theta) = (u * TAU).sin_cos();
                let normal = Vec3::new(cos_theta * sin_phi, cos_phi, sin_theta * sin_phi);
                vertices.push(Vertex::new(normal * radius, normal, [u, v]));
            }
        }

        let south = vertices.len() as u32;
        for i in 0..segments {
            let u = i as f32 / segments as f32;
            vertices.push(Vertex::new(Vec3::new(0.0, -radius, 0.0), -Vec3::Y, [u, 1.0]));
        }

        let north_ring = row_start[0];
        for i in 0..segments {
            indices.extend([i, north_ring + i, north_ring + i + 1]);
        }
        for band in 0..row_start.len() - 1 {
            let top = row_start[band];
            let bottom = row_start[band + 1];
            for i in 0..segments {
                let (t0, t1) = (top + i, top + i + 1);
                let (b0, b1) = (bottom + i, bottom + i + 1);
                indices.extend([t0, b1, t1, t0, b0, b1]);
            }
        }
        let south_ring = *row_start.last().expect("rings >= 2 produces a middle row");
        for i in 0..segments {
            indices.extend([south + i, south_ring + i + 1, south_ring + i]);
        }

        debug_assert_eq!(vertices.len(), vertex_count as usize);
        debug_assert_eq!(indices.len(), index_count as usize);
        Ok(Self::new(vertices, indices))
    }

    /// Cylinder centred on the origin, aligned with Y, with top and bottom caps.
    ///
    /// This is [`Self::cylinder_with_caps`] with `caps` set. The side uses radial normals and a
    /// seam vertex at `u = 1`. Caps use axial normals and their own vertices.
    pub fn cylinder(radius: f32, height: f32, segments: u32) -> Result<Self, MeshError> {
        Self::cylinder_with_caps(radius, height, segments, true)
    }

    /// [`Self::cylinder`] with an explicit cap choice. `caps == false` keeps only the side.
    pub fn cylinder_with_caps(radius: f32, height: f32, segments: u32, caps: bool) -> Result<Self, MeshError> {
        require_length("radius", radius)?;
        require_length("height", height)?;
        require_face("radius", "radius", radius, radius)?;
        require_face("radius", "height", radius, height)?;
        require_segments(segments)?;
        let (vertex_count, index_count) = cylinder_counts(segments, caps)?;
        let mut vertices: Vec<Vertex> = Vec::with_capacity(vertex_count as usize);
        let mut indices = Vec::with_capacity(index_count as usize);
        let half = height * 0.5;

        for i in 0..=segments {
            let u = i as f32 / segments as f32;
            if i == segments {
                let mut top = vertices[0];
                let mut bottom = vertices[1];
                top.uv[0] = 1.0;
                bottom.uv[0] = 1.0;
                vertices.push(top);
                vertices.push(bottom);
                continue;
            }
            let (sin_theta, cos_theta) = (u * TAU).sin_cos();
            let normal = Vec3::new(cos_theta, 0.0, sin_theta);
            let x = cos_theta * radius;
            let z = sin_theta * radius;
            vertices.push(Vertex::new(Vec3::new(x, half, z), normal, [u, 0.0]));
            vertices.push(Vertex::new(Vec3::new(x, -half, z), normal, [u, 1.0]));
        }
        for i in 0..segments {
            let base = i * 2;
            indices.extend([base, base + 3, base + 2, base, base + 1, base + 3]);
        }

        if caps {
            let top_center = vertices.len() as u32;
            vertices.push(Vertex::new(Vec3::new(0.0, half, 0.0), Vec3::Y, [0.5, 0.5]));
            for i in 0..segments {
                let theta = i as f32 / segments as f32 * TAU;
                let (sin_theta, cos_theta) = theta.sin_cos();
                vertices.push(Vertex::new(
                    Vec3::new(cos_theta * radius, half, sin_theta * radius),
                    Vec3::Y,
                    [0.5 + cos_theta * 0.5, 0.5 + sin_theta * 0.5],
                ));
            }
            let top_rim = top_center + 1;
            for i in 0..segments {
                let next = if i + 1 == segments { top_rim } else { top_rim + i + 1 };
                indices.extend([top_center, top_rim + i, next]);
            }

            let bottom_center = vertices.len() as u32;
            vertices.push(Vertex::new(Vec3::new(0.0, -half, 0.0), -Vec3::Y, [0.5, 0.5]));
            for i in 0..segments {
                let theta = i as f32 / segments as f32 * TAU;
                let (sin_theta, cos_theta) = theta.sin_cos();
                vertices.push(Vertex::new(
                    Vec3::new(cos_theta * radius, -half, sin_theta * radius),
                    -Vec3::Y,
                    [0.5 + cos_theta * 0.5, 0.5 + sin_theta * 0.5],
                ));
            }
            let bottom_rim = bottom_center + 1;
            for i in 0..segments {
                let next = if i + 1 == segments { bottom_rim } else { bottom_rim + i + 1 };
                indices.extend([bottom_center, next, bottom_rim + i]);
            }
        }

        debug_assert_eq!(vertices.len(), vertex_count as usize);
        debug_assert_eq!(indices.len(), index_count as usize);
        Ok(Self::new(vertices, indices))
    }

    /// Cone with its base on `y = 0` and apex at `y = height`, smooth side normals.
    pub fn cone(radius: f32, height: f32, segments: u32) -> Self {
        assert!(segments >= 3);
        let (mut vertices, mut indices) = (Vec::new(), Vec::new());
        let slope = radius / height;
        for i in 0..=segments {
            let a = i as f32 / segments as f32 * TAU;
            let (s, c) = a.sin_cos();
            let n = Vec3::new(c, slope, s).normalized();
            let u = i as f32 / segments as f32;
            vertices.push(Vertex::new(Vec3::new(c * radius, 0.0, s * radius), n, [u, 1.0]));
            vertices.push(Vertex::new(Vec3::new(0.0, height, 0.0), n, [u, 0.0]));
        }
        for i in 0..segments {
            let b = i * 2;
            indices.extend([b, b + 2, b + 1]);
        }
        let cap = vertices.len() as u32;
        vertices.push(Vertex::new(Vec3::ZERO, -Vec3::Y, [0.5, 0.5]));
        for i in 0..=segments {
            let a = i as f32 / segments as f32 * TAU;
            let (s, c) = a.sin_cos();
            vertices.push(Vertex::new(
                Vec3::new(c * radius, 0.0, s * radius),
                -Vec3::Y,
                [0.5 + c * 0.5, 0.5 + s * 0.5],
            ));
        }
        for i in 0..segments {
            indices.extend([cap, cap + 2 + i, cap + 1 + i]);
        }
        Self::new(vertices, indices)
    }
}

fn require_length(axis: &'static str, value: f32) -> Result<(), MeshError> {
    if !value.is_finite() {
        return Err(MeshError::DimensionNotFinite { axis });
    }
    if value <= 0.0 {
        return Err(MeshError::DimensionNotPositive { axis, value });
    }
    let half = value * 0.5;
    if !half.is_finite() || half == 0.0 {
        return Err(MeshError::DimensionNotRepresentable { axis, value });
    }
    Ok(())
}

fn require_face(first: &'static str, second: &'static str, a: f32, b: f32) -> Result<(), MeshError> {
    let product = a * b;
    if !product.is_finite() || product < MeshData::MIN_FACE_PRODUCT {
        return Err(MeshError::FaceNotRepresentable { first, second });
    }
    Ok(())
}

fn require_segments(segments: u32) -> Result<(), MeshError> {
    if segments < MeshData::MIN_SEGMENTS {
        return Err(MeshError::TooFewSegments { segments, minimum: MeshData::MIN_SEGMENTS });
    }
    Ok(())
}

fn checked_total(vertices: u64, indices: u64) -> Result<(u32, u32), MeshError> {
    if vertices > u64::from(MeshData::MAX_GENERATED_VERTICES) {
        return Err(MeshError::TooManyVertices { vertices });
    }
    let vertices = u32::try_from(vertices).map_err(|_| MeshError::CountOverflow)?;
    let indices = u32::try_from(indices).map_err(|_| MeshError::IndexCountOverflow)?;
    Ok((vertices, indices))
}

fn sphere_counts(segments: u32, rings: u32) -> Result<(u32, u32), MeshError> {
    let segments = u64::from(segments);
    let middle_rows = u64::from(rings) - 1;
    let row = segments.checked_add(1).ok_or(MeshError::CountOverflow)?;
    let middle = middle_rows.checked_mul(row).ok_or(MeshError::CountOverflow)?;
    let vertices =
        segments.checked_mul(2).ok_or(MeshError::CountOverflow)?.checked_add(middle).ok_or(MeshError::CountOverflow)?;
    let indices = segments
        .checked_mul(6)
        .ok_or(MeshError::CountOverflow)?
        .checked_mul(middle_rows)
        .ok_or(MeshError::CountOverflow)?;
    checked_total(vertices, indices)
}

fn cylinder_counts(segments: u32, caps: bool) -> Result<(u32, u32), MeshError> {
    let segments = u64::from(segments);
    let columns = segments.checked_add(1).ok_or(MeshError::CountOverflow)?;
    let mut vertices = columns.checked_mul(2).ok_or(MeshError::CountOverflow)?;
    let mut indices = segments.checked_mul(6).ok_or(MeshError::CountOverflow)?;
    if caps {
        let cap_vertices =
            segments.checked_add(1).ok_or(MeshError::CountOverflow)?.checked_mul(2).ok_or(MeshError::CountOverflow)?;
        vertices = vertices.checked_add(cap_vertices).ok_or(MeshError::CountOverflow)?;
        indices = indices
            .checked_add(segments.checked_mul(6).ok_or(MeshError::CountOverflow)?)
            .ok_or(MeshError::CountOverflow)?;
    }
    checked_total(vertices, indices)
}

impl Asset for MeshData {
    const KIND: AssetKind = AssetKind::Mesh;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_normals_match_winding(m: &MeshData) {
        for t in m.indices.chunks(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| Vec3::from(m.vertices[i as usize].position));
            let g = (c - a).cross(b - a);
            assert!(g.length() > 1.0e-6, "triangle {t:?} is degenerate");
            let n = Vec3::from(m.vertices[t[0] as usize].normal);
            assert!(g.normalized().dot(n) > 0.0, "triangle {t:?} winds against its normal");
        }
    }

    fn assert_valid(m: &MeshData) {
        assert!(m.indices.len().is_multiple_of(3));
        assert!(m.indices.iter().all(|&i| (i as usize) < m.vertices.len()));
        for v in &m.vertices {
            assert!(v.position.iter().all(|c| c.is_finite()));
            assert!(v.normal.iter().all(|c| c.is_finite()));
            assert!(v.uv.iter().all(|c| c.is_finite()));
            let length = Vec3::from(v.normal).length();
            assert!((length - 1.0).abs() < 1.0e-4, "normal length {length}");
        }
        check_normals_match_winding(m);
    }

    fn same_geometry(a: &MeshData, b: &MeshData) {
        assert_eq!(a.vertices, b.vertices);
        assert_eq!(a.indices, b.indices);
        let (ac, ar) = a.bounding_sphere();
        let (bc, br) = b.bounding_sphere();
        assert!(ac.approx_eq(bc, 1e-6));
        assert!((ar - br).abs() < 1e-6);
    }

    #[test]
    fn plane_is_a_centred_horizontal_quad() {
        let m = MeshData::plane(4.0, 2.0).unwrap();
        assert_eq!((m.vertices.len(), m.indices.len()), (4, 6));
        assert_valid(&m);
        assert!(m.vertices.iter().all(|v| v.position[1] == 0.0 && v.normal == [0.0, 1.0, 0.0]));
        let xs: Vec<_> = m.vertices.iter().map(|v| v.position[0]).collect();
        let zs: Vec<_> = m.vertices.iter().map(|v| v.position[2]).collect();
        assert!(xs.iter().all(|x| x.abs() == 2.0));
        assert!(zs.iter().all(|z| z.abs() == 1.0));
        let uvs: Vec<_> = m.vertices.iter().map(|v| v.uv).collect();
        assert_eq!(uvs, vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
        let (c, r) = m.bounding_sphere();
        assert!(c.approx_eq(Vec3::ZERO, 1e-6));
        assert!((r - Vec3::new(2.0, 0.0, 1.0).length()).abs() < 1e-5);
        same_geometry(&m, &MeshData::plane(4.0, 2.0).unwrap());
    }

    #[test]
    fn plane_rejects_bad_dimensions() {
        assert!(matches!(MeshData::plane(0.0, 1.0), Err(MeshError::DimensionNotPositive { axis: "width", .. })));
        assert!(matches!(MeshData::plane(1.0, -2.0), Err(MeshError::DimensionNotPositive { axis: "depth", .. })));
        assert!(matches!(MeshData::plane(f32::NAN, 1.0), Err(MeshError::DimensionNotFinite { axis: "width" })));
        assert!(matches!(MeshData::plane(1.0, f32::INFINITY), Err(MeshError::DimensionNotFinite { axis: "depth" })));
        assert!(matches!(MeshData::plane(1.0e-20, 1.0e-20), Err(MeshError::FaceNotRepresentable { .. })));
    }

    #[test]
    fn cube_matches_an_equal_cuboid() {
        let cube = MeshData::cube(2.0).unwrap();
        let cuboid = MeshData::cuboid(Vec3::splat(2.0)).unwrap();
        same_geometry(&cube, &cuboid);
        assert_eq!((cube.vertices.len(), cube.indices.len()), (24, 36));
        assert_valid(&cube);
        let (c, r) = cube.bounding_sphere();
        assert!(c.approx_eq(Vec3::ZERO, 1e-6));
        assert!((r - Vec3::splat(1.0).length()).abs() < 1e-5);
        assert_eq!(MeshData::cube(0.0).unwrap_err(), MeshData::cuboid(Vec3::ZERO).unwrap_err());
        assert_eq!(MeshData::cube(-1.0).unwrap_err(), MeshData::cuboid(Vec3::splat(-1.0)).unwrap_err());
        assert_eq!(MeshData::cube(f32::NAN).unwrap_err(), MeshData::cuboid(Vec3::splat(f32::NAN)).unwrap_err());
    }

    #[test]
    fn cuboid_has_six_flat_outward_faces() {
        let m = MeshData::cuboid(Vec3::new(2.0, 4.0, 6.0)).unwrap();
        assert_eq!((m.vertices.len(), m.indices.len()), (24, 36));
        assert_valid(&m);
        same_geometry(&m, &MeshData::cuboid(Vec3::new(2.0, 4.0, 6.0)).unwrap());
        let (c, r) = m.bounding_sphere();
        assert!(c.approx_eq(Vec3::ZERO, 1e-6));
        assert!((r - Vec3::new(1.0, 2.0, 3.0).length()).abs() < 1e-5);
        for face in m.vertices.chunks(4) {
            assert!(face.windows(2).all(|w| w[0].normal == w[1].normal));
            let uvs: Vec<_> = face.iter().map(|v| v.uv).collect();
            assert_eq!(uvs, vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
        }
        let normals: Vec<_> = m.vertices.iter().map(|v| v.normal).collect();
        for expected in
            [[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, -1.0]]
        {
            assert_eq!(normals.iter().filter(|n| **n == expected).count(), 4);
        }
        assert!(m
            .vertices
            .iter()
            .all(|v| { v.position[0].abs() == 1.0 && v.position[1].abs() == 2.0 && v.position[2].abs() == 3.0 }));
    }

    #[test]
    fn cuboid_rejects_each_bad_dimension() {
        let cases = [
            Vec3::new(0.0, 1.0, 1.0),
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(-1.0, 1.0, 1.0),
            Vec3::new(1.0, f32::NAN, 1.0),
            Vec3::new(1.0, 1.0, f32::NEG_INFINITY),
        ];
        for size in cases {
            assert!(MeshData::cuboid(size).is_err(), "{size:?}");
        }
        assert!(matches!(MeshData::cuboid(Vec3::new(1.0e-20, 1.0, 1.0)), Err(MeshError::FaceNotRepresentable { .. })));
        assert!(MeshData::cuboid(Vec3::new(1.0e-3, 2.0e-3, 3.0e-3)).is_ok());
        assert!(MeshData::cuboid(Vec3::new(1.0e5, 2.0e5, 3.0e5)).is_ok());
        assert!(matches!(
            MeshData::cuboid(Vec3::splat(1.0e30)),
            Err(MeshError::FaceNotRepresentable { .. } | MeshError::DimensionNotRepresentable { .. })
        ));
    }

    #[test]
    fn sphere_counts_poles_and_the_seam() {
        let coarse = MeshData::sphere(2.0, 4, 2).unwrap();
        assert_eq!((coarse.vertices.len(), coarse.indices.len()), (13, 24));
        let finer = MeshData::sphere(2.0, 4, 3).unwrap();
        assert_eq!((finer.vertices.len(), finer.indices.len()), (18, 48));
        for m in [&coarse, &finer] {
            assert_valid(m);
            assert!(m.vertices.iter().all(|v| (Vec3::from(v.position).length() - 2.0).abs() < 1e-4));
            let (c, r) = m.bounding_sphere();
            assert!(c.approx_eq(Vec3::ZERO, 1e-5));
            assert!((r - 2.0).abs() < 1e-4);
        }
        let seam = &coarse.vertices[4];
        let wrapped = &coarse.vertices[8];
        assert_eq!(seam.position, wrapped.position);
        assert_eq!(seam.uv[0], 0.0);
        assert_eq!(wrapped.uv[0], 1.0);
        same_geometry(&coarse, &MeshData::sphere(2.0, 4, 2).unwrap());
    }

    #[test]
    fn sphere_rejects_bad_arguments() {
        assert!(matches!(MeshData::sphere(0.0, 4, 2), Err(MeshError::DimensionNotPositive { axis: "radius", .. })));
        assert!(matches!(MeshData::sphere(-1.0, 4, 2), Err(MeshError::DimensionNotPositive { .. })));
        assert!(matches!(MeshData::sphere(f32::NAN, 4, 2), Err(MeshError::DimensionNotFinite { axis: "radius" })));
        assert!(matches!(MeshData::sphere(1.0, 2, 2), Err(MeshError::TooFewSegments { minimum: 3, .. })));
        assert!(matches!(MeshData::sphere(1.0, 3, 1), Err(MeshError::TooFewRings { minimum: 2, .. })));
        assert!(matches!(MeshData::sphere(1.0, 40_000, 3), Err(MeshError::TooManyVertices { .. })));
        assert!(matches!(MeshData::sphere(1.0, u32::MAX, u32::MAX), Err(MeshError::CountOverflow)));
    }

    #[test]
    fn cylinder_side_and_caps_have_separate_normals() {
        let capped = MeshData::cylinder(2.0, 4.0, 4).unwrap();
        assert_eq!((capped.vertices.len(), capped.indices.len()), (20, 48));
        assert_valid(&capped);
        let open = MeshData::cylinder_with_caps(2.0, 4.0, 4, false).unwrap();
        assert_eq!((open.vertices.len(), open.indices.len()), (10, 24));
        assert_valid(&open);
        assert!(open.vertices.iter().all(|v| v.normal[1] == 0.0));
        let cap_normals: Vec<_> = capped.vertices.iter().skip(10).map(|v| v.normal[1]).collect();
        assert!(cap_normals.contains(&1.0) && cap_normals.contains(&-1.0));
        assert!(capped.vertices.iter().take(10).all(|v| v.normal[1] == 0.0));
        assert!(capped.vertices.iter().all(|v| v.position[0].abs() <= 2.0 + 1e-5 && v.position[1].abs() <= 2.0 + 1e-5));
        let (c, r) = capped.bounding_sphere();
        assert!(c.approx_eq(Vec3::ZERO, 1e-5));
        assert!((r - Vec3::new(2.0, 2.0, 0.0).length()).abs() < 1e-4);
        let seam = &capped.vertices[0];
        let wrapped = &capped.vertices[8];
        assert_eq!(seam.position, wrapped.position);
        assert_eq!((seam.uv[0], wrapped.uv[0]), (0.0, 1.0));
        same_geometry(&capped, &MeshData::cylinder(2.0, 4.0, 4).unwrap());
    }

    #[test]
    fn cylinder_rejects_bad_arguments() {
        assert!(matches!(MeshData::cylinder(0.0, 1.0, 4), Err(MeshError::DimensionNotPositive { axis: "radius", .. })));
        assert!(matches!(MeshData::cylinder(1.0, 0.0, 4), Err(MeshError::DimensionNotPositive { axis: "height", .. })));
        assert!(matches!(MeshData::cylinder(-1.0, 1.0, 4), Err(MeshError::DimensionNotPositive { .. })));
        assert!(matches!(MeshData::cylinder(f32::INFINITY, 1.0, 4), Err(MeshError::DimensionNotFinite { .. })));
        assert!(matches!(MeshData::cylinder(1.0, 1.0, 2), Err(MeshError::TooFewSegments { .. })));
        assert!(matches!(
            MeshData::cylinder_with_caps(1.0, 1.0, u32::MAX, true),
            Err(MeshError::TooManyVertices { .. })
        ));
    }

    #[test]
    fn generated_meshes_register_as_ordinary_assets() {
        use crate::AssetManager;
        let mut assets = AssetManager::in_memory();
        let sphere = MeshData::sphere(1.0, 8, 4).unwrap();
        let handle = assets.insert("sphere", sphere).unwrap();
        assert_eq!(assets.get(handle).unwrap().vertices.len(), MeshData::sphere(1.0, 8, 4).unwrap().vertices.len());
        assets.insert("cube", MeshData::cube(1.0).unwrap()).unwrap();
        assets.insert("plane", MeshData::plane(1.0, 2.0).unwrap()).unwrap();
    }

    #[test]
    fn cone_is_well_formed() {
        let m = MeshData::cone(2.0, 5.0, 16);
        let (c, _) = m.bounding_sphere();
        assert!((c.y - 2.5).abs() < 1e-5);
        check_normals_match_winding(&m);
    }
}
