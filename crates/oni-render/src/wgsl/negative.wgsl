// Lighting on the negative, operation for operation as `cpu::negative`: the density above the bare film read back to
// each layer's log exposure through the couplers, moved, developed again, toned, then coded as the print's light.

struct Params {
    head: vec4<u32>,
    gain_pedestal: vec4<f32>,
    black_guard: vec4<f32>,
    pivot_sense: vec4<f32>,
    slope_low: vec4<f32>,
    shift_step: vec4<f32>,
    span_origin: vec4<f32>,
    points_black_shadows: vec4<f32>,
    points_white_highlights: vec4<f32>,
    mids: vec4<f32>,
    tone_low: vec4<f32>,
    tone_step: vec4<f32>,
    couplers: array<vec4<f32>, 3>,
    foot: vec4<f32>,
    top: vec4<f32>,
    own_pivot: vec4<f32>,
    own_slope: vec4<f32>,
    own_shift: vec4<f32>,
    normal: array<vec4<f32>, 256>,
    after: array<vec4<f32>, 256>,
    tone: array<vec4<f32>, 128>,
}

const NEGATIVE_CURVE_NODES: u32 = 256u;
const NEGATIVE_STEPS: u32 = 4u;
const NEGATIVE_TONE_NODES: f32 = 128.0;

// Channel `c` of the normal curve (`after` false) or the pulled one, and its slope: held at its first node under it,
// straight on past its last.
fn negative_at(after: bool, e: f32, c: u32) -> vec2<f32> {
    let f = (e - P.slope_low.w) / P.shift_step.w;
    if (f < 0.0) {
        if (after) { return vec2<f32>(P.after[0][c], 0.0); }
        return vec2<f32>(P.normal[0][c], 0.0);
    }
    let i = min(floor(f), f32(NEGATIVE_CURVE_NODES) - 2.0);
    let at = u32(i);
    var a = P.normal[at][c];
    var b = P.normal[at + 1u][c];
    if (after) {
        a = P.after[at][c];
        b = P.after[at + 1u][c];
    }
    return vec2<f32>(mix(a, b, f - i), (b - a) / P.shift_step.w);
}

fn negative_back(density: f32, c: u32) -> f32 {
    var low = 0u;
    var high = NEGATIVE_CURVE_NODES - 1u;
    while (high - low > 1u) {
        let mid = (low + high) / 2u;
        if (P.after[mid][c] <= density) { low = mid; } else { high = mid; }
    }
    let a = P.after[low][c];
    let b = P.after[low + 1u][c];
    return P.slope_low.w + (f32(low) + (density - a) / (b - a)) * P.shift_step.w;
}

fn negative_normal(e: vec3<f32>) -> array<vec2<f32>, 3> {
    return array<vec2<f32>, 3>(negative_at(false, e.x, 0u), negative_at(false, e.y, 1u), negative_at(false, e.z, 2u));
}

fn negative_pull(n: array<vec2<f32>, 3>) -> vec3<f32> {
    let density = vec3<f32>(n[0].x, n[1].x, n[2].x);
    return vec3<f32>(
        dot(density, vec3<f32>(P.couplers[0].x, P.couplers[1].x, P.couplers[2].x)),
        dot(density, vec3<f32>(P.couplers[0].y, P.couplers[1].y, P.couplers[2].y)),
        dot(density, vec3<f32>(P.couplers[0].z, P.couplers[1].z, P.couplers[2].z)),
    );
}

fn negative_coupled() -> bool {
    return any(P.couplers[0].xyz != vec3<f32>(0.0)) || any(P.couplers[1].xyz != vec3<f32>(0.0)) || any(P.couplers[2].xyz != vec3<f32>(0.0));
}

fn negative_exposures(density: vec3<f32>) -> vec3<f32> {
    let inhibited = vec3<f32>(negative_back(density.x, 0u), negative_back(density.y, 1u), negative_back(density.z, 2u));
    if (!negative_coupled()) { return inhibited; }
    var e = inhibited + negative_pull(negative_normal(inhibited));
    for (var step = 0u; step < NEGATIVE_STEPS; step = step + 1u) {
        let n = negative_normal(e);
        let miss = e - negative_pull(n) - inhibited;
        let s = vec3<f32>(n[0].y, n[1].y, n[2].y);
        // Row m, column k: d(miss_m)/d(log_k).
        let r0 = vec3<f32>(1.0, 0.0, 0.0) - s * vec3<f32>(P.couplers[0].x, P.couplers[1].x, P.couplers[2].x);
        let r1 = vec3<f32>(0.0, 1.0, 0.0) - s * vec3<f32>(P.couplers[0].y, P.couplers[1].y, P.couplers[2].y);
        let r2 = vec3<f32>(0.0, 0.0, 1.0) - s * vec3<f32>(P.couplers[0].z, P.couplers[1].z, P.couplers[2].z);
        let ca = r1.y * r2.z - r1.z * r2.y;
        let cb = r1.z * r2.x - r1.x * r2.z;
        let cc = r1.x * r2.y - r1.y * r2.x;
        let det = r0.x * ca + r0.y * cb + r0.z * cc;
        let i0 = vec3<f32>(ca, r0.z * r2.y - r0.y * r2.z, r0.y * r1.z - r0.z * r1.y);
        let i1 = vec3<f32>(cb, r0.x * r2.z - r0.z * r2.x, r0.z * r1.x - r0.x * r1.z);
        let i2 = vec3<f32>(cc, r0.y * r2.x - r0.x * r2.y, r0.x * r1.y - r0.y * r1.x);
        e = e - vec3<f32>(dot(i0, miss), dot(i1, miss), dot(i2, miss)) / det;
    }
    return e;
}

// Each layer's density from its log exposure, the couplers' pull taken, then its own moves laid.
fn negative_developed(e: vec3<f32>) -> vec3<f32> {
    let pulled = e - negative_pull(negative_normal(e));
    let x = P.own_pivot.xyz + (pulled - P.own_pivot.xyz) * P.own_slope.xyz + P.own_shift.xyz;
    return vec3<f32>(negative_at(true, x.x, 0u).x, negative_at(true, x.y, 1u).x, negative_at(true, x.z, 2u).x);
}

fn negative_pass(p: f32, t: f32, high: bool) -> f32 {
    if (high) { return p + t * 4.0 * p * p * p * (1.0 - p); }
    return p + t * 4.0 * p * (1.0 - p) * (1.0 - p) * (1.0 - p);
}

fn negative_quartered(p: f32, travel: f32, high: bool) -> f32 {
    if (abs(travel) <= 0.25) { return negative_pass(p, travel, high); }
    let first = select(-0.25, 0.25, travel > 0.0);
    return negative_pass(negative_pass(p, first, high), travel - first, high);
}

fn negative_midded(p: f32, mids: f32) -> f32 {
    if (mids == 0.0 || p < 0.0 || p > 1.0) { return p; }
    return p + mids * 16.0 * p * p * (1.0 - p) * (1.0 - p);
}

fn negative_toned(density: vec3<f32>) -> vec3<f32> {
    let span = P.span_origin.xyz;
    let origin = P.span_origin.w;
    let black = P.points_black_shadows.xyz;
    let white = P.points_white_highlights.xyz;
    let zones = vec2<f32>(P.points_black_shadows.w, P.points_white_highlights.w);
    let mids = P.mids.xyz;
    if (all(black == vec3<f32>(0.0)) && all(white == vec3<f32>(0.0)) && all(zones == vec2<f32>(0.0)) && all(mids == vec3<f32>(0.0))) {
        return density;
    }
    let u = (vec3<f32>(origin) + density / span - black) / (vec3<f32>(1.0) - black - white);
    let tone = (u.x + u.y + u.z) / 3.0;
    var lift = 0.0;
    if (any(zones != vec2<f32>(0.0)) && tone >= 0.0 && tone <= 1.0) {
        lift = negative_quartered(negative_quartered(tone, zones.x, false), zones.y, true) - tone;
    }
    let v = u + vec3<f32>(lift);
    let toned = vec3<f32>(negative_midded(v.x, mids.x), negative_midded(v.y, mids.y), negative_midded(v.z, mids.z));
    return (toned - vec3<f32>(origin)) * span;
}

fn negative_px(s: vec4<f32>) -> vec4<f32> {
    let sense = P.pivot_sense.w;
    let guard = P.black_guard.w;
    var density = sense * (-log(max((s.rgb + P.gain_pedestal.w) * P.gain_pedestal.xyz, vec3<f32>(guard))) * LOG10_E - P.black_guard.xyz);
    let slope = P.slope_low.xyz;
    let shift = P.shift_step.xyz;
    let own = any(P.own_slope.xyz != vec3<f32>(1.0)) || any(P.own_shift.xyz != vec3<f32>(0.0));
    if (any(slope != vec3<f32>(1.0)) || any(shift != vec3<f32>(0.0)) || own) {
        // Over the shoulder a density takes the shoulder's move; under the toe's foot the foot's, less the nearer the fog.
        let read = min(max(density, P.foot.xyz), P.top.xyz);
        let e = negative_exposures(read);
        let pivot = P.pivot_sense.xyz;
        let fog = P.after[0].xyz;
        let share = select(vec3<f32>(1.0), clamp((density - fog) / max(P.foot.xyz - fog, vec3<f32>(1e-6)), vec3<f32>(0.0), vec3<f32>(1.0)), density < P.foot.xyz);
        density = density + share * (negative_developed(pivot + (e - pivot) * slope + shift) - read);
    }
    let toned = negative_toned(sense * density);
    let f = (toned - P.tone_low.xyz) / P.tone_step.xyz;
    let i = clamp(floor(f), vec3<f32>(0.0), vec3<f32>(NEGATIVE_TONE_NODES - 2.0));
    let along = f - i;
    let at = vec3<u32>(i);
    let printed = vec3<f32>(
        mix(P.tone[at.x].x, P.tone[at.x + 1u].x, along.x),
        mix(P.tone[at.y].y, P.tone[at.y + 1u].y, along.y),
        mix(P.tone[at.z].z, P.tone[at.z + 1u].z, along.z),
    );
    return vec4<f32>(exp2(-3.3219280948873623 * printed), s.a);
}
