//! The picture encoded for the screen on the CPU, the software path's canvas.

use super::*;

/// `display::encode` on every `step`-th row and column, its rows shared among the cores: the shown
/// bytes at a step of 1, a histogram's sample past it. A 2000 px preview costs 150 ms on one core.
pub fn encoded(linear: &Linear, k: &Params, step: usize) -> Vec<u8> {
    let (w, h) = (linear.width, linear.height);
    let rows: Vec<usize> = (0..h).step_by(step).collect();
    let row_bytes = w.div_ceil(step) * 4;
    let mut out = vec![0u8; rows.len() * row_bytes];
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let per = rows.len().div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        for (band, chunk) in rows.chunks(per).zip(out.chunks_mut(per * row_bytes)) {
            scope.spawn(move || {
                for (&y, line) in band.iter().zip(chunk.chunks_mut(row_bytes)) {
                    for (i, x) in (0..w).step_by(step).enumerate() {
                        let p = display::encode_pixel(linear.pixels[y * w + x], x as u32, y as u32, k);
                        line[i * 4..i * 4 + 4].copy_from_slice(&p);
                    }
                }
            });
        }
    });
    out
}
