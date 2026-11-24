use super::*;
use oni_settings::axis::{graduation, pipeline};

/// One channel's window as the stage 4-7 kernel's scalar reference runs it: a density under the
/// black passes on, below zero.
fn window(e: f32, l: &Levels) -> f32 {
    let n = (e - l.black) / (l.white - l.black).max(1e-4);
    let t = if n > 0.0 { n.powf(l.gamma()) } else { n };
    let (c1, c3) = (l.shadow_ordinate(), l.highlight_ordinate());
    if c1 == 0.25 && c3 == 0.75 {
        return t;
    }
    let u = t.clamp(0.0, 1.0);
    let v = 1.0 - u;
    c1 * (4.0 * u * v * v * v) + 0.5 * (6.0 * u * u * v * v) + c3 * (4.0 * u * u * u * v) + u * u * u * u + (t - u)
}

const MODES: [ConversionMode; 3] = [ConversionMode::Negative, ConversionMode::Positive, ConversionMode::Monochrome];

/// `LevelsView.selfCheck`, every line with its twin.
#[test]
fn the_levels_track_holds_and_its_black_reaches_under_zero() {
    let mut strayed: f32 = 0.0;
    let mut round_trip: f32 = 0.0;
    for mode in MODES {
        let (low, _) = LevelsChannel::Red.domain(mode);
        for step in 0..=400 {
            let d = low - 0.2 + step as f32 / 400.0 * (graduation::span() + 0.4);
            let placed = LevelsChannel::Red.position_of_value(d, mode);
            strayed = strayed.max((placed - graduation::fraction(d, mode)).abs());
            round_trip = round_trip.max((LevelsChannel::Red.value_at_position(placed, mode) - d).abs());
        }
    }
    assert!(strayed < 1e-6 && round_trip / graduation::span() < 1e-6, "{strayed} {round_trip}");
    assert!((0.5 - graduation::fraction(0.5, ConversionMode::Negative)).abs() > 0.1, "twin: a raw 0.5 is not its place");

    let transmitting = -0.243;
    let bottom = LevelsChannel::Red.value_at_position(0.0, ConversionMode::Negative);
    let rescued = pipeline::clip(window(transmitting, &Levels::with_ends(bottom, pipeline::DMAX)));
    assert!(bottom < transmitting && rescued > 0.0, "{bottom} {rescued}");
    let sunk = pipeline::clip(window(transmitting, &Levels::with_ends(0.0, pipeline::DMAX)));
    assert_eq!(sunk, 0.0, "twin: a track floored at zero renders that channel black");

    let hand = Levels { black: 0.4137, white: 2.7311, mid: 0.3117, ..Levels::default() };
    let mut inside = true;
    for mode in MODES {
        for channel in CHANNELS {
            let rest = LevelsSet::resting(channel, mode, oni_settings::axis::density_axis::base());
            inside &= (0.0..=1.0).contains(&channel.position_of_value(rest.white, mode));
            inside &= (0.0..=1.0).contains(&channel.position_of_value(rest.black, mode));
            for g in Grip::ALL {
                inside &= (0.0..=1.0).contains(&place(g, &hand, channel, mode));
            }
        }
    }
    assert!(inside, "every handle and every rest falls on the track");
    let rest = LevelsSet::resting(LevelsChannel::Red, ConversionMode::Negative, oni_settings::axis::density_axis::base());
    assert!(!(0.0..=1.0).contains(&rest.white), "twin: the raw resting white sits past any track");
}

#[test]
fn the_five_handles_rest_a_quarter_apart_and_stay_reachable() {
    let base = oni_settings::axis::density_axis::base();
    let at_rest = LevelsSet::resting(LevelsChannel::Red, ConversionMode::Negative, base);
    let mut laid: Vec<f32> = Grip::ALL.iter().map(|g| place(*g, &at_rest, LevelsChannel::Red, ConversionMode::Negative)).collect();
    laid.sort_by(f32::total_cmp);
    let span = LevelsChannel::Red.position_of_value(at_rest.white, ConversionMode::Negative)
        - LevelsChannel::Red.position_of_value(at_rest.black, ConversionMode::Negative);
    assert!(laid.windows(2).all(|w| (w[1] - w[0] - span / 4.0).abs() < 1e-5), "{laid:?}");
    // On a panel-wide track the five stand further apart than a grab area.
    let width = 340.0 - 2.0 * ds::SPACE_MD;
    let xs: Vec<f64> = laid.iter().map(|p| f64::from(*p) * width).collect();
    assert!(xs.windows(2).all(|w| w[1] - w[0] > ds::GRAB_SIZE), "{xs:?}");
    // Twin: pinched to the narrowest window a hand can set, they crowd under one grab area.
    let mut pinched = at_rest;
    pinched.set_white(pinched.black + Levels::EPSILON);
    let mut crowded: Vec<f64> =
        Grip::ALL.iter().map(|g| knob_x(*g, &pinched, LevelsChannel::Red, ConversionMode::Negative, width)).collect();
    crowded.sort_by(f64::total_cmp);
    assert!(crowded.windows(2).map(|w| w[1] - w[0]).fold(f64::INFINITY, f64::min) < ds::GRAB_SIZE);
}

#[test]
fn a_handle_moves_through_one_path_and_resets_alone() {
    let mode = ConversionMode::Negative;
    let red = LevelsChannel::Red;
    let base = oni_settings::axis::density_axis::base();
    let mut l = Levels { black: 0.4137, white: 2.7311, ..Levels::default() };
    // Dragged off both ends, the quarter points stop at their travels.
    l = moved(Grip::Shadows, &l, -9.0, red, mode);
    l = moved(Grip::Highlights, &l, 9.0, red, mode);
    assert_eq!((l.shadows, l.highlights), (Levels::SHADOW_RANGE.0, Levels::HIGHLIGHT_RANGE.1));
    // The black never crosses the white's epsilon.
    let crossed = moved(Grip::Black, &l, 1.0, red, mode);
    assert_eq!(crossed.black, l.white - Levels::EPSILON);
    // The median is drawn on the value that renders mid-grey, and written back from there.
    l.mid = 0.3;
    let p = place(Grip::Mid, &l, red, mode);
    assert!((red.value_at_position(p, mode) - l.display_mid()).abs() < 1e-5);
    let back = moved(Grip::Mid, &l, p, red, mode);
    assert!((back.mid - l.mid).abs() < 1e-6, "{} {}", back.mid, l.mid);
    // A reset moves its own handle alone.
    let r = reset(Grip::White, &l, red, mode, base);
    assert_eq!((r.white, r.black, r.shadows), (pipeline::DMAX, l.black, l.shadows));
    let r = reset(Grip::Shadows, &l, red, mode, base);
    assert_eq!((r.shadows, r.highlights, r.black), (0.25, l.highlights, l.black));
    // Twin: the median's former place, its travel stretched across the window, drifts once written back.
    let stretched = l.black + (l.mid - Levels::MID_RANGE.0) / (Levels::MID_RANGE.1 - Levels::MID_RANGE.0) * (l.white - l.black);
    let drifted = moved(Grip::Mid, &l, red.position_of_value(stretched, mode), red, mode);
    assert!((drifted.mid - l.mid).abs() > 0.01, "{}", drifted.mid);
}

/// Each inner point sits between its neighbours on the gradation: the ends carry all three, the midpoint
/// its two quarters, and none ever crosses another.
#[test]
fn the_inner_points_follow_their_neighbours_and_never_cross() {
    let cases = [(LevelsChannel::Red, ConversionMode::Negative, 0.4137, 2.7311), (LevelsChannel::Linked, ConversionMode::Digital, 0.08, 0.93)];
    let steps = |(low, high): (f32, f32), n: usize| (0..=n).map(move |i| low + (high - low) * i as f32 / n as f32);
    let mut worst: f32 = 0.0;
    for (channel, mode, black, white) in cases {
        for mid in steps(Levels::MID_RANGE, 14) {
            for shadows in steps(Levels::SHADOW_RANGE, 8) {
                for highlights in steps(Levels::HIGHLIGHT_RANGE, 8) {
                    let l = Levels { black, white, mid, shadows, highlights };
                    let drawn: Vec<f32> = Grip::READING.iter().map(|g| place(*g, &l, channel, mode)).collect();
                    assert!(drawn.windows(2).all(|w| w[0] < w[1]), "{l:?} {drawn:?}");
                    for g in Grip::ALL {
                        let back = moved(g, &l, place(g, &l, channel, mode), channel, mode);
                        worst = [back.black - black, back.white - white, back.mid - mid, back.shadows - shadows, back.highlights - highlights]
                            .into_iter()
                            .fold(worst, |m, d| m.max(d.abs()));
                    }
                    // A higher midpoint carries both quarters up, their stored shares untouched.
                    let raised = Levels { mid: (mid + 0.05).min(Levels::MID_RANGE.1), ..l };
                    if raised.mid > mid {
                        assert!(place(Grip::Shadows, &raised, channel, mode) > drawn[1] && place(Grip::Highlights, &raised, channel, mode) > drawn[3]);
                    }
                }
            }
        }
    }
    assert!(worst < 1e-5, "a handle written back from its place moves by {worst}");
}

/// At rest, off the resting midpoint, each inner handle sits on the input its window renders as its quarter.
#[test]
fn the_inner_handles_sit_where_their_quarter_renders() {
    for mid in [0.15, 0.3, 0.5, 0.62, 0.85] {
        let l = Levels { black: 0.1, white: 0.9, mid, ..Levels::default() };
        let rendered = |v: f32| f64::from(window(v, &l));
        for (v, quarter) in [(l.display_shadows(), 0.25), (l.display_mid(), 0.5), (l.display_highlights(), 0.75)] {
            assert!((rendered(v) - quarter).abs() < 1e-6, "mid {mid}: {v} renders {}", rendered(v));
        }
        // Twin: the former linear place of the shadows renders off its quarter once the midpoint moves.
        if (mid - 0.5).abs() > 0.1 {
            assert!((rendered(l.black + l.shadows * (l.white - l.black)) - 0.25).abs() > 0.02, "mid {mid}");
        }
    }
    // At the resting midpoint the quarter points are drawn where a straight share puts them, bit for bit.
    let rest = Levels { black: 0.1, white: 0.9, shadows: 0.3, highlights: 0.7, ..Levels::default() };
    let span = rest.white - rest.black;
    assert_eq!((rest.display_shadows(), rest.display_highlights()), (rest.black + rest.shadows * span, rest.black + rest.highlights * span));
}

#[test]
fn a_press_catches_the_front_handle_and_a_scroll_the_nearest() {
    let mode = ConversionMode::Negative;
    let red = LevelsChannel::Red;
    let width = 284.0;
    // The ends together: the white is drawn last, so it is caught first.
    let mut l = Levels::with_ends(1.0, 1.0 + Levels::EPSILON);
    l.mid = 0.5;
    let x = knob_x(Grip::White, &l, red, mode, width);
    assert_eq!(caught(x, 17.0, width, &l, red, mode), Some(Grip::White));
    assert_eq!(scrolled(x, width, &l, red, mode), Grip::White);
    // Off the handle's height nothing is caught.
    assert_eq!(caught(x, 40.0, width, &l, red, mode), None);
    // Twin: far from every handle a press catches none, a scroll still the nearest.
    let far = width - 1.0;
    assert_eq!(caught(far, 17.0, width, &l, red, mode), None);
    assert_eq!(scrolled(far, width, &l, red, mode), Grip::White);
    assert_eq!(scrolled(0.0, width, &l, red, mode), Grip::Black);
}

#[test]
fn the_readouts_are_densities_to_three_decimals() {
    let l = Levels::with_ends(-0.243, 2.5);
    let r = readouts(&l);
    assert_eq!(r[0], ("Black", "\u{2212}0.243".to_string()));
    assert_eq!(r.iter().map(|(n, _)| *n).collect::<Vec<_>>(), vec!["Black", "Shadows", "Mid", "Highs", "White"]);
    // Each readout reads where its handle is drawn, the midpoint off its rest included.
    let bent = Levels { mid: 0.31, shadows: 0.2, highlights: 0.8, ..l };
    let (red, negative) = (LevelsChannel::Red, ConversionMode::Negative);
    let drawn: Vec<String> = Grip::READING.iter().map(|g| readout(red.value_at_position(place(*g, &bent, red, negative), negative))).collect();
    assert_eq!(readouts(&bent).into_iter().map(|(_, v)| v).collect::<Vec<_>>(), drawn);
    // Twin: the shares of the output a picture's window would read are no densities.
    assert_ne!(readout(0.5), "50.0%");
}

/// A digital photo's editor moves its negative's three density windows, which rest on its film's reference:
/// at rest no tab is lit, a handle moved lights its own, the block's reset lands back on the film's windows.
#[test]
fn a_digital_photos_windows_rest_on_its_films_reference() {
    use super::super::{Gestures, Reset};
    let (mut store, dir) = crate::store::tests::fresh("editors-digital-levels");
    store.apply(Command::Write(Box::new(store.settings().carried(ConversionMode::Digital))));
    let digital = store.settings().clone();
    let reference = oni_settings::VirtualStock::of("kodak-portra-800").reference().levels();
    assert_eq!(digital.levels, reference);
    assert_eq!(marked(&digital), vec![false; 3]);
    let mut moved = Windows::of(&digital);
    moved.set(LevelsChannel::Red, moved_white(&digital.levels.red));
    assert_eq!(steps(&digital, &moved), vec![Command::Step { knob: Knob::Level(Channel::Red, Handle::White), by: -0.25 }]);
    let mut at = digital.clone();
    moved.lay(&mut at);
    assert_eq!(marked(&at), vec![true, false, false]);
    store.apply(Command::Write(Box::new(at)));
    Gestures::default().reset(&mut store, Reset::Levels);
    assert_eq!(store.settings().levels, reference);
    // Twin: a negative on no roll rests on the bare axis, so the film's windows are lit there.
    let negative = PipelineSettings { mode: ConversionMode::Negative, ..digital };
    assert_eq!(marked(&negative), vec![true; 3]);
    let _ = std::fs::remove_dir_all(&dir);
}

fn moved_white(l: &Levels) -> Levels {
    Levels { white: l.white - 0.25, ..*l }
}
