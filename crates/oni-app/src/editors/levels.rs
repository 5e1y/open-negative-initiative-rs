//! `LevelsView`: five handles on one track under the histogram, the two sharing `dsHandleInset`;
//! one write path for drag and scroll, a double click resetting the handle alone. A film shows its
//! three windows on the density axis, a digital photo its five on the picture.

use super::geometry::{Drawing, Ink, Line, Size, dashed, ds};
use crate::track;
use oni_i18n::tr;
use oni_core::{Channel, Command, Handle, Knob};
use oni_settings::levels::{Levels, LevelsChannel, LevelsSet, PictureLevels};
use oni_settings::{ConversionMode, PipelineSettings};

/// The handles in drawing order: the ends are drawn last, so they sit in front.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grip {
    Shadows,
    Mid,
    Highlights,
    Black,
    White,
}

impl Grip {
    pub const ALL: [Grip; 5] = [Grip::Shadows, Grip::Mid, Grip::Highlights, Grip::Black, Grip::White];
    /// Left to right on the track, as the readouts read.
    pub const READING: [Grip; 5] = [Grip::Black, Grip::Shadows, Grip::Mid, Grip::Highlights, Grip::White];

    pub fn label(self) -> &'static str {
        match self {
            Grip::Shadows => tr("Shadows"),
            Grip::Mid => tr("Midpoint"),
            Grip::Highlights => tr("Highlights"),
            Grip::Black => tr("Black"),
            Grip::White => tr("White"),
        }
    }

    pub fn readout_name(self) -> &'static str {
        match self {
            Grip::Mid => tr("Mid"),
            Grip::Highlights => tr("Highs"),
            other => other.label(),
        }
    }

    pub fn tint(self) -> Ink {
        match self {
            Grip::Shadows => Ink::GreyQuarter,
            Grip::Mid => Ink::GreyHalf,
            Grip::Highlights => Ink::GreyThreeQuarters,
            Grip::Black => Ink::Black,
            Grip::White => Ink::White,
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|g| *g == self).unwrap_or(0)
    }
}

/// The per-channel block's tabs.
pub const CHANNELS: [LevelsChannel; 3] = LevelsChannel::PER_CHANNEL;

/// The channel of tab `tab` among those a frame of `mode` shows.
pub fn channel(mode: ConversionMode, tab: usize) -> LevelsChannel {
    let shown: &[LevelsChannel] = if mode.is_film() { &CHANNELS } else { &LevelsChannel::PICTURE };
    shown[tab.min(shown.len() - 1)]
}

/// The windows the editor moves: a film's on the density axis, a digital photo's on the picture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Windows {
    Film(LevelsSet),
    Picture(PictureLevels),
}

impl Windows {
    /// The frame's own windows, as its mode renders them.
    pub fn of(s: &PipelineSettings) -> Windows {
        if s.mode.is_film() { Windows::Film(s.levels) } else { Windows::Picture(s.picture_levels) }
    }

    /// The tabs, in the order the editor shows them.
    pub fn channels(&self) -> &'static [LevelsChannel] {
        match self {
            Windows::Film(_) => &CHANNELS,
            Windows::Picture(_) => &LevelsChannel::PICTURE,
        }
    }

    /// The channel of tab `tab`, held to the tabs shown.
    pub fn channel(&self, tab: usize) -> LevelsChannel {
        let shown = self.channels();
        shown[tab.min(shown.len() - 1)]
    }

    pub fn get(&self, channel: LevelsChannel) -> Levels {
        match self {
            Windows::Film(set) => set.get(channel),
            Windows::Picture(p) => p.get(channel),
        }
    }

    pub fn set(&mut self, channel: LevelsChannel, levels: Levels) {
        match self {
            Windows::Film(set) => set.set(channel, levels),
            Windows::Picture(p) => p.set(channel, levels),
        }
    }

    /// `s` with these windows in their own field.
    pub fn lay(&self, s: &mut PipelineSettings) {
        match self {
            Windows::Film(set) => s.levels = *set,
            Windows::Picture(p) => s.picture_levels = *p,
        }
    }
}

fn on_track(mid: f32) -> f32 {
    track::position(mid, &(Levels::MID_RANGE.0..=Levels::MID_RANGE.1), track::Bend::Even)
}

fn from_track(position: f32) -> f32 {
    track::value(position, &(Levels::MID_RANGE.0..=Levels::MID_RANGE.1), track::Bend::Even)
}

/// Where a grip is drawn, 0 to 1 of the track: the median travels its own range across the window,
/// the quarter points read straight.
pub fn place(grip: Grip, levels: &Levels, channel: LevelsChannel, mode: ConversionMode) -> f32 {
    let value = match grip {
        Grip::Mid => levels.black + on_track(levels.mid) * (levels.white - levels.black),
        Grip::Shadows => levels.display_shadows(),
        Grip::Highlights => levels.display_highlights(),
        Grip::Black => levels.black,
        Grip::White => levels.white,
    };
    channel.position_of_value(value, mode)
}

pub fn resting(channel: LevelsChannel, mode: ConversionMode, ceiling: f32) -> Levels {
    if mode.is_film() { LevelsSet::resting(channel, mode, ceiling) } else { Levels::default() }
}

/// The single write path: a drag and a scroll both land here, on the channel's own domain.
pub fn moved(grip: Grip, levels: &Levels, position: f32, channel: LevelsChannel, mode: ConversionMode) -> Levels {
    let asked = channel.value_at_position(position, mode);
    let mut out = *levels;
    match grip {
        Grip::Black => out.set_black(asked),
        Grip::White => out.set_white(asked),
        Grip::Shadows => out.set_display_shadows(asked),
        Grip::Highlights => out.set_display_highlights(asked),
        Grip::Mid => {
            let span = out.white - out.black;
            if span > 0.0 {
                out.mid = from_track(((asked - out.black) / span).clamp(0.0, 1.0));
            }
        }
    }
    out
}

/// Each handle resets itself alone; inner points return to their quarter, wherever the ends are.
pub fn reset(grip: Grip, levels: &Levels, channel: LevelsChannel, mode: ConversionMode, ceiling: f32) -> Levels {
    let rest = resting(channel, mode, ceiling);
    let mut out = *levels;
    match grip {
        Grip::Black => out.black = rest.black,
        Grip::White => out.white = rest.white,
        Grip::Mid => out.mid = rest.mid,
        Grip::Shadows => out.shadows = rest.shadows,
        Grip::Highlights => out.highlights = rest.highlights,
    }
    out
}

/// The store's knob for `grip` on `windows`' `channel`.
fn knob(grip: Grip, channel: LevelsChannel, windows: &Windows) -> Knob {
    let h = match grip {
        Grip::Black => Handle::Black,
        Grip::Shadows => Handle::Shadows,
        Grip::Mid => Handle::Mid,
        Grip::Highlights => Handle::Highlights,
        Grip::White => Handle::White,
    };
    let c = match channel {
        LevelsChannel::Green => Channel::Green,
        LevelsChannel::Blue => Channel::Blue,
        _ => Channel::Red,
    };
    match windows {
        Windows::Film(_) => Knob::Level(c, h),
        Windows::Picture(_) => Knob::Picture(channel, h),
    }
}

/// Each handle `windows` moved off `settings`' own as one step of its knob.
pub fn steps(settings: &PipelineSettings, windows: &Windows) -> Vec<Command> {
    let mut next = settings.clone();
    windows.lay(&mut next);
    let knobs = windows.channels().iter().flat_map(|c| Grip::ALL.map(|g| knob(g, *c, windows)));
    knobs.filter_map(|knob| Some(knob.get(&next) - knob.get(settings)).filter(|by| *by != 0.0).map(|by| Command::Step { knob, by })).collect()
}

/// A double click's rest of one handle, the value set on every frame shown.
pub fn rested(grip: Grip, channel: LevelsChannel, view: &PipelineSettings) -> Command {
    let mut windows = Windows::of(view);
    windows.set(channel, reset(grip, &windows.get(channel), channel, view.mode, view.density_ceiling));
    let mut at = view.clone();
    windows.lay(&mut at);
    let knob = knob(grip, channel, &windows);
    Command::Set { knob, value: knob.get(&at) }
}

/// The handle a press catches: its knob's grab area, the front one first.
pub fn caught(x: f64, y: f64, width: f64, levels: &Levels, channel: LevelsChannel, mode: ConversionMode) -> Option<Grip> {
    if !(0.0..=ds::HANDLE_LENGTH).contains(&y) {
        return None;
    }
    Grip::ALL.iter().rev().copied().find(|g| (knob_x(*g, levels, channel, mode, width) - x).abs() <= ds::GRAB_SIZE / 2.0)
}

/// The handle a scroll moves: the one under the pointer, else the nearest, front to back.
pub fn scrolled(x: f64, width: f64, levels: &Levels, channel: LevelsChannel, mode: ConversionMode) -> Grip {
    let distance = |g: Grip| (knob_x(g, levels, channel, mode, width) - x).abs();
    let front_to_back: Vec<Grip> = Grip::ALL.iter().rev().copied().collect();
    if let Some(g) = front_to_back.iter().copied().find(|g| distance(*g) <= ds::GRAB_SIZE / 2.0) {
        return g;
    }
    let mut best = Grip::White;
    let mut least = f64::INFINITY;
    for g in front_to_back {
        if distance(g) < least {
            least = distance(g);
            best = g;
        }
    }
    best
}

/// `DSTrack.x`: the knob's centre at its share of the inset width.
pub fn knob_x(grip: Grip, levels: &Levels, channel: LevelsChannel, mode: ConversionMode, width: f64) -> f64 {
    f64::from(place(grip, levels, channel, mode).clamp(0.0, 1.0)) * width
}

/// The value each handle sets, never where it is drawn, in reading order.
pub fn readouts(levels: &Levels, channel: LevelsChannel, mode: ConversionMode) -> Vec<(&'static str, String)> {
    Grip::READING
        .iter()
        .map(|g| {
            let v = match g {
                Grip::Black => levels.black,
                Grip::Shadows => levels.display_shadows(),
                Grip::Mid => levels.display_mid(),
                Grip::Highlights => levels.display_highlights(),
                Grip::White => levels.white,
            };
            (g.readout_name(), readout(channel, mode, v))
        })
        .collect()
}

/// Three decimals of density where values are densities, a share of the output otherwise.
pub fn readout(channel: LevelsChannel, mode: ConversionMode, value: f32) -> String {
    use crate::fields::{Unit, number};
    if channel.is_density(mode) { number(f64::from(value), 3, false, Unit::Bare) } else { number(f64::from(value) * 100.0, 1, false, Unit::Percent) }
}

/// The handles' own places marked on the histogram, dashed.
pub fn markers(levels: &Levels, channel: LevelsChannel, mode: ConversionMode, size: Size) -> Drawing {
    let lines = Grip::ALL
        .iter()
        .map(|g| {
            let x = knob_x(*g, levels, channel, mode, size.w);
            Line::new(dashed(&[(x, 0.0), (x, size.h)], ds::MARKER_DASH), Ink::Marker, ds::STROKE_LINE)
        })
        .collect();
    Drawing { lines, dots: Vec::new() }
}

/// Tabs lit for a window off its rest.
pub fn marked(view: &PipelineSettings) -> Vec<bool> {
    let windows = Windows::of(view);
    let touched = match windows {
        Windows::Film(set) => set.touched(view.mode, view.density_ceiling),
        Windows::Picture(p) => p.touched(),
    };
    windows.channels().iter().map(|c| touched.contains(c)).collect()
}

#[cfg(test)]
mod tests;
