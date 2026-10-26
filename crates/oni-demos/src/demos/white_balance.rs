//! Color › Auto White Balance: a frame with a cast, the hand presses the balance key, the picture
//! turns neutral and Temperature and Tint spring to what the app's own reader found; then back.

use oni_app::fields::Field;
use oni_settings::PipelineSettings;
use slint_interpreter::Value;

use super::{Picture, fraction_of, stored_at};
use crate::encode::Tuning;
use crate::film::{Demo, Hand};
use crate::photo::{self, Photo, Rgba};
use crate::stage::Stage;
use crate::track::{Presses, Track};

/// The cast it starts from, as fractions of the Temperature and Tint tracks: warm and green, far
/// from where the reader lands them, so both knobs are seen to travel.
const CAST: (f32, f32) = (0.85, 0.25);
/// When the picture goes back to its cast, so the loop starts again where it began.
const RESET: f32 = 5.0;

pub struct WhiteBalance {
    cast: PipelineSettings,
    balanced: PipelineSettings,
    pictures: Option<(Rgba, Rgba)>,
    path: Track<(f32, f32)>,
    presses: Presses,
    /// Whether the scene shows the balanced state, as last posed.
    shown: Option<bool>,
}

impl WhiteBalance {
    pub fn new() -> WhiteBalance {
        let base = photo::framed(photo::base());
        let cast = stored_at(Field::Tint, &stored_at(Field::Temperature, &base, CAST.0), CAST.1);
        WhiteBalance { balanced: cast.clone(), cast, pictures: None, path: Track::default(), presses: Presses::default(), shown: None }
    }

    fn show(&mut self, stage: &Stage, balanced: bool) -> Result<(), String> {
        let (cast, neutral) = self.pictures.as_ref().ok_or("not prepared")?;
        let (picture, settings) = if balanced { (neutral, &self.balanced) } else { (cast, &self.cast) };
        let (temperature, tint) = Self::fractions(settings);
        stage.set_image("photo", picture);
        stage.set("temperature", temperature);
        stage.set("tint", tint);
        self.shown = Some(balanced);
        Ok(())
    }

    fn fractions(s: &PipelineSettings) -> (Value, Value) {
        (Value::Number(f64::from(fraction_of(Field::Temperature, s))), Value::Number(f64::from(fraction_of(Field::Tint, s))))
    }
}

impl Demo for WhiteBalance {
    fn topic(&self) -> &'static str {
        "white-balance"
    }

    fn component(&self) -> &'static str {
        "WhiteBalance"
    }


    fn seconds(&self) -> f32 {
        5.5
    }

    fn prepare(&mut self, stage: &Stage, photo: &mut Photo) -> Result<(), String> {
        let path = photo::source().to_string_lossy().into_owned();
        let outcome = oni_app::measuring::balance_outcomes(&[(path.clone(), self.cast.clone())]);
        let outcome = outcome.get(&path).ok_or("Auto White Balance read nothing on the frame")?;
        self.balanced = oni_core::balance::applied_outcome(outcome, &self.cast);
        let (w, h) = Picture::of(stage).pixels();
        self.pictures = Some((photo.cover(&self.cast, w, h)?, photo.cover(&self.balanced, w, h)?));
        let key = (stage.number("key-x"), stage.number("key-y"));
        let rest = (key.0 + 46.0, key.1 - 58.0);
        let aside = (key.0 + 30.0, key.1 + 14.0);
        self.show(stage, false)?;
        self.path = Track::from(rest).hold(0.5).to(1.1, key).hold(1.6).to(2.3, aside).hold(3.8).to(4.5, rest);
        self.presses = Presses(vec![(1.25, 1.35)]);
        Ok(())
    }

    /// The picture changes twice only: the photograph may take more colours than the default.
    fn tuning(&self) -> Tuning {
        Tuning { tolerance: 10, colours: 128 }
    }

    fn hand(&self, t: f32) -> Hand {
        Hand { at: self.path.at(t), down: self.presses.down(t) }
    }

    fn pose(&mut self, t: f32, stage: &Stage, _photo: &mut Photo) -> Result<(), String> {
        if t >= RESET && stage.flag("balanced") {
            stage.set("balanced", Value::Bool(false));
        }
        let balanced = stage.flag("balanced");
        if self.shown == Some(balanced) {
            return Ok(());
        }
        self.show(stage, balanced)
    }
}
