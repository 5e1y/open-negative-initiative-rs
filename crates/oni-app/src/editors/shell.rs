//! The editors in the window: each callback hands its gesture to `Gestures`, then the window draws
//! the hand's value at once and publishes the store only when it was written.

use super::geometry::{Dot, Drawing, Ink, Line, Size};
use super::sprung::Sprung;
use super::{Gestures, Plot, Reset, Then, curve, levels, print, saturation, spectrogram};
use crate::app::Ctl;
use crate::{Editors as Global, Ink as PlotInk, MainWindow, PlotDot, PlotKnob, PlotLine, PlotReadout};
use oni_settings::ConversionMode;
use oni_settings::curves::{Curve, SaturationCurve};
use oni_settings::levels::Levels;
use oni_settings::spectrogram::{Band, Spectrogram};
use oni_settings::tone::Print;
use slint::{Color, ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::Instant;

pub(crate) mod ink;
mod drawing;
mod running;
mod wiring;

pub use drawing::*;
pub use ink::*;
use running::*;
pub use wiring::*;

/// The editors' share of the window: their gestures, the trailing hand-off and what each plot last drew.
#[derive(Default)]
pub struct Shell {
    pub gestures: RefCell<Gestures>,
    trailing: Timer,
    /// Closes a lifted hand's step once the edit has rested.
    settling: Timer,
    drawn: RefCell<Drawn>,
    /// The open frame's path and whether it is an 8-bit scan, read once per frame.
    precision: RefCell<Option<(String, bool)>>,
    /// The levels handles and the band sliders' knobs, sprung to a value from elsewhere.
    knobs: RefCell<Knobs>,
    springing: Timer,
    /// The next drawing places every knob at once: a photo opened.
    jump: std::cell::Cell<bool>,
}

#[derive(Default)]
struct Knobs {
    levels: [Sprung; 5],
    bands: [Sprung; super::BAND_FIELDS],
    /// The track the handles were placed on: a new width lays them where they stand.
    levels_on: Option<Size>,
    /// A spring showed a frame short of its rest, so the next drawing lays the rest itself.
    levels_flying: bool,
    bands_flying: bool,
}

/// `FRAME_TICK`: a sprung knob is redrawn once a frame until it rests.
const FRAME: std::time::Duration = std::time::Duration::from_millis(16);

impl Shell {
    fn low_precision(&self, path: Option<&str>) -> bool {
        let Some(path) = path else { return false };
        let mut known = self.precision.borrow_mut();
        match known.as_ref() {
            Some((p, low)) if p == path => *low,
            _ => {
                let low = oni_decode::is_low_precision(std::path::Path::new(path));
                *known = Some((path.to_string(), low));
                low
            }
        }
    }
}

/// What each plot was last drawn from, so a notch elsewhere redraws nothing here.
#[derive(Default)]
struct Drawn {
    curves: Option<(Curve, usize, Size, Vec<bool>)>,
    levels: Option<(Levels, usize, ConversionMode, f32, Size, Size, Option<levels::Grip>, Vec<bool>, bool)>,
    wheel: Option<(Spectrogram, Size, Option<spectrogram::Grip>, Option<spectrogram::Grip>, Band, Option<(usize, f32)>)>,
    saturation: Option<(SaturationCurve, Size, Option<usize>)>,
    print: Option<(Print, bool, Size)>,
}

/// Where a plot sits in the window, for a check that aims the pointer at it.
pub fn origin(ctl: &Ctl, plot: Plot) -> (f64, f64) {
    ctl.editors.gestures.borrow().origins[plot as usize]
}

/// A plot's laid-out size.
pub fn size(ctl: &Ctl, plot: Plot) -> Size {
    ctl.editors.gestures.borrow().sizes[plot as usize]
}

/// The store writes the editors made, the hand's cadence's check.
pub fn writes(ctl: &Ctl) -> usize {
    ctl.editors.gestures.borrow().writes
}
