// `halation`: each layer's exposure joined by `dose` of its glow; alpha kept.

struct Params { head: vec4<u32>, dose: vec4<f32>, saturation: vec4<f32> }

fn halation_px(e: vec4<f32>, glow: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(e.rgb + P.dose.rgb * glow.rgb, e.a);
}
