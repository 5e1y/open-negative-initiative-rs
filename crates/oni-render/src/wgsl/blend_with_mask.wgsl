// `CIBlendWithMask`: the background where the mask's green is 0, the image where it is 1.

struct Params { head: vec4<u32> }

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> picture: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read> background: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read> mask: array<vec4<f32>>;
@group(0) @binding(4) var<storage, read_write> dest: array<vec4<f32>>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let i = flat_index(id, groups);
    if (i >= P.head.x) { return; }
    dest[i] = mix(background[i], picture[i], vec4<f32>(mask[i].g));
}
