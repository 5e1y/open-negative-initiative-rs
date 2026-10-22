// `curves`: the channels' tables, the linked table, then the luma table as one gain on all three.
// Each table is read linearly at `v * (size - 1)`, as Core Image samples a texel row inset by half.

struct Params { head: vec4<u32>, bent: vec4<f32> }

const LUMA_SCALE_MAX: f32 = 8.0;

// The tables are read through `curves_rgb_lut(k)` and `curves_luma_lut(k)`, which each entry point defines.
fn rgb_at(v: f32) -> vec4<f32> {
    let n = P.head.w;
    let f = clamp(v, 0.0, 1.0) * f32(n - 1u);
    let k = min(u32(floor(f)), n - 1u);
    return mix(curves_rgb_lut(k), curves_rgb_lut(min(k + 1u, n - 1u)), f - f32(k));
}

fn luma_at(v: f32) -> f32 {
    let n = P.head.w;
    let f = clamp(v, 0.0, 1.0) * f32(n - 1u);
    let k = min(u32(floor(f)), n - 1u);
    return mix(curves_luma_lut(k).r, curves_luma_lut(min(k + 1u, n - 1u)).r, f - f32(k));
}

// Past 1 the curve continues as a line of slope 1 instead of clipping.
fn curve_rgb(v: f32, c: u32) -> f32 {
    if (v <= 1.0) { return rgb_at(v)[c]; }
    return rgb_at(1.0)[c] + (v - 1.0);
}

fn curve_luma(v: f32) -> f32 {
    if (v <= 1.0) { return luma_at(v); }
    return luma_at(1.0) + (v - 1.0);
}

fn curves_px(s: vec4<f32>) -> vec4<f32> {
    var e = s.rgb;
    if (P.bent.x > 0.5) {
        e = vec3<f32>(curve_rgb(e.r, 1u), curve_rgb(e.g, 2u), curve_rgb(e.b, 3u));
    }
    if (P.bent.y > 0.5) {
        e = vec3<f32>(curve_rgb(e.r, 0u), curve_rgb(e.g, 0u), curve_rgb(e.b, 0u));
    }
    let y = dot(e, LUMA);
    if (P.bent.z > 0.5 && y > 1e-4) {
        e = e * clamp(curve_luma(y) / y, 0.0, LUMA_SCALE_MAX);
    }
    return vec4<f32>(e, s.a);
}
