//! `Pipette.selfCheck` without a file: the stored radius held to the track, and the rim the hand
//! aims with being the sample, at every radius.

use oni_measure::pipette::{self, DEFAULT_RADIUS, RADIUS_RANGE, disc_offsets, read};
use oni_measure::plane::Plane;
use oni_settings::cg::Point;

#[test]
fn a_stored_radius_is_held_to_the_track_and_the_smallest_disc_holds_pixels() {
    assert_eq!(pipette::radius(0.0), RADIUS_RANGE.0);
    assert_eq!(pipette::radius(1e6), RADIUS_RANGE.1);
    assert_eq!(pipette::radius(f64::NAN), DEFAULT_RADIUS);
    assert_eq!(pipette::radius(7.0), 7.0);
    assert!(disc_offsets(RADIUS_RANGE.0).len() >= 5, "{} px", disc_offsets(RADIUS_RANGE.0).len());
    // Twin: under a pixel the disc holds its centre alone.
    assert_eq!(disc_offsets(0.9).len(), 1);
}

/// The last integer offset inside the rim and the first outside, `dx` positive.
fn rim_straddle(radius: f64) -> Option<((i64, i64), (i64, i64))> {
    let reach = radius.ceil() as i64 + 1;
    let r2 = (radius * radius) as f32;
    type Best = Option<((i64, i64), i64)>;
    let (mut inside, mut outside): (Best, Best) = (None, None);
    for y in 0..=reach {
        for x in 1..=reach {
            let d2 = x * x + y * y;
            if d2 as f32 <= r2 {
                if inside.is_none_or(|i| d2 > i.1) {
                    inside = Some(((x, y), d2));
                }
            } else if outside.is_none_or(|o| d2 < o.1) {
                outside = Some(((x, y), d2));
            }
        }
    }
    Some((inside?.0, outside?.0))
}

/// A flat 1400 × 933 frame with at most one white pixel at an offset from (700, 466), rows top first.
fn speckled(offset: Option<(i64, i64)>) -> Plane {
    let (width, height) = (1400usize, 933usize);
    let mut p = Plane::flat(width, height, [0.5, 0.5, 0.5, 1.0]);
    if let Some((x, y)) = offset {
        p.pixels[(466 + y) as usize * width + (700 + x) as usize] = [1.0, 1.0, 1.0, 1.0];
    }
    p
}

#[test]
fn the_ring_the_hand_aims_with_is_the_sample_at_every_radius() {
    let centre = Point::new(700.5 / 1400.0, 466.5 / 933.0);
    let mean = |p: &Plane, at: Point, r: f64| read(p, at, r);
    let radii = [RADIUS_RANGE.0, DEFAULT_RADIUS, DEFAULT_RADIUS + 0.5, (RADIUS_RANGE.0 + RADIUS_RANGE.1) / 2.0, RADIUS_RANGE.1];
    let flat_plane = speckled(None);
    for radius in radii {
        let flat = mean(&flat_plane, centre, radius).unwrap();
        assert_eq!(flat.pixels, disc_offsets(radius).len(), "r={radius}: the disc holds its membership test's count");
        let (inside, outside) = rim_straddle(radius).unwrap();
        let moved = |offset, at| (mean(&speckled(Some(offset)), at, radius).unwrap().fractions[0] - flat.fractions[0]).abs();
        assert!(moved(inside, centre) > 0.0, "r={radius}: a speck inside the rim at {inside:?} moves the reading");
        assert_eq!(moved(outside, centre), 0.0, "r={radius}: a speck outside it at {outside:?} does not");
        // Twin: half a pixel of slip in the centre drops the inside speck out of the sample.
        let half_off = Point::new((centre.x * 1400.0 - 0.5) / 1400.0, centre.y);
        assert_eq!(moved(inside, half_off), 0.0, "r={radius}: the slipped disc still holds {inside:?}");
    }
}

#[test]
fn a_disc_off_the_frame_reads_nothing_and_one_at_its_edge_reads_what_it_holds() {
    let p = speckled(None);
    assert!(read(&p, Point::new(1.2, 0.5), 4.0).is_none());
    let edge = read(&p, Point::new(0.0, 1.0), 4.0).unwrap();
    // Twin: the corner holds about a quarter of the disc, never the whole of it.
    assert!(edge.pixels < disc_offsets(4.0).len() / 2 && edge.pixels > 0, "{} px at the corner", edge.pixels);
}
