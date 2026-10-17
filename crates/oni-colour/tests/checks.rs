//! The app's own self-checks for the proof, Paper Correction, the profiles and the zebra, each
//! doubled by a twin that fails by construction when the code is wrong.

mod common;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use moxcms::{ColorProfile, ProfileClass};
use oni_colour::legs::{self, BAND};
use oni_colour::{Display, InstallRecord, OutputProfile, PaperCorrection, Proof, Quantised, Space, gamut, paper};
use url::Url;

fn patch(r: f32, g: f32, b: f32) -> Vec<f32> {
    [r, g, b, 1.0].repeat(4)
}

fn first(px: &[f32]) -> [f32; 3] {
    [px[0], px[1], px[2]]
}

fn far(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>().sqrt()
}

fn spread(v: [f32; 3]) -> f32 {
    v.iter().cloned().fold(f32::MIN, f32::max) / v.iter().cloned().fold(f32::MAX, f32::min) - 1.0
}

fn write_matrix(dir: &Path, name: &str, mut profile: ColorProfile, class: ProfileClass) -> PathBuf {
    profile.profile_class = class;
    let path = dir.join(name);
    fs::write(&path, profile.encode().unwrap()).unwrap();
    path
}

fn proofed(proof: &Proof, px: &[f32]) -> [f32; 3] {
    first(&proof.proofed(px).expect("the proof reads its profile"))
}

// MARK: - The proof

#[test]
fn the_proof_clips_what_the_export_space_cannot_hold() {
    let dir = common::sandbox("proof");
    let wide = patch(0.02, 0.6, 0.1);
    let grey = patch(0.18, 0.18, 0.18);
    let srgb = Proof::new(Space::Srgb);
    assert!(far(proofed(&srgb, &grey), first(&grey)) < 1e-4, "proofing sRGB leaves a grey where it was");
    assert!(far(proofed(&srgb, &wide), first(&wide)) > 0.01, "and moves a Rec. 2020 green the sRGB file clips");
    // Twin: a profile as wide as the working space clips nothing.
    let full = write_matrix(&dir, "ITU-2020.icc", ColorProfile::new_bt2020(), ProfileClass::DisplayDevice);
    let full = Proof::new(Space::Profile(OutputProfile::new(full.to_string_lossy())));
    assert!(far(proofed(&full, &wide), first(&wide)) < 0.01, "through a Rec. 2020 profile the same green holds");

    let file = write_matrix(&dir, "sRGB Profile.icc", ColorProfile::new_srgb(), ProfileClass::DisplayDevice);
    let cubed = Proof::new(Space::Profile(OutputProfile::new(file.to_string_lossy())));
    let gap = far(proofed(&cubed, &wide), proofed(&srgb, &wide)).max(far(proofed(&cubed, &grey), proofed(&srgb, &grey)));
    assert!(gap < 0.01, "a profile's cube lands on the matrix proof of the same space ({gap})");
}

#[test]
fn a_screen_shows_less_than_a_wider_space() {
    let (srgb, p3) = (ColorProfile::new_srgb(), ColorProfile::new_display_p3());
    assert!(Proof::shows_less(&srgb, &Space::DisplayP3));
    // Twin: the same space on a P3 screen, and a narrower one on sRGB.
    assert!(!Proof::shows_less(&p3, &Space::DisplayP3) && !Proof::shows_less(&srgb, &Space::Srgb));
    assert!(Proof::shows_less(&p3, &Space::AdobeRgb), "even a P3 screen shows less than Adobe RGB's greens");
}

#[test]
fn colour_only_drops_the_paper_black_to_the_screen_s() {
    let dir = common::sandbox("colour-only");
    let file = dir.join("Lustre.icc");
    common::write_stand_in(&file, "Lustre", 0.03, None, false);
    let space = Space::Profile(OutputProfile::new(file.to_string_lossy()));
    let only = Proof { colour_only: true, ..Proof::new(space.clone()) };
    let max = |v: [f32; 3]| v.iter().cloned().fold(f32::MIN, f32::max);
    let shown_black = max(proofed(&only, &patch(0.0, 0.0, 0.0)));
    let shown_dark = max(proofed(&only, &patch(0.01, 0.01, 0.01)));
    let printed = max(proofed(&Proof::new(space), &patch(0.0, 0.0, 0.0)));
    assert!(shown_black < printed / 3.0 && shown_dark > shown_black, "{shown_black} {shown_dark} {printed}");
    // Twin: the proof at rest reads the black back as printed.
    assert!(printed > 0.02, "at rest the proof shows the paper's own black, {printed}");
}

#[test]
fn a_bluish_paper_shows_its_white_as_printed() {
    let dir = common::sandbox("paper-white");
    let weights = [0.2627f32, 0.6780, 0.0593];
    let simulated = |path: &Path| {
        let space = Space::Profile(OutputProfile::new(path.to_string_lossy()));
        let white = patch(1.0, 1.0, 1.0);
        let shown = proofed(&Proof::new(space.clone()), &white);
        let only = proofed(&Proof { colour_only: true, ..Proof::new(space) }, &white);
        let luma = |v: [f32; 3]| (0..3).map(|i| v[i] * weights[i]).sum::<f32>();
        (luma(shown) / luma(only), (shown[2] / shown[0]) / (only[2] / only[0]))
    };
    let bluish = common::bluish();
    let pearl = dir.join("Pearl.icc");
    common::write_stand_in(&pearl, "Pearl", 0.02, Some(bluish), false);
    let (light, blue) = simulated(&pearl);
    assert!((light - bluish[1] as f32).abs() < 0.02 && blue > 1.05, "{light} for {}, blue ×{blue}", bluish[1]);
    // Twin: a paper whose white is the PCS's own keeps its white.
    let plain = dir.join("Plain.icc");
    common::write_stand_in(&plain, "Plain", 0.02, None, false);
    let (light, blue) = simulated(&plain);
    assert!((light - 1.0).abs() < 0.01 && (blue - 1.0).abs() < 0.01, "{light} {blue}");
}

#[test]
fn a_v4_lab_paper_reads_its_greys_back_grey() {
    let data = common::stand_in("Lab v4", 0.02, None, true);
    let back = legs::legs_back(&data).unwrap();
    let spread_of = |b: &legs::Back| {
        let greys = [0.25, 0.25, 0.25, 1.0, 0.5, 0.5, 0.5, 1.0, 0.75, 0.75, 0.75, 1.0];
        let g = legs::run_back(b, &greys, None).unwrap();
        g.chunks_exact(4).map(|p| spread([p[0], p[1], p[2]])).fold(0.0, f32::max)
    };
    assert!(spread_of(&back) < 0.005, "channels {} % apart", spread_of(&back) * 100.0);
    // Twin: the half a* and b* a CMM can read on a v4 Lab neutral, left on, parts the channels.
    let off = back.with_offset([0.5, 0.5]);
    assert!(spread_of(&off) > 0.01, "{}", spread_of(&off));
}

// MARK: - Paper Correction

#[test]
fn paper_correction_is_laid_on_a_paper_alone() {
    let dir = common::sandbox("paper-correction");
    let plain = dir.join("Plain.icc");
    common::write_stand_in(&plain, "Plain", 0.02, None, false);
    let paper = Space::Profile(OutputProfile::new(plain.to_string_lossy()));
    let grey = patch(0.2, 0.2, 0.2);
    let blue = |v: [f32; 3]| v[2] / v[0];
    let bluer = PaperCorrection { stops: [0.0, 0.0, -0.5] };
    let through = |space: &Space, c: PaperCorrection| first(&Proof::new(space.clone()).correcting(c).applied(&grey));
    let on_paper = blue(through(&paper, bluer)) / blue(through(&paper, PaperCorrection::default()));
    assert!(on_paper < 0.85, "on a paper the proof lays the gains, blue over red ×{on_paper}");
    // Twin: the same gains in a space naming no paper are inert, to the bit.
    assert_eq!(through(&Space::Srgb, bluer), through(&Space::Srgb, PaperCorrection::default()));

    let mut written = grey.clone();
    oni_colour::paper::laid(&bluer, &mut written, &paper);
    let mut elsewhere = grey.clone();
    oni_colour::paper::laid(&bluer, &mut elsewhere, &Space::DisplayP3);
    assert!((blue(first(&written)) - 0.5f32.sqrt()).abs() < 1e-3 && elsewhere == grey);
}

#[test]
fn the_automatic_key_solves_the_paper_s_white() {
    let dir = common::sandbox("automatic");
    let pearl_file = dir.join("Pearl.icc");
    common::write_stand_in(&pearl_file, "Pearl", 0.02, Some(common::bluish()), false);
    let plain_file = dir.join("Plain.icc");
    common::write_stand_in(&plain_file, "Plain", 0.02, None, false);
    let data = fs::read(&pearl_file).unwrap();
    let (legs, back) = (legs::legs(&data).unwrap(), legs::legs_back(&data).unwrap());
    let pearl = paper::automatic(&OutputProfile::new(pearl_file.to_string_lossy())).unwrap();
    let plain = paper::automatic(&OutputProfile::new(plain_file.to_string_lossy())).unwrap();
    let gain = pearl.gains();
    let spreads = |by: [f32; 3]| -> Vec<f32> {
        [PaperCorrection::PROBE, 0.4, 0.6]
            .iter()
            .map(|g| spread(paper::proofed(by.map(|b| b * g), &legs, &back).unwrap()))
            .collect()
    };
    let (fixed, raw) = (spreads(gain), spreads([1.0; 3]));
    let top = pearl.stops.iter().cloned().fold(f32::MIN, f32::max);
    assert!(top == 0.0 && pearl.stops[2] < -0.1, "{:?}", pearl.stops);
    assert!(fixed[0] < 0.001 && fixed[1] < raw[1] / 5.0 && fixed[2] < raw[2] / 5.0, "{fixed:?} against {raw:?}");
    // Twin: the same greys uncorrected carry the paper's blue.
    assert!(raw.iter().all(|s| *s > 0.1), "{raw:?}");
    let most = plain.stops.iter().map(|s| s.abs()).fold(0.0, f32::max);
    assert!(most < pearl.stops[2].abs() / 10.0, "a D50 paper asks a tenth at most ({most})");
    let screen = write_matrix(&dir, "sRGB.icc", ColorProfile::new_srgb(), ProfileClass::DisplayDevice);
    assert!(paper::automatic(&OutputProfile::new(screen.to_string_lossy())).is_none());
}

// MARK: - Converting a picture

fn gradient(rows: usize) -> impl FnMut(usize, usize, &mut [f32]) {
    let (c0, c1) = ([0.9f32, 0.02, 0.1], [0.02f32, 0.6, 0.95]);
    move |top, n, out| {
        for r in 0..n {
            let t = (top + r) as f32 / (rows - 1) as f32;
            for x in 0..8 {
                let i = (r * 8 + x) * 4;
                for c in 0..3 {
                    out[i + c] = c0[c] + (c1[c] - c0[c]) * t;
                }
                out[i + 3] = 1.0;
            }
        }
    }
}

#[test]
fn through_the_srgb_file_the_export_lands_on_the_srgb_space_band_after_band() {
    let dir = common::sandbox("converted");
    let tall = 2 * BAND + 88;
    let srgb = OutputProfile::new(write_matrix(&dir, "sRGB.icc", ColorProfile::new_srgb(), ProfileClass::DisplayDevice).to_string_lossy());
    let p3 = OutputProfile::new(write_matrix(&dir, "P3.icc", ColorProfile::new_display_p3(), ProfileClass::DisplayDevice).to_string_lossy());
    let mut whole = vec![0.0f32; 8 * tall * 4];
    gradient(tall)(0, tall, &mut whole);
    let reference: Vec<u16> = Space::Srgb.encoded(&whole).unwrap().iter().map(|v| (v * 65535.0).round() as u16).collect();
    let Some(Quantised::Sixteen(through)) = srgb.converted(8, tall, 16, gradient(tall)) else { panic!() };
    let gap = |a: &[u16], b: &[u16]| a.iter().zip(b).map(|(x, y)| (*x as i32 - *y as i32).abs()).max().unwrap();
    assert_eq!(through.len(), reference.len());
    assert!(gap(&reference, &through) <= 2, "largest gap {} of 65535", gap(&reference, &through));
    // Twin: the same rows through another space's profile move.
    let Some(Quantised::Sixteen(other)) = p3.converted(8, tall, 16, gradient(tall)) else { panic!() };
    assert!(gap(&reference, &other) > 1000, "{}", gap(&reference, &other));
    let Some(Quantised::Eight(eight)) = srgb.converted(8, tall, 8, gradient(tall)) else { panic!() };
    assert_eq!(eight.len(), 8 * tall * 4, "at 8 bits for a JPEG it keeps the frame's size");
    assert!(srgb.converted(8, tall, 12, gradient(tall)).is_none(), "and refuses any other depth");
}

#[test]
fn a_display_space_s_black_is_zero_and_a_paper_s_is_not() {
    let srgb = Space::Srgb.icc_data().unwrap();
    assert!(legs::legs(&srgb).unwrap().black < 1e-4);
    // Twin: a paper prints its black at 2 % of its white.
    let paper = legs::legs(&common::stand_in("Paper", 0.02, None, false)).unwrap();
    assert!((paper.black - 0.02).abs() < 2e-3, "{}", paper.black);
}

// MARK: - Installing what a maker ships

fn zip_folder(source: &Path, archive: &Path) {
    let mut zip = zip::ZipWriter::new(fs::File::create(archive).unwrap());
    let mut stack = vec![source.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            let name = path.strip_prefix(source).unwrap().to_string_lossy().into_owned();
            if path.is_dir() {
                stack.push(path);
            } else {
                zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
                zip.write_all(&fs::read(&path).unwrap()).unwrap();
            }
        }
    }
    zip.finish().unwrap();
}

fn cmyk_printer() -> Vec<u8> {
    let mut b = vec![0u8; 132];
    b[0..4].copy_from_slice(&132u32.to_be_bytes());
    b[8] = 2;
    b[12..24].copy_from_slice(b"prtrCMYKLab ");
    b[36..40].copy_from_slice(b"acsp");
    b
}

#[test]
fn a_maker_s_zip_installs_its_one_rgb_printer_profile() {
    let sandbox = common::sandbox("import");
    let (folder, outer, inner, scratch) = (sandbox.join("Profiles"), sandbox.join("outer"), sandbox.join("inner"), sandbox.join("scratch"));
    let junk = outer.join("__MACOSX");
    for d in [&folder, &outer, &inner, &junk, &scratch] {
        fs::create_dir_all(d).unwrap();
    }
    common::write_stand_in(&inner.join("Paper Matte.icc"), "Paper Matte", 0.02, None, false);
    write_matrix(&outer, "Wide Space.icc", ColorProfile::new_pro_photo_rgb(), ProfileClass::ColorSpace);
    fs::write(outer.join("Paper CMYK.icc"), cmyk_printer()).unwrap();
    fs::write(junk.join("._Paper Matte.icc"), b"not a profile").unwrap();
    fs::write(outer.join("Read me.pdf"), b"%PDF").unwrap();
    zip_folder(&inner, &outer.join("inner.zip"));
    let archive = sandbox.join("maker.zip");
    zip_folder(&outer, &archive);

    let (installed, refused) = OutputProfile::importing(std::slice::from_ref(&archive), &folder, &scratch);
    let laid: Vec<String> = fs::read_dir(&folder).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert!(installed.len() == 1 && laid == vec!["Paper Matte.icc"], "{laid:?}");
    assert_eq!(installed[0].name, "Paper Matte", "named by its own description");
    // Twin: the resource fork is an `.icc` too, and read it would be refused.
    assert_eq!(refused, 2, "the CMYK and the colour-space profiles refused, the fork and the PDF never counted");
    let (again, _) = OutputProfile::importing(&[archive], &folder, &scratch);
    assert_eq!(again, installed, "imported twice it reuses the file installed");
    assert_eq!(fs::read_dir(&folder).unwrap().count(), 1);
    assert_eq!(OutputProfile::installed(std::slice::from_ref(&folder), false), installed);
    // Twin: a convertible colour-space profile beside it stays off the list.
    let wide = write_matrix(&folder, "Wide Space.icc", ColorProfile::new_pro_photo_rgb(), ProfileClass::ColorSpace);
    assert_eq!(OutputProfile::installed(std::slice::from_ref(&folder), false).len(), 1);
    assert!(OutputProfile::is_rgb_output(&fs::read(&wide).unwrap()));
    assert!(fs::read_dir(&scratch).unwrap().next().is_none(), "the unpacked archives are gone");
}

#[test]
fn a_profile_gone_is_reported_not_replaced() {
    let sandbox = common::sandbox("gone");
    let gone = Space::Profile(OutputProfile::new(sandbox.join("gone.icc").to_string_lossy()));
    let back = Space::from_key(&gone.key());
    assert_eq!(back.as_ref(), Some(&gone), "a profile gone from disk stays chosen");
    assert!(gone.icc_profile().is_none() && Proof::new(gone.clone()).proofed(&patch(0.5, 0.5, 0.5)).is_none());
    let Space::Profile(p) = &gone else { unreachable!() };
    assert!(p.converted(8, 8, 16, gradient(8)).is_none(), "and renders nothing, so the export reports it");
    let wide = write_matrix(&sandbox, "ROMM RGB.icc", ColorProfile::new_pro_photo_rgb(), ProfileClass::ColorSpace);
    assert!(Space::from_key(&format!("icc:{}", wide.display())).is_none(), "a stored colour-space profile reads as no space");
    // Twin: a printer profile stored the same way is kept.
    let paper = sandbox.join("Paper Matte.icc");
    common::write_stand_in(&paper, "Paper Matte", 0.02, None, false);
    let key = format!("icc:{}", paper.display());
    assert_eq!(Space::from_key(&key), Some(Space::Profile(OutputProfile::new(paper.to_string_lossy()))));
    assert!(Space::BUILT_IN.iter().all(|s| Space::from_key(&s.key()).as_ref() == Some(s)));
    assert!(Space::from_key("cmyk").is_none());
}

// MARK: - What a link installed

#[test]
fn a_link_s_install_is_recorded_and_undone() {
    let sandbox = common::sandbox("link");
    let (folder, scratch, downloads) = (sandbox.join("Profiles"), sandbox.join("scratch"), sandbox.join("dl"));
    for d in [&folder, &scratch, &downloads] {
        fs::create_dir_all(d).unwrap();
    }
    let link = Url::parse("https://www.pictoonline.com/files/Baryta%20Gloss.icc").unwrap();
    let file = downloads.join("Baryta Gloss.icc");
    common::write_stand_in(&file, "Baryta Gloss", 0.02, None, false);
    let mut record = InstallRecord::default();
    let (installed, _) = record.adopt(&file, &link, &folder, &scratch);
    assert_eq!(installed.len(), 1);
    assert_eq!(record.installed_files(&link, &folder), vec![PathBuf::from(&installed[0].path)]);
    let removed = record.uninstall(&link, &folder, |p: &Path| fs::remove_file(p));
    assert_eq!(removed, vec![installed[0].path.clone()]);
    assert!(record.0.is_empty() && !Path::new(&installed[0].path).exists());
    // Twin: unrecorded, a file is found by the name its link promises, and only then.
    fs::copy(&file, folder.join("Baryta Gloss.icc")).unwrap();
    assert_eq!(record.installed_files(&link, &folder).len(), 1);
    let pdf = Url::parse("https://www.pictoonline.com/files/notice.pdf").unwrap();
    assert!(record.installed_files(&pdf, &folder).is_empty());
    assert_eq!(serde_json::to_value(&record).unwrap(), serde_json::json!({}));
}

// MARK: - The screen gamut zebra

#[test]
fn the_zebra_stripes_what_the_screen_clips() {
    let side = 64;
    let tile = |r: f32, g: f32, b: f32| [r, g, b, 1.0].repeat(side * side);
    let share = |a: &[f32], b: &[f32]| {
        let moved = a.chunks_exact(4).zip(b.chunks_exact(4)).filter(|(x, y)| (0..3).any(|c| (x[c] - y[c]).abs() > 1e-3)).count();
        moved as f32 / (side * side) as f32
    };
    let striped = |px: &[f32], display: &Display, phase: f32| {
        let mut out = px.to_vec();
        gamut::striped(&mut out, side, display, phase);
        out
    };
    let srgb = Display::new(&ColorProfile::new_srgb()).unwrap();
    let p3 = Display::new(&ColorProfile::new_display_p3()).unwrap();
    let green = tile(0.2, 0.6, 0.25);
    let grey = tile(0.18, 0.18, 0.18);
    let expected = (0..gamut::PERIOD as usize).filter(|i| (*i as f32) < gamut::DUTY * gamut::PERIOD).count() as f32 / gamut::PERIOD;
    let on_srgb = share(&striped(&green, &srgb, 0.0), &green);
    assert!((on_srgb - expected).abs() < 0.03, "{on_srgb} for {expected}");
    let moved = share(&striped(&green, &srgb, gamut::PERIOD / 2.0), &striped(&green, &srgb, 0.0));
    assert!(moved > 0.2, "half a period later the stripes have moved ({moved})");
    assert_eq!(share(&striped(&grey, &srgb, 0.0), &grey), 0.0);
    // Twin: the same green on a screen that shows it carries no stripe.
    assert_eq!(share(&striped(&green, &p3, 0.0), &green), 0.0);
    assert!((gamut::phase(1.0) - gamut::PERIOD / 2.0).abs() < 1e-6 && gamut::phase(2.0) == 0.0);
    // A truncated remainder, as Swift's: a time under zero reads under zero, never wrapped up.
    assert!((gamut::phase(-1.0) + gamut::PERIOD / 2.0).abs() < 1e-6);
}

#[test]
fn the_stripe_tint_is_the_key_s_lime() {
    let tint = gamut::tint();
    let back = Space::Srgb.encoded(&tint).unwrap();
    assert!((0..3).all(|c| (back[c] - gamut::LIME[c]).abs() < 2e-3), "{back:?}");
    // Twin: the lime's sRGB code read as working light is another colour.
    let raw = Space::Srgb.encoded(&[gamut::LIME[0], gamut::LIME[1], gamut::LIME[2], 1.0]).unwrap();
    assert!((0..3).any(|c| (raw[c] - gamut::LIME[c]).abs() > 0.05));
}
