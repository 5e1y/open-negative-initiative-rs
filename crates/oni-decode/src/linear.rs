//! The camera's 16-bit image to linear Rec. 2020 floats through the frame's matrix.

use std::os::raw::c_ushort;

/// The `(x0, y0, width, height)` window of `raw`, `stride` pixels a row of three channels, through
/// `m`; bands of rows on every core, since 24 M pixels in one loop took a third of a second.
pub(crate) fn converted(raw: &[c_ushort], stride: usize, window: (usize, usize, usize, usize), m: &[[f32; 3]; 3]) -> Vec<[f32; 4]> {
    let (x0, y0, w, h) = window;
    let linear = |p: &[c_ushort]| {
        let cam = [p[0] as f32 / 65535.0, p[1] as f32 / 65535.0, p[2] as f32 / 65535.0];
        let t = |r: usize| m[r][0] * cam[0] + m[r][1] * cam[1] + m[r][2] * cam[2];
        [t(0), t(1), t(2), 1.0]
    };
    let mut pixels = vec![[0.0f32; 4]; w * h];
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(h.max(1));
    let band = h.div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        for (i, rows) in pixels.chunks_mut(band * w.max(1)).enumerate() {
            let linear = &linear;
            scope.spawn(move || {
                for (r, row) in rows.chunks_mut(w.max(1)).enumerate() {
                    let y = y0 + i * band + r;
                    for (out, p) in row.iter_mut().zip(raw[(y * stride + x0) * 3..(y * stride + x0 + w) * 3].chunks_exact(3)) {
                        *out = linear(p);
                    }
                }
            });
        }
    });
    pixels
}
