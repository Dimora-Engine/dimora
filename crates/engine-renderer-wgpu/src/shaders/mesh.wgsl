struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) m0: vec4<f32>,
    @location(4) m1: vec4<f32>,
    @location(5) m2: vec4<f32>,
    @location(6) m3: vec4<f32>,
    @location(7) color: vec4<f32>,
    @location(8) flags: vec4<u32>,
}

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world_pos: vec3<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) color: vec4<f32>,
    @location(4) @interpolate(flat) lit: u32,
    @location(5) @interpolate(flat) cutoff_bits: u32,
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let model = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    let world = model * vec4<f32>(in.position, 1.0);
    // Cofactor matrix == inverse-transpose up to a scalar: correct under non-uniform scale.
    let c0 = in.m0.xyz;
    let c1 = in.m1.xyz;
    let c2 = in.m2.xyz;
    var out: VsOut;
    out.clip = globals.view_proj * world;
    out.world_pos = world.xyz;
    out.normal = cross(c1, c2) * in.normal.x + cross(c2, c0) * in.normal.y + cross(c0, c1) * in.normal.z;
    out.uv = in.uv;
    out.color = in.color;
    out.lit = in.flags.x;
    out.cutoff_bits = in.flags.y;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(albedo_tex, albedo_sampler, in.uv) * in.color;
    var rgb = texel.rgb;
    if in.lit != 0u {
        rgb = shade(rgb, in.world_pos, in.normal, 0.0);
    }
    return vec4<f32>(apply_fog(rgb, in.world_pos), 1.0);
}

@fragment
fn fs_mask(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(albedo_tex, albedo_sampler, in.uv) * in.color;
    if texel.a < bitcast<f32>(in.cutoff_bits) {
        discard;
    }
    var rgb = texel.rgb;
    if in.lit != 0u {
        rgb = shade(rgb, in.world_pos, in.normal, 0.0);
    }
    return vec4<f32>(apply_fog(rgb, in.world_pos), 1.0);
}

@fragment
fn fs_blend(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(albedo_tex, albedo_sampler, in.uv) * in.color;
    var rgb = texel.rgb;
    if in.lit != 0u {
        rgb = shade(rgb, in.world_pos, in.normal, 0.0);
    }
    return vec4<f32>(apply_fog(rgb, in.world_pos), texel.a);
}
