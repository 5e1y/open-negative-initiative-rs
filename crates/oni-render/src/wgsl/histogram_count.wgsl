// `CIAreaHistogram`'s counts: a value lands in bin `floor(v * bins)`, held on the first and last
// bin, as Core Image measures it. Counts are whole numbers, kept exact past 2^24 pixels.

struct Params { head: vec4<u32>, rect: vec4<u32> }

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> src_px: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> counts: array<atomic<u32>>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let i = flat_index(id, groups);
    if (i >= P.head.x) { return; }
    let x = P.rect.x + i % P.rect.z;
    let y = P.rect.y + i / P.rect.z;
    let v = src_px[y * P.head.y + x];
    let bins = f32(P.head.z);
    let b = vec4<u32>(clamp(floor(v * bins), vec4<f32>(0.0), vec4<f32>(bins - 1.0)));
    atomicAdd(&counts[4u * b.x], 1u);
    atomicAdd(&counts[4u * b.y + 1u], 1u);
    atomicAdd(&counts[4u * b.z + 2u], 1u);
    atomicAdd(&counts[4u * b.w + 3u], 1u);
}
