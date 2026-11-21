//! The serialised enumerations named by their raw value: what the source is, which face renders,
//! the scanner machine, a file's declared encoding, a frame's label.

use serde::{Deserialize, Deserializer, Serialize};

/// What the source is. Set by hand only, never guessed from pixels, EXIF, filename or history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConversionMode {
    #[default]
    Negative,
    Positive,
    Monochrome,
    /// A digital photo: its scene shot on a virtual film, whose negative is read as a scanned one is.
    Digital,
}

impl ConversionMode {
    pub const ALL: [ConversionMode; 4] = [Self::Negative, Self::Positive, Self::Monochrome, Self::Digital];

    pub fn raw_value(self) -> &'static str {
        match self {
            Self::Negative => "negative",
            Self::Positive => "positive",
            Self::Monochrome => "monochrome",
            Self::Digital => "digital",
        }
    }

    pub fn from_raw(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.raw_value() == raw)
    }

    pub fn invert_flag(self) -> f32 {
        if self.reads_positive() { 0.0 } else { 1.0 }
    }

    /// In negative a gain lowers the output, in positive it raises it, as a filter does.
    pub fn inverts_gain_sense(self) -> bool {
        !self.reads_positive()
    }

    /// Read on the positive axis, never inverted: a slide.
    pub fn reads_positive(self) -> bool {
        self == Self::Positive
    }

    /// A camera's own photo: its lens and its film's grain and halation are its own, and no copy stand
    /// lit it, so it takes neither a flat-field nor a roll's capture exposure.
    pub fn is_digital(self) -> bool {
        self == Self::Digital
    }

    /// Prepare the Roll measures its reference: a slide is balanced on a click instead, and a digital
    /// photo stands on its film's own reference, so either roll is its folder.
    pub fn is_measured(self) -> bool {
        matches!(self, Self::Negative | Self::Monochrome)
    }

    /// A slide: its RAW decoded at 5500 K and its roll prepared on a white it clicks.
    pub fn is_slide(self) -> bool {
        self == Self::Positive
    }

    pub fn is_monochrome(self) -> bool {
        self == Self::Monochrome
    }
}

/// An unknown value reads as a negative: a newer sidecar must open read-only, not fail to decode.
impl<'de> Deserialize<'de> for ConversionMode {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Ok(Self::from_raw(&raw).unwrap_or(Self::Negative))
    }
}

/// Which edit renders the photograph: Pro's chain, or the measured scanner's keypad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Face {
    #[default]
    Pro,
    Scanner,
}

/// The table the Scanner face renders through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScannerMachine {
    #[default]
    Oni,
    Frontier,
}

impl ScannerMachine {
    pub fn raw_value(self) -> &'static str {
        match self {
            Self::Oni => "oni",
            Self::Frontier => "frontier",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Oni => "ONI",
            Self::Frontier => "Frontier",
        }
    }
}

/// How a file naming no colour profile is read, as the hand says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceEncoding {
    #[serde(rename = "srgb")]
    Srgb,
    #[serde(rename = "linear")]
    Linear,
    #[serde(rename = "displayP3")]
    DisplayP3,
}

impl SourceEncoding {
    pub fn label(self) -> &'static str {
        match self {
            Self::Srgb => "sRGB",
            Self::Linear => "Linear",
            Self::DisplayP3 => "Display P3",
        }
    }
}

/// A frame's label; an enum makes the four exclusive by construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Medal {
    #[default]
    None,
    Bronze,
    Silver,
    Gold,
}

impl Medal {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Bronze => "Bronze",
            Self::Silver => "Silver",
            Self::Gold => "Gold",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_mode_reads_negative_and_a_known_one_itself() {
        let unknown: ConversionMode = serde_json::from_str("\"sepia\"").unwrap();
        assert_eq!(unknown, ConversionMode::Negative);
        let known: ConversionMode = serde_json::from_str("\"positive\"").unwrap();
        assert_eq!(known, ConversionMode::Positive);
        let digital: ConversionMode = serde_json::from_str("\"digital\"").unwrap();
        assert_eq!((digital, serde_json::to_string(&digital).unwrap().as_str()), (ConversionMode::Digital, "\"digital\""));
    }

    #[test]
    fn a_digital_photos_virtual_negative_is_read_as_a_negative_is() {
        let d = ConversionMode::Digital;
        assert!(!d.reads_positive() && d.is_digital() && !d.is_measured() && d.invert_flag() == 1.0 && d.inverts_gain_sense());
        // Twin: a slide is read on the positive axis, never measured; a negative is measured.
        assert!(ConversionMode::Positive.reads_positive() && !ConversionMode::Positive.is_measured() && ConversionMode::Positive.invert_flag() == 0.0);
        assert!(ConversionMode::Negative.is_measured() && !ConversionMode::Negative.is_digital());
    }

    #[test]
    fn a_medal_is_written_by_name_and_an_unknown_one_refused() {
        assert_eq!(serde_json::to_string(&Medal::Silver).unwrap(), "\"silver\"");
        assert!(serde_json::from_str::<Medal>("\"platine\"").is_err());
        assert_eq!(serde_json::from_str::<Medal>("\"gold\"").unwrap(), Medal::Gold);
    }
}
