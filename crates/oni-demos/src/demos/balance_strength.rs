//! Roll › Balance Strength: a cast, Auto White Balance read on it; the hand lowers Strength and
//! Temperature and Tint give up the correction, the cast returning; a double click rests it.

use std::collections::BTreeMap;

use oni_app::fields::Field;
use oni_core::balance::Outcome;
use oni_settings::PipelineSettings;
use slint_interpreter::Value;

use super::gesture::Drag;
use super::{Picture, fraction_of, stored_at};
use crate::encode::Tuning;
use crate::film::{Demo, Hand};
use crate::photo::{self, Photo, Rgba};
use crate::stage::Stage;

/// The cast laid before the key reads, as shifts of Source Balance's red and blue tracks: a yellow one.
const CAST: (f32, f32) = (0.0, 0.08);
/// Where the hand takes Strength from its rest: none of the correction, the cast whole again.
const NONE: f32 = 0.0;
const LEVELS: f32 = 8.0;

pub struct BalanceStrength {
    cast: PipelineSettings,
    outcome: Option<Outcome>,
    size: (u32, u32),
    drag: Option<Drag>,
    renders: BTreeMap<i32, Rgba>,
}

impl BalanceStrength {
    pub fn new() -> BalanceStrength {
        let base = photo::framed(photo::base());
        let red = stored_at(Field::GainRed, &base, fraction_of(Field::GainRed, &base) + CAST.0);
        let cast = stored_at(Field::GainBlue, &red, fraction_of(Field::GainBlue, &red) + CAST.1);
        BalanceStrength { cast, outcome: None, size: (0, 0), drag: None, renders: BTreeMap::new() }
    }

    /// Temperature and Tint where Strength at `fraction` puts them.
    fn follow(&self, stage: &Stage, fraction: f32) -> Result<(), String> {
        let now = self.dosed(fraction)?;
        stage.set("temperature", Value::Number(f64::from(fraction_of(Field::Temperature, &now))));
        stage.set("tint", Value::Number(f64::from(fraction_of(Field::Tint, &now))));
        Ok(())
    }

    /// The frame with the key's correction dosed at Strength's `fraction`.
    fn dosed(&self, fraction: f32) -> Result<PipelineSettings, String> {
        let outcome = self.outcome.as_ref().ok_or("not prepared")?;
        Ok(oni_core::balance::applied_outcome(outcome, &stored_at(Field::BalanceStrength, &self.cast, fraction)))
    }
}

impl Demo for BalanceStrength {
    fn topic(&self) -> &'static str {
        "balance-strength"
    }

    fn component(&self) -> &'static str {
        "BalanceStrength"
    }


    fn seconds(&self) -> f32 {
        super::gesture::SLIDE
    }

    fn prepare(&mut self, stage: &Stage, _photo: &mut Photo) -> Result<(), String> {
        let path = photo::source().to_string_lossy().into_owned();
        let outcomes = oni_app::measuring::balance_outcomes(&[(path.clone(), self.cast.clone())]);
        self.outcome = Some(outcomes.get(&path).ok_or("Auto White Balance read nothing on the frame")?.clone());
        self.size = Picture::of(stage).pixels();
        let f = Field::BalanceStrength;
        let rest = fraction_of(f, &f.knob().set(&self.cast, f.double_click_base(&self.cast)));
        self.drag = Some(Drag::lay(stage, rest, NONE, LEVELS, true));
        self.follow(stage, rest)
    }

    /// The cast comes back in small steps: a pixel is drawn again past a small stray.
    fn tuning(&self) -> Tuning {
        Tuning { tolerance: 4, colours: 128 }
    }

    fn hand(&self, t: f32) -> Hand {
        self.drag.as_ref().expect("prepared").hand(t)
    }

    /// Temperature and Tint follow the knob as it goes; the photograph at each step it crosses.
    fn pose(&mut self, t: f32, stage: &Stage, photo: &mut Photo) -> Result<(), String> {
        self.follow(stage, stage.number("amount"))?;
        let Some((step, fraction)) = self.drag.as_mut().ok_or("not prepared")?.moved(t, stage) else { return Ok(()) };
        if !self.renders.contains_key(&step) {
            let s = self.dosed(fraction)?;
            self.renders.insert(step, photo.cover(&s, self.size.0, self.size.1)?);
        }
        stage.set_image("photo", &self.renders[&step]);
        Ok(())
    }
}
