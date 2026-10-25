// Nearest copy of an internal camera image into the destination display rectangle.
//
// The pipeline viewport and scissor are that rectangle. Group 0 is not the scene bind group.
// textureLoad keeps each internal pixel as a scale-by-scale block. There is no lighting, fog,
// or tint. The source is a one-sample color image that is no longer a color attachment.

struct Present {
    origin: vec2<f32>,
    scale: u32,
    _pad: u32,
}

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var<uniform> params: Present;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
}

@vertex
fn vs_present(@builtin(vertex_index) index: u32) -> VsOut {
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
fn fs_present(in: VsOut) -> @location(0) vec4<f32> {
    let scale = max(i32(params.scale), 1);
    let px = i32(in.pos.x) - i32(params.origin.x);
    let py = i32(in.pos.y) - i32(params.origin.y);
    return textureLoad(src, vec2<i32>(px / scale, py / scale), 0);
}
