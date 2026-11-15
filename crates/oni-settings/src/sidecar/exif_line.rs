//! A frame's EXIF line, as an export writes it and as its sidecar keeps what a hand set of it.

use serde::{Deserialize, Serialize};

/// A frame's EXIF line as an export writes it, an empty field writing no tag; every typed key is required.
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
    /// The gear's own facts, filled from the Kit at export and never typed, so no line keeps them.
    #[serde(default)]
    pub camera_make: String,
    #[serde(default)]
    pub lens_make: String,
    #[serde(default)]
    pub lens_specification: Option<LensSpecification>,
}

/// Exif's LensSpecification in tenths (70 mm is 700, f/5.6 is 56); a zero states "unknown".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct LensSpecification {
    pub focal_min: u32,
    pub focal_max: u32,
    pub aperture_at_min: u32,
    pub aperture_at_max: u32,
}

impl LensSpecification {
    /// Millimetres and f-numbers rounded to tenths; an unknown aperture, or a value that is not a
    /// positive number, is zero.
    pub fn new(focal_min: f64, focal_max: f64, aperture_at_min: Option<f64>, aperture_at_max: Option<f64>) -> Self {
        let tenths = |v: f64| (v * 10.0).round() as u32;
        Self {
            focal_min: tenths(focal_min),
            focal_max: tenths(focal_max),
            aperture_at_min: aperture_at_min.map_or(0, tenths),
            aperture_at_max: aperture_at_max.map_or(0, tenths),
        }
    }

    /// A single focal length: a prime's, which also states the frame's FocalLength.
    pub fn prime(&self) -> Option<u32> {
        (self.focal_min == self.focal_max && self.focal_min > 0).then_some(self.focal_min)
    }
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
    pub fn beyond(&self, derived: &[&ExifMetadata]) -> ExifLine {
        let set = |get: fn(&ExifMetadata) -> &String| Some(get(self)).filter(|mine| derived.iter().all(|d| get(d) != *mine)).cloned();
        ExifLine {
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
pub struct ExifLine {
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

impl ExifLine {
    pub fn is_empty(&self) -> bool {
        *self == ExifLine::default()
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
            ..ExifMetadata::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ExifLine, ExifMetadata, LensSpecification};

    #[test]
    fn a_line_kept_beyond_its_derived_one_comes_back_over_it() {
        let derived = ExifMetadata { camera: "Nikon FM2".into(), film: "Kodak Portra 400".into(), artist: "A".into(), ..ExifMetadata::default() };
        let shown = ExifMetadata { lens: "50mm".into(), artist: "B".into(), film: String::new(), ..derived.clone() };
        let kept = shown.beyond(&[&derived]);
        let expected = ExifLine { lens: Some("50mm".into()), artist: Some("B".into()), film: Some(String::new()), ..ExifLine::default() };
        assert_eq!(kept, expected, "only what differs is kept, a cleared film as cleared");
        assert_eq!(kept.over(&derived), shown, "the film cleared by hand stays cleared");
        // Twin: without its empty key the cleared film derives again.
        assert_ne!(ExifLine { film: None, ..kept }.over(&derived), shown);
    }

    fn kit() -> ExifMetadata {
        ExifMetadata {
            camera: "Nikon FM2".into(),
            lens: "Nikkor 50".into(),
            camera_make: "Nikon".into(),
            lens_make: "Nikon".into(),
            lens_specification: Some(LensSpecification::new(50.0, 50.0, Some(1.4), Some(1.4))),
            ..ExifMetadata::default()
        }
    }

    #[test]
    fn a_stored_line_without_the_gear_s_facts_reads_with_them_empty() {
        let stored = r#"{"camera":"Nikon FM2","lens":"","film":"","iso":"","date":"","artist":"","copyright":""}"#;
        let read: ExifMetadata = serde_json::from_str(stored).unwrap();
        assert_eq!(read, ExifMetadata { camera: "Nikon FM2".into(), ..ExifMetadata::default() });
        let back: ExifMetadata = serde_json::from_str(&serde_json::to_string(&kit()).unwrap()).unwrap();
        assert_eq!(back, kit(), "the facts travel whole once written");
        // Twin: a typed key is still required, so a line cut short is refused.
        assert!(serde_json::from_str::<ExifMetadata>(r#"{"camera":"Nikon FM2"}"#).is_err());
    }

    #[test]
    fn a_kept_line_never_carries_the_gear_s_facts() {
        let derived = ExifMetadata { camera: "Nikon FM2".into(), ..ExifMetadata::default() };
        let kept = kit().beyond(&[&derived]);
        assert_eq!(kept, ExifLine { lens: Some("Nikkor 50".into()), ..ExifLine::default() });
        let shown = kept.over(&kit());
        assert_eq!((shown.camera_make.as_str(), shown.lens_make.as_str(), shown.lens_specification), ("", "", None));
        // Twin: the derived line's own typed fields do come through.
        assert_eq!(shown.camera, "Nikon FM2");
    }

    #[test]
    fn a_specification_rounds_to_tenths_and_an_unknown_aperture_is_zero() {
        let zoom = LensSpecification::new(70.0, 300.04, Some(4.0), Some(5.6));
        assert_eq!(zoom, LensSpecification { focal_min: 700, focal_max: 3000, aperture_at_min: 40, aperture_at_max: 56 });
        assert_eq!(zoom.prime(), None);
        assert_eq!(LensSpecification::new(50.0, 50.0, Some(1.4), None).aperture_at_max, 0);
        assert_eq!(LensSpecification::new(50.0, 50.0, Some(1.4), None).prime(), Some(500));
        // Twin: a 0.06 mm difference is a tenth apart, so the zoom stays a zoom.
        assert_ne!(LensSpecification::new(50.0, 50.06, None, None).prime(), Some(500));
    }
}
