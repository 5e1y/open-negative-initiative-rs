//! The grain's clumps taking the blurred scene: kept where the blur is the pixel or the light has no
//! share, none between clumps, the whole dose inside them; each verdict with a twin.

use super::*;

/// A grain of 0.4 stop on cells of 0.7 pixel at the whole share, taking all of the blur in its clumps.
fn dissolving() -> Grain {
    Grain::on([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], 0.7, 1.0, 0.4, [1.0; GRAIN_NODES]).coloured([2.0; GRAIN_NODES]).dissolving(1.0)
}

/// 4096 pixel centres, 64 a side.
fn centres() -> impl Iterator<Item = V2> {
    (0..64).flat_map(|y| (0..64).map(move |x| [x as f32 + 0.5, y as f32 + 0.5]))
}

const SHARP: V4 = [0.05, 0.032, 0.0215, 1.0];
const BLURRED: V4 = [0.03, 0.04, 0.05, 1.0];

#[test]
fn a_blur_equal_to_the_pixel_is_kept_to_the_bit() {
    let (k, none) = (dissolving(), dissolving().dissolving(0.0));
    let mut moved = 0usize;
    for at in centres() {
        assert_eq!(grain(SHARP, SHARP, at, &k), grain(SHARP, SHARP, at, &none), "at {at:?}");
        // Twin: a blur unlike the pixel moves it somewhere.
        moved += usize::from(grain(SHARP, BLURRED, at, &k) != grain(SHARP, SHARP, at, &k));
    }
    assert!(moved > 500, "the twin moves {moved} pixels");
}

#[test]
fn a_light_with_no_share_is_kept_however_its_blur_differs() {
    let mut k = dissolving();
    k.share = [0.0; GRAIN_NODES];
    let mut twin = 0usize;
    for at in centres() {
        assert_eq!(grain(SHARP, BLURRED, at, &k), SHARP, "at {at:?}");
        // Twin: the blur mixed in before the share is read, at the whole dose.
        let early = with_alpha(map(rgb(SHARP), |c, v| v + mixed(field(at, &k), 1.0, &k) * (BLURRED[c] - v)), 1.0);
        twin += usize::from(early != SHARP);
    }
    assert!(twin > 500, "the twin moves {twin} pixels");
}

#[test]
fn between_clumps_the_pixel_stays_sharp_and_inside_one_it_takes_the_blur_at_its_dose() {
    let k = Grain { share: [0.6; GRAIN_NODES], ..dissolving().dissolving(0.8) };
    let spread = mean_spread(k.per_cell);
    let (mut between, mut inside, mut linear) = (0usize, 0usize, 0usize);
    for at in centres() {
        let f = field(at, &k);
        let out = grain(SHARP, BLURRED, at, &k);
        let sharp = grain(SHARP, SHARP, at, &k);
        let reach = f.abs() / spread;
        if reach < 0.5 {
            between += 1;
            assert_eq!(out, sharp, "at {at:?}: a pixel between clumps moves");
            // Twin: a mix linear in the field, which softens between clumps too.
            linear += usize::from(reach.min(1.0) * 0.48 > 1e-3);
        } else if reach > 1.5 {
            inside += 1;
            let w = 0.8 * 0.6;
            let base = with_alpha(map(rgb(SHARP), |c, v| v + w * (BLURRED[c] - v)), 1.0);
            assert_eq!(out, laid(base, k.stops * 0.6 * f, 2.0, -k.stops * 0.6 * f), "at {at:?}: a clump takes another dose");
        }
        // The mix is convex: the base never leaves the span from the pixel to its blur.
        let w = mixed(f, 0.6, &k);
        assert!((0.0..=0.8 * 0.6).contains(&w), "at {at:?}: {w}");
    }
    assert!(between > 500 && inside > 100, "{between} between clumps, {inside} inside");
    assert!(linear > between / 2, "the twin softens {linear} of {between}");
}

/// Hazed, a pixel between clumps takes `haze` of the dose, one inside a clump the whole of it, light or dark
/// alike; twin: the haze left out keeps the pixel between clumps sharp.
#[test]
fn a_haze_lays_its_share_of_the_dose_between_clumps_and_the_whole_inside_them() {
    let k = Grain { share: [0.6; GRAIN_NODES], ..dissolving().dissolving(0.8).hazed(0.5) };
    let spread = mean_spread(k.per_cell);
    let (mut between, mut light, mut dark) = (0usize, 0usize, 0usize);
    for at in centres() {
        let f = field(at, &k);
        let reach = f.abs() / spread;
        let w = mixed(f, 0.6, &k);
        if reach < 0.5 {
            between += 1;
            assert!((w - 0.8 * 0.6 * 0.5).abs() < 1e-6, "at {at:?}: {w} between clumps");
            assert_ne!(grain(SHARP, BLURRED, at, &k), grain(SHARP, BLURRED, at, &k.hazed(0.0)), "twin: the haze moves it");
            assert_eq!(grain(SHARP, BLURRED, at, &k.hazed(0.0)), grain(SHARP, SHARP, at, &k.hazed(0.0)));
        } else if reach > 1.5 {
            assert!((w - 0.8 * 0.6).abs() < 1e-6, "at {at:?}: {w} inside a clump");
            if f > 0.0 { light += 1 } else { dark += 1 }
        }
    }
    assert!(between > 100 && light > 50 && dark > 50, "{between} between, {light} light, {dark} dark");
}
