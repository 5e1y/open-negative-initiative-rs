// `CIColorCube`: the colour held on 0…1 and read trilinearly off the cube, red running fastest;
// what the cube returns under zero is clipped, alpha kept.

struct Params { head: vec4<u32> }

// Node (r, g, b) is read through `cube_node(r, g, b, n)`, which each entry point defines.
fn color_cube_px(s: vec4<f32>) -> vec4<f32> {
    let n = P.head.w;
    let f = clamp(s.rgb, vec3<f32>(0.0), vec3<f32>(1.0)) * f32(n - 1u);
    let lo = min(vec3<u32>(floor(f)), vec3<u32>(n - 2u));
    let t = f - vec3<f32>(lo);
    let hi = lo + 1u;
    let c00 = mix(cube_node(lo.x, lo.y, lo.z, n), cube_node(hi.x, lo.y, lo.z, n), t.x);
    let c10 = mix(cube_node(lo.x, hi.y, lo.z, n), cube_node(hi.x, hi.y, lo.z, n), t.x);
    let c01 = mix(cube_node(lo.x, lo.y, hi.z, n), cube_node(hi.x, lo.y, hi.z, n), t.x);
    let c11 = mix(cube_node(lo.x, hi.y, hi.z, n), cube_node(hi.x, hi.y, hi.z, n), t.x);
    let c = mix(mix(c00, c10, t.y), mix(c01, c11, t.y), t.z);
    return vec4<f32>(max(c, vec3<f32>(0.0)), s.a);
}

// MARK: - Buffers

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> src_px: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read> cube: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> dest: array<vec4<f32>>;

fn cube_node(r: u32, g: u32, b: u32, n: u32) -> vec3<f32> {
    return cube[(b * n + g) * n + r].rgb;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let i = flat_index(id, groups);
    if (i >= P.head.x) { return; }
    dest[i] = color_cube_px(src_px[i]);
}
