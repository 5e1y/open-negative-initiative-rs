//! A speck's strokes on shapes whose walks are known.

use super::stroke::{HALO, distance_to_segment};
use super::*;
use oni_settings::frame::Correction;

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
    let parts = strokes(&pixels_of(&runs), 1000, 1000);
    assert!(parts.iter().all(|(_, radius)| *radius <= HALO + 4.0), "the walk runs along it: {parts:?}");
    assert!(parts.windows(2).all(|w| w[0].0.last() == w[1].0.first()), "one walk, each part from the last's end");
    let points: Vec<(f64, f64)> = parts.iter().flat_map(|(points, _)| points.iter().copied()).collect();
    assert!(points.len() > 10);
    // Twin: the chord between its two ends would have to reach 80 pixels down.
    let (a, b) = (points[0], *points.last().unwrap());
    let chord = distance_to_segment((100.5, 180.5), (a.0 * 1000.0, a.1 * 1000.0), (b.0 * 1000.0, b.1 * 1000.0));
    assert!(chord > 70.0, "{chord}");
}

#[test]
fn a_stroke_covers_a_blob_and_a_lone_pixel_holds_the_brushs_least() {
    let blob: Vec<(u32, u32, u32)> = (50..70).map(|row| (row, 50, 70)).collect();
    let [(points, radius)] = &strokes(&pixels_of(&blob), 100, 100)[..] else { panic!("a blob is one stroke") };
    let radius = *radius;
    // Corner to corner: the far corners stand 14.1 off the diagonal.
    assert!((radius - (HALO + 14.14)).abs() < 1.0, "{radius}");
    assert!(points.iter().all(|p| (0.5..0.7).contains(&p.0) && (0.5..0.7).contains(&p.1)), "{points:?}");
    let [(points, radius)] = &strokes(&pixels_of(&[(10, 20, 21)]), 100, 100)[..] else { panic!("a pixel is one stroke") };
    let radius = *radius;
    assert_eq!(points.len(), 2, "two points, as a hand's stroke holds");
    assert_ne!(points[0], points[1], "twin: one point twice paints nothing in a path");
    assert_eq!(radius, Correction::RADIUS_RANGE.0.max(HALO));
}

/// How far past its halo the nearest stroke leaves each pixel of a shape: none, when every pixel is covered.
fn uncovered(shape: &Speck, parts: &[(Vec<(f64, f64)>, f64)], side: f64) -> f64 {
    let cells = shape.runs.iter().flat_map(|r| (r.start..r.end).map(move |x| (f64::from(x) + 0.5, f64::from(r.row) + 0.5)));
    let on = |p: (f64, f64)| (p.0 * side, p.1 * side);
    let off = |c: (f64, f64), (points, radius): &(Vec<(f64, f64)>, f64)| points.windows(2).map(|w| distance_to_segment(c, on(w[0]), on(w[1]))).fold(f64::MAX, f64::min) - (radius - HALO);
    cells.map(|c| parts.iter().map(|part| off(c, part)).fold(f64::MAX, f64::min)).fold(f64::MIN, f64::max)
}

#[test]
fn a_loop_and_a_branch_get_walks_of_their_own_where_one_walk_would_cover_what_they_enclose() {
    // A square loop 80 across, a pixel wide, and a hair 120 long with a branch 50 long from its middle.
    let mut ring: Vec<(u32, u32, u32)> = vec![(100, 100, 181), (180, 100, 181)];
    ring.extend((101..180).flat_map(|row| [(row, 100, 101), (row, 180, 181)]));
    ring.sort();
    let mut branched: Vec<(u32, u32, u32)> = (100..220).map(|row| (row, 100, 101)).collect();
    branched[60] = (160, 100, 151);
    for runs in [ring, branched] {
        let shape = pixels_of(&runs);
        let parts = strokes(&shape, 1000, 1000);
        // A walk takes in what lies within a halo of it, the next walk's first pixels: never wider than two.
        assert!(parts.iter().all(|(_, radius)| *radius <= 2.0 * HALO), "{:?}", parts.iter().map(|p| p.1).collect::<Vec<_>>());
        assert!(uncovered(&shape, &parts, 1000.0) <= 1e-9, "{}", uncovered(&shape, &parts, 1000.0));
        assert!(parts.windows(2).any(|w| w[0].0.last() != w[1].0.first()), "more than one walk");
        // Twin: the first walk alone, however wide a brush's range lets it be, leaves the far side bare.
        let first: Vec<_> = parts.iter().take_while(|p| p.0.first() == parts[0].0.first() || parts.iter().any(|q| q.0.last() == p.0.first())).cloned().collect();
        assert!(first.len() < parts.len() && uncovered(&shape, &first, 1000.0) > 30.0, "{}", uncovered(&shape, &first, 1000.0));
    }
}

#[test]
fn a_hairs_strokes_span_what_a_hands_may_and_a_clump_on_it_widens_its_own_alone() {
    // A hair 200 long, a pixel wide, with a clump 16 across at its middle.
    let mut runs: Vec<(u32, u32, u32)> = (100..300).map(|row| (row, 100, 101)).collect();
    for row in 192..208 {
        runs[row - 100] = (row as u32, 92, 108);
    }
    let shape = pixels_of(&runs);
    let parts = strokes(&shape, 1000, 1000);
    let span = |points: &[(f64, f64)]| points.iter().fold(0.0f64, |most, p| most.max((p.1 - points[0].1).abs().max((p.0 - points[0].0).abs()) * 1000.0));
    assert!(parts.iter().all(|(points, radius)| span(points) <= Correction::SPAN * radius * 1.25 + 1e-9), "{parts:?}");
    assert!(uncovered(&shape, &parts, 1000.0) <= 1e-9);
    let radii: Vec<f64> = parts.iter().map(|p| p.1).collect();
    let (thin, wide) = (radii.iter().filter(|r| **r <= HALO + 1.0).count(), radii.iter().filter(|r| **r >= HALO + 6.0).count());
    assert!(thin >= 6 && (1..=2).contains(&wide), "{radii:?}");
    // Twin: the hair alone is thin all along.
    let hair = pixels_of(&(100..300).map(|row| (row, 100, 101)).collect::<Vec<_>>());
    assert!(strokes(&hair, 1000, 1000).iter().all(|(_, radius)| *radius <= HALO + 1.0));
}
