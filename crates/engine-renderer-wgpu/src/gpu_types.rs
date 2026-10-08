//! Byte layouts shared with the WGSL shaders. Keep in sync with `shaders/*.wgsl`.

use bytemuck::{Pod, Zeroable};
use engine_math::Vec3;
use engine_renderer::{LightData, LightKind, RenderScene, MAX_LIGHTS};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct GpuLight {
    position_range: [f32; 4],
    direction_kind: [f32; 4],
    color: [f32; 4],
    cone: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct GpuGlobals {
    view_proj: [f32; 16],
    camera_pos: [f32; 4],
    ambient: [f32; 4],
    fog: [f32; 4],
    counts: [u32; 4],
    lights: [GpuLight; MAX_LIGHTS],
}

impl GpuGlobals {
    pub fn from_scene(scene: &RenderScene) -> Self {
        let mut lights = [GpuLight::zeroed(); MAX_LIGHTS];
        for (slot, l) in lights.iter_mut().zip(&scene.lights) {
            *slot = GpuLight::from_light(l);
        }
        let [ar, ag, ab] = scene.ambient;
        Self {
            view_proj: scene.view.view_proj.to_cols_array(),
            camera_pos: scene.view.position.extend(1.0).to_array(),
            ambient: [ar, ag, ab, 0.0],
            fog: scene.fog,
            counts: [scene.lights.len().min(MAX_LIGHTS) as u32, 0, 0, 0],
            lights,
        }
    }
}

impl GpuLight {
    fn from_light(l: &LightData) -> Self {
        let (kind, range, cone) = match l.kind {
            LightKind::Directional => (0.0, 0.0, [0.0, 0.0]),
            LightKind::Point { range } => (1.0, range, [0.0, 0.0]),
            LightKind::Spot { range, inner_angle, outer_angle } => (2.0, range, [inner_angle.cos(), outer_angle.cos()]),
        };
        let Vec3 { x, y, z } = l.position;
        Self {
            position_range: [x, y, z, range],
            direction_kind: [l.direction.x, l.direction.y, l.direction.z, kind],
            color: [l.color[0], l.color[1], l.color[2], 0.0],
            cone: [cone[0], cone[1], 0.0, 0.0],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct GpuSpriteInstance {
    pub model: [f32; 16],
    pub uv: [f32; 4],
    pub tint: [f32; 4],
    pub flags: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct GpuMeshInstance {
    pub model: [f32; 16],
    pub color: [f32; 4],
    pub flags: [u32; 4],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_match_wgsl() {
        // WGSL: Light = 4 x vec4; Globals = mat4 + 4 x vec4 + 16 lights.
        assert_eq!(core::mem::size_of::<GpuLight>(), 64);
        assert_eq!(core::mem::size_of::<GpuGlobals>(), 64 + 64 + 16 * 64);
        assert_eq!(core::mem::size_of::<GpuSpriteInstance>(), 112);
        assert_eq!(core::mem::size_of::<GpuMeshInstance>(), 96);
    }
}
