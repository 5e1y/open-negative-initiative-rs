//! The welcome's direction: where the camera stands, what each gift does and what light falls, at every
//! moment of every act. One function of the cue, so a check steps the film frame by frame.

use super::ease::{glide, mix, mix3, span};
use super::film::{self, Plate};
use super::unwrap::{self, opened, untied};
use super::{Frame, Gift};

/// What plays: the overture once, the hall where the gifts wait, a gift untied, its page, the message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Overture,
    Hall,
    Unwrap(usize),
    Page(usize),
    Message,
}

/// Everything a picture depends on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cue {
    pub act: Act,
    /// Seconds since the act began.
    pub t: f32,
    /// Seconds since the welcome opened, which the idle moves and the shimmer run on.
    pub clock: f32,
    /// The picture's width over its height, and the words' column beside the stage, in picture heights.
    pub aspect: f32,
    pub words: f32,
    /// The footer's keys under the stage, in picture heights: the stage goes dark behind them.
    pub foot: f32,
    pub opened: [bool; 4],
    /// How far the hand is on each gift, 0 away to 1 resting on it.
    pub hover: [f32; 4],
    /// The pointer about the window's middle, -1 to 1 each way.
    pub pointer: (f32, f32),
    /// The huge gift's step and the seconds into it; the photo laid back over its print, 0 for none.
    pub step: usize,
    pub step_t: f32,
    pub compare: f32,
    /// The act left for this one and the seconds it had played, which the camera sets out from.
    pub before: Option<(Act, f32)>,
}

impl Cue {
    pub fn at(act: Act, t: f32) -> Cue {
        Cue { act, t, clock: t, aspect: 1.6, words: 0.0, foot: 0.0, opened: [false; 4], hover: [0.0; 4], pointer: (0.0, 0.0), step: 0, step_t: 0.0, compare: 0.0, before: None }
    }
}

/// A gift where it stands: across, toward the camera, its half-width, its body's half-height in half-widths,
/// its turn at rest, its foil (0 silver, 1 copper, 2 gold, 3 holographic).
#[derive(Clone, Copy)]
pub(super) struct Stand {
    pub x: f32,
    pub z: f32,
    pub size: f32,
    pub tall: f32,
    pub turn: f32,
    pub foil: f32,
}

/// The four gifts: a tall silver one, a low wide copper one, the huge gold one twice their height, the
/// holographic mystery.
pub(super) const STAND: [Stand; 4] = [
    Stand { x: -6.3, z: 0.6, size: 0.78, tall: 0.98, turn: 0.38, foil: 0.0 },
    Stand { x: -3.5, z: 1.0, size: 0.95, tall: 0.5, turn: -0.3, foil: 1.0 },
    Stand { x: 0.9, z: -0.5, size: 2.1, tall: 0.72, turn: 0.34, foil: 2.0 },
    Stand { x: 6.2, z: 0.7, size: 0.95, tall: 0.9, turn: -0.42, foil: 3.0 },
];
/// The lid's half-height, in half-widths, as the shader's.
pub(super) const LID: f32 = 0.2;
/// How long the overture lasts; the hall and a page hold until the person moves on.
pub const OVERTURE: f32 = 7.4;
/// When each gift takes its light in the overture: the huge one first and alone, then the others in a row.
const ENTRANCE: [f32; 4] = [5.0, 5.25, 0.0, 5.5];

/// A camera's stand.
#[derive(Clone, Copy)]
struct Shot {
    eye: [f32; 3],
    aim: [f32; 3],
    focal: f32,
    pool: f32,
}

fn between(a: Shot, b: Shot, p: f32) -> Shot {
    Shot { eye: mix3(a.eye, b.eye, p), aim: mix3(a.aim, b.aim, p), focal: mix(a.focal, b.focal, p), pool: mix(a.pool, b.pool, p) }
}

/// The hall: the four gifts in a row under the headline, seen a little from above, the camera drifting a hair; a
/// narrow window stands it further back.
fn hall(cue: &Cue) -> Shot {
    let back = (1.6 / cue.aspect).max(1.0);
    let drift = 0.25 * (cue.clock * 0.13).sin();
    Shot { eye: [0.1 + drift + 0.9 * cue.pointer.0, 5.4 - 0.4 * cue.pointer.1, 24.5 * back], aim: [0.1, 3.5, 0.0], focal: 2.2, pool: 9.5 }
}

/// A gift alone: the camera `turn` radians round it, `far` box heights away, aiming `rise` box heights up it.
fn hero(gift: usize, turn: f32, rise: f32, far: f32) -> Shot {
    let s = STAND[gift];
    let height = 2.0 * (s.tall + LID) * s.size + 0.5 * s.size;
    let away = far * height;
    Shot { eye: [s.x + away * turn.sin(), 0.62 * height + 0.12 * away, s.z + away * turn.cos()], aim: [s.x, rise * height, s.z], focal: 2.3, pool: 3.0 * s.size }
}

/// The film's place over the huge gift: its middle and its frame's half-height. The photo rises out of the box
/// to it; the three prints, fanned, stand higher still, the box left under the picture.
fn plate_place(plate: &Plate) -> [f32; 4] {
    let s = STAND[2];
    let top = 2.0 * s.tall * s.size;
    [s.x, mix(top * 0.5, top + 6.4, plate.risen) + 6.5 * plate.fan, s.z, mix(0.9, 2.0, plate.risen)]
}

/// The camera on the huge gift's page and how far its lens looks down off its axis: where the photo rises to, the
/// box left under the picture; close on the film's grain, back for the scan, then before the print and the fan.
fn film_shot(cue: &Cue, plate: &Plate) -> (Shot, f32) {
    let [x, y, z, half] = plate_place(&Plate { risen: 1.0, ..*plate });
    // Square to the film and on its middle; never quite still: a slow push in while the steps play.
    let near = 1.0 - 0.0025 * cue.t.min(16.0);
    let on = |far: f32, pool: f32| Shot { eye: [x, y, z + far * near], aim: [x, y, z], focal: 2.3, pool };
    // The lighthouse, where the grain and the halation read.
    let light = [x + 0.585 * half, y + 0.55 * half, z];
    let close = Shot { eye: [light[0], light[1], z + 3.5], aim: light, focal: 2.3, pool: 5.0 };
    let back = (1.6 / cue.aspect).max(1.0);
    let shot = between(between(between(on(20.0 * back, 5.0), close, plate.close), on(14.6 * back, 5.0), plate.forward), on(26.5 * back, 6.0), plate.fan);
    (shot, mix(mix(-0.075, -0.105, plate.forward), -0.17, plate.fan))
}

/// The camera on a small gift's page: the opened gift small at the foot of its light, its scene's card above it.
fn page(gift: usize, cue: &Cue) -> Shot {
    let sway = 0.08 * (cue.clock * 0.22).sin();
    let s = STAND[gift];
    let height = 2.0 * s.tall * s.size;
    // The box a fifth of the picture's height, its mouth seen from above.
    let seen = height / 0.2;
    let away = seen * 2.3;
    let (round, up) = (sway - 0.2, 0.5 * height + 0.29 * seen);
    Shot { eye: [s.x + away * round.sin(), up + 0.12 * away, s.z + away * round.cos()], aim: [s.x, up, s.z], focal: 2.3, pool: 3.0 * s.size }
}

fn gift(i: usize, cue: &Cue) -> Gift {
    let s = STAND[i];
    let sway = 0.03 * (cue.clock * 0.5 + i as f32 * 1.7).sin();
    let (mut turn, mut lift, mut light) = (s.turn + sway, 0.0, 1.0);
    let mut parts = if cue.opened[i] { opened(i) } else { unwrap::Parts::tied() };
    match cue.act {
        // Nothing moves: each stands in the dark until its own light finds it.
        Act::Overture => light = glide(span(cue.t, ENTRANCE[i], ENTRANCE[i] + 0.9)),
        Act::Hall => {
            lift = 0.26 * s.size * cue.hover[i];
            turn += 0.16 * cue.hover[i] + 0.05 * cue.hover[i] * (cue.clock * 2.2).sin();
            // The hand on one gift sets the others back; back from a gift's own scene, they come out of the dark.
            let others = cue.hover.iter().enumerate().filter(|(j, _)| *j != i).fold(0.0f32, |most, (_, h)| most.max(*h));
            let back = match cue.before {
                Some((Act::Page(shown) | Act::Unwrap(shown), _)) if shown != i => glide(span(cue.t, 0.1, 1.3)),
                _ => 1.0,
            };
            light = (0.9 + 0.2 * cue.hover[i]) * (1.0 - 0.32 * others) * back;
            // An opened gift's light stays low, under the headline.
            parts.rise *= 0.5;
        }
        Act::Unwrap(chosen) if chosen == i => {
            parts = untied(i, cue.t);
            let length = unwrap::length(i);
            turn = s.turn + parts.spin + 0.3 * glide(span(cue.t, 0.0, length));
            lift = parts.lift * s.size;
        }
        // The gifts left go dark at once: the stage is the chosen one's.
        Act::Unwrap(_) => light = 1.0 - glide(span(cue.t, 0.0, 0.4)),
        Act::Page(shown) if shown == i => {
            parts = opened(i);
            // Once the film stands in the light, the box's own light draws back behind it.
            let drawn = if i == 2 && cue.step > 0 { 1.0 } else { 0.0 };
            parts.tie[3] = mix(0.9, 0.3, drawn) + 0.04 * (cue.clock * 1.3).sin();
            turn = s.turn + 0.3;
        }
        Act::Page(_) => light = 0.0,
        Act::Message => {
            parts = opened(i);
            // Their lights stand up one after the other, hold together, then sink under the words; a last swell as
            // the way on arrives.
            let up = glide(span(cue.t, 0.3 + 0.35 * i as f32, 1.3 + 0.35 * i as f32));
            let down = glide(span(cue.t, 3.4, 4.8));
            parts.tie[3] = 1.1 * up * (1.0 - 0.55 * down) * (1.0 + 0.25 * glide(span(cue.t, 6.6, 7.4)));
            parts.rise *= up;
            light = 0.8;
        }
    }
    Gift { place: [s.x, 0.0, s.z, s.size], pose: [turn, lift, s.foil, light], lid: parts.lid, tie: parts.tie, bow: [0.0; 4], shape: [s.tall, parts.open, parts.seam, parts.rise] }
}

/// A camera's stand with what the lens and the lights do there: the exposure, how far the lens looks off its
/// axis, and the key, the softbox, the rim.
#[derive(Clone, Copy)]
struct View {
    shot: Shot,
    exposure: f32,
    off: (f32, f32),
    lights: [f32; 3],
    /// Where a strip of light stands round the stage, in radians; 0 puts it away.
    sweep: f32,
}

/// Where the camera stands `t` seconds into `act`.
fn view(act: Act, t: f32, cue: &Cue, plate: &Plate) -> View {
    let wide = hall(cue);
    let level = (0.0, -0.03);
    let lit = [1.0, 1.0, 0.9];
    match act {
        Act::Overture => {
            // Before the huge gift, standing in the dark: the rim finds its edges, then the softbox its faces;
            // then back and up to the hall as the others take their light.
            let near = hero(2, -0.36, 0.53, 4.3);
            let on = glide(span(t, 3.5, 4.8));
            let back = glide(span(t, 5.0, OVERTURE - 0.3));
            // A strip of light is carried across the gift's face as the softbox comes up, then put away.
            let sweep = if t > 3.4 && t < 5.4 { -0.36 + mix(-0.42, 0.42, glide(span(t, 3.5, 5.3))) } else { 0.0 };
            View { shot: between(near, wide, back), exposure: 1.0, off: (0.0, mix(0.035, level.1, back)), lights: [on, on, 0.9 * glide(span(t, 2.8, 3.8))], sweep }
        }
        Act::Hall => View { shot: wide, exposure: 1.0, off: level, lights: lit, sweep: 0.0 },
        Act::Unwrap(i) => {
            let length = unwrap::length(i);
            let (from, sweep) = unwrap::orbit(i);
            let around = from + sweep * glide(span(t, 0.0, length));
            let pop = unwrap::pop(i);
            // After its lid goes the camera follows the light up; the mystery, fallen open, is looked down on.
            let after = glide(span(t, pop, pop + 1.6));
            // The silver one's lid swings up: the camera aims higher, to keep it under the gift's name.
            let mut shot = hero(i, around, 0.45 + [0.5, 0.1, 0.1, -0.22][i] * after, 4.4 + 0.5 * glide(span(t, pop, length)));
            // It rises too, to look into the opened box.
            shot.eye[1] += 0.35 * (shot.eye[1] - shot.aim[1]).abs().max(1.0) * after;
            View { shot, exposure: 1.0, off: (0.0, 0.03), lights: lit, sweep: 0.0 }
        }
        Act::Page(2) => {
            let (shot, down) = film_shot(cue, plate);
            View { shot, exposure: 1.0, off: (0.0, down), lights: lit, sweep: 0.0 }
        }
        Act::Page(i) => View { shot: page(i, cue), exposure: 1.0, off: (-cue.words / 2.0, -0.04), lights: lit, sweep: 0.0 },
        Act::Message => {
            let high = Shot { eye: [0.1, 7.6, 35.0], aim: [0.1, 5.6, 0.0], focal: 2.2, pool: 11.0 };
            View { shot: between(wide, high, glide(span(t, 0.0, 2.6))), exposure: mix(1.0, 0.85, glide(span(t, 3.4, 4.8))), off: (0.0, 0.03), lights: lit, sweep: 0.0 }
        }
    }
}

/// How long the camera takes from one act to the next, and whether it goes through black: a move where the two
/// stands belong together, a cut through the dark between two gifts.
fn travel(from: Act, to: Act) -> (f32, bool) {
    match (from, to) {
        (Act::Hall, Act::Unwrap(_)) => (1.0, false),
        (Act::Unwrap(a), Act::Page(b)) if a == b => (1.3, false),
        (Act::Hall, Act::Page(_) | Act::Message) | (_, Act::Hall) => (1.3, false),
        _ => (0.7, true),
    }
}

/// The camera at `cue`: its act's stand, reached from where the act before left it.
fn camera(cue: &Cue, plate: &Plate) -> View {
    let to = view(cue.act, cue.t, cue, plate);
    let Some((act, t)) = cue.before else { return to };
    let (length, dark) = travel(act, cue.act);
    if cue.t >= length {
        return to;
    }
    let from = view(act, t, cue, plate);
    let p = glide(span(cue.t, 0.0, length));
    if dark {
        let half = span(cue.t, 0.0, length);
        return if half < 0.5 { View { exposure: from.exposure * (1.0 - glide(half * 2.0)), ..from } } else { View { exposure: to.exposure * glide(half * 2.0 - 1.0), ..to } };
    }
    let off = (mix(from.off.0, to.off.0, p), mix(from.off.1, to.off.1, p));
    View { shot: between(from.shot, to.shot, p), exposure: mix(from.exposure, to.exposure, p), off, lights: mix3(from.lights, to.lights, p), sweep: to.sweep }
}

/// The picture at `cue`.
pub fn frame(cue: &Cue) -> Frame {
    let plate = film::plate(cue.step, cue.step_t);
    let View { shot, exposure, off, lights, sweep } = camera(cue, &plate);
    // Through a cut in the dark, the stage stays the act's it leaves until the dark is whole.
    let staged = match cue.before {
        Some((act, t)) if travel(act, cue.act).1 && cue.t < travel(act, cue.act).0 * 0.5 => Cue { act, t, ..*cue },
        _ => *cue,
    };
    let mut place = plate_place(&plate);
    if cue.act != Act::Page(2) {
        place[3] = 0.0;
    }
    Frame {
        view: [0.0, 0.0, cue.clock, exposure],
        eye: [shot.eye[0], shot.eye[1], shot.eye[2], shot.focal],
        aim: [shot.aim[0], shot.aim[1], shot.aim[2], shot.pool],
        light: [lights[0], lights[1], lights[2], 1.0],
        plate: place,
        turn: [0.0, plate.sheet, plate.light, plate.grain],
        film: [plate.head, plate.inverted, plate.lamp, plate.strip],
        lens: [off.0, cue.compare, off.1, (-2.0 * off.0).max(0.0)],
        show: [plate.stock, if plate.shimmer { 1.0 } else { 0.0 }, 0.0, cue.foot],
        strip: [plate.slide, plate.developed, plate.latent, plate.halation],
        fan: [plate.fan, 0.0, plate.close.clamp(0.0, 1.0), sweep],
        gifts: std::array::from_fn(|i| gift(i, &staged)),
    }
}

/// Where a point of the stage falls in `frame`'s picture, as shares of its width and height.
fn seen(frame: &Frame, aspect: f32, p: [f32; 3]) -> (f32, f32) {
    let sub = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let unit = |a: [f32; 3]| a.map(|v| v / dot(a, a).sqrt());
    let eye = [frame.eye[0], frame.eye[1], frame.eye[2]];
    let forward = unit(sub([frame.aim[0], frame.aim[1], frame.aim[2]], eye));
    let right = unit(cross(forward, [0.0, 1.0, 0.0]));
    let up = cross(right, forward);
    let to = sub(p, eye);
    let depth = dot(to, forward).max(0.01);
    let x = dot(to, right) / depth * frame.eye[3] + frame.lens[0];
    let y = -dot(to, up) / depth * frame.eye[3] + frame.lens[2];
    (x / aspect + 0.5, y + 0.5)
}

/// The box about `corners` as `frame` shows them: its left, top, width and height, as shares of the picture.
fn boxed(frame: &Frame, aspect: f32, corners: impl Iterator<Item = [f32; 3]>) -> [f32; 4] {
    let (mut low, mut high) = ((f32::MAX, f32::MAX), (f32::MIN, f32::MIN));
    for (x, y) in corners.map(|c| seen(frame, aspect, c)) {
        low = (low.0.min(x), low.1.min(y));
        high = (high.0.max(x), high.1.max(y));
    }
    [low.0, low.1, high.0 - low.0, high.1 - low.1]
}

/// Where gift `i` stands in `frame`'s picture, its bow included.
pub fn gift_spot(frame: &Frame, aspect: f32, i: usize) -> [f32; 4] {
    let g = frame.gifts[i];
    let (sin, cos) = g.pose[0].sin_cos();
    let corner = |k: usize| {
        let (x, y, z) = (if k & 1 == 0 { -1.06 } else { 1.06 }, if k & 2 == 0 { 0.0 } else { 2.0 * (g.shape[0] + LID) + 0.6 }, if k & 4 == 0 { -1.06 } else { 1.06 });
        [g.place[0] + (cos * x - sin * z) * g.place[3], g.place[1] + g.pose[1] + y * g.place[3], g.place[2] + (sin * x + cos * z) * g.place[3]]
    };
    boxed(frame, aspect, (0..8).map(corner))
}

/// Where the mouth of gift `i` shows in `frame`'s picture: what rises out of the box rises from there.
pub fn mouth_spot(frame: &Frame, aspect: f32, i: usize) -> (f32, f32) {
    let g = frame.gifts[i];
    seen(frame, aspect, [g.place[0], g.pose[1] + 2.0 * g.shape[0] * g.place[3], g.place[2]])
}

/// Where the middle print stands in `frame`'s picture, none while no print's sheet shows.
pub fn plate_spot(frame: &Frame, aspect: f32) -> Option<[f32; 4]> {
    let half = frame.plate[3];
    if half <= 0.0 || frame.turn[1] <= 0.0 {
        return None;
    }
    let corner = |k: usize| [frame.plate[0] + if k & 1 == 0 { -1.5 } else { 1.5 } * half, frame.plate[1] + if k & 2 == 0 { -1.0 } else { 1.0 } * half, frame.plate[2]];
    Some(boxed(frame, aspect, (0..4).map(corner)))
}
