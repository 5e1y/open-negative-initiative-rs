//! An identifier, hyphenated and upper case: compared as text across a roll's frames and in the index.

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(pub uuid::Uuid);

impl Id {
    pub fn new() -> Self {
        Id(uuid::Uuid::new_v4())
    }

    /// An identity drawn from bytes, so one file without an id reads as the same preset every time.
    pub fn of_bytes(bytes: &[u8]) -> Self {
        let hash = bytes.iter().fold(0x6c62_272e_07bb_0142_62b8_2175_6295_c58d_u128, |h, b| (h ^ u128::from(*b)).wrapping_mul(0x0000_0000_0100_0000_0000_0000_0000_013b));
        Id(uuid::Uuid::from_u128(hash))
    }

    /// `UUID(uuidString:)`: the 36-character hyphenated form, either case.
    pub fn parse(text: &str) -> Option<Self> {
        if text.len() != 36 {
            return None;
        }
        uuid::Uuid::try_parse(text).ok().map(Id)
    }
}

impl Default for Id {
    fn default() -> Self {
        Id::new()
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.as_hyphenated().encode_upper(&mut uuid::Uuid::encode_buffer()))
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl Serialize for Id {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Id::parse(&text).ok_or_else(|| de::Error::custom(format!("Attempted to decode UUID from invalid UUID string: {text}")))
    }
}
