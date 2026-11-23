// The colour tools on the negative, as `Negative::coloured_on`: each layer's density read as the log exposure a grey
// develops it at, its chroma about the luma's neutral spread by Saturation, turned and spread by the mixer's band of its
// hue, then by the curve at the print's lightness, the luminance held; then lit by the band's luma.

const NEGATIVE_COLOURS: bool = true;
const NEGATIVE_LIGHTNESS_NODES: f32 = 128.0;
const NEGATIVE_LIGHTNESS_LOW: f32 = -4.0;
const NEGATIVE_LIGHTNESS_HIGH: f32 = 4.0;
// The chroma, in log exposure, over which a colour comes into its hue's band.
const NEGATIVE_PRESENCE: f32 = 0.1;
// A band's luma pull at its track's end, in stops of the scene's light.
const NEGATIVE_LUMA_STOPS: f32 = 2.0;
// Fixed, so editing a band never changes which colours it catches.
const NEGATIVE_BAND_CENTRE = array<f32, 8>(0.0, 30.0, 60.0, 120.0, 180.0, 240.0, 270.0, 300.0);
// How far past its neighbour a band owns a hue: stopping at its centre turns a hue edge into a lightness one.
const NEGATIVE_BAND_SPREAD: f32 = 1.8;

fn negative_band(hue: f32, i: i32) -> f32 {
    var centres = NEGATIVE_BAND_CENTRE;
    let centre = centres[i];
    var delta = hue - centre;
    if (delta > 180.0) { delta -= 360.0; }
    if (delta < -180.0) { delta += 360.0; }
    let neighbour = select((i + 7) & 7, (i + 1) & 7, delta >= 0.0);
    var span = abs(centres[neighbour] - centre);
    if (span > 180.0) { span = 360.0 - span; }
    return smoothstep(0.0, 1.0, max(0.0, 1.0 - abs(delta) / (span * NEGATIVE_BAND_SPREAD)));
}

fn negative_lightness(n: f32) -> f32 {
    let step = (NEGATIVE_LIGHTNESS_HIGH - NEGATIVE_LIGHTNESS_LOW) / (NEGATIVE_LIGHTNESS_NODES - 1.0);
    let f = clamp((n - NEGATIVE_LIGHTNESS_LOW) / step, 0.0, NEGATIVE_LIGHTNESS_NODES - 1.0);
    let i = min(floor(f), NEGATIVE_LIGHTNESS_NODES - 2.0);
    let at = u32(i);
    let next = at + 1u;
    return mix(P.lightness[at / 4u][at % 4u], P.lightness[next / 4u][next % 4u], f - i);
}

// The saturation curve's factor at lightness `x`: its natural cubic spline, doubled.
fn negative_curve(x: f32) -> f32 {
    let knots = P.knots_high.xyz;
    var xs = array<f32, 5>(0.0, knots.x, knots.y, knots.z, 1.0);
    var ys = array<f32, 5>(P.lows.x, P.lows.y, P.lows.z, P.lows.w, P.knots_high.w);
    var ms = array<f32, 5>(0.0, P.bends.x, P.bends.y, P.bends.z, 0.0);
    let k = select(select(select(3, 2, x < xs[3]), 1, x < xs[2]), 0, x < xs[1]);
    let h = max(xs[k + 1] - xs[k], 1e-6);
    let t = x - xs[k];
    let b = (ys[k + 1] - ys[k]) / h - h * (2.0 * ms[k] + ms[k + 1]) / 6.0;
    let value = ys[k] + b * t + ms[k] / 2.0 * t * t + (ms[k + 1] - ms[k]) / (6.0 * h) * t * t * t;
    return max(2.0 * value, 0.0);
}

fn negative_back_normal(density: f32, c: u32) -> f32 {
    var low = 0u;
    var high = NEGATIVE_CURVE_NODES - 1u;
    while (high - low > 1u) {
        let mid = (low + high) / 2u;
        if (P.normal[mid][c] <= density) { low = mid; } else { high = mid; }
    }
    let a = P.normal[low][c];
    let b = P.normal[low + 1u][c];
    return P.slope_low.w + (f32(low) + (density - a) / (b - a)) * P.shift_step.w;
}

// What each layer develops on, read as the log exposure a grey develops its density at, its colour moved, read back.
fn negative_scene(x: vec3<f32>) -> vec3<f32> {
    let grey = vec3<f32>(
        negative_back_normal(negative_at(true, x.x, 0u).x, 0u),
        negative_back_normal(negative_at(true, x.y, 1u).x, 1u),
        negative_back_normal(negative_at(true, x.z, 2u).x, 2u),
    );
    let moved = negative_coloured(grey);
    return vec3<f32>(
        negative_back(negative_at(false, moved.x, 0u).x, 0u),
        negative_back(negative_at(false, moved.y, 1u).x, 1u),
        negative_back(negative_at(false, moved.z, 2u).x, 2u),
    );
}

// The grey exposures' colour moved by the colour tools: Saturation, the mixer's band of its hue, the curve at its
// lightness.
fn negative_coloured(e: vec3<f32>) -> vec3<f32> {
    let pivot = P.pivot_sense.xyz;
    let about = e - pivot;
    var neutral = dot(about, LUMA);
    let before = about - vec3<f32>(neutral);
    var chroma = before * P.colour.x;
    var lift = 0.0;
    if (P.colour.z > 0.5) {
        let a = dot(chroma, vec3<f32>(1.0, -0.5, -0.5));
        let b = dot(chroma, vec3<f32>(0.0, 0.8660254, -0.8660254));
        let presence = smoothstep(0.0, NEGATIVE_PRESENCE, length(vec2<f32>(a, b)));
        if (presence > 0.0) {
            var hue = atan2(b, a) * 57.29578;
            if (hue < 0.0) { hue += 360.0; }
            var w_low = vec4<f32>(negative_band(hue, 0), negative_band(hue, 1), negative_band(hue, 2), negative_band(hue, 3));
            var w_high = vec4<f32>(negative_band(hue, 4), negative_band(hue, 5), negative_band(hue, 6), negative_band(hue, 7));
            let total = dot(w_low, vec4<f32>(1.0)) + dot(w_high, vec4<f32>(1.0));
            if (total > 0.0) {
                w_low = w_low / total;
                w_high = w_high / total;
            }
            let shift = (dot(w_low, P.hue_low) + dot(w_high, P.hue_high)) * presence;
            let boost = (dot(w_low, P.sat_low) + dot(w_high, P.sat_high)) * presence;
            let light = (dot(w_low, P.luma_low) + dot(w_high, P.luma_high)) * presence;
            // Rodrigues about the grey axis, then the luma's weight taken back out, so the turn keeps the neutral.
            let axis = vec3<f32>(0.57735026);
            let angle = shift * 0.017453292;
            let cs = cos(angle);
            let sn = sin(angle);
            let turned = chroma * cs + cross(axis, chroma) * sn + axis * dot(axis, chroma) * (1.0 - cs);
            chroma = (turned - vec3<f32>(dot(turned, LUMA))) * max(1.0 + boost, 0.0);
            lift = light * NEGATIVE_LUMA_STOPS * 0.3010299956639812;
        }
    }
    if (P.colour.y > 0.5) {
        chroma = chroma * negative_curve(negative_lightness(neutral + lift));
    }
    // The scene's luminance as it was: a colour spread or greyed keeps its brightness.
    let was = dot(exp2(before * 3.3219280948873623), LUMA);
    let now = dot(exp2(chroma * 3.3219280948873623), LUMA);
    neutral = neutral + log2(was / now) * 0.3010299956639812 + lift;
    return pivot + vec3<f32>(neutral) + chroma;
}

