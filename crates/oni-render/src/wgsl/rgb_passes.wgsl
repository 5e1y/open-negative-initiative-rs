// `rgbPasses`: one frame from three narrowband passes, each giving the channel of its band.

struct Params { head: vec4<u32> }

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> red: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read> green: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read> blue: array<vec4<f32>>;
@group(0) @binding(4) var<storage, read_write> dest: array<vec4<f32>>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let i = flat_index(id, groups);
    if (i >= P.head.x) { return; }
    dest[i] = vec4<f32>(red[i].r, green[i].g, blue[i].b, 1.0);
}
