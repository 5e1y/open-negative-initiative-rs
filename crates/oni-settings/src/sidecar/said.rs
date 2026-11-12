//! A sidecar's reading as text, each value under its Rust name: the corpus compares readings by it
//! across formats, so nothing in it depends on a stored key or on the stored spelling of a number.

use super::{ExifMetadata, Loaded};
use std::fmt::Write;
use std::path::Path;

impl Loaded {
    /// The envelope, the EXIF line over an empty derived one, the draft with its base by file name,
    /// then every setting.
    pub fn said(&self) -> String {
        let mut out = String::new();
        let line = self.exif.clone().unwrap_or_default().over(&ExifMetadata::default());
        let _ = writeln!(out, "writable: {}", self.writable);
        let _ = writeln!(out, "label: {:?}", self.label);
        let _ = writeln!(out, "imported: {:?}", self.imported_at);
        let _ = writeln!(out, "trashed: {:?}", self.trashed_at);
        let _ = writeln!(out, "exif: {line:#?}");
        match &self.roll_draft {
            None => out.push_str("draft: none\n"),
            Some(d) => {
                let base = d.base.as_ref().map(|b| {
                    let file = Path::new(&b.path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    (file, b.x, b.y, b.radius)
                });
                let _ = writeln!(out, "draft: {} {} {:?} {}", d.id, d.step, d.film, d.capture);
                let _ = writeln!(out, "  passes: {:?} {:?}", d.pass_order, d.pass_offset);
                let _ = writeln!(out, "  flat field: {:?}", d.flat_field);
                let _ = writeln!(out, "  base: {base:?}");
                let _ = writeln!(out, "  window: {:?}", d.window);
                let _ = writeln!(out, "  ticked {} flagged {}", d.ticked, d.flagged);
            }
        }
        let _ = writeln!(out, "settings: {:#?}", self.settings);
        out
    }
}
