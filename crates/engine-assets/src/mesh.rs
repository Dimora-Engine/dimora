//! CPU-side mesh data and a few primitive generators.

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

/// Indexed triangle list with a precomputed bounding sphere (used for culling).
#[derive(Clone, Debug)]
pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    bounds_center: Vec3,
    bounds_radius: f32,
}

impl MeshData {
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

    /// Axis-aligned box centered at the origin with flat-shaded faces.
    pub fn cuboid(size: Vec3) -> Self {
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

impl Asset for MeshData {
    const KIND: AssetKind = AssetKind::Mesh;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_normals_match_winding(m: &MeshData) {
        // Geometric normal of every triangle should agree with its vertex normals, using the
        // left-handed convention that front faces are clockwise from outside: n = (c-a) x (b-a).
        for t in m.indices.chunks(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| Vec3::from(m.vertices[i as usize].position));
            let g = (c - a).cross(b - a);
            if g.length() < 1e-6 {
                continue;
            }
            let n = Vec3::from(m.vertices[t[0] as usize].normal);
            assert!(g.normalized().dot(n) > 0.0, "triangle {t:?} winds against its normal");
        }
    }

    #[test]
    fn cuboid_is_well_formed() {
        let m = MeshData::cuboid(Vec3::new(2.0, 4.0, 6.0));
        assert_eq!((m.vertices.len(), m.indices.len()), (24, 36));
        let (c, r) = m.bounding_sphere();
        assert!(c.approx_eq(Vec3::ZERO, 1e-6));
        assert!((r - Vec3::new(1.0, 2.0, 3.0).length()).abs() < 1e-5);
        assert!(m.vertices.iter().all(|v| (Vec3::from(v.normal).length() - 1.0).abs() < 1e-5));
        check_normals_match_winding(&m);
    }

    #[test]
    fn cone_is_well_formed() {
        let m = MeshData::cone(2.0, 5.0, 16);
        let (c, _) = m.bounding_sphere();
        assert!((c.y - 2.5).abs() < 1e-5);
        check_normals_match_winding(&m);
    }
}
