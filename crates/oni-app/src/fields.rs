//! Every value the panel draws: its label, travel, rest, readout and the store knob it moves, copied
//! from the Swift panes (`ColorPane`, `PrintPanel`, `SettingsPane`, `DetailsPane`, …).

use oni_core::knob::{CHROMA_FORCE_RANGE, SAUCE_RANGE};
use oni_settings::blocks::{Bloom, ChromaDenoise, Gains, Grain, Halation, PaperCorrection, Sharpen, Texture, Vignette};
use oni_settings::frame::FlatField;
use oni_settings::lens::LensCorrection;
use oni_settings::tone::{Print, color_density};
use oni_settings::VirtualFilm;
use crate::track::Bend;

mod block;
mod knob;
mod names;
mod readout;

pub use block::Block;

pub use readout::{Axis, Readout, Unit, number};

/// One slider's value, its index the `field` the Slint `Store` passes back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Field {
    Temperature,
    Tint,
    ColorDensity,
    Exposure,
    Highlights,
    Shadows,
    WhitePoint,
    BlackPoint,
    Contrast,
    Vignette,
    Chrome,
    BalanceStrength,
    DensityCeiling,
    Angle,
    SharpenAmount,
    SharpenRadius,
    SharpenHalo,
    ChromaAmount,
    ChromaRadius,
    Texture,
    PaperRed,
    PaperGreen,
    PaperBlue,
    GainRed,
    GainGreen,
    GainBlue,
    FlatAmount,
    FlatTemperature,
    FlatTint,
    GainTemperature,
    GainTint,
    RedWhitePoint,
    GreenWhitePoint,
    BlueWhitePoint,
    RedBlackPoint,
    GreenBlackPoint,
    BlueBlackPoint,
    GrainAmount,
    GrainSize,
    HalationAmount,
    HalationSize,
    Distortion,
    FringeRed,
    FringeBlue,
    LensVignetting,
    Bloom,
    RedMid,
    GreenMid,
    BlueMid,
    RedContrast,
    GreenContrast,
    BlueContrast,
    FilmExposure,
    Brightness,
    /// Temperature and Tint read as an enlarger's C, M and Y, a choice of Settings.
    Cyan,
    Magenta,
    Yellow,
}

#[derive(Clone, Copy, Debug)]
pub struct Spec {
    pub label: &'static str,
    pub range: (f32, f32),
    /// How the track lays the range under the hand.
    pub bend: Bend,
    pub readout: Readout,
    /// The colour axis its track runs along, whose end the readout names before its number.
    pub axis: Option<Axis>,
    /// The track runs the other way: right lifts the black, the stored value falls.
    pub inverted: bool,
}

impl Field {
    pub const ALL: [Field; 57] = [
        Field::Temperature,
        Field::Tint,
        Field::ColorDensity,
        Field::Exposure,
        Field::Highlights,
        Field::Shadows,
        Field::WhitePoint,
        Field::BlackPoint,
        Field::Contrast,
        Field::Vignette,
        Field::Chrome,
        Field::BalanceStrength,
        Field::DensityCeiling,
        Field::Angle,
        Field::SharpenAmount,
        Field::SharpenRadius,
        Field::SharpenHalo,
        Field::ChromaAmount,
        Field::ChromaRadius,
        Field::Texture,
        Field::PaperRed,
        Field::PaperGreen,
        Field::PaperBlue,
        Field::GainRed,
        Field::GainGreen,
        Field::GainBlue,
        Field::FlatAmount,
        Field::FlatTemperature,
        Field::FlatTint,
        Field::GainTemperature,
        Field::GainTint,
        Field::RedWhitePoint,
        Field::GreenWhitePoint,
        Field::BlueWhitePoint,
        Field::RedBlackPoint,
        Field::GreenBlackPoint,
        Field::BlueBlackPoint,
        Field::GrainAmount,
        Field::GrainSize,
        Field::HalationAmount,
        Field::HalationSize,
        Field::Distortion,
        Field::FringeRed,
        Field::FringeBlue,
        Field::LensVignetting,
        Field::Bloom,
        Field::RedMid,
        Field::GreenMid,
        Field::BlueMid,
        Field::RedContrast,
        Field::GreenContrast,
        Field::BlueContrast,
        Field::FilmExposure,
        Field::Brightness,
        Field::Cyan,
        Field::Magenta,
        Field::Yellow,
    ];

    pub fn index(self) -> i32 {
        Self::ALL.iter().position(|f| *f == self).unwrap_or(0) as i32
    }

    pub fn from_index(i: i32) -> Option<Field> {
        Self::ALL.get(usize::try_from(i).ok()?).copied()
    }

    pub fn block(self) -> Option<Block> {
        Block::ALL.into_iter().find(|b| b.fields().contains(&self))
    }

    /// Whether the panel shows this value for `settings`: a digital photo's film, its effects and its lens on it alone,
    /// Brightness on a paper alone, so a block's reset and its key read only what it shows.
    pub fn shown_in(self, settings: &oni_settings::PipelineSettings) -> bool {
        let mode = settings.mode;
        match self {
            Field::GrainAmount | Field::GrainSize => mode.is_digital(),
            Field::HalationAmount | Field::HalationSize | Field::Bloom => mode.is_digital(),
            Field::Distortion | Field::FringeRed | Field::FringeBlue | Field::LensVignetting => mode.is_digital(),
            Field::FilmExposure => mode.is_digital(),
            Field::Brightness => settings.paper().is_some(),
            _ => true,
        }
    }

    /// The panel's spec for this value on a mosaic colour negative, the only mode the shell opens.
    pub fn spec(self) -> Spec {
        let s = |label, range, readout| Spec { label, range, bend: Bend::Even, readout, axis: None, inverted: false };
        let warmth = |spec: Spec| Spec { axis: Some(Axis::Warmth), ..spec };
        let tint = |spec: Spec| Spec { axis: Some(Axis::Tint), ..spec };
        let black = |label| Spec { inverted: true, ..s(label, Print::BLACK_POINT_RANGE, Readout::SignedPercent) };
        match self {
            // The print filters' gap in stops: red against blue, then magenta against green.
            Field::Temperature => warmth(s("Temperature", Print::TEMPERATURE_RANGE, Readout::Stops)),
            Field::Tint => tint(s("Tint", Print::TINT_RANGE, Readout::Stops)),
            Field::ColorDensity => s("Saturation", color_density::RANGE, Readout::SignedPercent),
            Field::Exposure => s("Exposure", Print::EXPOSURE_RANGE, Readout::Stops),
            Field::Highlights => s("Highlights", Print::QUARTER_RANGE, Readout::SignedPercent),
            Field::Shadows => s("Shadows", Print::QUARTER_RANGE, Readout::SignedPercent),
            Field::WhitePoint => s("White point", Print::POINT_RANGE, Readout::SignedPercent),
            Field::BlackPoint => black("Black point"),
            Field::Contrast => s("Contrast", Print::CONTRAST_RANGE, Readout::SignedPercent),
            Field::Vignette => s("Vignette", Vignette::RANGE, Readout::Stops),
            Field::Chrome => s("Chrome", SAUCE_RANGE, Readout::Percent),
            Field::BalanceStrength => s("Auto White Balance Strength", oni_settings::blocks::filter_balance::STRENGTH_RANGE, Readout::Percent),
            Field::DensityCeiling => {
                let range = oni_settings::axis::density_axis::mosaic_range();
                s("Ceiling", (*range.start(), *range.end()), Readout::Ceiling)
            }
            Field::Angle => s("Angle", oni_settings::frame::Geometry::ANGLE_RANGE, Readout::Degrees),
            Field::SharpenAmount => s("Amount", Sharpen::AMOUNT_RANGE, Readout::Times),
            Field::SharpenRadius => Spec {
                bend: Bend::Centre(Sharpen::RADIUS_ANCHOR),
                ..s("Radius", Sharpen::radius_range(), Readout::Pixels)
            },
            Field::SharpenHalo => s("Halo suppression", Sharpen::HALO_RANGE, Readout::Percent),
            Field::ChromaAmount => s("Amount", CHROMA_FORCE_RANGE, Readout::Percent),
            Field::ChromaRadius => s("Radius", ChromaDenoise::RADIUS_RANGE, Readout::Pixels),
            Field::Texture => s("Texture", Texture::RANGE, Readout::SignedTimes),
            // `GainPanel` titled by its channel alone, as the Paper Correction block names them.
            Field::PaperRed => s("Red", PaperCorrection::RANGE, Readout::Stops),
            Field::PaperGreen => s("Green", PaperCorrection::RANGE, Readout::Stops),
            Field::PaperBlue => s("Blue", PaperCorrection::RANGE, Readout::Stops),
            // `GainPanel`'s rows, named by the ends the mode gives them, which the panel lays.
            Field::GainRed => s("Red", Gains::RANGE, Readout::Stops),
            Field::GainGreen => s("Green", Gains::RANGE, Readout::Stops),
            Field::GainBlue => s("Blue", Gains::RANGE, Readout::Stops),
            Field::GainTemperature => s("Temperature", Gains::RANGE, Readout::Stops),
            Field::GainTint => s("Tint", Gains::RANGE, Readout::Stops),
            Field::FlatAmount => s("Amount", FlatField::AMOUNT_RANGE, Readout::Percent),
            Field::FlatTemperature => warmth(s("Temperature", FlatField::COLOUR_RANGE, Readout::SignedPercent)),
            Field::FlatTint => tint(s("Tint", FlatField::COLOUR_RANGE, Readout::SignedPercent)),
            // Each channel's own points, read as Lighting's.
            Field::RedWhitePoint => s("Red white point", Print::POINT_RANGE, Readout::SignedPercent),
            Field::GreenWhitePoint => s("Green white point", Print::POINT_RANGE, Readout::SignedPercent),
            Field::BlueWhitePoint => s("Blue white point", Print::POINT_RANGE, Readout::SignedPercent),
            Field::RedBlackPoint => black("Red black point"),
            Field::GreenBlackPoint => black("Green black point"),
            Field::BlueBlackPoint => black("Blue black point"),
            // Each channel's own mids and contrast, read as Lighting's.
            Field::RedMid => s("Red mid", Print::ZONE_RANGE, Readout::SignedPercent),
            Field::GreenMid => s("Green mid", Print::ZONE_RANGE, Readout::SignedPercent),
            Field::BlueMid => s("Blue mid", Print::ZONE_RANGE, Readout::SignedPercent),
            Field::RedContrast => s("Red contrast", Print::CONTRAST_RANGE, Readout::SignedPercent),
            Field::GreenContrast => s("Green contrast", Print::CONTRAST_RANGE, Readout::SignedPercent),
            Field::BlueContrast => s("Blue contrast", Print::CONTRAST_RANGE, Readout::SignedPercent),
            Field::GrainAmount => s("Grain", Grain::AMOUNT_RANGE, Readout::Percent),
            // A grain's width on the film, which `readout_on` writes in the photo's own pixels.
            Field::GrainSize => s("Grain size", Grain::SIZE_RANGE, Readout::Microns),
            // The grain's colour line, its saturation at the picture's black and at its white.
            Field::HalationAmount => s("Halation", Halation::AMOUNT_RANGE, Readout::Percent),
            Field::HalationSize => s("Halation size", Halation::SIZE_RANGE, Readout::Microns),
            // Stops the threshold is lowered by: right, the dimmer light glows too.
            Field::Bloom => s("Bloom", Bloom::RANGE, Readout::FineMicrons),
            // A lens's corrections, each a share of its slider's reach.
            Field::Distortion => s("Distortion", LensCorrection::RANGE, Readout::SignedWhole),
            Field::FringeRed => s("Red fringe", LensCorrection::RANGE, Readout::SignedWhole),
            Field::FringeBlue => s("Blue fringe", LensCorrection::RANGE, Readout::SignedWhole),
            Field::LensVignetting => s("Vignetting", LensCorrection::RANGE, Readout::SignedWhole),
            // The stops of light a virtual film takes over the scene's own.
            Field::FilmExposure => s("Film exposure", VirtualFilm::EXPOSURE_RANGE, Readout::Stops),
            // A paper's print brightened between its anchored pick and its white, in the stops of ONI's Exposure.
            Field::Brightness => s("Brightness", Print::BRIGHTNESS_RANGE, Readout::Stops),
            // Each band's filter on the negative, in stops either way: C on red's band, M on green's, Y on blue's.
            Field::Cyan => s("C", Print::FILTER_RANGE, Readout::Stops),
            Field::Magenta => s("M", Print::FILTER_RANGE, Readout::Stops),
            Field::Yellow => s("Y", Print::FILTER_RANGE, Readout::Stops),
        }
    }

    /// The value's one rest on this frame: where its track's tick stands, where a double click lands
    /// and what its block's reset writes, the frame's import state in its own mode and decode.
    pub fn base(self, settings: &oni_settings::PipelineSettings) -> f32 {
        let mut rest = oni_settings::PipelineSettings::rest(settings.mode, settings.decoded_from_mosaic);
        if self.block() == Some(Block::SourceBalance) {
            // `restingGains`: a measured roll rests on its window's stops, a slide on its own measure's, a
            // digital photo on its film's reference.
            let film = settings.film().map(|f| f.stock.reference().stops);
            let stops = settings.roll_reference.as_ref().map(|r| r.stops).or(settings.measured_gains).or(film).unwrap_or([0.0; 3]);
            rest = settings.clone();
            rest.gains.stops = stops;
        }
        // A digital photo's film effects rest where its own film lays them.
        if let Some(film) = settings.film().filter(|_| self.block() == Some(Block::FilmEffects)) {
            rest = rest.picking(film.stock);
        }
        self.knob().get(&rest)
    }

    /// What the track shows for a stored value: flipped where the track runs the other way.
    pub fn shown(self, stored: f32) -> f32 {
        if self.spec().inverted { -stored } else { stored }
    }

    /// The readout of a stored value, in the sense the slider shows it, after the end of its colour
    /// axis it leans to.
    pub fn readout(self, stored: f32) -> String {
        self.readout_on(stored, None)
    }

    /// `readout` on a photo whose scan's long side is `long` full-resolution pixels, once known.
    pub fn readout_on(self, stored: f32, long: Option<f64>) -> String {
        let (readout, figure) = self.figure(stored, long);
        readout.along(self.spec().axis, figure)
    }

    /// The readout of stored values from `low` to `high` in the sense the slider shows them, as the
    /// frames shown together span them.
    pub fn span_readout(self, low: f32, high: f32) -> String {
        self.span_readout_on(low, high, None)
    }

    /// `span_readout` on a photo whose scan's long side is `long` full-resolution pixels, once known.
    pub fn span_readout_on(self, low: f32, high: f32, long: Option<f64>) -> String {
        let ((readout, a), (_, b)) = (self.figure(low, long), self.figure(high, long));
        readout.span(self.spec().axis, a.min(b), a.max(b))
    }

    /// How the readout writes `stored` and the figure it writes, in the shown sense: Grain size in pixels of a
    /// scan whose long side is `long` pixels, so its readout follows the photo's resolution, on the film
    /// while none is known.
    pub fn figure(self, stored: f32, long: Option<f64>) -> (Readout, f32) {
        match (self, long) {
            (Field::GrainSize, Some(long)) => (Readout::Pixels, Grain::pixels(stored, long) as f32),
            _ => (self.spec().readout, self.shown(stored)),
        }
    }

    /// The value, in the shown sense, whose readout writes `figure` on a scan whose long side is `long` pixels.
    pub fn unfigured(self, figure: f32, long: Option<f64>) -> f32 {
        match (self, long) {
            (Field::GrainSize, Some(long)) => Grain::microns(f64::from(figure), long),
            _ => figure,
        }
    }

    /// The range the track spans, in the shown sense.
    pub fn track(self) -> (f32, f32) {
        let spec = self.spec();
        if spec.inverted { (-spec.range.1, -spec.range.0) } else { spec.range }
    }
}

#[cfg(test)]
mod tests;
