//! The grain's colour: how far it deepens the photo's own hue where it darkens, on a straight line along the
//! picture's tone, from its black to its white.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// Each end's colour: a grain `d` stops darker raises each channel's ratio to the luma to the power
/// 2^(colour·d), 0 a monochrome grain; a grey stays grey.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct GrainColour {
    pub black: f32,
    pub white: f32,
}

impl GrainColour {
    pub const RANGE: (f32, f32) = (0.0, 3.0);
    /// Skin's chroma in the shadows about 1.25 times its own at Amount's top, and monochrome at the white.
    pub const REST: GrainColour = GrainColour { black: 2.0, white: 0.0 };

    pub fn rests(&self) -> bool {
        *self == Self::REST
    }
}

impl Default for GrainColour {
    fn default() -> Self {
        Self::REST
    }
}

/// Absent keys, and any value that is no finite number, read as rest; the others are held in `RANGE`.
impl<'de> Deserialize<'de> for GrainColour {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(d)?;
        let read = |key: &str, rest: f32| {
            let v = value.get(key).and_then(Value::as_f64).map(|v| v as f32).filter(|v| v.is_finite());
            v.map_or(rest, |v| v.clamp(Self::RANGE.0, Self::RANGE.1))
        };
        Ok(GrainColour { black: read("black", Self::REST.black), white: read("white", Self::REST.white) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> GrainColour {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn a_moved_colour_round_trips_to_the_bit_and_a_stray_one_reads_as_rest_or_held() {
        let moved = GrainColour { black: 1.234_567_8, white: 0.1 };
        let again = read(&serde_json::to_string(&moved).unwrap());
        assert_eq!((again.black.to_bits(), again.white.to_bits()), (moved.black.to_bits(), moved.white.to_bits()));
        assert_eq!(read("{}"), GrainColour::REST);
        assert_eq!(read(r#"{"black": null, "white": "x"}"#), GrainColour::REST);
        assert_eq!(read("[1, 2]"), GrainColour::REST);
        assert_eq!(read(r#"{"black": 9, "white": -1}"#), GrainColour { black: 3.0, white: 0.0 });
        // Twin: serde's own reader refuses the stray value the tolerant one reads as rest.
        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct Plain {
            black: f32,
        }
        assert!(serde_json::from_str::<Plain>(r#"{"black": "x"}"#).is_err());
    }

    #[test]
    fn only_a_moved_colour_rests_off() {
        assert!(GrainColour::default().rests());
        // Twin: either end moved.
        assert!(!GrainColour { black: 1.5, ..GrainColour::REST }.rests() && !GrainColour { white: 0.5, ..GrainColour::REST }.rests());
    }
}
