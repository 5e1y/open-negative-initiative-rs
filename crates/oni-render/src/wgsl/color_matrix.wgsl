// `CIColorMatrix` on an opaque image: each channel a dot with its vector, plus the bias.

struct Params { head: vec4<u32>, r: vec4<f32>, g: vec4<f32>, b: vec4<f32>, a: vec4<f32>, bias: vec4<f32> }

fn color_matrix_px(s: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(dot(s, P.r), dot(s, P.g), dot(s, P.b), dot(s, P.a)) + P.bias;
}

// MARK: - Buffers

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> src_px: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> dest: array<vec4<f32>>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let i = flat_index(id, groups);
    if (i >= P.head.x) { return; }
    dest[i] = color_matrix_px(src_px[i]);
}
