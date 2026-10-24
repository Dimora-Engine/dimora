// Unlit debug lines. This file is NOT prepended with common.wgsl: that file also declares
// the group-1 albedo texture, and this pipeline binds group 0 only.
//
// `Globals` must stay layout-compatible with `GpuGlobals` and with `struct Globals` in
// common.wgsl. The vertex format is vec4 position plus vec4 color (32 bytes). A vec3
// attribute would be padded to 16 bytes and would not match the Rust struct.

struct Light {
    position_range: vec4<f32>,
    direction_kind: vec4<f32>,
    color: vec4<f32>,
    cone: vec4<f32>,
}

struct Globals {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    ambient: vec4<f32>,
    fog: vec4<f32>,
    coplanar: vec4<f32>,
    counts: vec4<u32>,
    lights: array<Light, 16>,
}

@group(0) @binding(0) var<uniform> globals: Globals;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_debug(@location(0) position: vec4<f32>, @location(1) color: vec4<f32>) -> VsOut {
    var out: VsOut;
    out.clip = globals.view_proj * position;
    out.color = color;
    return out;
}

@fragment
fn fs_debug(in: VsOut) -> @location(0) vec4<f32> {
    return in.color;
}
