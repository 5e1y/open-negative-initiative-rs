// The paper, operation for operation as `cpu::paper`: Lighting's tones on the film's density, the enlarger's
// light through its dyes on the paper's layers, Brightness above its knee, the grade, its curves, its dyes seen under
// D50 in Rec. 2020, its colour density, then its ends: the pick on its anchor, the white held; alpha kept.

struct Params {
    head: vec4<u32>,
    gain_pedestal: vec4<f32>,
    black_guard: vec4<f32>,
    span_shadows: vec4<f32>,
    points_black_highlights: vec4<f32>,
    points_white_reversal: vec4<f32>,
    mids: vec4<f32>,
    filters_scale: vec4<f32>,
    pivot_pick: vec4<f32>,
    slope_knee: vec4<f32>,
    curve_axis: vec4<f32>,
    pick_density: vec4<f32>,
    white: vec4<f32>,
    anchor: vec4<f32>,
    negative: array<vec4<f32>, 81>,
    exposure: array<vec4<f32>, 81>,
    dichroic: array<vec4<f32>, 81>,
    print: array<vec4<f32>, 81>,
    view: array<vec4<f32>, 81>,
    curves: array<vec4<f32>, 256>,
}

const PAPER_CURVE_NODES: f32 = 256.0;
const PAPER_EXPOSURE_FLOOR: f32 = 1e-10;
const PAPER_KNEE_SLOPE: f32 = 4.0;

fn paper_excess(x: f32, knee: f32) -> f32 {
    let y = x - knee;
    if (y <= 0.0) { return 0.0; }
    if (y < knee) { return y * y / (2.0 * knee); }
    return y - 0.5 * knee;
}

fn paper_kneed(x: f32, lift: f32, knee: f32) -> f32 {
    let e = paper_excess(x, knee);
    let gain = PAPER_KNEE_SLOPE - 1.0;
    if (lift >= 0.0) {
        if (e < 2.0 * lift / gain) { return x + gain * e - gain * gain * e * e / (4.0 * lift); }
        return x + lift;
    }
    let fall = -lift;
    let steep = PAPER_KNEE_SLOPE;
    if (e < (steep + 1.0) * fall / gain) {
        return x + 2.0 * e / (steep + sqrt(steep * steep - gain * gain * e / fall)) - e;
    }
    return x + lift;
}

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
    let white = P.points_white_reversal.xyz;
    let zones = vec2<f32>(P.span_shadows.w, P.points_black_highlights.w);
    let mids = P.mids.xyz;
    if (all(black == vec3<f32>(0.0)) && all(white == vec3<f32>(0.0)) && all(zones == vec2<f32>(0.0)) && all(mids == vec3<f32>(0.0))) {
        return amounts;
    }
    let reversal = P.points_white_reversal.w > 0.5;
    let read = select(amounts / span, vec3<f32>(1.0) - amounts / span, reversal);
    let u = (read - black) / (vec3<f32>(1.0) - black - white);
    let tone = (u.x + u.y + u.z) / 3.0;
    var lift = 0.0;
    if (any(zones != vec2<f32>(0.0)) && tone >= 0.0 && tone <= 1.0) {
        lift = paper_quartered(paper_quartered(tone, zones.x, false), zones.y, true) - tone;
    }
    let v = u + vec3<f32>(lift);
    let toned = vec3<f32>(paper_midded(v.x, mids.x), paper_midded(v.y, mids.y), paper_midded(v.z, mids.z));
    return select(toned, vec3<f32>(1.0) - toned, reversal) * span;
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
    let brightness = P.curve_axis.z;
    if (brightness != 0.0) {
        let sense = select(1.0, -1.0, P.points_white_reversal.w > 0.5);
        let x = sense * (P.pivot_pick.w - (log_raw.x + log_raw.y + log_raw.z) / 3.0);
        log_raw = log_raw - vec3<f32>(sense * (paper_kneed(x, brightness * 0.3010299956639812, P.slope_knee.w) - x));
    }
    if (any(P.slope_knee.xyz != vec3<f32>(1.0))) {
        log_raw = P.pivot_pick.xyz + (log_raw - P.pivot_pick.xyz) * P.slope_knee.xyz;
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
    let colour = P.pick_density.w;
    if (colour != 1.0) {
        let pw = pow(max(out, vec3<f32>(0.0)), vec3<f32>(colour));
        let p = dot(pw, LUMA);
        if (p > 1e-12) { out = pw * pow(p, (1.0 / colour) - 1.0); }
    }
    let pick = P.pick_density.xyz;
    let anchor = P.anchor.xyz;
    if (any(anchor != pick)) {
        let white = P.white.xyz;
        let above = anchor + (white - anchor) * ((out - pick) / (white - pick));
        out = select(above, out * anchor / pick, out <= pick);
    }
    return vec4<f32>(out, s.a);
}
