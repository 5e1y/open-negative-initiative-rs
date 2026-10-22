// `spectrogram`: eight hue bands, each turning, saturating and lifting the pixels it owns; the hue
// read in the plane every grey projects to a point of, faded in away from neutral.

struct Params {
    head: vec4<u32>,
    hue_low: vec4<f32>,
    hue_high: vec4<f32>,
    sat_low: vec4<f32>,
    sat_high: vec4<f32>,
    luma_low: vec4<f32>,
    luma_high: vec4<f32>,
}

// Fixed, so editing a band never changes which pixels it catches.
const BAND_CENTRE = array<f32, 8>(0.0, 30.0, 60.0, 120.0, 180.0, 240.0, 270.0, 300.0);
// How far past its neighbour a band owns a hue: stopping at its centre turns a hue edge into a luminance one.
const BAND_SPREAD: f32 = 1.8;
// A luma pull is a gain in stops, never `1 + x`, which reaches zero and prints black.
const LUMA_STOPS: f32 = 2.0;

fn band_weight(hue: f32, i: i32) -> f32 {
    var centres = BAND_CENTRE;
    let centre = centres[i];
    var delta = hue - centre;
    if (delta > 180.0) { delta -= 360.0; }
    if (delta < -180.0) { delta += 360.0; }
    let neighbour = select((i + 7) & 7, (i + 1) & 7, delta >= 0.0);
    var span = abs(centres[neighbour] - centre);
    if (span > 180.0) { span = 360.0 - span; }
    return smoothstep(0.0, 1.0, max(0.0, 1.0 - abs(delta) / (span * BAND_SPREAD)));
}

fn spectrogram_px(s: vec4<f32>) -> vec4<f32> {
    let y = dot(s.rgb, LUMA);
    let chroma = s.rgb - y;
    let a = dot(chroma, vec3<f32>(1.0, -0.5, -0.5));
    let b = dot(chroma, vec3<f32>(0.0, 0.8660254, -0.8660254));
    let presence = smoothstep(0.0, 0.10 + 0.25 * max(y, 0.0), length(vec2<f32>(a, b)));
    if (presence <= 0.0) {
        return s;
    }
    var hue = atan2(b, a) * 57.29577951;
    if (hue < 0.0) { hue += 360.0; }
    var w_low = vec4<f32>(band_weight(hue, 0), band_weight(hue, 1), band_weight(hue, 2), band_weight(hue, 3));
    var w_high = vec4<f32>(band_weight(hue, 4), band_weight(hue, 5), band_weight(hue, 6), band_weight(hue, 7));
    let total = dot(w_low, vec4<f32>(1.0)) + dot(w_high, vec4<f32>(1.0));
    if (total > 0.0) {
        w_low = w_low / total;
        w_high = w_high / total;
    }
    let shift = (dot(w_low, P.hue_low) + dot(w_high, P.hue_high)) * presence;
    let boost = (dot(w_low, P.sat_low) + dot(w_high, P.sat_high)) * presence;
    let lift = (dot(w_low, P.luma_low) + dot(w_high, P.luma_high)) * presence;

    // Rodrigues about the grey axis, then the arriving luminance re-imposed.
    let axis = vec3<f32>(0.5773502692);
    let angle = shift * 0.01745329252;
    let c = cos(angle);
    let sn = sin(angle);
    var turned = s.rgb * c + cross(axis, s.rgb) * sn + axis * dot(axis, s.rgb) * (1.0 - c);
    turned = turned + (y - dot(turned, LUMA));
    let mixed = (y + (turned - y) * max(1.0 + boost, 0.0)) * exp2(lift * LUMA_STOPS);
    return vec4<f32>(mixed, s.a);
}
