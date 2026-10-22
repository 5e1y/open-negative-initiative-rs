// `textureCoeff` as its own pass; needs `TEXTURE_MATH`.

struct Params { head: vec4<u32>, relative: vec4<f32> }

fn texture_coeff_px(m: vec4<f32>) -> vec4<f32> {
    return texture_coeff_of(m, P.relative.x);
}
