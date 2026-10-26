// Average four color samples into the destination viewport.
//
// The pass viewport and scissor are the camera rectangle. This shader does not write pixels
// outside that rectangle. A hardware resolve is not used: it would store the whole attachment.
//
// `textureLoad` of an sRGB image returns linear values. The average stays in that space. Writing
// an sRGB attachment encodes once. There is no manual gamma in this shader.

@group(0) @binding(0) var source: texture_multisampled_2d<f32>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
}

@vertex
fn vs_resolve(@builtin(vertex_index) index: u32) -> VsOut {
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
fn fs_resolve(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(position.xy);
    let s0 = textureLoad(source, pixel, 0);
    let s1 = textureLoad(source, pixel, 1);
    let s2 = textureLoad(source, pixel, 2);
    let s3 = textureLoad(source, pixel, 3);
    return (s0 + s1 + s2 + s3) * 0.25;
}
