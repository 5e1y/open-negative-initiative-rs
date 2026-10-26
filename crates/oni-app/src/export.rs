//! Export as a mode of the card, the Swift `Export` and `ExportPanel` without their views: the
//! targets, each frame's line, the labels that count them, the renders and the write.

use oni_colour::{OutputProfile, Space};
use oni_export::{Format, Resize, Row};
use oni_i18n::{tr, trn};
use oni_settings::PipelineSettings;
use oni_settings::film::{Film, film_stock};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub use oni_export::{PREVIEW_SIDE, SAMPLE_SIDE};

pub mod batch;
pub mod clash;
mod renders;

pub use renders::*;

/// Rows the full-resolution render reads back at a time, so a frame never needs every stage at once.
const BAND_ROWS: u32 = 1024;

/// `Export.Target`: a photo and the settings it is written with.
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub path: PathBuf,
    pub settings: PipelineSettings,
}

/// `exportNow`: the gallery's selection in the gallery's order, else the open photo.
pub fn targets(order: &[String], selection: &BTreeSet<usize>, open: Option<&str>) -> Vec<String> {
    if selection.is_empty() {
        return open.map(str::to_owned).into_iter().collect();
    }
    selection.iter().filter_map(|i| order.get(*i)).cloned().collect()
}

/// The four places an export names its count, each as the Swift app words it.
pub mod label {
    use oni_i18n::tr;

    /// `exportLabel`, the File menu's item.
    pub fn menu(count: usize) -> String {
        if count > 1 { tr!("Export {count} Photos…", count = count) } else { tr("Export…").into() }
    }

    /// `AppBar.ExportButton.title` and the card's own key.
    pub fn key(count: usize) -> String {
        if count > 1 { tr!("EXPORT {count} PHOTOS", count = count) } else { tr("EXPORT").into() }
    }

    /// `GalleryView.label`, the photo's context menu.
    pub fn context(count: usize) -> String {
        if count > 1 { tr!("Export… ({count} photos)", count = count) } else { tr("Export…").into() }
    }
}

/// `openingRow`: the sidecar's typed line, else the scan's own facts with the roll's camera and
/// stock and the artist and copyright last written.
pub fn opening_row(path: &Path, roll: Option<&Film>, artist: &str, copyright: &str) -> Row {
    let name = oni_export::stem(path);
    let mosaic = oni_decode::decodes_via_mosaic(path);
    if let Some(typed) = oni_settings::sidecar::read(path, mosaic, false).ok().and_then(|l| l.exif) {
        return Row { name, metadata: typed };
    }
    let mut scan = oni_export::scan_metadata(path);
    if let Some(body) = roll.and_then(|f| f.camera_body.as_deref()).filter(|b| !b.is_empty()) {
        scan.camera = body.to_owned();
    }
    scan.film = film_stock(roll.and_then(|f| f.stock.as_deref())).map(|k| k.display_name.to_owned()).unwrap_or_default();
    if scan.artist.is_empty() {
        scan.artist = artist.to_owned();
    }
    if scan.copyright.is_empty() {
        scan.copyright = copyright.to_owned();
    }
    Row { name, metadata: scan }
}

/// `Geometry.croppedPixelSize`: the frame's size with its quarter turns and crop applied, rounded.
pub fn cropped_size(full: (f64, f64), settings: &PipelineSettings) -> (f64, f64) {
    let g = &settings.geometry;
    let (w, h) = if g.quarter_turns.rem_euclid(2) == 1 { (full.1, full.0) } else { full };
    ((g.crop.width as f64 * w).round(), (g.crop.height as f64 * h).round())
}

/// The panel's first summary line.
pub fn bits(format: Format) -> &'static str {
    if format == Format::Tiff16 { tr("16 bits per channel") } else { tr("8 bits per channel") }
}

/// `ExportPanel.weight`: one frame's estimate times the count, an order of magnitude only.
pub fn weight(estimate: Option<u64>, count: usize) -> String {
    let Some(estimate) = estimate.filter(|e| *e > 0) else { return tr("estimating…").into() };
    let total = estimate as f64 * count.max(1) as f64;
    if total >= 1e9 {
        tr!("≈ {size} GB", size = format!("{:.1}", total / 1e9))
    } else {
        tr!("≈ {size} MB", size = format!("{:.0}", total / 1e6))
    }
}

/// `ExportPanel.header`'s second line.
pub fn subtitle(targets: &[Target], exporting: bool, progress: usize, stopped: Option<usize>) -> String {
    let batch = targets.len() > 1;
    if exporting {
        return if batch { tr!("Writing {progress} of {count}…", progress = progress, count = targets.len()) } else { tr("Writing…").into() };
    }
    if let Some(n) = stopped {
        return trn("Stopped after {n} file was written.", "Stopped after {n} files were written.", n as u64);
    }
    if batch {
        return tr!("{count} photos. You will be asked for a folder; each file takes the name beside its preview.", count = targets.len());
    }
    targets.first().and_then(|t| t.path.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The low-precision warning, or nothing.
pub fn low_precision(count: usize, of: usize) -> String {
    match count {
        0 => String::new(),
        n if n == of => tr("8-bit source: shadows may band once inverted").into(),
        n => tr!("{count} of {of} sources are 8-bit: their shadows may band once inverted", count = n, of = of),
    }
}

/// The two size fields' text: the first target's output size, empty before it is measured.
pub fn size_texts(full: Option<(f64, f64)>, resize: Resize) -> (String, String) {
    match full {
        Some((w, h)) if w > 0.0 => {
            let (ow, oh) = resize.output_size(w, h);
            (format!("{}", ow as i64), format!("{}", oh as i64))
        }
        _ => (String::new(), String::new()),
    }
}

/// `SizeFields.commit`: the field typed decides the edge the ceiling rides.
pub fn typed_size(width: &str, height: &str, shown: (&str, &str), full: (f64, f64)) -> Option<Resize> {
    let digits = |s: &str| s.chars().filter(|c| c.is_ascii_digit()).collect::<String>().parse::<i64>().unwrap_or(0);
    if width != shown.0 {
        return Some(Resize::typing_width(digits(width), full));
    }
    if height != shown.1 {
        return Some(Resize::typing_height(digits(height), full));
    }
    None
}

/// The profiles the picker lists: the installed ones, the chosen one first when it is not among them.
pub fn listed_profiles(installed: &[OutputProfile], chosen: &OutputProfile) -> Vec<OutputProfile> {
    if installed.contains(chosen) {
        return installed.to_vec();
    }
    std::iter::once(chosen.clone()).chain(installed.iter().cloned()).collect()
}

/// `ExportSpaceField.options`: a menu needs distinct titles, so a shared description gains its
/// file name, and a shared file name its folder.
pub fn profile_titles(listed: &[OutputProfile]) -> Vec<String> {
    let file = |p: &OutputProfile| Path::new(&p.path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let named = |n: &str| listed.iter().filter(|p| p.name == n).count();
    let filed = |n: &str, f: &str| listed.iter().filter(|p| p.name == n && file(p) == f).count();
    listed
        .iter()
        .map(|p| {
            let f = file(p);
            if named(&p.name) <= 1 {
                return p.name.clone();
            }
            if filed(&p.name, &f) <= 1 {
                return format!("{} — {f}", p.name);
            }
            let folder = oni_library::paths::parent(&p.path);
            format!("{} — {f} — {folder}", p.name)
        })
        .collect()
}

/// The note under the profile picker, empty when there is nothing to say.
pub fn profile_note(chosen: &OutputProfile, installed: &[OutputProfile]) -> &'static str {
    if !chosen.is_usable() {
        tr("This profile can no longer be read. Choose another one.")
    } else if installed.is_empty() {
        tr("No printer or paper profile is installed yet: load one, or get your paper's below.")
    } else {
        ""
    }
}

/// `loadProfile`'s report, `None` when every file chosen was installed.
pub fn import_report(installed: usize, refused: usize) -> Option<(String, String)> {
    if refused == 0 && installed > 0 {
        return None;
    }
    let title = match installed {
        0 => tr("No Profile Installed").to_owned(),
        n => trn("{n} Profile Installed", "{n} Profiles Installed", n as u64),
    };
    let message = if installed == 0 && refused == 0 {
        tr("No ICC profile was found in what you chose.").to_owned()
    } else {
        tr!(
            "{refused} left out: Custom takes RGB printer and paper profiles. CMYK profiles are not supported, and sRGB, Display P3, Adobe RGB and ProPhoto have their own tabs.",
            refused = refused
        )
    };
    Some((title, message))
}

/// The note under the space when the previews show another space than the one edited in.
pub fn edited_in_note(edited_in: Option<&Space>, space: &Space, batch: bool) -> String {
    match edited_in {
        Some(e) if e != space && batch => {
            tr!("These images were edited in {edited}. The previews show them in {space}.", edited = e.title(), space = space.title())
        }
        Some(e) if e != space => tr!("This image was edited in {edited}. The previews show them in {space}.", edited = e.title(), space = space.title()),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{Decoded, Engine, Linear};
    use oni_export::Image;

    #[test]
    fn the_target_is_the_selection_else_the_open_photo() {
        let order: Vec<String> = ["a", "b", "c", "d"].map(String::from).to_vec();
        assert_eq!(targets(&order, &BTreeSet::new(), Some("c")), ["c"]);
        assert_eq!(targets(&order, &BTreeSet::from([3, 0]), Some("c")), ["a", "d"], "the gallery's order, not the set's");
        assert!(targets(&order, &BTreeSet::new(), None).is_empty());
        // Twin: the open photo outside the selection is not added to it.
        assert!(!targets(&order, &BTreeSet::from([1]), Some("c")).contains(&"c".to_string()));
    }

    #[test]
    fn every_label_counts_its_photos_past_one() {
        assert_eq!(label::menu(1), "Export…");
        assert_eq!(label::menu(3), "Export 3 Photos…");
        assert_eq!(label::key(0), "EXPORT");
        assert_eq!(label::key(12), "EXPORT 12 PHOTOS");
        assert_eq!(label::context(2), "Export… (2 photos)");
        // Twin: one photo is never counted.
        assert!(!label::key(1).contains('1') && !label::context(1).contains('1'));
    }

    #[test]
    fn the_summary_reads_as_the_panel_reads() {
        assert_eq!(weight(None, 3), "estimating…");
        assert_eq!(weight(Some(12_400_000), 1), "≈ 12 MB");
        assert_eq!(weight(Some(400_000_000), 3), "≈ 1.2 GB");
        assert_eq!(low_precision(2, 2), "8-bit source: shadows may band once inverted");
        assert_eq!(low_precision(1, 3), "1 of 3 sources are 8-bit: their shadows may band once inverted");
        assert_eq!(low_precision(0, 3), "");
        let one = [Target { path: PathBuf::from("/r/DSC_1.NEF"), settings: PipelineSettings::default() }];
        assert_eq!(subtitle(&one, false, 0, None), "DSC_1.NEF");
        let two = [one[0].clone(), one[0].clone()];
        assert_eq!(subtitle(&two, true, 1, None), "Writing 1 of 2…");
        assert_eq!(subtitle(&two, false, 0, Some(1)), "Stopped after 1 file was written.");
        // Twin: a batch names no single file.
        assert!(!subtitle(&two, false, 0, None).contains("DSC_1"));
    }

    #[test]
    fn the_size_field_typed_rides_its_own_edge() {
        let full = (6000.0, 4000.0);
        assert_eq!(size_texts(Some(full), Resize::Full), ("6000".into(), "4000".into()));
        assert_eq!(size_texts(None, Resize::Full), (String::new(), String::new()));
        assert_eq!(typed_size("3000", "4000", ("6000", "4000"), full), Some(Resize::LongestEdge(3000)));
        assert_eq!(typed_size("6000", "1000", ("6000", "4000"), full), Some(Resize::ShortestEdge(1000)));
        assert_eq!(typed_size("6000", "4000", ("6000", "4000"), full), None, "an untouched pair commits nothing");
        // Twin: a width at or past the original is full resolution, never an enlargement.
        assert_eq!(typed_size("9000", "4000", ("6000", "4000"), full), Some(Resize::Full));
    }

    #[test]
    fn a_turned_frame_measures_its_crop_across_the_turn() {
        let mut s = PipelineSettings::default();
        s.geometry.crop.width = 0.5;
        assert_eq!(cropped_size((6000.0, 4000.0), &s), (3000.0, 4000.0));
        s.geometry.quarter_turns = 1;
        assert_eq!(cropped_size((6000.0, 4000.0), &s), (2000.0, 6000.0));
        // Twin: three turns is one turn's shape, never the unturned one.
        s.geometry.quarter_turns = -1;
        assert_eq!(cropped_size((6000.0, 4000.0), &s), (2000.0, 6000.0));
    }

    #[test]
    fn profile_titles_stay_distinct_as_the_menu_needs() {
        let p = |path: &str, name: &str| OutputProfile { path: path.into(), name: name.into() };
        let listed = [p("/a/x.icc", "Baryta"), p("/b/x.icc", "Baryta"), p("/a/y.icc", "Baryta"), p("/a/z.icc", "Matte")];
        assert_eq!(
            profile_titles(&listed),
            ["Baryta — x.icc — /a", "Baryta — x.icc — /b", "Baryta — y.icc", "Matte"]
        );
        let chosen = p("/gone.icc", "Gone");
        assert_eq!(listed_profiles(&listed, &chosen)[0], chosen, "a chosen profile no longer installed stays listed, first");
        assert_eq!(listed_profiles(&listed, &listed[3]).len(), 4);
        // Twin: unique names are never decorated.
        assert_eq!(profile_titles(&listed[3..]), ["Matte"]);
    }

    #[test]
    fn the_import_report_speaks_only_of_what_was_left_out() {
        assert_eq!(import_report(2, 0), None);
        assert_eq!(import_report(0, 0).unwrap(), ("No Profile Installed".into(), "No ICC profile was found in what you chose.".into()));
        let (title, message) = import_report(1, 2).unwrap();
        assert_eq!(title, "1 Profile Installed");
        assert!(message.starts_with("2 left out: Custom takes RGB printer and paper profiles."));
        assert_eq!(edited_in_note(Some(&Space::Srgb), &Space::DisplayP3, true), "These images were edited in sRGB. The previews show them in Display P3.");
        // Twin: the same space says nothing.
        assert_eq!(edited_in_note(Some(&Space::DisplayP3), &Space::DisplayP3, false), "");
    }

    /// The export renders the frame with the custom mask and the dust patches the canvas lays, so
    /// the file holds what the canvas shows; a frame naming none renders as without them.
    #[test]
    fn the_export_render_lays_the_canvas_s_masks_and_patches() {
        let Ok(device) = crate::render::Device::open() else {
            eprintln!("no adapter: the export's inputs are not exercised");
            return;
        };
        let root = std::env::temp_dir().join(format!("oni-export-inputs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut engine = Engine::on(&device);
        engine.support = Some(root.join("support"));
        engine.caches = Some(root.join("caches"));
        let (w, h) = (64usize, 48usize);
        let decoded = || Decoded { linear: Linear { width: w, height: h, pixels: vec![[0.3, 0.3, 0.3, 1.0]; w * h] }, full_width: w as f64, undeclared: None };
        let tiff = |path: &Path, pixels: &[f32]| {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let out = std::fs::File::create(path).unwrap();
            tiff::encoder::TiffEncoder::new(out).unwrap().write_image::<tiff::encoder::colortype::RGBA32Float>(w as u32, h as u32, pixels).unwrap();
        };
        let rest = PipelineSettings::rest(oni_settings::ConversionMode::Positive, true);
        let plain = render_full(&mut engine, decoded(), &rest).unwrap();
        let at = |i: &Image, x: usize, y: usize| i.pixels[(y * w + x) * 4 + 1];

        // A mask darker at the left edge: dividing by it lifts the left of the frame.
        let mask: Vec<f32> = (0..w * h).flat_map(|i| { let v = 0.5 + 0.5 * (i % w) as f32 / w as f32; [v, v, v, 1.0] }).collect();
        tiff(&crate::flat_fields::folder(root.join("support").as_path()).join("ramp.tiff"), &mask);
        let mut masked = rest.clone();
        masked.flat_field.mask = oni_settings::frame::FlatFieldMask::Custom;
        masked.flat_field.custom = Some("ramp".into());
        let flat = render_full(&mut engine, decoded(), &masked).unwrap();
        assert!((at(&flat, 2, h / 2) - at(&plain, 2, h / 2)).abs() > 1e-2, "the mask reaches the export");

        // A bright patch under a stroke at the centre.
        let id = oni_settings::id::Id::parse("0F2B6E4C-9A1D-4E7B-8C3A-5D6E7F809A1B").unwrap();
        tiff(&crate::patches::file(root.join("caches").as_path(), id), &vec![0.9f32; w * h * 4]);
        let mut healed = rest.clone();
        let point = |x: f64, y: f64| oni_settings::cg::Point { x, y };
        healed.corrections = vec![oni_settings::frame::Correction { id, stroke: vec![point(0.5, 0.5)], radius: 6.0 }];
        let patched = render_full(&mut engine, decoded(), &healed).unwrap();
        assert!((at(&patched, w / 2, h / 2) - at(&plain, w / 2, h / 2)).abs() > 1e-2, "the patch reaches the export");
        // Twin: far from the stroke the frame is its own, and a mask no file backs changes nothing.
        assert!((at(&patched, 2, 2) - at(&plain, 2, 2)).abs() < 1e-5);
        masked.flat_field.custom = Some("gone".into());
        let missing = render_full(&mut engine, decoded(), &masked).unwrap();
        assert!(missing.pixels.iter().zip(&plain.pixels).all(|(a, b)| (a - b).abs() < 1e-6));
        let _ = std::fs::remove_dir_all(&root);
    }
}
