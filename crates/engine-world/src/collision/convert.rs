//! Dimora matrix to a rigid Parry pose plus positive extents.
//!
//! The center uses the full matrix, so a negative scale mirrors `Collider::offset`. The rotation
//! and the shape sizes are taken from an orthogonal basis with reflections folded out.

use engine_math::{Mat3, Mat4, Quat, Vec3};
use parry3d::bounding_volume::Aabb;
use parry3d::math::{Pose, Rotation, Vector};
use parry3d::shape::{Ball, Capsule, Cuboid, Shape};

use super::types::{point_finite, shape_ok, ColliderShape, SkipReason};

/// Normalized column dots above this are shear.
const SHEAR_DOT: f32 = 1e-3;
/// Sphere and capsule axes must agree within this.
const UNIFORM_EPS: f32 = 1e-4;
/// A shorter column is degenerate.
const MIN_EXTENT: f32 = 1e-6;

pub(crate) struct Prepared {
    pub center: Vec3,
    pub rotation: Quat,
    pub shape: ColliderShape,
    // Read by overlap and ray queries.
    #[allow(dead_code)]
    pub pose: Pose,
    #[allow(dead_code)]
    pub parry: StoredShape,
    #[allow(dead_code)]
    pub aabb: Aabb,
}

pub(crate) enum StoredShape {
    Ball(Ball),
    Cuboid(Cuboid),
    Capsule(Capsule),
}

impl StoredShape {
    #[allow(dead_code)]
    pub fn as_shape(&self) -> &dyn Shape {
        match self {
            StoredShape::Ball(shape) => shape,
            StoredShape::Cuboid(shape) => shape,
            StoredShape::Capsule(shape) => shape,
        }
    }
}

pub(crate) fn prepare(shape: ColliderShape, offset: Vec3, matrix: &Mat4) -> Result<Prepared, SkipReason> {
    if !shape_ok(shape) || !point_finite(offset) {
        return Err(SkipReason::BadShape);
    }
    let center = matrix.transform_point3(offset);
    if !point_finite(center) || !matrix_finite(matrix) {
        return Err(SkipReason::NonFinite);
    }

    let mut columns = [matrix.cols[0].xyz(), matrix.cols[1].xyz(), matrix.cols[2].xyz()];
    let lengths = [columns[0].length(), columns[1].length(), columns[2].length()];
    if lengths.iter().any(|length| !length.is_finite()) {
        return Err(SkipReason::NonFinite);
    }
    if lengths.iter().any(|length| *length < MIN_EXTENT) {
        return Err(SkipReason::Degenerate);
    }

    let triple = columns[0].dot(columns[1].cross(columns[2]));
    if !triple.is_finite() {
        return Err(SkipReason::NonFinite);
    }
    if triple < 0.0 {
        columns[0] = -columns[0];
    }

    let normals =
        [columns[0] / columns[0].length(), columns[1] / columns[1].length(), columns[2] / columns[2].length()];
    if normals.iter().any(|axis| !point_finite(*axis)) {
        return Err(SkipReason::NonFinite);
    }
    if normals[0].dot(normals[1]).abs() > SHEAR_DOT
        || normals[1].dot(normals[2]).abs() > SHEAR_DOT
        || normals[2].dot(normals[0]).abs() > SHEAR_DOT
    {
        return Err(SkipReason::Shear);
    }

    let uniform = (lengths[0] - lengths[1]).abs() <= UNIFORM_EPS && (lengths[1] - lengths[2]).abs() <= UNIFORM_EPS;
    let scaled = match shape {
        ColliderShape::Sphere { radius } => {
            if !uniform {
                return Err(SkipReason::NonUniformScale);
            }
            ColliderShape::Sphere { radius: radius * lengths[0] }
        }
        ColliderShape::Capsule { radius, half_height } => {
            if !uniform {
                return Err(SkipReason::NonUniformScale);
            }
            ColliderShape::Capsule { radius: radius * lengths[0], half_height: half_height * lengths[0] }
        }
        ColliderShape::Box { half_extents } => ColliderShape::Box {
            half_extents: Vec3::new(
                half_extents.x * lengths[0],
                half_extents.y * lengths[1],
                half_extents.z * lengths[2],
            ),
        },
    };
    if !shape_ok(scaled) {
        return Err(SkipReason::BadShape);
    }

    let rotation = Quat::from_mat3(&Mat3::from_cols(normals[0], normals[1], normals[2])).normalize();
    if !rotation.x.is_finite() || !rotation.y.is_finite() || !rotation.z.is_finite() || !rotation.w.is_finite() {
        return Err(SkipReason::NonFinite);
    }

    let pose = Pose::from_parts(to_vector(center), Rotation::from_xyzw(rotation.x, rotation.y, rotation.z, rotation.w));
    let parry = stored(scaled);
    let aabb = parry.as_shape().compute_aabb(&pose);
    Ok(Prepared { center, rotation, shape: scaled, pose, parry, aabb })
}

fn stored(shape: ColliderShape) -> StoredShape {
    match shape {
        ColliderShape::Sphere { radius } => StoredShape::Ball(Ball::new(radius)),
        ColliderShape::Box { half_extents } => {
            StoredShape::Cuboid(Cuboid::new(Vector::new(half_extents.x, half_extents.y, half_extents.z)))
        }
        ColliderShape::Capsule { radius, half_height } => StoredShape::Capsule(Capsule::new_y(half_height, radius)),
    }
}

fn matrix_finite(matrix: &Mat4) -> bool {
    matrix
        .cols
        .iter()
        .all(|column| column.x.is_finite() && column.y.is_finite() && column.z.is_finite() && column.w.is_finite())
}

pub(crate) fn to_vector(value: Vec3) -> Vector {
    Vector::new(value.x, value.y, value.z)
}
