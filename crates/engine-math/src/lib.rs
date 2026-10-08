//! Math types for the engine.
//!
//! Conventions (see `docs/ARCHITECTURE.md`):
//! * `f32`, column-major matrices, column vectors (`M * v`).
//! * Left-handed, +Y up, +Z away from the default camera.
//! * Projection matrices produce NDC depth in `[0, 1]`.

mod mat3;
mod mat4;
mod quat;
mod rect;
mod transform;
mod vec2;
mod vec3;
mod vec4;

pub use mat3::Mat3;
pub use mat4::Mat4;
pub use quat::Quat;
pub use rect::Rect;
pub use transform::Transform;
pub use vec2::Vec2;
pub use vec3::Vec3;
pub use vec4::Vec4;

/// Tolerance used by `approx_eq` helpers and degenerate-case checks.
pub const EPSILON: f32 = 1e-6;

/// Linear interpolation between two scalars.
#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Implements the component-wise operator set shared by all vector types.
macro_rules! impl_vec_ops {
    ($T:ident { $($f:ident),+ }) => {
        impl core::ops::Add for $T { type Output = $T; #[inline] fn add(self, r: $T) -> $T { $T { $($f: self.$f + r.$f),+ } } }
        impl core::ops::Sub for $T { type Output = $T; #[inline] fn sub(self, r: $T) -> $T { $T { $($f: self.$f - r.$f),+ } } }
        impl core::ops::Mul for $T { type Output = $T; #[inline] fn mul(self, r: $T) -> $T { $T { $($f: self.$f * r.$f),+ } } }
        impl core::ops::Div for $T { type Output = $T; #[inline] fn div(self, r: $T) -> $T { $T { $($f: self.$f / r.$f),+ } } }
        impl core::ops::Mul<f32> for $T { type Output = $T; #[inline] fn mul(self, r: f32) -> $T { $T { $($f: self.$f * r),+ } } }
        impl core::ops::Div<f32> for $T { type Output = $T; #[inline] fn div(self, r: f32) -> $T { $T { $($f: self.$f / r),+ } } }
        impl core::ops::Mul<$T> for f32 { type Output = $T; #[inline] fn mul(self, r: $T) -> $T { r * self } }
        impl core::ops::Neg for $T { type Output = $T; #[inline] fn neg(self) -> $T { $T { $($f: -self.$f),+ } } }
        impl core::ops::AddAssign for $T { #[inline] fn add_assign(&mut self, r: $T) { *self = *self + r; } }
        impl core::ops::SubAssign for $T { #[inline] fn sub_assign(&mut self, r: $T) { *self = *self - r; } }
        impl core::ops::MulAssign<f32> for $T { #[inline] fn mul_assign(&mut self, r: f32) { *self = *self * r; } }
        impl core::ops::DivAssign<f32> for $T { #[inline] fn div_assign(&mut self, r: f32) { *self = *self / r; } }
    };
}
pub(crate) use impl_vec_ops;
