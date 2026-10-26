// Copy one resolved destination pixel into every sample of the multisampled color attachment.
//
// The pass viewport and scissor are the camera rectangle. A fragment that covers the pixel writes
// this color to every covered sample. The fullscreen triangle covers the pixel, so all four
// samples receive the same value. The scene pass then blends against those samples.
//
// `textureLoad` of an sRGB image returns linear values. Writing an sRGB multisample attachment
// encodes. The later resolve decodes, averages four identical samples, and encodes once more.
// There is no manual gamma in this shader.

@group(0) @binding(0) var source: texture_2d<f32>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
}

@vertex
fn vs_expand(@builtin(vertex_index) index: u32) -> VsOut {
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
fn fs_expand(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(source, vec2<i32>(position.xy), 0);
}
