//! Naming, the resize rules, the settings preference and the presets, each against its twin.

use std::path::{Path, PathBuf};

use oni_colour::Space;
use oni_export::{Format, Preset, Resize, Settings, clashes, name, stem};

#[test]
fn a_batch_flags_what_it_would_overwrite() {
    let there = PathBuf::from("/tmp/oni-export/a.tif");
    let absent = PathBuf::from("/tmp/oni-export/b.tif");
    let found = clashes(&[there.clone(), absent.clone()], |p| p == there.as_path());
    assert_eq!(found, vec![there.clone()]);
    // Twin: an empty folder flags nothing, so the predicate is what flagged.
    assert!(clashes(std::slice::from_ref(&absent), |_| false).is_empty());
    // A name produced twice within the batch clashes even in an empty folder.
    assert_eq!(clashes(&[absent.clone(), absent.clone()], |_| false).len(), 1);
    // Lexically the same path, spelt with a `.` segment.
    assert_eq!(clashes(&[absent.clone(), PathBuf::from("/tmp/./oni-export/b.tif")], |_| false).len(), 1);
}

#[test]
fn names_cannot_fabricate_a_path() {
    let source = Path::new("/tmp/P1072392.RW2");
    assert_eq!(name(source, "P1072392", Format::Tiff16), "P1072392.tiff");
    assert_eq!(name(source, "", Format::Jpeg), "P1072392.jpg");
    assert_eq!(name(source, "  ", Format::Jpeg), "P1072392.jpg");
    assert_eq!(name(source, "a/b:c", Format::Jpeg), "a-b-c.jpg");
    assert_eq!(stem(source), "P1072392");
}

#[test]
fn one_ceiling_on_an_edge_holds_across_a_roll() {
    let landscape = (8368.0, 5584.0);
    let portrait = (5584.0, 8368.0);
    assert_eq!(Resize::Full.scale(landscape.0, landscape.1), 1.0);
    assert_eq!(
        Resize::LongestEdge(4184).scale(landscape.0, landscape.1),
        Resize::LongestEdge(4184).scale(portrait.0, portrait.1)
    );
    assert_eq!(Resize::LongestEdge(4184).output_size(landscape.0, landscape.1), (4184.0, 2792.0));
    assert_eq!(Resize::LongestEdge(20000).scale(landscape.0, landscape.1), 1.0);

    let three_two = (8368.0, 5579.0);
    let four_five = (4000.0, 5000.0);
    assert_eq!(Resize::ShortestEdge(1440).output_size(three_two.0, three_two.1), (2160.0, 1440.0));
    assert_eq!(Resize::ShortestEdge(1440).output_size(four_five.0, four_five.1), (1440.0, 1800.0));
    // Twin: the same number on the long edge narrows the portrait.
    assert_eq!(Resize::LongestEdge(1440).output_size(four_five.0, four_five.1).0, 1152.0);
    assert_eq!(Resize::ShortestEdge(9000).scale(three_two.0, three_two.1), 1.0);
}

#[test]
fn the_two_size_fields_ride_the_edge_they_measure() {
    let sheet = (8368.0, 5584.0);
    let by_width = Resize::typing_width(2000, sheet);
    assert_eq!(by_width, Resize::LongestEdge(2000));
    assert_eq!(by_width.output_size(sheet.0, sheet.1), (2000.0, 1335.0));
    let by_height = Resize::typing_height(1335, sheet);
    assert_eq!(by_height, Resize::ShortestEdge(1335));
    assert_eq!(by_height.output_size(sheet.0, sheet.1), (2001.0, 1335.0));
    // Twin: truncating the ratio says 2000 where the file says 2001.
    assert_eq!((1335.0 * sheet.0 / sheet.1) as i64, 2000);
    let tall = (558.0, 837.0);
    assert_eq!(Resize::typing_width(341, tall), Resize::ShortestEdge(341));
    assert_eq!(Resize::typing_height(512, tall), Resize::LongestEdge(512));
    for typed in [9000, 8368, 0] {
        assert_eq!(Resize::typing_width(typed, sheet), Resize::Full);
    }
    // A size that is no number never rides an edge; twin: the same width on a real sheet does.
    assert_eq!(Resize::typing_width(2000, (f64::NAN, 5584.0)), Resize::Full);
    assert_eq!(Resize::LongestEdge(2000).scale(f64::NAN, f64::NAN), 1.0);
    assert_eq!(Resize::typing_width(2000, (8368.0, 5584.0)), Resize::LongestEdge(2000));
}

#[test]
fn the_settings_round_trip_through_their_preference() {
    let mut tuned = Settings::default();
    tuned.set_format(Format::Tiff16);
    tuned.set_space(Space::AdobeRgb);
    tuned.compress_tiff = false;
    tuned.jpeg_quality = 0.92;
    tuned.resize = Resize::ShortestEdge(1440);
    tuned.hdr = true;
    assert_eq!(Settings::from_raw_value(&tuned.raw_value()), Some(tuned.clone()));
    let rest = Settings::default();
    assert_eq!(Settings::from_raw_value(&rest.raw_value()), Some(rest.clone()));
    assert!(rest.format() == Format::Jpeg && *rest.space() == Space::DisplayP3 && !rest.hdr);
    // Twin: a document that only names the default reads the default, not the tuned state.
    assert_ne!(Settings::from_raw_value(&rest.raw_value()), Some(tuned));
}

#[test]
fn the_preference_text_is_the_swift_encoders() {
    // `JSONEncoder` with `.sortedKeys`: no optional written as null, whole numbers without `.0`.
    assert_eq!(
        Settings::default().raw_value(),
        r#"{"compressTIFF":true,"format":"jpeg","hdr":false,"jpegQuality":0.85,"resizeEdge":"full","space":"p3"}"#
    );
    let mut print = Preset::FullPrint.applied(&Settings::default());
    print.resize = Resize::LongestEdge(4000);
    assert_eq!(
        print.raw_value(),
        r#"{"compressTIFF":true,"format":"tiff16","hdr":false,"jpegQuality":1,"resizeEdge":"long","resizePixels":4000,"space":"adobergb"}"#
    );
    // Twin: serde's own text writes `1.0`, which the Swift encoder never does.
    assert!(serde_json::to_string(&1.0f64).unwrap().contains('.'));
}

#[test]
fn prophoto_is_written_in_tiff_only() {
    let mut wide = Settings::default();
    wide.set_space(Space::ProPhoto);
    let wide_format = wide.format();
    wide.set_format(Format::Jpeg);
    let stored = Settings::from_raw_value(r#"{"format":"jpeg","space":"prophoto"}"#).unwrap();
    assert_eq!(wide_format, Format::Tiff16);
    assert_eq!(*wide.space(), Space::AdobeRgb);
    assert_eq!((stored.space().clone(), stored.format()), (Space::ProPhoto, Format::Tiff16));
    // Twin: Adobe RGB keeps its JPEG, set or stored.
    let mut narrow = Settings::default();
    narrow.set_space(Space::AdobeRgb);
    narrow.set_format(Format::Jpeg);
    assert_eq!((narrow.space().clone(), narrow.format()), (Space::AdobeRgb, Format::Jpeg));
    let stored = Settings::from_raw_value(r#"{"format":"jpeg","space":"adobergb"}"#).unwrap();
    assert_eq!(stored.format(), Format::Jpeg);
}

#[test]
fn an_elder_preference_reads_its_defaults() {
    let elder = Settings::from_raw_value(r#"{"format":"tiff16","space":"Wide Gamut","jpegQuality":0.7}"#).unwrap();
    assert_eq!(elder.format(), Format::Tiff16);
    assert_eq!(elder.jpeg_quality, 0.7);
    assert!(!elder.hdr);
    assert_eq!(elder.resize, Resize::Full);
    assert_eq!(*elder.space(), Space::DisplayP3);
    // Twin: a strict decoder of the full shape refuses that same document.
    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct Strict {
        format: String,
        hdr: bool,
        #[serde(rename = "resizeEdge")]
        resize_edge: String,
    }
    assert!(serde_json::from_str::<Strict>(r#"{"format":"tiff16","space":"Wide Gamut","jpegQuality":0.7}"#).is_err());
    assert!(Settings::from_raw_value("not json").is_none());
    // A size of zero or less is no ceiling.
    let zero = Settings::from_raw_value(r#"{"resizeEdge":"long","resizePixels":0}"#).unwrap();
    assert_eq!(zero.resize, Resize::Full);
}

#[test]
fn the_metadata_keeps_the_swift_field_names() {
    let typed = oni_export::ExifMetadata { camera: "Leica M6".into(), iso: "400".into(), ..Default::default() };
    let text = serde_json::to_string(&typed).unwrap();
    for key in ["camera", "lens", "film", "iso", "date", "artist", "copyright"] {
        assert!(text.contains(&format!("\"{key}\":")), "{key} missing in {text}");
    }
    assert_eq!(serde_json::from_str::<oni_export::ExifMetadata>(&text).unwrap(), typed);
    // Twin: the Swift decoder requires every key, and so does this one.
    assert!(serde_json::from_str::<oni_export::ExifMetadata>(r#"{"camera":"x"}"#).is_err());
}

// MARK: - Presets

#[test]
fn every_key_writes_a_distinct_complete_state() {
    let hand = Settings::default();
    let states: Vec<Settings> = Preset::ALL.iter().map(|p| p.applied(&hand)).collect();
    for (i, a) in states.iter().enumerate() {
        for b in &states[i + 1..] {
            assert_ne!(a, b);
        }
    }
    assert!(states.iter().all(|s| !s.hdr), "no key turns the HDR hack on");
    let insta = Preset::Instagram.applied(&hand);
    assert_eq!(
        (insta.resize, insta.space().clone(), insta.jpeg_quality),
        (Resize::ShortestEdge(1440), Space::Srgb, 0.92)
    );
    let mut print_as_jpeg = Preset::FullPrint.applied(&hand);
    print_as_jpeg.set_format(Format::Jpeg);
    print_as_jpeg.set_space(Space::DisplayP3);
    assert_eq!(Preset::FullPrint.applied(&hand).format(), Format::Tiff16);
    assert_eq!(*Preset::FullPrint.applied(&hand).space(), Space::AdobeRgb);
    assert_eq!(print_as_jpeg, Preset::FullDisplay.applied(&hand));

    let mut other = Settings::default();
    other.set_format(Format::Tiff16);
    other.set_space(Space::DisplayP3);
    other.compress_tiff = false;
    other.jpeg_quality = 0.3;
    other.resize = Resize::LongestEdge(100);
    other.hdr = true;
    assert!(Preset::ALL.iter().all(|p| p.applied(&hand) == p.applied(&other)));
    // ProPhoto behind a key still lands on the key's state.
    let mut prophoto = Settings::default();
    prophoto.set_space(Space::ProPhoto);
    assert!(Preset::ALL.iter().all(|p| p.applied(&hand) == p.applied(&prophoto)));
}

#[test]
fn the_lit_key_is_read_off_the_state() {
    let hand = Settings::default();
    assert!(Preset::ALL.iter().all(|p| Preset::matching(&p.applied(&hand)) == Some(*p)));
    let mut edited = Preset::Instagram.applied(&hand);
    edited.jpeg_quality = 0.9;
    assert_eq!(Preset::matching(&edited), None);
    edited.jpeg_quality = 0.92;
    assert_eq!(Preset::matching(&edited), Some(Preset::Instagram));
    // Twin: a plate remembering the last click would still light Instagram.
    let last = Preset::FullPrint.applied(&Preset::Instagram.applied(&hand));
    assert_eq!(Preset::matching(&last), Some(Preset::FullPrint));
    assert_eq!(Preset::matching(&Settings::default()), None);
}

#[test]
fn presets_serialise_under_their_swift_names() {
    let names: Vec<String> = Preset::ALL.iter().map(|p| serde_json::to_string(p).unwrap()).collect();
    assert_eq!(names, ["\"Instagram\"", "\"Display\"", "\"Print\""]);
    assert!(Preset::ALL.iter().all(|p| format!("\"{}\"", p.raw_value()) == serde_json::to_string(p).unwrap()));
    assert_eq!(serde_json::from_str::<Preset>("\"Display\"").unwrap(), Preset::FullDisplay);
    // Twin: the Rust variant name is not the serialised one.
    assert!(serde_json::from_str::<Preset>("\"FullDisplay\"").is_err());
}
