//! The editors' demos, Curves, Spectrogram Finishing and the Saturation Curve: the hand presses on
//! the plot and drags, stroke after stroke, through the app's own gestures; the photograph follows.

use std::collections::BTreeMap;

use oni_app::editors::geometry::{Drawing, Size};
use oni_settings::PipelineSettings;
use slint_interpreter::Value;

use super::Picture;
use crate::encode::Tuning;
use crate::film::{Demo, Hand};
use crate::photo::{self, Photo, Rgba};
use crate::plot;
use crate::stage::Stage;
use crate::track::{Presses, Track};

/// A press at `.0` dragged to `.1`, in the plot's points.
pub type Stroke = ((f64, f64), (f64, f64));

/// Renders along a stroke: each costs a GIF a redrawn photograph.
const LEVELS: f64 = 8.0;
/// When the plot goes back to rest, the hand away.
const RESET: f32 = 5.35;

/// An editor of the app, its gestures replayed.
pub trait Editor {
    /// The plot's shape, as `Edited` in `parts.slint` takes it: wide, and with its grid.
    fn plot(&self) -> (bool, bool) {
        (false, true)
    }
    /// The plot's height in points: the photograph takes what it leaves.
    fn side(&self) -> f32 {
        104.0
    }
    /// The strokes the hand draws on a plot of `size`, from the settings it starts on.
    fn strokes(&self, size: Size, s: &PipelineSettings) -> Vec<Stroke>;
    /// The settings after `strokes`, laid one after the other on `s` as the app takes them, and the
    /// plot then; `held` whether the hand still holds the last.
    fn edit(&self, s: &PipelineSettings, strokes: &[Stroke], held: bool, size: Size) -> (PipelineSettings, Drawing);
}

pub struct Edits {
    topic: &'static str,
    settings: PipelineSettings,
    editor: Box<dyn Editor>,
    size: Size,
    pixels: (u32, u32),
    plot: (f32, f32),
    strokes: Vec<Stroke>,
    path: Track<(f32, f32)>,
    presses: Presses,
    renders: BTreeMap<(usize, i32), Rgba>,
    shown: Option<(usize, i32)>,
}

impl Edits {
    pub fn new(topic: &'static str, editor: Box<dyn Editor>) -> Edits {
        Edits {
            topic,
            settings: photo::framed(photo::base()),
            editor,
            size: Size::default(),
            pixels: (0, 0),
            plot: (0.0, 0.0),
            strokes: Vec::new(),
            path: Track::default(),
            presses: Presses::default(),
            renders: BTreeMap::new(),
            shown: None,
        }
    }

    fn in_scene(&self, p: (f64, f64)) -> (f32, f32) {
        (self.plot.0 + p.0 as f32, self.plot.1 + p.1 as f32)
    }

    /// The strokes drawn at `t`, the last as far as the hand has taken it, and the photograph's
    /// key: how many strokes it shows whole and the step of the next, 0 before its press, 1 at it.
    fn drawn(&self, t: f32, hand: Hand) -> (Vec<Stroke>, bool, (usize, i32)) {
        if t >= RESET {
            return (Vec::new(), false, (0, 0));
        }
        let mut out = Vec::new();
        for (k, ((from, until), stroke)) in self.presses.0.iter().zip(&self.strokes).enumerate() {
            if t >= *until {
                out.push(*stroke);
                continue;
            }
            if t < *from {
                return (out, false, (k, 0));
            }
            let at = (f64::from(hand.at.0 - self.plot.0), f64::from(hand.at.1 - self.plot.1));
            let (a, b) = *stroke;
            let reach = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt().max(1e-9);
            let gone = ((at.0 - a.0) * (b.0 - a.0) + (at.1 - a.1) * (b.1 - a.1)) / (reach * reach);
            out.push((a, at));
            return (out, true, (k, 1 + (gone.clamp(0.0, 1.0) * LEVELS).round() as i32));
        }
        let n = out.len();
        (out, false, (n, 0))
    }
}

impl Demo for Edits {
    fn topic(&self) -> &'static str {
        self.topic
    }

    fn component(&self) -> &'static str {
        "Edited"
    }


    fn seconds(&self) -> f32 {
        5.6
    }

    fn prepare(&mut self, stage: &Stage, _photo: &mut Photo) -> Result<(), String> {
        let (wide, grid) = self.editor.plot();
        stage.set("wide", Value::Bool(wide));
        stage.set("grid", Value::Bool(grid));
        stage.set("side", Value::Number(f64::from(self.editor.side())));
        self.pixels = Picture::of(stage).pixels();
        self.plot = (stage.number("plot-x"), stage.number("plot-y"));
        self.size = Size::new(f64::from(stage.number("plot-width")), f64::from(stage.number("plot-height")));
        self.strokes = self.editor.strokes(self.size, &self.settings);
        let first = self.strokes.first().ok_or("an editor's demo draws a stroke")?.0;
        let rest = self.in_scene(first);
        let rest = (rest.0 + 48.0, rest.1 - 44.0);
        // Each stroke: reach its start, press, drag, hold, let go; spread over the clip's first four seconds.
        let span = 3.9 / self.strokes.len() as f32;
        let mut path = Track::from(rest).hold(0.3);
        let mut presses = Vec::new();
        for (k, (a, b)) in self.strokes.iter().enumerate() {
            let t = 0.3 + k as f32 * span;
            path = path.to(t + 0.2 * span, self.in_scene(*a)).hold(t + 0.3 * span).to(t + 0.75 * span, self.in_scene(*b)).hold(t + span);
            presses.push((t + 0.25 * span, t + 0.95 * span));
        }
        self.path = path.to(4.8, rest);
        self.presses = Presses(presses);
        Ok(())
    }

    /// Every step redraws the whole photograph: fewer colours pay for them.
    fn tuning(&self) -> Tuning {
        Tuning { tolerance: 14, colours: 96 }
    }

    fn hand(&self, t: f32) -> Hand {
        Hand { at: self.path.at(t), down: self.presses.down(t) }
    }

    fn pose(&mut self, t: f32, stage: &Stage, photo: &mut Photo) -> Result<(), String> {
        let (strokes, held, key) = self.drawn(t, self.hand(t));
        let (_, drawing) = self.editor.edit(&self.settings, &strokes, held, self.size);
        let (lines, dots) = plot::values(&drawing);
        stage.set("lines", lines);
        stage.set("dots", dots);
        if self.shown == Some(key) {
            return Ok(());
        }
        if !self.renders.contains_key(&key) {
            // The photograph's strokes: those done whole, the next as far as its step.
            let mut shown: Vec<Stroke> = self.strokes[..key.0].to_vec();
            // A press alone may already change the drawing, a point laid on the curve: so the photograph.
            if key.1 > 0 {
                let (a, b) = self.strokes[key.0];
                let f = f64::from(key.1 - 1) / LEVELS;
                shown.push((a, (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f)));
            }
            let (s, _) = self.editor.edit(&self.settings, &shown, false, self.size);
            self.renders.insert(key, photo.cover(&s, self.pixels.0, self.pixels.1)?);
        }
        stage.set_image("photo", &self.renders[&key]);
        self.shown = Some(key);
        Ok(())
    }
}
