//! The panel's editors: the manual curves, the levels (a film's per channel, a digital photo's on the
//! picture), the spectrogram wheel and its band sliders, the saturation curve, the grain curve and the
//! print curve over the main histogram.

mod bands;
pub mod curve;
pub mod geometry;
pub mod grain;
pub mod levels;
pub mod points;
pub mod print;
pub mod saturation;
pub mod shell;
pub mod spectrogram;
mod stored;

use oni_core::EditorStore;
use crate::track::{self, DOUBLE_CLICK, Hand, Offer, TRACKPAD_TRAVEL};
use geometry::Size;
use oni_core::Command;
use oni_settings::PipelineSettings;
use oni_settings::blocks::GrainCurve;
use oni_settings::curves::{CurveSet, SaturationCurve};
use oni_settings::levels::LevelsChannel;
use oni_settings::spectrogram::{Band, Spectrogram};
use std::time::Instant;

pub use bands::{BAND_FIELDS, band_range, band_readout, band_value};
pub use shell::{Shell, let_go, origin, pressed_anywhere, publish, size, wire, writes};
pub use stored::{RESETS, face, lay, levels_modified, rest_of, rested_levels, rested_windows, resting_levels};

/// The plots, as the interface numbers them in every callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plot {
    Curves = 0,
    Levels = 1,
    Wheel = 2,
    Saturation = 3,
    LevelsHistogram = 4,
    Print = 5,
    Grain = 6,
}

impl Plot {
    pub const ALL: [Plot; 7] = [Plot::Curves, Plot::Levels, Plot::Wheel, Plot::Saturation, Plot::LevelsHistogram, Plot::Print, Plot::Grain];

    pub fn from_index(i: i32) -> Option<Plot> {
        Self::ALL.get(usize::try_from(i).ok()?).copied()
    }
}

/// The blocks whose reset keys the editors own, in the interface's numbering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reset {
    Curves = 0,
    Levels = 1,
    Spectrogram = 2,
    Saturation = 3,
    Grain = 4,
}

/// What a gesture is changing, kept whole so the store takes it in one write.
#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    Curves(CurveSet),
    Levels(levels::Windows),
    Spectrogram(Spectrogram),
    Saturation(SaturationCurve),
    Grain(GrainCurve),
}

/// The press under way, decided once at the touch.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Held {
    Curve { index: Option<usize>, double: bool, at: (f64, f64) },
    Level { grip: levels::Grip, start: f32, from: f64 },
    Wheel { grab: spectrogram::Grab, from: (f64, f64) },
    /// On the saturation's or the grain's five points.
    Points { double: bool, at: (f64, f64) },
    Band { field: usize, start: f32 },
}

/// The editors' gestures, apart from the window: each takes the store and the time it happens at,
/// writes at the hand's cadence and makes one history step per press.
#[derive(Default)]
pub struct Gestures {
    pub sizes: [Size; Plot::ALL.len()],
    pub origins: [(f64, f64); Plot::ALL.len()],
    pub curve_channel: usize,
    pub levels_channel: usize,
    /// The spectrogram's selected point; it is lit only while the wheel holds the focus.
    pub selected: spectrogram::Grip,
    pub focused: bool,
    pub hovered: Option<spectrogram::Grip>,
    held: Option<Held>,
    live: Option<Edit>,
    hand: Hand,
    /// The last press on each plot, for a double click.
    presses: [Option<Instant>; Plot::ALL.len()],
    band_presses: [Option<Instant>; BAND_FIELDS],
    /// A band slider's knob under the hand, which stays where the pointer put it.
    pub band_shown: Option<(usize, f32)>,
    saturation: points::Presses,
    grain: points::Presses,
    /// A scroll moved a knob: the hand drives it, so the next drawing places it unsprung.
    pub scrolled: bool,
    /// Store writes this session, the cadence's check.
    pub writes: usize,
}

/// What the window does after a gesture's event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Then {
    /// Nothing changed.
    Nothing,
    /// The editor redraws from the hand; the store was not written.
    Draw,
    /// The store was written: everything that reads it is published.
    Publish,
    /// The editor redraws, and one trailing write is scheduled after this delay.
    Trail(std::time::Duration),
    /// The press ended: the store took its last value, the step closing once it rests.
    Released,
}

impl Gestures {
    /// The face's settings with the hand's edit laid on: what the editors draw.
    pub fn view(&self, store: &mut EditorStore) -> PipelineSettings {
        let scanner = store.scanner_face;
        let mut f = face(store.settings(), scanner);
        match &self.live {
            Some(Edit::Curves(c)) => f.manual_curves = c.clone(),
            Some(Edit::Levels(l)) => l.lay(&mut f),
            Some(Edit::Spectrogram(s)) => f.spectrogram = *s,
            Some(Edit::Saturation(s)) => f.saturation = s.clone(),
            Some(Edit::Grain(c)) => f.grain.curve = *c,
            None => {}
        }
        f
    }

    pub fn holding(&self) -> bool {
        self.held.is_some()
    }

    /// The levels handle under the hand, drawn grown.
    pub fn held_grip(&self) -> Option<levels::Grip> {
        match self.held {
            Some(Held::Level { grip, .. }) => Some(grip),
            _ => None,
        }
    }

    pub fn curve_channel(&self) -> LevelsChannel {
        curve::CHANNELS[self.curve_channel.min(curve::CHANNELS.len() - 1)]
    }

    pub fn wheel(&self) -> spectrogram::Wheel {
        spectrogram::Wheel { side: self.sizes[Plot::Wheel as usize].w }
    }

    /// The point the wheel lights: the selection, while the wheel holds the focus.
    pub fn active(&self) -> Option<spectrogram::Grip> {
        self.focused.then_some(self.selected)
    }

    fn write(&mut self, store: &mut EditorStore, edit: &Edit) {
        let scanner = store.scanner_face;
        match edit {
            // Each handle is a knob, which frames together move by its distance from their own window.
            Edit::Levels(set) => levels::steps(store.settings(), set).into_iter().for_each(|c| {
                store.apply(c);
            }),
            _ => {
                store.apply(Command::Write(Box::new(lay(store.settings(), scanner, edit))));
            }
        }
        self.writes += 1;
    }

    /// One write outside a press: a reset, an arrow, a scroll notch, its step closing once it
    /// rests like a press's.
    fn once(&mut self, store: &mut EditorStore, edit: Edit) -> Then {
        store.defer();
        self.write(store, &edit);
        Then::Publish
    }

    fn double(&mut self, slot: usize, now: Instant) -> bool {
        // `DSClick` reads AppKit's click count, which keeps counting past two.
        let double = self.presses[slot].is_some_and(|t| now.duration_since(t) <= DOUBLE_CLICK);
        self.presses[slot] = Some(now);
        double
    }

    /// The hand's value to the store at the cadence, or a trailing write scheduled.
    fn offer(&mut self, store: &mut EditorStore, now: Instant) -> Then {
        match self.hand.offer(now) {
            Offer::Now => {
                if let Some(edit) = self.live.clone() {
                    self.write(store, &edit);
                }
                Then::Publish
            }
            Offer::After(delay) => Then::Trail(delay),
            Offer::Wait => Then::Draw,
        }
    }

    /// The trailing hand-off fired: the latest value the hand left.
    pub fn trailed(&mut self, store: &mut EditorStore, now: Instant) -> Then {
        self.hand.trailed(now);
        match self.live.clone() {
            Some(edit) if self.held.is_some() => {
                self.write(store, &edit);
                Then::Publish
            }
            _ => Then::Nothing,
        }
    }

    fn begin(&mut self, store: &mut EditorStore, held: Held, edit: Edit) {
        store.hold();
        self.held = Some(held);
        self.live = Some(edit);
    }

    // MARK: - Pointer

    pub fn press(&mut self, store: &mut EditorStore, plot: Plot, at: (f64, f64), now: Instant) -> Then {
        let size = self.sizes[plot as usize];
        let view = self.view(store);
        match plot {
            Plot::Curves => {
                let double = self.double(0, now);
                let channel = self.curve_channel();
                let mut curves = view.manual_curves.clone();
                let index = curve::press(curves.get_mut(channel), at, size);
                self.begin(store, Held::Curve { index, double, at }, Edit::Curves(curves));
                self.offer(store, now)
            }
            Plot::Levels => {
                let windows = levels::Windows::of(&view);
                let channel = windows.channel(self.levels_channel);
                let window = windows.get(channel);
                let Some(grip) = levels::caught(at.0, at.1, size.w, &window, channel, view.mode) else { return Then::Nothing };
                if self.double(1, now) {
                    store.defer();
                    store.apply(levels::rested(grip, channel, &view));
                    self.writes += 1;
                    return Then::Publish;
                }
                let start = levels::place(grip, &window, channel, view.mode);
                self.begin(store, Held::Level { grip, start, from: at.0 }, Edit::Levels(windows));
                Then::Draw
            }
            Plot::Wheel => {
                let double = self.double(2, now);
                let wheel = self.wheel();
                let grab = spectrogram::grab(wheel, at, &view.spectrogram);
                self.selected = grab.grip;
                self.focused = true;
                self.hovered = None;
                let s = if double { spectrogram::reset(self.selected, &view.spectrogram) } else { view.spectrogram };
                self.begin(store, Held::Wheel { grab, from: at }, Edit::Spectrogram(s));
                if double { self.offer(store, now) } else { Then::Draw }
            }
            Plot::Saturation | Plot::Grain => self.points_press(store, plot, &view, at, now),
            Plot::LevelsHistogram | Plot::Print => Then::Nothing,
        }
    }

    pub fn drag(&mut self, store: &mut EditorStore, plot: Plot, at: (f64, f64), fine: bool, now: Instant) -> Then {
        let size = self.sizes[plot as usize];
        let (Some(held), Some(live)) = (self.held, self.live.clone()) else { return Then::Nothing };
        let next = match (held, live) {
            (Held::Curve { index: Some(index), .. }, Edit::Curves(mut curves)) if plot == Plot::Curves => {
                curve::drag(curves.get_mut(self.curve_channel()), index, at, size);
                Edit::Curves(curves)
            }
            (Held::Level { grip, start, from }, Edit::Levels(mut windows)) if plot == Plot::Levels => {
                let channel = windows.channel(self.levels_channel);
                let mode = store.settings().mode;
                let asked = track::clamped(track::asked(start, (at.0 - from) as f32, size.w as f32, fine));
                windows.set(channel, levels::moved(grip, &windows.get(channel), asked, channel, mode));
                Edit::Levels(windows)
            }
            (Held::Wheel { grab, from }, Edit::Spectrogram(s)) if plot == Plot::Wheel => {
                Edit::Spectrogram(spectrogram::dragged(self.wheel(), grab, (at.0 - from.0, at.1 - from.1), &s))
            }
            (Held::Points { .. }, edit) => match self.points_drag(plot, edit, at) {
                Some(edit) => edit,
                None => return Then::Nothing,
            },
            _ => return Then::Nothing,
        };
        if Some(&next) == self.live.as_ref() {
            return Then::Nothing;
        }
        self.live = Some(next);
        self.offer(store, now)
    }

    /// The press's end, on whichever plot holds it: a double click's second release removes the disc
    /// under it, or puts a five-point curve's point back at its rest; then the last value lands and the step closes.
    pub fn release(&mut self, store: &mut EditorStore) -> Then {
        let Some(held) = self.held.take() else { return Then::Nothing };
        let channel = self.curve_channel();
        self.band_shown = None;
        match (held, self.live.as_mut()) {
            (Held::Curve { double: true, at, .. }, Some(Edit::Curves(curves))) => {
                let curve = curves.get_mut(channel);
                if let Some(i) = curve::dot_under(curve, at, self.sizes[Plot::Curves as usize]) {
                    curve.remove(i);
                }
            }
            (Held::Points { double: true, at }, _) => self.points_double(at),
            _ => {}
        }
        self.points_released();
        self.hand.released();
        if let Some(edit) = self.live.take() {
            self.write(store, &edit);
        }
        store.release_deferred();
        Then::Released
    }

    /// The pointer over the wheel without a press: the point it would take lights up.
    pub fn hover(&mut self, store: &mut EditorStore, at: Option<(f64, f64)>) -> Then {
        let hovered = match at {
            Some(at) if self.held.is_none() => Some(self.wheel().nearest(at, &self.view(store).spectrogram)),
            _ => None,
        };
        if hovered == self.hovered {
            return Then::Nothing;
        }
        self.hovered = hovered;
        Then::Draw
    }

    /// A scroll along the levels track moves the handle under the pointer, else the nearest, through
    /// the same path as a drag; one across it is left to the panel.
    pub fn scroll(&mut self, store: &mut EditorStore, plot: Plot, x: f64, dx: f64, dy: f64, fine: bool) -> Option<Then> {
        if plot != Plot::Levels || dx == 0.0 || dx.abs() <= dy.abs() || self.held.is_some() {
            return None;
        }
        let view = self.view(store);
        let mut windows = levels::Windows::of(&view);
        let channel = windows.channel(self.levels_channel);
        let width = self.sizes[plot as usize].w;
        let window = windows.get(channel);
        let grip = levels::scrolled(x, width, &window, channel, view.mode);
        let from = levels::place(grip, &window, channel, view.mode);
        let asked = track::clamped(track::asked(from, dx as f32 * TRACKPAD_TRAVEL, width as f32, fine));
        windows.set(channel, levels::moved(grip, &window, asked, channel, view.mode));
        self.scrolled = true;
        Some(self.once(store, Edit::Levels(windows)))
    }

    /// A tab of the curves, the levels or the bands.
    pub fn pick(&mut self, which: i32, tab: usize) -> Then {
        match which {
            0 => self.curve_channel = tab.min(curve::CHANNELS.len() - 1),
            1 => self.levels_channel = tab.min(LevelsChannel::PICTURE.len() - 1),
            _ => self.selected.band = Band::ALL[tab.min(Band::ALL.len() - 1)],
        }
        Then::Draw
    }

    /// A reset key: the block at each frame's own rest, every frame shown with the open one.
    pub fn reset(&mut self, store: &mut EditorStore, block: Reset) -> Then {
        let scanner = store.scanner_face;
        store.defer();
        store.apply(Command::Each(rest_of(block, scanner)));
        self.writes += 1;
        Then::Publish
    }
}

#[cfg(test)]
mod tests;
