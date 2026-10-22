// Viewport-sized color fill. The pipeline's viewport and scissor are the camera rectangle.
// This shader does not include the scene globals: a later camera must not depend on them
// to replace only its own rectangle.

struct ClearColor {
    color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> clear_color: ClearColor;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
}

@vertex
fn vs_clear(@builtin(vertex_index) index: u32) -> VsOut {
    // One triangle that covers the whole NDC square. The viewport maps that square onto the
    // camera rectangle, and the scissor rejects anything outside it.
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(positions[index], 0.0, 1.0);
    return out;
}

@fragment
fn fs_clear() -> @location(0) vec4<f32> {
    return clear_color.color;
}
