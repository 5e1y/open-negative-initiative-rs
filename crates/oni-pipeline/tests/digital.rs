//! A digital photo: a positive with no film between the light and the sensor. At its decode's white it
//! renders as a positive at rest, its film blocks stored but never rendered, and its white balances it.

mod common;

use common::{graph, noise, whole};
use oni_graph::{Graph, Source};
use oni_pipeline::{Inputs, Pipeline, Request, conversion};
use oni_settings::frame::{FlatField, FlatFieldMask, RollReference};
use oni_settings::white::{self, White};
use oni_settings::{ConversionMode, Id, PipelineSettings};

const SIZE: (u32, u32) = (64, 48);

fn source(g: &mut Graph) -> Source {
    g.upload(SIZE.0, SIZE.1, &noise(SIZE.0, SIZE.1, 7, 0.01, 1.2)).expect("the scan")
}

fn render(g: &mut Graph, scan: &Source, s: &PipelineSettings) -> Vec<[f32; 4]> {
    let image = Pipeline::new().image(g, scan, s, &Request::default(), &Inputs::default()).expect("the recipe builds");
    whole(g, &image)
}

fn digital(mosaic: bool) -> PipelineSettings {
    PipelineSettings::rest(ConversionMode::Digital, mosaic)
}

/// Every film block a prepared negative roll holds, off its rest.
fn filmed(mut s: PipelineSettings) -> PipelineSettings {
    s.capture_stops = 0.7;
    s.flat_field = FlatField { mask: FlatFieldMask::ValoiEasy35V1, ..Default::default() };
    s.gains.stops = [0.4, 0.0, -0.3];
    s.levels.red.black += 0.2;
    s.print.red.exposure = 0.3;
    s.separation = 0.9;
    s.roll_reference = Some(RollReference { id: Id::new(), stops: [0.4, 0.0, -0.3], black: [0.1; 3], white: [0.9; 3], base: None, median: None });
    s
}

#[test]
fn a_plain_file_in_digital_at_its_white_renders_as_a_positive_at_rest() {
    let check = "a_plain_file_in_digital_at_its_white_renders_as_a_positive_at_rest";
    let Some(mut g) = graph(check) else { return };
    let scan = source(&mut g);
    let positive = render(&mut g, &scan, &PipelineSettings::rest(ConversionMode::Positive, false));
    let mut at_d65 = digital(false);
    assert_eq!(render(&mut g, &scan, &at_d65), positive, "no white chosen");
    at_d65.white = Some(white::of_uv(white::FILE_WHITE));
    assert_eq!(render(&mut g, &scan, &at_d65), positive, "the file's own white chosen");
    // Twin: a tungsten white moves every pixel.
    let tungsten = PipelineSettings { white: Some(White { kelvin: 3200.0, tint: 0.0 }), ..digital(false) };
    assert_ne!(render(&mut g, &scan, &tungsten), positive);
}

#[test]
fn a_digital_frame_renders_none_of_its_film_blocks() {
    let check = "a_digital_frame_renders_none_of_its_film_blocks";
    let Some(mut g) = graph(check) else { return };
    let scan = source(&mut g);
    let fresh = render(&mut g, &scan, &digital(true));
    assert_eq!(render(&mut g, &scan, &filmed(digital(true))), fresh, "a prepared roll's blocks wait, unrendered");
    // Twin: the same blocks on a slide reach its render.
    let slide = PipelineSettings::rest(ConversionMode::Positive, true);
    assert_ne!(render(&mut g, &scan, &filmed(slide.clone())), render(&mut g, &scan, &slide));
}

#[test]
fn a_film_frame_renders_no_white_and_a_digital_one_its_own() {
    let check = "a_film_frame_renders_no_white_and_a_digital_one_its_own";
    let Some(mut g) = graph(check) else { return };
    let scan = source(&mut g);
    let warm = Some(White { kelvin: 7500.0, tint: 12.0 });
    for mode in [ConversionMode::Negative, ConversionMode::Positive, ConversionMode::Monochrome] {
        let rest = PipelineSettings::rest(mode, true);
        let whitened = PipelineSettings { white: warm, shot: warm, ..rest.clone() };
        assert_eq!(render(&mut g, &scan, &whitened), render(&mut g, &scan, &rest), "{mode:?}");
        assert_eq!(conversion::stage47(&whitened), conversion::stage47(&rest), "{mode:?}");
    }
    // Twin: the same white on a digital frame moves its render.
    let balanced = PipelineSettings { white: warm, ..digital(true) };
    assert_ne!(render(&mut g, &scan, &balanced), render(&mut g, &scan, &digital(true)));
}

#[test]
fn a_digital_white_is_a_stage_only_off_its_decodes_own() {
    assert_eq!(conversion::white(&digital(true)), None);
    assert_eq!(conversion::white(&PipelineSettings { white: Some(White { kelvin: 5000.0, tint: 0.0 }), ..digital(true) }), None);
    assert_eq!(conversion::white(&PipelineSettings { white: Some(white::of_uv(white::FILE_WHITE)), ..digital(false) }), None);
    let film = PipelineSettings { white: Some(White { kelvin: 3200.0, tint: 0.0 }), ..PipelineSettings::rest(ConversionMode::Positive, true) };
    assert_eq!(conversion::white(&film), None);
    // Twin: the same white on a digital frame is a stage, and the mosaic's 5000 K on a plain file one too.
    assert!(conversion::white(&PipelineSettings { mode: ConversionMode::Digital, ..film }).is_some());
    assert!(conversion::white(&PipelineSettings { white: Some(White { kelvin: 5000.0, tint: 0.0 }), ..digital(false) }).is_some());
}

#[test]
fn a_digital_photo_rests_as_a_positive_does() {
    for mosaic in [true, false] {
        let (d, p) = (digital(mosaic), PipelineSettings::rest(ConversionMode::Positive, mosaic));
        assert_eq!((d.levels, d.density_ceiling, d.print), (p.levels, p.density_ceiling, p.print));
        assert_eq!(conversion::stage47(&d), conversion::stage47(&p));
    }
    // Twin: a negative rests elsewhere.
    assert_ne!(conversion::stage47(&digital(true)), conversion::stage47(&PipelineSettings::rest(ConversionMode::Negative, true)));
}

/// Every window a hand can move on a digital photo, off its rest.
fn levelled(mut s: PipelineSettings) -> PipelineSettings {
    use oni_settings::levels::Levels;
    s.picture_levels.linked = Levels { black: 0.05, white: 0.85, mid: 0.4, ..Levels::default() };
    s.picture_levels.luma = Levels { highlights: 0.65, ..Levels::default() };
    s.picture_levels.red.set_white(0.7);
    s
}

#[test]
fn picture_levels_render_on_a_digital_photo_alone_and_rest_bit_for_bit() {
    let check = "picture_levels_render_on_a_digital_photo_alone_and_rest_bit_for_bit";
    let Some(mut g) = graph(check) else { return };
    let scan = source(&mut g);
    let mut pipeline = Pipeline::new();
    let mut seen = |s: &PipelineSettings| {
        let image = pipeline.image(&mut g, &scan, s, &Request::default(), &Inputs::default()).expect("the recipe builds");
        (image.key(), image.stages(), whole(&mut g, &image))
    };
    for mode in [ConversionMode::Negative, ConversionMode::Positive, ConversionMode::Monochrome] {
        let rest = PipelineSettings::rest(mode, true);
        assert!(seen(&levelled(rest.clone())) == seen(&rest), "{mode:?}: stored levels wait, unrendered");
        // With a curve of its own, the film frame's tables are its curves' alone.
        let mut curved = rest.clone();
        curved.manual_curves.linked.add(oni_settings::cg::Point::new(0.5, 0.6));
        assert!(seen(&levelled(curved.clone())) == seen(&curved), "{mode:?}: a curve's tables bake no stored level");
    }
    let rest = digital(true);
    let resting = PipelineSettings { picture_levels: Default::default(), ..levelled(rest.clone()) };
    assert!(seen(&resting) == seen(&rest), "levels back at rest leave no stage");
    // Twin: the same values on a digital photo lay the curves' stage and move its pixels.
    let (key, stages, pixels) = seen(&levelled(rest.clone()));
    let (rest_key, rest_stages, rest_pixels) = seen(&rest);
    assert!(key != rest_key && stages > rest_stages && pixels != rest_pixels);
}

#[test]
fn a_pipeline_bakes_new_tables_for_new_levels_on_the_same_curves() {
    let check = "a_pipeline_bakes_new_tables_for_new_levels_on_the_same_curves";
    let Some(mut g) = graph(check) else { return };
    let scan = source(&mut g);
    let mut a = levelled(digital(true));
    a.manual_curves.red.add(oni_settings::cg::Point::new(0.5, 0.45));
    let mut b = a.clone();
    b.picture_levels.red.set_white(0.9);
    let mut kept = Pipeline::new();
    render_on(&mut kept, &mut g, &scan, &a);
    let after_a = render_on(&mut kept, &mut g, &scan, &b);
    assert_eq!(after_a, render(&mut g, &scan, &b), "B after A is B");
    // Twin: A and B share their curves, so tables kept by the curves alone would render A's pixels for B.
    assert_ne!(render(&mut g, &scan, &a), after_a);
}

fn render_on(pipeline: &mut Pipeline, g: &mut Graph, scan: &Source, s: &PipelineSettings) -> Vec<[f32; 4]> {
    let image = pipeline.image(g, scan, s, &Request::default(), &Inputs::default()).expect("the recipe builds");
    whole(g, &image)
}

/// Black and white, a digital photo prints its luminance alone, every pixel grey; the switch off, its colour.
#[test]
fn a_digital_photo_in_black_and_white_prints_grey() {
    let check = "a_digital_photo_in_black_and_white_prints_grey";
    let Some(mut g) = graph(check) else { return };
    let scan = source(&mut g);
    let grey = |px: &[[f32; 4]]| px.iter().all(|p| (p[0] - p[1]).abs() < 1e-5 && (p[1] - p[2]).abs() < 1e-5);
    let toned = render(&mut g, &scan, &PipelineSettings { black_and_white: true, ..digital(true) });
    assert!(grey(&toned), "every pixel grey");
    assert!(!grey(&render(&mut g, &scan, &digital(true))), "twin: the switch off keeps the colour");
    // A film frame ignores it: the switch is a digital photo's own.
    let slide = PipelineSettings::rest(ConversionMode::Positive, true);
    assert_eq!(render(&mut g, &scan, &PipelineSettings { black_and_white: true, ..slide.clone() }), render(&mut g, &scan, &slide));
}
