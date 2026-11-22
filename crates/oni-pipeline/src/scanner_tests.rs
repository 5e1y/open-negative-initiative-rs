use super::*;

/// `ScannerKey.selfCheck`'s render half: the keypad's letters read through the measured table.
#[test]
fn a_density_key_lightens_and_a_colour_key_takes_its_own_channel_off() {
    let roll = check_roll();
    let mid = CHECK_GREY.map(|d| 10f32.powf(-d));
    let keyed = |d: i64, c: i64, w: i64| LegacyPrint { machine: LegacyMachine::Frontier, d, c, w, ..LegacyPrint::default() };
    let rendered = |l: LegacyPrint| replica(mid, &PipelineSettings { legacy_print: Some(l), ..roll.clone() }).unwrap();
    let rest = rendered(keyed(0, 0, 0));
    let (lighter, darker) = (rendered(keyed(2, 0, 0)), rendered(keyed(-2, 0, 0)));
    let (cooler, redder) = (rendered(keyed(0, 2, 0)), rendered(keyed(0, -2, 0)));
    assert!(lighter[1] > rest[1] && darker[1] < rest[1], "D + lightens: green {} against {}", lighter[1], rest[1]);
    assert!(cooler[0] < rest[0] && redder[0] > rest[0], "C + takes red off: {} against {}", cooler[0], rest[0]);
    // Twin: off the Frontier no table is read, so a check through it cannot pass by default; nor with it not laid.
    assert!(replica(mid, &PipelineSettings { legacy_print: Some(LegacyPrint { machine: LegacyMachine::Oni, ..keyed(0, 0, 0) }), ..roll.clone() }).is_none());
    assert!(replica(mid, &PipelineSettings { legacy_print: Some(LegacyPrint { laid: false, ..keyed(0, 0, 0) }), ..roll.clone() }).is_none());
    // And TEMP + warms the grey: more red than blue than at rest.
    let warm = rendered(keyed(0, 0, 4));
    assert!(warm[0] - warm[2] > rest[0] - rest[2], "{warm:?} against {rest:?}");
}

#[test]
fn the_look_lab_s_coordinates_unmix_as_the_swift_machine_did() {
    let roll = check_roll();
    let m = separation(&roll);
    assert!(m.iter().all(|r| (r[0] + r[1] + r[2] - 1.0).abs() < 1e-6), "a grey is a fixed point: {m:?}");
    assert!((m[0][1] - 0.6 * (-0.2706 / (1.0 - 0.2706 - 0.0076))).abs() < 1e-6, "{m:?}");
    // Twin: on RGB passes the coordinates unmix nothing.
    let passes = oni_settings::frame::RgbPasses { red: "r".into(), green: "g".into(), blue: "b".into(), registration: None };
    assert_eq!(separation(&PipelineSettings { rgb_passes: Some(passes), ..roll.clone() }), IDENTITY);
}

#[test]
fn the_table_reads_and_an_odd_count_sits_between_its_neighbours() {
    let m = &*MACHINE;
    assert_eq!((m.render.cube_dimension, m.warmth_plus.cube_dimension), (27, 17));
    assert_eq!(m.offset([0; 4]), [0.0; 3]);
    let one = m.offset([1, 0, 0, 0]);
    let two = m.offset([2, 0, 0, 0]);
    assert!((one[0] - two[0] / 2.0).abs() < 1e-6 && two[0] > 0.0, "{one:?} {two:?}");
    let minus = m.offset([-3, 0, 0, 0]);
    assert!(minus[0] < 0.0);
    // The lift is on every node of a look, never on the render.
    assert!(m.warmth_plus.cube_data.iter().all(|n| n[0] > -LIFT));
}

/// A whole unit of the screen moves the render as one count did before the half step: the mean of
/// the measured clicks around half of it, so a frame edited on whole units renders unchanged.
#[test]
fn a_whole_unit_renders_as_a_count_did_and_a_half_step_lies_between() {
    let m = &*MACHINE;
    let table = |n: i64| -> [f32; 3] {
        let v = &m.table.clicks["Y"][&n.to_string()];
        [v[0], v[1], v[2]]
    };
    let former = |count: i64| -> [f32; 3] {
        let (a, b) = (table(count.div_euclid(2)), table(count.div_euclid(2) + count.rem_euclid(2)));
        std::array::from_fn(|c| (a[c] + b[c]) / 2.0)
    };
    let at = |steps: i64| m.offset([0, 0, 0, steps]);
    for count in -24..=24 {
        let (got, want) = (at(count * LegacyPrint::PER_POINT), former(count));
        assert!((0..3).all(|c| (got[c] - want[c]).abs() < 1e-6), "{count}: {got:?} against {want:?}");
    }
    for steps in [-47, -3, 1, 5, 47] {
        let (got, low, high) = (at(steps), at(steps - 1), at(steps + 1));
        assert!((0..3).all(|c| (got[c] - (low[c] + high[c]) / 2.0).abs() < 1e-6), "{steps}");
        // Twin: a half step is a move of its own, on neither neighbour.
        assert!(got != low && got != high, "{steps}");
    }
    // And the warmth's share is per unit: two steps weigh what one count weighed.
    let share = |steps: i64| m.warmth(steps).unwrap().1;
    assert!((share(2) - 1.0 / m.table.looks["warmthPlus"].per_count).abs() < 1e-6);
    assert!((share(1) - share(2) / 2.0).abs() < 1e-6 && share(-3) > share(-2));
}

#[test]
fn a_legacy_print_moves_the_gains_by_its_keys_and_the_frontier_s_rest_off_a_paper() {
    let mut s = PipelineSettings::rest(oni_settings::ConversionMode::Negative, true);
    s.legacy_print = Some(LegacyPrint { d: 2, ..LegacyPrint::default() });
    let oni = on_legacy_print(&s).unwrap();
    assert!(oni.gains.stops[0] < 0.0, "more density is less light on a negative");
    assert_eq!(PipelineSettings { gains: s.gains, ..oni.clone() }, s, "the gains alone move");
    // The Frontier's look takes its rest in the gains with the keys; on a paper, the keys alone.
    s.legacy_print = Some(LegacyPrint { machine: LegacyMachine::Frontier, d: 2, ..LegacyPrint::default() });
    let frontier = on_legacy_print(&s).unwrap();
    let rest = stops_of(frontier_rest(), &s);
    assert!((0..3).all(|c| (frontier.gains.stops[c] - oni.gains.stops[c] - rest[c]).abs() < 1e-6), "{:?} {:?}", frontier.gains, oni.gains);
    assert!(rest.iter().any(|r| r.abs() > 0.01), "the rest moves the gains");
    let papered = PipelineSettings { print_paper: Some(oni_settings::PaperStock::default()), ..s.clone() };
    assert_eq!(on_legacy_print(&papered).unwrap().gains, oni.gains);
    // Twin: no legacy print, or one not laid, leaves the settings to themselves.
    assert!(on_legacy_print(&PipelineSettings { legacy_print: None, ..s.clone() }).is_none());
    assert!(on_legacy_print(&PipelineSettings { legacy_print: Some(LegacyPrint { laid: false, ..s.legacy_print.unwrap() }), ..s }).is_none());
}
