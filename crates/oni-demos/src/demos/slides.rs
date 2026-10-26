//! Two sliders dragged in turn over the whole frame, Color's Lighting and Effects: the hand takes
//! each knob to its mark and lets go, the photograph following; then both rows go back to rest.

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

/// Renders between a row's rest and its mark: each costs a GIF a redrawn photograph.
const LEVELS: f32 = 4.0;
/// When both rows go back to rest, the hand away.
const RESET: f32 = 5.1;
/// The scene's rows, as `Slides` in `parts.slint` names them.
const ROWS: [&str; 2] = ["first", "second"];

/// A row: the field it moves and where the hand leaves its knob, as a share of the track from rest.
#[derive(Clone, Copy)]
pub struct Move {
    pub field: Field,
    pub by: f32,
}

pub struct Slides {
    topic: &'static str,
    tuning: Tuning,
    settings: PipelineSettings,
    moves: [Move; 2],
    rests: [f32; 2],
    size: (u32, u32),
    path: Track<(f32, f32)>,
    presses: Presses,
    renders: BTreeMap<[i32; 2], Rgba>,
    shown: Option<[i32; 2]>,
    reset: bool,
}

/// Lighting: Exposure, then Shadows, each to the right, the photograph brightening.
pub fn lighting() -> Slides {
    let moves = [Move { field: Field::Exposure, by: 0.22 }, Move { field: Field::Shadows, by: 0.3 }];
    // Both brighten every pixel: fewer colours and a wider tolerance pay for the redrawn photograph.
    Slides::new("lighting", moves, Tuning { tolerance: 22, colours: 88 })
}

/// Effects: Contrast, then Vignette, the print bending into an S and its bright corner darkening.
pub fn effects() -> Slides {
    let moves = [Move { field: Field::Contrast, by: 0.3 }, Move { field: Field::Vignette, by: 0.5 }];
    Slides::new("effects", moves, Tuning { tolerance: 10, colours: 96 })
}

impl Slides {
    fn new(topic: &'static str, moves: [Move; 2], tuning: Tuning) -> Slides {
        Slides {
            topic,
            tuning,
            settings: photo::framed(photo::base()),
            moves,
            rests: [0.0; 2],
            size: (0, 0),
            path: Track::default(),
            presses: Presses::default(),
            renders: BTreeMap::new(),
            shown: None,
            reset: false,
        }
    }

    fn rest(&self, stage: &Stage) {
        for (row, rest) in ROWS.iter().zip(self.rests) {
            stage.set(row, Value::Number(f64::from(rest)));
        }
    }
}

impl Demo for Slides {
    fn topic(&self) -> &'static str {
        self.topic
    }

    fn component(&self) -> &'static str {
        "TwoRows"
    }


    fn seconds(&self) -> f32 {
        5.6
    }

    fn prepare(&mut self, stage: &Stage, _photo: &mut Photo) -> Result<(), String> {
        self.size = Picture::of(stage).pixels();
        self.rests = self.moves.map(|m| fraction_of(m.field, &self.settings));
        self.rest(stage);
        let travel = stage.number("travel");
        let knob = |row: &str| (stage.number(&format!("{row}-x")), stage.number(&format!("{row}-y")));
        let (a, b) = (knob(ROWS[0]), knob(ROWS[1]));
        let (a_to, b_to) = ((a.0 + self.moves[0].by * travel, a.1), (b.0 + self.moves[1].by * travel, b.1));
        let rest = (a.0 + 40.0, a.1 - 52.0);
        self.path = Track::from(rest)
            .hold(0.3)
            .to(0.8, a)
            .hold(1.0)
            .to(1.8, a_to)
            .hold(2.2)
            .to(2.7, b)
            .hold(2.9)
            .to(3.7, b_to)
            .hold(4.1)
            .to(4.8, rest);
        self.presses = Presses(vec![(0.9, 2.15), (2.8, 4.05)]);
        Ok(())
    }

    fn tuning(&self) -> Tuning {
        self.tuning
    }

    fn hand(&self, t: f32) -> Hand {
        Hand { at: self.path.at(t), down: self.presses.down(t) }
    }

    fn pose(&mut self, t: f32, stage: &Stage, photo: &mut Photo) -> Result<(), String> {
        if t >= RESET && !self.reset {
            self.rest(stage);
            self.reset = true;
        }
        let mut steps = [0i32; 2];
        for (i, row) in ROWS.iter().enumerate() {
            let share = (stage.number(row) - self.rests[i]) / self.moves[i].by;
            steps[i] = (share * LEVELS).round().clamp(0.0, LEVELS) as i32;
        }
        if self.shown == Some(steps) {
            return Ok(());
        }
        if !self.renders.contains_key(&steps) {
            let mut s = self.settings.clone();
            for (i, m) in self.moves.iter().enumerate() {
                s = stored_at(m.field, &s, self.rests[i] + steps[i] as f32 / LEVELS * m.by);
            }
            self.renders.insert(steps, photo.cover(&s, self.size.0, self.size.1)?);
        }
        stage.set_image("photo", &self.renders[&steps]);
        self.shown = Some(steps);
        Ok(())
    }
}
