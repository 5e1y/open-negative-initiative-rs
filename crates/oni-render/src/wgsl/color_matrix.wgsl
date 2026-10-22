// `CIColorMatrix` on an opaque image: each channel a dot with its vector, plus the bias.

struct Params { head: vec4<u32>, r: vec4<f32>, g: vec4<f32>, b: vec4<f32>, a: vec4<f32>, bias: vec4<f32> }

fn color_matrix_px(s: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(dot(s, P.r), dot(s, P.g), dot(s, P.b), dot(s, P.a)) + P.bias;
}
