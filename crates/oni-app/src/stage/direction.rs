//! The welcome's direction: one gift on its screen, still and wrapped, then coming undone where it stands,
//! under a camera that barely moves. One function of the cue, so a check steps the film frame by frame.

use super::ease::{glide, span};
use super::unwrap::{self, opened, untied};
use super::{Frame, Gift};

/// Everything a picture depends on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cue {
    /// The gift on the screen shown, none on the message's.
    pub gift: Option<usize>,
    /// Seconds since it was opened; none while it is wrapped.
    pub since: Option<f32>,
    /// Seconds since the welcome opened, which the idle moves run on.
    pub clock: f32,
    /// The picture's width over its height, and the footer's keys under the stage, in picture heights.
    pub aspect: f32,
    pub foot: f32,
    /// How much of its light the stage shows, 0 while one screen gives way to the next.
    pub shown: f32,
    /// Where the gift stands across the picture, as a share of its width: where its film will come.
    pub middle: f32,
}

impl Cue {
    pub fn at(gift: Option<usize>, since: Option<f32>) -> Cue {
        Cue { gift, since, clock: since.unwrap_or(0.0), aspect: 1.6, foot: 0.0, shown: 1.0, middle: 0.5 }
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

/// The four gifts at the scale their words tell: two small ones, a tall silver and a low wide copper, the huge
/// gold one that fills the picture's height, the holographic mystery between.
pub(super) const STAND: [Stand; 4] = [
    Stand { x: 0.0, z: 0.0, size: 0.62, tall: 0.98, turn: 0.38, foil: 0.0 },
    Stand { x: 0.0, z: 0.0, size: 0.78, tall: 0.5, turn: -0.3, foil: 1.0 },
    Stand { x: 0.0, z: 0.0, size: 2.05, tall: 0.72, turn: 0.34, foil: 2.0 },
    Stand { x: 0.0, z: 0.0, size: 1.15, tall: 0.9, turn: -0.42, foil: 3.0 },
];
/// The lid's half-height, in half-widths, as the shader's.
pub(super) const LID: f32 = 0.2;
/// One camera for every gift, on one floor, so a difference of size reads as one: where it stands, where it looks.
const EYE: [f32; 3] = [-5.2, 4.0, 15.2];
const AIM: [f32; 3] = [0.0, 2.35, 0.0];
/// Once its words come, a gift fades away in this many seconds and its demo takes the stage.
const FADE: f32 = 0.6;

/// How much of the stage's light a gift opened `since` seconds ago keeps: whole until its words come, then none.
pub fn kept(gift: usize, since: Option<f32>) -> f32 {
    since.map_or(1.0, |t| 1.0 - glide(span(t, unwrap::title(gift), unwrap::title(gift) + FADE)))
}

fn gift(i: usize, cue: &Cue) -> Gift {
    let s = STAND[i];
    let sway = 0.025 * (cue.clock * 0.5).sin();
    let (mut turn, mut lift) = (s.turn + sway, 0.0);
    let parts = match cue.since {
        None => unwrap::Parts::tied(),
        Some(t) if t < unwrap::length(i) => {
            let parts = untied(i, t);
            turn = s.turn + parts.spin + sway;
            lift = parts.lift * s.size;
            parts
        }
        Some(_) => opened(i),
    };
    // No column of light: what was inside glows low in the open box, and the light of its opening fades.
    let glow = if cue.since.is_some() { parts.tie[3].min(1.0) * 0.6 } else { 0.0 };
    let tie = [parts.tie[0], parts.tie[1], parts.tie[2], glow];
    Gift { place: [s.x, 0.0, s.z, s.size], pose: [turn, lift, s.foil, 1.0], lid: parts.lid, tie, bow: [0.0; 4], shape: [s.tall, parts.open, parts.seam, 0.0] }
}

/// The picture at `cue`: the screen's gift alone in the middle, the same camera for each, fading once opened.
pub fn frame(cue: &Cue) -> Frame {
    let Some(i) = cue.gift else {
        return Frame { view: [0.0, 0.0, cue.clock, 0.0], show: [0.0, 0.0, 0.0, cue.foot], ..Frame::default() };
    };
    let s = STAND[i];
    let drift = 0.25 * (cue.clock * 0.11).sin();
    let gifts = std::array::from_fn(|k| if k == i { gift(i, cue) } else { Gift::default() });
    let frame = Frame {
        view: [0.0, 0.0, cue.clock, cue.shown * kept(i, cue.since)],
        eye: [EYE[0] + drift, EYE[1], EYE[2], 2.3],
        aim: [AIM[0], AIM[1], AIM[2], 3.0 * s.size.max(1.0)],
        light: [1.0, 1.0, 0.9, 1.0],
        lens: [(cue.middle - 0.5) * cue.aspect, 0.0, 0.0, 0.0],
        show: [0.0, 0.0, 0.0, cue.foot],
        gifts,
    };
    // The wrapped gift's middle on the picture's middle line, whatever its size: the film's line once it is opened.
    let rest = Frame { gifts: std::array::from_fn(|k| if k == i { gift(i, &Cue { since: None, ..*cue }) } else { Gift::default() }), ..frame };
    let [_, top, _, tall] = gift_spot(&rest, cue.aspect).unwrap_or([0.0, 0.25, 0.0, 0.5]);
    Frame { lens: [frame.lens[0], 0.0, 0.5 - (top + tall / 2.0), 0.0], ..frame }
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

/// Where the screen's gift stands in `frame`'s picture, its lid included: its left, top, width and height, as
/// shares of the picture; none on the message's screen.
pub fn gift_spot(frame: &Frame, aspect: f32) -> Option<[f32; 4]> {
    let g = frame.gifts.iter().find(|g| g.place[3] > 0.0)?;
    let (sin, cos) = g.pose[0].sin_cos();
    let corner = |k: usize| {
        let (x, y, z) = (if k & 1 == 0 { -1.06 } else { 1.06 }, if k & 2 == 0 { 0.0 } else { 2.0 * (g.shape[0] + LID) }, if k & 4 == 0 { -1.06 } else { 1.06 });
        [g.place[0] + (cos * x - sin * z) * g.place[3], g.place[1] + g.pose[1] + y * g.place[3], g.place[2] + (sin * x + cos * z) * g.place[3]]
    };
    let (mut low, mut high) = ((f32::MAX, f32::MAX), (f32::MIN, f32::MIN));
    for (x, y) in (0..8).map(|k| seen(frame, aspect, corner(k))) {
        low = (low.0.min(x), low.1.min(y));
        high = (high.0.max(x), high.1.max(y));
    }
    Some([low.0, low.1, high.0 - low.0, high.1 - low.1])
}
