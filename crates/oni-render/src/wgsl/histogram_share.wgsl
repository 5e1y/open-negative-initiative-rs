// `CIAreaHistogram`'s output: each bin's share of the pixels, times `scale`.

struct Params { head: vec4<u32>, scale: vec4<f32> }

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> counts: array<vec4<u32>>;
@group(0) @binding(2) var<storage, read_write> dest: array<vec4<f32>>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let i = flat_index(id, groups);
    if (i >= P.head.x) { return; }
    dest[i] = vec4<f32>(counts[i]) / f32(P.head.y) * P.scale.x;
}
