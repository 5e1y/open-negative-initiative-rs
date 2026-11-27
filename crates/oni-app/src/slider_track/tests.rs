use super::*;
use oni_settings::{ConversionMode, PipelineSettings};

#[test]
fn an_arrow_moves_the_readout_by_one_and_lands_on_its_numbers() {
    let track = track_range(Field::Temperature);
    let top = *track.end();
    // +0.186 reads +0.19 EV: the next press reads +0.20 EV, never +0.21 EV.
    let at = stepped(Field::Temperature, 0.186, 1, &track);
    assert_eq!(Field::Temperature.readout(at), "Warm +0.20 EV");
    assert_eq!(Field::Temperature.readout(stepped(Field::Temperature, at, -10, &track)), "Warm +0.10 EV");
    // Twin: a step of 1 % of the range, as accessibility's was, skips three numbers in four.
    let span = top - *track.start();
    assert_eq!(Field::Temperature.readout(0.20 + span / 100.0), "Warm +0.24 EV");
    // Held at the track's end, and the black point keeps its shown sense: right lifts the black.
    assert_eq!(stepped(Field::Temperature, top, 5, &track), top);
    let black = track_range(Field::BlackPoint);
    let lifted = stepped(Field::BlackPoint, 0.0, 1, &black);
    assert!(lifted < 0.0 && Field::BlackPoint.readout(lifted) == "+1%", "{}", Field::BlackPoint.readout(lifted));
    assert_eq!(Field::Exposure.readout(stepped(Field::Exposure, 0.5, 1, &track_range(Field::Exposure))), "+0.51 EV");
}

#[test]
fn a_field_typed_over_a_readout_starts_from_its_figure_alone() {
    assert_eq!(number("Warm +0.46 EV"), "+0.46");
    assert_eq!(number("\u{2212}0.06 EV"), "\u{2212}0.06");
    assert_eq!(number("85%"), "85");
    assert_eq!(number("2.1 px"), "2.1");
    assert_eq!(number("1.00\u{d7}"), "1.00");
    // The figure read back is the readout's own: typed unchanged, it moves nothing.
    assert_eq!(typed_steps("Warm +0.46 EV", number("Warm +0.46 EV")), 0);
    // Twin: the readout whole would carry its side and unit into the field.
    assert_ne!(number("Warm +0.46 EV"), "Warm +0.46 EV");
}

#[test]
fn a_number_typed_over_a_readout_is_the_steps_an_arrow_would_take() {
    // The readout's last digit is a step: +1.50 EV to 2 is fifty hundredths, to −0.3 one hundred and eighty.
    assert_eq!(typed_steps("+1.50 EV", "2"), 50);
    assert_eq!(typed_steps("+1.50 EV", "\u{2212}0.3"), -180);
    assert_eq!(typed_steps("+23", "-4"), -27);
    assert_eq!(typed_steps("0%", "15 %"), 15);
    assert_eq!(typed_steps("12.5 px", "12,8"), 3);
    assert_eq!(typed_steps("3 / 24", "7"), 4);
    // The same number, or none, moves nothing.
    assert_eq!(typed_steps("+1.50 EV", "1.5"), 0);
    assert_eq!(typed_steps("+1.50 EV", "warm"), 0);
    // The side a colour readout names is left aside, its sign read: Green −0.06 EV to 0 is six.
    assert_eq!((typed_steps("Warm +0.46 EV", "0.5"), typed_steps("Green \u{2212}0.06 EV", "0")), (4, 6));
    // Twin: the side named without its sign would step the wrong way.
    assert_eq!(typed_steps("Green 0.06 EV", "0"), -6);
    assert_eq!(typed_steps("", "2"), 0);
    // Twin: the readout's tenths count tenths, ten times fewer than its hundredths.
    assert_eq!(typed_steps("+1.5 EV", "2"), 5);
    // Typed then stepped, the value lands on the number typed.
    let track = track_range(Field::Exposure);
    let at = stepped(Field::Exposure, 1.5, typed_steps(&Field::Exposure.readout(1.5), "-0.37"), &track);
    assert_eq!(Field::Exposure.readout(at), "\u{2212}0.37 EV");
}

#[test]
fn a_keys_press_is_three_shown_units_and_its_tooltip_names_them() {
    // The value editor's − and + step one unit of the last digit; a key's press is three of them.
    let track = track_range(Field::Exposure);
    assert_eq!(Field::Exposure.readout(stepped(Field::Exposure, 0.5, 3, &track)), "+0.53 EV");
    assert_eq!(Field::Exposure.readout(stepped(Field::Exposure, 0.5, -3, &track)), "+0.47 EV");
    // Twin: the value editor's own step moves one unit, a third of the press.
    assert_eq!(Field::Exposure.readout(stepped(Field::Exposure, 0.5, 1, &track)), "+0.51 EV");
    // Every readout's press in its own unit, a span's and a side's included.
    let cases = [
        ("+1.50 EV", "0.03 EV"),
        ("Warm +0.46 EV", "0.03 EV"),
        ("\u{2212}0.20 \u{2026} +0.30 EV", "0.03 EV"),
        ("85%", "3%"),
        ("+12°", "3°"),
        ("1.00\u{d7}", "0.03\u{d7}"),
        ("12.5 px", "0.3 px"),
        ("0.950", "0.003"),
        ("3 / 24", "3"),
        ("Off", ""),
    ];
    for (readout, words) in cases {
        assert_eq!(pressed(readout, 3), words, "{readout}");
    }
    // Twin: one step, the value editor's, names a third of the press.
    assert_eq!(pressed("+1.50 EV", 1), "0.01 EV");
    assert_ne!(pressed("+1.50 EV", 1), pressed("+1.50 EV", 3));
    // The press named is the press taken: three units of the readout's digit from any Store readout.
    for f in Field::ALL {
        let range = track_range(f);
        let mid = f.shown((range.start() + range.end()) / 2.0);
        let (from, to) = (f.readout(mid), f.readout(stepped(f, mid, 3, &range)));
        if let (Some((a, _)), Some((b, _))) = (read(&from), read(&to)) {
            let unit = 10f64.powi(-(read(&from).unwrap().1 as i32));
            let named = read(&pressed(&from, 3)).unwrap().0;
            assert!(((b - a).abs() - named).abs() < unit / 2.0 && (named / unit - 3.0).abs() < 1e-6, "{f:?}: {from} to {to}, {named}");
        }
    }
}

#[test]
fn a_drag_held_on_rest_lands_on_it_exactly() {
    let s = PipelineSettings::rest(ConversionMode::Negative, true);
    let (ceiling, track) = (Field::DensityCeiling, track_on(Field::DensityCeiling, &s));
    let (at, base) = (rests(&s)[ceiling.index() as usize], ceiling.base(&s));
    assert!((stored_on(ceiling, at, &track) - s.density_ceiling).abs() < 1e-5, "the ceiling rests where the frame came in");
    assert_eq!(landed(ceiling, at + 2e-6, &track, base), base);
    // Twin: a place a hair past rest is the hand's own, never rest.
    assert_ne!(landed(ceiling, at + 1e-3, &track, base), base);
    // A slide's ceiling rests elsewhere on its track.
    let slide = PipelineSettings::rest(ConversionMode::Positive, true);
    assert_ne!(rests(&slide)[ceiling.index() as usize], at);
}

#[test]
fn a_sliders_tick_its_double_click_and_its_blocks_reset_are_one_rest() {
    use crate::fields::Block;
    use crate::store::{is_modified, rested};
    let mut measured = PipelineSettings::rest(ConversionMode::Negative, true);
    let id = oni_settings::Id::new();
    let stops = [0.3, 0.0, -0.2];
    measured.roll_reference = Some(oni_settings::frame::RollReference { id, stops, black: [0.0; 3], white: [1.0; 3], base: None, median: None });
    let mut frames: Vec<PipelineSettings> = ConversionMode::ALL.iter().flat_map(|m| [true, false].map(|mosaic| PipelineSettings::rest(*m, mosaic))).collect();
    frames.push(measured);
    for frame in &frames {
        // Every slider moved off, then each block reset: its tick stands where the reset landed.
        let mut moved = frame.clone();
        for f in Field::ALL.iter().filter(|f| f.block().is_some()) {
            moved = f.knob().set(&moved, f.knob().get(&moved) + 0.05);
        }
        let ticks = rests(&moved);
        for block in Block::ALL.into_iter().filter(|b| *b != Block::FlatField) {
            let reset = rested(&moved, block);
            assert!(!is_modified(&reset, block), "{block:?} lit after its reset on {:?}", frame.mode);
            for f in block.fields().iter().filter(|f| f.shown_in(&moved)) {
                let (at, landed) = (ticks[f.index() as usize], position_on(*f, f.knob().get(&reset), &track_on(*f, &moved)));
                assert!((at - landed).abs() < 1e-5, "{f:?} ticked at {at}, reset to {landed} on {:?}", frame.mode);
                assert!((f.base(&moved) - f.knob().get(&reset)).abs() < 1e-5, "{f:?}'s double click beside its reset");
            }
        }
    }
    // A fresh negative opens on its rest: 0.00 EV under the tick at the track's middle, its Lighting's Reset away.
    let negative = &frames[0];
    let exposure = Field::Exposure;
    assert_eq!(exposure.readout(exposure.knob().get(negative)), "0.00 EV");
    assert_eq!(rests(negative)[exposure.index() as usize], position_on(exposure, 0.0, &track_on(exposure, negative)));
    assert!((rests(negative)[exposure.index() as usize] - 0.5).abs() < 1e-6);
    assert!(!is_modified(negative, Block::Lighting));
    // Twin: the former rest's +1.50 EV stands off the rest the reset returns to.
    assert_ne!(rests(negative)[exposure.index() as usize], position_on(exposure, 1.5, &track_on(exposure, negative)));
    assert!(is_modified(&exposure.knob().set(negative, 1.5), Block::Lighting));
}

#[test]
fn a_digital_photos_film_effects_walk_their_levels_one_notch_a_step() {
    use oni_settings::effect_levels::{GRAIN_AMOUNT, HALATION_SIZE};
    let s = PipelineSettings::rest(ConversionMode::Digital, true).picking(oni_settings::VirtualStock::of("kodak-portra-800"));
    let (grain, track) = (Field::GrainAmount, track_on(Field::GrainAmount, &s));
    assert_eq!(track.levels.as_deref(), Some(&GRAIN_AMOUNT[..]));
    // Each level at its own place, evenly along the track, and back.
    for (i, v) in GRAIN_AMOUNT.iter().enumerate() {
        let at = position_on(grain, *v, &track);
        assert!((at - i as f32 / 5.0).abs() < 1e-6 && stored_on(grain, at, &track) == *v, "{v} at {at}");
    }
    // A key's press, the value editor's step and an arrow's are each one notch; held at the ends.
    assert_eq!(stepped_on(grain, 0.1719, 3, &track, None), 0.2545);
    assert_eq!(stepped_on(grain, 0.1719, -1, &track, None), 0.11);
    assert_eq!(stepped_on(grain, 0.5708, 10, &track, None), 0.5708);
    // A drag between two notches lands on the nearer.
    assert_eq!(stored_on(grain, 0.52, &track), 0.2545);
    // A number typed lands on the level nearest it: 30 % is Medium's 25 %, 33 % Strong's 38 %.
    let typed = |text: &str| typed_on(grain, 0.11, typed_steps(&grain.readout(0.11), text), &track, None);
    assert_eq!((typed("30"), typed("33")), (0.2545, 0.3783));
    // The readout names its notch, the rest's tick on one of them.
    assert_eq!(readout_named(grain, &s, 0.2545, grain.readout(0.2545)), "25% · Medium");
    let halation = track_on(Field::HalationSize, &s);
    assert_eq!(halation.levels.as_deref(), Some(&HALATION_SIZE[..]));
    assert_eq!(rests(&s)[Field::HalationSize.index() as usize], 0.5);
    assert_eq!(notches(&s)[Field::GrainAmount.index() as usize], 6);
    // Twin: a scanned negative's sliders keep their even tracks and their digit's steps.
    let scan = PipelineSettings::rest(ConversionMode::Negative, true);
    assert_eq!(track_on(Field::Exposure, &scan).levels, None);
    assert_eq!(notches(&scan).iter().sum::<i32>(), 0);
    assert_eq!(stepped_on(Field::Exposure, 0.5, 3, &track_on(Field::Exposure, &scan), None), 0.53);
    // Twin: the same step on an even track moves a digit, never a whole notch.
    assert_ne!(stepped_on(grain, 0.1719, 3, &track_range(grain), None), 0.2545);
}

#[test]
fn a_monochrome_frame_spans_its_own_contrast_and_a_value_round_trips() {
    let mono = PipelineSettings::rest(ConversionMode::Monochrome, true);
    let colour = PipelineSettings::rest(ConversionMode::Negative, true);
    // Twin: the colour track is another, wider one.
    assert_ne!(track_on(Field::Contrast, &mono), track_on(Field::Contrast, &colour));
    assert_eq!(track_on(Field::Contrast, &colour), track_range(Field::Contrast));
    let range = track_on(Field::Contrast, &mono);
    let at = position_on(Field::Contrast, 0.4, &range);
    assert!((stored_on(Field::Contrast, at, &range) - 0.4).abs() < 1e-5);
    // The black point keeps its shown sense on its own track.
    let black = track_on(Field::BlackPoint, &colour);
    assert!((stored_on(Field::BlackPoint, position_on(Field::BlackPoint, -0.1, &black), &black) + 0.1).abs() < 1e-5);
}
