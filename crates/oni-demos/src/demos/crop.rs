//! Crop: the hand drags the frame's corner in, then tilts Straighten a few degrees, the photograph
//! turning with the knob; then the crop is reset and the loop starts again.

use std::collections::BTreeMap;

use oni_app::fields::Field;
use oni_settings::PipelineSettings;
use slint_interpreter::Value;

use super::{Picture, fraction_of, stored_at};
use crate::encode::Tuning;
use crate::film::{Demo, Hand};
use crate::photo::{self, Photo, Rgba};
use crate::stage::Stage;
use crate::track::{Presses, Track};

/// How far the corner is dragged in, in points.
const DRAG: (f32, f32) = (-48.0, -32.0);
/// The tilt, as a share of Straighten's track, finer about its middle: a few degrees.
const TILT: f32 = 0.4;
/// Renders along the track: the photograph turns when the knob crosses a step, each costing a GIF a
/// redrawn photograph.
const STEPS: f32 = 20.0;
/// When the crop is reset, the frame whole and the angle level again.
const RESET: f32 = 5.5;

pub struct Crop {
    settings: PipelineSettings,
    size: (u32, u32),
    level: f32,
    path: Track<(f32, f32)>,
    presses: Presses,
    renders: BTreeMap<i32, Rgba>,
    shown: Option<i32>,
    reset: bool,
}

impl Crop {
    pub fn new() -> Crop {
        let settings = photo::base();
        Crop { settings, size: (0, 0), level: 0.5, path: Track::default(), presses: Presses::default(), renders: BTreeMap::new(), shown: None, reset: false }
    }
}

impl Demo for Crop {
    fn topic(&self) -> &'static str {
        "crop"
    }

    fn component(&self) -> &'static str {
        "Crop"
    }


    fn seconds(&self) -> f32 {
        6.0
    }

    fn prepare(&mut self, stage: &Stage, _photo: &mut Photo) -> Result<(), String> {
        self.size = Picture::of(stage).pixels();
        self.level = fraction_of(Field::Angle, &self.settings);
        stage.set("angle", Value::Number(f64::from(self.level)));
        // A corner's catch lies inside the frame: the hand takes it a little in from the very corner.
        let corner = (stage.number("corner-x") - 4.0, stage.number("corner-y") - 4.0);
        let dragged = (corner.0 + DRAG.0, corner.1 + DRAG.1);
        let knob = (stage.number("knob-x"), stage.number("knob-y"));
        let tilted = (knob.0 + TILT * stage.number("travel"), knob.1);
        let rest = (corner.0 - 70.0, corner.1 - 50.0);
        self.path = Track::from(rest)
            .hold(0.4)
            .to(1.0, corner)
            .hold(1.2)
            .to(1.65, dragged)
            .hold(2.4)
            .to(3.0, knob)
            .hold(3.2)
            .to(3.9, tilted)
            .hold(4.3)
            .to(5.2, rest);
        self.presses = Presses(vec![(1.1, 2.3), (3.1, 4.2)]);
        Ok(())
    }

    /// Every step of the corner's drag redraws the guides across the photograph: fewer colours pay for them.
    fn tuning(&self) -> Tuning {
        Tuning { tolerance: 12, colours: 96 }
    }

    fn hand(&self, t: f32) -> Hand {
        Hand { at: self.path.at(t), down: self.presses.down(t) }
    }

    fn pose(&mut self, t: f32, stage: &Stage, photo: &mut Photo) -> Result<(), String> {
        if t >= RESET && !self.reset {
            stage.set("inset-right", Value::Number(0.0));
            stage.set("inset-bottom", Value::Number(0.0));
            stage.set("angle", Value::Number(f64::from(self.level)));
            self.reset = true;
        }
        let step = ((stage.number("angle") - self.level) * STEPS).round() as i32;
        if self.shown == Some(step) {
            return Ok(());
        }
        if !self.renders.contains_key(&step) {
            let s = stored_at(Field::Angle, &self.settings, self.level + step as f32 / STEPS);
            self.renders.insert(step, photo.cover(&s, self.size.0, self.size.1)?);
        }
        stage.set_image("photo", &self.renders[&step]);
        self.shown = Some(step);
        Ok(())
    }
}
