// `texture`: the band between the narrow luma and its edge-aware base, scaled by `gain` on the luma alone.

struct Params { head: vec4<u32>, gain: vec4<f32> }

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> src_px: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read> narrow_luma: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read> coeff: array<vec4<f32>>;
@group(0) @binding(4) var<storage, read_write> dest: array<vec4<f32>>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let i = flat_index(id, groups);
    if (i >= P.head.x) { return; }
    let s = src_px[i];
    let n = narrow_luma[i].r;
    let c = coeff[i];
    let y = dot(s.rgb, LUMA);
    let chroma = s.rgb - y;
    let base = c.r * n + c.g;
    let shaped = y + (n - base) * P.gain.x;
    dest[i] = vec4<f32>(max(shaped, 0.0) + chroma, s.a);
}
