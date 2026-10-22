// `texture`: the band between the narrow luma and its edge-aware base, scaled by `gain` on the luma alone.

struct Params { head: vec4<u32>, gain: vec4<f32> }

fn texture_px(s: vec4<f32>, narrow: vec4<f32>, c: vec4<f32>) -> vec4<f32> {
    let n = narrow.r;
    let y = dot(s.rgb, LUMA);
    let chroma = s.rgb - y;
    let base = c.r * n + c.g;
    let shaped = y + (n - base) * P.gain.x;
    return vec4<f32>(max(shaped, 0.0) + chroma, s.a);
}
