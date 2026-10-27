//! Prepare the Roll on slides: one window for the roll, each frame levelled on its own through it,
//! one RGB black and white, gains at rest; a negative roll still combines into one reference.

use oni_measure::key::{self, Decoded, Measurer};
use oni_measure::roll;
use oni_measure::window::in_crop;
use oni_settings::cg::Rect;
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

fn levelled(m: &Measurer, path: &str, window: Rect, before: &PipelineSettings) -> PipelineSettings {
    let start = roll::slide(before, window, None, 0.0);
    m.with(path, &start, |r, copy| key::slide_levelled(r, copy, &start)).expect("the slide reads")
}

fn ends(s: &PipelineSettings) -> (f32, f32) {
    (s.levels.red.black, s.levels.red.white)
}

fn alike(s: &PipelineSettings) -> bool {
    s.levels.red == s.levels.green && s.levels.green == s.levels.blue
}

#[test]
fn each_slide_takes_its_own_rgb_black_and_white_inside_the_roll_s_window() {
    let Some(m) = measurer() else { return eprintln!("no GPU adapter: the slide roll is not exercised") };
    let rest = PipelineSettings::rest(ConversionMode::Positive, false);
    let frames: Vec<PipelineSettings> = (0..3).map(|i| levelled(&m, &format!("f{i}"), WINDOW, &rest)).collect();
    let read: Vec<(f32, f32)> = frames.iter().map(ends).collect();
    println!("slides through the window: {read:?}");
    for s in &frames {
        assert!(alike(s) && s.gains.stops == [0.0; 3] && s.roll_reference.is_none(), "{:?} {:?}", s.levels, s.gains);
        assert_eq!(s.geometry.measure_window, Some(WINDOW));
        assert!(s.levels.red.black > rest.levels.red.black || s.levels.red.white < rest.levels.red.white, "{:?}", s.levels);
    }
    for (a, b) in [(0, 1), (1, 2), (0, 2)] {
        assert!((read[a].0 - read[b].0).abs() > 1e-3 && (read[a].1 - read[b].1).abs() > 1e-3, "frames {a} and {b}: {read:?}");
    }
    // The border outside the window moves nothing: the same frames without it read the same.
    for (i, own) in read.iter().enumerate() {
        let bare = ends(&levelled(&m, &format!("bare{i}"), WINDOW, &rest));
        assert!((bare.0 - own.0).abs() < 1e-4 && (bare.1 - own.1).abs() < 1e-4, "frame {i}: {bare:?} against {own:?}");
    }
    // Twin: a window over the whole scan takes the border in, and the white moves to it.
    let whole = ends(&levelled(&m, "f0", Rect::UNIT, &rest));
    assert!(whole.1 > read[0].1 + 0.05, "{whole:?} against {:?}", read[0]);
}

/// The Auto levels key on a frame's own settings, as the editor presses it.
fn keyed(m: &Measurer, path: &str, s: &PipelineSettings) -> PipelineSettings {
    m.with(path, s, |r, copy| key::levels(r, copy, s)).map(|b| oni_core::balance::levelled(&b.core(), s)).expect("the slide reads")
}

#[test]
fn the_key_on_a_prepared_slide_finds_prepare_s_levels_and_again_the_same() {
    let Some(m) = measurer() else { return eprintln!("no GPU adapter: the key's fixed point is not exercised") };
    let rest = PipelineSettings::rest(ConversionMode::Positive, false);
    for i in 0..3 {
        let path = format!("f{i}");
        let prepared = levelled(&m, &path, WINDOW, &rest);
        let once = keyed(&m, &path, &prepared);
        assert_eq!(once, prepared, "frame {i}: the key after Prepare moves nothing");
        assert_eq!(keyed(&m, &path, &once), once, "frame {i}: a second press moves nothing");
        // Twin: the window moved over the bright border, the key reads another histogram.
        let mut moved = prepared.clone();
        moved.geometry.measure_window = Some(Rect::UNIT);
        let other = keyed(&m, &path, &moved);
        assert!(other.levels.red.white > prepared.levels.red.white + 0.05, "frame {i}: {:?} against {:?}", ends(&other), ends(&prepared));
    }
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
