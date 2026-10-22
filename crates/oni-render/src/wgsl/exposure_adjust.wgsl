// `CIExposureAdjust`: the colour times `2^ev`, alpha kept.

struct Params { head: vec4<u32>, ev: vec4<f32> }

fn exposure_adjust_px(s: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(s.rgb * exp2(P.ev.x), s.a);
}
