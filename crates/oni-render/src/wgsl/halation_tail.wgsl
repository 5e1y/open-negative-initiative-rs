// `halation_tail`: the glow of one pixel, its light spread by three Gaussians weighed and summed, which decay as an
// exponential of the distance; alpha zero. The weights are `HALATION_TAIL`'s.

struct Params { head: vec4<u32> }

fn halation_tail_px(near: vec4<f32>, mid: vec4<f32>, far: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(0.14147 * near.rgb + 0.67379 * mid.rgb + 0.18474 * far.rgb, 0.0);
}
