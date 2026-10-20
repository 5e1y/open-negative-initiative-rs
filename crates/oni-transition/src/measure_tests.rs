use super::*;

/// A textured negative-like frame: soft gradients and scattered blobs, every channel inside the band.
fn scene(width: usize, height: usize) -> Vec<[f32; 4]> {
    let mut seed = 0x2545_f491_u32;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        f64::from(seed) / f64::from(u32::MAX)
    };
    let blobs: Vec<(f64, f64, f64, [f64; 3])> =
        (0..120).map(|_| (next() * width as f64, next() * height as f64, 4.0 + next() * 30.0, [next(), next(), next()])).collect();
    let mut out = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let mut v = [0.05 + 0.1 * x as f64 / width as f64, 0.08, 0.04 + 0.08 * y as f64 / height as f64];
            for (bx, by, r, c) in &blobs {
                let d2 = ((x as f64 - bx).powi(2) + (y as f64 - by).powi(2)) / (r * r);
                if d2 < 1.0 {
                    (0..3).for_each(|k| v[k] += 0.25 * c[k] * (1.0 - d2));
                }
            }
            out.push([v[0] as f32, v[1] as f32, v[2] as f32, 1.0]);
        }
    }
    out
}

/// Apple's view of `libraw`: its frame starting at `offset`, gained by `stops`, and its grid.
fn apple_of(libraw: Plane<'_>, offset: (usize, usize), stops: [f64; 3], flipped: bool) -> Grid {
    let m = [0, 1, 2].map(|r| [0, 1, 2].map(|c| if r == c { stops[c].exp2() } else { 0.0 }));
    seen_through(libraw, offset, &m, flipped)
}

/// Apple's view of `libraw` through a 3x3 `m`.
fn seen_through(libraw: Plane<'_>, offset: (usize, usize), m: &M3, flipped: bool) -> Grid {
    let (w, h) = (libraw.width - offset.0, libraw.height - offset.1);
    let mut pixels = Vec::with_capacity(w * h);
    for y in 0..h {
        let row = if flipped { libraw.height - 1 - y } else { y + offset.1 };
        for x in 0..w {
            let p = libraw.pixels[row * libraw.width + x + offset.0];
            let v = apply(m, [0, 1, 2].map(|c| f64::from(p[c])));
            pixels.push([v[0] as f32, v[1] as f32, v[2] as f32, 1.0]);
        }
    }
    Grid::of(Plane { width: w, height: h, pixels: &pixels })
}

const ZF: [f64; 3] = [0.063, -0.026, -0.126];

#[test]
fn the_offset_and_the_gains_are_found_to_the_pixel() {
    let (w, h) = (400, 300);
    let pixels = scene(w, h);
    let libraw = Plane { width: w, height: h, pixels: &pixels };
    let shift = fit(&apple_of(libraw, (3, 5), ZF, false), libraw).expect("a fit");
    assert_eq!(shift.offset, (3, 5));
    for c in 0..3 {
        assert!((shift.stops[c] - ZF[c]).abs() < 1e-4, "{:?}", shift.stops);
    }
    assert!(shift.correlation > 0.999 && shift.share > 0.9, "{shift:?}");
    // Twin: a picture upside down against the other is refused, never measured.
    assert!(matches!(fit(&apple_of(libraw, (0, 0), ZF, true), libraw), Err(Refusal::Misaligned(_))));
}

#[test]
fn a_frame_mostly_clipped_is_refused() {
    let (w, h) = (800, 600);
    let mut pixels = scene(w, h);
    // Every pixel but a band on the left pushed past the clip.
    for (i, p) in pixels.iter_mut().enumerate() {
        if i % w > w / 8 {
            (0..3).for_each(|c| p[c] += 1.0);
        }
    }
    let libraw = Plane { width: w, height: h, pixels: &pixels };
    assert!(matches!(fit(&apple_of(libraw, (0, 0), ZF, false), libraw), Err(Refusal::TooFewUsable(_))));
    // Twin: the same frame unclipped is measured.
    let clear = scene(w, h);
    let libraw = Plane { width: w, height: h, pixels: &clear };
    assert!(fit(&apple_of(libraw, (0, 0), ZF, false), libraw).is_ok());
}

#[test]
fn a_camera_takes_each_channel_median_of_its_frames() {
    let frames = [[0.06, -0.02, -0.12], [0.03, -0.05, -0.14], [0.07, -0.03, -0.13]];
    assert_eq!(combined(&frames), [0.06, -0.03, -0.13]);
    // Twin: an outlier frame moves a mean, not the median.
    let mean = frames.iter().map(|f| f[1]).sum::<f64>() / 3.0;
    assert!((mean - combined(&frames)[1]).abs() > 1e-3);
    assert_eq!(median(vec![1.0, 3.0]), 2.0);
}

/// The study's Z f matrix, LibRaw's Rec. 2020 to Apple's.
const ZF_MATRIX: M3 = [[1.1252, -0.1698, -0.0064], [0.0259, 0.9537, -0.0640], [0.0092, -0.0109, 0.8969]];

#[test]
fn the_matrix_is_recovered_where_gains_leave_a_residual() {
    let (w, h) = (400, 300);
    let pixels = scene(w, h);
    let libraw = Plane { width: w, height: h, pixels: &pixels };
    let shift = fit(&seen_through(libraw, (2, 1), &ZF_MATRIX, false), libraw).expect("a fit");
    let m = matrix(&shift.pairs).expect("a matrix");
    for r in 0..3 {
        for c in 0..3 {
            assert!((m[r][c] - ZF_MATRIX[r][c]).abs() < 1e-3, "{m:?}");
        }
    }
    assert!(residual(&shift.pairs, &m).iter().all(|e| *e < 1e-4));
    // Twin: the channels' own gains alone leave the mixing unexplained.
    let gains = [0, 1, 2].map(|r| [0, 1, 2].map(|c| if r == c { shift.stops[c].exp2() } else { 0.0 }));
    assert!(residual(&shift.pairs, &gains).iter().any(|e| *e > 1e-3), "{:?}", residual(&shift.pairs, &gains));
    assert!(matrix(&[]).is_none(), "no pair, no matrix");
}
