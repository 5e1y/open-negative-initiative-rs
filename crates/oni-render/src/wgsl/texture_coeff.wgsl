// `textureCoeff`: the self-guided filter's coefficients, `a` the share of local detail the base
// follows; the threshold follows the local level, since a scene is multiplicative.

struct Params { head: vec4<u32>, relative: vec4<f32> }

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> moments: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> dest: array<vec4<f32>>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let i = flat_index(id, groups);
    if (i >= P.head.x) { return; }
    let m = moments[i];
    let mean = m.r;
    let variance = max(m.g - mean * mean, 0.0);
    let edge = P.relative.x * max(mean, 0.0);
    let eps = max(edge * edge, 1e-8);
    let a = variance / (variance + eps);
    dest[i] = vec4<f32>(a, (1.0 - a) * mean, 0.0, 1.0);
}
