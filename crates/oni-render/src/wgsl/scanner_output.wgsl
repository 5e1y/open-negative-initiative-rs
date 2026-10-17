// `scannerOutput`: the rendered display sRGB moved by the warmth look at its share, the look's lift
// taken back off, linearised into Rec. 2020 on the same white, then bent by the face's Contrast.

struct Params { head: vec4<u32>, share_lift_contrast: vec4<f32> }

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> code: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read> warmth: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> dest: array<vec4<f32>>;

fn bent(v: f32, contrast: f32) -> f32 {
    return pow(contrasted(pow(v, 1.0 / DISPLAY_GAMMA), contrast), DISPLAY_GAMMA);
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let i = flat_index(id, groups);
    if (i >= P.head.x) { return; }
    let k = P.share_lift_contrast;
    let s = code[i];
    let c = clamp(s.rgb + (warmth[i].rgb - k.y) * k.x, vec3<f32>(0.0), vec3<f32>(1.0));
    let linear = select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
    var out = vec3<f32>(dot(linear, vec3<f32>(0.627404, 0.329283, 0.043313)),
                        dot(linear, vec3<f32>(0.069097, 0.919540, 0.011362)),
                        dot(linear, vec3<f32>(0.016391, 0.088013, 0.895595)));
    if (k.z != 0.0) {
        out = vec3<f32>(bent(out.r, k.z), bent(out.g, k.z), bent(out.b, k.z));
    }
    dest[i] = vec4<f32>(out, s.a);
}
