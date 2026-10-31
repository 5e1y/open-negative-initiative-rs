use super::*;
use crate::fields::declared_names;

#[test]
fn every_field_is_a_rows_or_excluded_and_never_both() {
    let covered: BTreeSet<&str> = all().iter().flat_map(|r| r.covers.iter().copied()).collect();
    let excluded: BTreeSet<&str> = EXCLUDED.iter().map(|(f, _)| *f).collect();
    let declared: BTreeSet<&str> = declared_names().iter().copied().collect();
    assert!(covered.is_disjoint(&excluded));
    assert_eq!(covered.union(&excluded).copied().collect::<BTreeSet<_>>(), declared);
    // Twin: dropping one excluded field leaves the union short of the struct.
    let short: BTreeSet<&str> = excluded.iter().copied().filter(|f| *f != "passOf").collect();
    assert_ne!(covered.union(&short).copied().collect::<BTreeSet<_>>(), declared);
}

#[test]
fn a_row_carries_its_field_and_a_slider_carries_only_its_value() {
    let mut donor = PipelineSettings::default();
    donor.gains.stops = [0.4, -0.2, 0.1];
    donor.sharpen.amount = 2.0;
    let target = PipelineSettings::default();
    let rows = BTreeSet::from(["gains".to_string()]);
    assert_eq!(apply(&rows, &donor, &target).gains, donor.gains);
    assert_eq!(apply(&rows, &donor, &target).sharpen, target.sharpen);
    let one = BTreeSet::from(["gains.green".to_string()]);
    assert_eq!(apply(&one, &donor, &target).gains.stops, [0.0, -0.2, 0.0]);
    // Twin: a name no row or slider owns carries nothing.
    let none = BTreeSet::from(["geometry".to_string()]);
    assert_eq!(apply(&none, &donor, &target), target);
}

#[test]
fn lighting_leaves_the_filters_and_color_carries_them() {
    let mut donor = PipelineSettings::default();
    donor.print.exposure = 0.9;
    donor.print.red.exposure = 0.3;
    let target = PipelineSettings::default();
    let lit = apply(&BTreeSet::from(["levelsGlobal".to_string()]), &donor, &target);
    assert_eq!((lit.print.exposure, lit.print.red.exposure), (0.9, 0.0));
    let coloured = apply(&BTreeSet::from(["curves".to_string()]), &donor, &target);
    assert_eq!((coloured.print.exposure, coloured.print.red.exposure), (target.print.exposure, 0.3));
}

#[test]
fn the_reset_drops_the_crop_and_keeps_its_window_flat_field_and_dye_family() {
    use oni_settings::blocks::SeparationFamily;
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.geometry.angle = 2.5;
    s.geometry.crop = oni_settings::cg::Rect::new(0.1, 0.1, 0.8, 0.8);
    s.geometry.measure_window = Some(oni_settings::cg::Rect::new(0.2, 0.2, 0.5, 0.5));
    s.flat_field.amount = 0.4;
    s.separation_family = Some(SeparationFamily::FujiC41);
    s.separation = 1.3;
    s.print.exposure = 0.2;
    s.sharpen.amount = 2.5;
    let blank = reset_blank(&s, false);
    assert_eq!((blank.geometry.angle, blank.geometry.crop), (0.0, oni_settings::cg::Rect::UNIT), "the crop gone");
    assert_eq!((blank.geometry.measure_window, blank.flat_field.amount), (s.geometry.measure_window, 0.4));
    assert_eq!(blank.separation_family, s.separation_family);
    // Twin: the look goes back to rest, so the check sees a reset that kept too much.
    let rest = PipelineSettings::rest(ConversionMode::Negative, true);
    assert_eq!((blank.separation, blank.print.exposure, blank.sharpen), (rest.separation, rest.print.exposure, rest.sharpen));
    assert_ne!(blank, s);
}

/// A slide's gains-only measure stands on no reference: its measured gains are its rest and a reset lands there.
#[test]
fn the_reset_lands_a_slide_back_on_its_measured_gains() {
    let mut slide = PipelineSettings::rest(ConversionMode::Positive, true);
    slide.measured_gains = Some([-0.3, 0.0, 0.4]);
    slide.gains.stops = [-0.1, 0.0, 0.6];
    slide.print.exposure = 0.6;
    let blank = reset_blank(&slide, false);
    assert_eq!(blank.gains.stops, [-0.3, 0.0, 0.4], "the hand's move gone, the measure kept");
    assert_eq!(blank.print.exposure, PipelineSettings::rest(ConversionMode::Positive, true).print.exposure, "the look reset");
    assert_eq!(unconverted(&slide).gains.stops, [0.0; 3], "reset to default forgets the measure");
    // Twin: a slide never measured has no gains to land on but rest.
    let mut bare = slide.clone();
    bare.measured_gains = None;
    assert_eq!(reset_blank(&bare, false).gains.stops, [0.0; 3]);
}

#[test]
fn changed_names_the_rows_off_rest() {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, false);
    assert!(changed(&s).is_empty());
    s.texture.amount = 0.5;
    assert_eq!(changed(&s), BTreeSet::from(["texture"]));
}
