//! A scene's backdrop: its photo brought down to a few dozen pixels and blurred, which the frame draws
//! large behind everything, so each scene glows in its own photo's light on the black.

/// The backdrop's long side, in pixels: drawn a thousand wide it holds no detail, only light.
pub const SIDE: usize = 40;
/// The blur's reach and its passes: three passes of a box are near a Gaussian.
const REACH: usize = 3;
const PASSES: usize = 3;

/// `rgba`, `width` by `height`, as its backdrop: the means of its blocks, blurred, with its size.
pub fn blurred(rgba: &[u8], width: usize, height: usize) -> Option<(Vec<u8>, usize, usize)> {
    if width == 0 || height == 0 || rgba.len() != width * height * 4 {
        return None;
    }
    let scale = (width.max(height) as f32 / SIDE as f32).max(1.0);
    let (w, h) = (((width as f32 / scale).round() as usize).max(1), ((height as f32 / scale).round() as usize).max(1));
    // Each small pixel is the mean of the block of the photo it covers.
    let mut small = vec![[0.0f32; 3]; w * h];
    for (y, row) in small.chunks_mut(w).enumerate() {
        let (top, bottom) = (y * height / h, ((y + 1) * height / h).max(y * height / h + 1));
        for (x, pixel) in row.iter_mut().enumerate() {
            let (left, right) = (x * width / w, ((x + 1) * width / w).max(x * width / w + 1));
            let mut sum = [0.0f32; 3];
            for sy in top..bottom {
                for sx in left..right {
                    let at = (sy * width + sx) * 4;
                    for (s, v) in sum.iter_mut().zip(&rgba[at..at + 3]) {
                        *s += f32::from(*v);
                    }
                }
            }
            let count = ((bottom - top) * (right - left)) as f32;
            *pixel = sum.map(|s| s / count);
        }
    }
    for _ in 0..PASSES {
        small = boxed(&small, w, h, true);
        small = boxed(&small, w, h, false);
    }
    let out = small.iter().flat_map(|p| [p[0].round() as u8, p[1].round() as u8, p[2].round() as u8, 255]).collect();
    Some((out, w, h))
}

/// One pass of a box blur along the rows or the columns, the edge pixels standing for what lies past them.
fn boxed(pixels: &[[f32; 3]], w: usize, h: usize, along_rows: bool) -> Vec<[f32; 3]> {
    let (span, lines) = if along_rows { (w, h) } else { (h, w) };
    let at = |line: usize, i: usize| if along_rows { line * w + i } else { i * w + line };
    let mut out = pixels.to_vec();
    for line in 0..lines {
        for i in 0..span {
            let mut sum = [0.0f32; 3];
            for k in 0..=2 * REACH {
                let j = (i + k).saturating_sub(REACH).min(span - 1);
                for (s, v) in sum.iter_mut().zip(pixels[at(line, j)]) {
                    *s += v;
                }
            }
            out[at(line, i)] = sum.map(|s| s / (2 * REACH + 1) as f32);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_backdrop_keeps_a_photos_light_and_none_of_its_detail() {
        // A photo half red, half blue, with a white line a pixel wide across it.
        let (w, h) = (400, 200);
        let rgba: Vec<u8> = (0..h).flat_map(|y| (0..w).flat_map(move |x| if y == 100 { [255, 255, 255, 255] } else if x < w / 2 { [200, 20, 20, 255] } else { [20, 20, 200, 255] })).collect();
        let (small, sw, sh) = blurred(&rgba, w, h).unwrap();
        assert_eq!((sw, sh), (40, 20));
        let at = |x: usize, y: usize| &small[(y * sw + x) * 4..(y * sw + x) * 4 + 3];
        // Its left is still red and its right blue, and the middle is between them: the edge is gone.
        assert!(at(2, 3)[0] > 150 && at(2, 3)[2] < 60 && at(37, 3)[2] > 150 && at(37, 3)[0] < 60, "{:?} {:?}", at(2, 3), at(37, 3));
        assert!(at(20, 3)[0] > 60 && at(20, 3)[0] < 160, "{:?}", at(20, 3));
        // The white line is nowhere: no pixel of the backdrop is near white.
        assert!(small.chunks(4).all(|p| p[1] < 120), "the line's white is spread away");
        // Twin: the photo itself holds the line, and a photo with no pixel has no backdrop.
        assert_eq!(&rgba[(100 * w + 5) * 4..(100 * w + 5) * 4 + 3], &[255, 255, 255]);
        assert_eq!(blurred(&[], 0, 0), None);
        // A photo smaller than the backdrop keeps its size.
        assert_eq!(blurred(&[9; 4 * 6], 3, 2).map(|b| (b.1, b.2)), Some((3, 2)));
    }
}
