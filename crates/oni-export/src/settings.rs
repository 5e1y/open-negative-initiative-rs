//! How the hand exports: format, space, compression, quality, resize, HDR, kept as one preference.

use oni_colour::Space;
use serde::{Deserialize, Serialize};

use crate::resize::Resize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    Tiff16,
    Jpeg,
}

impl Format {
    pub const ALL: [Format; 2] = [Format::Tiff16, Format::Jpeg];

    /// The name the interface shows, also the format's identity.
    pub fn raw_value(self) -> &'static str {
        match self {
            Format::Tiff16 => "TIFF 16-bit",
            Format::Jpeg => "JPEG",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Format::Tiff16 => "tiff16",
            Format::Jpeg => "jpeg",
        }
    }

    pub fn file_extension(self) -> &'static str {
        match self {
            Format::Tiff16 => "tiff",
            Format::Jpeg => "jpg",
        }
    }

    pub fn bits(self) -> u32 {
        match self {
            Format::Tiff16 => 16,
            Format::Jpeg => 8,
        }
    }
}

/// Format and space are reached through setters, which keep ProPhoto on a TIFF either way round.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    format: Format,
    /// Also the working space the canvas is proofed in: Display P3, what Apple panels render.
    space: Space,
    /// Lossless ZIP compression of the TIFF. LZW is not offered: it inflates grainy 16-bit
    /// scans instead of shrinking them.
    pub compress_tiff: bool,
    pub jpeg_quality: f64,
    pub resize: Resize,
    /// Writes the JPEG with a flat gain map so HDR screens lift the picture; the pixels do not
    /// change. Honoured by the writer alone, and only on JPEG.
    pub hdr: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            format: Format::Jpeg,
            space: Space::DisplayP3,
            compress_tiff: true,
            jpeg_quality: 0.85,
            resize: Resize::Full,
            hdr: false,
        }
    }
}

impl Settings {
    pub fn format(&self) -> Format {
        self.format
    }

    pub fn space(&self) -> &Space {
        &self.space
    }

    /// A JPEG leaves ProPhoto for Adobe RGB, the widest space eight bits carry without banding.
    pub fn set_format(&mut self, format: Format) {
        self.format = format;
        if format == Format::Jpeg && self.space.needs_sixteen_bits() {
            self.space = Space::AdobeRgb;
        }
    }

    /// ProPhoto takes the TIFF along.
    pub fn set_space(&mut self, space: Space) {
        self.space = space;
        if self.space.needs_sixteen_bits() {
            self.format = Format::Tiff16;
        }
    }

    pub fn writes_gain_map(&self) -> bool {
        self.hdr && self.format == Format::Jpeg
    }
}

// MARK: - The settings as one preference

pub const PREFERENCE_KEY: &str = "exportSettings";

/// Every field optional, so a document written before a field existed still reads.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Wire {
    #[serde(rename = "compressTIFF", skip_serializing_if = "Option::is_none")]
    compress_tiff: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hdr: Option<bool>,
    #[serde(rename = "jpegQuality", skip_serializing_if = "Option::is_none")]
    jpeg_quality: Option<f64>,
    #[serde(rename = "resizeEdge", skip_serializing_if = "Option::is_none")]
    resize_edge: Option<String>,
    #[serde(rename = "resizePixels", skip_serializing_if = "Option::is_none")]
    resize_pixels: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    space: Option<String>,
}

impl Settings {
    /// The preference's text as `JSONEncoder` with `.sortedKeys` writes it: keys sorted, `/`
    /// escaped, a whole number without its `.0`.
    pub fn raw_value(&self) -> String {
        let (edge, pixels) = match self.resize {
            Resize::Full => ("full", None),
            Resize::LongestEdge(n) => ("long", Some(n)),
            Resize::ShortestEdge(n) => ("short", Some(n)),
        };
        let mut fields = vec![
            ("compressTIFF", self.compress_tiff.to_string()),
            ("format", string(self.format.key())),
            ("hdr", self.hdr.to_string()),
            ("jpegQuality", number(self.jpeg_quality)),
            ("resizeEdge", string(edge)),
        ];
        if let Some(n) = pixels {
            fields.push(("resizePixels", n.to_string()));
        }
        fields.push(("space", string(&self.space.key())));
        let body: Vec<String> = fields.iter().map(|(k, v)| format!("\"{k}\":{v}")).collect();
        format!("{{{}}}", body.join(","))
    }

    /// Decoded tolerantly: an absent or unknown key reads its default; text that is not the
    /// document at all reads as absent.
    pub fn from_raw_value(raw: &str) -> Option<Settings> {
        let wire: Wire = serde_json::from_str(raw).ok()?;
        let mut out = Settings::default();
        if let Some(value) = wire.format.as_deref().and_then(|k| Format::ALL.into_iter().find(|f| f.key() == k)) {
            out.format = value;
        }
        if let Some(value) = wire.space.as_deref().and_then(Space::from_key) {
            out.space = value;
        }
        if out.space.needs_sixteen_bits() {
            out.format = Format::Tiff16;
        }
        if let Some(value) = wire.compress_tiff {
            out.compress_tiff = value;
        }
        if let Some(value) = wire.jpeg_quality {
            out.jpeg_quality = value;
        }
        if let Some(value) = wire.hdr {
            out.hdr = value;
        }
        match (wire.resize_edge.as_deref(), wire.resize_pixels) {
            (Some("long"), Some(n)) if n > 0 => out.resize = Resize::LongestEdge(n),
            (Some("short"), Some(n)) if n > 0 => out.resize = Resize::ShortestEdge(n),
            _ => {}
        }
        Some(out)
    }
}

fn string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default().replace('/', "\\/")
}

/// Swift's shortest digits; a whole number loses its `.0`, as `JSONEncoder` writes it.
fn number(v: f64) -> String {
    if v.is_finite() && v.fract() == 0.0 && v.abs() < 1e15 { format!("{}", v as i64) } else { format!("{v}") }
}
