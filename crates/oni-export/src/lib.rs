//! Exported files of Open Negative Initiative: one writer for TIFF and JPEG, the metadata the hand
//! stated and nothing else, resize on one edge, the size estimate, the presets and the HDR gain map.

pub mod gain_map;
pub mod ifd;
pub mod jpeg;
pub mod metadata;
pub mod presets;
pub mod resize;
pub mod settings;
pub mod tiff;

use std::fmt;
use std::path::{Path, PathBuf};

use oni_colour::{PaperCorrection, Quantised, Space};

pub use metadata::{ExifMetadata, Row, capture_date, scan_metadata};
pub use presets::Preset;
pub use resize::{Resize, resized};
pub use settings::{Format, Settings};

/// The reduced copy a file is weighed on; the estimate's ±2 % is priced at this side.
pub const SAMPLE_SIDE: f64 = 500.0;

/// The copy a cell of the export column renders: large enough to judge the picture.
pub const PREVIEW_SIDE: f64 = 1600.0;

/// RGBA in floating point, rows top first: the rendered frame in the working space.
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<f32>,
}

/// A frame in its output space, quantised for the file, with the profile it was converted into.
#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    pub width: usize,
    pub height: usize,
    /// RGBA, alpha at full scale.
    pub pixels: Quantised,
    pub icc: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    Decode,
    Render,
    Write,
    Profile,
    WouldOverwriteSource,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Failure::Decode => "The RAW file could not be decoded.",
            Failure::Render => "Full-resolution rendering failed.",
            Failure::Profile => "The ICC profile chosen for the export can no longer be read.",
            Failure::Write => "The file could not be written.",
            Failure::WouldOverwriteSource => {
                "Exporting there would overwrite the original scan. Pick another folder, or change \
                 the naming pattern so the output cannot land on its own source."
            }
        })
    }
}

impl std::error::Error for Failure {}

/// The file name a typed stem yields: sanitised so it cannot manufacture a path, the
/// original's name when empty so no hidden `.tiff` is written, the format's extension added.
pub fn name(source: &Path, typed: &str, format: Format) -> String {
    let mut stem = typed.replace(['/', ':'], "-").trim_matches(is_blank).to_owned();
    if stem.is_empty() {
        stem = self::stem(source);
    }
    format!("{stem}.{}", format.file_extension())
}

/// Foundation's `.whitespaces`: spaces and tabs, never a line break.
fn is_blank(c: char) -> bool {
    c.is_whitespace() && !matches!(c, '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

pub fn stem(source: &Path) -> String {
    source.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The destinations a batch would overwrite: one already on disk, or one named twice.
pub fn clashes(destinations: &[PathBuf], exists: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    let mut seen = std::collections::HashSet::new();
    let mut found = Vec::new();
    for url in destinations {
        let path = oni_colour::profile::standardized(url);
        if !seen.insert(path) || exists(url) {
            found.push(url.clone());
        }
    }
    found
}

/// The path with symlinks resolved where it exists: a file not yet written resolves its folder.
fn canonical(path: &Path) -> PathBuf {
    if let Ok(found) = std::fs::canonicalize(path) {
        return found;
    }
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(file)) => {
            match std::fs::canonicalize(if parent.as_os_str().is_empty() { Path::new(".") } else { parent }) {
                Ok(folder) => folder.join(file),
                Err(_) => PathBuf::from(oni_colour::profile::standardized(path)),
            }
        }
        _ => PathBuf::from(oni_colour::profile::standardized(path)),
    }
}

/// The one door from the working-space frame to the file's pixels: the space's matrix and curve,
/// or the profile's own conversion, quantised to the format's depth.
pub fn converted(image: &Image, export: &Settings) -> Result<Rendered, Failure> {
    let icc = export.space().icc_data().ok_or(Failure::Profile)?;
    let bits = export.format().bits();
    let (w, h) = (image.width, image.height);
    if w == 0 || h == 0 || image.pixels.len() != w * h * 4 {
        return Err(Failure::Render);
    }
    let pixels = match export.space() {
        Space::Profile(profile) => profile
            .converted(w, h, bits, |top, rows, out| {
                out.copy_from_slice(&image.pixels[top * w * 4..(top + rows) * w * 4])
            })
            .ok_or(Failure::Render)?,
        space => {
            let scale = if bits == 16 { 65535.0 } else { 255.0 };
            let mut eight = Vec::new();
            let mut sixteen = Vec::new();
            for band in image.pixels.chunks(oni_colour::legs::BAND * w * 4) {
                let mut encoded = space.encoded(band).ok_or(Failure::Render)?;
                // Premultiplied, as Core Image renders: a clear pixel, a contact sheet's gutter, writes black.
                for (p, s) in encoded.chunks_exact_mut(4).zip(band.chunks_exact(4)) {
                    let a = s[3].clamp(0.0, 1.0);
                    for v in &mut p[..3] {
                        *v *= a;
                    }
                    p[3] = a;
                }
                let quantised = encoded.iter().map(|v| (v.clamp(0.0, 1.0) * scale).round());
                if bits == 16 {
                    sixteen.extend(quantised.map(|v| v as u16));
                } else {
                    eight.extend(quantised.map(|v| v as u8));
                }
            }
            if bits == 16 { Quantised::Sixteen(sixteen) } else { Quantised::Eight(eight) }
        }
    };
    Ok(Rendered { width: w, height: h, pixels, icc })
}

/// Renders through the output space so a switch of space shows on screen; encodes nothing.
pub fn preview(image: &Image, export: &Settings) -> Result<Rendered, Failure> {
    converted(image, export)
}

/// One function for the written file and the estimate, so the estimate weighs what the file carries.
pub fn encoded(rendered: &Rendered, export: &Settings, date: Option<&str>, metadata: &ExifMetadata) -> Option<Vec<u8>> {
    let (primary, exif) = metadata::stated(metadata, date);
    let (w, h) = (rendered.width, rendered.height);
    match export.format() {
        Format::Tiff16 => tiff::encoded(w, h, &rendered.pixels, export.compress_tiff, &rendered.icc, &primary, &exif),
        Format::Jpeg => {
            let base = jpeg::encoded(w, h, &rendered.pixels, export.jpeg_quality, &rendered.icc, &primary, &exif)?;
            if export.writes_gain_map() { gain_map::file(&base, w, h) } else { Some(base) }
        }
    }
}

/// Writes one frame. Refuses a destination equal to its source before `render` runs, compared on
/// canonical paths with symlinks resolved; `render` yields the pipeline's full-resolution output.
pub fn write(
    source: &Path,
    destination: &Path,
    render: impl FnOnce() -> Result<Image, Failure>,
    paper: &PaperCorrection,
    export: &Settings,
    metadata: &ExifMetadata,
    date_override: Option<&str>,
) -> Result<(), Failure> {
    if canonical(destination) == canonical(source) {
        return Err(Failure::WouldOverwriteSource);
    }
    let out = render()?;
    let mut out = resized(&out, export.resize.scale(out.width as f64, out.height as f64));
    oni_colour::paper::laid(paper, &mut out.pixels, export.space());
    let rendered = converted(&out, export)?;
    let date = date_override.map(str::to_owned).or_else(|| capture_date(source));
    let data = encoded(&rendered, export, date.as_deref(), metadata).ok_or(Failure::Write)?;
    std::fs::write(destination, data).map_err(|_| Failure::Write)
}

/// Estimated weight, extrapolated from the reduced preview by ratio of areas: more accurate than
/// a full-resolution crop, which lands on one particular area.
pub fn estimate(preview: &Image, full_pixels: f64, export: &Settings) -> u64 {
    let preview_pixels = (preview.width * preview.height) as f64;
    if preview_pixels <= 0.0 {
        return 0;
    }
    // Uncompressed, the weight is exact: 4 channels of 16 bits.
    if export.format() == Format::Tiff16 && !export.compress_tiff {
        return full_pixels as u64 * 8;
    }
    let Ok(rendered) = converted(preview, export) else { return 0 };
    let Some(data) = encoded(&rendered, export, None, &ExifMetadata::default()) else { return 0 };
    (data.len() as f64 * (full_pixels / preview_pixels)) as u64
}
