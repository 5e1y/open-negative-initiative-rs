use super::*;

/// `ScannerKey.selfCheck`'s render half: the keypad's letters read through the measured table.
#[test]
fn a_density_key_lightens_and_a_colour_key_takes_its_own_channel_off() {
    let roll = check_roll();
    let mid = CHECK_GREY.map(|d| 10f32.powf(-d));
    let rendered = |clicks: ScannerClicks| replica(mid, &PipelineSettings { scanner: clicks, ..roll.clone() }).unwrap();
    let rest = rendered(ScannerClicks::default());
    let (lighter, darker) = (rendered(ScannerClicks::new(2, 0, 0, 0, 0)), rendered(ScannerClicks::new(-2, 0, 0, 0, 0)));
    let (cooler, redder) = (rendered(ScannerClicks::new(0, 2, 0, 0, 0)), rendered(ScannerClicks::new(0, -2, 0, 0, 0)));
    assert!(lighter[1] > rest[1] && darker[1] < rest[1], "D + lightens: green {} against {}", lighter[1], rest[1]);
    assert!(cooler[0] < rest[0] && redder[0] > rest[0], "C + takes red off: {} against {}", cooler[0], rest[0]);
    // Twin: off the Frontier no table is read, so a check through it cannot pass by default.
    assert!(replica(mid, &PipelineSettings { scanner_machine: ScannerMachine::Oni, ..roll.clone() }).is_none());
    // And TEMP + warms the grey: more red than blue than at rest.
    let warm = rendered(ScannerClicks::new(0, 0, 0, 0, 4));
    assert!(warm[0] - warm[2] > rest[0] - rest[2], "{warm:?} against {rest:?}");
}

#[test]
fn the_machine_unmixes_at_the_frame_s_own_dye_separation() {
    let roll = check_roll();
    assert_eq!(roll.separation, 0.6, "a frame at rest unmixes as the Swift machine did");
    let family = roll.separation_family.or(SeparationFamily::resting(roll.mode)).unwrap();
    assert_eq!(separation(&roll), separation_matrix(family, 0.6));
    assert_eq!(separation(&PipelineSettings { separation: 0.0, ..roll.clone() }), IDENTITY);
    // Twin: a dose that moved changes the rows, so the slider is not ignored.
    assert_ne!(separation(&PipelineSettings { separation: 1.0, ..roll.clone() }), separation(&roll));
}

#[test]
fn the_table_reads_and_an_odd_count_sits_between_its_neighbours() {
    let m = &*MACHINE;
    assert_eq!((m.render.cube_dimension, m.warmth_plus.cube_dimension), (27, 17));
    assert_eq!(m.offset(&ScannerClicks::default()), [0.0; 3]);
    let one = m.offset(&ScannerClicks::new(1, 0, 0, 0, 0));
    let two = m.offset(&ScannerClicks::new(2, 0, 0, 0, 0));
    assert!((one[0] - two[0] / 2.0).abs() < 1e-6 && two[0] > 0.0, "{one:?} {two:?}");
    let minus = m.offset(&ScannerClicks::new(-3, 0, 0, 0, 0));
    assert!(minus[0] < 0.0);
    // The lift is on every node of a look, never on the render.
    assert!(m.warmth_plus.cube_data.iter().all(|n| n[0] > -LIFT));
}

#[test]
fn the_oni_machine_rests_lighting_and_moves_the_gains_by_the_clicks() {
    let mut s = PipelineSettings::rest(oni_settings::ConversionMode::Negative, true);
    s.face = Face::Scanner;
    s.print.exposure = 0.7;
    s.density = 0.4;
    s.scanner = ScannerClicks::new(2, 0, 0, 0, 0);
    s.scanner_finish.contrast = 0.2;
    let oni = on_oni_machine(&s).unwrap();
    assert_eq!((oni.print.exposure, oni.print.contrast, oni.density), (0.0, 0.2, 0.0));
    assert!(oni.gains.stops[0] < 0.0, "more density is less light on a negative");
    // The Frontier takes the same conversion, its print bypassed for its own look.
    s.scanner_machine = ScannerMachine::Frontier;
    let frontier = on_oni_machine(&s).unwrap();
    // The ONI machine's conversion whole, its gains moved by the Frontier's rest.
    let rest = stops_of(frontier_rest(), &s);
    assert!(frontier == PipelineSettings { scanner_machine: ScannerMachine::Frontier, gains: frontier.gains, ..oni.clone() });
    assert!((0..3).all(|c| (frontier.gains.stops[c] - oni.gains.stops[c] - rest[c]).abs() < 1e-6), "{:?} {:?}", frontier.gains, oni.gains);
    assert!(rest.iter().any(|r| r.abs() > 0.01), "the rest moves the gains");
    // Twin: Pro leaves the settings alone.
    assert!(on_oni_machine(&PipelineSettings { face: Face::Pro, ..s.clone() }).is_none());
}
