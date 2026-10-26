// Linearized view depth for one camera viewport, from a 4× depth image.
//
// The displayed value is the nearest reverse-Z sample at that pixel: the maximum of the four
// stored samples. It is not an average of the samples and it is not a coverage fraction. A pixel
// with one covered sample and three clear samples shows that covered surface. Clear is 0, so an
// uncovered sample loses to any written sample.
//
// The pipeline's viewport and scissor are that camera's rectangle. This shader does not include
// the scene globals. Group 0 is not the scene bind group. The pass writes the one-sample
// destination. It does not write the multisampled depth.
//
// Reverse-Z: stored 1 is the near plane, stored 0 is the clear and is unwritten. NDC 1 and NDC 0
// are unprojected through the camera's inverse projection. A span that is not a positive
// view-space range, or a singular matrix (mode == 0), is the unsupported color.

struct DepthView {
    inv_proj: mat4x4<f32>,
    viewport: vec4<f32>,
    empty_color: vec4<f32>,
    unsupported_color: vec4<f32>,
    mode: u32,
    min_span: f32,
    _pad0: u32,
    _pad1: u32,
}

@group(0) @binding(0) var depth_tex: texture_depth_multisampled_2d;
@group(0) @binding(1) var<uniform> params: DepthView;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
}

@vertex
fn vs_depth(@builtin(vertex_index) index: u32) -> VsOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(positions[index], 0.0, 1.0);
    return out;
}

fn usable(v: f32) -> bool {
    return v == v && abs(v) < 1e20;
}

// x is view-space Z. y is 1 when the unproject succeeded.
fn view_z(ndc_x: f32, ndc_y: f32, ndc_z: f32) -> vec2<f32> {
    let h = params.inv_proj * vec4<f32>(ndc_x, ndc_y, ndc_z, 1.0);
    if (!usable(h.x) || !usable(h.y) || !usable(h.z) || !usable(h.w) || abs(h.w) <= 1e-8) {
        return vec2<f32>(0.0, 0.0);
    }
    let z = h.z / h.w;
    if (!usable(z)) {
        return vec2<f32>(0.0, 0.0);
    }
    return vec2<f32>(z, 1.0);
}

@fragment
fn fs_depth(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    if (params.mode == 0u) {
        return params.unsupported_color;
    }
    let origin = params.viewport.xy;
    let size = max(params.viewport.zw, vec2<f32>(1.0, 1.0));
    let local = (pos.xy - origin) / size;
    let ndc_x = local.x * 2.0 - 1.0;
    let ndc_y = 1.0 - local.y * 2.0;
    let near = view_z(ndc_x, ndc_y, 1.0);
    let far = view_z(ndc_x, ndc_y, 0.0);
    if (near.y < 0.5 || far.y < 0.5) {
        return params.unsupported_color;
    }
    let span = far.x - near.x;
    if (!usable(span) || span <= params.min_span) {
        return params.unsupported_color;
    }
    let texel = vec2<i32>(i32(pos.x), i32(pos.y));
    let s0 = textureLoad(depth_tex, texel, 0);
    let s1 = textureLoad(depth_tex, texel, 1);
    let s2 = textureLoad(depth_tex, texel, 2);
    let s3 = textureLoad(depth_tex, texel, 3);
    // Nearest reverse-Z sample. Not the mean of the four depths.
    let stored = max(max(s0, s1), max(s2, s3));
    if (stored <= 0.0) {
        return params.empty_color;
    }
    let here = view_z(ndc_x, ndc_y, stored);
    if (here.y < 0.5) {
        return params.unsupported_color;
    }
    let nearness = clamp((far.x - here.x) / span, 0.0, 1.0);
    return vec4<f32>(nearness, nearness, nearness, 1.0);
}
