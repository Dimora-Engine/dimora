// Instanced sprite/tile quads. The unit quad [0,1]^2 is transformed by a per-instance model
// matrix that already carries size, pivot and the entity's world transform, so there is no
// vertex buffer and one draw call covers a whole batch.

struct Instance {
    @builtin(vertex_index) vertex: u32,
    @location(0) m0: vec4<f32>,
    @location(1) m1: vec4<f32>,
    @location(2) m2: vec4<f32>,
    @location(3) m3: vec4<f32>,
    @location(4) uv_rect: vec4<f32>,
    @location(5) tint: vec4<f32>,
    @location(6) flags: vec4<u32>,
}

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world_pos: vec3<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) tint: vec4<f32>,
    @location(4) @interpolate(flat) lit: u32,
    @location(5) @interpolate(flat) cutoff_bits: u32,
}

const CORNERS = array<vec2<f32>, 6>(
    vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
    vec2(0.0, 0.0), vec2(1.0, 1.0), vec2(0.0, 1.0),
);

@vertex
fn vs_main(in: Instance) -> VsOut {
    let p = CORNERS[in.vertex];
    let model = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    let world = model * vec4<f32>(p, 0.0, 1.0);
    var out: VsOut;
    out.clip = globals.view_proj * world;
    // Opt-in only. flags.z is 0 for every sprite that has no CoplanarPriority, and for tiles.
    // clip.xy and clip.w stay on the real projection. Only depth moves.
    if in.flags.z != 0u {
        out.clip = depth_only(out.clip, world, globals.coplanar.x * (1.0 + f32(in.flags.w)));
    }
    out.world_pos = world.xyz;
    out.uv = vec2<f32>(mix(in.uv_rect.x, in.uv_rect.z, p.x), mix(in.uv_rect.w, in.uv_rect.y, p.y));
    // A sprite faces local -Z (toward the default camera).
    out.normal = -cross(in.m0.xyz, in.m1.xyz);
    out.tint = in.tint;
    out.lit = in.flags.x;
    out.cutoff_bits = in.flags.y;
    return out;
}

// Move an opted-in sprite toward the camera in depth only.
//
// `away` is the view ray through this vertex, in world space, built in homogeneous form so a
// vertex on the camera plane does not divide by zero. The original clip.xy and clip.w are kept.
// A vertex that was inside the visible interval cannot be pushed through the near or far plane.
fn depth_only(clip: vec4<f32>, world: vec4<f32>, amount: f32) -> vec4<f32> {
    if amount <= 0.0 || clip.w <= 0.0 {
        return clip;
    }
    let p0 = globals.inv_view_proj * vec4<f32>(clip.xy, 0.0, clip.w);
    let p1 = globals.inv_view_proj * vec4<f32>(clip.xy, clip.w, clip.w);
    if abs(p0.w) < 1e-6 || abs(p1.w) < 1e-6 {
        return clip;
    }
    var away = p1.xyz / p1.w - p0.xyz / p0.w;
    let len = length(away);
    if len < 1e-6 {
        return clip;
    }
    away = away / len;
    if dot(away, world.xyz - globals.camera_pos.xyz) < 0.0 {
        away = -away;
    }
    let moved = globals.view_proj * vec4<f32>(world.xyz - away * amount, 1.0);
    if moved.w <= 1e-6 {
        return clip;
    }
    var out = clip;
    var z = (moved.z / moved.w) * clip.w;
    if clip.z >= 0.0 && clip.z <= clip.w {
        z = clamp(z, 0.0, clip.w);
    }
    out.z = z;
    return out;
}

fn color_of(in: VsOut) -> vec4<f32> {
    let texel = textureSample(albedo_tex, albedo_sampler, in.uv) * in.tint;
    var rgb = texel.rgb;
    if in.lit != 0u {
        rgb = shade(rgb, in.world_pos, in.normal, 0.35);
    }
    return vec4<f32>(apply_fog(rgb, in.world_pos), texel.a);
}

@fragment
fn fs_cutout(in: VsOut) -> @location(0) vec4<f32> {
    let c = color_of(in);
    if c.a < 0.5 {
        discard;
    }
    return vec4<f32>(c.rgb, 1.0);
}

@fragment
fn fs_opaque(in: VsOut) -> @location(0) vec4<f32> {
    let c = color_of(in);
    return vec4<f32>(c.rgb, 1.0);
}

@fragment
fn fs_mask(in: VsOut) -> @location(0) vec4<f32> {
    let c = color_of(in);
    if c.a < bitcast<f32>(in.cutoff_bits) {
        discard;
    }
    return vec4<f32>(c.rgb, 1.0);
}

@fragment
fn fs_blend(in: VsOut) -> @location(0) vec4<f32> {
    let c = color_of(in);
    return c;
}
