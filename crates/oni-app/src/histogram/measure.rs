//! `MetalImageView.encodeMeasure`: the picture just rendered, point-sampled to `SIDE` on its long
//! side, clipped to the screen's gamut and counted back in linear Rec. 2020, its luminance beside.

use crate::display::Screen;
use oni_measure::histogram::Histogram;

/// `measureSide`: the long side of the copy counted; four curves need a tenth of the pixels.
pub const SIDE: f64 = 640.0;
/// `HistogramShape.bins`.
pub const BINS: usize = 1024;
/// `Pipeline.lumaWeights`, Rec. 2020.
const LUMA: [f32; 3] = [0.2627, 0.6780, 0.0593];

/// The counted copy's size: the picture scaled to `SIDE` on its long side, never up.
pub fn grid(width: usize, height: usize) -> (usize, usize) {
    let factor = (SIDE / width.max(height).max(1) as f64).min(1.0);
    let side = |v: usize| ((v as f64 * factor).round() as usize).max(1);
    (side(width), side(height))
}

/// The source column or row a cell of the copy reads, nearest to its centre.
pub fn nearest(cell: usize, cells: usize, size: usize) -> usize {
    (((cell as f32 + 0.5) * size as f32 / cells as f32) as usize).min(size.saturating_sub(1))
}

/// `CIAreaHistogram`'s bin of a value, the ends holding everything past them.
pub fn bin(v: f32) -> usize {
    ((v.clamp(0.0, 1.0) * BINS as f32) as usize).min(BINS - 1)
}

/// One sample as the plot counts it: the screen's gamut clips it, then it is linear Rec. 2020 again.
pub fn counted(p: [f32; 4], to: &[[f32; 3]; 3], back: &[[f32; 3]; 3]) -> [f32; 3] {
    let shown: [f32; 3] = std::array::from_fn(|c| (to[c][0] * p[0] + to[c][1] * p[1] + to[c][2] * p[2]).clamp(0.0, 1.0));
    std::array::from_fn(|c| back[c][0] * shown[0] + back[c][1] * shown[1] + back[c][2] * shown[2])
}

/// The CPU reference of the display's histogram kernel, on a linear Rec. 2020 picture rows top first.
pub fn measure(pixels: &[[f32; 4]], width: usize, height: usize, screen: Screen) -> Histogram {
    let (to, back) = (screen.matrix(), screen.back());
    let (gw, gh) = grid(width, height);
    let mut rgb = vec![[0f32; 3]; BINS];
    let mut luma = vec![0f32; BINS];
    for gy in 0..gh {
        let y = nearest(gy, gh, height);
        for gx in 0..gw {
            let v = counted(pixels[y * width + nearest(gx, gw, width)], &to, &back);
            for c in 0..3 {
                rgb[bin(v[c])][c] += 1.0;
            }
            luma[bin(LUMA[0] * v[0] + LUMA[1] * v[1] + LUMA[2] * v[2])] += 1.0;
        }
    }
    Histogram { rgb, luma, ends: None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_copy_is_counted_at_640_and_a_gamut_the_screen_lacks_is_clipped() {
        assert_eq!(grid(6048, 4024), (640, 426));
        assert_eq!(grid(300, 200), (300, 200), "a small picture is counted whole");
        // A grey sits where it is; a saturated Rec. 2020 green outside sRGB comes back inside it.
        let grey = counted([0.5, 0.5, 0.5, 1.0], &Screen::Srgb.matrix(), &Screen::Srgb.back());
        assert!(grey.iter().all(|v| (v - 0.5).abs() < 1e-4), "{grey:?}");
        let green = [0.0, 0.8, 0.0, 1.0];
        let clipped = counted(green, &Screen::Srgb.matrix(), &Screen::Srgb.back());
        // Twin: without the screen's clip the green is counted where it lies.
        assert!((clipped[1] - 0.8).abs() > 1e-2 && clipped[0] > 1e-3, "{clipped:?}");
        let h = measure(&vec![[0.3, 0.3, 0.3, 1.0]; 2000 * 1000], 2000, 1000, Screen::Srgb);
        assert_eq!(h.rgb[bin(0.3)][1], (640 * 320) as f32);
        assert_eq!(h.luma[bin(0.3)], (640 * 320) as f32);
    }
}
