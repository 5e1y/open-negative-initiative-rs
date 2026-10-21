//! `Blocks.selfCheck`: the grid names its cells, drops a speck, keeps an area, and reads a roll
//! through its windows; each claim beside the twin that fails it.

use oni_measure::blocks::{Grid, histogram, reading};
use oni_measure::plane::Plane;
use oni_settings::cg::Rect;

const BINS: usize = 64;

fn flat(v: f32, w: usize, h: usize) -> Plane {
    Plane::flat(w, h, [v, v, v, 1.0])
}

fn patch(p: Plane, v: f32, r: Rect) -> Plane {
    p.painted(&r, [v, v, v, 1.0])
}

/// `Pipeline.histogram`: every pixel counted, the twin a block reading is judged against.
fn pixel_histogram(p: &Plane) -> Vec<[f32; 3]> {
    let rgb: Vec<[f32; 3]> = p.pixels.iter().map(|q| [q[0], q[1], q[2]]).collect();
    histogram(&rgb, &[], BINS).rgb
}

fn white_end(r: &oni_measure::blocks::Reading) -> f32 {
    r.histogram.ends.as_ref().map_or(-1.0, |e| e[0][1])
}

fn bright_bin() -> usize {
    ((0.9 * BINS as f32) as usize).min(BINS - 1)
}

#[test]
fn the_grid_names_its_cells_as_a_sheet_is_read() {
    let window = Rect::new(0.0, 0.0, 300.0, 200.0);
    let grid = Grid::new(window);
    assert_eq!((grid.rows, grid.columns), (10, 15), "a 3:2 window holds square cells");
    assert_eq!(
        [Grid::name(0, 0), Grid::name(1, 0), Grid::name(0, 1), Grid::name(0, 26), grid.name_of(grid.columns)],
        ["A1", "A2", "B1", "AA1", "A2"].map(String::from)
    );
    let top_left = grid.cell(0, 0);
    assert!((top_left.y + top_left.height - 200.0).abs() < 1e-9 && top_left.x.abs() < 1e-9, "A1 is the top-left in a bottom-up frame");
    let copy = Rect::new(0.0, 0.0, 1400.0, 933.0);
    let sheet_frame = Grid::new(Rect::new(0.0, 0.0, 168.0, 112.0));
    assert_eq!((Grid::new(copy).rows, sheet_frame.rows, sheet_frame.columns), (48, 6, 9));
    // Twin: a sliver still holds one row, never none.
    assert_eq!(Grid::new(Rect::new(0.0, 0.0, 40.0, 10.0)).rows, 1);
}

#[test]
fn a_speck_never_votes_and_an_area_places_the_end_on_its_own_value() {
    let window = Rect::new(0.0, 0.0, 300.0, 200.0);
    let grid = Grid::new(window);
    let ground = flat(0.2, 300, 200);
    let speck = patch(ground.clone(), 0.9, Rect::new(100.0, 100.0, 1.0, 1.0));
    let blocks = reading(&speck, &[window], BINS);
    let bright = bright_bin();
    let counted: f32 = blocks.histogram.rgb.iter().map(|b| b[0]).sum();
    assert!(blocks.histogram.rgb[bright][0] == 0.0 && counted == grid.count() as f32, "a 1 px speck leaves no block in its bin");
    // Twin: the pixel histogram counts the same speck.
    assert!(pixel_histogram(&speck)[bright][0] > 0.0);
    let ends = blocks.histogram.ends.as_ref().unwrap()[0];
    assert!((ends[1] - 0.2).abs() < 0.02 && (ends[0] - 0.2).abs() < 0.02, "the ends stay on the ground: {ends:?}");

    // A 6 × 6 area inside one cell, 9 % of it: past the tail, so it places the white end alone.
    let lit = reading(&patch(ground.clone(), 0.9, Rect::new(100.0, 100.0, 6.0, 6.0)), &[window], BINS);
    assert!((white_end(&lit) - 0.9).abs() < 0.02 && lit.histogram.rgb[bright][0] == 0.0, "white end {}", white_end(&lit));
    assert_eq!(lit.report.as_ref().unwrap().white[0], "F5", "the white end is named after its cell");

    // Larger than a cell: whole cells inside it carry its value, and are named.
    let patch_rect = Rect::new(100.0, 100.0, 30.0, 30.0);
    let patched = reading(&patch(ground.clone(), 0.9, patch_rect), &[window], BINS);
    assert!(patched.histogram.rgb[bright][0] >= 1.0, "a 30 px patch survives the median");
    let report = patched.report.unwrap();
    let inside: Vec<String> = (0..grid.count())
        .filter(|&i| {
            let c = grid.cell(i / grid.columns, i % grid.columns);
            c.x < patch_rect.x + patch_rect.width && patch_rect.x < c.x + c.width && c.y < patch_rect.y + patch_rect.height && patch_rect.y < c.y + c.height
        })
        .map(|i| grid.name_of(i))
        .collect();
    assert!(inside.contains(&report.white[0]), "{} is not inside the patch", report.white[0]);
    assert!(!report.black[0].is_empty() && report.black[0] != report.white[0]);
}

#[test]
fn the_tail_is_a_share_of_the_cell_so_a_hair_falls_and_a_thread_stays() {
    let copy = Rect::new(0.0, 0.0, 1400.0, 933.0);
    let hair = patch(flat(0.2, 1400, 933), 0.9, Rect::new(0.0, 400.0, 1400.0, 1.0));
    assert!((white_end(&reading(&hair, &[copy], BINS)) - 0.2).abs() < 0.02, "a 1 px hair never places the white end");
    // Twin: a 3 px thread does.
    let thread = patch(flat(0.2, 1400, 933), 0.9, Rect::new(0.0, 400.0, 1400.0, 3.0));
    assert!((white_end(&reading(&thread, &[copy], BINS)) - 0.9).abs() < 0.02);
    // A click pays the reading three times and may take 100 ms in all; the best of three keeps a
    // machine busy with other checks out of the measure.
    let cost = (0..3)
        .map(|_| {
            let clocked = std::time::Instant::now();
            let _ = reading(&thread, &[copy], BINS);
            clocked.elapsed().as_secs_f64() * 1000.0
        })
        .fold(f64::MAX, f64::min);
    println!("a 1400 × 933 reading of {} cells costs {cost:.1} ms", Grid::new(copy).count());
    assert!(cost < 30.0, "{cost:.1} ms, past 30");
}

#[test]
fn a_window_reads_only_what_it_holds_and_a_roll_reads_through_its_frames() {
    let window = Rect::new(0.0, 0.0, 300.0, 200.0);
    let outside = patch(flat(0.2, 300, 200), 0.9, Rect::new(20.0, 20.0, 60.0, 60.0));
    let right = Rect::new(150.0, 0.0, 150.0, 200.0);
    let windowed = reading(&outside, &[right], BINS);
    assert!(windowed.histogram.rgb[bright_bin()][0] == 0.0 && windowed.report.unwrap().grids[0].columns == 8);
    // Twin: the whole window does see it.
    assert!(reading(&outside, &[window], BINS).histogram.rgb[bright_bin()][0] > 0.0);

    // Ten frames on a dark ground: one grid over the ten, no cell astride a gutter voting.
    let roll = Rect::new(0.0, 0.0, 1000.0, 100.0);
    let frames: Vec<Rect> = (0..10).map(|i| Rect::new(f64::from(i) * 100.0 + 10.0, 10.0, 80.0, 80.0)).collect();
    let framed = frames.iter().fold(flat(0.02, 1000, 100), |p, r| patch(p, 0.5, *r));
    let rolled = reading(&framed, &frames, BINS);
    let black = rolled.histogram.ends.as_ref().unwrap()[0][0];
    assert!((black - 0.5).abs() < 0.02 && rolled.report.unwrap().grids.len() == 1, "the roll's black is the frames': {black}");
    // Twin: read whole, the gutters set the black.
    let whole = reading(&framed, &[roll], BINS).histogram.ends.unwrap()[0][0];
    assert!(whole < 0.1, "{whole}");
}

/// The wiring: every balance reader takes `Reader::reading`, and none is offered a pixel path.
#[test]
fn every_balance_reader_reads_through_one_door() {
    let key = include_str!("../src/key.rs");
    let door = "r.reading(";
    assert!(key.matches(door).count() >= 3, "the click, the re-measure and the ends read through one door");
    assert!(!key.contains("blocks::medians") && !key.contains("blocks::reading("), "no key reads cells itself");
}
