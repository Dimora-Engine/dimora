// Shared by every pass: camera + lights, and the one lighting function used for sprites,
// tiles and meshes alike. Everything is evaluated in *world space*, so a point light's falloff
// depends on true 3D distance, including depth.

struct Light {
    position_range: vec4<f32>,  // xyz = world position, w = range
    direction_kind: vec4<f32>,  // xyz = travel direction, w = kind (0 dir, 1 point, 2 spot)
    color: vec4<f32>,           // rgb = color * intensity
    cone: vec4<f32>,            // x = cos(inner), y = cos(outer)
}

struct Globals {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    ambient: vec4<f32>,
    fog: vec4<f32>,             // rgb = color, w = density
    counts: vec4<u32>,          // x = light count
    lights: array<Light, 16>,
}

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var albedo_tex: texture_2d<f32>;
@group(1) @binding(1) var albedo_sampler: sampler;

fn apply_fog(rgb: vec3<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    let d = distance(globals.camera_pos.xyz, world_pos) * globals.fog.w;
    return mix(rgb, globals.fog.rgb, 1.0 - exp(-d * d));
}

// `wrap` softens the terminator (0 = pure Lambert). Sprites are flat cards, so a little wrap
// keeps them from going black when a light sits roughly in their plane.
fn shade(albedo: vec3<f32>, world_pos: vec3<f32>, normal_in: vec3<f32>, wrap: f32) -> vec3<f32> {
    var n = normalize(normal_in);
    // Two-sided: always shade the face that points at the viewer.
    if dot(n, globals.camera_pos.xyz - world_pos) < 0.0 {
        n = -n;
    }
    var light = globals.ambient.rgb;
    for (var i = 0u; i < globals.counts.x; i = i + 1u) {
        let l = globals.lights[i];
        let kind = u32(l.direction_kind.w + 0.5);
        var to_light: vec3<f32>;
        var atten = 1.0;
        if kind == 0u {
            to_light = -l.direction_kind.xyz;
        } else {
            let d = l.position_range.xyz - world_pos;
            let dist = length(d);
            to_light = d / max(dist, 1e-4);
            let r = dist / max(l.position_range.w, 1e-4);
            let window = clamp(1.0 - r * r, 0.0, 1.0);
            atten = window * window;
            if kind == 2u {
                let c = dot(-to_light, l.direction_kind.xyz);
                atten = atten * smoothstep(l.cone.y, l.cone.x, c);
            }
        }
        let ndl = clamp((dot(n, to_light) + wrap) / (1.0 + wrap), 0.0, 1.0);
        light = light + l.color.rgb * (ndl * atten);
    }
    return albedo * light;
}
