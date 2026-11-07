use super::stroke::{HALO, distance_to_segment};
use super::*;
use oni_settings::ConversionMode;
use oni_settings::frame::Correction;
use oni_settings::levels::Levels;

/// The least sensitive reading's jump.
const STRICT: f32 = JUMPS.0;
/// A map as large as the checks' frames.
const SIDE: usize = 4096;

fn grey(v: f32) -> [f32; 4] {
    [v, v, v, 1.0]
}

fn limits() -> Limits {
    Limits { densest: [1.0; 3], opaque_below: [0.1; 3], gain: [1.0; 3], pedestal: 0.0, guard: 1e-6 }
}

fn frame(width: usize, height: usize, dark: &[(usize, usize)]) -> Vec<[f32; 4]> {
    let mut pixels = vec![grey(0.5); width * height];
    for &(x, y) in dark {
        pixels[y * width + x] = grey(0.01);
    }
    pixels
}

fn shapes(pixels: &[[f32; 4]], width: usize) -> Vec<Speck> {
    let mut found = gathered(&runs_of(pixels, width, &limits()));
    found.sort_by_key(|s| s.bounds);
    found
}

fn dust_at(pixels: &[[f32; 4]], s: f32) -> usize {
    read(pixels, 200, &limits(), SIDE).at(s).len()
}

#[test]
fn a_negatives_pixel_is_past_where_stage_4_leaves_its_white() {
    use oni_pipeline::conversion::stage47;
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.gains.stops = [0.4, -0.2, 0.1];
    s.levels.red = Levels::with_ends(0.2, 1.1);
    s.levels.green = Levels::with_ends(0.3, 1.3);
    s.levels.blue = Levels::with_ends(0.1, 0.9);
    let limits = Limits::of(&s);
    let k = stage47(&s);
    let lv = [k.red, k.green, k.blue];
    let place = |c: usize, x: f32| {
        let density = -((x + k.pedestal) * k.gain[c]).max(k.log_guard).log10();
        (density - lv[c][0]) / (lv[c][1] - lv[c][0])
    };
    for c in 0..3 {
        assert!((place(c, limits.opaque_below[c]) - 1.0).abs() < 1e-3, "{c}: {}", place(c, limits.opaque_below[c]));
        // Twin: on the wrong side of the limit, the place is inside the window.
        assert!(place(c, limits.opaque_below[c] * 1.05) < 1.0 && place(c, limits.opaque_below[c] * 0.95) > 1.0);
    }
    // Twin: clearer than the window's black is the scene's shadows, not shown.
    assert!(!limits.past(&[1.0, 1.0, 1.0, 1.0]));
    assert!(limits.past(&[0.0, 0.0, 0.0, 1.0]));
}

#[test]
fn a_positives_pixel_is_past_where_stage_4_leaves_its_black() {
    use oni_pipeline::conversion::stage47;
    let mut s = PipelineSettings::rest(ConversionMode::Positive, true);
    s.gains.stops = [0.0; 3];
    let window = Levels::with_ends(1.5, 2.8);
    (s.levels.red, s.levels.green, s.levels.blue) = (window, window, window);
    let limits = Limits::of(&s);
    let k = stage47(&s);
    let place = |x: f32| {
        let density = -((x + k.pedestal) * k.gain[0]).max(k.log_guard).log10();
        (k.window - density - k.red[0]) / (k.red[1] - k.red[0])
    };
    assert!(place(limits.opaque_below[0]).abs() < 1e-3, "{}", place(limits.opaque_below[0]));
    assert!(place(limits.opaque_below[0] * 0.95) < 0.0, "darker than the limit is under the black");
    // Twin: lighter than the window's white is left alone.
    assert!(place(1.0) > 1.0);
    assert!(!limits.past(&[1.0, 1.0, 1.0, 1.0]));
}

#[test]
fn the_rolls_window_sets_the_limits_and_a_hands_levels_do_not() {
    use oni_settings::frame::RollReference;
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    let reference = RollReference { id: oni_settings::Id::new(), stops: [0.2, 0.1, -0.1], black: [0.2; 3], white: [0.6; 3], base: None, median: None };
    s.roll_reference = Some(reference);
    let at_roll = Limits::of(&s);
    s.levels.red = Levels::with_ends(0.1, 1.0);
    assert_eq!(Limits::of(&s), at_roll, "a hand's move leaves the roll's window");
    // Twin: off the roll, the frame's own levels are the window.
    s.roll_reference = None;
    assert_ne!(Limits::of(&s), at_roll);
}

#[test]
fn shapes_join_on_the_diagonal_and_stay_apart_past_a_pixels_gap() {
    // A diagonal pair, a lone pixel two columns off, and a run of three on the last row.
    let pixels = frame(8, 5, &[(1, 0), (2, 1), (4, 1), (5, 4), (6, 4), (7, 4)]);
    let found = shapes(&pixels, 8);
    assert_eq!(found.iter().map(|s| s.area).collect::<Vec<_>>(), vec![2, 1, 3], "{found:?}");
    assert_eq!(found[0].bounds, [1, 0, 3, 2]);
    assert_eq!(found[2].bounds, [5, 4, 8, 5], "a run reaching the row's end closes there");
    // Twin: one pixel more between them, the diagonal pair splits.
    assert_eq!(shapes(&frame(8, 5, &[(1, 0), (3, 1)]), 8).len(), 2);
}

#[test]
fn a_shape_wrapping_round_joins_from_both_arms() {
    // A U: its arms are two shapes until the row that joins them.
    let mut dark = vec![(1, 0), (5, 0), (1, 1), (5, 1)];
    dark.extend((1..=5).map(|x| (x, 2)));
    let found = shapes(&frame(7, 3, &dark), 7);
    assert_eq!((found.len(), found[0].area), (1, 9), "{found:?}");
    // Twin: its bottom row gone, two arms.
    assert_eq!(shapes(&frame(7, 3, &[(1, 0), (5, 0), (1, 1), (5, 1)]), 7).len(), 2);
}

/// A 200 × 200 frame at light 0.5 with a block of `side` pixels at (`x`, 100) holding `light`.
fn with_block(x: usize, side: usize, light: [f32; 3]) -> Vec<[f32; 4]> {
    let mut pixels = vec![grey(0.5); 40_000];
    for row in 100..100 + side {
        for col in x..x + side {
            pixels[row * 200 + col] = [light[0], light[1], light[2], 1.0];
        }
    }
    pixels
}

#[test]
fn a_sharp_neutral_speck_is_dust_and_noise_a_glint_or_a_highlight_are_not() {
    let dust = |pixels: &[[f32; 4]]| dust_at(pixels, 0.0);
    assert_eq!(dust(&with_block(100, 3, [0.01; 3])), 1, "a speck stopping every channel alike");
    assert_eq!(dust(&with_block(100, 1, [0.01; 3])), 0, "twin: a lone pixel is noise");
    // Past the window on its blue alone: the red and green do not jump.
    assert_eq!(dust(&with_block(100, 3, [0.5, 0.5, 0.01])), 0, "twin: a dye's jump is no dust");
    // Every channel past, the blue twenty times as far: a glint's colour, not a speck's.
    assert_eq!(dust(&with_block(100, 3, [0.09, 0.09, 0.0001])), 0, "twin: not neutral");
    assert_eq!(dust(&with_block(100, 100, [0.01; 3])), 0, "twin: 10 000 pixels are a highlight");
    assert_eq!(dust(&with_block(4, 3, [0.01; 3])), 0, "twin: at the frame's edge is the holder");
    // Clearer than the window: a scratch or the scene's shadows, never read.
    assert_eq!(dust(&with_block(100, 3, [10.0; 3])), 0, "twin: past the clear end shows nothing");
    assert!(read(&with_block(100, 3, [10.0; 3]), 200, &limits(), SIDE).map.texels.iter().all(|&t| t != 0));
}

#[test]
fn a_soft_edge_measures_no_jump_where_a_sharp_one_does() {
    // The same core just past the window, its neighbours just inside it: two pixels off, it reads them.
    let mut soft = with_block(100, 3, [0.09; 3]);
    for row in 97..106 {
        for col in 97..106 {
            if !(100..103).contains(&row) || !(100..103).contains(&col) {
                soft[row * 200 + col] = grey(0.11);
            }
        }
    }
    let sharp = with_block(100, 3, [0.09; 3]);
    let speck = |pixels: &[[f32; 4]]| gathered(&runs_of(pixels, 200, &limits())).remove(0);
    let at = |pixels: &[[f32; 4]]| jumps(&speck(pixels), pixels, 200, &limits(), JUMP_AT);
    assert!(dust_jump(at(&sharp)).is_some_and(|j| j >= STRICT), "{:?}", at(&sharp));
    assert!(dust_jump(at(&soft)).is_none_or(|j| j < STRICT), "twin: {:?}", at(&soft));
    assert_eq!(dust_at(&soft, 0.0), 0);
}

#[test]
fn a_highlights_dense_core_does_not_make_its_soft_edge_a_jump() {
    // A 30-pixel lamp: its core far past the window, its rim just past it, what lies around just inside.
    let mut lamp = vec![grey(0.5); 40_000];
    for row in 90..130usize {
        for col in 90..130usize {
            let off = (row.abs_diff(110)).max(col.abs_diff(110));
            lamp[row * 200 + col] = grey(if off < 13 { 0.001 } else if off < 16 { 0.09 } else { 0.11 });
        }
    }
    assert_eq!(dust_at(&lamp, 0.0), 0);
    // Twin: the core alone, with no soft rim, is what a large opaque speck looks like.
    assert_eq!(dust_at(&with_block(95, 30, [0.001; 3]), 0.0), 1);
}

fn with_speck_at(x: usize, y: usize, light: f32, pixels: &mut [[f32; 4]]) {
    for row in y..y + 4 {
        for col in x..x + 4 {
            pixels[row * 200 + col] = grey(light);
        }
    }
}

#[test]
fn sensitivity_reads_further_inside_the_window() {
    // Inside the window by 0.3 density: density 0.7 on a ground at 0.3, the white at 1.0.
    let mut pixels = vec![grey(0.5); 40_000];
    with_speck_at(100, 100, 0.2, &mut pixels);
    assert_eq!(dust_at(&pixels, 0.0), 0, "twin: at rest, the window's end alone");
    assert_eq!(dust_at(&pixels, 0.74), 1, "read at the inset past its density");
    assert_eq!(dust_at(&pixels, 1.0), 1);
}

#[test]
fn a_speck_ringed_stands_clear_of_the_veil_on_the_map() {
    // The same speck inside the window: the display veils what the map has not reached, never a speck ringed.
    let mut pixels = vec![grey(0.5); 40_000];
    with_speck_at(100, 100, 0.2, &mut pixels);
    let reading = read(&pixels, 200, &limits(), SIDE);
    assert_eq!(reading.at(0.5).len(), 1);
    for s in [0.5, 0.74, 1.0] {
        for speck in reading.at(s) {
            for run in &speck.runs {
                assert!((run.start..run.end).all(|x| u32::from(reading.map.texels[run.row as usize * 200 + x as usize]) <= notch(s)), "at {s}");
            }
        }
    }
    assert!(u32::from(reading.map.texels[50 * 200 + 50]) > notch(0.74), "twin: the ground beside it stays veiled");
}

#[test]
fn a_speck_once_found_stays_found_as_sensitivity_rises() {
    // A speck past the window, in a band that a deeper inset reads too: there the two merge into one
    // shape far too wide to be a speck.
    let mut pixels = vec![grey(0.5); 40_000];
    for row in 90..110usize {
        for col in 20..180usize {
            pixels[row * 200 + col] = grey(0.2);
        }
    }
    with_speck_at(100, 98, 0.01, &mut pixels);
    for s in [0.0, 0.2, 0.4, 0.6, 0.8, 1.0] {
        assert_eq!(dust_at(&pixels, s), 1, "at {s}");
    }
    // Twin: read at the deep inset alone, the speck is lost in the band.
    let alone = candidates(&pixels, 200, &limits(), 2);
    assert!(alone.iter().all(|c| c.speck.area > 16), "{alone:?}");
}

#[test]
fn a_speck_read_at_two_insets_counts_once_and_whole() {
    // A core past the window, a rim read at the next inset: both are shapes, nested.
    let mut pixels = vec![grey(0.5); 40_000];
    for row in 98..108usize {
        for col in 98..108usize {
            pixels[row * 200 + col] = grey(0.2);
        }
    }
    with_speck_at(101, 101, 0.01, &mut pixels);
    let reading = read(&pixels, 200, &limits(), SIDE);
    assert_eq!(reading.at(0.0)[0].area, 16, "the core alone at rest");
    let deep = reading.at(0.74);
    assert_eq!(deep.len(), 1, "{deep:?}");
    assert_eq!(deep[0].area, 100, "twin: the whole shape, not the core");
}

#[test]
fn the_map_holds_what_shows_at_every_notch_with_no_pixel_read_again() {
    // A patch inside the reach, a speck past the window, the rest of the frame beyond the reach.
    let mut pixels = vec![grey(0.95); 40_000];
    for row in 20..40usize {
        for col in 20..40usize {
            pixels[row * 200 + col] = grey(0.2);
        }
    }
    with_speck_at(100, 100, 0.01, &mut pixels);
    let reading = read(&pixels, 200, &limits(), SIDE);
    let at = |x: usize, y: usize| reading.map.texels[y * 200 + x];
    assert_eq!((reading.map.width, reading.map.height), (200, 200));
    assert_eq!(reading.past, 16, "the speck alone is past the window itself");
    assert_eq!(at(101, 101), 0, "past at rest");
    // Density 0.7 under a white at 1.0: 0.3 of a reach of 0.8.
    let expected = (0.3 / REACH * NOTCHES).ceil() as i32;
    assert!((i32::from(at(30, 30)) - expected).abs() <= 1, "{:?} against {expected}", at(30, 30));
    assert_eq!(at(150, 150), NEVER, "twin: beyond the reach, never");
    // The map answers as the reading does.
    assert!(u32::from(at(30, 30)) > notch(0.0) && u32::from(at(30, 30)) <= notch(0.5));
}

#[test]
fn a_lone_pixel_of_a_large_frame_stays_lit_in_the_reduced_map() {
    let (w, h) = (1000usize, 800usize);
    let mut pixels = vec![grey(0.5); w * h];
    pixels[(h - 1) * w + w - 1] = grey(0.01);
    let reading = read(&pixels, w, &limits(), 300);
    assert_eq!((reading.map.width, reading.map.height), (300, 240));
    let lit: Vec<usize> = reading.map.texels.iter().enumerate().filter(|&(_, &t)| t == 0).map(|(i, _)| i).collect();
    assert_eq!(lit, vec![300 * 240 - 1], "twin: rounding would have lost the last cell");
    // Every source pixel reads once, each cell's rows being its own.
    assert_eq!(reading.past, 1);
}

#[test]
fn a_map_reduced_keeps_the_earliest_notch_of_its_pixels() {
    let (w, h) = (400usize, 400usize);
    let mut pixels = vec![grey(0.5); w * h];
    pixels[100 * w + 100] = grey(0.2);
    pixels[100 * w + 101] = grey(0.01);
    let reading = read(&pixels, w, &limits(), 200);
    assert_eq!(reading.map.texels[50 * 200 + 50], 0, "the darkest of the cell's pixels sets it");
}

fn pixels_of(runs: &[(u32, u32, u32)]) -> Speck {
    let runs: Vec<Run> = runs.iter().map(|&(row, start, end)| Run { row, start, end }).collect();
    Speck { area: runs.iter().map(|r| r.end - r.start).sum(), bounds: [0; 4], runs }
}

#[test]
fn a_stroke_follows_a_curved_hair_where_a_straight_one_could_not() {
    // A U of a thousand frame pixels: two arms 80 long, 60 apart, joined along the bottom.
    let mut runs: Vec<(u32, u32, u32)> = (100..180).map(|row| (row, 100, 101)).collect();
    runs.extend((100..180).map(|row| (row, 160, 161)));
    runs.push((180, 100, 161));
    runs.sort();
    let (points, radius) = stroke(&pixels_of(&runs), 1000, 1000);
    assert!(radius <= HALO + 4.0, "the walk runs along it: {radius}");
    assert!(points.len() > 10);
    // Twin: the chord between its two ends would have to reach 80 pixels down.
    let (a, b) = (points[0], *points.last().unwrap());
    let chord = distance_to_segment((100.5, 180.5), (a.0 * 1000.0, a.1 * 1000.0), (b.0 * 1000.0, b.1 * 1000.0));
    assert!(chord > 70.0, "{chord}");
}

#[test]
fn a_stroke_covers_a_blob_and_a_lone_pixel_holds_the_brushs_least() {
    let blob: Vec<(u32, u32, u32)> = (50..70).map(|row| (row, 50, 70)).collect();
    let (points, radius) = stroke(&pixels_of(&blob), 100, 100);
    // Corner to corner: the far corners stand 14.1 off the diagonal.
    assert!((radius - (HALO + 14.14)).abs() < 1.0, "{radius}");
    assert!(points.iter().all(|p| (0.5..0.7).contains(&p.0) && (0.5..0.7).contains(&p.1)), "{points:?}");
    let (points, radius) = stroke(&pixels_of(&[(10, 20, 21)]), 100, 100);
    assert_eq!(points.len(), 2, "two points, as a hand's stroke holds");
    assert_ne!(points[0], points[1], "twin: one point twice paints nothing in a path");
    assert_eq!(radius, Correction::RADIUS_RANGE.0.max(HALO));
}
