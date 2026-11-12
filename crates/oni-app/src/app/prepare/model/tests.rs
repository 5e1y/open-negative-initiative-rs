//! The model's checks, each against Swift's self-check where one exists.

use super::*;
use std::sync::{Mutex, MutexGuard};

fn frame(path: &str) -> Frame {
    Frame::new(path, "")
}

/// A grouping holds its triplets in one process-wide map, so the checks that group take turns.
fn holding() -> MutexGuard<'static, ()> {
    static HOLD: Mutex<()> = Mutex::new(());
    HOLD.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
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
    // A step behind is done once it holds its value; a skipped one or one not walked never is.
    assert!(done(Step::FlatField, Step::Base, Capture::Camera, n, &flat, false) && done(Step::Film, Step::Base, Capture::Camera, n, &flat, false));
    assert!(!done(Step::FlatField, Step::Base, Capture::Camera, n, &FlatField::default(), false), "twin: a flat-field skipped is not done");
    assert!(!done(Step::Base, Step::Flags, Capture::Camera, n, &flat, false) && done(Step::Base, Step::Flags, Capture::Camera, n, &flat, true));
    assert!(!done(Step::FlatField, Step::Base, Capture::Scanner, n, &flat, false), "a step the roll does not walk is not done");
    assert!(!done(Step::Base, Step::Base, Capture::Camera, n, &flat, true), "twin: the current step is not behind");
    // The keycap names the walk's end as the bar's key does, and NEXT on every other step, picked or not.
    assert_eq!((key(Step::Window), key(Step::Flags), key(Step::Base)), ("PREPARE ROLL", "NEXT", "NEXT"));
    assert_ne!(key(Step::FlatField), "Skip", "twin: no step turns its keycap into Skip");
    assert_eq!(left_out_text(1), "1 frame of this folder, prepared with another roll, is left out.");
    assert_eq!(left_out_text(2), "2 frames of this folder, prepared with another roll, are left out.");
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
        pass_offset: None,
        flat_field: None,
        base: Some(DraftBase { path: "b".into(), x: 0.1, y: 0.2, radius: 3.0 }),
        window: None,
        ticked: true,
        flagged: true,
    };
    let known = passes::Known::new();
    r.resume(None, "a", &known);
    assert!(r.frames[0].ticked, "with no draft yet a frame keeps its tick");
    r.resume(Some(&draft), "b", &known);
    assert_eq!((r.step, r.film, r.capture), (Step::Flags, ConversionMode::Negative, Capture::Scanner));
    assert_eq!(r.base, Some(BasePick { path: "b".into(), unit: (0.1, 0.2), radius: 3.0 }));
    assert!(!r.frames[0].ticked, "a frame landed before the draft, of none, is left out");
    assert!(r.frames[1].ticked && r.frames[1].flagged);
    let other = RollDraft { id: Id::new(), ..draft.clone() };
    r.resume(Some(&other), "c", &known);
    assert!(!r.frames[2].ticked, "twin: another draft's frame is left out");
    // The draft written back carries each frame's own place; none before every frame is read, the
    // step then written once the last one lands.
    r.shown = true;
    assert!(r.step_drafts().is_none());
    for f in ["a", "b"] {
        r.read.insert(f.into(), PipelineSettings::default());
        assert!(!r.draft_landed(), "twin: a frame still unread keeps the step waiting");
    }
    r.read.insert("c".into(), PipelineSettings::default());
    assert!(r.draft_landed(), "the last frame read writes the step's draft");
    assert!(!r.draft_landed(), "once");
    let drafts = r.drafts().unwrap();
    assert_eq!(drafts.iter().map(|(_, d)| d.as_ref().map(|d| (d.ticked, d.flagged, d.id))).collect::<Vec<_>>(), [Some((false, false, id)), Some((true, true, id)), Some((false, false, id))]);
    assert_eq!(drafts[0].1.as_ref().map(|d| d.step.as_str()), Some("flags"));
}

/// A draft walked on RGB Passes rides its leads, its passes cleared; resumed, the roll's files are grouped
/// again whatever order they land in, each lead keeping its own place.
#[test]
fn a_roll_resumed_on_rgb_passes_groups_its_files_and_keeps_each_leads_place() {
    let _turn = holding();
    let files = ["/r/a1", "/r/a2", "/r/a3", "/r/b1", "/r/b2", "/r/b3"].map(frame).to_vec();
    let fresh = || Roll { shown: true, sources: files.clone(), frames: files.clone(), base_frame: 4, ..Roll::default() };
    let id = Id::new();
    let draft = RollDraft {
        id,
        step: "flags".into(),
        film: ConversionMode::Negative,
        capture: "rgbPasses".into(),
        pass_order: Some(PassOrder::Rgb),
        pass_offset: Some(0),
        flat_field: None,
        base: None,
        window: None,
        ticked: true,
        flagged: true,
    };
    let known = passes::Known::new();
    let mut r = fresh();
    // The files land in their order: a lead with its draft, its two passes with none, the next lead.
    assert!(r.resume(Some(&draft), "/r/a1", &known), "the first draft found sets the roll");
    r.resume(None, "/r/a2", &known);
    r.resume(None, "/r/a3", &known);
    r.resume(Some(&RollDraft { ticked: false, flagged: false, ..draft.clone() }), "/r/b1", &known);
    assert_eq!((r.capture, r.pass_order), (Capture::RgbPasses, PassOrder::Rgb));
    assert_eq!(r.frame_paths(), ["/r/a1", "/r/b1"], "the roll's frames are its leads");
    assert!(r.grouping.is_some() && r.frames.iter().all(|f| f.passes.is_some()));
    assert_eq!(oni_library::rgb_passes::named_by("/r/a1").map(|p| p.green), Some("a2".into()), "the measure finds each lead's passes");
    assert!(r.frames[0].ticked && r.frames[0].flagged, "the lead landed before the grouping keeps its flag");
    assert!(!r.frames[1].ticked, "the lead landed after it takes its own draft's place");
    assert_eq!(r.base_frame, 0, "the base step's frame is counted among the leads");
    // The draft written back names the grouping it was made on, on the leads; the passes hold none.
    r.read = r.sources.iter().map(|f| (f.path.clone(), PipelineSettings::default())).collect();
    let written = r.drafts().unwrap();
    let laid: Vec<_> = written.iter().map(|(p, d)| (p.as_str(), d.as_ref().map(|d| (d.capture.as_str(), d.pass_offset)))).collect();
    let lead = Some(("rgbPasses", Some(0)));
    assert_eq!(laid, [("/r/a1", lead), ("/r/a2", None), ("/r/a3", None), ("/r/b1", lead), ("/r/b2", None), ("/r/b3", None)]);
    // Twin: a draft walked on Camera leaves the six files as six frames.
    let mut camera = fresh();
    camera.resume(Some(&RollDraft { capture: "camera".into(), ..draft.clone() }), "/r/a1", &known);
    assert_eq!((camera.frames.len(), camera.grouping.is_some()), (6, false));
    oni_library::rgb_passes::release();
}

/// Drafts laid on disk as `save_draft` writes them, a `None` clearing the file's draft.
fn saved(disk: &mut HashMap<String, RollDraft>, drafts: Drafts) {
    for (path, draft) in drafts {
        match draft {
            Some(d) => disk.insert(path, d),
            None => disk.remove(&path),
        };
    }
}

/// The roll entered again: every file landing in its order with the draft its sidecar holds.
fn resumed(files: &[Frame], disk: &HashMap<String, RollDraft>) -> Roll {
    let mut r = Roll { shown: true, sources: files.to_vec(), frames: files.to_vec(), ..Roll::default() };
    for f in files {
        r.resume(disk.get(&f.path), &f.path, &passes::Known::new());
    }
    r
}

/// A roll regrouped after a draft was written resumes on the grouping it was left on, whichever
/// file lands first: a file no longer a frame keeps no draft of the grouping before.
#[test]
fn a_roll_regrouped_after_its_draft_resumes_on_its_last_grouping() {
    let _turn = holding();
    let files = ["/r/a1", "/r/a2", "/r/a3", "/r/b1", "/r/b2", "/r/b3", "/r/c1"].map(frame).to_vec();
    let known = passes::Known::new();
    let walked = |capture, offset| {
        let mut r = Roll { shown: true, step: Step::Flags, sources: files.clone(), frames: files.clone(), ..Roll::default() };
        r.read = files.iter().map(|f| (f.path.clone(), PipelineSettings::default())).collect();
        r.group_as(capture, PassOrder::Rgb, offset, &known);
        r
    };
    // On RGB Passes from the first file, then Shift by One File: the leads move to a2 and b2.
    let mut r = walked(Capture::RgbPasses, 0);
    let mut disk = HashMap::new();
    saved(&mut disk, r.drafts().unwrap());
    let before = r.drafts().unwrap();
    r.group_as(Capture::RgbPasses, PassOrder::Rgb, 1, &known);
    saved(&mut disk, r.drafts().unwrap());
    let back = resumed(&files, &disk);
    assert_eq!((back.frame_paths(), back.pass_offset), (vec!["/r/a2".to_string(), "/r/b2".into()], 1));
    assert!(back.frames.iter().all(|f| f.ticked), "each lead takes its own draft's place");
    // Twin: the shifted drafts written on the frames alone leave a1 its first draft, which wins.
    let mut stale: HashMap<String, RollDraft> = before.into_iter().filter_map(|(p, d)| Some((p, d?))).collect();
    stale.extend(r.drafts().unwrap().into_iter().filter_map(|(p, d)| Some((p, d?))));
    assert_eq!(resumed(&files, &stale).frame_paths(), ["/r/a1", "/r/b1"]);
    // Walked on Camera, then switched to RGB Passes shifted by one: a1 keeps no Camera draft.
    let mut r = walked(Capture::Camera, 0);
    let mut disk = HashMap::new();
    saved(&mut disk, r.drafts().unwrap());
    r.group_as(Capture::RgbPasses, PassOrder::Rgb, 1, &known);
    saved(&mut disk, r.drafts().unwrap());
    let back = resumed(&files, &disk);
    assert_eq!((back.capture, back.frame_paths()), (Capture::RgbPasses, vec!["/r/a2".to_string(), "/r/b2".into()]));
    oni_library::rgb_passes::release();
}

/// A regrouping, by pass order or by capture, keeps the tick and the flag each frame holds by hand.
#[test]
fn a_regrouping_keeps_each_frames_tick_and_flag() {
    let _turn = holding();
    let files = ["/r/a1", "/r/a2", "/r/a3", "/r/b1", "/r/b2", "/r/b3"].map(frame).to_vec();
    let known = passes::Known::new();
    let mut r = Roll { shown: true, sources: files.clone(), frames: files.clone(), ..Roll::default() };
    r.group_as(Capture::RgbPasses, PassOrder::Rgb, 0, &known);
    flagging(&mut r.frames, 0);
    r.frames[1].ticked = false;
    r.group_as(Capture::RgbPasses, PassOrder::Bgr, 0, &known);
    let places = |r: &Roll| r.frames.iter().map(|f| (f.ticked, f.flagged)).collect::<Vec<_>>();
    assert_eq!((r.frame_paths(), places(&r)), (vec!["/r/a1".to_string(), "/r/b1".into()], vec![(true, true), (false, false)]));
    r.group_as(Capture::Camera, PassOrder::Bgr, 0, &known);
    assert_eq!(places(&r)[..4], [(true, true), (true, false), (true, false), (false, false)], "back to six frames, a1 still flagged and b1 out");
    // Twin: the same regrouping laid from the sources, as the files came, loses both.
    r.frames = r.sources.clone();
    r.group_as(Capture::RgbPasses, PassOrder::Rgb, 0, &known);
    assert_eq!(places(&r), [(true, false), (true, false)]);
    oni_library::rgb_passes::release();
}

#[test]
fn a_roll_holds_a_choice_once_a_base_is_picked_a_frame_flagged_or_its_window_drawn() {
    let mut r = Roll { frames: vec![frame("a"), frame("b")], ..Roll::default() };
    assert!(!r.holds_choices(), "a roll walked by NEXT alone holds nothing to lose");
    r.base = Some(BasePick { path: "a".into(), unit: (0.1, 0.1), radius: 2.0 });
    assert!(r.holds_choices());
    r.base = None;
    flagging(&mut r.frames, 1);
    assert!(r.holds_choices());
    flagging(&mut r.frames, 1);
    r.window.x += 0.05;
    assert!(r.holds_choices(), "a window drawn away from the one the roll brought");
    // Twin: drawn back where it was laid, the window holds no choice.
    r.window = r.laid_window;
    assert!(!r.holds_choices());
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
        "1 of 3 frames, marked, were digitised with other settings than the roll's 1/50 s, f/5.6, ISO 100. Their exposure is evened out to it. Shoot the light panel at f/5.6, as most of the roll; a frame at another aperture may keep slightly uneven corners."
    );
    // Twin: a frame apart at the roll's aperture leaves the light panel as it is.
    frames[2].capture = Some(CaptureExposure { shutter: 1.0 / 25.0, ..c });
    assert_eq!(
        differ_text(&frames),
        "1 of 3 frames, marked, were digitised with other settings than the roll's 1/50 s, f/5.6, ISO 100. Their exposure is evened out to it: nothing to do."
    );
    // Twin: a frame whose EXIF reads nothing is never apart.
    frames[2].capture = None;
    assert_eq!(differ_text(&frames), "");
}
