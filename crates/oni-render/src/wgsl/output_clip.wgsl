// `outputClip`: the hard, constant clip, the only bound above in the chain.

struct Params { head: vec4<u32> }

fn output_clip_px(s: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(clamp(s.rgb, vec3<f32>(0.0), vec3<f32>(1.0)), s.a);
}

// MARK: - Buffers

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> src_px: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> dest: array<vec4<f32>>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let i = flat_index(id, groups);
    if (i >= P.head.x) { return; }
    dest[i] = output_clip_px(src_px[i]);
}
