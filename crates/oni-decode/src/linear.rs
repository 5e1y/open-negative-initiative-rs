//! The camera's 16-bit image to linear Rec. 2020 floats through the frame's matrix, turned upright.

use std::os::raw::c_ushort;

/// LibRaw's `flip` as `flip_index` reads it: 4 swaps rows and columns, then 2 mirrors the rows and
/// 1 the columns, so 3 is a half turn, 5 a quarter turn left and 6 a quarter turn right.
pub(crate) fn upright(flip: i32, (w, h): (usize, usize)) -> (usize, usize) {
    if flip & 4 != 0 { (h, w) } else { (w, h) }
}

/// The `(x0, y0, width, height)` window of `raw`, `stride` pixels a row of three channels, through
/// `m` and turned by `flip`, as `CIRAWFilter` hands a frame out; bands of rows on every core, since
/// 24 M pixels in one loop took a third of a second.
pub(crate) fn converted(raw: &[c_ushort], stride: usize, window: (usize, usize, usize, usize), flip: i32, m: &[[f32; 3]; 3]) -> Vec<[f32; 4]> {
    let (x0, y0, w, h) = window;
    let (ow, oh) = upright(flip, (w, h));
    let linear = |p: &[c_ushort]| {
        let cam = [p[0] as f32 / 65535.0, p[1] as f32 / 65535.0, p[2] as f32 / 65535.0];
        let t = |r: usize| m[r][0] * cam[0] + m[r][1] * cam[1] + m[r][2] * cam[2];
        [t(0), t(1), t(2), 1.0]
    };
    let source = |row: usize, col: usize| {
        let (mut sr, mut sc) = if flip & 4 != 0 { (col, row) } else { (row, col) };
        if flip & 2 != 0 {
            sr = h - 1 - sr;
        }
        if flip & 1 != 0 {
            sc = w - 1 - sc;
        }
        ((y0 + sr) * stride + x0 + sc) * 3
    };
    let mut pixels = vec![[0.0f32; 4]; ow * oh];
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(oh.max(1));
    let band = oh.div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        for (i, rows) in pixels.chunks_mut(band * ow.max(1)).enumerate() {
            let (linear, source) = (&linear, &source);
            scope.spawn(move || {
                for (r, row) in rows.chunks_mut(ow.max(1)).enumerate() {
                    let y = i * band + r;
                    if flip == 0 {
                        let at = source(y, 0);
                        for (out, p) in row.iter_mut().zip(raw[at..at + 3 * ow].chunks_exact(3)) {
                            *out = linear(p);
                        }
                    } else {
                        for (x, out) in row.iter_mut().enumerate() {
                            let at = source(y, x);
                            *out = linear(&raw[at..at + 3]);
                        }
                    }
                }
            });
        }
    });
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDENTITY: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

    /// A 3 × 2 window inside a 4 × 3 sensor whose red channel numbers each pixel by its place.
    fn turned(flip: i32) -> (usize, Vec<u16>) {
        let mut raw = vec![0u16; 4 * 3 * 3];
        for (i, p) in raw.chunks_exact_mut(3).enumerate() {
            p[0] = i as u16;
        }
        let out = converted(&raw, 4, (1, 1, 3, 2), flip, &IDENTITY);
        let (w, _) = upright(flip, (3, 2));
        (w, out.iter().map(|p| (p[0] * 65535.0).round() as u16).collect())
    }

    #[test]
    fn each_flip_turns_the_window_as_libraw_s_flip_index() {
        // The window reads sensor places 5 6 7 / 9 10 11.
        assert_eq!(turned(0), (3, vec![5, 6, 7, 9, 10, 11]));
        assert_eq!(turned(3), (3, vec![11, 10, 9, 7, 6, 5]), "a half turn");
        assert_eq!(turned(6), (2, vec![9, 5, 10, 6, 11, 7]), "a quarter turn right: the bottom row becomes the left column");
        assert_eq!(turned(5), (2, vec![7, 11, 6, 10, 5, 9]), "a quarter turn left");
        // Twin: the two quarter turns are not the same picture.
        assert_ne!(turned(5), turned(6));
    }
}
