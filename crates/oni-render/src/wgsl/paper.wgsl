// The paper, operation for operation as `cpu::paper`: the window's shares laid back on density, each layer exposed by its
// own channel through the time and its filter, Brightness faded in from the pick, its dyes on their curves, each channel
// reading them through the Status A crosstalk over the D-min, each density absorbing its own Rec. 2020 channel alone,
// then its ends: the pick and the paper's white each on its anchor; alpha kept.

struct Params {
    head: vec4<u32>,
    density_scale: vec4<f32>,
    filters: vec4<f32>,
    pick_reach: vec4<f32>,
    curve_axis: vec4<f32>,
    mix: array<vec4<f32>, 3>,
    pick: vec4<f32>,
    white: vec4<f32>,
    anchor: vec4<f32>,
    white_anchor: vec4<f32>,
    floor: vec4<f32>,
    crosstalk: array<vec4<f32>, 3>,
    curves: array<vec4<f32>, 256>,
}

const PAPER_CURVE_NODES: f32 = 256.0;
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

fn paper_px(t: vec4<f32>) -> vec4<f32> {
    let span = P.density_scale.x;
    let bend = P.density_scale.y;
    let d = span * sign(t.rgb) * pow(abs(t.rgb), vec3<f32>(bend));
    let mixed = vec3<f32>(dot(P.mix[0].xyz, d), dot(P.mix[1].xyz, d), dot(P.mix[2].xyz, d));
    var log_raw = log(P.density_scale.z * P.filters.xyz) * LOG10_E - mixed;
    let brightness = P.pick_reach.z;
    if (brightness != 0.0) {
        let x = P.pick_reach.x - (log_raw.x + log_raw.y + log_raw.z) / 3.0;
        log_raw = log_raw - vec3<f32>(paper_lifted(x, brightness * 0.3010299956639812, P.pick_reach.y) - x);
    }
    let amounts = vec3<f32>(paper_layer(log_raw.x, 0u), paper_layer(log_raw.y, 1u), paper_layer(log_raw.z, 2u));
    let density = P.floor.xyz + vec3<f32>(dot(P.crosstalk[0].xyz, amounts), dot(P.crosstalk[1].xyz, amounts), dot(P.crosstalk[2].xyz, amounts));
    var out = exp2(-3.3219280948873623 * density);
    if (P.density_scale.w > 0.5) {
        out = vec3<f32>(dot(out, LUMA));
    }
    let pick = P.pick.xyz;
    let anchor = P.anchor.xyz;
    let white = P.white.xyz;
    let white_anchor = P.white_anchor.xyz;
    if (any(anchor != pick) || any(white_anchor != white)) {
        let above = anchor + (white_anchor - anchor) * ((out - pick) / (white - pick));
        out = select(above, out * anchor / pick, out <= pick);
    }
    return vec4<f32>(out, t.a);
}
