//! Lighting's readers: tolerant of absent keys and of values past their tracks.

use super::{Print, Tone, V3};
use crate::tolerant;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

impl<'de> Deserialize<'de> for Tone {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let map = tolerant::object(Value::deserialize(d)?, "Tone")?;
        let f = |key: &str| -> Result<f32, D::Error> { Ok(tolerant::opt::<f32, D::Error>(&map, key)?.unwrap_or(0.0)) };
        Ok(Tone {
            exposure: f("exposure")?,
            contrast: f("contrast")?,
            blacks: f("blacks")?,
            shadows: f("shadows")?,
            highlights: f("highlights")?,
            whites: f("whites")?,
            mids: f("mids")?,
        })
    }
}

/// Tolerant of absent keys; a contrast or black past the widest track lands on its stop, a channel
/// keeps its exposure alone.
impl<'de> Deserialize<'de> for Print {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let map = tolerant::object(Value::deserialize(d)?, "Print")?;
        let f = |key: &str, default: f32| -> Result<f32, D::Error> {
            Ok(tolerant::opt::<f32, D::Error>(&map, key)?.unwrap_or(default))
        };
        let t = |key: &str| -> Result<Option<Tone>, D::Error> { tolerant::opt::<Tone, D::Error>(&map, key) };
        let channels = |key: &str, (low, high): (f32, f32)| -> Result<V3, D::Error> {
            Ok(tolerant::opt::<V3, D::Error>(&map, key)?.unwrap_or_default().map(|p| p.clamp(low, high)))
        };
        let filter = |tone: Tone| Tone::exposure(tone.exposure.clamp(Print::FILTER_RANGE.0, Print::FILTER_RANGE.1));
        Ok(Print {
            exposure: f("exposure", 0.0)?,
            contrast: f("contrast", 0.0)?
                .clamp(Print::MONOCHROME_CONTRAST_RANGE.0, Print::MONOCHROME_CONTRAST_RANGE.1),
            shadows: f("shadows", 0.0)?,
            highlights: f("highlights", 0.0)?,
            red: filter(t("red")?.unwrap_or_default()),
            green: filter(t("green")?.unwrap_or_default()),
            blue: filter(t("blue")?.unwrap_or_default()),
            black_point: f("blackPoint", Print::RESTING_BLACK_POINT)?
                .clamp(Print::MONOCHROME_POINT_RANGE.0, Print::MONOCHROME_POINT_RANGE.1),
            white_point: f("whitePoint", 0.0)?,
            channel_white: channels("channelWhite", Print::POINT_RANGE)?,
            channel_black: channels("channelBlack", Print::BLACK_POINT_RANGE)?,
            channel_mid: channels("channelMid", Print::ZONE_RANGE)?,
            channel_contrast: channels("channelContrast", Print::CONTRAST_RANGE)?,
            colour_compensation: tolerant::opt::<bool, D::Error>(&map, "colourCompensation")?.unwrap_or(true),
            brightness: f("brightness", 0.0)?.clamp(Print::BRIGHTNESS_RANGE.0, Print::BRIGHTNESS_RANGE.1),
            print_lift: tolerant::opt::<bool, D::Error>(&map, "printLift")?.unwrap_or(false),
            projected_blacks: tolerant::opt::<bool, D::Error>(&map, "projectedBlacks")?.unwrap_or(false),
            bypassed: false,
        })
    }
}
