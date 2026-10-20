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
fn pro_s_black_prints_black_and_the_roll_s_grey_on_the_machine_s_neutral() {
    let roll = PipelineSettings { separation: 0.0, ..check_roll() };
    let k = density(&roll);
    let (black, white) = *ENDS;
    assert!(black < 0.3 && white > 1.2 && white < 2.0, "{black} {white}");
    let g = roll.levels.green;
    let read = |k: &ScannerDensity, d: f32| {
        // The scan whose green density, pedestal and gain included, reads `d`.
        let t: [f32; 3] = std::array::from_fn(|c| 10f32.powf(-d) / k.gain[c] - k.pedestal);
        MACHINE.low() + MACHINE.span() * cpu::scanner_density([t[0], t[1], t[2], 1.0], k)[1]
    };
    let at = |d: f32| read(&k, d);
    let pro = g.black + pro_black(&roll) * (g.white - g.black);
    assert!((at(pro) - black).abs() < 0.01, "{} against {black}", at(pro));
    assert!((at(roll_median(&roll)) - MACHINE.neutral()[1]).abs() < 0.01);
    // Twin: the fit's stretch lands Pro's black elsewhere than the table's black.
    assert!((read(&fitted_density(&roll), pro) - black).abs() > 0.03);
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
fn the_frontier_rests_on_its_own_neutral_and_the_others_on_zero() {
    let mut s = PipelineSettings::rest(oni_settings::ConversionMode::Negative, true);
    (s.face, s.scanner_machine) = (Face::Scanner, ScannerMachine::Frontier);
    let frontier = on_oni_machine(&s).unwrap();
    let oni = on_oni_machine(&PipelineSettings { scanner_machine: ScannerMachine::Oni, scanner: FRONTIER_REST, ..s.clone() }).unwrap();
    assert_eq!(frontier.gains, oni.gains, "the Frontier at zero is ONI at D+1 M+3 Y-3");
    // Twin: the ONI machine and Frontier 2 rest on zero.
    let plain = on_oni_machine(&PipelineSettings { scanner_machine: ScannerMachine::Oni, ..s.clone() }).unwrap();
    assert_ne!(plain.gains, frontier.gains);
    assert_eq!(resting_clicks(&PipelineSettings { scanner_print: true, ..s.clone() }), s.scanner);
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
    let at_rest = on_oni_machine(&PipelineSettings { scanner_machine: ScannerMachine::Oni, scanner: resting_clicks(&s), ..s.clone() }).unwrap();
    assert!(frontier == PipelineSettings { scanner_machine: ScannerMachine::Frontier, scanner: s.scanner, ..at_rest }, "the ONI machine's conversion whole");
    // Frontier 2 takes it whole too, Pro's print included; twin: Pro leaves the settings alone.
    assert!(!on_oni_machine(&PipelineSettings { scanner_print: true, ..s.clone() }).unwrap().print.bypassed);
    assert!(on_oni_machine(&PipelineSettings { face: Face::Pro, ..s.clone() }).is_none());
}
