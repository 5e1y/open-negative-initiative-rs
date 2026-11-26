//! How each gift comes undone: its bow drawn through its knot, its bands fallen open, light leaking under its lid,
//! then the lid's own way off: a case's for the silver one, a toss for the copper, a flower's for the mystery.

use super::direction::{LID, STAND};
use super::ease::{give, glide, land, mix, span};
use std::f32::consts::TAU;

/// A gift's moving parts as the shader reads them, and what its body does meanwhile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parts {
    pub lid: [f32; 4],
    pub tie: [f32; 4],
    /// How far its walls have fallen open, 0 a box to 1 flat.
    pub open: f32,
    /// The light leaking under its lid, and how high its light has climbed out of it, each 0 to 1.
    pub seam: f32,
    pub rise: f32,
    /// A turn about itself and a lift off the floor, added to its rest.
    pub spin: f32,
    pub lift: f32,
}

impl Parts {
    pub fn tied() -> Parts {
        Parts { lid: [0.0; 4], tie: [1.0, 1.0, 1.0, 0.0], open: 0.0, seam: 0.0, rise: 0.0, spin: 0.0, lift: 0.0 }
    }
}

/// How long each gift's untying lasts, and the moment its lid goes.
const LENGTH: [f32; 4] = [3.6, 4.2, 6.2, 7.0];
const POP: [f32; 4] = [1.6, 1.9, 2.6, 4.0];
/// How high each opened gift's light stands, a share of the tallest: the silver one's is a breath over its mouth.
const COLUMN: [f32; 4] = [0.45, 0.8, 1.0, 0.9];

pub fn length(gift: usize) -> f32 {
    LENGTH[gift]
}

pub fn pop(gift: usize) -> f32 {
    POP[gift]
}

/// The second a gift's title arrives: once its lid is down, or its walls open.
pub fn title(gift: usize) -> f32 {
    POP[gift] + [0.9, 0.9, 1.3, 1.3][gift]
}

/// Where the camera starts round each gift and how far it turns while the gift comes undone, in radians.
pub fn orbit(gift: usize) -> (f32, f32) {
    [(-0.5, 0.42), (0.5, -0.5), (-0.85, 1.4), (0.35, -0.7)][gift]
}

/// Where a tossed lid rests: leaning against the back of its box.
fn rest(gift: usize) -> [f32; 4] {
    let seat = 2.0 * STAND[gift].tall + LID - 0.08;
    [0.0, 0.42 - seat, -2.4, -0.5]
}

/// A lid `t` seconds after it jumps: up to `peak` half-widths, `turns` tumbles, down behind its box by `landing`
/// seconds, where it rocks once.
fn flight(gift: usize, t: f32, peak: f32, turns: f32, landing: f32) -> [f32; 4] {
    let rest = rest(gift);
    if t <= 0.0 {
        return [0.0; 4];
    }
    if t >= landing {
        let rock = (-(t - landing) * 5.0).exp() * ((t - landing) * 14.0).sin() * 0.07;
        return [rest[0], rest[1] + rock.abs() * 0.4, rest[2], rest[3] + rock];
    }
    let (p, top) = (t / landing, 0.42);
    // Up fast and slowing, a breath held at the top, then down faster and faster.
    let height = if p < top { peak * land(p / top) } else { mix(peak, rest[1], give(span(p, top + 0.12, 1.0))) };
    [0.0, height, rest[2] * glide(span(p, 0.15, 1.0)), mix(0.0, rest[3] - TAU * turns, glide(p))]
}

/// A lid swung back by `angle` about its rear edge, as a case's: the edge stays where it sat.
fn hinged(angle: f32) -> [f32; 4] {
    let (y, z) = (-LID, -1.06);
    let (sin, cos) = (-angle).sin_cos();
    [0.0, y - (cos * y - sin * z), z - (sin * y + cos * z), -angle]
}

/// A gift untied `t` seconds into its act.
pub fn untied(gift: usize, t: f32) -> Parts {
    let pop = POP[gift];
    let mut parts = Parts::tied();
    // When the bow is drawn through its knot, and when the body's bands let go.
    let (pulled, fall) = match gift {
        2 => (0.5, (1.2, 1.9)),
        3 => (2.9, (3.1, 3.8)),
        _ => (0.3, (0.8, 1.4)),
    };
    // Its loops swell a little as the tails take the strain, then run through the knot; what is left narrows away.
    let drawn = glide(span(t, pulled, pulled + 0.9));
    parts.tie[0] = mix(1.0 + 0.14 * glide(span(t, (pulled - 0.5f32).max(0.0), pulled)), 0.3, drawn) - 0.3 * glide(span(t, pulled + 0.9, pulled + 1.4));
    parts.tie[1] = 1.0 - glide(span(t, pulled + 0.7, pulled + 1.4));
    // The bands fall open and stay where they lie.
    parts.tie[2] = mix(1.0, 0.5, give(span(t, fall.0, fall.1)));
    // A hairline of light under the lid grows as the box fills; the lid trembles, then goes, and the light stands up.
    parts.seam = glide(span(t, pop - 1.0, pop - 0.1)) * (1.0 - span(t, pop, pop + 0.15));
    let pressed = span(t, pop - 0.35, pop) * (1.0 - span(t, pop, pop + 0.01));
    parts.lid[3] = 0.02 * pressed * (t * 46.0).sin();
    parts.lid[1] = 0.012 * pressed * (t * 38.0).sin().abs();
    if t >= pop {
        parts.tie[3] = 1.0 + 0.9 * (-(t - pop) * 2.2).exp();
        parts.rise = COLUMN[gift] * land(span(t, pop, pop + 0.55));
    }
    match gift {
        // The silver one opens like a case: its lid swings back on its rear edge and stays.
        0 => parts.lid = if t >= pop { hinged(1.95 * land(span(t, pop, pop + 0.45)) + 0.08 * (-(t - pop - 0.45).max(0.0) * 6.0).exp() * ((t - pop - 0.45).max(0.0) * 15.0).sin()) } else { parts.lid },
        2 => parts.lid = if t >= pop { flight(gift, t - pop, 2.6, 1.0, 1.4) } else { parts.lid },
        3 => {
            // It turns once on itself, its inks sweeping, then its lid rises away and its walls fall open.
            parts.spin = TAU * glide(span(t, 0.9, 3.0));
            parts.lift = 0.12 * glide(span(t, 0.7, 1.7)) * (1.0 - glide(span(t, 3.0, 3.6)));
            if t >= pop {
                parts.lid = [0.0, 12.0 * give(span(t, pop, pop + 1.3)), 0.0, -0.9 * glide(span(t, pop, pop + 1.3))];
            }
            parts.open = glide(span(t, pop + 0.15, pop + 1.6));
        }
        // The copper one tosses its lid: a hop, two tumbles, down behind it.
        _ => parts.lid = if t >= pop { flight(gift, t - pop, 1.5, 2.0, 0.95) } else { parts.lid },
    }
    parts
}

/// An opened gift at rest: its lid where it went, its light low; the mystery flat on the floor.
pub fn opened(gift: usize) -> Parts {
    let mut parts = Parts { tie: [0.0, 0.0, 0.5, 0.4], rise: COLUMN[gift], ..Parts::tied() };
    match gift {
        0 => parts.lid = hinged(1.95),
        3 => (parts.lid, parts.open) = ([0.0, 60.0, 0.0, 0.0], 1.0),
        _ => parts.lid = flight(gift, 30.0, 0.0, 0.0, 1.0),
    }
    parts
}
