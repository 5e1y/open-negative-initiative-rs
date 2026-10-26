//! The demos, one a help topic. Each is a scene in `scenes/<topic>.slint` and a timeline here.

mod balance_strength;
mod crop;
mod dust;
mod editing;
mod editors;
mod film_type;
mod flat_field;
mod gesture;
mod measured_roll;
mod paper;
mod slider;
mod sliders;
mod slides;
mod source_encoding;
mod white_balance;

pub use balance_strength::BalanceStrength;
pub use crop::Crop;
pub use dust::Dust;
pub use editors::{curves, saturation_curve, spectrogram};
pub use film_type::FilmType;
pub use flat_field::FlatField;
pub use measured_roll::MeasuredRoll;
pub use paper::PaperCorrection;
pub use slider::{OneSlider, Spec, View, pictured};
pub use sliders::{chroma_denoise, density_ceiling, dye_separation, sharpening, texture};
pub use slides::{effects, lighting};
pub use source_encoding::SourceEncodingDemo;
pub use white_balance::WhiteBalance;

use oni_app::fields::Field;
use oni_app::slider_track::{position_on, stored_on, track_on};
use oni_settings::PipelineSettings;

use crate::film::Demo;
use crate::stage::{SCALE, Stage};

/// Every demo, in the order they are filmed.
pub fn all() -> Result<Vec<Box<dyn Demo>>, String> {
    Ok(vec![
        Box::new(sharpening()),
        Box::new(WhiteBalance::new()),
        Box::new(Crop::new()),
        Box::new(MeasuredRoll::new()),
        Box::new(FilmType::new()),
        Box::new(FlatField::new()),
        Box::new(SourceEncodingDemo::new()),
        Box::new(density_ceiling()),
        Box::new(dye_separation()),
        Box::new(BalanceStrength::new()),
        Box::new(PaperCorrection::new()),
        Box::new(lighting()),
        Box::new(effects()),
        Box::new(curves()),
        Box::new(spectrogram()),
        Box::new(saturation_curve()),
        Box::new(chroma_denoise()),
        Box::new(texture()),
        Box::new(Dust::new()),
    ])
}

/// The scene's picture box, whose pixels the photograph is rendered at, one for one.
pub struct Picture {
    pub width: f32,
    pub height: f32,
}

impl Picture {
    pub fn of(stage: &Stage) -> Picture {
        Picture { width: stage.number("photo-width"), height: stage.number("photo-height") }
    }

    pub fn pixels(&self) -> (u32, u32) {
        ((self.width * SCALE).round() as u32, (self.height * SCALE).round() as u32)
    }
}

/// Where `f` stands on its slider for these settings, as the panel shows it.
pub fn fraction_of(f: Field, s: &PipelineSettings) -> f32 {
    position_on(f, f.knob().get(s), &track_on(f, s))
}

/// The settings with `f` where the slider's `fraction` puts it.
pub fn stored_at(f: Field, s: &PipelineSettings, fraction: f32) -> PipelineSettings {
    f.knob().set(s, stored_on(f, fraction, &track_on(f, s)))
}
