// A virtual negative: the scene exposed on the film's light per chromaticity, developed on its curves
// with its DIR couplers, each layer's density coded as its own channel's light; alpha kept. Its exposure and its
// development are passes of their own where an effect lies between them.

struct Params {
    head: vec4<u32>,
    to_film: array<vec4<f32>, 3>,
    couplers: array<vec4<f32>, 3>,
}

// The table is read through `negative_table(i, j)`, which the entry point defines: the light per
// chromaticity on `head.x` square, then the two curve rows of `head.y` nodes.
fn negative_light(tc: vec2<f32>) -> vec3<f32> {
    let n = P.head.x;
    let f = tc * f32(n - 1u);
    let lo = min(vec2<u32>(floor(f)), vec2<u32>(n - 2u));
    let t = f - vec2<f32>(lo);
    let low = mix(negative_table(lo.x, lo.y).rgb, negative_table(lo.x + 1u, lo.y).rgb, t.x);
    let high = mix(negative_table(lo.x, lo.y + 1u).rgb, negative_table(lo.x + 1u, lo.y + 1u).rgb, t.x);
    return mix(low, high, t.y);
}

fn negative_curve(e: vec3<f32>, row: u32) -> vec3<f32> {
    let m = P.head.y;
    let y = P.head.x + row;
    let f = clamp((e - P.to_film[1].w) / P.to_film[2].w, vec3<f32>(0.0), vec3<f32>(1.0)) * f32(m - 1u);
    let lo = min(vec3<u32>(floor(f)), vec3<u32>(m - 2u));
    let t = f - vec3<f32>(lo);
    let r = mix(negative_table(lo.x, y).r, negative_table(lo.x + 1u, y).r, t.x);
    let g = mix(negative_table(lo.y, y).g, negative_table(lo.y + 1u, y).g, t.y);
    let b = mix(negative_table(lo.z, y).b, negative_table(lo.z + 1u, y).b, t.z);
    return vec3<f32>(r, g, b);
}

// The exposure a scene pixel lays on each layer, the film's own exposure in.
fn virtual_exposure_px(s: vec4<f32>) -> vec4<f32> {
    let xyz = vec3<f32>(dot(P.to_film[0].xyz, s.rgb), dot(P.to_film[1].xyz, s.rgb), dot(P.to_film[2].xyz, s.rgb));
    let b = xyz.x + xyz.y + xyz.z;
    let safe = max(b, 1e-10);
    let x = xyz.x / safe;
    let tc = clamp(vec2<f32>((1.0 - x) * (1.0 - x), (xyz.y / safe) / max(1.0 - x, 1e-10)), vec2<f32>(0.0), vec2<f32>(1.0));
    return vec4<f32>(negative_light(tc) * b * P.to_film[0].w, s.a);
}

// The layers' exposures developed on the curves, the couplers acting between them.
fn virtual_development_px(e: vec4<f32>) -> vec4<f32> {
    let log = log2(max(e.rgb, vec3<f32>(0.0)) + 1e-10) * 0.30102999566398120;
    let first = negative_curve(log, 0u);
    let c = P.couplers;
    let inhibited = log - (first.x * c[0].xyz + first.y * c[1].xyz + first.z * c[2].xyz);
    let density = negative_curve(inhibited, 1u);
    return vec4<f32>(exp2(-3.3219280948873623 * density), e.a);
}

fn virtual_negative_px(s: vec4<f32>) -> vec4<f32> {
    return virtual_development_px(virtual_exposure_px(s));
}
