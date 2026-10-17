// `scannerEncode`: linear Rec. 2020 to display sRGB code held inside the cube's domain, then its
// `root`, which spends a cube's nodes on the shadows.

struct Params { head: vec4<u32>, root: vec4<f32> }

fn scanner_encode_px(s: vec4<f32>) -> vec4<f32> {
    let l = vec3<f32>(dot(s.rgb, vec3<f32>(1.660490, -0.587641, -0.072850)),
                      dot(s.rgb, vec3<f32>(-0.124550, 1.132900, -0.008349)),
                      dot(s.rgb, vec3<f32>(-0.018150, -0.100579, 1.118730)));
    let c = clamp(l, vec3<f32>(0.0), vec3<f32>(1.0));
    let code = select(1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3<f32>(0.0031308));
    return vec4<f32>(pow(max(code, vec3<f32>(0.0)), vec3<f32>(1.0 / P.root.x)), s.a);
}

// MARK: - Buffers

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> src_px: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> dest: array<vec4<f32>>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let i = flat_index(id, groups);
    if (i >= P.head.x) { return; }
    dest[i] = scanner_encode_px(src_px[i]);
}
