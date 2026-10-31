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

/// The key's title on the window step.
pub fn measure_title(n: usize) -> String {
    trn("MEASURE {n} FRAME", "MEASURE {n} FRAMES", n as u64)
}

pub fn left_out_text(n: usize) -> String {
    match n {
        0 => String::new(),
        n => trn("{n} frame of this folder, measured on another roll, is left out.", "{n} frames of this folder, measured on another roll, are left out.", n as u64),
    }
}

/// The roll's most common capture and the frames read apart from it.
pub fn apart(frames: &[Frame]) -> (Option<CaptureExposure>, HashSet<String>) {
    let captures: Vec<Option<CaptureExposure>> = frames.iter().map(|f| f.capture).collect();
    let Some(common) = CaptureExposure::common(&captures) else { return (None, HashSet::new()) };
    let apart = frames.iter().filter(|f| f.capture.is_some_and(|c| c != common)).map(|f| f.path.clone()).collect();
    (Some(common), apart)
}

/// `DSWarning`'s text when the roll mixes captures, empty when it does not.
pub fn differ_text(frames: &[Frame]) -> String {
    let (common, apart) = apart(frames);
    let Some(common) = common.filter(|_| !apart.is_empty()) else { return String::new() };
    tr!(
        "{apart} of {count} frames, marked, were digitised with other settings than the roll's {capture}. Their exposure is evened out to it; a different aperture also changes the vignetting the flat-field measured, so re-shoot them in manual for the cleanest roll.",
        apart = apart.len(),
        count = frames.len(),
        capture = common.label(),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(path: &str) -> Frame {
        Frame::new(path, "")
    }

    /// `RollPicking.selfCheck`'s `ownRoll`: frames 0 to 4, references a, a, b, none, none.
    #[test]
    fn a_frame_brings_its_own_roll_and_every_unmeasured_frame_of_its_folder() {
        let (a, b) = (Id::new(), Id::new());
        let folder: Vec<String> = (0..5).map(|i| format!("/r/{i}")).collect();
        let reference = |p: &str| match p {
            "/r/0" | "/r/1" => Some(a),
            "/r/2" => Some(b),
            _ => None,
        };
        assert_eq!(own_roll("/r/0", &folder, reference), ["/r/0", "/r/1", "/r/3", "/r/4"]);
        assert_eq!(own_roll("/r/3", &folder, reference), ["/r/3", "/r/4"], "an unmeasured frame brings the unmeasured ones");
        // Twin: the frame of roll b brings b's and the unmeasured, never a's.
        assert_eq!(own_roll("/r/2", &folder, reference), ["/r/2", "/r/3", "/r/4"]);
        assert_ne!(own_roll("/r/2", &folder, reference), ["/r/0", "/r/3", "/r/4"]);
    }

    /// `RollPicking.selfCheck`'s `flagging`.
    #[test]
    fn a_click_flags_a_frame_a_second_unflags_it_and_flagging_ticks_it() {
        let mut frames = vec![frame("a"), frame("b")];
        flagging(&mut frames, 0);
        assert!(frames[0].flagged && !frames[1].flagged);
        flagging(&mut frames, 0);
        assert!(!frames[0].flagged, "a second click unflags");
        frames[1].ticked = false;
        flagging(&mut frames, 1);
        assert!(frames[1].flagged && frames[1].ticked, "the frame taken as flat-field comes back in");
        // Twin: unflagging leaves the frame ticked, never unticked.
        flagging(&mut frames, 1);
        assert!(!frames[1].flagged && frames[1].ticked);
    }

    #[test]
    fn the_steps_walk_past_the_flat_field_on_a_scanner_and_hold_at_either_end() {
        let n = ConversionMode::Negative;
        assert_eq!(neighbour(Step::Film, 1, Capture::Camera, n), Step::FlatField);
        assert_eq!(neighbour(Step::Film, 1, Capture::Scanner, n), Step::Base);
        assert_eq!(neighbour(Step::Base, -1, Capture::Scanner, n), Step::Film);
        assert_eq!(neighbour(Step::Window, 1, Capture::Camera, n), Step::Window);
        assert_eq!(neighbour(Step::Film, -1, Capture::Camera, n), Step::Film);
        // Twin: on a camera the flat-field is a step of its own.
        assert_ne!(neighbour(Step::Base, -1, Capture::Camera, n), Step::Film);
        assert_eq!(Step::ALL.map(Step::name), ["film", "flatField", "base", "flags", "window"]);
        assert_eq!(Step::named("flags"), Some(Step::Flags));
        assert_eq!(Capture::ALL.map(Capture::raw), ["scanner", "camera", "rgbPasses"]);
        let flat = FlatField { mask: oni_settings::frame::FlatFieldMask::ValoiEasy35V1, ..FlatField::default() };
        assert!(filled(Step::FlatField, &flat, false) && !filled(Step::FlatField, &FlatField::default(), false));
        assert!(!filled(Step::Base, &flat, false) && filled(Step::Base, &flat, true) && filled(Step::Flags, &FlatField::default(), false));
        assert_eq!((measure_title(1), measure_title(3)), ("MEASURE 1 FRAME".to_string(), "MEASURE 3 FRAMES".to_string()));
        assert_eq!(left_out_text(1), "1 frame of this folder, measured on another roll, is left out.");
        assert_eq!(left_out_text(2), "2 frames of this folder, measured on another roll, are left out.");
    }

    /// `RollFrameView.selfCheck`: a quarter-turned, cropped frame shows in its render's shape.
    #[test]
    fn a_quarter_turned_frame_shows_in_its_own_scans_shape() {
        let g = Geometry { quarter_turns: 1, crop: Rect::new(0.1, 0.2, 0.7, 0.5), ..Geometry::default() };
        let a = aspect(Some((300, 200)), &g).unwrap();
        let b = fitted(a, (1000.0, 1000.0));
        let shown = frames::shown(g.crop, b);
        let rendered = g.cropped_pixel_size((300.0, 200.0));
        let ratio = (shown.2 / shown.3) / (rendered.0 / rendered.1);
        assert!((ratio - 1.0).abs() < 0.01, "shown {shown:?}, rendered {rendered:?}");
        // Twin: in the unturned shape the same crop is off by more than half.
        let wrong = frames::shown(g.crop, fitted(1.5, (1000.0, 1000.0)));
        assert!(((wrong.2 / wrong.3) / (rendered.0 / rendered.1) - 1.0).abs() > 0.5);
    }

    #[test]
    fn a_draft_resumes_its_step_and_leaves_out_the_frames_of_none() {
        let frames = vec![frame("a"), frame("b"), frame("c")];
        let mut r = Roll { sources: frames.clone(), frames, ..Roll::default() };
        let id = Id::new();
        let draft = RollDraft {
            id,
            step: "flags".into(),
            film: ConversionMode::Negative,
            capture: "scanner".into(),
            pass_order: None,
            flat_field: None,
            base: Some(DraftBase { path: "b".into(), x: 0.1, y: 0.2, radius: 3.0 }),
            window: None,
            ticked: true,
            flagged: true,
        };
        r.resume(None, 0);
        assert!(r.frames[0].ticked, "with no draft yet a frame keeps its tick");
        r.resume(Some(&draft), 1);
        assert_eq!((r.step, r.film, r.capture), (Step::Flags, ConversionMode::Negative, Capture::Scanner));
        assert_eq!(r.base, Some(BasePick { path: "b".into(), unit: (0.1, 0.2), radius: 3.0 }));
        assert!(!r.frames[0].ticked, "a frame landed before the draft, of none, is left out");
        assert!(r.frames[1].ticked && r.frames[1].flagged);
        let other = RollDraft { id: Id::new(), ..draft.clone() };
        r.resume(Some(&other), 2);
        assert!(!r.frames[2].ticked, "twin: another draft's frame is left out");
        // The draft written back carries each frame's own place; none before every frame is read.
        r.shown = true;
        assert!(r.drafts().is_none());
        for f in ["a", "b", "c"] {
            r.read.insert(f.into(), PipelineSettings::default());
        }
        let drafts = r.drafts().unwrap();
        assert_eq!(drafts.iter().map(|(_, d)| (d.ticked, d.flagged, d.id)).collect::<Vec<_>>(), [(false, false, id), (true, true, id), (false, false, id)]);
        assert_eq!(drafts[0].1.step, "flags");
    }

    #[test]
    fn the_loupe_holds_the_pointers_pixel_at_its_centre_past_an_edge() {
        // A 1400 px display copy drawn 700 points wide; the pointer at its very left edge.
        let laid = loupe((10.0, 60.0), (10.0, 10.0, 700.0, 466.0), 1400, (1400, 932), false, 4.0, None);
        assert_eq!(laid.rings[2], 2.0 * 4.0 * 160.0 / 32.0, "an 8 px disc over a 32 px span");
        assert!((laid.rings[0] - 80.0).abs() < 1e-9, "the pointer's pixel stays at the centre: {laid:?}");
        assert!(laid.at[0] > 0.0, "the canvas shows left of the scan's edge: {laid:?}");
        // Twin: on the full-size source the span is 64 pixels, not 32, until twice the disc is wider.
        let full = loupe((360.0, 243.0), (10.0, 10.0, 700.0, 466.0), 1400, (6000, 4000), true, 2.0, None);
        assert_eq!(full.cut[2], 64);
        assert!(full.cut[2] != laid.cut[2]);
        let wide = loupe((360.0, 243.0), (10.0, 10.0, 700.0, 466.0), 1400, (6000, 4000), true, 4.0, None);
        assert_eq!(wide.cut[2], 69, "an 8 px disc is 34.3 source pixels, twice that 69");
    }

    #[test]
    fn the_rolls_settings_differ_note_counts_the_frames_shot_apart() {
        let c = CaptureExposure { shutter: 1.0 / 50.0, aperture: 5.6, iso: 100.0 };
        let mut frames = vec![frame("a"), frame("b"), frame("c")];
        frames[0].capture = Some(c);
        frames[1].capture = Some(c);
        frames[2].capture = Some(CaptureExposure { aperture: 8.0, ..c });
        assert_eq!(
            differ_text(&frames),
            "1 of 3 frames, marked, were digitised with other settings than the roll's 1/50 s, f/5.6, ISO 100. Their exposure is evened out to it; a different aperture also changes the vignetting the flat-field measured, so re-shoot them in manual for the cleanest roll."
        );
        // Twin: a frame whose EXIF reads nothing is never apart.
        frames[2].capture = None;
        assert_eq!(differ_text(&frames), "");
    }
}
