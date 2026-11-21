// The paper, operation for operation as `cpu::paper`: the enlarger's light through the negative's dyes on the
// paper's layers, its curves, its dyes seen under D50 in Rec. 2020, then its window; alpha kept.

struct Params {
    head: vec4<u32>,
    gain_pedestal: vec4<f32>,
    black_guard: vec4<f32>,
    filters_scale: vec4<f32>,
    curve_axis: vec4<f32>,
    window_black_luma: vec4<f32>,
    window_white: vec4<f32>,
    window_gamma: vec4<f32>,
    negative: array<vec4<f32>, 81>,
    exposure: array<vec4<f32>, 81>,
    dichroic: array<vec4<f32>, 81>,
    print: array<vec4<f32>, 81>,
    view: array<vec4<f32>, 81>,
    curves: array<vec4<f32>, 256>,
}

const PAPER_CURVE_NODES: f32 = 256.0;
const PAPER_EXPOSURE_FLOOR: f32 = 1e-10;

fn paper_layer(log: f32, c: u32) -> f32 {
    let f = clamp((log - P.curve_axis.x) / P.curve_axis.y, 0.0, PAPER_CURVE_NODES - 1.0);
    let i = min(floor(f), PAPER_CURVE_NODES - 2.0);
    let at = u32(i);
    return mix(P.curves[at][c], P.curves[at + 1u][c], f - i);
}

fn paper_px(s: vec4<f32>) -> vec4<f32> {
    let guard = P.black_guard.w;
    let amounts = -log(max((s.rgb + P.gain_pedestal.w) * P.gain_pedestal.xyz, vec3<f32>(guard))) * LOG10_E - P.black_guard.xyz;
    let filters = P.filters_scale.xyz;
    var raw = vec3<f32>(0.0);
    for (var k = 0u; k < P.head.x; k = k + 1u) {
        let dye = P.negative[k];
        let dimmed = vec3<f32>(1.0) - (vec3<f32>(1.0) - P.dichroic[k].xyz) * (vec3<f32>(1.0) - filters);
        let passed = exp2(-3.3219280948873623 * (dot(amounts, dye.xyz) + dye.w)) * dimmed.x * dimmed.y * dimmed.z;
        raw = raw + passed * P.exposure[k].xyz;
    }
    let log_raw = log(max(raw, vec3<f32>(0.0)) * P.filters_scale.w + PAPER_EXPOSURE_FLOOR) * LOG10_E;
    let density = vec3<f32>(paper_layer(log_raw.x, 0u), paper_layer(log_raw.y, 1u), paper_layer(log_raw.z, 2u));
    var out = vec3<f32>(0.0);
    for (var k = 0u; k < P.head.y; k = k + 1u) {
        let dye = P.print[k];
        out = out + exp2(-3.3219280948873623 * (dot(density, dye.xyz) + dye.w)) * P.view[k].xyz;
    }
    let black = P.window_black_luma.xyz;
    let read = select(out, vec3<f32>(dot(out, LUMA)), P.window_black_luma.w > 0.5);
    let windowed = pow(max((read - black) / (P.window_white.xyz - black), vec3<f32>(0.0)), P.window_gamma.xyz);
    return vec4<f32>(windowed, s.a);
}
