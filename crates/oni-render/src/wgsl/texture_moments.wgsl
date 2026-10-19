// `textureMoments` as its own pass; needs `TEXTURE_MATH`.

struct Params { head: vec4<u32> }

fn texture_moments_px(luma: vec4<f32>) -> vec4<f32> {
    return texture_moments_of(luma);
}

// MARK: - Buffers

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> luma: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> dest: array<vec4<f32>>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let i = flat_index(id, groups);
    if (i >= P.head.x) { return; }
    dest[i] = texture_moments_px(luma[i]);
}
