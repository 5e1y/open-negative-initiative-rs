//! `NegativeView.selfCheck`, its reference: Compare's before is the frame at rest, the look's every
//! gesture gone and the facts of the frame kept.

use oni_core::{balance, compare};
use oni_settings::cg::{Point, Rect};
use oni_settings::curves::SaturationCurve;
use oni_settings::frame::{Correction, FlatField, FlatFieldMask, RollReference};
use oni_settings::id::Id;
use oni_settings::scanner::ScannerClicks;
use oni_settings::tone::Print;
use oni_settings::{ConversionMode, Face, PipelineSettings};

#[test]
fn compares_before_is_the_frame_at_rest_with_its_window_rig_capture_crop_and_dust() {
    let rest = PipelineSettings::rest(ConversionMode::Negative, true);
    let roll = RollReference { id: Id::new(), stops: [0.4, 0.0, -0.3], black: [0.19, 0.23, 0.31], white: [0.53, 0.58, 0.67], base: None, median: None };
    let mut edited = balance::written(&roll, &rest);
    edited.geometry.crop = Rect { x: 0.1, y: 0.1, width: 0.8, height: 0.8 };
    edited.flat_field = FlatField { mask: FlatFieldMask::Custom, custom: Some("3f9a0c7d51e2b846".into()), amount: 1.0, ..FlatField::default() };
    edited.capture_stops = 0.37;
    edited.corrections = vec![Correction { radius: 9.5, ..Correction::new(vec![Point::new(0.3, 0.4)]) }];
    let rested = edited.clone();
    edited.print.exposure += 0.8;
    edited.print.black_point = 0.2;
    edited.print.red.exposure = 0.3;
    edited.saturation = SaturationCurve::converted(0.5, 0.0);
    edited.sharpen.amount = 0.9;
    edited.separation = 0.6;
    edited.density = -0.2;
    edited.scanner = ScannerClicks::new(3, -2, 0, 0, 0);
    let reference = compare::reference(&edited);
    assert_eq!(reference, rested, "the look's every gesture gone, nothing else");
    assert!(reference.roll_reference == edited.roll_reference && reference.levels == rested.levels);
    assert!(reference.flat_field == edited.flat_field && reference.capture_stops == edited.capture_stops);
    assert!(reference.geometry == edited.geometry && reference.corrections == edited.corrections);
    assert!(reference.print.exposure == Print::resting_exposure(ConversionMode::Negative) && reference.scanner == ScannerClicks::default());
    // Twin: a neutral block carrying the gains and levels alone misses the exposure and the flat-field.
    let former = PipelineSettings { mode: edited.mode, geometry: edited.geometry, gains: edited.gains, levels: edited.levels, ..PipelineSettings::default() };
    assert!(former.print.exposure != reference.print.exposure && former.flat_field != reference.flat_field);
}

#[test]
fn on_the_scanner_face_the_before_is_the_pro_faces_plain_conversion() {
    let roll = RollReference { id: Id::new(), stops: [0.4, 0.0, -0.3], black: [0.19, 0.23, 0.31], white: [0.53, 0.58, 0.67], base: None, median: None };
    let mut pro = balance::written(&roll, &PipelineSettings::rest(ConversionMode::Negative, true));
    pro.print.exposure += 0.5;
    let scanner = PipelineSettings { face: Face::Scanner, scanner: ScannerClicks::new(2, 0, -1, 0, 0), ..pro.clone() };
    let before = compare::reference(&scanner);
    assert_eq!(before.face, Face::Pro, "the Scanner's before leaves its table out");
    assert_eq!(before, compare::reference(&pro), "the same before on either face");
    // Twin: the reset the bar lands on keeps the Scanner face, so its table would still show.
    assert_eq!(oni_core::parameters::imported(&scanner).face, Face::Scanner);
}
