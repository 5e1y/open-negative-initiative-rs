//! The selected band's three sliders under the wheel: a press, a drag, a scroll or a step on its hue,
//! saturation or luminance, and a double click that zeroes it.

use super::{Edit, Gestures, Held, Then};
use crate::track::{self, DOUBLE_CLICK, TRACKPAD_TRAVEL};
use oni_core::EditorStore;
use oni_settings::spectrogram::{Band, Spectrogram};
use std::time::Instant;

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
    if field == 0 { crate::fields::number(f64::from(v), 0, true, crate::fields::Unit::Degrees) } else { crate::fields::Readout::SignedPercent.format(v) }
}

impl Gestures {
    /// A press on a band slider; the second of a double click puts the axis back on zero.
    pub fn band_press(&mut self, store: &mut EditorStore, field: usize, now: Instant) -> (bool, Then) {
        let field = field.min(BAND_FIELDS - 1);
        let double = self.band_presses[field].is_some_and(|t| now.duration_since(t) <= DOUBLE_CLICK);
        self.band_presses[field] = Some(now);
        let s = self.view(store).spectrogram;
        let band = self.selected.band;
        if double {
            return (true, self.once(store, Edit::Spectrogram(with_band_value(&s, band, field, 0.0))));
        }
        let range = band_range(field, band);
        let start = track::position(band_value(&s, band, field), &(range.0..=range.1), track::Bend::Even);
        self.band_shown = Some((field, start));
        self.begin(store, Held::Band { field, start }, Edit::Spectrogram(s));
        (false, Then::Draw)
    }

    pub fn band_drag(&mut self, store: &mut EditorStore, field: usize, translation: f32, span: f32, fine: bool, now: Instant) -> Then {
        let (Some(Held::Band { field: held, start }), Some(Edit::Spectrogram(s))) = (self.held, self.live.clone()) else { return Then::Nothing };
        if held != field {
            return Then::Nothing;
        }
        let band = self.selected.band;
        let range = band_range(field, band);
        let asked = track::clamped(track::asked(start, translation, span, fine));
        self.band_shown = Some((field, asked));
        let next = with_band_value(&s, band, field, track::value(asked, &(range.0..=range.1), track::Bend::Even));
        if next == s {
            return Then::Draw;
        }
        self.live = Some(Edit::Spectrogram(next));
        self.offer(store, now)
    }

    pub fn band_release(&mut self, store: &mut EditorStore) -> Then {
        if !matches!(self.held, Some(Held::Band { .. })) {
            return Then::Nothing;
        }
        self.release(store)
    }

    pub fn band_scroll(&mut self, store: &mut EditorStore, field: usize, dx: f32, dy: f32, span: f32, fine: bool) -> bool {
        if dx == 0.0 || dx.abs() <= dy.abs() || self.held.is_some() {
            return false;
        }
        let s = self.view(store).spectrogram;
        let band = self.selected.band;
        let range = band_range(field, band);
        let from = track::position(band_value(&s, band, field), &(range.0..=range.1), track::Bend::Even);
        let asked = track::clamped(track::asked(from, dx * TRACKPAD_TRAVEL, span, fine));
        let v = track::value(asked, &(range.0..=range.1), track::Bend::Even);
        self.scrolled = true;
        self.once(store, Edit::Spectrogram(with_band_value(&s, band, field, v)));
        true
    }

    /// An arrow's or accessibility's step: a unit of the readout, a degree or a percent, on its grid.
    pub fn band_step(&mut self, store: &mut EditorStore, field: usize, direction: i32) -> Then {
        let s = self.view(store).spectrogram;
        let (band, unit) = (self.selected.band, if field == 0 { 1.0 } else { 0.01 });
        let (low, high) = band_range(field, band);
        let v = (((band_value(&s, band, field) / unit).round() + direction as f32) * unit).clamp(low, high);
        self.once(store, Edit::Spectrogram(with_band_value(&s, band, field, v)))
    }
}

