// The paper, operation for operation as `cpu::paper`: Lighting's tones on the negative's density, the enlarger's
// light through its dyes on the paper's layers, the grade, its curves, its dyes seen under D50 in Rec. 2020, then
// Brightness and True blacks between the paper's Dmax and white; alpha kept.

struct Params {
    head: vec4<u32>,
    gain_pedestal: vec4<f32>,
    black_guard: vec4<f32>,
    span_shadows: vec4<f32>,
    points_black_highlights: vec4<f32>,
    points_white: vec4<f32>,
    mids: vec4<f32>,
    filters_scale: vec4<f32>,
    pivot: vec4<f32>,
    slope: vec4<f32>,
    curve_axis: vec4<f32>,
    dmax_true_blacks: vec4<f32>,
    white: vec4<f32>,
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

fn paper_pass(p: f32, t: f32, high: bool) -> f32 {
    if (high) { return p + t * 4.0 * p * p * p * (1.0 - p); }
    return p + t * 4.0 * p * (1.0 - p) * (1.0 - p) * (1.0 - p);
}

fn paper_quartered(p: f32, travel: f32, high: bool) -> f32 {
    if (abs(travel) <= 0.25) { return paper_pass(p, travel, high); }
    let first = select(-0.25, 0.25, travel > 0.0);
    return paper_pass(paper_pass(p, first, high), travel - first, high);
}

fn paper_midded(p: f32, mids: f32) -> f32 {
    if (mids == 0.0 || p < 0.0 || p > 1.0) { return p; }
    return p + mids * 16.0 * p * p * (1.0 - p) * (1.0 - p);
}

fn paper_toned(amounts: vec3<f32>) -> vec3<f32> {
    let span = P.span_shadows.xyz;
    let black = P.points_black_highlights.xyz;
    let white = P.points_white.xyz;
    let zones = vec2<f32>(P.span_shadows.w, P.points_black_highlights.w);
    let mids = P.mids.xyz;
    if (all(black == vec3<f32>(0.0)) && all(white == vec3<f32>(0.0)) && all(zones == vec2<f32>(0.0)) && all(mids == vec3<f32>(0.0))) {
        return amounts;
    }
    let u = (amounts / span - black) / (vec3<f32>(1.0) - black - white);
    let tone = (u.x + u.y + u.z) / 3.0;
    var lift = 0.0;
    if (any(zones != vec2<f32>(0.0)) && tone >= 0.0 && tone <= 1.0) {
        lift = paper_quartered(paper_quartered(tone, zones.x, false), zones.y, true) - tone;
    }
    let v = u + vec3<f32>(lift);
    return vec3<f32>(paper_midded(v.x, mids.x), paper_midded(v.y, mids.y), paper_midded(v.z, mids.z)) * span;
}

fn paper_px(s: vec4<f32>) -> vec4<f32> {
    let guard = P.black_guard.w;
    let read = -log(max((s.rgb + P.gain_pedestal.w) * P.gain_pedestal.xyz, vec3<f32>(guard))) * LOG10_E - P.black_guard.xyz;
    let amounts = paper_toned(read);
    let filters = P.filters_scale.xyz;
    var raw = vec3<f32>(0.0);
    for (var k = 0u; k < P.head.x; k = k + 1u) {
        let dye = P.negative[k];
        let dimmed = vec3<f32>(1.0) - (vec3<f32>(1.0) - P.dichroic[k].xyz) * (vec3<f32>(1.0) - filters);
        let passed = exp2(-3.3219280948873623 * (dot(amounts, dye.xyz) + dye.w)) * dimmed.x * dimmed.y * dimmed.z;
        raw = raw + passed * P.exposure[k].xyz;
    }
    var log_raw = log(max(raw, vec3<f32>(0.0)) * P.filters_scale.w + PAPER_EXPOSURE_FLOOR) * LOG10_E;
    if (any(P.slope.xyz != vec3<f32>(1.0))) {
        log_raw = P.pivot.xyz + (log_raw - P.pivot.xyz) * P.slope.xyz;
    }
    let density = vec3<f32>(paper_layer(log_raw.x, 0u), paper_layer(log_raw.y, 1u), paper_layer(log_raw.z, 2u));
    var out = vec3<f32>(0.0);
    for (var k = 0u; k < P.head.y; k = k + 1u) {
        let dye = P.print[k];
        out = out + exp2(-3.3219280948873623 * (dot(density, dye.xyz) + dye.w)) * P.view[k].xyz;
    }
    if (P.curve_axis.w > 0.5) {
        out = vec3<f32>(dot(out, LUMA));
    }
    let brightness = P.curve_axis.z;
    let true_blacks = P.dmax_true_blacks.w > 0.5;
    if (brightness != 0.0 || true_blacks) {
        let dmax = P.dmax_true_blacks.xyz;
        let white = P.white.xyz;
        let g = exp2(brightness);
        let u = max((out - dmax) / (white - dmax), vec3<f32>(0.0));
        let floor_ = select(dmax, vec3<f32>(0.0), true_blacks);
        out = floor_ + (white - floor_) * (g * u / (vec3<f32>(1.0) + (g - 1.0) * u));
    }
    return vec4<f32>(out, s.a);
}
