//! A photo that fills a card: the middle of it that the card's shape holds, brought down to the card's
//! own pixels when it has more, so a frame is laid from a picture as sharp as it is drawn.

/// The long side a photo `aspect` wide for one tall is developed at to fill a box `across` by `down`.
pub fn side(aspect: f32, (across, down): (usize, usize)) -> usize {
    let aspect = if aspect.is_finite() && aspect > 0.0 { aspect } else { 1.5 };
    let tall = (down as f32).max(across as f32 / aspect);
    tall.max(tall * aspect).ceil() as usize
}

/// The middle of the picture that fills a box `across` by `down`, as wide for its height as the box,
/// and no larger than it: each of its pixels the mean of those it stands for.
pub fn filling(rgba: &[u8], width: usize, height: usize, (across, down): (usize, usize)) -> Option<(Vec<u8>, usize, usize)> {
    if width == 0 || height == 0 || across == 0 || down == 0 || rgba.len() != width * height * 4 {
        return None;
    }
    // The part kept: the picture's whole height or its whole width, whichever the box's shape leaves.
    let (kept_w, kept_h) = if width * down > height * across { ((height * across / down).max(1), height) } else { (width, (width * down / across).max(1)) };
    let (left, top) = ((width - kept_w) / 2, (height - kept_h) / 2);
    let (out_w, out_h) = if kept_w > across { (across, down) } else { (kept_w, kept_h) };
    let mut out = vec![0u8; out_w * out_h * 4];
    for y in 0..out_h {
        let (from_y, to_y) = (top + y * kept_h / out_h, top + ((y + 1) * kept_h / out_h).max(y * kept_h / out_h + 1));
        for x in 0..out_w {
            let (from_x, to_x) = (left + x * kept_w / out_w, left + ((x + 1) * kept_w / out_w).max(x * kept_w / out_w + 1));
            let mut sum = [0u32; 4];
            for row in from_y..to_y {
                for pixel in rgba[(row * width + from_x) * 4..(row * width + to_x) * 4].chunks_exact(4) {
                    for (total, value) in sum.iter_mut().zip(pixel) {
                        *total += u32::from(*value);
                    }
                }
            }
            let count = ((to_y - from_y) * (to_x - from_x)) as u32;
            for (channel, total) in sum.iter().enumerate() {
                out[(y * out_w + x) * 4 + channel] = (total / count) as u8;
            }
        }
    }
    Some((out, out_w, out_h))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A picture whose every pixel holds its own column and row.
    fn picture(width: usize, height: usize) -> Vec<u8> {
        (0..height).flat_map(|y| (0..width).flat_map(move |x| [x as u8, y as u8, 7, 255])).collect()
    }

    #[test]
    fn a_photo_is_developed_as_large_as_the_card_it_fills() {
        // A landscape three by two fills a card 1080 by 1920 with its height: 1920 tall, 2880 wide.
        assert_eq!(side(1.5, (1080, 1920)), 2880);
        // A portrait two by three, its height: 1920 by 1280. A panorama's height too.
        assert_eq!((side(2.0 / 3.0, (1080, 1920)), side(3.0, (1080, 1920))), (1920, 5760));
        // A photo narrower than the card fills it with its width.
        assert_eq!(side(0.5, (1080, 1920)), 2160);
        // Twin: the long side of a print laid whole in the card's width is a third of it.
        assert!(side(1.5, (1080, 1920)) > 1080 * 2);
        assert_eq!(side(f32::NAN, (1080, 1920)), 2880, "a shape unread is a landscape's");
    }

    #[test]
    fn the_middle_of_a_photo_fills_the_card_no_larger_than_it() {
        // A landscape 12 by 4 in a box 2 by 4: its middle 2 columns, every row, kept as they are.
        let (kept, width, height) = filling(&picture(12, 4), 12, 4, (2, 4)).unwrap();
        assert_eq!((width, height), (2, 4));
        assert_eq!(&kept[..8], &[5, 0, 7, 255, 6, 0, 7, 255]);
        // Twin: the picture's left edge is not in it.
        assert_ne!(kept[0], 0);
        // A portrait 4 by 12 in a box 4 by 2: its middle rows.
        let (kept, width, height) = filling(&picture(4, 12), 4, 12, (4, 2)).unwrap();
        assert_eq!((width, height, kept[1], kept[4 * 4 + 1]), (4, 2, 5, 6));
        // A picture with more pixels than the box is brought down to it, each pixel a mean: 8 by 16
        // into 2 by 4, the first of them the mean of columns 0 to 3 and rows 0 to 3.
        let (small, width, height) = filling(&picture(8, 16), 8, 16, (2, 4)).unwrap();
        assert_eq!((width, height), (2, 4));
        assert_eq!(&small[..4], &[1, 1, 7, 255]);
        assert_eq!(&small[small.len() - 4..], &[5, 13, 7, 255]);
        // One with fewer is never made larger.
        assert_eq!(filling(&picture(2, 2), 2, 2, (100, 100)).map(|(_, w, h)| (w, h)), Some((2, 2)));
        assert_eq!(filling(&picture(2, 2), 2, 3, (1, 1)), None, "twin: a size its pixels do not make");
    }
}
