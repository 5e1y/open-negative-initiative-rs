//! A frame's EXIF line, as an export writes it and as its sidecar keeps what a hand set of it.

use serde::{Deserialize, Serialize};

/// A frame's EXIF line as an export writes it, an empty field writing no tag; every key is required.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ExifMetadata {
    pub camera: String,
    pub lens: String,
    pub film: String,
    pub iso: String,
    /// EXIF form, `YYYY:MM:DD HH:MM:SS`, settled as it is read: the file writes it as it stands.
    #[serde(deserialize_with = "settled_date")]
    pub date: String,
    pub artist: String,
    pub copyright: String,
}

fn settled_date<'de, D: serde::Deserializer<'de>>(stored: D) -> Result<String, D::Error> {
    String::deserialize(stored).map(|d| crate::exif::settled_date(&d))
}

/// A kept date that reads as no date is a date cleared, never one left to derive.
fn settled_typed_date<'de, D: serde::Deserializer<'de>>(stored: D) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(stored).map(|d| d.map(|d| crate::exif::settled_date(&d)))
}

impl ExifMetadata {
    pub fn is_empty(&self) -> bool {
        *self == ExifMetadata::default()
    }

    /// What a hand set on this line: a field equal to any of `derived`'s is left to derive.
    pub fn beyond(&self, derived: &[&ExifMetadata]) -> TypedExif {
        let set = |get: fn(&ExifMetadata) -> &String| Some(get(self)).filter(|mine| derived.iter().all(|d| get(d) != *mine)).cloned();
        TypedExif {
            camera: set(|m| &m.camera),
            lens: set(|m| &m.lens),
            film: set(|m| &m.film),
            iso: set(|m| &m.iso),
            date: set(|m| &m.date),
            artist: set(|m| &m.artist),
            copyright: set(|m| &m.copyright),
        }
    }
}

/// The EXIF fields a hand set, as the sidecar keeps them: an absent field derives at each opening, an
/// empty one was cleared, so a block written whole by an older build reads as set in full.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct TypedExif {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lens: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub film: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iso: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "settled_typed_date")]
    pub date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copyright: Option<String>,
}

impl TypedExif {
    pub fn is_empty(&self) -> bool {
        *self == TypedExif::default()
    }

    /// The line shown: each field set here over `derived`'s.
    pub fn over(&self, derived: &ExifMetadata) -> ExifMetadata {
        let f = |mine: &Option<String>, theirs: &String| mine.as_ref().unwrap_or(theirs).clone();
        ExifMetadata {
            camera: f(&self.camera, &derived.camera),
            lens: f(&self.lens, &derived.lens),
            film: f(&self.film, &derived.film),
            iso: f(&self.iso, &derived.iso),
            date: f(&self.date, &derived.date),
            artist: f(&self.artist, &derived.artist),
            copyright: f(&self.copyright, &derived.copyright),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ExifMetadata, TypedExif};

    #[test]
    fn a_line_kept_beyond_its_derived_one_comes_back_over_it() {
        let derived = ExifMetadata { camera: "Nikon FM2".into(), film: "Kodak Portra 400".into(), artist: "A".into(), ..ExifMetadata::default() };
        let shown = ExifMetadata { lens: "50mm".into(), artist: "B".into(), film: String::new(), ..derived.clone() };
        let kept = shown.beyond(&[&derived]);
        let expected = TypedExif { lens: Some("50mm".into()), artist: Some("B".into()), film: Some(String::new()), ..TypedExif::default() };
        assert_eq!(kept, expected, "only what differs is kept, a cleared film as cleared");
        assert_eq!(kept.over(&derived), shown, "the film cleared by hand stays cleared");
        // Twin: without its empty key the cleared film derives again.
        assert_ne!(TypedExif { film: None, ..kept }.over(&derived), shown);
    }
}
