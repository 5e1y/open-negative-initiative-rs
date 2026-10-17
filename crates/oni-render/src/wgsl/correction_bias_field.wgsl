// `correctionBiasField`: a bias varying linearly across the patch, read where it writes.

struct Params {
    head: vec4<u32>,
    base: vec4<f32>,
    grad_x: vec4<f32>,
    grad_y: vec4<f32>,
    origins: vec4<f32>,
}

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> patch_px: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> dest: array<vec4<f32>>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let i = flat_index(id, groups);
    if (i >= P.head.x) { return; }
    let pos = ci_coord(i % P.head.y, i / P.head.y, P.origins.zw, P.head.z);
    let bias = P.base.xyz + P.grad_x.xyz * (pos.x - P.origins.x) + P.grad_y.xyz * (pos.y - P.origins.y);
    let p = patch_px[i];
    dest[i] = vec4<f32>(p.rgb + bias, p.a);
}
