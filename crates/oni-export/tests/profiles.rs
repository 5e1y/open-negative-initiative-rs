//! A file's profile: every format declares its space's, and the pixels read back through it by an
//! independent colour engine land where they were, each against its twin.

use oni_colour::Space;
use oni_export::{ExifMetadata, Format, Image, converted, encoded};

mod common;
use common::{frame, jpeg_profile, read_tiff, settings};

/// Where the stand-in profile `name` lies; a file of its own, as a test rewriting another's reads
/// it empty halfway through and the space as gone.
fn stand_in(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("oni-export-profile-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(format!("{name}.icc"))
}

/// A profile on disk standing in for a paper's: ProPhoto's own bytes, a matrix-shaper profile.
fn profile_space(name: &str) -> Space {
    let path = stand_in(name);
    std::fs::write(&path, Space::ProPhoto.icc_data().unwrap()).unwrap();
    Space::Profile(oni_colour::OutputProfile::new(path.to_string_lossy()))
}

#[test]
fn a_stand_in_profile_rewritten_leaves_every_other_whole() {
    let kept = profile_space("kept");
    let rewritten = profile_space("rewritten");
    // What a write does first: the file emptied before its bytes land.
    std::fs::write(stand_in("rewritten"), []).unwrap();
    assert!(kept.icc_data().is_some(), "another test's stand-in is never this one's file");
    // Twin: the stand-in caught halfway through its own rewrite reads as gone.
    assert!(rewritten.icc_data().is_none());
}

#[test]
fn every_format_declares_the_profile_of_its_space() {
    let image = frame(16, 16);
    let variants = [(Format::Jpeg, false), (Format::Jpeg, true), (Format::Tiff16, false)];
    let mut p3_jpeg = None;
    let profile = profile_space("declared");
    assert!(profile.icc_data().is_some(), "the stand-in profile loads");
    for space in Space::BUILT_IN.into_iter().chain([profile]) {
        for (format, hdr) in variants {
            let mut export = settings(format, space.clone());
            if export.format() != format {
                // ProPhoto is TIFF only; its JPEG is not a state the settings reach.
                continue;
            }
            export.hdr = hdr;
            let rendered = converted(&image, &export).unwrap();
            let file = encoded(&rendered, &export, &ExifMetadata::default()).unwrap();
            let declared = match format {
                Format::Jpeg => jpeg_profile(&file),
                Format::Tiff16 => read_tiff(&file).get_tag_u8_vec(tiff::tags::Tag::IccProfile).ok(),
            };
            assert_eq!(declared, space.icc_data(), "{format:?} hdr {hdr} in {}", space.title());
            if space == Space::DisplayP3 && format == Format::Jpeg && !hdr {
                p3_jpeg = Some(file);
            }
        }
    }
    let p3 = p3_jpeg.unwrap();
    // Twin: the same bytes with the profile segments cut out read as another space.
    assert_ne!(jpeg_profile(&stripping_profile(&p3)), Space::DisplayP3.icc_data());
    assert_ne!(jpeg_profile(&p3), Space::Srgb.icc_data());
}

/// The written TIFF's pixels read back through its own embedded profile by lcms2, an independent
/// colour engine, into linear Rec. 2020: the largest gap to `colour` (alpha 1).
fn read_back_gap(space: Space, colour: [f32; 3], read_as: Option<&[u8]>) -> f64 {
    let image = Image { width: 4, height: 4, pixels: [colour[0], colour[1], colour[2], 1.0].repeat(16) };
    let export = settings(Format::Tiff16, space);
    let file = encoded(&converted(&image, &export).unwrap(), &export, &ExifMetadata::default()).unwrap();
    let mut d = read_tiff(&file);
    let embedded = d.get_tag_u8_vec(tiff::tags::Tag::IccProfile).unwrap();
    let tiff::decoder::DecodingResult::U16(samples) = d.read_image().unwrap() else { panic!("16-bit samples") };
    let rgb: Vec<[u16; 3]> = samples.chunks(4).map(|p| [p[0], p[1], p[2]]).collect();
    let source = lcms2::Profile::new_icc(read_as.unwrap_or(&embedded)).unwrap();
    let xy = |x, y| lcms2::CIExyY { x, y, Y: 1.0 };
    let primaries = lcms2::CIExyYTRIPLE { Red: xy(0.708, 0.292), Green: xy(0.170, 0.797), Blue: xy(0.131, 0.046) };
    let linear = lcms2::ToneCurve::new(1.0);
    let working = lcms2::Profile::new_rgb(&xy(0.3127, 0.3290), &primaries, &[&linear, &linear, &linear]).unwrap();
    let to_working: lcms2::Transform<[u16; 3], [f64; 3]> = lcms2::Transform::new(
        &source,
        lcms2::PixelFormat::RGB_16,
        &working,
        lcms2::PixelFormat::RGB_DBL,
        lcms2::Intent::RelativeColorimetric,
    )
    .unwrap();
    let mut read = vec![[0.0f64; 3]; rgb.len()];
    to_working.transform_pixels(&rgb, &mut read);
    read.iter().flat_map(|p| (0..3).map(move |c| (p[c] - colour[c] as f64).abs())).fold(0.0, f64::max)
}

#[test]
fn a_wide_green_survives_prophoto_and_clips_in_adobe_rgb() {
    let green = [0.01, 0.6, 0.01];
    let pro_photo = read_back_gap(Space::ProPhoto, green, None);
    assert!(pro_photo < 2e-3, "ProPhoto TIFF gap {pro_photo}");
    let through_profile = read_back_gap(profile_space("green"), green, None);
    assert!(through_profile < 2e-3, "ProPhoto as an ICC profile, gap {through_profile}");
    // A grey sits in every space: Adobe RGB and sRGB hold it, so their clip below is the green's.
    for space in [Space::AdobeRgb, Space::Srgb, Space::DisplayP3] {
        let grey = read_back_gap(space.clone(), [0.18, 0.18, 0.18], None);
        assert!(grey < 2e-3, "{} grey gap {grey}", space.title());
    }
    // Twins: Adobe RGB clips the green, and the ProPhoto pixels read as sRGB land elsewhere.
    let adobe = read_back_gap(Space::AdobeRgb, green, None);
    assert!(adobe > 0.01, "Adobe RGB gap {adobe}");
    let misread = read_back_gap(Space::ProPhoto, green, Some(&Space::Srgb.icc_data().unwrap()));
    assert!(misread > 0.05, "ProPhoto read as sRGB, gap {misread}");
    println!("gaps: ProPhoto {pro_photo:.2e}, profile {through_profile:.2e}, Adobe {adobe:.4}, misread {misread:.4}");
}

/// A JPEG with every APP2 `ICC_PROFILE` segment cut out, the file a stripping re-encode leaves.
fn stripping_profile(jpeg: &[u8]) -> Vec<u8> {
    let mut out = vec![0xFF, 0xD8];
    let mut i = 2;
    while i + 4 <= jpeg.len() && jpeg[i] == 0xFF && jpeg[i + 1] != 0xDA {
        let length = (jpeg[i + 2] as usize) << 8 | jpeg[i + 3] as usize;
        let end = (i + 2 + length).min(jpeg.len());
        let icc = jpeg[i + 1] == 0xE2 && end - i > 15 && &jpeg[i + 4..i + 15] == b"ICC_PROFILE";
        if !icc {
            out.extend(&jpeg[i..end]);
        }
        i = end;
    }
    out.extend(&jpeg[i..]);
    out
}
