//! The panel's editors: the manual curves, the per-channel levels, the spectrogram wheel and its
//! band sliders, the saturation curve and the print curve over the main histogram.

pub mod curve;
pub mod geometry;
pub mod levels;
pub mod print;
pub mod saturation;
pub mod shell;
pub mod spectrogram;
pub mod sprung;

use crate::store::Store;
use crate::track::{self, DOUBLE_CLICK, Hand, Offer, TRACKPAD_TRAVEL};
use geometry::Size;
use oni_core::Command;
use oni_settings::PipelineSettings;
use oni_settings::curves::{CurveSet, SaturationCurve};
use oni_settings::levels::{LevelsChannel, LevelsSet};
use oni_settings::spectrogram::{Band, Spectrogram};
use std::time::Instant;

pub use shell::{Shell, origin, pressed_anywhere, publish, size, wire, writes};

/// The plots, as the interface numbers them in every callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plot {
    Curves = 0,
    Levels = 1,
    Wheel = 2,
    Saturation = 3,
    LevelsHistogram = 4,
    Print = 5,
}

impl Plot {
    pub const ALL: [Plot; 6] = [Plot::Curves, Plot::Levels, Plot::Wheel, Plot::Saturation, Plot::LevelsHistogram, Plot::Print];

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
}

/// What a gesture is changing, kept whole so the store takes it in one write.
#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    Curves(CurveSet),
    Levels(LevelsSet),
    Spectrogram(Spectrogram),
    Saturation(SaturationCurve),
}

/// The settings as the open face edits them: the Scanner face reads its own finishing.
pub fn face(settings: &PipelineSettings, scanner: bool) -> PipelineSettings {
    if scanner { settings.as_scanner() } else { settings.clone() }
}

/// The stored settings with `edit` laid on the face, the other face's finishing kept.
pub fn lay(settings: &PipelineSettings, scanner: bool, edit: &Edit) -> PipelineSettings {
    let mut f = face(settings, scanner);
    match edit {
        Edit::Curves(c) => f.manual_curves = c.clone(),
        Edit::Levels(l) => f.levels = *l,
        Edit::Spectrogram(s) => f.spectrogram = *s,
        Edit::Saturation(s) => f.saturation = s.clone(),
    }
    if scanner { settings.storing_scanner_edit(&f) } else { f }
}

/// `restingLevels`: a measured roll's window when the frame stands on one, else the bare axis.
pub fn resting_levels(settings: &PipelineSettings) -> LevelsSet {
    match &settings.roll_reference {
        Some(reference) => oni_core::balance::written(reference, settings).levels,
        None => LevelsSet::neutral(settings.mode, settings.density_ceiling),
    }
}

/// The Per-Channel Levels block's three windows at their rest, the linked and luma ones kept.
pub fn rested_levels(settings: &PipelineSettings) -> LevelsSet {
    let rest = resting_levels(settings);
    let mut out = settings.levels;
    for c in LevelsChannel::PER_CHANNEL {
        out.set(c, rest.get(c));
    }
    out
}

/// The band slider's three axes, in their order on screen.
pub const BAND_FIELDS: usize = 3;

pub fn band_range(field: usize, band: Band) -> (f32, f32) {
    match field {
        0 => band.hue_range(),
        1 => Spectrogram::SATURATION_RANGE,
        _ => Spectrogram::LUMA_RANGE,
    }
}

pub fn band_value(s: &Spectrogram, band: Band, field: usize) -> f32 {
    let (h, sat, l) = s.band(band);
    [h, sat, l][field.min(2)]
}

fn with_band_value(s: &Spectrogram, band: Band, field: usize, v: f32) -> Spectrogram {
    let (mut h, mut sat, mut l) = s.band(band);
    match field {
        0 => h = v,
        1 => sat = v,
        _ => l = v,
    }
    let mut out = *s;
    out.set(band, h, sat, l);
    out
}

/// `SpectrogramBands`' readouts: the hue in degrees, the others in percent, signed.
pub fn band_readout(field: usize, v: f32) -> String {
    if field == 0 { format!("{v:+.0}°") } else { format!("{:+.0}%", v * 100.0) }
}

/// The press under way, decided once at the touch.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Held {
    Curve { index: Option<usize>, double: bool, at: (f64, f64) },
    Level { grip: levels::Grip, start: f32, from: f64 },
    Wheel { grab: spectrogram::Grab, from: (f64, f64) },
    Saturation { double: bool, at: (f64, f64) },
    Band { field: usize, start: f32 },
}

/// The editors' gestures, apart from the window: each takes the store and the time it happens at,
/// writes at the hand's cadence and makes one history step per press.
#[derive(Default)]
pub struct Gestures {
    pub sizes: [Size; 6],
    pub origins: [(f64, f64); 6],
    pub curve_channel: usize,
    pub levels_channel: usize,
    /// The spectrogram's selected point; it is lit only while the wheel holds the focus.
    pub selected: spectrogram::Grip,
    pub focused: bool,
    pub hovered: Option<spectrogram::Grip>,
    held: Option<Held>,
    live: Option<Edit>,
    hand: Hand,
    presses: [Option<Instant>; 4],
    band_presses: [Option<Instant>; BAND_FIELDS],
    /// A band slider's knob under the hand, which stays where the pointer put it.
    pub band_shown: Option<(usize, f32)>,
    saturation: saturation::Presses,
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
    fn scanner(store: &mut dyn Store) -> bool {
        store.editor().is_some_and(|e| e.scanner_face)
    }

    /// The face's settings with the hand's edit laid on: what the editors draw.
    pub fn view(&self, store: &mut dyn Store) -> PipelineSettings {
        let scanner = Self::scanner(store);
        let mut f = face(store.settings(), scanner);
        match &self.live {
            Some(Edit::Curves(c)) => f.manual_curves = c.clone(),
            Some(Edit::Levels(l)) => f.levels = *l,
            Some(Edit::Spectrogram(s)) => f.spectrogram = *s,
            Some(Edit::Saturation(s)) => f.saturation = s.clone(),
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

    /// The saturation point under the hand, drawn lit.
    pub fn saturation_dragged(&self) -> Option<usize> {
        if matches!(self.held, Some(Held::Saturation { .. })) { self.saturation.dragged } else { None }
    }

    pub fn curve_channel(&self) -> LevelsChannel {
        curve::CHANNELS[self.curve_channel.min(curve::CHANNELS.len() - 1)]
    }

    pub fn levels_channel(&self) -> LevelsChannel {
        levels::CHANNELS[self.levels_channel.min(levels::CHANNELS.len() - 1)]
    }

    pub fn wheel(&self) -> spectrogram::Wheel {
        spectrogram::Wheel { side: self.sizes[Plot::Wheel as usize].w }
    }

    /// The point the wheel lights: the selection, while the wheel holds the focus.
    pub fn active(&self) -> Option<spectrogram::Grip> {
        self.focused.then_some(self.selected)
    }

    fn write(&mut self, store: &mut dyn Store, edit: &Edit) {
        let scanner = Self::scanner(store);
        let next = lay(store.settings(), scanner, edit);
        store.apply(Command::Write(Box::new(next)));
        self.writes += 1;
    }

    /// One write outside a press: a reset, an arrow, a scroll notch, its step closing once it
    /// rests like a press's.
    fn once(&mut self, store: &mut dyn Store, edit: Edit) -> Then {
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
    fn offer(&mut self, store: &mut dyn Store, now: Instant) -> Then {
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
    pub fn trailed(&mut self, store: &mut dyn Store, now: Instant) -> Then {
        self.hand.trailed(now);
        match self.live.clone() {
            Some(edit) if self.held.is_some() => {
                self.write(store, &edit);
                Then::Publish
            }
            _ => Then::Nothing,
        }
    }

    fn begin(&mut self, store: &mut dyn Store, held: Held, edit: Edit) {
        store.hold();
        self.held = Some(held);
        self.live = Some(edit);
    }

    // MARK: - Pointer

    pub fn press(&mut self, store: &mut dyn Store, plot: Plot, at: (f64, f64), now: Instant) -> Then {
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
                let channel = self.levels_channel();
                let window = view.levels.get(channel);
                let Some(grip) = levels::caught(at.0, at.1, size.w, &window, channel, view.mode) else { return Then::Nothing };
                if self.double(1, now) {
                    let rested = levels::reset(grip, &window, channel, view.mode, view.density_ceiling);
                    let mut set = view.levels;
                    set.set(channel, rested);
                    return self.once(store, Edit::Levels(set));
                }
                let start = levels::place(grip, &window, channel, view.mode);
                self.begin(store, Held::Level { grip, start, from: at.0 }, Edit::Levels(view.levels));
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
            Plot::Saturation => {
                let double = self.double(3, now);
                let mut curve = view.saturation.clone();
                self.saturation.press(&mut curve, at, size);
                self.begin(store, Held::Saturation { double, at }, Edit::Saturation(curve));
                self.offer(store, now)
            }
            Plot::LevelsHistogram | Plot::Print => Then::Nothing,
        }
    }

    pub fn drag(&mut self, store: &mut dyn Store, plot: Plot, at: (f64, f64), fine: bool, now: Instant) -> Then {
        let size = self.sizes[plot as usize];
        let (Some(held), Some(live)) = (self.held, self.live.clone()) else { return Then::Nothing };
        let next = match (held, live) {
            (Held::Curve { index: Some(index), .. }, Edit::Curves(mut curves)) if plot == Plot::Curves => {
                curve::drag(curves.get_mut(self.curve_channel()), index, at, size);
                Edit::Curves(curves)
            }
            (Held::Level { grip, start, from }, Edit::Levels(mut set)) if plot == Plot::Levels => {
                let channel = self.levels_channel();
                let mode = store.settings().mode;
                let asked = track::clamped(track::asked(start, (at.0 - from) as f32, size.w as f32, fine));
                set.set(channel, levels::moved(grip, &set.get(channel), asked, channel, mode));
                Edit::Levels(set)
            }
            (Held::Wheel { grab, from }, Edit::Spectrogram(s)) if plot == Plot::Wheel => {
                Edit::Spectrogram(spectrogram::dragged(self.wheel(), grab, (at.0 - from.0, at.1 - from.1), &s))
            }
            (Held::Saturation { .. }, Edit::Saturation(mut curve)) if plot == Plot::Saturation => {
                self.saturation.drag(&mut curve, at, size);
                Edit::Saturation(curve)
            }
            _ => return Then::Nothing,
        };
        if Some(&next) == self.live.as_ref() {
            return Then::Nothing;
        }
        self.live = Some(next);
        self.offer(store, now)
    }

    /// The press's end: a double click's second release removes the disc under it, or puts the
    /// saturation point back on neutral; then the last value lands and the step closes.
    pub fn release(&mut self, store: &mut dyn Store, plot: Plot) -> Then {
        let Some(held) = self.held.take() else { return Then::Nothing };
        let size = self.sizes[plot as usize];
        let channel = self.curve_channel();
        match (held, self.live.as_mut()) {
            (Held::Curve { double: true, at, .. }, Some(Edit::Curves(curves))) => {
                let curve = curves.get_mut(channel);
                if let Some(i) = curve::dot_under(curve, at, size) {
                    curve.remove(i);
                }
            }
            (Held::Saturation { double: true, at }, Some(Edit::Saturation(curve))) => {
                if let Some(next) = self.saturation.double(curve, at, size) {
                    *curve = next;
                }
            }
            _ => {}
        }
        self.saturation.release();
        self.hand.released();
        if let Some(edit) = self.live.take() {
            self.write(store, &edit);
        }
        store.release_deferred();
        Then::Released
    }

    /// The pointer over the wheel without a press: the point it would take lights up.
    pub fn hover(&mut self, store: &mut dyn Store, at: Option<(f64, f64)>) -> Then {
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
    pub fn scroll(&mut self, store: &mut dyn Store, plot: Plot, x: f64, dx: f64, dy: f64, fine: bool) -> Option<Then> {
        if plot != Plot::Levels || dx == 0.0 || dx.abs() <= dy.abs() || self.held.is_some() {
            return None;
        }
        let view = self.view(store);
        let channel = self.levels_channel();
        let width = self.sizes[plot as usize].w;
        let window = view.levels.get(channel);
        let grip = levels::scrolled(x, width, &window, channel, view.mode);
        let from = levels::place(grip, &window, channel, view.mode);
        let asked = track::clamped(track::asked(from, dx as f32 * TRACKPAD_TRAVEL, width as f32, fine));
        let mut set = view.levels;
        set.set(channel, levels::moved(grip, &window, asked, channel, view.mode));
        self.scrolled = true;
        Some(self.once(store, Edit::Levels(set)))
    }

    /// The wheel's keys while it holds the focus: the arrows step the selected point, Tab walks the
    /// wheel; every other key goes on to the window.
    pub fn key(&mut self, store: &mut dyn Store, text: &str, option: bool) -> Option<Then> {
        use slint::platform::Key;
        if !self.focused {
            return None;
        }
        let is = |k: Key| text == slint::SharedString::from(k).as_str();
        let nudge = if is(Key::LeftArrow) {
            spectrogram::Nudge::Left
        } else if is(Key::RightArrow) {
            spectrogram::Nudge::Right
        } else if is(Key::UpArrow) {
            spectrogram::Nudge::Up
        } else if is(Key::DownArrow) {
            spectrogram::Nudge::Down
        } else if is(Key::Tab) {
            self.selected = spectrogram::walked(self.selected, option);
            return Some(Then::Draw);
        } else {
            return None;
        };
        let s = spectrogram::nudge(self.selected, nudge, &self.view(store).spectrogram);
        Some(self.once(store, Edit::Spectrogram(s)))
    }

    /// `dsReleasesFocus`: a press anywhere in the window off the wheel releases its focus, so its
    /// point unlights and the arrows go back to the window.
    pub fn pressed_anywhere(&mut self, at: (f64, f64)) -> bool {
        let (o, s) = (self.origins[Plot::Wheel as usize], self.sizes[Plot::Wheel as usize]);
        let inside = (o.0..=o.0 + s.w).contains(&at.0) && (o.1..=o.1 + s.h).contains(&at.1);
        if !self.focused || inside {
            return false;
        }
        self.focused = false;
        true
    }

    /// A tab of the curves, the levels or the bands.
    pub fn pick(&mut self, which: i32, tab: usize) -> Then {
        match which {
            0 => self.curve_channel = tab.min(curve::CHANNELS.len() - 1),
            1 => self.levels_channel = tab.min(levels::CHANNELS.len() - 1),
            _ => self.selected.band = Band::ALL[tab.min(Band::ALL.len() - 1)],
        }
        Then::Draw
    }

    pub fn reset(&mut self, store: &mut dyn Store, block: Reset) -> Then {
        let view = self.view(store);
        let edit = match block {
            Reset::Curves => Edit::Curves(CurveSet::default()),
            Reset::Levels => Edit::Levels(rested_levels(&view)),
            Reset::Spectrogram => Edit::Spectrogram(Spectrogram::default()),
            Reset::Saturation => Edit::Saturation(SaturationCurve::default()),
        };
        self.once(store, edit)
    }

    // MARK: - The band sliders

    /// A press on a band slider; the second of a double click puts the axis back on zero.
    pub fn band_press(&mut self, store: &mut dyn Store, field: usize, now: Instant) -> (bool, Then) {
        let field = field.min(BAND_FIELDS - 1);
        let double = self.band_presses[field].is_some_and(|t| now.duration_since(t) <= DOUBLE_CLICK);
        self.band_presses[field] = Some(now);
        let s = self.view(store).spectrogram;
        let band = self.selected.band;
        if double {
            return (true, self.once(store, Edit::Spectrogram(with_band_value(&s, band, field, 0.0))));
        }
        let range = band_range(field, band);
        let start = track::position(band_value(&s, band, field), &(range.0..=range.1), None);
        self.band_shown = Some((field, start));
        self.begin(store, Held::Band { field, start }, Edit::Spectrogram(s));
        (false, Then::Draw)
    }

    pub fn band_drag(&mut self, store: &mut dyn Store, field: usize, translation: f32, span: f32, fine: bool, now: Instant) -> Then {
        let (Some(Held::Band { field: held, start }), Some(Edit::Spectrogram(s))) = (self.held, self.live.clone()) else { return Then::Nothing };
        if held != field {
            return Then::Nothing;
        }
        let band = self.selected.band;
        let range = band_range(field, band);
        let asked = track::clamped(track::asked(start, translation, span, fine));
        self.band_shown = Some((field, asked));
        let next = with_band_value(&s, band, field, track::value(asked, &(range.0..=range.1), None));
        if next == s {
            return Then::Draw;
        }
        self.live = Some(Edit::Spectrogram(next));
        self.offer(store, now)
    }

    pub fn band_release(&mut self, store: &mut dyn Store) -> Then {
        if !matches!(self.held, Some(Held::Band { .. })) {
            return Then::Nothing;
        }
        self.band_shown = None;
        self.release(store, Plot::Wheel)
    }

    pub fn band_scroll(&mut self, store: &mut dyn Store, field: usize, dx: f32, dy: f32, span: f32, fine: bool) -> bool {
        if dx == 0.0 || dx.abs() <= dy.abs() || self.held.is_some() {
            return false;
        }
        let s = self.view(store).spectrogram;
        let band = self.selected.band;
        let range = band_range(field, band);
        let from = track::position(band_value(&s, band, field), &(range.0..=range.1), None);
        let asked = track::clamped(track::asked(from, dx * TRACKPAD_TRAVEL, span, fine));
        let v = track::value(asked, &(range.0..=range.1), None);
        self.scrolled = true;
        self.once(store, Edit::Spectrogram(with_band_value(&s, band, field, v)));
        true
    }

    /// An accessibility step: a hundredth of the axis.
    pub fn band_step(&mut self, store: &mut dyn Store, field: usize, direction: i32) -> Then {
        let s = self.view(store).spectrogram;
        let band = self.selected.band;
        let (low, high) = band_range(field, band);
        let v = (band_value(&s, band, field) + direction as f32 * (high - low) / 100.0).clamp(low, high);
        self.once(store, Edit::Spectrogram(with_band_value(&s, band, field, v)))
    }
}

#[cfg(test)]
mod tests;
