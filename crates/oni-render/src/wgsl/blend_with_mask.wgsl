// `CIBlendWithMask`: the background where the mask's green is 0, the image where it is 1.

struct Params { head: vec4<u32> }

fn blend_with_mask_px(picture: vec4<f32>, background: vec4<f32>, mask: vec4<f32>) -> vec4<f32> {
    return mix(background, picture, vec4<f32>(mask.g));
}
