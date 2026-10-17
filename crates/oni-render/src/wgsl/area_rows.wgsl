// The area reductions' first pass: one invocation per row of the rectangle, its sum then its maximum.

struct Params { head: vec4<u32>, rect: vec4<u32> }

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> src_px: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> partial: array<vec4<f32>>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let r = flat_index(id, groups);
    if (r >= P.head.x) { return; }
    let start = (P.rect.y + r) * P.head.y + P.rect.x;
    var sum = vec4<f32>(0.0);
    var highest = vec4<f32>(-infinity());
    for (var x = 0u; x < P.rect.z; x++) {
        let v = src_px[start + x];
        sum += v;
        highest = max(highest, v);
    }
    partial[2u * r] = sum;
    partial[2u * r + 1u] = highest;
}
