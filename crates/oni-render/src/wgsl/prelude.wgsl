// Shared by every kernel: Rec. 2020 luminance, the display gamma, the contrast family of
// `Contrast.h`, and the conventions every binding follows.

const LUMA = vec3<f32>(0.2627, 0.6780, 0.0593);
const DISPLAY_GAMMA: f32 = 2.2;
const CONTRAST_WARP: f32 = 0.88927292;
const LOG10_E: f32 = 0.43429448190325176;
const WORKGROUP: u32 = 64u;

// The invocation's flat index: past 65535 workgroups a dispatch folds onto its second dimension.
fn flat_index(id: vec3<u32>, groups: vec3<u32>) -> u32 {
    return id.x + id.y * groups.x * WORKGROUP;
}

// Core Image's coordinate of a pixel's centre: buffers hold the top row first, Core Image's y runs up.
fn ci_coord(x: u32, y: u32, origin: vec2<f32>, height: u32) -> vec2<f32> {
    return vec2<f32>(origin.x + f32(x) + 0.5, origin.y + f32(height - 1u - y) + 0.5);
}

fn infinity() -> f32 {
    return bitcast<f32>(0x7f800000u);
}

// The contrast family, pinned at 0 and 1 and at the pivot, as `Contrast.h` writes it.
fn contrasted(x: f32, travel: f32) -> f32 {
    let b = abs(travel) * 15.2;
    if (b <= 1e-4) { return x; }
    var t = pow(clamp(x, 0.0, 1.0), CONTRAST_WARP);
    let low = 1.0 / (1.0 + exp(b * 0.5));
    let high = 1.0 / (1.0 + exp(-b * 0.5));
    if (travel > 0.0) {
        t = (1.0 / (1.0 + exp(-b * (t - 0.5))) - low) / (high - low);
    } else {
        let g = low + t * (high - low);
        t = clamp(0.5 - log(1.0 / g - 1.0) / b, 0.0, 1.0);
    }
    return pow(t, 1.0 / CONTRAST_WARP);
}

