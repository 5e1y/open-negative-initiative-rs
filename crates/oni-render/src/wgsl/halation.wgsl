// `halation`: each layer's exposure joined by `dose` of its blur, over one and the dose, so an even exposure
// stays as it is; alpha kept.

struct Params { head: vec4<u32>, dose: vec4<f32> }

fn halation_px(e: vec4<f32>, blurred: vec4<f32>) -> vec4<f32> {
    return vec4<f32>((e.rgb + P.dose.rgb * blurred.rgb) / (1.0 + P.dose.rgb), e.a);
}
