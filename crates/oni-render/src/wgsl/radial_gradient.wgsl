// `CIRadialGradient`: `color0` inside `radius0` to `color1` past `radius1`.

struct Params { head: vec4<u32>, origin: vec4<f32>, circle: vec4<f32>, color0: vec4<f32>, color1: vec4<f32> }

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read_write> dest: array<vec4<f32>>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let i = flat_index(id, groups);
    if (i >= P.head.x) { return; }
    let p = ci_coord(i % P.head.y, i / P.head.y, P.origin.xy, P.head.z);
    let c = P.circle;
    let t = clamp((length(p - c.xy) - c.z) / (c.w - c.z), 0.0, 1.0);
    dest[i] = mix(P.color0, P.color1, vec4<f32>(t));
}
