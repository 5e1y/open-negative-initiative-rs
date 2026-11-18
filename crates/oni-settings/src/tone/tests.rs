use super::*;

#[test]
fn a_stored_contrast_past_the_widest_track_lands_on_its_stop() {
    let reopened = |c: f32| {
        let stored = Print { contrast: c, ..Print::default() };
        serde_json::from_value::<Print>(serde_json::to_value(stored).unwrap()).unwrap().contrast
    };
    assert_eq!(reopened(1.2), Print::MONOCHROME_CONTRAST_RANGE.1);
    assert_eq!(reopened(-0.9), Print::MONOCHROME_CONTRAST_RANGE.0);
    // Twin: values inside the track survive to the bit.
    assert_eq!(reopened(0.8), 0.8);
    assert_eq!(reopened(-0.3), -0.3);
}

#[test]
fn a_stored_channel_reads_its_exposure_alone_held_to_a_stop() {
    let stored = r#"{"contrast":0.2,"red":{"exposure":2,"contrast":0.3,"mids":0.1},"blue":{"exposure":-0.4,"shadows":0.1}}"#;
    let decoded: Print = serde_json::from_str(stored).unwrap();
    assert_eq!(decoded.red, Tone::exposure(Print::FILTER_RANGE.1));
    assert_eq!(decoded.blue, Tone::exposure(-0.4));
    assert_eq!(decoded.green, Tone::NEUTRAL);
    assert_eq!(decoded.contrast, 0.2);
    assert_eq!(decoded.black_point, Print::RESTING_BLACK_POINT);
    // Twin: a Tone read on its own keeps every slider, so the narrowing above is Print's.
    let tone: Tone = serde_json::from_str(r#"{"exposure":2,"contrast":0.3,"mids":0.1}"#).unwrap();
    assert_eq!(tone.contrast, 0.3);
    assert_eq!(tone.mids, 0.1);
}

#[test]
fn channel_points_at_zero_write_no_key_and_read_back_held_to_their_tracks() {
    let rest = serde_json::to_value(Print::default()).unwrap();
    assert!(rest.get("channelWhite").is_none() && rest.get("channelBlack").is_none());
    // Twin: one channel off zero writes its array, read back to the bit.
    let red = Print { channel_white: [0.01, 0.0, 0.0], ..Print::default() };
    let written = serde_json::to_value(red).unwrap();
    assert_eq!(written["channelWhite"], serde_json::json!([0.01f32, 0.0, 0.0]));
    assert!(written.get("channelBlack").is_none());
    assert_eq!(serde_json::from_value::<Print>(written).unwrap(), red);
    // A stored point past its track lands on its stop; an absent key reads zero.
    let stored: Print = serde_json::from_str(r#"{"channelWhite":[0.9,-0.1,-2],"channelBlack":[0.5,0,-0.3]}"#).unwrap();
    assert_eq!(stored.channel_white, [Print::POINT_RANGE.1, -0.1, Print::POINT_RANGE.0]);
    assert_eq!(stored.channel_black, [Print::BLACK_POINT_RANGE.1, 0.0, Print::BLACK_POINT_RANGE.0]);
    assert_eq!(serde_json::from_str::<Print>("{}").unwrap().channel_white, [0.0; 3]);
}

#[test]
fn each_channel_holds_its_own_sum_and_black_and_white_reads_the_global_pair() {
    let print = Print { black_point: 0.2, white_point: 1.0 / 3.0, channel_black: [0.2, 0.0, 0.0], channel_white: [1.0 / 3.0, 0.0, 0.0], ..Print::default() };
    let points = print.channel_points(false);
    for (black, white) in points {
        assert!(1.0 - black - white >= Print::MINIMUM_SPAN - 1e-6, "{points:?}");
    }
    assert_eq!(points[1], (0.2, 1.0 / 3.0));
    // Twin: red's sum taken unheld leaves its window less than the minimum span.
    assert!(1.0 - (0.2 + 0.2) - 2.0 / 3.0 < Print::MINIMUM_SPAN);
    assert_ne!(points[0], (0.4, 2.0 / 3.0));
    // Black and white shows no channel: every window takes the global pair, and red's alone at rest.
    assert_eq!(print.channel_points(true), [print.held_points(); 3]);
    let red_only = Print { channel_white: [0.1, 0.0, 0.0], ..Print::default() };
    assert!(red_only.points_at_rest(true) && !red_only.points_at_rest(false));
}

#[test]
fn at_rest_the_print_is_the_diagonal() {
    let rest = Print::default();
    for v in [0.0f32, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0] {
        let out = rest.output(v, LevelsChannel::Linked, true).powf(1.0 / Print::DISPLAY_GAMMA);
        assert!((out - v).abs() < 1e-5, "{v} printed {out}");
    }
    // Twin: a stop of exposure leaves the diagonal.
    let lit = Print { exposure: 1.0, ..Print::default() };
    assert!((lit.output(0.5, LevelsChannel::Linked, true).powf(1.0 / 2.2) - 0.5).abs() > 0.05);
}

#[test]
fn contrast_is_monotone_pinned_and_inverted_by_its_opposite() {
    for travel in [0.3f32, -0.3] {
        assert!(contrast::apply(0.0, travel, 0.5).abs() < 1e-6);
        assert!((contrast::apply(1.0, travel, 0.5) - 1.0).abs() < 1e-6);
    }
    let x = 0.37;
    let there = contrast::apply(x, 0.4, 0.5);
    let back = contrast::apply(there, -0.4, 0.5);
    assert!((back - x).abs() < 1e-5, "{back}");
    // Twin: a travel of the same sign does not undo it.
    assert!((contrast::apply(there, 0.4, 0.5) - x).abs() > 0.01);
}
