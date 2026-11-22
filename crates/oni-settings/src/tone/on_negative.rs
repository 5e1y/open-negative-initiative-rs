//! What Lighting lays on the negative, and its rest there.

use super::{Print, Tone};

impl Print {
    /// This print with every slider Lighting lays on the negative at rest: Exposure, Contrast, the zones, the filters,
    /// the points and each channel's own; Brightness, Paper blacks and the colour compensation kept.
    pub fn unlit(&self, mode: crate::modes::ConversionMode) -> Print {
        let unfiltered = |t: Tone| Tone { exposure: 0.0, ..t };
        Print {
            exposure: Self::resting_exposure(mode),
            contrast: 0.0,
            shadows: 0.0,
            highlights: 0.0,
            red: unfiltered(self.red),
            green: unfiltered(self.green),
            blue: unfiltered(self.blue),
            black_point: Self::RESTING_BLACK_POINT,
            white_point: 0.0,
            channel_white: [0.0; 3],
            channel_black: [0.0; 3],
            channel_mid: [0.0; 3],
            channel_contrast: [0.0; 3],
            ..*self
        }
    }

    /// Whether Lighting lays nothing on the negative: bypassed, or every slider it lays there at rest.
    pub fn rests_on_negative(&self, mode: crate::modes::ConversionMode) -> bool {
        self.bypassed || *self == self.unlit(mode)
    }
}
