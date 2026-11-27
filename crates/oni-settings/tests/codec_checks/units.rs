//! The migrations that change a unit, whose order is load-bearing: windows onto the density axis, radii onto
//! pixels, the decode's gain shift and the scanner's counts.

use super::*;

#[test]
fn the_per_channel_windows_move_onto_the_density_axis_before_the_defaults() {
    let mut placed = PipelineSettings::default();
    placed.levels.red = Levels { black: 0.0731, white: 0.8137, mid: 0.3117, ..Levels::default() };
    placed.levels.green = Levels { black: 0.1279, white: 0.7411, mid: 0.6413, ..Levels::default() };
    placed.levels.blue = Levels { black: 0.2113, white: 0.9337, mid: 0.5, ..Levels::default() };
    placed.gains.stops = [1.5, -0.5, 2.125];
    let mut scaled = placed.levels;
    for lv in [&mut scaled.red, &mut scaled.green, &mut scaled.blue] {
        (lv.black, lv.white) = (lv.black * pipeline::DMAX, lv.white * pipeline::DMAX);
    }
    for stamp in [0, before_1_0::DENSITY_AXIS_VERSION - 1, before_1_0::DENSITY_AXIS_VERSION] {
        let read = decode(&aged(&placed, stamp, |_| {}));
        let wanted = if stamp < before_1_0::DENSITY_AXIS_VERSION { scaled } else { placed.levels };
        assert_eq!(read.levels, wanted, "version {stamp}");
    }
    // Twin: the placed windows really move under the scaling.
    assert_ne!(placed.levels, scaled);
    let old = aged(&placed, before_1_0::DENSITY_AXIS_VERSION - 1, |_| {});
    let read = decode(&old);
    for c in LevelsChannel::PER_CHANNEL {
        assert_eq!(read.levels.get(c).mid, placed.levels.get(c).mid);
    }
    assert_eq!((read.gains, &read.manual_curves, read.print), (placed.gains, &placed.manual_curves, migrate::departure::read_back(1, placed.clone()).print));

    let missing = aged(&placed, before_1_0::DENSITY_AXIS_VERSION - 1, |s| {
        s["levels"].as_object_mut().unwrap().remove("red");
    });
    let holed = decode(&missing);
    assert_eq!(holed.levels.red, LevelsSet::default().red);
    assert_eq!((holed.levels.green, holed.levels.blue), (scaled.green, scaled.blue));
    // Twin: the forbidden order, defaults then scale, triples the absent default.
    let stored: Value = serde_json::from_slice(&missing).unwrap();
    let completed = chain::merged(&stored, chain::neutral_document());
    let wrong = before_1_0::in_density(completed.as_object().unwrap().clone());
    let wrong: PipelineSettings = serde_json::from_value(wrong["settings"].clone()).unwrap();
    assert_eq!(wrong.levels.red.white, LevelsSet::default().red.white * pipeline::DMAX);

    let partial = decode(&aged(&placed, before_1_0::DENSITY_AXIS_VERSION - 1, |s| {
        s["levels"]["blue"].as_object_mut().unwrap().remove("white");
    }));
    assert_eq!(partial.levels.blue.white, LevelsSet::default().blue.white);
    assert_eq!(partial.levels.blue.black, scaled.blue.black);
}

#[test]
fn the_two_radii_move_onto_pixels_before_the_defaults() {
    let side = document::RADIUS_REFERENCE_SIDE as f32;
    let document = |chroma: Option<(f32, f32)>, sharpen: Option<f32>, stamp: i64, holing: bool| {
        aged(&PipelineSettings::default(), stamp, |s| {
            if let Some((r, f)) = chroma {
                s.insert("chroma".into(), json!({"radius": r as f64, "force": f as f64}));
            }
            let block = s["sharpen"].as_object_mut().unwrap();
            if holing {
                block.remove("radius");
            } else if let Some(r) = sharpen {
                block.insert("radius".into(), json!(r as f64));
            }
        })
    };
    let hand = (0.0005f32, 0.62f32);
    let sharpened = 0.00037f32;
    for stamp in [0, before_1_0::RADIUS_PIXELS_VERSION - 1, before_1_0::RADIUS_PIXELS_VERSION] {
        let read = decode(&document(Some(hand), Some(sharpened), stamp, false));
        let migrates = stamp < before_1_0::RADIUS_PIXELS_VERSION;
        let wanted = if migrates { (hand.0 * side, sharpened * side) } else { (hand.0, sharpened) };
        assert!((read.chroma.radius - wanted.0).abs() < 1e-4, "version {stamp}: {}", read.chroma.radius);
        assert!((read.sharpen.radius - wanted.1).abs() < 1e-4, "version {stamp}: {}", read.sharpen.radius);
    }
    let holed = document(Some(hand), None, before_1_0::RADIUS_PIXELS_VERSION - 1, true);
    let read = decode(&holed);
    assert_eq!(read.sharpen.radius, Sharpen::default().radius);
    assert!((read.chroma.radius - hand.0 * side).abs() < 1e-4);
    // Twin: the forbidden order puts the absent default on the track's stop.
    let stored: Value = serde_json::from_slice(&holed).unwrap();
    let completed = chain::merged(&stored, chain::neutral_document());
    let wrong = before_1_0::in_pixels(completed.as_object().unwrap().clone(), false);
    let wrong: PipelineSettings = serde_json::from_value(wrong["settings"].clone()).unwrap();
    assert_eq!(wrong.sharpen.radius, Sharpen::radius_range().1);
    assert_ne!(wrong.sharpen.radius, Sharpen::default().radius);

    for (r, f) in ChromaDenoise::FORMER_RESTS {
        assert_eq!(decode(&document(Some((r, f)), Some(sharpened), 1, false)).chroma, ChromaDenoise::default());
    }
    let former = decode(&document(None, Some(Sharpen::FORMER_REST), before_1_0::RADIUS_PIXELS_VERSION - 1, false));
    assert_eq!(former.sharpen.radius, Sharpen::RADIUS_ANCHOR);
    // Twin: one percent off that floor is a decision, and is scaled.
    let nearby = decode(&document(None, Some(Sharpen::FORMER_REST * 1.01), before_1_0::RADIUS_PIXELS_VERSION - 1, false));
    assert!(nearby.sharpen.radius != Sharpen::RADIUS_ANCHOR && nearby.sharpen.radius > 0.0);
    let past = decode(&document(Some((0.01, 1.0)), Some(0.002), before_1_0::RADIUS_PIXELS_VERSION - 1, false));
    assert_eq!(past.chroma.radius, ChromaDenoise::RADIUS_RANGE.1);
    assert_eq!(past.sharpen.radius, Sharpen::radius_range().1);
}

#[test]
fn a_mosaic_gain_shifts_for_the_decode_change_and_a_scanner_gain_never() {
    let hand = [0.6f32, -0.3, 0.9];
    let delta = before_1_0::NEUTRAL_DECODE_RATIO.map(libm::log2f);
    let expected: [f32; 3] = std::array::from_fn(|i| hand[i] - delta[i]);
    // A digital photo's gains, which it never rendered then, land on its film's reference.
    for mode in ConversionMode::ALL.into_iter().filter(|m| !m.is_digital()) {
        for stamp in [0, before_1_0::NEUTRAL_DECODE_GAINS_VERSION - 1, before_1_0::NEUTRAL_DECODE_GAINS_VERSION] {
            let mut written = PipelineSettings { mode, ..PipelineSettings::default() };
            written.gains.stops = hand;
            let data = aged(&written, stamp, |_| {});
            let mosaic = decode_from(&data, Some(true));
            let migrates = stamp < before_1_0::NEUTRAL_DECODE_GAINS_VERSION;
            assert_eq!(mosaic.gains.stops, if migrates { expected } else { hand }, "{mode:?} v{stamp}");
            // Twin: read as scanner-sourced, the same document keeps its gains at every version.
            assert_eq!(decode(&data).gains.stops, hand);
        }
    }
    let mut near = PipelineSettings::default();
    near.gains.stops = [-2.9, 0.0, 2.9];
    let read = decode_from(&aged(&near, 0, |_| {}), Some(true));
    assert_eq!((read.gains.stops[0], read.gains.stops[2]), (Gains::RANGE.0, Gains::RANGE.1));
    // Twin: green's small shift stays inside the track.
    assert!((read.gains.stops[1] + delta[1]).abs() < 1e-4);
    assert!(read.gains.stops[1] > Gains::RANGE.0 && read.gains.stops[1] < Gains::RANGE.1);
}

#[test]
fn a_mosaic_raw_never_balanced_rests_knee_free_and_a_balanced_one_keeps_its_axis() {
    let stamped = |build: &dyn Fn(&mut PipelineSettings), version: i64| {
        let mut s = PipelineSettings::default();
        build(&mut s);
        aged(&s, version, |inner| {
            inner.remove("decodedFromMosaic");
        })
    };
    let v = before_1_0::MOSAIC_AXIS_VERSION - 1;
    let read = |data: &[u8], mosaic: bool| decode_from(data, mosaic.then_some(true));
    let untouched = stamped(&|_| {}, v);
    let rested = read(&untouched, true);
    assert_eq!(rested.density_ceiling, density_axis::MOSAIC_REST);
    assert!(rested.decoded_from_mosaic);
    // Twin: the same bytes beside a scanner file move nothing.
    let scanner = read(&untouched, false);
    assert_eq!((scanner.density_ceiling, scanner.decoded_from_mosaic), (density_axis::base(), false));
    let kept = read(
        &stamped(
            &|s| {
                s.levels.blue.white = 2.21;
                s.levels.red.black = 0.37;
            },
            v,
        ),
        true,
    );
    assert!((kept.density_ceiling - former_axis(density_axis::base())).abs() < 1e-6);
    assert_eq!((kept.levels.blue.white, kept.levels.red.black), (2.21, 0.37));
    let former = density_axis::pedestal(density_axis::base()) - before_1_0::FORMER_SHADOW_BIAS;
    let rebuilt = density_axis::pedestal(former_axis(density_axis::base()));
    assert!((rebuilt - former).abs() / former < 1e-5, "{rebuilt} against {former}");
    let dialled = read(
        &stamped(
            &|s| {
                s.density_ceiling = 2.3;
                s.levels.green.white = 2.4;
            },
            v,
        ),
        true,
    );
    assert!((dialled.density_ceiling - former_axis(2.3)).abs() < 1e-6 && dialled.density_ceiling > 2.9);
    let past = read(
        &stamped(
            &|s| {
                s.density_ceiling = 2.5;
                s.levels.green.white = 2.4;
            },
            v,
        ),
        true,
    );
    assert_eq!(past.density_ceiling, density_axis::MOSAIC_REST);
    let current = read(&stamped(&|_| {}, before_1_0::MOSAIC_AXIS_VERSION), true);
    assert_eq!(current.density_ceiling, density_axis::base());
    let slide = read(
        &stamped(
            &|s| {
                s.mode = ConversionMode::Positive;
                s.levels = LevelsSet::neutral(ConversionMode::Positive, density_axis::base());
            },
            v,
        ),
        true,
    );
    assert!((slide.density_ceiling - former_axis(density_axis::base())).abs() < 1e-6);
    assert_eq!(slide.levels.green.black, Levels::resting_black(ConversionMode::Positive, density_axis::base()));
    let graded = read(&stamped(&|s| s.gains.stops = [0.4, 0.0, -0.3], v), true);
    assert!((graded.density_ceiling - former_axis(density_axis::base())).abs() < 1e-6);
}

#[test]
fn a_scan_from_before_keeps_its_converted_primaries() {
    let stamped = |version: i64| {
        let mut s = PipelineSettings::default();
        s.levels.red.black = 0.41;
        aged(&s, version, |inner| {
            inner.remove("primariesConverted");
        })
    };
    let before = chain::PRIMARIES_HELD_VERSION - 1;
    assert!(decode_from(&stamped(before), Some(false)).primaries_converted);
    // Twin: a RAW's sidecar from before, and a scan's written since, are left alone.
    assert!(!decode_from(&stamped(before), Some(true)).primaries_converted);
    assert!(!decode_from(&stamped(chain::PRIMARIES_HELD_VERSION), Some(false)).primaries_converted);
}

#[test]
fn a_scanner_edit_from_before_reads_its_counts_doubled() {
    let stamped = |version: i64| {
        aged(&PipelineSettings::default(), version, |inner| {
            inner.insert("face".into(), json!("scanner"));
            inner.insert("scanner".into(), json!({"d": 3, "c": -5, "m": 7, "y": -12, "w": 4}));
        })
    };
    let keys = |version: i64| decode_from(&stamped(version), None).legacy_print.map(|l| ((l.d, l.c, l.m, l.y, l.w), l.machine));
    assert_eq!(keys(chain::SCANNER_HALF_STEP_VERSION - 1), Some(((12, -20, 28, -48, 8), LegacyMachine::Frontier)));
    // Twin: written since, the counts read as stored, and the ONI machine named since it came.
    assert_eq!(keys(chain::SCANNER_HALF_STEP_VERSION), Some(((6, -10, 14, -24, 8), LegacyMachine::Frontier)));
    assert_eq!(keys(chain::ONI_MACHINE_VERSION), Some(((6, -10, 14, -24, 8), LegacyMachine::Oni)));
}
