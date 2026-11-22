// The paper, operation for operation as `cpu::paper`: the enlarger's light through the negative's dyes on the paper's
// layers, Brightness faded in from the pick, its curves, its dyes seen under D50 in Rec. 2020, its colour density, then its
// ends: the pick on its anchor, the white held; alpha kept.

struct Params {
    head: vec4<u32>,
    gain_pedestal: vec4<f32>,
    black_guard: vec4<f32>,
    filters_scale: vec4<f32>,
    pick_reach: vec4<f32>,
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
fn paper_share(x: f32, width: f32) -> f32 {
    let t = clamp(x / width, 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

fn paper_lifted(x: f32, lift: f32, reach: f32) -> f32 {
    let width = max(reach, sqrt(abs(lift) * 0.3010299956639812));
    if (lift >= 0.0) { return x + lift * paper_share(x, width); }
    let fall = -lift;
    if (x <= 0.0) { return x; }
    if (x >= width + fall) { return x - fall; }
    var low = max(x - fall, 0.0);
    var high = min(x, width);
    for (var i = 0u; i < 24u; i = i + 1u) {
        let mid = 0.5 * (low + high);
        if (mid + fall * paper_share(mid, width) < x) { low = mid; } else { high = mid; }
    }
    return 0.5 * (low + high);
}

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
    var log_raw = log(max(raw, vec3<f32>(0.0)) * P.filters_scale.w + PAPER_EXPOSURE_FLOOR) * LOG10_E;
    let brightness = P.curve_axis.z;
    if (brightness != 0.0) {
        let x = P.pick_reach.x - (log_raw.x + log_raw.y + log_raw.z) / 3.0;
        log_raw = log_raw - vec3<f32>(paper_lifted(x, brightness * 0.3010299956639812, P.pick_reach.y) - x);
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
