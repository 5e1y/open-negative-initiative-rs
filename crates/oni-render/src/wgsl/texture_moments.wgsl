// `textureMoments` as its own pass; needs `TEXTURE_MATH`.

struct Params { head: vec4<u32> }

fn texture_moments_px(luma: vec4<f32>) -> vec4<f32> {
    return texture_moments_of(luma);
}
