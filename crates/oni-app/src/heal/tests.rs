//! The search and the tone match on synthetic frames whose answers are known.

use super::*;
use oni_settings::cg::Point;
use oni_settings::id::Id;

/// A textured frame whose neighbourhood of the dust is a copy of a block `known` away: the search
/// must find that block, and the patch must carry its pixels, not the dust's.
#[test]
fn the_search_finds_the_place_the_surroundings_were_copied_from() {
    let (w, h) = (1600usize, 1200usize);
    // White noise smoothed over 17 pixels: no period, and a basin wider than the coarse step.
    let hash = |x: usize, y: usize| {
        let mut v = (x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663);
        v ^= v >> 13;
        v = v.wrapping_mul(1_274_126_177);
        (v >> 8) as f32 / 16_777_216.0
    };
    let noise: Vec<f32> = (0..w * h).map(|i| hash(i % w, i / w)).collect();
    let r = 8usize;
    let smooth: Vec<f32> = (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            let (mut sum, mut n) = (0.0, 0.0);
            for yy in y.saturating_sub(r)..(y + r + 1).min(h) {
                for xx in x.saturating_sub(r)..(x + r + 1).min(w) {
                    sum += noise[yy * w + xx];
                    n += 1.0;
                }
            }
            sum / n
        })
        .collect();
    // Stretched back to a photograph's contrast: a mean of 0.4 and a spread of 0.08.
    let mean = smooth.iter().sum::<f32>() / smooth.len() as f32;
    let spread = (smooth.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / smooth.len() as f32).sqrt();
    let smooth: Vec<f32> = smooth.iter().map(|v| 0.4 + 0.08 * (v - mean) / spread).collect();
    let texture = |x: usize, y: usize| smooth[y * w + x];
    let mut pixels: Vec<[f32; 4]> = (0..w * h).map(|i| { let v = smooth[i]; [v, v * 0.9, v * 0.8, 1.0] }).collect();
    let (dot_x, dot_y) = (800usize, 600usize);
    let known = (-288i64, 168i64);
    for y in dot_y - 200..dot_y + 200 {
        for x in dot_x - 200..dot_x + 200 {
            let from = ((y as i64 + known.1) as usize) * w + (x as i64 + known.0) as usize;
            pixels[y * w + x] = pixels[from];
        }
    }
    for y in dot_y - 20..=dot_y + 20 {
        for x in dot_x - 20..=dot_x + 20 {
            pixels[y * w + x] = [0.02, 0.02, 0.02, 1.0];
        }
    }
    let src = Region { x: 0, y: 0, width: w, height: h, pixels, frame_width: w as f64, frame_height: h as f64 };
    let c = Correction { id: Id::new(), stroke: vec![Point { x: dot_x as f64 / w as f64, y: dot_y as f64 / h as f64 }], radius: 30.0 };
    let (dx, dy) = best_offset(&src, &c, w as f64, h as f64);
    // The offset is y up: a block lower on the frame (y down + 168) is 168 under, −168 up.
    assert_eq!((dx, dy), (known.0 as f64, -known.1 as f64), "found {dx}, {dy}");
    let patch = generate(&src, &c, w as f64, h as f64);
    let (px, py) = (dot_x - patch.rect[0] as usize, dot_y - patch.rect[1] as usize);
    let healed = patch.pixels[py * patch.width + px][0];
    let want = texture((dot_x as i64 + known.0) as usize, (dot_y as i64 + known.1) as usize);
    assert!((healed - want).abs() < 0.02, "healed to {healed}, the copied block holds {want}");
    // Twin: the frame still holds the dust there, so a patch copied in place would be seen.
    assert!((src.pixels[dot_y * w + dot_x][0] - want).abs() > 0.2);
}

#[test]
fn a_bias_lifts_the_copy_to_its_surroundings_tone() {
    // The right half a step brighter: a copy taken from the left half is lifted by the base.
    let (w, h) = (400usize, 300usize);
    let pixels: Vec<[f32; 4]> = (0..w * h).map(|i| { let v = if i % w >= 200 { 0.5 } else { 0.4 }; [v, v, v, 1.0] }).collect();
    let src = Region { x: 0, y: 0, width: w, height: h, pixels, frame_width: w as f64, frame_height: h as f64 };
    let destination = Rect { x: 250.0, y: 100.0, w: 60.0, h: 60.0 };
    let source = Rect { x: 50.0, ..destination };
    let field = tone_field(&src, destination, source, Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 });
    assert!((field.base[0] - 0.1).abs() < 1e-4, "{:?}", field.base);
    // Twin: the same rings on one tone need no bias.
    let flat = tone_field(&src, destination, Rect { x: 320.0, ..destination }, Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 });
    assert!(flat.base[0].abs() < 1e-5);
}

/// White noise box-smoothed over `2r + 1` pixels by a summed table, at a photograph's contrast.
fn textured(w: usize, h: usize, r: usize) -> Vec<[f32; 4]> {
    let hash = |x: usize, y: usize| {
        let mut v = (x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663);
        v ^= v >> 13;
        v = v.wrapping_mul(1_274_126_177);
        f64::from(v >> 8) / 16_777_216.0
    };
    let mut table = vec![0.0f64; (w + 1) * (h + 1)];
    for y in 0..h {
        for x in 0..w {
            table[(y + 1) * (w + 1) + x + 1] = hash(x, y) + table[y * (w + 1) + x + 1] + table[(y + 1) * (w + 1) + x] - table[y * (w + 1) + x];
        }
    }
    (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            let (x0, y0, x1, y1) = (x.saturating_sub(r), y.saturating_sub(r), (x + r + 1).min(w), (y + r + 1).min(h));
            let sum = table[y1 * (w + 1) + x1] - table[y0 * (w + 1) + x1] - table[y1 * (w + 1) + x0] + table[y0 * (w + 1) + x0];
            // A box of n uniform values spreads by 1 / sqrt(12 n): stretched to 0.08 about 0.4.
            let n = ((x1 - x0) * (y1 - y0)) as f64;
            let v = (0.4 + 0.08 * (sum / n - 0.5) * (12.0 * n).sqrt()) as f32;
            [v, v * 0.9, v * 0.8, 1.0]
        })
        .collect()
}

/// The search reads its window alone: the patch made from that window is the one made from the
/// whole frame, and a window short of the copy's place makes another.
#[test]
fn a_search_reads_its_window_alone_and_a_narrower_one_misses() {
    let (w, h) = (3600usize, 1200usize);
    let mut pixels = textured(w, h, 8);
    let (dot_x, dot_y, known) = (1800usize, 600usize, (-1100i64, 120i64));
    for y in dot_y - 200..dot_y + 200 {
        for x in dot_x - 200..dot_x + 200 {
            pixels[y * w + x] = pixels[(y as i64 + known.1) as usize * w + (x as i64 + known.0) as usize];
        }
    }
    for y in dot_y - 20..=dot_y + 20 {
        for x in dot_x - 20..=dot_x + 20 {
            pixels[y * w + x] = [0.02, 0.02, 0.02, 1.0];
        }
    }
    let cut = |a: Area| {
        let (x0, y0, cw, ch) = (a[0] as usize, a[1] as usize, a[2] as usize, a[3] as usize);
        let kept = (y0..y0 + ch).flat_map(|y| pixels[y * w + x0..y * w + x0 + cw].iter().copied()).collect();
        Region { x: x0 as i64, y: y0 as i64, width: cw, height: ch, pixels: kept, frame_width: w as f64, frame_height: h as f64 }
    };
    let c = Correction { id: Id::new(), stroke: vec![Point { x: dot_x as f64 / w as f64, y: dot_y as f64 / h as f64 }], radius: 30.0 };
    let whole = generate(&cut([0.0, 0.0, w as f64, h as f64]), &c, w as f64, h as f64);
    assert_eq!(((whole.offset.0 * w as f64).round(), (whole.offset.1 * h as f64).round()), (known.0 as f64, known.1 as f64), "the copy is found");
    let window = search_window(&c, w as f64, h as f64);
    assert!(window[2] < w as f64 * 0.9, "the window leaves most of a wide frame out: {window:?}");
    let windowed = generate(&cut(window), &c, w as f64, h as f64);
    assert_eq!(windowed.offset, whole.offset);
    assert!(windowed.pixels == whole.pixels, "the window's patch is the whole frame's");
    // Twin: a window cut short of the copy, 1100 pixels left, finds another source.
    let narrow = generate(&cut([window[0] + 600.0, window[1], window[2] - 1200.0, window[3]]), &c, w as f64, h as f64);
    assert_ne!(narrow.offset, whole.offset);
}

/// The taps read straight inside the region are the clamped reads, everywhere the search samples.
#[test]
fn the_unclamped_taps_read_what_the_clamped_ones_do() {
    let (rw, rh) = (40usize, 30usize);
    let pixels: Vec<[f32; 4]> = (0..rw * rh).map(|i| [i as f32, (i * 7 % 13) as f32, (i % rw) as f32, 1.0]).collect();
    let src = Region { x: 5, y: 7, width: rw, height: rh, pixels, frame_width: 60.0, frame_height: 50.0 };
    let reference = |x: f64, y: f64, swapped: bool| -> [f32; 3] {
        let (u, v) = (x - 0.5, y - 0.5);
        let (x0, y0) = (u.floor(), v.floor());
        let (fx, fy) = ((u - x0) as f32, (v - y0) as f32);
        let (x0, y0) = (x0 as i64, y0 as i64);
        let (mut a, b, mut c, d) = (src.texel(x0, y0), src.texel(x0 + 1, y0), src.texel(x0, y0 + 1), src.texel(x0 + 1, y0 + 1));
        if swapped {
            std::mem::swap(&mut a, &mut c);
        }
        std::array::from_fn(|k| (a[k] * (1.0 - fx) + b[k] * fx) * (1.0 - fy) + (c[k] * (1.0 - fx) + d[k] * fx) * fy)
    };
    let points: Vec<(f64, f64)> = (0..180).flat_map(|i| (0..150).map(move |j| (-2.0 + i as f64 * 0.37, -2.0 + j as f64 * 0.37))).collect();
    for &(x, y) in &points {
        assert_eq!(src.sample(x, y), reference(x, y, false), "at {x}, {y}");
    }
    assert!(points.iter().any(|&(x, y)| src.sample(x, y) != reference(x, y, true)), "twin: rows read the wrong way round are seen");
}

/// Dots of radius 12 at fractions of a 6000 × 4000 frame, whose search windows reach about 1 450 px.
fn dots(at: &[(f64, f64)]) -> Vec<Correction> {
    at.iter().map(|&(x, y)| Correction { id: Id::new(), stroke: vec![Point { x, y }, Point { x: x + 0.001, y }], radius: 12.0 }).collect()
}

const FRAME: (f64, f64) = (6000.0, 4000.0);

/// Whether `j`'s patch lies where a search for `k` reads.
fn lies_for(cs: &[Correction], j: usize, k: usize) -> bool {
    let (r, w) = (corrections::patch_rect(&cs[j], [0.0, 0.0, FRAME.0, FRAME.1]), search_window(&cs[k], FRAME.0, FRAME.1));
    r[0] < w[0] + w[2] && w[0] < r[0] + r[2] && r[1] < w[1] + w[3] && w[1] < r[1] + r[3]
}

/// The pending corrections a search for `k` waits on, by fixed point: earlier ones lying in its
/// window, and in theirs, and so on.
fn waits_on(cs: &[Correction], pending: &[usize], k: usize) -> Vec<usize> {
    let mut set = vec![k];
    loop {
        let more: Vec<usize> = pending.iter().copied().filter(|&j| !set.contains(&j) && set.iter().any(|&m| j < m && lies_for(cs, j, m))).collect();
        if more.is_empty() {
            set.retain(|&j| j != k);
            return set;
        }
        set.extend(more);
    }
}

/// Corrections healed in turn by `pick`, from `pending`, until `stop` is healed or none is left: the
/// order, and how many earlier corrections of each one's window were not healed yet when it was.
fn healed(cs: &[Correction], pending: &mut Vec<usize>, stop: Option<usize>, pick: impl Fn(&[usize]) -> Option<usize>) -> (Vec<usize>, usize) {
    let (mut order, mut unsound) = (Vec::new(), 0);
    while let Some(k) = pick(pending) {
        unsound += pending.iter().filter(|&&j| j < k && lies_for(cs, j, k)).count();
        pending.retain(|&j| j != k);
        order.push(k);
        if Some(k) == stop {
            break;
        }
    }
    (order, unsound)
}

#[test]
fn the_newest_stroke_heals_first_after_the_earlier_ones_its_window_holds() {
    // 0 and 1 far from the rest; 3 lies in 4's window, 2 in 3's alone; 4 the hand's newest.
    let cs = dots(&[(0.9, 0.9), (0.85, 0.1), (0.1, 0.5), (0.3, 0.5), (0.45, 0.5)]);
    let next = |pending: &[usize]| next(&cs, pending, FRAME.0, FRAME.1);
    assert_eq!(next(&[0, 1, 4]), Some(4), "nothing it reads is pending: the newest at once");
    assert_eq!(next(&[0, 1, 3, 4]), Some(3), "an earlier stroke in its window first");
    assert_eq!(next(&[0, 1, 2, 3, 4]), Some(2), "and the one that stroke's window holds before it");
    assert_eq!(next(&[0, 1]), Some(1));
    assert_eq!(next(&[]), None);
    // Twin: 2 does not lie in 4's window, so with 3 healed it is not 4's to wait for.
    assert!(!lies_for(&cs, 2, 4) && lies_for(&cs, 2, 3) && lies_for(&cs, 3, 4));
    assert_eq!(next(&[0, 1, 2, 4]), Some(4));
}

#[test]
fn a_stroke_painted_during_a_batch_heals_before_the_batch_s_rest_and_every_search_holds_its_window() {
    // Sixty dots on the frame's left third and sixty on its right, all to make again.
    let spread = |x0: f64| (0..60).map(move |k| (x0 + 0.25 * ((k * 37) % 60) as f64 / 60.0, 0.05 + 0.9 * ((k * 53) % 60) as f64 / 60.0));
    let mut cs = dots(&spread(0.03).chain(spread(0.72)).collect::<Vec<_>>());
    let pick = |cs: &[Correction], p: &[usize]| next(cs, p, FRAME.0, FRAME.1);
    let (order, unsound) = healed(&cs, &mut (0..cs.len()).collect(), None, |p| pick(&cs, p));
    assert_eq!((order.len(), unsound), (120, 0), "a search ran before an earlier patch of its window");
    // The left third healed, the hand paints there: its stroke heals next, the right's sixty after.
    cs.extend(dots(&[(0.15, 0.5), (0.85, 0.5)]));
    let (hand, among) = (cs.len() - 2, cs.len() - 1);
    let batch: Vec<usize> = (60..120).collect();
    let (order, _) = healed(&cs, &mut [&batch[..], &[hand]].concat(), Some(hand), |p| pick(&cs, p));
    assert_eq!(order, vec![hand], "the stroke waited behind the batch");
    // Painted among the right's dots, it waits for the earlier ones its window holds, and theirs.
    let pending = [&batch[..], &[among]].concat();
    let waits = waits_on(&cs, &pending, among);
    let (order, unsound) = healed(&cs, &mut pending.clone(), Some(among), |p| pick(&cs, p));
    assert_eq!((order.len(), unsound), (waits.len() + 1, 0));
    // Twins: the list's order serves the stroke after the whole batch, and the newest first at once
    // runs searches before earlier patches of their windows.
    let (listed, _) = healed(&cs, &mut [&batch[..], &[hand]].concat(), Some(hand), |p| p.first().copied());
    assert_eq!(listed.len(), 61);
    assert!(healed(&cs, &mut pending.clone(), None, |p| p.last().copied()).1 > 0, "newest first missed no earlier patch");
}
