//! What belongs to the frame itself: its framing, the rig's flat-field, its dust corrections, a
//! contact sheet's grid, the three passes of a narrowband capture, the roll's measured window.

use crate::cg::{Point, Rect, Size};
use crate::id::Id;
use crate::tolerant;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AspectRatio {
    #[serde(rename = "3:2")]
    ThreeTwo,
    #[serde(rename = "4:3")]
    FourThree,
    #[serde(rename = "1:1")]
    Square,
    #[serde(rename = "6:7")]
    SixSeven,
    #[serde(rename = "5:4")]
    FiveFour,
}

impl AspectRatio {
    pub fn value(self) -> f64 {
        match self {
            Self::ThreeTwo => 3.0 / 2.0,
            Self::FourThree => 4.0 / 3.0,
            Self::Square => 1.0,
            Self::SixSeven => 6.0 / 7.0,
            Self::FiveFour => 5.0 / 4.0,
        }
    }
}

/// Stage 0: straightening and cropping, in fractions of the upright oriented box.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Geometry {
    #[serde(rename = "quarterTurns")]
    pub quarter_turns: i64,
    pub angle: f32,
    pub crop: Rect,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub ratio: Option<AspectRatio>,
    pub portrait: bool,
    pub mirrored: bool,
    #[serde(rename = "mirroredVertically")]
    pub mirrored_vertically: bool,
    /// Where the balance and the input histogram read; `None` follows the crop.
    #[serde(rename = "measureWindow", skip_serializing_if = "Option::is_none", default)]
    pub measure_window: Option<Rect>,
    /// A display flag for the length of a crop gesture, never serialised.
    #[serde(skip)]
    pub crop_bypassed: bool,
}

impl Default for Geometry {
    fn default() -> Self {
        Geometry {
            quarter_turns: 0,
            angle: 0.0,
            crop: Rect::UNIT,
            ratio: None,
            portrait: false,
            mirrored: false,
            mirrored_vertically: false,
            measure_window: None,
            crop_bypassed: false,
        }
    }
}

impl Geometry {
    pub const ANGLE_RANGE: (f32, f32) = (-8.0, 8.0);

    pub fn is_neutral(&self) -> bool {
        self.angle == 0.0
            && self.quarter_turns == 0
            && !self.mirrored
            && !self.mirrored_vertically
            && self.crop == Rect::UNIT
    }

    /// The reset keeps the measurement window, which no crop gesture moves.
    pub fn reset_keeping(current: &Geometry) -> Geometry {
        Geometry { measure_window: current.measure_window, ..Geometry::default() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum FlatFieldMask {
    #[default]
    #[serde(rename = "None")]
    None,
    #[serde(rename = "Valoi Easy35 V1")]
    ValoiEasy35V1,
    #[serde(rename = "Custom")]
    Custom,
}

/// Divides the frame by the light the panel put on it, before stage 0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlatField {
    pub mask: FlatFieldMask,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub custom: Option<String>,
    pub amount: f32,
    pub temperature: f32,
    pub tint: f32,
}

impl Default for FlatField {
    fn default() -> Self {
        FlatField { mask: FlatFieldMask::None, custom: None, amount: 1.0, temperature: 0.0, tint: 0.0 }
    }
}

impl FlatField {
    pub const COLOUR_RANGE: (f32, f32) = (-1.0, 1.0);
    pub const AMOUNT_RANGE: (f32, f32) = (0.0, 1.0);

    pub fn is_neutral(&self) -> bool {
        self.mask == FlatFieldMask::None
            || self.amount <= 0.0
            || (self.mask == FlatFieldMask::Custom && self.custom.is_none())
    }
}

/// A painted dust or scratch stroke, in fractions of the cropped, oriented frame, origin top-left.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Correction {
    pub id: Id,
    pub stroke: Vec<Point>,
    /// The brush width at the time of the stroke, in full-resolution pixels.
    pub radius: f64,
}

impl Correction {
    pub const DEFAULT_RADIUS: f64 = 12.0;
    pub const RADIUS_RANGE: (f64, f64) = (4.0, 120.0);

    pub fn new(stroke: Vec<Point>) -> Self {
        Correction { id: Id::new(), stroke, radius: Self::DEFAULT_RADIUS }
    }
}

/// Every key may be absent: a missing id is a fresh one, a missing radius the default brush.
impl<'de> Deserialize<'de> for Correction {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let map = tolerant::object(Value::deserialize(d)?, "Correction")?;
        Ok(Correction {
            id: tolerant::opt::<Id, D::Error>(&map, "id")?.unwrap_or_default(),
            stroke: tolerant::opt::<Vec<Point>, D::Error>(&map, "stroke")?.unwrap_or_default(),
            radius: tolerant::opt::<f64, D::Error>(&map, "radius")?.unwrap_or(Self::DEFAULT_RADIUS),
        })
    }
}

/// A contact sheet's grid: where its frames lie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContactSheetGrid {
    pub columns: i64,
    pub rows: i64,
    pub frames: i64,
    pub tile: Size,
    pub margin: f64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub windows: Option<Vec<Rect>>,
}

/// A pass's magnification and turn about the frame's centre, then its offset in frame widths.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Fit {
    pub scale: f64,
    pub rotation: f64,
    pub dx: f64,
    pub dy: f64,
}

impl Default for Fit {
    fn default() -> Self {
        Fit { scale: 1.0, rotation: 0.0, dx: 0.0, dy: 0.0 }
    }
}

impl<'de> Deserialize<'de> for Fit {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let map = tolerant::object(Value::deserialize(d)?, "Fit")?;
        let f = |key: &str, default: f64| -> Result<f64, D::Error> {
            Ok(tolerant::opt::<f64, D::Error>(&map, key)?.unwrap_or(default))
        };
        Ok(Fit { scale: f("scale", 1.0)?, rotation: f("rotation", 0.0)?, dx: f("dx", 0.0)?, dy: f("dy", 0.0)? })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Registration {
    pub red: Fit,
    pub blue: Fit,
}

/// A frame exposed three times under narrowband light, one file per pass; the lead names the three.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RgbPasses {
    pub red: String,
    pub green: String,
    pub blue: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub registration: Option<Registration>,
}

impl RgbPasses {
    pub fn name(&self, channel: Channel) -> &str {
        match channel {
            Channel::Red => &self.red,
            Channel::Green => &self.green,
            Channel::Blue => &self.blue,
        }
    }
}

/// A pass's band, red, green then blue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Channel {
    Red,
    Green,
    Blue,
}

impl Channel {
    pub const ALL: [Channel; 3] = [Channel::Red, Channel::Green, Channel::Blue];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn letter(self) -> &'static str {
        ["R", "G", "B"][self.index()]
    }
}

/// The order a rig takes its three exposures in: read off the first triplet on a click, moved by the hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PassOrder {
    Bgr,
    Brg,
    Gbr,
    Grb,
    Rbg,
    Rgb,
}

impl PassOrder {
    pub const ALL: [PassOrder; 6] = [PassOrder::Bgr, PassOrder::Brg, PassOrder::Gbr, PassOrder::Grb, PassOrder::Rbg, PassOrder::Rgb];

    /// The bands in capture order.
    pub fn channels(self) -> [Channel; 3] {
        use Channel::{Blue as B, Green as G, Red as R};
        match self {
            PassOrder::Bgr => [B, G, R],
            PassOrder::Brg => [B, R, G],
            PassOrder::Gbr => [G, B, R],
            PassOrder::Grb => [G, R, B],
            PassOrder::Rbg => [R, B, G],
            PassOrder::Rgb => [R, G, B],
        }
    }

    /// `label`: the letters spaced, "B G R".
    pub fn label(self) -> String {
        self.channels().map(Channel::letter).join(" ")
    }

    /// `init?(_ channels:)`: `None` for two of one band.
    pub fn from_channels(channels: &[Channel]) -> Option<PassOrder> {
        PassOrder::ALL.into_iter().find(|o| o.channels().as_slice() == channels)
    }

    /// `init?(fitting:)`: the order whose bands carry most of each file's own light, the shares in
    /// capture order; under narrowband light a pass's band holds most of its signal.
    pub fn fitting(shares: &[[f64; 3]]) -> Option<PassOrder> {
        if shares.len() != 3 {
            return None;
        }
        let score = |o: PassOrder| shares.iter().zip(o.channels()).map(|(s, c)| s[c.index()]).sum::<f64>();
        // `max(by:)` keeps the first of equal scores.
        PassOrder::ALL.into_iter().fold(None, |best: Option<PassOrder>, o| match best {
            Some(b) if score(b) >= score(o) => Some(b),
            _ => Some(o),
        })
    }
}

/// The per-channel window a whole roll was measured on, kept by every frame measured with it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RollReference {
    pub id: Id,
    pub stops: [f32; 3],
    pub black: [f32; 3],
    pub white: [f32; 3],
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub base: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub median: Option<[f32; 3]>,
}

impl RollReference {
    pub const FACTOR_RANGE: (f32, f32) = (0.25, 4.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_correction_missing_its_radius_reads_the_default_brush() {
        let full = Correction { id: Id::new(), stroke: vec![Point::new(0.12, 0.34), Point::new(0.56, 0.78)], radius: 9.5 };
        let mut value = serde_json::to_value(&full).unwrap();
        assert_eq!(serde_json::from_value::<Correction>(value.clone()).unwrap(), full);
        value.as_object_mut().unwrap().remove("radius");
        let read: Correction = serde_json::from_value(value).unwrap();
        assert_eq!(read.radius, Correction::DEFAULT_RADIUS);
        assert_eq!((read.id, &read.stroke), (full.id, &full.stroke));
        // Twin: a radius actually written is not the default.
        assert_ne!(full.radius, Correction::DEFAULT_RADIUS);
    }

    /// `RGBPasses.selfCheck`'s pass orders.
    #[test]
    fn the_pass_order_is_the_one_carrying_most_light_in_capture_order() {
        let lit = [[0.05, 0.69, 0.26], [0.94, 0.04, 0.02], [0.10, 0.20, 0.70]];
        assert_eq!(PassOrder::fitting(&lit), Some(PassOrder::Grb));
        // Two files both leaning red still read as one order, the one carrying the most light.
        let crossed = [[0.5, 0.4, 0.1], [0.6, 0.3, 0.1], [0.1, 0.2, 0.7]];
        assert_eq!(PassOrder::fitting(&crossed), Some(PassOrder::Grb));
        // Twin: the same files taken in another order read as that order.
        assert_eq!(PassOrder::fitting(&[lit[1], lit[0], lit[2]]), Some(PassOrder::Rgb));
        assert_eq!(PassOrder::fitting(&lit[..2]), None);
        assert!(PassOrder::ALL.iter().all(|o| PassOrder::from_channels(&o.channels()) == Some(*o)));
        assert_eq!(PassOrder::from_channels(&[Channel::Red, Channel::Red, Channel::Blue]), None, "two of one band are no order");
        assert_eq!(PassOrder::Bgr.label(), "B G R");
        assert_eq!(serde_json::to_string(&PassOrder::Grb).unwrap(), "\"grb\"");
    }

    #[test]
    fn a_fit_missing_keys_reads_the_identity() {
        let read: Fit = serde_json::from_str(r#"{"dx":0.25}"#).unwrap();
        assert_eq!(read, Fit { dx: 0.25, ..Fit::default() });
        assert_ne!(read, Fit::default());
    }

    #[test]
    fn the_crop_display_flag_never_enters_the_document() {
        let bypassed = Geometry { crop_bypassed: true, ..Geometry::default() };
        let text = serde_json::to_string(&bypassed).unwrap();
        assert!(!text.contains("cropBypassed") && !text.contains("crop_bypassed"));
        let back: Geometry = serde_json::from_str(&text).unwrap();
        assert!(!back.crop_bypassed);
        assert!(text.contains("\"crop\":[[0.0,0.0],[1.0,1.0]]"));
    }
}
