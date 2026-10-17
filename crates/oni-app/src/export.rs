//! Export as a mode of the card, the Swift `Export` and `ExportPanel` without their views: the
//! targets, each frame's line, the labels that count them, the renders and the write.

use crate::render::{Decoded, Engine, Linear, Negative, Shared};
use oni_export::{ExifMetadata, Failure, Format, Image, Resize, Row, Settings};
use oni_graph::Image as Recipe;
use oni_pipeline::{Inputs, Request};
use oni_settings::PipelineSettings;
use oni_settings::film::{Film, film_stock};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub use oni_export::{PREVIEW_SIDE, SAMPLE_SIDE};

/// Rows the full-resolution render reads back at a time, so a frame never needs every stage at once.
const BAND_ROWS: u32 = 1024;

/// `Export.Target`: a photo and the settings it is written with.
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub path: PathBuf,
    pub settings: PipelineSettings,
}

/// `exportNow`: the gallery's selection in the gallery's order, else the open photo.
pub fn targets(order: &[String], selection: &BTreeSet<usize>, open: Option<usize>) -> Vec<String> {
    if selection.is_empty() {
        return open.and_then(|i| order.get(i)).cloned().into_iter().collect();
    }
    selection.iter().filter_map(|i| order.get(*i)).cloned().collect()
}

/// The four places an export names its count, each as the Swift app words it.
pub mod label {
    /// `exportLabel`, the File menu's item.
    pub fn menu(count: usize) -> String {
        if count > 1 { format!("Export {count} Photos…") } else { "Export…".into() }
    }

    /// `AppBar.ExportButton.title` and the card's own key.
    pub fn key(count: usize) -> String {
        if count > 1 { format!("EXPORT {count} PHOTOS") } else { "EXPORT".into() }
    }

    /// `GalleryView.label`, the photo's context menu.
    pub fn context(count: usize) -> String {
        if count > 1 { format!("Export… ({count} photos)") } else { "Export…".into() }
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
    if format == Format::Tiff16 { "16 bits per channel" } else { "8 bits per channel" }
}

/// `ExportPanel.weight`: one frame's estimate times the count, an order of magnitude only.
pub fn weight(estimate: Option<u64>, count: usize) -> String {
    let Some(estimate) = estimate.filter(|e| *e > 0) else { return "estimating…".into() };
    let total = estimate as f64 * count.max(1) as f64;
    if total >= 1e9 { format!("≈ {:.1} GB", total / 1e9) } else { format!("≈ {:.0} MB", total / 1e6) }
}

/// `ExportPanel.header`'s second line.
pub fn subtitle(targets: &[Target], exporting: bool, progress: usize, stopped: Option<usize>) -> String {
    let batch = targets.len() > 1;
    if exporting {
        return if batch { format!("Writing {progress} of {}…", targets.len()) } else { "Writing…".into() };
    }
    if let Some(n) = stopped {
        return if n == 1 { "Stopped after 1 file was written.".into() } else { format!("Stopped after {n} files were written.") };
    }
    if batch {
        return format!("{} photos. You will be asked for a folder; each file takes the name beside its preview.", targets.len());
    }
    targets.first().and_then(|t| t.path.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The low-precision warning, or nothing.
pub fn low_precision(count: usize, of: usize) -> String {
    match count {
        0 => String::new(),
        n if n == of => "8-bit source: shadows may band once inverted".into(),
        n => format!("{n} of {of} sources are 8-bit: their shadows may band once inverted"),
    }
}

/// `RawDecode.isLowPrecision`: a linear scan of 8 bits or fewer; a RAW never is.
pub fn is_low_precision(path: &Path) -> bool {
    oni_decode::route(path) == oni_decode::Branch::LinearRgb && oni_decode::container::bits_per_sample(path).is_some_and(|b| b <= 8)
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

// MARK: - Renders

/// `RawDecode.linear(source)` at full resolution, the export's own decode.
pub fn full_decode(path: &Path, primaries_converted: bool) -> Result<Decoded, Failure> {
    match oni_decode::route(path) {
        oni_decode::Branch::Mosaic => {
            let d = oni_decode::decode(path, 1.0).map_err(|_| Failure::Decode)?;
            let full_width = d.width as f64;
            Ok(Decoded { linear: Linear { width: d.width, height: d.height, pixels: d.pixels }, full_width })
        }
        oni_decode::Branch::LinearRgb if !primaries_converted => {
            let d = oni_decode::container::linear(path, None).ok_or(Failure::Decode)?;
            let full_width = d.width as f64;
            Ok(Decoded { linear: Linear { width: d.width, height: d.height, pixels: d.pixels }, full_width })
        }
        _ => Err(Failure::Decode),
    }
}

/// `Pipeline.apply` at full resolution and unproofed, read back in bands of rows.
pub fn render_full(engine: &mut Engine, decoded: Decoded, settings: &PipelineSettings) -> Result<Image, Failure> {
    let l = decoded.linear;
    let (graph, pipeline) = engine.parts();
    let source = graph.upload(l.width as u32, l.height as u32, &l.pixels).map_err(|_| Failure::Render)?;
    drop(l.pixels);
    let image = pipeline.image(graph, &source, settings, &Request::default(), &Inputs::default()).map_err(|_| Failure::Render)?;
    let out = read_back(graph, &image);
    drop(source);
    graph.purge();
    out
}

fn read_back(graph: &mut oni_graph::Graph, image: &Recipe) -> Result<Image, Failure> {
    let e = image.extent();
    let mut pixels = Vec::with_capacity(e.area() as usize * 4);
    for top in (0..e.h).step_by(BAND_ROWS as usize) {
        let band = oni_graph::Rect::new(e.x, e.y + top as i32, e.w, BAND_ROWS.min(e.h - top));
        let rows = graph.render(image, band, None).map_err(|_| Failure::Render)?;
        pixels.extend(rows.into_iter().flatten());
    }
    Ok(Image { width: e.w as usize, height: e.h as usize, pixels })
}

/// `Pipeline.apply` on a preview reduced to `side`, unproofed: the estimate's sample.
pub fn render_reduced(engine: &mut Engine, negative: &Negative, settings: &PipelineSettings, side: usize) -> Result<Image, Failure> {
    let (w, h) = negative.size();
    let scale = (side as f64 / w.max(h) as f64).min(1.0);
    let request = Request { scale, full_width: Some(negative.full_width), ..Request::default() };
    let (graph, pipeline) = engine.parts();
    let image = pipeline.image(graph, &negative.preview, settings, &request, &Inputs::default()).map_err(|_| Failure::Render)?;
    read_back(graph, &image)
}

/// `refreshEstimate`: the first target weighed on its 500 px sample, scaled to its pixel count.
pub fn weigh(engine: &Shared, negative: &Negative, full: (f64, f64), settings: &PipelineSettings, export: &Settings) -> Option<u64> {
    let sample = render_reduced(&mut *engine.lock().ok()?, negative, settings, SAMPLE_SIDE as usize).ok()?;
    Some(oni_export::estimate(&sample, full.0 * full.1, export))
}

/// One frame written: its full render, its file, then its line kept in its sidecar.
pub fn write_one(engine: &Shared, target: &Target, destination: &Path, export: &Settings, metadata: &ExifMetadata) -> Result<(), Failure> {
    let render = || {
        let decoded = full_decode(&target.path, target.settings.primaries_converted)?;
        let mut engine = engine.lock().map_err(|_| Failure::Render)?;
        render_full(&mut engine, decoded, &target.settings)
    };
    let date = (!metadata.date.is_empty()).then_some(metadata.date.as_str());
    oni_export::write(&target.path, destination, render, &target.settings.paper_correction, export, metadata, date)?;
    let mosaic = oni_decode::decodes_via_mosaic(&target.path);
    let _ = oni_settings::sidecar::set_exif(metadata.clone(), &target.path, mosaic);
    Ok(())
}

/// The destinations of a batch in a folder: each file the name beside its preview.
pub fn destinations(folder: &Path, targets: &[Target], rows: &[Option<Row>], format: Format) -> Vec<PathBuf> {
    targets
        .iter()
        .zip(rows)
        .map(|(t, r)| folder.join(oni_export::name(&t.path, r.as_ref().map_or("", |r| r.name.as_str()), format)))
        .collect()
}

/// The clash dialog's title and message, `None` when nothing is overwritten.
pub fn clash_notice(clashes: usize, of: usize) -> Option<(String, String)> {
    if clashes == 0 {
        return None;
    }
    let title = if clashes == 1 { "1 File Already Exists in That Folder".to_owned() } else { format!("{clashes} Files Already Exist in That Folder") };
    let message = if clashes == of {
        "Exporting will replace them. The originals are not touched.".to_owned()
    } else {
        format!("Exporting will replace {clashes} of the {of} files. The originals are not touched.")
    };
    Some((title, message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_target_is_the_selection_else_the_open_photo() {
        let order: Vec<String> = ["a", "b", "c", "d"].map(String::from).to_vec();
        assert_eq!(targets(&order, &BTreeSet::new(), Some(2)), ["c"]);
        assert_eq!(targets(&order, &BTreeSet::from([3, 0]), Some(2)), ["a", "d"], "the gallery's order, not the set's");
        assert!(targets(&order, &BTreeSet::new(), None).is_empty());
        // Twin: the open photo outside the selection is not added to it.
        assert!(!targets(&order, &BTreeSet::from([1]), Some(2)).contains(&"c".to_string()));
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
    fn a_batch_names_its_clashes_as_the_dialog_does() {
        assert_eq!(clash_notice(0, 3), None);
        let (title, message) = clash_notice(1, 3).unwrap();
        assert_eq!(title, "1 File Already Exists in That Folder");
        assert_eq!(message, "Exporting will replace 1 of the 3 files. The originals are not touched.");
        assert_eq!(clash_notice(3, 3).unwrap().1, "Exporting will replace them. The originals are not touched.");
    }
}
