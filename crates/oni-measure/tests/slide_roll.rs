//! Prepare the Roll on slides: the roll's gains from a white picked on one slide, its windows at rest,
//! one window laid on every slide; a negative roll still combines into one reference.

use oni_measure::key::{self, Decoded, Measurer};
use oni_measure::truncate::Stage;
use oni_measure::{pipette, roll};
use oni_measure::window::in_crop;
use oni_settings::cg::{Point, Rect};
use oni_settings::frame::RgbPasses;
use oni_settings::{ConversionMode, PipelineSettings};

const SIDE: (u32, u32) = (300, 200);
const EXPOSURES: [f32; 3] = [0.5, 1.0, 1.8];
/// Inside the bright border every frame carries, so a window there never sees it.
const WINDOW: Rect = Rect { x: 0.25, y: 0.25, width: 0.5, height: 0.5 };

/// A warm frame: a ramp at `exposure` inside a near-white border a fifth of each side wide.
fn frame(exposure: f32, bordered: bool) -> Vec<[f32; 4]> {
    let (w, h) = SIDE;
    (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as f32 / w as f32, (i / w) as f32 / h as f32);
            let edge = !(0.2..0.8).contains(&x) || !(0.2..0.8).contains(&y);
            if bordered && edge {
                return [0.97, 0.97, 0.97, 1.0];
            }
            let v = exposure * (0.04 + 0.4 * (x + y) / 2.0);
            [v, 0.8 * v, 0.6 * v, 1.0]
        })
        .collect()
}

/// Each path names its frame: `f<i>` bordered, `bare<i>` without the border.
fn measurer() -> Option<Measurer> {
    let graph = oni_graph::Graph::new().ok()?;
    Some(Measurer::new(
        graph,
        Box::new(|path, _| {
            let name = path.to_string_lossy();
            let i: usize = name.trim_start_matches(char::is_alphabetic).parse().unwrap();
            let pixels = frame(EXPOSURES[i], name.starts_with('f'));
            Ok(Decoded { width: SIDE.0, height: SIDE.1, pixels, full_width: f64::from(SIDE.0) })
        }),
        Box::new(|_: &std::path::Path, s: &PipelineSettings| format!("{:?}", s.mode)),
    ))
}

/// The gains a click at `unit` on the frame's scan gives the roll, the frame read uncropped at rest.
fn picked(m: &Measurer, path: &str, unit: (f64, f64)) -> Option<[f32; 3]> {
    let bare = PipelineSettings::rest(ConversionMode::Positive, false);
    let read = m.base(path, &bare, Point::new(unit.0, unit.1), 4.0);
    read.and_then(|f| oni_measure::slide_gains::white(f, ConversionMode::Positive))
}

fn alike(s: &PipelineSettings) -> bool {
    s.levels.red == s.levels.green && s.levels.green == s.levels.blue
}

#[test]
fn a_white_picked_on_a_slide_gives_the_roll_its_gains() {
    let Some(m) = measurer() else { return eprintln!("no GPU adapter: the slide roll is not exercised") };
    // The near-white border is neutral: the roll's gains stay at rest.
    let border = picked(&m, "f1", (0.1, 0.1)).expect("the border reads");
    assert!(border.iter().all(|g| g.abs() < 0.02), "{border:?}");
    // Twin: a click on the warm ramp brings it to grey, red down and blue up, the gains' mean held.
    let ramp = picked(&m, "f1", (0.5, 0.5)).expect("the ramp reads");
    assert!(ramp[0] < -0.05 && ramp[2] > 0.05 && ramp.iter().sum::<f32>().abs() < 1e-4, "{ramp:?}");
    // The same click on a darker exposure of the same ramp asks the same gains: the light's colour, not its level.
    let darker = picked(&m, "f0", (0.5, 0.5)).expect("the darker ramp reads");
    assert!((0..3).all(|c| (darker[c] - ramp[c]).abs() < 0.05), "{darker:?} against {ramp:?}");
}

#[test]
fn a_negative_roll_of_the_same_frames_still_combines_into_one_reference_and_aligns_its_gains() {
    let Some(m) = measurer() else { return eprintln!("no GPU adapter: the negative roll is not exercised") };
    let film = ConversionMode::Negative;
    let rest = PipelineSettings::rest(film, false);
    let jobs: Vec<roll::Job> = (0..3)
        .map(|i| roll::Job { path: format!("f{i}"), before: rest.clone(), start: roll::start(&rest, film, WINDOW, None, 0.0), flagged: false })
        .collect();
    let written = roll::measure_roll(&jobs, film, None, |j| m.with(&j.path, &j.start, |r, copy| key::roll_measured(r, copy, &j.path, &j.start))).expect("the roll reads");
    let ids: Vec<_> = written.frames.iter().map(|(_, _, s)| s.roll_reference.as_ref().map(|r| r.id)).collect();
    assert!(ids.iter().all(|id| *id == Some(written.reference.id)), "{ids:?}");
    assert!(written.frames.iter().all(|(_, _, s)| s.levels == written.frames[0].2.levels), "one roll, one set of windows");
    // Twin: the negative's gains align its channels, where a slide's stay at rest.
    assert_ne!(written.reference.stops, [0.0; 3]);
    assert!(!alike(&written.frames[0].2), "{:?}", written.frames[0].2.levels);
}

#[test]
fn the_roll_s_window_is_laid_whole_on_frames_of_different_crops() {
    let crops = [Rect::new(0.0, 0.0, 1.0, 1.0), Rect::new(0.1, 0.05, 0.8, 0.9)];
    let laid: Vec<PipelineSettings> = crops
        .iter()
        .map(|crop| {
            let mut before = PipelineSettings::rest(ConversionMode::Positive, true);
            before.geometry.crop = *crop;
            roll::slide(&before, WINDOW, None, 0.0)
        })
        .collect();
    assert!(laid.iter().all(|s| s.geometry.measure_window == Some(WINDOW)), "the same window in the scan's fractions");
    assert_eq!(laid.iter().map(|s| s.geometry.crop).collect::<Vec<_>>(), crops, "each frame keeps its crop");
    // Twin: seen from inside each crop the window differs, so a window copied in crop space would move.
    assert_ne!(in_crop(&laid[0].geometry), in_crop(&laid[1].geometry));
}

/// A uniform frame decoded as the app decodes it: `grey` reads grey on any settings; any other file
/// reads grey on its passes composed or at a slide's 5500 K, warm alone at a negative's 5000 K.
fn decoding() -> Option<Measurer> {
    let graph = oni_graph::Graph::new().ok()?;
    Some(Measurer::new(
        graph,
        Box::new(|path, s: &PipelineSettings| {
            let grey = path.to_string_lossy() == "grey" || s.rgb_passes.is_some() || s.mode == ConversionMode::Positive;
            let pixel = if grey { [0.6, 0.6, 0.6, 1.0] } else { [0.6, 0.45, 0.3, 1.0] };
            Ok(Decoded { width: SIDE.0, height: SIDE.1, pixels: vec![pixel; (SIDE.0 * SIDE.1) as usize], full_width: f64::from(SIDE.0) })
        }),
        Box::new(|_: &std::path::Path, s: &PipelineSettings| format!("{:?} {}", s.mode, s.rgb_passes.is_some())),
    ))
}

/// The base read as it once was: decoded on one settings, read on another.
fn split(m: &Measurer, path: &str, decoded: &PipelineSettings, start: &PipelineSettings) -> Option<[f32; 3]> {
    let centre = Point::new(0.5, 0.5);
    m.with(path, decoded, |r, copy| {
        let plane = r.plane(copy, &key::bare(start), Stage::Levels)?;
        pipette::read(&plane, centre, start.mode, 4.0, pipette::RETAINED_SPACE, pipette::RETAINED_STATISTIC).map(|x| x.fractions)
    })
}

fn apart(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|c| (a[c] - b[c]).abs()).fold(0.0, f32::max)
}

#[test]
fn a_white_picked_on_a_slide_never_prepared_is_read_on_the_slides_own_decode() {
    let Some(m) = decoding() else { return eprintln!("no GPU adapter: the white's decode is not exercised") };
    // A slide never prepared: its sidecar still says negative, decoded at 5000 K.
    let before = PipelineSettings::rest(ConversionMode::Negative, false);
    let start = roll::slide(&before, WINDOW, None, 0.0);
    let white = m.base("slide", &start, Point::new(0.5, 0.5), 4.0).expect("the white reads");
    let gains = oni_measure::slide_gains::white(white, ConversionMode::Positive).expect("gains");
    assert!(gains.iter().all(|g| g.abs() < 0.02), "a neutral slide asks no gains: {gains:?}");
    assert!(apart(white, m.base("grey", &start, Point::new(0.5, 0.5), 4.0).unwrap()) < 1e-4);
    // Twin: decoded on the sidecar and read on the slide's start, the white takes the 5000 K cast.
    let cast = split(&m, "slide", &before, &start).expect("the white reads");
    let wrong = oni_measure::slide_gains::white(cast, ConversionMode::Positive).expect("gains");
    assert!(wrong.iter().any(|g| g.abs() > 0.05), "{wrong:?}");
}

#[test]
fn a_base_picked_on_a_lead_is_read_on_its_passes_composed() {
    let Some(m) = decoding() else { return eprintln!("no GPU adapter: the base's decode is not exercised") };
    let film = ConversionMode::Negative;
    let before = PipelineSettings::rest(film, false);
    // The measure lays the lead's passes on its start, which its sidecar does not yet name.
    let mut start = roll::start(&before, film, WINDOW, None, 0.0);
    start.rgb_passes = Some(RgbPasses { red: "lead".into(), green: "g".into(), blue: "b".into(), registration: None });
    let composed = m.base("grey", &roll::start(&before, film, WINDOW, None, 0.0), Point::new(0.5, 0.5), 4.0).expect("the base reads");
    let base = m.base("lead", &start, Point::new(0.5, 0.5), 4.0).expect("the base reads");
    assert!(apart(base, composed) < 1e-4, "{base:?} against {composed:?}");
    // Twin: decoded on the lead alone, as its sidecar names no passes, the base is another colour.
    let alone = split(&m, "lead", &before, &start).expect("the base reads");
    assert!(apart(alone, composed) > 0.01, "{alone:?} against {composed:?}");
}
