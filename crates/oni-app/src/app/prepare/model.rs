//! The surface's model, pure: its steps, its frames, their picking and flagging, the card's
//! shape and the loupe's cut, each checked against Swift's self-checks.

use super::*;

mod step;
pub use step::*;

/// `RollCapture`: what the roll was digitised with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Capture {
    Scanner,
    #[default]
    Camera,
    RgbPasses,
}

impl Capture {
    pub const ALL: [Capture; 3] = [Capture::Scanner, Capture::Camera, Capture::RgbPasses];

    pub fn raw(self) -> &'static str {
        match self {
            Capture::Scanner => "scanner",
            Capture::Camera => "camera",
            Capture::RgbPasses => "rgbPasses",
        }
    }

    pub fn from_raw(raw: &str) -> Option<Capture> {
        Capture::ALL.into_iter().find(|c| c.raw() == raw)
    }
}

/// `RollFrame`: a frame of the roll, its crop in its scan's fractions and its place in the measure.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub path: String,
    pub fingerprint: String,
    pub crop: Rect,
    /// Width over height of the scan after its quarter turns.
    pub aspect: Option<f64>,
    pub ticked: bool,
    /// Taking the roll's window but never measured, so it sets no end.
    pub flagged: bool,
    pub capture: Option<CaptureExposure>,
    pub passes: Option<RgbPasses>,
}

impl Frame {
    pub(super) fn new(path: &str, fingerprint: &str) -> Frame {
        Frame { path: path.into(), fingerprint: fingerprint.into(), crop: Rect::UNIT, aspect: None, ticked: true, flagged: false, capture: None, passes: None }
    }
}

/// `RollFrame.measured`: the frames ticked and not flagged.
pub fn measured(frames: &[Frame]) -> Vec<&Frame> {
    frames.iter().filter(|f| f.ticked && !f.flagged).collect()
}

/// `RollFrame.aspect`: the scan's width over height after its quarter turns.
pub fn aspect(scan: Option<(usize, usize)>, geometry: &Geometry) -> Option<f64> {
    let (w, h) = scan?;
    let (tw, th) = geometry.oriented_size((w as f64, h as f64));
    (th > 0.0).then(|| tw / th)
}

/// `RollPicking.ownRoll`: the picked frame, those measured with its roll, and those never measured.
pub fn own_roll(picked: &str, folder: &[String], reference: impl Fn(&str) -> Option<Id>) -> Vec<String> {
    let own = reference(picked);
    folder.iter().filter(|p| *p == picked || reference(p).is_none_or(|r| Some(r) == own)).cloned().collect()
}

/// `RollPicking.flagging`: a click toggles the flag, and flagging brings an unticked frame back in.
pub fn flagging(frames: &mut [Frame], index: usize) {
    let Some(f) = frames.get_mut(index) else { return };
    f.flagged = !f.flagged;
    if f.flagged {
        f.ticked = true;
    }
}

/// `RollFrameView.fitted`: a box of `aspect` centred in `size`.
pub fn fitted(aspect: f64, size: (f64, f64)) -> (f64, f64, f64, f64) {
    if size.0 <= 0.0 || size.1 <= 0.0 || aspect <= 0.0 {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let w = size.0.min(size.1 * aspect);
    let h = w / aspect;
    ((size.0 - w) / 2.0, (size.1 - h) / 2.0, w, h)
}

/// `RollBaseSurface.fitted`: an image of `image` pixels scaled whole into `size`, centred.
pub fn fitted_image(image: (f64, f64), size: (f64, f64)) -> (f64, f64, f64, f64) {
    if image.0 <= 0.0 || image.1 <= 0.0 || size.0 <= 0.0 || size.1 <= 0.0 {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let scale = (size.0 / image.0).min(size.1 / image.1);
    let (w, h) = (image.0 * scale, image.1 * scale);
    ((size.0 - w) / 2.0, (size.1 - h) / 2.0, w, h)
}

pub fn left_out_text(n: usize) -> String {
    match n {
        0 => String::new(),
        n => trn("{n} frame of this folder, prepared with another roll, is left out.", "{n} frames of this folder, prepared with another roll, are left out.", n as u64),
    }
}

/// The roll's most common capture and the frames read apart from it.
pub fn apart(frames: &[Frame]) -> (Option<CaptureExposure>, HashSet<String>) {
    let captures: Vec<Option<CaptureExposure>> = frames.iter().map(|f| f.capture).collect();
    let Some(common) = CaptureExposure::common(&captures) else { return (None, HashSet::new()) };
    let apart = frames.iter().filter(|f| f.capture.is_some_and(|c| c != common)).map(|f| f.path.clone()).collect();
    (Some(common), apart)
}

/// `DSWarning`'s text when the roll mixes captures, empty when it does not: their exposure evened out,
/// and the light panel's aperture once an aperture differs, the one choice the step leaves the hand.
pub fn differ_text(frames: &[Frame]) -> String {
    let (common, apart) = apart(frames);
    let Some(common) = common.filter(|_| !apart.is_empty()) else { return String::new() };
    let (apart, count, capture) = (apart.len(), frames.len(), common.label());
    if frames.iter().all(|f| f.capture.is_none_or(|c| c.aperture == common.aperture)) {
        return tr!(
            "{apart} of {count} frames, marked, were digitised with other settings than the roll's {capture}. Their exposure is evened out to it: nothing to do.",
            apart = apart,
            count = count,
            capture = capture,
        );
    }
    tr!(
        "{apart} of {count} frames, marked, were digitised with other settings than the roll's {capture}. Their exposure is evened out to it. Shoot the light panel at {aperture}, as most of the roll; a frame at another aperture may keep slightly uneven corners.",
        apart = apart,
        count = count,
        capture = capture,
        aperture = common.f_number(),
    )
}


/// The loupe's cut of the source in pixels, where it lands in points, and its rings: the pointer's
/// disc at x, y, diameter, then the picked one.
#[derive(Debug, PartialEq)]
pub struct LoupeLaid {
    pub cut: [i32; 4],
    pub at: [f64; 4],
    pub rings: [f64; 6],
    pub picked: bool,
}

/// `RollBaseSurface.loupe`: the pointer's pixel held at the centre past an edge too, nearest
/// neighbour over at least 64 source pixels, 32 on the display copy, or twice the disc.
pub fn loupe(pointer: (f64, f64), rect: (f64, f64, f64, f64), display_width: usize, source: (usize, usize), whole: bool, radius: f64, picked: Option<((f64, f64), f64)>) -> LoupeLaid {
    let side = 160.0;
    let (width, height) = (source.0 as f64, source.1 as f64);
    let display = display_width.max(1) as f64;
    let disc_pixels = 2.0 * radius * width / display;
    let least: f64 = if whole { 64.0 } else { 32.0 };
    let span = least.max((2.0 * disc_pixels).ceil());
    let unit = ((pointer.0 - rect.0) / rect.2.max(1.0), (pointer.1 - rect.1) / rect.3.max(1.0));
    let origin = ((unit.0 * width - span / 2.0).round(), (unit.1 * height - span / 2.0).round());
    let inside = oni_measure::window::intersection(&Rect::new(origin.0, origin.1, span, span), &Rect::new(0.0, 0.0, width, height));
    let scale = side / span;
    let place = |u: (f64, f64)| ((u.0 * width - origin.0) * scale, (u.1 * height - origin.1) * scale);
    let here = place(unit);
    let (picked_at, picked_disc) = picked.map_or(((0.0, 0.0), 0.0), |(u, r)| (place(u), 2.0 * r * width / display * scale));
    let empty = inside.width <= 0.0 || inside.height <= 0.0;
    LoupeLaid {
        cut: if empty { [0; 4] } else { [inside.x as i32, inside.y as i32, inside.width as i32, inside.height as i32] },
        at: [(inside.x - origin.0) * scale, (inside.y - origin.1) * scale, inside.width * scale, inside.height * scale],
        rings: [here.0, here.1, disc_pixels * scale, picked_at.0, picked_at.1, picked_disc],
        picked: picked.is_some(),
    }
}

/// Each file's draft: `None` clears it, so a file no longer a frame keeps no other grouping's draft.
pub(super) type Drafts = Vec<(String, Option<RollDraft>)>;

impl Roll {
    /// The drafts a step writes now; a step taken while a frame is still read waits for the last one.
    pub(super) fn step_drafts(&mut self) -> Option<Drafts> {
        self.draft_waiting = self.shown && self.read.len() < self.sources.len();
        self.drafts()
    }

    /// Whether the frame just read was the last a step's draft waited for, which is then written once.
    pub(super) fn draft_landed(&mut self) -> bool {
        let landed = self.draft_waiting && self.read.len() >= self.sources.len();
        self.draft_waiting &= !landed;
        landed
    }

    /// The draft every frame shares, its own ticked and flagged laid per frame, on every file the roll
    /// read: whichever lands first on a resume finds the grouping. `None` until it may be written.
    pub(super) fn drafts(&mut self) -> Option<Drafts> {
        if !self.shown || self.progress.is_some() || (self.step == Step::Film && self.draft_id.is_none()) || self.sources.is_empty() || self.read.len() < self.sources.len() {
            return None;
        }
        let id = *self.draft_id.get_or_insert_with(Id::new);
        let shared = RollDraft {
            id,
            step: self.step.name().into(),
            film: self.film,
            capture: self.capture.raw().into(),
            pass_order: Some(self.pass_order),
            pass_offset: Some(self.pass_offset),
            flat_field: Some(self.flat.clone()),
            base: self.base.as_ref().map(|b| DraftBase { path: b.path.clone(), x: b.unit.0, y: b.unit.1, radius: b.radius }),
            window: Some(self.window),
            ticked: false,
            flagged: false,
        };
        let places = self.places();
        let drafts = self.sources.iter().map(|f| (f.path.clone(), places.get(f.path.as_str()).map(|&(ticked, flagged)| RollDraft { ticked, flagged, ..shared.clone() })));
        Some(drafts.collect())
    }

    /// Each frame's tick and flag, by its path.
    pub(super) fn places(&self) -> HashMap<&str, (bool, bool)> {
        self.frames.iter().map(|f| (f.path.as_str(), (f.ticked, f.flagged))).collect()
    }
}

#[cfg(test)]
mod tests;
