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
fn a_digital_white_rests_on_its_decode_and_moves_its_kelvin_and_tint_apart() {
    let s = PipelineSettings::rest(oni_settings::ConversionMode::Digital, true);
    assert_eq!((Knob::WhiteKelvin.get(&s), Knob::WhiteTint.get(&s)), (5000.0, 0.0));
    let warm = Knob::WhiteKelvin.set(&Knob::WhiteTint.set(&s, 12.0), 3200.0);
    assert_eq!(warm.white, Some(White { kelvin: 3200.0, tint: 12.0 }));
    assert_eq!(FieldSet::between(&s, &warm).iter().collect::<Vec<_>>(), vec![Field::White]);
    assert_eq!(Knob::WhiteKelvin.get(&Knob::WhiteKelvin.set(&s, 90000.0)), white::KELVIN_RANGE.1);
    // Twin: a plain file rests on D65, not on the mosaic's 5000 K.
    let plain = PipelineSettings::rest(oni_settings::ConversionMode::Digital, false);
    assert!((Knob::WhiteKelvin.get(&plain) - 6504.0).abs() < 2.0);
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
fn a_dose_by_hand_and_a_strength_redose_as_the_panes_do() {
    let mut s = PipelineSettings::default();
    let dosed = Knob::Separation.set(&s, 1.2);
    assert!(dosed.separation_by_hand);
    s.balance_filters = Some([0.4, 0.0, -0.4]);
    let half = Knob::BalanceStrength.set(&s, 0.5);
    assert_eq!(half.print.red.exposure, 0.2);
    // Twin: a frame no key balanced keeps its filters under the same strength.
    let unbalanced = Knob::BalanceStrength.set(&PipelineSettings::default(), 0.5);
    assert_eq!(unbalanced.print.red.exposure, 0.0);
}

#[test]
fn a_picture_window_keeps_its_ends_on_the_picture_and_in_order() {
    let s = PipelineSettings::rest(oni_settings::ConversionMode::Digital, true);
    let black = Knob::Picture(LevelsChannel::Red, Handle::Black);
    let floored = black.set(&s, -0.3);
    assert_eq!(black.get(&floored), 0.0, "a black under the picture lands on 0");
    assert_eq!(FieldSet::between(&s, &floored).iter().collect::<Vec<_>>(), Vec::<Field>::new(), "0 is its rest");
    let lifted = black.set(&s, 0.2);
    assert_eq!((lifted.picture_levels.red.black, lifted.levels), (0.2, s.levels), "the picture's window alone");
    assert_eq!(FieldSet::between(&s, &lifted).iter().collect::<Vec<_>>(), vec![Field::PictureLevels]);
    let white = Knob::Picture(LevelsChannel::Red, Handle::White);
    assert_eq!(white.get(&white.set(&lifted, 0.1)), 0.2 + Levels::EPSILON, "the white never crosses the black");
    assert_eq!(white.get(&white.set(&s, 1.7)), 1.0);
    // Twin: a film window's black reaches under zero, where its track begins.
    let film = Knob::Level(Channel::Red, Handle::Black);
    assert!(film.get(&film.set(&s, -0.3)) < 0.0);
}
