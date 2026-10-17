// The area reductions' second pass, one invocation: the mean over every pixel, then the maximum.

struct Params { head: vec4<u32> }

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> partial: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> dest: array<vec4<f32>>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    if (flat_index(id, groups) != 0u) { return; }
    var sum = vec4<f32>(0.0);
    var highest = vec4<f32>(-infinity());
    for (var r = 0u; r < P.head.x; r++) {
        sum += partial[2u * r];
        highest = max(highest, partial[2u * r + 1u]);
    }
    dest[0] = sum / f32(P.head.y);
    dest[1] = highest;
}
