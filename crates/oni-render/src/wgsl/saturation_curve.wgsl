// `saturationCurve`: saturation by luminance, a natural cubic spline through five points on
// perceived light (gamma 2.2); the chroma scaled, its luminance taken back out.

struct Params {
    head: vec4<u32>,
    knots_high: vec4<f32>,
    lows: vec4<f32>,
    bends: vec4<f32>,
}

fn saturation_curve_px(s: vec4<f32>) -> vec4<f32> {
    let knots = P.knots_high.xyz;
    let y = dot(s.rgb, LUMA);
    let x = clamp(pow(max(y, 0.0), 1.0 / 2.2), 0.0, 1.0);
    var xs = array<f32, 5>(0.0, knots.x, knots.y, knots.z, 1.0);
    var ys = array<f32, 5>(P.lows.x, P.lows.y, P.lows.z, P.lows.w, P.knots_high.w);
    var ms = array<f32, 5>(0.0, P.bends.x, P.bends.y, P.bends.z, 0.0);
    let k = select(select(select(3, 2, x < xs[3]), 1, x < xs[2]), 0, x < xs[1]);
    let h = max(xs[k + 1] - xs[k], 1e-6);
    let t = x - xs[k];
    let b = (ys[k + 1] - ys[k]) / h - h * (2.0 * ms[k] + ms[k + 1]) / 6.0;
    let value = ys[k] + b * t + ms[k] / 2.0 * t * t + (ms[k + 1] - ms[k]) / (6.0 * h) * t * t * t;
    let factor = max(2.0 * value, 0.0);
    var chroma = (s.rgb - y) * factor;
    chroma = chroma - dot(chroma, LUMA);
    return vec4<f32>(y + chroma, s.a);
}

// MARK: - Buffers

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> src_px: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> dest: array<vec4<f32>>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let i = flat_index(id, groups);
    if (i >= P.head.x) { return; }
    dest[i] = saturation_curve_px(src_px[i]);
}
