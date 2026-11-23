use super::*;
use crate::fields::FieldSet;

#[test]
fn a_knob_writes_its_own_field_and_holds_its_range() {
    let s = PipelineSettings::default();
    for knob in [Knob::Exposure, Knob::Gain(Channel::Blue), Knob::SharpenAmount, Knob::Angle, Knob::Chrome] {
        let moved = knob.set(&s, knob.get(&s) + 0.25);
        let fields: Vec<Field> = FieldSet::between(&s, &moved).iter().collect();
        assert_eq!(fields, vec![knob.field()], "{knob:?}");
    }
    assert_eq!(Knob::Exposure.get(&Knob::Exposure.set(&s, 99.0)), Print::EXPOSURE_RANGE.1);
    // Twin: a value inside the range is written as given.
    assert_eq!(Knob::Exposure.get(&Knob::Exposure.set(&s, 0.75)), 0.75);
}

#[test]
fn a_channels_point_writes_its_own_channel_on_the_global_tracks() {
    let s = PipelineSettings::rest(oni_settings::ConversionMode::Negative, true);
    let red = Knob::ChannelWhite(Channel::Red).set(&s, 0.2);
    assert_eq!(red.print.channel_white, [0.2, 0.0, 0.0]);
    assert_eq!(FieldSet::between(&s, &red).iter().collect::<Vec<_>>(), vec![Field::Print]);
    let blue = Knob::ChannelBlack(Channel::Blue).set(&s, -9.0);
    assert_eq!(blue.print.channel_black, [0.0, 0.0, Print::BLACK_POINT_RANGE.0]);
    assert_eq!(Knob::ChannelWhite(Channel::Green).set(&s, 9.0).print.channel_white[1], Print::POINT_RANGE.1);
    // Twin: the global white point is not the channel's, and stays where it was.
    assert_eq!(red.print.white_point, s.print.white_point);
    assert_ne!(Knob::ChannelWhite(Channel::Green).get(&red), 0.2);
}

#[test]
fn a_channels_mid_and_contrast_write_their_own_channel_and_hold_their_tracks() {
    let s = PipelineSettings::rest(oni_settings::ConversionMode::Digital, true);
    let green = Knob::ChannelMid(Channel::Green).set(&s, 0.1);
    assert_eq!((green.print.channel_mid, green.print.channel_contrast), ([0.0, 0.1, 0.0], [0.0; 3]));
    assert_eq!(FieldSet::between(&s, &green).iter().collect::<Vec<_>>(), vec![Field::Print]);
    assert_eq!(Knob::ChannelMid(Channel::Red).set(&s, 9.0).print.channel_mid[0], Print::ZONE_RANGE.1);
    assert_eq!(Knob::ChannelContrast(Channel::Blue).set(&s, -9.0).print.channel_contrast[2], Print::CONTRAST_RANGE.0);
    let keen = Knob::HalationSensitivity.set(&s, 9.0);
    assert_eq!(keen.halation.sensitivity, oni_settings::blocks::Halation::SENSITIVITY_RANGE.1);
    assert_eq!(FieldSet::between(&s, &keen).iter().collect::<Vec<_>>(), vec![Field::Halation]);
    // Twin: Lighting's own contrast is not the channel's, and stays where it was.
    let red = Knob::ChannelContrast(Channel::Red).set(&s, 0.3);
    assert_eq!((red.print.contrast, Knob::Contrast.get(&red)), (s.print.contrast, s.print.contrast));
    assert_ne!(Knob::ChannelContrast(Channel::Green).get(&red), 0.3);
}

#[test]
fn temperature_and_tint_move_on_axes_that_leave_each_other_alone() {
    let s = PipelineSettings::default();
    let warm = Knob::Temperature.set(&s, 0.8);
    assert!((Knob::Temperature.get(&warm) - 0.8).abs() < 1e-6);
    assert!(Knob::Tint.get(&warm).abs() < 1e-6);
    let both = Knob::Tint.set(&warm, -0.3);
    assert!((Knob::Temperature.get(&both) - 0.8).abs() < 1e-6 && (Knob::Tint.get(&both) + 0.3).abs() < 1e-6);
    // Twin: writing red alone moves both readings, which the axes exist to avoid.
    let mut raw = s.clone();
    raw.print.red.exposure = 0.8;
    assert!(Knob::Tint.get(&raw).abs() > 0.1);
}

#[test]
fn the_gains_temperature_and_tint_move_apart_and_hold_the_gains_mean() {
    let mut s = PipelineSettings::default();
    s.gains.stops = [0.3, -0.1, -0.5];
    let mean = |s: &PipelineSettings| s.gains.stops.iter().sum::<f32>() / 3.0;
    let warm = Knob::GainTemperature.set(&s, 0.6);
    assert!((Knob::GainTemperature.get(&warm) - 0.6).abs() < 1e-6);
    assert!((Knob::GainTint.get(&warm) - Knob::GainTint.get(&s)).abs() < 1e-6 && (mean(&warm) - mean(&s)).abs() < 1e-6);
    let green = Knob::GainTint.set(&warm, 0.4);
    assert!((Knob::GainTint.get(&green) - 0.4).abs() < 1e-6 && (Knob::GainTemperature.get(&green) - 0.6).abs() < 1e-6);
    assert!((mean(&green) - mean(&s)).abs() < 1e-6);
    assert_eq!(FieldSet::between(&s, &green).iter().collect::<Vec<_>>(), vec![Field::Gains]);
    // Twin: moving red alone shifts the tint and the mean, which the axes exist to avoid.
    let red = Knob::Gain(Channel::Red).set(&s, 0.9);
    assert!((Knob::GainTint.get(&red) - Knob::GainTint.get(&s)).abs() > 0.1 && (mean(&red) - mean(&s)).abs() > 0.1);
}

#[test]
fn a_strength_redoses_as_the_panes_do() {
    let mut s = PipelineSettings::default();
    s.balance_filters = Some([0.4, 0.0, -0.4]);
    let half = Knob::BalanceStrength.set(&s, 0.5);
    assert_eq!(half.print.red.exposure, 0.2);
    // Twin: a frame no key balanced keeps its filters under the same strength.
    let unbalanced = Knob::BalanceStrength.set(&PipelineSettings::default(), 0.5);
    assert_eq!(unbalanced.print.red.exposure, 0.0);
}

#[test]
fn the_grains_colour_ends_write_the_grain_alone_and_hold_their_range() {
    let s = PipelineSettings::rest(oni_settings::ConversionMode::Digital, true);
    let black = Knob::GrainColourBlack.set(&s, 9.0);
    assert_eq!(black.grain.colour, GrainColour { black: GrainColour::RANGE.1, white: GrainColour::REST.white });
    assert_eq!(FieldSet::between(&s, &black).iter().collect::<Vec<_>>(), vec![Field::Grain]);
    let white = Knob::GrainColourWhite.set(&s, 0.6);
    assert_eq!((Knob::GrainColourWhite.get(&white), Knob::GrainColourBlack.get(&white)), (0.6, GrainColour::REST.black));
    // Twin: the black end is not the white's, and stays where it was.
    assert_ne!(Knob::GrainColourWhite.get(&black), GrainColour::RANGE.1);
}

#[test]
fn the_film_exposure_moves_a_digital_photos_film_alone() {
    let s = PipelineSettings::rest(oni_settings::ConversionMode::Digital, true);
    let over = Knob::FilmExposure.set(&s, 9.0);
    assert_eq!((Knob::FilmExposure.get(&over), over.film().map(|f| f.stock)), (VirtualFilm::EXPOSURE_RANGE.1, s.film().map(|f| f.stock)));
    assert_eq!(FieldSet::between(&s, &over).iter().collect::<Vec<_>>(), vec![Field::VirtualFilm]);
    // Twin: a negative shot on no film keeps its settings whole.
    let negative = PipelineSettings::rest(oni_settings::ConversionMode::Negative, true);
    assert_eq!(Knob::FilmExposure.set(&negative, 1.0), negative);
}

/// C, M and Y read Temperature and Tint back to the bit, and an edit of either lands on the other exactly.
#[test]
fn c_m_and_y_read_temperature_and_tint_and_write_them_back_without_drift() {
    let cmy = |s: &PipelineSettings| Channel::ALL.map(|c| Knob::Filtration(c).get(s));
    let (mut worst, mut largest) = (0f32, 0f32);
    for i in 0..=20 {
        for j in 0..=20 {
            let t = -2.0 + 0.2 * i as f32;
            let tint = -1.5 + 0.15 * j as f32;
            let s = Knob::Tint.set(&Knob::Temperature.set(&PipelineSettings::default(), t), tint);
            let read = cmy(&s);
            assert!(read.contains(&0.0) && read.iter().all(|v| *v >= 0.0), "the least at none: {read:?}");
            largest = largest.max(read.iter().copied().fold(0.0, f32::max));
            // Each value written back as read moves nothing, to the bit.
            for c in Channel::ALL {
                assert_eq!(Knob::Filtration(c).set(&s, read[c as usize]), s, "{c:?} at {t} {tint}");
            }
            // Six CC more of a filter above none reads six more, Temperature and Tint moving by its share inside their tracks.
            let inside = t.abs() <= 1.6 && tint.abs() <= 1.2;
            if let Some(c) = Channel::ALL.into_iter().find(|c| inside && read[*c as usize] > 0.0) {
                let moved = Knob::Filtration(c).set(&s, read[c as usize] + 6.0);
                let after = cmy(&moved);
                worst = worst.max((after[c as usize] - read[c as usize] - 6.0).abs());
                // And six back lands on the settings it left.
                let back = Knob::Filtration(c).set(&moved, after[c as usize] - 6.0);
                worst = worst.max((Knob::Temperature.get(&back) - t).abs().max((Knob::Tint.get(&back) - tint).abs()));
            }
        }
    }
    println!("C, M and Y within {worst:.1e} of the edit, the largest filter {largest:.1} CC");
    assert!(worst < 1e-5 && largest <= oni_settings::tone::Print::FILTRATION_RANGE.1, "{worst} {largest}");
    // Twin: a mapping that writes Tint as the filters' plain mean, forgetting green's double weight, drifts.
    let s = Knob::Tint.set(&PipelineSettings::default(), 0.6);
    let c = cmy(&s);
    let lossy = ((c[0] + c[2]) / 2.0 - c[1]) / oni_settings::tone::CC_PER_STOP * (2.0 / 3.0);
    assert!((lossy - Knob::Tint.get(&s)).abs() > 0.1, "{lossy}");
}
