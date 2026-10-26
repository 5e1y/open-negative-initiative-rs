//! The demos' shared gestures: a knob taken, dragged to a mark, let go and double-clicked back to
//! rest; or a row of clicks, the hand resting between them. The scene's pictures change on a new state only.

use slint_interpreter::Value;

use crate::film::{Hand, SIDE};
use crate::photo::Rgba;
use crate::stage::Stage;
use crate::track::{Presses, Track};

/// How far from its first aim the hand rests, in points.
const AWAY: (f32, f32) = (52.0, -64.0);
/// A click: the hand arrives this long before it, and rests this long after.
const MOVE: f32 = 0.6;
const REST: f32 = 0.4;

/// Where the hand rests off `aim`, toward the room's middle so it never leaves the frame.
pub fn away(aim: (f32, f32)) -> (f32, f32) {
    let x = if aim.0 < SIDE / 2.0 { aim.0 + AWAY.0 } else { aim.0 - AWAY.0 };
    (x, (aim.1 + AWAY.1).max(8.0))
}

/// A slide's timeline, in seconds: the hand reaches the knob, drags it, lets go, then double-clicks
/// it, which brings the value back to rest as in the panel; the knob springs home and the hand leaves.
const REACH: (f32, f32) = (0.4, 1.0);
const DRAG: (f32, f32) = (1.15, 2.45);
const LET_GO: f32 = 2.55;
const CLICKS: [(f32, f32); 2] = [(2.95, 3.03), (3.15, 3.23)];
const LEAVE: (f32, f32) = (3.9, 4.5);
/// A slide's length, the last still frames leading back into the first.
pub const SLIDE: f32 = 4.8;
/// Dragged back instead, for a slider whose rest is not where the double click brings it.
const BACK: (f32, f32) = (2.9, 3.9);

/// The knob of a one-slider scene dragged from `rest` to `high`, then double-clicked back to rest,
/// its picture changing at `levels` steps between the two.
pub struct Drag {
    pub rest: f32,
    pub high: f32,
    levels: f32,
    path: Track<(f32, f32)>,
    presses: Presses,
    shown: Option<i32>,
    /// Whether a double click brings the knob back, and whether it did.
    clicked: bool,
    reset: bool,
}

impl Drag {
    /// `clicked`: the double click brings the knob back, else the hand drags it back.
    pub fn lay(stage: &Stage, rest: f32, high: f32, levels: f32, clicked: bool) -> Drag {
        stage.set("amount", Value::Number(f64::from(rest)));
        let knob = (stage.number("knob-x"), stage.number("knob-y"));
        let far = (knob.0 + (high - rest) * stage.number("travel"), knob.1);
        let off = away(knob);
        let path = Track::from(off).hold(REACH.0).to(REACH.1, knob).hold(DRAG.0).to(DRAG.1, far);
        let (path, presses) = if clicked {
            (path.hold(LEAVE.0).to(LEAVE.1, off), vec![(REACH.1 + 0.05, LET_GO), CLICKS[0], CLICKS[1]])
        } else {
            (path.hold(BACK.0).to(BACK.1, knob).hold(LEAVE.0).to(LEAVE.1, off), vec![(REACH.1 + 0.05, BACK.1 + 0.05)])
        };
        Drag { rest, high, levels, path, presses: Presses(presses), shown: None, clicked, reset: false }
    }

    pub fn hand(&self, t: f32) -> Hand {
        Hand { at: self.path.at(t), down: self.presses.down(t) }
    }

    /// The step the knob stands on at `t` and the fraction it is rendered at, when it changed; the
    /// double-click's second press puts the value back to rest.
    pub fn moved(&mut self, t: f32, stage: &Stage) -> Option<(i32, f32)> {
        // The first click let go, the next press is the second of a double click: the knob springs home.
        if self.clicked && t >= CLICKS[0].1 {
            stage.set("double", Value::Bool(true));
        }
        let back = if self.clicked { CLICKS[1].0 } else { BACK.1 + 0.05 };
        if t >= back && !self.reset {
            stage.set("amount", Value::Number(f64::from(self.rest)));
            self.reset = true;
        }
        // Back at rest the value is, whatever the knob's spring still shows.
        let at = if self.reset { self.rest } else { stage.number("amount") };
        let step = ((at - self.rest) / (self.high - self.rest) * self.levels).round() as i32;
        if self.shown == Some(step) {
            return None;
        }
        self.shown = Some(step);
        Some((step, self.at(step)))
    }

    /// The fraction step `step` is rendered at.
    pub fn at(&self, step: i32) -> f32 {
        self.rest + step as f32 / self.levels * (self.high - self.rest)
    }

    pub fn steps(&self) -> std::ops::RangeInclusive<i32> {
        0..=self.levels as i32
    }
}

/// The hand clicking each aim at its second, then going back to rest by `home`.
pub struct Clicks {
    path: Track<(f32, f32)>,
    presses: Presses,
}

impl Clicks {
    pub fn at(aims: &[(f32, (f32, f32))], home: f32) -> Clicks {
        let off = away(aims.first().map_or((SIDE / 2.0, SIDE / 2.0), |a| a.1));
        let (mut path, mut presses, mut free) = (Track::from(off), Vec::new(), 0.0f32);
        for &(t, aim) in aims {
            path = path.hold(free.max(t - MOVE)).to(t, aim);
            presses.push((t + 0.1, t + 0.22));
            free = t + REST;
        }
        Clicks { path: path.hold(free).to(home, off), presses: Presses(presses) }
    }

    pub fn hand(&self, t: f32) -> Hand {
        Hand { at: self.path.at(t), down: self.presses.down(t) }
    }
}

/// The centre of tab `i` of the `n` a `PhotoTabs` scene lays.
pub fn tab(stage: &Stage, i: usize, n: usize) -> (f32, f32) {
    (stage.number("tabs-x") + stage.number("tabs-width") * (i as f32 + 0.5) / n as f32, stage.number("tabs-y"))
}

/// A picture a state of the scene, laid on its `photo` when the state changes.
#[derive(Default)]
pub struct States {
    pub pictures: Vec<Rgba>,
    shown: Option<usize>,
}

impl States {
    pub fn show(&mut self, stage: &Stage, state: usize) -> Result<(), String> {
        if self.shown != Some(state) {
            stage.set_image("photo", self.pictures.get(state).ok_or("a state with no picture")?);
            self.shown = Some(state);
        }
        Ok(())
    }
}

/// Tab names for a scene's `labels`, in the language the film is shot in.
pub fn labels(names: &[&str]) -> Value {
    let names: Vec<Value> = names.iter().map(|n| Value::String((*n).into())).collect();
    Value::Model(slint::ModelRc::new(slint::VecModel::from(names)))
}
