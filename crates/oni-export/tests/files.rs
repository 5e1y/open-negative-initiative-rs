//! Files written then read back by independent readers: the compression, the profile, the pixels,
//! the metadata and nothing else, and the gain-map container, each against its twin.

use std::collections::BTreeSet;
use std::io::Cursor;

use oni_colour::{Quantised, Space};
use oni_export::metadata::tag;
use oni_export::{Format, Image, ExifMetadata, Rendered, Settings, converted, encoded, estimate, gain_map};

mod common;
use common::{frame, jpeg_profile, read_tiff, settings};

fn typed() -> ExifMetadata {
    ExifMetadata {
        camera: "Leica M6".into(),
        lens: "Summicron 50".into(),
        film: "Kodak Portra 400".into(),
        iso: "400".into(),
        date: "2001:02:03 04:05:06".into(),
        artist: "A. Photographer".into(),
        copyright: "© A. Photographer".into(),
    }
}

/// Every tag a reader finds, primary and Exif directories alike, by number.
fn tags(file: &[u8]) -> BTreeSet<u16> {
    let mut reader = exif::Reader::new();
    reader.continue_on_error(true);
    let found = if file.starts_with(b"MM\0*") || file.starts_with(b"II*\0") {
        reader.read_raw(file.to_vec())
    } else {
        reader.read_from_container(&mut Cursor::new(file))
    };
    let found = match found {
        Ok(f) => f,
        Err(exif::Error::PartialResult(p)) => p.into_inner().0,
        Err(_) => return BTreeSet::new(),
    };
    found.fields().map(|f| f.tag.number()).collect()
}

fn text(file: &[u8], wanted: exif::Tag) -> Option<String> {
    let found = if file.starts_with(b"MM\0*") {
        exif::Reader::new().read_raw(file.to_vec()).ok()?
    } else {
        exif::Reader::new().read_from_container(&mut Cursor::new(file)).ok()?
    };
    let field = found.get_field(wanted, exif::In::PRIMARY)?;
    match &field.value {
        exif::Value::Ascii(v) => Some(String::from_utf8_lossy(&v[0]).into_owned()),
        other => Some(format!("{}", other.display_as(wanted))),
    }
}

#[test]
fn the_tiff_is_zip_compressed_lossless_and_profiled() {
    let image = frame(320, 200);
    let export = settings(Format::Tiff16, Space::AdobeRgb);
    let rendered = converted(&image, &export).unwrap();
    let file = encoded(&rendered, &export, &ExifMetadata::default()).unwrap();
    let mut d = read_tiff(&file);
    assert_eq!(d.dimensions().unwrap(), (320, 200));
    assert_eq!(d.colortype().unwrap(), tiff::ColorType::RGBA(16));
    assert_eq!(d.get_tag_u32(tiff::tags::Tag::Compression).unwrap(), 8);
    let icc = d.get_tag_u8_vec(tiff::tags::Tag::IccProfile).unwrap();
    assert_eq!(Some(icc.clone()), Space::AdobeRgb.icc_data());
    let tiff::decoder::DecodingResult::U16(read) = d.read_image().unwrap() else { panic!("16-bit samples") };
    let Quantised::Sixteen(written) = &rendered.pixels else { panic!("16-bit render") };
    assert_eq!(&read, written, "ZIP is lossless to the sample");
    assert!(written.chunks(4).all(|p| p[3] == 65535), "opaque");

    // Twin: switched off, the same frame reads Compression 1 and weighs its exact samples.
    let mut plain = export.clone();
    plain.compress_tiff = false;
    let raw = encoded(&rendered, &plain, &ExifMetadata::default()).unwrap();
    let mut d = read_tiff(&raw);
    assert_eq!(d.get_tag_u32(tiff::tags::Tag::Compression).unwrap(), 1);
    assert!(raw.len() > file.len());
    let samples = 320 * 200 * 8;
    assert!(raw.len() >= samples && raw.len() - samples < icc.len() + 1024, "{} for {samples}", raw.len());
    assert_eq!(estimate(&image, (320 * 200) as f64, &plain), samples as u64);
    // And the profile check discriminates: Adobe RGB's bytes are not sRGB's.
    assert_ne!(Space::AdobeRgb.icc_data(), Space::Srgb.icc_data());
}

#[test]
fn an_eight_bit_tiff_is_written_as_well() {
    let rendered = Rendered {
        width: 3,
        height: 2,
        pixels: Quantised::Eight((0..24).map(|v| v as u8 * 10).collect()),
        icc: Space::Srgb.icc_data().unwrap(),
    };
    let file = encoded(&rendered, &settings(Format::Tiff16, Space::Srgb), &ExifMetadata::default()).unwrap();
    let mut d = read_tiff(&file);
    assert_eq!(d.colortype().unwrap(), tiff::ColorType::RGBA(8));
    let tiff::decoder::DecodingResult::U8(read) = d.read_image().unwrap() else { panic!("8-bit samples") };
    assert_eq!(Quantised::Eight(read), rendered.pixels);
}

#[test]
fn a_large_tiff_spans_several_strips_and_reads_back_whole() {
    // 1100 × 260 at 16 bits is 2.2 MB: three strips of 1 MiB.
    let image = frame(1100, 260);
    let export = settings(Format::Tiff16, Space::Srgb);
    let rendered = converted(&image, &export).unwrap();
    let file = encoded(&rendered, &export, &ExifMetadata::default()).unwrap();
    let mut d = read_tiff(&file);
    assert!(d.strip_count().unwrap() >= 3, "{} strips", d.strip_count().unwrap());
    let tiff::decoder::DecodingResult::U16(read) = d.read_image().unwrap() else { panic!() };
    assert_eq!(Quantised::Sixteen(read), rendered.pixels);
}

/// The layout, and what ImageIO states of every TIFF: fill order, orientation, unit, sample format.
const STRUCTURE: [u16; 16] = [256, 257, 258, 259, 262, 266, 273, 274, 277, 278, 279, 284, 296, 338, 339, 34675];
const STATED: [u16; 8] = [
    tag::IMAGE_DESCRIPTION,
    tag::MODEL,
    tag::DATE_TIME,
    tag::ARTIST,
    tag::COPYRIGHT,
    tag::ISO_SPEED_RATINGS,
    tag::DATE_TIME_ORIGINAL,
    tag::LENS_MODEL,
];

#[test]
fn the_tiff_carries_its_structure_and_the_stated_fields_only() {
    let export = settings(Format::Tiff16, Space::DisplayP3);
    let rendered = converted(&frame(64, 40), &export).unwrap();
    let file = encoded(&rendered, &export, &typed()).unwrap();
    // With an Exif directory, ImageIO also states the picture's size in it.
    let expected: BTreeSet<u16> = STRUCTURE.into_iter().chain(STATED).chain([40962, 40963]).collect();
    assert_eq!(tags(&file), expected);
    assert_eq!(text(&file, exif::Tag::Model).as_deref(), Some("Leica M6"));
    assert_eq!(text(&file, exif::Tag::LensModel).as_deref(), Some("Summicron 50"));
    assert_eq!(text(&file, exif::Tag::ImageDescription).as_deref(), Some("Kodak Portra 400"));
    assert_eq!(text(&file, exif::Tag::Copyright).as_deref(), Some("© A. Photographer"));
    assert_eq!(text(&file, exif::Tag::DateTimeOriginal).as_deref(), Some("2001:02:03 04:05:06"));
    assert_eq!(text(&file, exif::Tag::PhotographicSensitivity).as_deref(), Some("400"));
    // The image survives the extra directory.
    let mut d = read_tiff(&file);
    assert!(matches!(d.read_image().unwrap(), tiff::decoder::DecodingResult::U16(_)));

    // Twin: nothing stated, no date, and only the structure is left.
    let bare = encoded(&rendered, &export, &ExifMetadata::default()).unwrap();
    assert_eq!(tags(&bare), STRUCTURE.into_iter().collect());
}

#[test]
fn the_jpeg_carries_the_stated_fields_only_and_its_profile() {
    let export = settings(Format::Jpeg, Space::DisplayP3);
    let rendered = converted(&frame(64, 40), &export).unwrap();
    let file = encoded(&rendered, &export, &typed()).unwrap();
    assert_eq!(tags(&file), STATED.into_iter().collect());
    assert_eq!(text(&file, exif::Tag::Artist).as_deref(), Some("A. Photographer"));
    assert_eq!(text(&file, exif::Tag::DateTime).as_deref(), Some("2001:02:03 04:05:06"));
    assert_eq!(jpeg_profile(&file), Space::DisplayP3.icc_data());
    // Twin: nothing stated writes no Exif segment at all.
    let bare = encoded(&rendered, &export, &ExifMetadata::default()).unwrap();
    assert!(tags(&bare).is_empty());
    assert!(!bare.windows(6).any(|w| w == b"Exif\0\0"));
    assert!(file.windows(6).any(|w| w == b"Exif\0\0"));
}

#[test]
fn an_empty_field_writes_no_tag() {
    let export = settings(Format::Jpeg, Space::Srgb);
    let rendered = converted(&frame(16, 16), &export).unwrap();
    let lens_only = ExifMetadata { lens: "Summicron 50".into(), ..Default::default() };
    let file = encoded(&rendered, &export, &lens_only).unwrap();
    assert_eq!(tags(&file), [tag::LENS_MODEL].into_iter().collect());
    for (typed, expected) in [("ISO 400", Some(400)), ("", None), ("0", None), ("١٢", None), ("100000", Some(100000))]
    {
        assert_eq!(oni_export::metadata::iso(typed), expected, "{typed:?}");
    }
    // A rating past a SHORT is written as Exif's ceiling.
    let fast = ExifMetadata { iso: "100000".into(), ..Default::default() };
    let file = encoded(&rendered, &export, &fast).unwrap();
    assert_eq!(text(&file, exif::Tag::PhotographicSensitivity).as_deref(), Some("65535"));
}

#[test]
fn the_jpeg_quality_is_honoured() {
    let image = frame(256, 160);
    let mut export = settings(Format::Jpeg, Space::Srgb);
    let rendered = converted(&image, &export).unwrap();
    let Quantised::Eight(source) = &rendered.pixels else { panic!() };
    let error = |file: &[u8]| {
        let mut d = jpeg_decoder::Decoder::new(Cursor::new(file));
        let rgb = d.decode().unwrap();
        let sum: f64 =
            rgb.iter().zip(source.chunks(4).flat_map(|p| &p[..3])).map(|(a, b)| (*a as f64 - *b as f64).abs()).sum();
        sum / rgb.len() as f64
    };
    export.jpeg_quality = 0.85;
    let good = encoded(&rendered, &export, &ExifMetadata::default()).unwrap();
    export.jpeg_quality = 0.1;
    let poor = encoded(&rendered, &export, &ExifMetadata::default()).unwrap();
    let (good_error, poor_error) = (error(&good), error(&poor));
    assert!(good_error < 1.5, "0.85 mean error {good_error}");
    // Twin: at 0.1 the same reader sees the loss.
    assert!(poor_error > 2.0 * good_error && poor.len() < good.len(), "0.1 mean error {poor_error}");
    assert_eq!(oni_export::jpeg::quality(0.85), 85);
    assert_eq!(oni_export::jpeg::quality(0.0), 1);
}

#[test]
fn the_estimate_weighs_the_same_bytes_as_the_file() {
    let image = frame(500, 333);
    for export in [settings(Format::Jpeg, Space::Srgb), settings(Format::Tiff16, Space::AdobeRgb)] {
        let rendered = converted(&image, &export).unwrap();
        let file = encoded(&rendered, &export, &ExifMetadata::default()).unwrap();
        let pixels = (500 * 333) as f64;
        assert_eq!(estimate(&image, pixels, &export), file.len() as u64);
        // Scaled by area: four times the pixels, four times the weight.
        assert_eq!(estimate(&image, 4.0 * pixels, &export), 4 * file.len() as u64);
    }
    let empty = Image { width: 0, height: 0, pixels: vec![] };
    assert_eq!(estimate(&empty, 1e6, &Settings::default()), 0);
}

// MARK: - The gain map container

fn base_and_container() -> (Vec<u8>, Vec<u8>) {
    let mut export = settings(Format::Jpeg, Space::Srgb);
    export.jpeg_quality = 0.92;
    let rendered = converted(&frame(400, 300), &export).unwrap();
    let base = encoded(&rendered, &export, &typed()).unwrap();
    export.hdr = true;
    let built = encoded(&rendered, &export, &typed()).unwrap();
    (base, built)
}

#[test]
fn the_container_says_two_stops_in_both_families() {
    let (base, built) = base_and_container();
    assert_eq!(gain_map::file(&base, 400, 300).unwrap(), built, "the switch appends to the very base");
    let xml = String::from_utf8_lossy(&built);
    for needle in [
        "hdrgm:Version=\"1.0\"",
        "hdrgm:GainMapMax=\"2.000000\"",
        "hdrgm:HDRCapacityMax=\"2.000000\"",
        "hdrgm:BaseRenditionIsHDR=\"False\"",
        "Item:Semantic=\"GainMap\"",
        "urn:com:apple:photo:2020:aux:hdrgainmap",
        "hdrgm:AlternateHeadroom=\"2.000000\"",
        "MPF\0",
    ] {
        assert!(xml.contains(needle), "{needle} missing");
    }
    assert_eq!(gain_map::scan(&built), gain_map::scan(&base), "the base's scan is byte-identical");
    // The base's own fields and profile still read in the container.
    assert_eq!(text(&built, exif::Tag::Model), Some("Leica M6".into()));
    assert_eq!(jpeg_profile(&built), Space::Srgb.icc_data());

    let map = gain_map::second_image(&built).expect("a second JPEG follows the first");
    let mut d = jpeg_decoder::Decoder::new(Cursor::new(map));
    let grey = d.decode().unwrap();
    let info = d.info().unwrap();
    assert_eq!((info.width, info.height, info.pixel_format), (200, 150, jpeg_decoder::PixelFormat::L8));
    assert!(grey.iter().all(|v| *v == 255), "every pixel takes the full lift");
    // The map is a JPEG of its own, and the rule holds in it: no camera data.
    assert!(tags(map).is_empty());

    // An FF D8 planted in a comment ahead of the first scan is not taken for the map.
    let mut planted = built[..2].to_vec();
    planted.extend([0xFF, 0xFE, 0x00, 0x04, 0xFF, 0xD8]);
    planted.extend(&built[2..]);
    assert_eq!(gain_map::second_image(&planted), Some(map));
    assert_eq!(planted[2..].windows(2).position(|w| w == [0xFF, 0xD8]), Some(4));
    // Twin: a file of one image has no second.
    assert_eq!(gain_map::second_image(&base), None);
}

#[test]
fn an_independent_reader_finds_the_map_through_the_mpf_index() {
    use ultrahdr_rs::container::{extract_secondary_images, parse_mpf_segment, scan_segments};
    let (_, built) = base_and_container();
    let map = gain_map::second_image(&built).unwrap();
    let segments = scan_segments(&built);
    let mpf = segments.iter().find(|s| s.is_mpf()).expect("an MPF segment");
    let directory = parse_mpf_segment(&mpf.data, mpf.offset).unwrap();
    assert_eq!(directory.entries.len(), 2);
    assert_eq!(directory.entries[0].size as usize, built.len() - map.len(), "the primary's size includes the index");
    assert_eq!(extract_secondary_images(&built, &directory), vec![map]);

    // Twin: the map's offset counted from the marker instead of the MP header misses its SOI.
    let field = mpf.offset + 8 + 50 + 16 + 8;
    let mut shifted = built.clone();
    let offset = u32::from_be_bytes(shifted[field..field + 4].try_into().unwrap()) + 8;
    shifted[field..field + 4].copy_from_slice(&offset.to_be_bytes());
    let moved =
        parse_mpf_segment(&scan_segments(&shifted).iter().find(|s| s.is_mpf()).unwrap().data, mpf.offset).unwrap();
    let found = extract_secondary_images(&shifted, &moved);
    assert!(found.first().is_none_or(|m| !m.starts_with(&[0xFF, 0xD8])));
}

#[test]
fn an_independent_reader_lifts_every_pixel_by_four() {
    use ultrahdr_rs::gainmap::apply::GainMapLut;
    let (base, built) = base_and_container();
    let decoder = ultrahdr_rs::Decoder::new(&built).unwrap();
    assert!(decoder.is_ultrahdr());
    assert_eq!(decoder.gainmap_jpeg(), gain_map::second_image(&built));
    let metadata = decoder.metadata().expect("the map's hdrgm fields are read").clone();
    assert_eq!(metadata.gain_map_max, [2.0; 3]);
    assert_eq!(metadata.base_offset, [0.0; 3], "offsets 0, so black stays black on a flat map");
    assert_eq!(metadata.gamma, [1.0; 3]);
    let lift = GainMapLut::new(&metadata, 1.0).lookup(255, 0);
    assert!((lift - gain_map::LIFT).abs() < 1e-4, "lift {lift}");

    // Twin: the same map declaring zero stops in every field, same byte count, lifts nothing.
    let flat = zero_stops(&built);
    assert_eq!(flat.len(), built.len());
    let none = ultrahdr_rs::Decoder::new(&flat).unwrap().metadata().cloned().unwrap_or_default();
    assert_eq!(GainMapLut::new(&none, 1.0).lookup(255, 0), 1.0);
    // Twin: the file written with the switch off is no Ultra HDR at all.
    assert!(!ultrahdr_rs::Decoder::new(&base).unwrap().is_ultrahdr());
}

/// The file with every `"2.000000"` replaced byte for byte, outside any decoding of the scans.
fn zero_stops(original: &[u8]) -> Vec<u8> {
    let (from, to) = (b"\"2.000000\"", b"\"0.000000\"");
    let mut out = original.to_vec();
    let mut i = 0;
    while i + from.len() <= out.len() {
        if &out[i..i + from.len()] == from {
            out[i..i + from.len()].copy_from_slice(to);
            i += from.len();
        } else {
            i += 1;
        }
    }
    out
}
