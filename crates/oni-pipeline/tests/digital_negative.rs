//! A digital photo's virtual negative read by the negative's own pipeline: its stage 4-7 a colour negative's,
//! its dyes unmixed on its stock's family, its scene lit at its stock's white, its Scanner face a negative's.

use oni_pipeline::{conversion, scanner};
use oni_settings::blocks::{SeparationFamily, separation};
use oni_settings::{ConversionMode, Face, PipelineSettings, VirtualStock};

fn digital() -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Digital, true);
    s.print.exposure = 1.9;
    s.print.set_filters([0.1, 0.0, -0.15]);
    s.levels.green.mid = 0.4;
    s.gains.stops[0] += 0.2;
    s
}

#[test]
fn a_digital_photos_stage_4_to_7_is_a_colour_negatives_on_the_same_settings() {
    let s = digital();
    let negative = PipelineSettings { mode: ConversionMode::Negative, ..s.clone() };
    assert_eq!(conversion::stage47(&s), conversion::stage47(&negative));
    // Twin: the same settings read as a slide are no negative.
    let slide = PipelineSettings { mode: ConversionMode::Positive, ..s.clone() };
    assert_ne!(conversion::stage47(&s), conversion::stage47(&slide));
}

#[test]
fn a_digital_photo_unmixes_its_stocks_dyes_and_decompresses_as_an_unprepared_negative() {
    let s = PipelineSettings::rest(ConversionMode::Digital, true);
    let k = conversion::stage47(&s);
    assert_eq!(s.separation_family, Some(VirtualStock::KodakPortra800.dyes()));
    let rows = conversion::separation_rows(Some(SeparationFamily::KodakC41), separation::BASE, ConversionMode::Negative);
    assert_eq!([k.sep_r, k.sep_g, k.sep_b], rows);
    assert_eq!((k.decompression, k.film_share), (0.5, 0.5));
    assert_eq!((k.invert, k.own_stops), (1.0, s.print.filters()));
    // Twin: a slide on the same settings unmixes nothing and keeps its film's colour.
    let slide = conversion::stage47(&PipelineSettings { mode: ConversionMode::Positive, ..s.clone() });
    assert_eq!([slide.sep_r, slide.sep_g, slide.sep_b], conversion::IDENTITY);
    assert_eq!((slide.decompression, slide.film_share), (1.0, 0.0));
}

#[test]
fn a_digital_photos_scene_is_lit_at_its_stocks_white_and_a_films_never() {
    let s = digital();
    let m = conversion::scene_white(&s).expect("a digital photo's scene is lit");
    let want = oni_settings::white::relative(oni_settings::white::uv(VirtualStock::KodakPortra800.white()), oni_settings::white::anchor(true));
    assert_eq!([&m.r_vector[..3], &m.g_vector[..3], &m.b_vector[..3]], [&want[0][..], &want[1][..], &want[2][..]]);
    // A plain file's decode stands on D65, so its matrix is another.
    let plain = PipelineSettings { decoded_from_mosaic: false, ..s.clone() };
    assert_ne!(conversion::scene_white(&plain), Some(m));
    // Twin: a negative's scene is no camera's.
    assert!(conversion::scene_white(&PipelineSettings { mode: ConversionMode::Negative, ..s }).is_none());
}

#[test]
fn a_digital_photos_scanner_keys_move_its_negatives_gains_as_a_negatives_do() {
    let mut s = digital();
    s.face = Face::Scanner;
    s.scanner.c = 2;
    let machine = scanner::on_oni_machine(&s).expect("the Scanner face prints on the machine");
    let keys = scanner::gain_stops(&s);
    let negative = scanner::on_oni_machine(&PipelineSettings { mode: ConversionMode::Negative, ..s.clone() }).unwrap();
    assert_eq!(machine.gains.stops, std::array::from_fn(|c| s.gains.stops[c] + keys[c]));
    assert_eq!(machine.gains.stops, negative.gains.stops);
    assert!(keys[0] != 0.0);
    // Twin: Pro's face prints on no machine.
    assert!(scanner::on_oni_machine(&PipelineSettings { face: Face::Pro, ..s }).is_none());
}
