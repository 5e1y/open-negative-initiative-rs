//! Exported files of Open Negative Initiative: one writer for TIFF and JPEG, the metadata the hand
//! stated and nothing else, resize on one edge, the size estimate, the presets and the HDR gain map.

pub mod file;
pub mod gain_map;
pub mod ifd;
pub mod jpeg;
pub mod landing;
pub mod metadata;
pub mod presets;
pub mod resize;
pub mod settings;
pub mod tiff;

use std::fmt;
use std::path::{Path, PathBuf};

use oni_colour::{PaperCorrection, Quantised, Space};
use oni_i18n::tr;

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
    /// The disk's own reason, so the notice can say full, read-only or refused.
    Write(std::io::ErrorKind),
    Profile,
    WouldOverwriteOriginal,
    /// Past a JPEG's 65,535 pixels a side or its 16.7 MB profile.
    TooLargeForJpeg,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Failure::Decode => tr("The RAW file could not be decoded."),
            Failure::Render => tr("Full-resolution rendering failed."),
            Failure::Profile => tr("The ICC profile chosen for the export can no longer be read."),
            Failure::Write(kind) => match kind {
                std::io::ErrorKind::StorageFull | std::io::ErrorKind::QuotaExceeded => {
                    tr("The disk is full: the file could not be written. A file already there is kept.")
                }
                std::io::ErrorKind::ReadOnlyFilesystem => tr("The disk is read-only: the file could not be written."),
                std::io::ErrorKind::PermissionDenied => tr("ONI is not allowed to write in that folder. A file already there is kept."),
                std::io::ErrorKind::NotFound => tr("The folder is no longer there: the file could not be written."),
                _ => tr("The file could not be written."),
            },
            Failure::TooLargeForJpeg => tr(
                "Too large for a JPEG, which holds at most 65,535 pixels a side and a 16.7 MB profile: choose TIFF or a smaller size.",
            ),
            Failure::WouldOverwriteOriginal => tr(
                "A file ONI did not export already has that name, an original perhaps: it is left as it was. Pick another folder or another name.",
            ),
        })
    }
}

impl std::error::Error for Failure {}

/// The file name a typed stem yields: one portable component that cannot manufacture a path, its
/// leading dots dropped and the original's name when empty, so no hidden file is written.
pub fn name(source: &Path, typed: &str, format: Format) -> String {
    let shown = |text: &str| oni_library::file_names::portable(text).trim_start_matches(|c| c == '.' || is_blank(c)).to_owned();
    let mut stem = shown(typed.trim_matches(is_blank));
    if stem.is_empty() {
        stem = shown(&self::stem(source));
    }
    if stem.is_empty() {
        stem = "export".into();
    }
    format!("{stem}.{}", format.file_extension())
}

/// The file a typed stem yields in `folder`, always directly inside it.
pub fn destination(folder: &Path, source: &Path, typed: &str, format: Format) -> PathBuf {
    let named = name(source, typed, format);
    oni_library::file_names::inside(folder, &named).unwrap_or_else(|| folder.join(format!("export.{}", format.file_extension())))
}

/// Foundation's `.whitespaces`: spaces and tabs, never a line break.
fn is_blank(c: char) -> bool {
    c.is_whitespace() && !matches!(c, '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

pub fn stem(source: &Path) -> String {
    source.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
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
            // Sized once: a vector doubling as it fills holds two copies at its last growth.
            let (mut eight, mut sixteen) = if bits == 16 { (Vec::new(), Vec::with_capacity(w * h * 4)) } else { (Vec::with_capacity(w * h * 4), Vec::new()) };
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

/// Where one frame lands: a new file, or an earlier export the ledger vouches for when `replacing`.
pub struct Landing<'a> {
    pub path: &'a Path,
    pub ledger: &'a mut file::Ledger,
    pub replacing: bool,
}

/// Writes one frame through a temporary file and a rename. Refuses its own source and any file ONI
/// did not export before `render` runs, and again at the rename; `render` yields the full frame.
pub fn write(
    source: &Path,
    to: Landing<'_>,
    render: impl FnOnce() -> Result<Image, Failure>,
    paper: &PaperCorrection,
    export: &Settings,
    metadata: &ExifMetadata,
    date_override: Option<&str>,
) -> Result<(), Failure> {
    let lands = match file::standing(to.path, to.ledger) {
        file::Standing::Free => true,
        file::Standing::Export => to.replacing,
        file::Standing::Original => false,
    };
    if !lands || file::canonical(to.path) == file::canonical(source) {
        return Err(Failure::WouldOverwriteOriginal);
    }
    let full = render()?;
    let scale = export.resize.scale(full.width as f64, full.height as f64);
    let mut out = resized(full, scale);
    let icc = export.space().icc_data().map_or(0, |icc| icc.len());
    if export.format() == Format::Jpeg && !jpeg::fits(out.width, out.height, icc) {
        return Err(Failure::TooLargeForJpeg);
    }
    oni_colour::paper::laid(paper, &mut out.pixels, export.space());
    let rendered = converted(&out, export)?;
    drop(out);
    let date = date_override.map(str::to_owned).or_else(|| capture_date(source));
    let data = encoded(&rendered, export, date.as_deref(), metadata).ok_or(Failure::Write(std::io::ErrorKind::InvalidData))?;
    match file::put(to.path, &data, to.ledger, to.replacing) {
        Ok(()) => {
            let _ = to.ledger.note(to.path);
            Ok(())
        }
        Err(file::Refused::Taken) => Err(Failure::WouldOverwriteOriginal),
        Err(file::Refused::Io(e)) => Err(Failure::Write(e.kind())),
    }
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
