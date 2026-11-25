use super::*;

/// The identity from a render's pixels to the scan's grid.
const SAME: [[f32; 3]; 2] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
const PURE: [V3; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// A 30 µm grain on a 47 Mpx frame's grid, 8368 pixels long, as the real Portra 800 scan was measured.
fn portra(deviation: f32) -> Grain {
    Grain::on(SAME, 0.5 * 30.0 * 8368.0 / 36_000.0, 1.0, [0.85 * deviation, deviation, deviation], PURE)
}

fn flat(side: usize, v: V4) -> Grid {
    Grid::new(0, 0, side, side, vec![v; side * side])
}

fn grained(input: &Grid, k: &Grain) -> Vec<V4> {
    (0..input.height).flat_map(|y| (0..input.width).map(move |x| [x as f32 + 0.5, y as f32 + 0.5])).map(|at| grain(input, at, k)).collect()
}

/// The mean deviation of channel `c`'s screen code in tiles of 32 pixels, as the scan's grain was measured.
fn tiled(pixels: &[V4], side: usize, c: usize) -> f32 {
    let tiles = side / 32;
    let mut sum = 0.0;
    for t in 0..tiles * tiles {
        let (tx, ty) = (t % tiles * 32, t / tiles * 32);
        let v: Vec<f32> = (0..32 * 32).map(|i| super::super::neutral_negative::screen_code(pixels[(ty + i / 32) * side + tx + i % 32][c])).collect();
        let mean = v.iter().sum::<f32>() / v.len() as f32;
        sum += (v.iter().map(|x| (x - mean) * (x - mean)).sum::<f32>() / v.len() as f32).sqrt();
    }
    sum / (tiles * tiles) as f32
}

#[test]
fn a_flat_film_is_its_own_share_where_grains_stray_by_none() {
    let input = flat(48, [0.18, 0.3, 0.05, 1.0]);
    let still = grained(&input, &portra(0.0));
    let worst = still.iter().map(|p| (0..3).map(|c| (p[c] / input.pixels[0][c] - 1.0).abs()).fold(0.0, f32::max)).fold(0.0, f32::max);
    assert!(worst < 2e-6, "a grain sampling a flat film moved it by {worst}");
    // Twin: grains that stray move it.
    let moved = grained(&input, &portra(0.04));
    assert!(moved.iter().any(|p| (p[1] / input.pixels[0][1] - 1.0).abs() > 1e-2));
}

#[test]
fn the_grain_measured_as_the_scan_was_is_its_deviation_through_the_grains() {
    let side = 256;
    let input = flat(side, [0.18, 0.18, 0.18, 1.0]);
    let sd = tiled(&grained(&input, &portra(0.037)), side, 1);
    println!("a 30 µm grain of deviation 0.037 on a 47 Mpx grid: {sd:.4} of the window in 32-pixel tiles");
    assert!((0.008..0.02).contains(&sd), "{sd}");
    // Twin: each pixel its own draw, no grain, spreads the whole deviation.
    let noisy = Grid::generated(0, 0, side, side, |q| {
        let h = hash([q[0] as u32, q[1] as u32, 9, 0]);
        let v = super::super::neutral_negative::screen_code(0.18) + 0.037 * normal(h[0]);
        [screen_light(v); 4]
    });
    assert!((tiled(&noisy.pixels, side, 1) - 0.037).abs() < 0.003);
}

#[test]
fn detail_finer_than_a_grain_goes() {
    let side = 64;
    // A one-pixel checker of two greys.
    let input = Grid::generated(0, 0, side, side, |q| if (q[0] + q[1]) % 2 == 0 { [0.1, 0.1, 0.1, 1.0] } else { [0.3, 0.3, 0.3, 1.0] });
    let out = grained(&input, &portra(0.0));
    let spread = |p: &[V4]| {
        let mean = p.iter().map(|v| v[1]).sum::<f32>() / p.len() as f32;
        (p.iter().map(|v| (v[1] - mean).powi(2)).sum::<f32>() / p.len() as f32).sqrt()
    };
    println!("a one-pixel checker's spread {:.4}, under the grains {:.4}", spread(&input.pixels), spread(&out));
    assert!(spread(&out) < 0.5 * spread(&input.pixels), "{} {}", spread(&out), spread(&input.pixels));
    // Twin: a detail far coarser than a grain, a 32-pixel checker, stays.
    let coarse = Grid::generated(0, 0, side, side, |q| if (q[0] / 32 + q[1] / 32) % 2 == 0 { [0.1, 0.1, 0.1, 1.0] } else { [0.3, 0.3, 0.3, 1.0] });
    assert!(spread(&grained(&coarse, &portra(0.0))) > 0.8 * spread(&coarse.pixels));
}

#[test]
fn each_layer_has_its_own_grains_and_the_gaze_mixes_them() {
    let side = 64;
    let input = flat(side, [0.18, 0.18, 0.18, 1.0]);
    let devs = |k: &Grain| -> Vec<V3> {
        let base = shared(input.pixels[0], k);
        grained(&input, k).iter().map(|p| map(shared(*p, k), |c, v| v - base[c])).collect()
    };
    let correlation = |d: &[V3], a: usize, b: usize| {
        let (sa, sb, sab) = d.iter().fold((0.0f32, 0.0f32, 0.0f32), |(x, y, z), v| (x + v[a] * v[a], y + v[b] * v[b], z + v[a] * v[b]));
        sab / (sa * sb).sqrt()
    };
    let own = devs(&portra(0.04));
    println!("red and green's grains correlate {:.3} on their own", correlation(&own, 0, 1));
    assert!(correlation(&own, 0, 1).abs() < 0.15, "{}", correlation(&own, 0, 1));
    // Twins: one layer's grains on the three channels correlate whole, and a gaze seeing two layers alike mixes them.
    let silver = devs(&portra(0.04).on_silver());
    assert!(correlation(&silver, 0, 1) > 0.999);
    let mixed = Grain { gaze: [[0.5, 0.5, 0.0], [0.5, 0.5, 0.0], [0.0, 0.0, 1.0]], ..portra(0.04) };
    assert!(correlation(&devs(&mixed), 0, 1) > 0.95);
}

#[test]
fn a_photos_seed_lays_its_own_arrangement_of_the_same_grain() {
    let side = 96;
    let input = flat(side, [0.18, 0.18, 0.18, 1.0]);
    let (a, b) = (grained(&input, &portra(0.04)), grained(&input, &portra(0.04).seeded([7, 3])));
    let gap = a.iter().zip(&b).map(|(x, y)| (x[1] - y[1]).abs()).fold(0.0, f32::max);
    assert!(gap > 1e-2, "another seed lays the same grains");
    assert!((tiled(&a, side, 1) / tiled(&b, side, 1) - 1.0).abs() < 0.2, "with the same strength");
    // Twin: the same seed lays the same grains, to the bit.
    assert_eq!(a, grained(&input, &portra(0.04)));
}

#[test]
fn a_negatives_grain_lies_in_its_windows_shares() {
    let negative = GrainedNegative { bare: [0.5003, 0.2503, 0.1003], veil: 0.0003, window: [0.8, 1.0, 1.2] };
    let k = portra(0.0).on_negative(negative);
    let light = [0.3, 0.05, 0.03, 1.0];
    let share = shared(light, &k);
    let back = coded(share, &k);
    assert!((0..3).all(|c| (back[c] / light[c] - 1.0).abs() < 1e-5), "{back:?}");
    assert!((share[1] - (-(0.0503f32 / 0.2503).log10() / 1.0)).abs() < 1e-6, "green's share of its window");
    // Twin: read as a positive's code, the same light is another share.
    assert!((shared(light, &portra(0.0))[1] - share[1]).abs() > 0.05);
}

#[test]
fn a_wide_footprint_takes_in_no_more_cells_than_the_bound_and_lays_the_mean_of_its_grains() {
    let wide = Grain::on(SAME, 3.0, 40.0, [0.1; 3], PURE);
    let cells = 2.0 * wide.reach() / (GRAIN_STEP * wide.radius) + 1.0;
    assert!(cells <= GRAIN_CELLS as f32 + 1e-4, "{cells} cells");
    assert!(wide.deviation[0] < 0.1 && wide.deviation[0] > 0.0, "a footprint past the bound spreads less: {:?}", wide.deviation);
    // Twin: at full resolution the deviation is the grain's own.
    let full = Grain::on(SAME, 3.0, 1.0, [0.1; 3], PURE);
    assert!((full.deviation[0] - 0.1).abs() < 1e-6 && (full.cloud - (GRAIN_CLOUD * 3.0).hypot(0.5)).abs() < 1e-6);
}
