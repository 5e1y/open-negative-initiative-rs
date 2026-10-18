//! `SpectrogramPanel`: one point per band on a wheel, its angle the hue and its radius the
//! saturation, a second point on the spoke its luminance; grabbed where it is, never jumped to.

use super::geometry::{Dot, Drawing, Ink, Line, circle_points, dashed, distance, ds, polyline};
use oni_settings::spectrogram::{Band, Spectrogram};

/// Where the luminance point rests along its spoke, and how far a full offset moves it.
pub const LUMA_REST: f64 = 0.5;
pub const LUMA_REACH: f64 = 0.25;
/// The radius share under which saturation never sinks, so eight greyed bands never pile up.
pub const FLOOR: f64 = 0.12;
/// A spoke's opacity when its band is neither selected nor hovered.
pub const SPOKE_REST: f64 = 0.65;
/// A mark's size off the hand, as a share of the handle's.
pub const MARK_REST: f64 = 0.7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Tip,
    Luma,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grip {
    pub band: Band,
    pub part: Part,
}

impl Default for Grip {
    fn default() -> Self {
        Grip { band: Band::Red, part: Part::Tip }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Nudge {
    Left,
    Right,
    Up,
    Down,
}

/// The wheel's square side; every reading follows from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wheel {
    pub side: f64,
}

impl Wheel {
    pub fn centre(self) -> (f64, f64) {
        (self.side / 2.0, self.side / 2.0)
    }

    pub fn outer(self) -> f64 {
        self.side / 2.0 - ds::SPACE_MD
    }

    pub fn fraction_of_saturation(s: f32) -> f64 {
        let (low, high) = Spectrogram::SATURATION_RANGE;
        FLOOR + (1.0 - FLOOR) * f64::from((s - low) / (high - low))
    }

    pub fn saturation_at(f: f64) -> f32 {
        let (low, high) = Spectrogram::SATURATION_RANGE;
        low + ((f - FLOOR) / (1.0 - FLOOR)) as f32 * (high - low)
    }

    fn luma_fraction(l: f32) -> f64 {
        LUMA_REST + LUMA_REACH * f64::from(l) / f64::from(Spectrogram::LUMA_RANGE.1.max(1e-6))
    }

    fn luma_at(f: f64) -> f32 {
        ((f - LUMA_REST) / LUMA_REACH) as f32 * Spectrogram::LUMA_RANGE.1
    }

    /// Hue runs anticlockwise from red at the right.
    pub fn tip(self, band: Band, s: &Spectrogram) -> (f64, f64) {
        let (hue, saturation, _) = s.band(band);
        let r = self.outer() * Self::fraction_of_saturation(saturation);
        let angle = f64::from(band.centre() + hue).to_radians();
        let c = self.centre();
        (c.0 + r * angle.cos(), c.1 - r * angle.sin())
    }

    pub fn position(self, grip: Grip, s: &Spectrogram) -> (f64, f64) {
        let tip = self.tip(grip.band, s);
        if grip.part == Part::Tip {
            return tip;
        }
        let f = Self::luma_fraction(s.band(grip.band).2);
        let c = self.centre();
        (c.0 + (tip.0 - c.0) * f, c.1 + (tip.1 - c.1) * f)
    }

    pub fn grips() -> Vec<Grip> {
        Band::ALL.iter().flat_map(|b| [Grip { band: *b, part: Part::Tip }, Grip { band: *b, part: Part::Luma }]).collect()
    }

    /// The closest of the sixteen points, at any distance: a press always takes one.
    pub fn nearest(self, at: (f64, f64), s: &Spectrogram) -> Grip {
        let mut best: Option<(Grip, f64)> = None;
        for g in Self::grips() {
            let d = distance(at, self.position(g, s));
            if best.is_none_or(|(_, b)| d < b) {
                best = Some((g, d));
            }
        }
        best.map(|(g, _)| g).unwrap_or_default()
    }

    /// The hue and saturation a point at `at` stands for, held to the band's room.
    pub fn reading(self, at: (f64, f64), band: Band) -> (f32, f32) {
        let c = self.centre();
        let degrees = (c.1 - at.1).atan2(at.0 - c.0).to_degrees() as f32;
        let mut offset = degrees - band.centre();
        while offset > 180.0 {
            offset -= 360.0;
        }
        while offset < -180.0 {
            offset += 360.0;
        }
        let (low, high) = band.hue_range();
        let hue = offset.max(low).min(high);
        let reach = (distance(at, c) / self.outer()).clamp(0.0, 1.0);
        (hue, Self::saturation_at(reach))
    }

    /// The luminance a point at `at` stands for, projected on its spoke.
    pub fn luma_reading(self, at: (f64, f64), band: Band, s: &Spectrogram) -> f32 {
        let tip = self.tip(band, s);
        let c = self.centre();
        let axis = (tip.0 - c.0, tip.1 - c.1);
        let reach = axis.0 * axis.0 + axis.1 * axis.1;
        if reach <= 0.0 {
            return s.band(band).2;
        }
        let along = ((at.0 - c.0) * axis.0 + (at.1 - c.1) * axis.1) / reach;
        Self::luma_at(along)
    }
}

/// A grabbed point and where it was: the drag moves it by the pointer's travel from there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grab {
    pub grip: Grip,
    pub origin: (f64, f64),
}

pub fn grab(wheel: Wheel, at: (f64, f64), s: &Spectrogram) -> Grab {
    let grip = wheel.nearest(at, s);
    Grab { grip, origin: wheel.position(grip, s) }
}

/// The spectrogram with the grabbed point moved by `travel`; a press with no travel moves nothing.
pub fn dragged(wheel: Wheel, grab: Grab, travel: (f64, f64), s: &Spectrogram) -> Spectrogram {
    if travel == (0.0, 0.0) {
        return *s;
    }
    let wanted = (grab.origin.0 + travel.0, grab.origin.1 + travel.1);
    let (mut hue, mut saturation, mut luma) = s.band(grab.grip.band);
    match grab.grip.part {
        Part::Tip => (hue, saturation) = wheel.reading(wanted, grab.grip.band),
        Part::Luma => luma = wheel.luma_reading(wanted, grab.grip.band, s),
    }
    let mut out = *s;
    out.set(grab.grip.band, hue, saturation, luma);
    out
}

/// A double click puts back the point's own axes and no other.
pub fn reset(grip: Grip, s: &Spectrogram) -> Spectrogram {
    let (mut hue, mut saturation, mut luma) = s.band(grip.band);
    match grip.part {
        Part::Tip => (hue, saturation) = (0.0, 0.0),
        Part::Luma => luma = 0.0,
    }
    let mut out = *s;
    out.set(grip.band, hue, saturation, luma);
    out
}

/// An arrow: a step of the axis's travel, hue left and right, the point's own axis up and down.
pub fn nudge(grip: Grip, by: Nudge, s: &Spectrogram) -> Spectrogram {
    let step = Spectrogram::DEFAULT_STEP;
    let span = |r: (f32, f32)| r.1 - r.0;
    let (mut hue, mut saturation, mut luma) = s.band(grip.band);
    let tip = grip.part == Part::Tip;
    match by {
        Nudge::Left => hue += step * span(grip.band.hue_range()),
        Nudge::Right => hue -= step * span(grip.band.hue_range()),
        Nudge::Up if tip => saturation += step * span(Spectrogram::SATURATION_RANGE),
        Nudge::Up => luma += step * span(Spectrogram::LUMA_RANGE),
        Nudge::Down if tip => saturation -= step * span(Spectrogram::SATURATION_RANGE),
        Nudge::Down => luma -= step * span(Spectrogram::LUMA_RANGE),
    }
    let mut out = *s;
    out.set(grip.band, hue, saturation, luma);
    out
}

/// Tab walks the wheel clockwise, Option-Tab the other way; the part stays.
pub fn walked(grip: Grip, option: bool) -> Grip {
    Grip { band: if option { grip.band.anticlockwise() } else { grip.band.clockwise() }, part: grip.part }
}

/// The ring, the dashed rest ring, a spoke and two marks per band; `active` is the selected point
/// while the wheel holds the focus.
pub fn drawing(wheel: Wheel, s: &Spectrogram, active: Option<Grip>, hovered: Option<Grip>) -> Drawing {
    let c = wheel.centre();
    let r = wheel.outer();
    let mut lines = vec![Line::new(polyline(&circle_points(c, r)), Ink::Track, ds::STROKE_LINE)];
    let rest = r * Wheel::fraction_of_saturation(0.0);
    lines.push(Line { opacity: 0.5, ..Line::new(dashed(&circle_points(c, rest), ds::SPACE_XS), Ink::Track, ds::STROKE_LINE) });
    let mut dots = Vec::new();
    for band in Band::ALL {
        let held = active.is_some_and(|g| g.band == band) || hovered.is_some_and(|g| g.band == band);
        let p = wheel.tip(band, s);
        let ink = Ink::Band(band.index());
        lines.push(Line { opacity: if held { 1.0 } else { SPOKE_REST }, ..Line::new(polyline(&[c, p]), ink, ds::STROKE_LINE) });
        for part in [Part::Tip, Part::Luma] {
            let grip = Grip { band, part };
            let at = wheel.position(grip, s);
            let lit = active == Some(grip);
            let size = if lit || hovered == Some(grip) { ds::HANDLE_SIZE } else { ds::HANDLE_SIZE * MARK_REST };
            let halo = if lit { size + 2.0 * ds::SPACE_XS } else { 0.0 };
            dots.push(match part {
                Part::Tip => Dot { halo, ..Dot::filled(at, size, ink) },
                Part::Luma => Dot { x: at.0, y: at.1, size, fill: Ink::Canvas, edge: ink, edge_width: ds::STROKE_LINE, halo },
            });
        }
    }
    Drawing { lines, dots }
}

/// The bands a hand moved, lit in the tab row.
pub fn marked(s: &Spectrogram) -> Vec<bool> {
    Band::ALL.iter().map(|b| s.band(*b) != (0.0, 0.0, 0.0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHEEL: Wheel = Wheel { side: 308.0 };

    /// `SpectrogramPanel.selfCheck`: the wheel is grabbed, never jumped to.
    #[test]
    fn the_colour_wheel_is_grabbed_never_jumped_to() {
        let wheel = Spectrogram::default();
        let band = Band::Red;
        let start = WHEEL.tip(band, &wheel);
        let (h, s) = WHEEL.reading(start, band);
        let mut back = wheel;
        back.set(band, h, s, 0.0);
        assert!(distance(WHEEL.tip(band, &back), start) < 0.001, "a point read back where it is drawn stays");

        let press = (start.0 + 15.0, start.1);
        let g = grab(WHEEL, press, &wheel);
        assert_eq!(g.grip, Grip { band, part: Part::Tip }, "a press beside a point takes that point");
        assert_eq!(dragged(WHEEL, g, (0.0, 0.0), &wheel), wheel, "a press with no travel moves nothing");

        let travel = (0.0, -12.0);
        let moved = dragged(WHEEL, g, travel, &wheel);
        let wanted = (start.0 + travel.0, start.1 + travel.1);
        assert!(distance(WHEEL.tip(band, &moved), wanted) < 0.001, "the drag moves the point by the travel");

        // Twin: reading the pointer's own spot jumps by the grab's offset.
        let (h, s) = WHEEL.reading((press.0 + travel.0, press.1 + travel.1), band);
        let mut absolute = Spectrogram::default();
        absolute.set(band, h, s, 0.0);
        assert!(distance(WHEEL.tip(band, &absolute), wanted) > 10.0);
    }

    #[test]
    fn a_double_click_resets_the_point_s_own_axes() {
        let mut loaded = Spectrogram::default();
        loaded.set(Band::Green, 10.0, 0.4, -0.6);
        loaded.set(Band::Blue, -5.0, 0.2, 0.3);
        let tip = reset(Grip { band: Band::Green, part: Part::Tip }, &loaded);
        assert_eq!(tip.band(Band::Green), (0.0, 0.0, -0.6));
        let luma = reset(Grip { band: Band::Green, part: Part::Luma }, &loaded);
        assert_eq!(luma.band(Band::Green), (10.0, 0.4, 0.0));
        assert_eq!((tip.band(Band::Blue), luma.band(Band::Blue)), (loaded.band(Band::Blue), loaded.band(Band::Blue)));
    }

    #[test]
    fn an_arrow_steps_two_percent_of_the_axis_and_tab_walks_the_wheel() {
        let s = Spectrogram::default();
        let tip = Grip { band: Band::Orange, part: Part::Tip };
        let span = Band::Orange.hue_range().1 - Band::Orange.hue_range().0;
        assert_eq!(nudge(tip, Nudge::Left, &s).band(Band::Orange).0, 0.02 * span);
        assert_eq!(nudge(tip, Nudge::Up, &s).band(Band::Orange), (0.0, 0.04, 0.0));
        let luma = Grip { part: Part::Luma, ..tip };
        assert_eq!(nudge(luma, Nudge::Down, &s).band(Band::Orange), (0.0, 0.0, -0.04));
        // Twin: right turns the other way, and the clamp holds a run of steps on the band's room.
        assert!(nudge(tip, Nudge::Right, &s).band(Band::Orange).0 < 0.0);
        let mut far = s;
        for _ in 0..200 {
            far = nudge(tip, Nudge::Left, &far);
        }
        assert_eq!(far.band(Band::Orange).0, Band::Orange.hue_range().1);
        assert_eq!(walked(tip, false).band, Band::Red, "Tab goes clockwise, to the previous centre");
        assert_eq!(walked(tip, true).band, Band::Yellow);
        assert_eq!(walked(Grip { band: Band::Red, part: Part::Luma }, false), Grip { band: Band::Magenta, part: Part::Luma });
    }

    #[test]
    fn the_luma_point_reads_along_its_spoke() {
        let mut s = Spectrogram::default();
        s.set(Band::Blue, 0.0, 0.5, 0.0);
        let g = Grip { band: Band::Blue, part: Part::Luma };
        let at = WHEEL.position(g, &s);
        let c = WHEEL.centre();
        let tip = WHEEL.tip(Band::Blue, &s);
        assert!((distance(at, c) / distance(tip, c) - LUMA_REST).abs() < 1e-9);
        let moved = dragged(WHEEL, Grab { grip: g, origin: at }, ((tip.0 - c.0) * 0.25, (tip.1 - c.1) * 0.25), &s);
        assert!((moved.band(Band::Blue).2 - 1.0).abs() < 1e-5, "a quarter spoke out is a full offset");
        assert_eq!(moved.band(Band::Blue).1, 0.5, "the tip's axes stay");
    }
}
