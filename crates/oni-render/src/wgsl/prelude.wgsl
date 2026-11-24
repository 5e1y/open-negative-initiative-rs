// Shared by every kernel: Rec. 2020 luminance and the display gamma.

const LUMA = vec3<f32>(0.2627, 0.6780, 0.0593);
const DISPLAY_GAMMA: f32 = 2.2;
const LOG10_E: f32 = 0.43429448190325176;

fn infinity() -> f32 {
    return bitcast<f32>(0x7f800000u);
}

