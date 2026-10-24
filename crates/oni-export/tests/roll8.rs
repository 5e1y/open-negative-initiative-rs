//! Roll #8, the reference roll, and the Lumix calibration files: the scan's own EXIF read for the
//! box, frames written and read back, the source never overwritten, and the size estimate measured.

use std::collections::BTreeSet;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::Instant;

use oni_colour::{PaperCorrection, Space};
use oni_export::{
    Failure, Format, Image, ExifMetadata, Resize, Settings, capture_date, converted, encoded, estimate, gain_map, resized,
    scan_metadata, write,
};
use oni_render::{Stage47, cpu};

fn files(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{} not found: {e}", dir.display()))
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case(extension)))
        .collect();
    out.sort();
    assert!(!out.is_empty(), "no {extension} in {}", dir.display());
    out
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oni-export-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A destination where nothing is replaced: any file found there is refused.
fn fresh(path: &Path) -> oni_export::Landing<'_> {
    oni_export::Landing { path, ledger: Box::leak(Box::new(oni_export::file::Ledger::none())), replacing: false }
}

/// Two decodes at once corrupt each other ("data corrupted at …"): the tests take turns.
static DECODING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The decode at full size, as the export reads it.
fn decoded(path: &Path) -> Image {
    let _turn = DECODING.lock().unwrap_or_else(|e| e.into_inner());
    let linear = oni_decode::decode(path, 1.0).unwrap_or_else(|e| panic!("{} does not decode: {e:?}", path.display()));
    Image { width: linear.width, height: linear.height, pixels: linear.pixels.into_iter().flatten().collect() }
}

/// The frame's own stage-4 gains and stage-6 levels from its sidecar, on a resting print: an
/// approximation of the photograph as set on the roll, per pixel, not the whole pipeline.
fn as_set(path: &Path, mut image: Image) -> Image {
    let sidecar = PathBuf::from(format!("{}.json", path.display()));
    let doc: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&sidecar).unwrap()).unwrap();
    let s = &doc["settings"];
    let number = |v: &serde_json::Value| v.as_f64().unwrap() as f32;
    let capture = number(&s["captureStops"]);
    let stops = &s["gains"]["stops"];
    let levels = |c: &str| [number(&s["levels"][c]["black"]), number(&s["levels"][c]["white"]), 1.0];
    let k = Stage47 {
        gain: [0, 1, 2].map(|c| (number(&stops[c]) + capture).exp2()),
        red: levels("red"),
        green: levels("green"),
        blue: levels("blue"),
        print_on: 1.0,
        ..Stage47::rest()
    };
    for p in image.pixels.chunks_exact_mut(4) {
        p.copy_from_slice(&cpu::stage47([p[0], p[1], p[2], p[3]], &k));
    }
    image
}

fn is_exif_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 19
        && b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b':',
            10 => *c == b' ',
            13 | 16 => *c == b':',
            _ => c.is_ascii_digit(),
        })
}

fn tags(file: &[u8]) -> BTreeSet<u16> {
    let mut reader = exif::Reader::new();
    reader.continue_on_error(true);
    let mut bytes = file.to_vec();
    if bytes.starts_with(b"IIU\0") {
        bytes[2] = 0x2A;
    }
    let found = if bytes.starts_with(b"MM\0*") || bytes.starts_with(b"II*\0") {
        reader.read_raw(bytes)
    } else {
        reader.read_from_container(&mut Cursor::new(bytes))
    };
    let found = match found {
        Ok(f) => f,
        Err(exif::Error::PartialResult(p)) => p.into_inner().0,
        Err(_) => return BTreeSet::new(),
    };
    found.fields().map(|f| f.tag.number()).collect()
}

/// Fields that describe the scan, not the film photograph: Make, Model, serials, lens, exposure.
const FORBIDDEN: [u16; 9] = [271, 272, 0xA431, 0xA434, 0xA433, 0x829D, 0x8827, 0x829A, 0x920A];

#[test]
fn the_scans_own_exif_is_read_for_the_box() {
    let Some(roll) = oni_fixtures::raw_dir("the_scans_own_exif_is_read_for_the_box") else { return };
    let Some(lumix) = oni_fixtures::lumix_dir("the_scans_own_exif_is_read_for_the_box") else { return };
    for path in files(&roll, "nef") {
        let scan = scan_metadata(&path);
        println!("{}: {scan:?}", path.file_name().unwrap().to_string_lossy());
        // The camera, joined from Make and Model as the Swift app wrote it in this roll's sidecar.
        assert_eq!(scan.camera, "NIKON CORPORATION NIKON Z f");
        assert!(!scan.lens.is_empty() && scan.iso.parse::<u32>().is_ok_and(|v| v > 0), "{scan:?}");
        assert!(is_exif_date(&scan.date), "{:?}", scan.date);
        assert_eq!(capture_date(&path).as_deref(), Some(scan.date.as_str()));
    }
    // A frame of the same session as DSC_6442, whose sidecar holds the box the Swift app read.
    let sidecar: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(roll.join("DSC_6442.NEF.json")).unwrap()).unwrap();
    let neighbour = scan_metadata(&roll.join("DSC_6440.NEF"));
    assert_eq!(sidecar["exif"]["camera"], neighbour.camera.as_str());
    assert_eq!(sidecar["exif"]["lens"], neighbour.lens.as_str());
    assert_eq!(sidecar["exif"]["iso"], neighbour.iso.as_str());

    for path in files(&lumix, "rw2") {
        let scan = scan_metadata(&path);
        println!("{}: {scan:?}", path.file_name().unwrap().to_string_lossy());
        assert_eq!(scan.camera, "Panasonic DC-S1R");
        assert!(is_exif_date(&scan.date), "{:?}", scan.date);
        assert_eq!(capture_date(&path).as_deref(), Some(scan.date.as_str()));
        // The ISO comes from the embedded preview; twin: the RAW's own directories carry none.
        assert_eq!(scan.iso, "100");
        assert!(!tags(&std::fs::read(&path).unwrap()).contains(&0x8827), "the RW2's own IFDs name an ISO");
    }
    // Twin: a file with no Exif reads an empty box and no date.
    let bare = scratch("bare").join("bare.tiff");
    let export = Settings::default();
    let rendered = converted(&Image { width: 2, height: 2, pixels: vec![0.5; 16] }, &export).unwrap();
    std::fs::write(&bare, encoded(&rendered, &export, None, &ExifMetadata::default()).unwrap()).unwrap();
    assert!(scan_metadata(&bare).is_empty());
    assert_eq!(capture_date(&bare), None);
}

#[test]
fn a_frame_exports_with_its_date_and_none_of_the_scans_fields() {
    let Some(roll) = oni_fixtures::raw_dir("a_frame_exports_with_its_date_and_none_of_the_scans_fields") else { return };
    let source = roll.join("DSC_6432.NEF");
    let dir = scratch("frame");
    let full = as_set(&source, decoded(&source));
    let date = capture_date(&source).expect("the frame has a capture date");
    // Twin first: the same reader sees the scan's own camera fields in the NEF itself.
    let scan_tags = tags(&std::fs::read(&source).unwrap());
    let leaking: Vec<u16> = FORBIDDEN.iter().copied().filter(|t| scan_tags.contains(t)).collect();
    assert!(leaking.contains(&271) && leaking.contains(&272) && leaking.len() >= 5, "the NEF carries {leaking:?}");

    let mut tiff = Settings::default();
    tiff.set_format(Format::Tiff16);
    tiff.set_space(Space::AdobeRgb);
    tiff.resize = Resize::LongestEdge(2000);
    let mut jpeg = Settings::default();
    jpeg.resize = Resize::LongestEdge(2000);
    let mut hdr = jpeg.clone();
    hdr.hdr = true;
    let expected_size = Resize::LongestEdge(2000).output_size(full.width as f64, full.height as f64);
    let mut files = Vec::new();
    for (name, export) in [("written.tiff", &tiff), ("written.jpg", &jpeg), ("written-hdr.jpg", &hdr)] {
        let out = dir.join(name);
        let started = Instant::now();
        write(&source, fresh(&out), || Ok(full.clone()), &PaperCorrection::default(), export, &ExifMetadata::default(), None)
            .unwrap();
        let file = std::fs::read(&out).unwrap();
        println!("{name}: {} bytes in {:.2} s", file.len(), started.elapsed().as_secs_f64());
        let found = tags(&file);
        assert!(FORBIDDEN.iter().all(|t| !found.contains(t)), "{name} leaks {found:?}");
        let stated: BTreeSet<u16> = [306, 36867].into_iter().collect();
        assert!(stated.is_subset(&found), "{name} lost the capture date: {found:?}");
        let mut reader = exif::Reader::new();
        let read = if name.ends_with("tiff") {
            reader.continue_on_error(true).read_raw(file.clone())
        } else {
            reader.read_from_container(&mut Cursor::new(&file))
        }
        .unwrap();
        let written = read.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY).unwrap();
        assert_eq!(written.display_value().to_string(), date.replacen(':', "-", 2));
        let (w, h) = if name.ends_with("tiff") {
            let mut d = tiff::decoder::Decoder::new(Cursor::new(&file)).unwrap();
            assert_eq!(d.get_tag_u32(tiff::tags::Tag::Compression).unwrap(), 8);
            d.dimensions().unwrap()
        } else {
            let mut d = jpeg_decoder::Decoder::new(Cursor::new(&file));
            d.decode().unwrap();
            let info = d.info().unwrap();
            (info.width as u32, info.height as u32)
        };
        assert_eq!((w as f64, h as f64), expected_size, "{name} measures what the sheet announced");
        files.push(file);
    }
    // The gain map's base is the file the switch off writes, to the byte of its scan.
    assert_eq!(gain_map::scan(&files[2]), gain_map::scan(&files[1]));
    assert!(gain_map::second_image(&files[2]).is_some() && gain_map::second_image(&files[1]).is_none());
    let lift = ultrahdr_rs::gainmap::apply::GainMapLut::new(
        ultrahdr_rs::Decoder::new(&files[2]).unwrap().metadata().unwrap(),
        1.0,
    )
    .lookup(255, 0);
    assert!((lift - 4.0).abs() < 1e-4, "lift {lift}");

    // A typed date replaces the capture date, and the hand's fields land.
    let typed = ExifMetadata { camera: "Leica M6".into(), film: "Kodak Gold 200".into(), ..Default::default() };
    let out = dir.join("dated.jpg");
    write(&source, fresh(&out), || Ok(full.clone()), &PaperCorrection::default(), &jpeg, &typed, Some("2001:02:03 04:05:06"))
        .unwrap();
    let file = std::fs::read(&out).unwrap();
    let read = exif::Reader::new().read_from_container(&mut Cursor::new(&file)).unwrap();
    let field = |t| read.get_field(t, exif::In::PRIMARY).map(|f| f.display_value().to_string());
    assert_eq!(field(exif::Tag::DateTimeOriginal).as_deref(), Some("2001-02-03 04:05:06"));
    assert_eq!(field(exif::Tag::Model).as_deref(), Some("\"Leica M6\""));
    assert_eq!(field(exif::Tag::ImageDescription).as_deref(), Some("\"Kodak Gold 200\""));
    assert_eq!(field(exif::Tag::Make), None, "the scan's Make stays out even with the box filled");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_source_is_never_overwritten() {
    let Some(roll) = oni_fixtures::raw_dir("the_source_is_never_overwritten") else { return };
    let source = roll.join("DSC_6436.NEF");
    let dir = scratch("overwrite");
    let spelt = roll.join(".").join("DSC_6436.NEF");
    let mut destinations = vec![source.clone(), spelt];
    // A symbolic link needs no privilege on Unix alone.
    #[cfg(unix)]
    {
        let link = dir.join("link.NEF");
        std::os::unix::fs::symlink(&source, &link).ok();
        destinations.push(link);
    }
    for destination in destinations {
        let mut rendered = false;
        let result = write(
            &source,
            fresh(&destination),
            || {
                rendered = true;
                Err(Failure::Render)
            },
            &PaperCorrection::default(),
            &Settings::default(),
            &ExifMetadata::default(),
            None,
        );
        assert_eq!(result, Err(Failure::WouldOverwriteOriginal), "{}", destination.display());
        assert!(!rendered, "refused before anything renders");
    }
    // Twin: another destination goes as far as the render.
    let mut rendered = false;
    let result = write(
        &source,
        fresh(&dir.join("DSC_6436.jpg")),
        || {
            rendered = true;
            Err(Failure::Render)
        },
        &PaperCorrection::default(),
        &Settings::default(),
        &ExifMetadata::default(),
        None,
    );
    assert_eq!(result, Err(Failure::Render));
    assert!(rendered);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_estimate_holds_on_roll_8() {
    let Some(roll) = oni_fixtures::raw_dir("the_estimate_holds_on_roll_8") else { return };
    let mut jpeg = Settings::default();
    jpeg.set_space(Space::Srgb);
    let mut tiff = Settings::default();
    tiff.set_format(Format::Tiff16);
    tiff.set_space(Space::AdobeRgb);
    let frames = files(&roll, "nef");
    let mut worst = [0.0f64; 2];
    let mut side_worst = 0.0f64;
    for path in frames.iter().step_by(3) {
        let full = decoded(path);
        let full_pixels = (full.width * full.height) as f64;
        let scale = oni_export::SAMPLE_SIDE / full.width.max(full.height) as f64;
        // The reduced decode rendered, as the estimate sees it.
        let preview = as_set(path, resized(&full, scale));
        let full = as_set(path, full);
        for (i, export) in [&jpeg, &tiff].into_iter().enumerate() {
            let started = Instant::now();
            let guess = estimate(&preview, full_pixels, export) as f64;
            let guessed_in = started.elapsed().as_secs_f64();
            let started = Instant::now();
            let actual =
                encoded(&converted(&full, export).unwrap(), export, None, &ExifMetadata::default()).unwrap().len() as f64;
            let error = guess / actual - 1.0;
            println!(
                "{} {:?}: estimated {:.1} MB in {guessed_in:.2} s, written {:.1} MB in {:.2} s, error {:+.1} %",
                path.file_name().unwrap().to_string_lossy(),
                export.format(),
                guess / 1e6,
                actual / 1e6,
                started.elapsed().as_secs_f64(),
                100.0 * error
            );
            worst[i] = worst[i].max(error.abs());
            // Twin: extrapolated by the ratio of sides instead of areas.
            let by_side = guess / (full_pixels / (preview.width * preview.height) as f64).sqrt();
            side_worst = side_worst.max((by_side / actual - 1.0).abs());
        }
    }
    println!("worst error: JPEG {:.1} %, TIFF ZIP {:.1} %", 100.0 * worst[0], 100.0 * worst[1]);
    // ZIP weighs the grain alike at every size. A JPEG's bytes per pixel climb as the frame
    // shrinks, so without sharpening its figure is printed, not held.
    assert!(worst[1] < 0.03, "TIFF ZIP estimate off by {:.1} %", 100.0 * worst[1]);
    assert!(side_worst > 0.5, "the twin extrapolation is off by only {side_worst}");
}
