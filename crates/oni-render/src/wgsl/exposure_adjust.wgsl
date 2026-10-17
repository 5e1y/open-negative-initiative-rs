// `CIExposureAdjust`: the colour times `2^ev`, alpha kept.

struct Params { head: vec4<u32>, ev: vec4<f32> }

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> src_px: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> dest: array<vec4<f32>>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let i = flat_index(id, groups);
    if (i >= P.head.x) { return; }
    let s = src_px[i];
    dest[i] = vec4<f32>(s.rgb * exp2(P.ev.x), s.a);
}
