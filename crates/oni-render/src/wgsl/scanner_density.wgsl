// `scannerDensity`: ONI's own density at the roll's gains, separated, offset from the roll's median
// onto the table's neutral and the clicks, then placed on the cube's domain.

struct Params {
    head: vec4<u32>,
    gain_pedestal: vec4<f32>,
    median_guard: vec4<f32>,
    sep_r_scale: vec4<f32>,
    sep_g_low: vec4<f32>,
    sep_b_span: vec4<f32>,
    shift_invert: vec4<f32>,
    reached_low_dmax: vec4<f32>,
    reached_high: vec4<f32>,
}

fn scanner_density_px(s: vec4<f32>) -> vec4<f32> {
    let t = (s.rgb + P.gain_pedestal.w) * P.gain_pedestal.xyz;
    let density = -log(max(t, vec3<f32>(P.median_guard.w))) * LOG10_E;
    // A slide re-inverts on `dmax` as Pro's stage 6 does.
    let e = select(P.reached_low_dmax.w - density, density, P.shift_invert.w > 0.5);
    let d = e - P.median_guard.xyz;
    let x = P.sep_r_scale.w * vec3<f32>(dot(P.sep_r_scale.xyz, d), dot(P.sep_g_low.xyz, d), dot(P.sep_b_span.xyz, d))
          + P.shift_invert.xyz;
    let held = clamp(x, P.reached_low_dmax.xyz, P.reached_high.xyz);
    return vec4<f32>(clamp((held - P.sep_g_low.w) / P.sep_b_span.w, vec3<f32>(0.0), vec3<f32>(1.0)), s.a);
}

// MARK: - Buffers

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> src_px: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> dest: array<vec4<f32>>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let i = flat_index(id, groups);
    if (i >= P.head.x) { return; }
    dest[i] = scanner_density_px(src_px[i]);
}
