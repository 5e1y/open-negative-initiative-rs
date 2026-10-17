// `vignette`: a radial darkening in stops on an ellipse following the frame, then Hunt: a power on
// the channels raises a darkened corner's chroma by the light it lost, the luminance restored.

struct Params {
    head: vec4<u32>,
    origin: vec4<f32>,
    centre_half_side: vec4<f32>,
    dose_hunt: vec4<f32>,
}

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> src_px: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> dest: array<vec4<f32>>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let i = flat_index(id, groups);
    if (i >= P.head.x) { return; }
    let s = src_px[i];
    let at = ci_coord(i % P.head.y, i / P.head.y, P.origin.xy, P.head.z);
    let dose = P.dose_hunt.xy;
    let hunt = P.dose_hunt.z;
    let d = (at - P.centre_half_side.xy) / P.centre_half_side.zw;
    let r = length(d) * 0.70710678;
    let t = max((r - dose.y) / (1.0 - dose.y), 0.0);
    let k = exp2(-dose.x * t * t);
    var e = s.rgb * k;
    // pow(1, y) is exactly 1 in C and Metal, not in every WGSL implementation: the flat centre needs
    // it, or its negative channels are floored.
    let base = clamp(k, 0.25, 4.0);
    let m = select(pow(base, -hunt), 1.0, base == 1.0);
    if (m != 1.0) {
        let p = pow(max(e, vec3<f32>(0.0)), vec3<f32>(m));
        let yl = dot(p, LUMA);
        if (yl > 1e-12) { e = p * pow(yl, 1.0 / m - 1.0); }
    }
    dest[i] = vec4<f32>(e, s.a);
}
