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
}

impl ConversionMode {
    pub const ALL: [ConversionMode; 3] = [Self::Negative, Self::Positive, Self::Monochrome];

    pub fn raw_value(self) -> &'static str {
        match self {
            Self::Negative => "negative",
            Self::Positive => "positive",
            Self::Monochrome => "monochrome",
        }
    }

    pub fn from_raw(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.raw_value() == raw)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Negative => "Negative",
            Self::Positive => "Positive",
            Self::Monochrome => "B&W",
        }
    }

    pub fn invert_flag(self) -> f32 {
        if self == Self::Positive { 0.0 } else { 1.0 }
    }

    /// In negative a gain lowers the output, in positive it raises it, as a filter does.
    pub fn inverts_gain_sense(self) -> bool {
        self != Self::Positive
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
    pub fn shortcut(self) -> char {
        match self {
            Self::None => '0',
            Self::Bronze => '1',
            Self::Silver => '2',
            Self::Gold => '3',
        }
    }

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
    }

    #[test]
    fn a_medal_is_written_by_name_and_an_unknown_one_refused() {
        assert_eq!(serde_json::to_string(&Medal::Silver).unwrap(), "\"silver\"");
        assert!(serde_json::from_str::<Medal>("\"platine\"").is_err());
        assert_eq!(serde_json::from_str::<Medal>("\"gold\"").unwrap(), Medal::Gold);
    }
}
