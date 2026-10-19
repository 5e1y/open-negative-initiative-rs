//! `LevelsView`: five handles on one track under the histogram, the two sharing `dsHandleInset`;
//! one write path for drag and scroll, a double click resetting the handle alone.

use super::geometry::{Drawing, Ink, Line, Size, dashed, ds};
use crate::track;
use oni_settings::ConversionMode;
use oni_settings::levels::{Levels, LevelsChannel, LevelsSet};

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
            Grip::Shadows => "Shadows",
            Grip::Mid => "Midpoint",
            Grip::Highlights => "Highlights",
            Grip::Black => "Black",
            Grip::White => "White",
        }
    }

    pub fn readout_name(self) -> &'static str {
        match self {
            Grip::Mid => "Mid",
            Grip::Highlights => "Highs",
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

fn on_track(mid: f32) -> f32 {
    track::position(mid, &(Levels::MID_RANGE.0..=Levels::MID_RANGE.1), None)
}

fn from_track(position: f32) -> f32 {
    track::value(position, &(Levels::MID_RANGE.0..=Levels::MID_RANGE.1), None)
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
    LevelsSet::resting(channel, mode, ceiling)
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
pub fn readouts(levels: &Levels, channel: LevelsChannel) -> Vec<(&'static str, String)> {
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
            (g.readout_name(), channel.readout(v))
        })
        .collect()
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
pub fn marked(set: &LevelsSet, mode: ConversionMode, ceiling: f32) -> Vec<bool> {
    let touched = set.touched(mode, ceiling);
    CHANNELS.iter().map(|c| touched.contains(c)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::axis::{graduation, pipeline};
    use oni_settings::density_migration::apply_levels;

    const MODES: [ConversionMode; 3] = [ConversionMode::Negative, ConversionMode::Positive, ConversionMode::Monochrome];

    /// `LevelsView.selfCheck`, every line with its twin.
    #[test]
    fn the_levels_track_holds_and_its_black_reaches_under_zero() {
        let mut strayed: f32 = 0.0;
        let mut round_trip: f32 = 0.0;
        for mode in MODES {
            let (low, _) = LevelsChannel::Red.domain(mode);
            for step in 0..=400 {
                let d = low - 0.2 + step as f32 / 400.0 * (graduation::span() + 0.4);
                let placed = LevelsChannel::Red.position_of_value(d, mode);
                strayed = strayed.max((placed - graduation::fraction(d, mode)).abs());
                round_trip = round_trip.max((LevelsChannel::Red.value_at_position(placed, mode) - d).abs());
            }
        }
        assert!(strayed < 1e-6 && round_trip / graduation::span() < 1e-6, "{strayed} {round_trip}");
        assert!((0.5 - graduation::fraction(0.5, ConversionMode::Negative)).abs() > 0.1, "twin: a raw 0.5 is not its place");

        let transmitting = -0.243;
        let bottom = LevelsChannel::Red.value_at_position(0.0, ConversionMode::Negative);
        let rescued = apply_levels(transmitting, &Levels::with_ends(bottom, pipeline::DMAX));
        assert!(bottom < transmitting && rescued > 0.0, "{bottom} {rescued}");
        let sunk = apply_levels(transmitting, &Levels::with_ends(0.0, pipeline::DMAX));
        assert_eq!(sunk, 0.0, "twin: a track floored at zero renders that channel black");

        let hand = Levels { black: 0.4137, white: 2.7311, mid: 0.3117, ..Levels::default() };
        let mut inside = true;
        for mode in MODES {
            for channel in CHANNELS {
                let rest = LevelsSet::resting(channel, mode, oni_settings::axis::density_axis::base());
                inside &= (0.0..=1.0).contains(&channel.position_of_value(rest.white, mode));
                inside &= (0.0..=1.0).contains(&channel.position_of_value(rest.black, mode));
                for g in Grip::ALL {
                    inside &= (0.0..=1.0).contains(&place(g, &hand, channel, mode));
                }
            }
        }
        assert!(inside, "every handle and every rest falls on the track");
        let rest = LevelsSet::resting(LevelsChannel::Red, ConversionMode::Negative, oni_settings::axis::density_axis::base());
        assert!(!(0.0..=1.0).contains(&rest.white), "twin: the raw resting white sits past any track");
    }

    #[test]
    fn the_five_handles_rest_a_quarter_apart_and_stay_reachable() {
        let base = oni_settings::axis::density_axis::base();
        let at_rest = LevelsSet::resting(LevelsChannel::Red, ConversionMode::Negative, base);
        let mut laid: Vec<f32> = Grip::ALL.iter().map(|g| place(*g, &at_rest, LevelsChannel::Red, ConversionMode::Negative)).collect();
        laid.sort_by(f32::total_cmp);
        let span = LevelsChannel::Red.position_of_value(at_rest.white, ConversionMode::Negative)
            - LevelsChannel::Red.position_of_value(at_rest.black, ConversionMode::Negative);
        assert!(laid.windows(2).all(|w| (w[1] - w[0] - span / 4.0).abs() < 1e-5), "{laid:?}");
        // On a panel-wide track the five stand further apart than a grab area.
        let width = 340.0 - 2.0 * ds::SPACE_MD;
        let xs: Vec<f64> = laid.iter().map(|p| f64::from(*p) * width).collect();
        assert!(xs.windows(2).all(|w| w[1] - w[0] > ds::GRAB_SIZE), "{xs:?}");
        // Twin: pinched to the narrowest window a hand can set, they crowd under one grab area.
        let mut pinched = at_rest;
        pinched.set_white(pinched.black + Levels::EPSILON);
        let mut crowded: Vec<f64> =
            Grip::ALL.iter().map(|g| knob_x(*g, &pinched, LevelsChannel::Red, ConversionMode::Negative, width)).collect();
        crowded.sort_by(f64::total_cmp);
        assert!(crowded.windows(2).map(|w| w[1] - w[0]).fold(f64::INFINITY, f64::min) < ds::GRAB_SIZE);
    }

    #[test]
    fn a_handle_moves_through_one_path_and_resets_alone() {
        let mode = ConversionMode::Negative;
        let red = LevelsChannel::Red;
        let base = oni_settings::axis::density_axis::base();
        let mut l = Levels { black: 0.4137, white: 2.7311, ..Levels::default() };
        // Dragged off both ends, the quarter points stop at their travels.
        l = moved(Grip::Shadows, &l, -9.0, red, mode);
        l = moved(Grip::Highlights, &l, 9.0, red, mode);
        assert_eq!((l.shadows, l.highlights), (Levels::SHADOW_RANGE.0, Levels::HIGHLIGHT_RANGE.1));
        // The black never crosses the white's epsilon.
        let crossed = moved(Grip::Black, &l, 1.0, red, mode);
        assert_eq!(crossed.black, l.white - Levels::EPSILON);
        // The median lands on its own travel across the window, off its fixed point at 0.5.
        l.mid = 0.3;
        let p = place(Grip::Mid, &l, red, mode);
        let back = moved(Grip::Mid, &l, p, red, mode);
        assert!((back.mid - l.mid).abs() < 1e-5, "{} {}", back.mid, l.mid);
        // A reset moves its own handle alone.
        let r = reset(Grip::White, &l, red, mode, base);
        assert_eq!((r.white, r.black, r.shadows), (pipeline::DMAX, l.black, l.shadows));
        let r = reset(Grip::Shadows, &l, red, mode, base);
        assert_eq!((r.shadows, r.highlights, r.black), (0.25, l.highlights, l.black));
        // Twin: the median's handle is not its stored value, so a write of the raw place drifts.
        let mut naive = l;
        naive.mid = (red.value_at_position(p, mode) - l.black) / (l.white - l.black);
        assert!((naive.mid - l.mid).abs() > 0.01);
    }

    #[test]
    fn a_press_catches_the_front_handle_and_a_scroll_the_nearest() {
        let mode = ConversionMode::Negative;
        let red = LevelsChannel::Red;
        let width = 284.0;
        // The ends together: the white is drawn last, so it is caught first.
        let mut l = Levels::with_ends(1.0, 1.0 + Levels::EPSILON);
        l.mid = 0.5;
        let x = knob_x(Grip::White, &l, red, mode, width);
        assert_eq!(caught(x, 17.0, width, &l, red, mode), Some(Grip::White));
        assert_eq!(scrolled(x, width, &l, red, mode), Grip::White);
        // Off the handle's height nothing is caught.
        assert_eq!(caught(x, 40.0, width, &l, red, mode), None);
        // Twin: far from every handle a press catches none, a scroll still the nearest.
        let far = width - 1.0;
        assert_eq!(caught(far, 17.0, width, &l, red, mode), None);
        assert_eq!(scrolled(far, width, &l, red, mode), Grip::White);
        assert_eq!(scrolled(0.0, width, &l, red, mode), Grip::Black);
    }

    #[test]
    fn the_readouts_are_densities_to_three_decimals() {
        let l = Levels::with_ends(-0.243, 2.5);
        let r = readouts(&l, LevelsChannel::Red);
        assert_eq!(r[0], ("Black", "-0.243".to_string()));
        assert_eq!(r.iter().map(|(n, _)| *n).collect::<Vec<_>>(), vec!["Black", "Shadows", "Mid", "Highs", "White"]);
        // Twin: a global set would read a share of its output.
        assert_eq!(LevelsChannel::Linked.readout(0.5), "50.0%");
    }
}
