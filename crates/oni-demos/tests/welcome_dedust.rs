//! The welcome's De-dust scene on a synthetic dusty negative: DETECT rings what the app's reading found,
//! REMOVE marks each speck and heals it; twins: an idle hand shows nothing, a clean frame offers nothing.

use std::path::Path;

use oni_demos::stage::SCALE;
use oni_demos::welcome::dedust::{self, Ask, Shot};
use oni_settings::levels::Levels;
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{ConversionMode, PipelineSettings};

/// Six specks a few pixels wide, apart enough for their numbers, in the frame's pixels.
const SPECKS: [(usize, usize); 6] = [(780, 420), (820, 500), (780, 580), (820, 660), (780, 740), (820, 820)];
const SIDE: usize = 6;

/// A negative of a grainy grey, past its window only where a speck lies, with its sidecar.
fn negative(path: &Path, dusty: bool) {
    oni_fixtures::tiff::pixels_tiff(path, 1600, 1200, |x, y| {
        let speck = dusty && SPECKS.iter().any(|&(sx, sy)| (sx..sx + SIDE).contains(&x) && (sy..sy + SIDE).contains(&y));
        let v = if speck { 0.0 } else { 0.8 + 0.1 * ((x as f32 * 0.13).sin() * (y as f32 * 0.17).cos()) };
        [v, v, v]
    });
    let mut settings = PipelineSettings::rest(ConversionMode::Negative, false);
    let window = Levels::with_ends(-0.5, 1.0);
    (settings.levels.red, settings.levels.green, settings.levels.blue) = (window, window, window);
    sidecar::write(&settings, path, WriteOptions::default()).unwrap();
}

/// A point of the strip on the frame, in pixels.
fn on_frame(shot: &Shot, p: (f32, f32)) -> (f32, f32) {
    let k = SCALE / shot.view.zoom;
    (shot.view.x as f32 + p.0 * k, shot.view.y as f32 + p.1 * k)
}

/// The frame's pixel at a point of the strip, in its film pixels.
fn pixel(shot: &Shot, frame: usize, p: (f32, f32)) -> [i32; 3] {
    let f = &shot.frames[frame];
    let (x, y) = ((shot.region.x as f32 + p.0 * SCALE) as u32, (shot.region.y as f32 + p.1 * SCALE) as u32);
    let at = ((y * f.width + x) * 4) as usize;
    [0, 1, 2].map(|c| i32::from(f.pixels[at + c]))
}

/// A speck's middle in the strip's points.
fn in_strip(shot: &Shot, (sx, sy): (usize, usize)) -> (f32, f32) {
    let k = shot.view.zoom / SCALE;
    let middle = |s: usize, from: u32| (s as f32 + SIDE as f32 / 2.0 - from as f32) * k;
    (middle(sx, shot.view.x), middle(sy, shot.view.y))
}

#[test]
fn detect_rings_the_readings_specks_and_remove_heals_each_under_its_number() {
    if oni_app::render::Device::open().is_err() {
        return eprintln!("no GPU: the De-dust scene is not filmed");
    }
    let dir = std::env::temp_dir().join(format!("oni-welcome-dedust-check-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (dusty, clean) = (dir.join("Dusty.tif"), dir.join("Clean.tif"));
    negative(&dusty, true);
    negative(&clean, false);

    let shot = dedust::shoot(&dusty, Ask::default()).unwrap();
    assert!(shot.detected && shot.removed, "the hand's clicks reached both keys");
    let ringed: Vec<_> = shot.seen.iter().filter(|s| !s.rings.is_empty()).collect();
    assert!(!ringed.is_empty(), "rings show after DETECT");
    for seen in &ringed {
        assert_eq!((seen.rings.len(), seen.found), (SPECKS.len(), SPECKS.len() as i32), "every ring the pane counts, every speck laid");
        for &ring in &seen.rings {
            let (x, y) = on_frame(&shot, ring);
            let on = SPECKS.iter().any(|&(sx, sy)| (x - (sx as f32 + 3.0)).abs() <= 2.0 && (y - (sy as f32 + 3.0)).abs() <= 2.0);
            assert!(on, "the ring at ({x}, {y}) lies on a speck laid");
        }
    }
    let last = shot.seen.last().unwrap();
    let mut numbers: Vec<i32> = last.marks.iter().map(|m| m.0).collect();
    numbers.sort();
    assert_eq!(numbers, last.rows, "each tile's number has its mark");
    assert_eq!((last.rows.len(), last.pending, last.found), (SPECKS.len(), 0, 0), "every patch landed, the reading gone");
    // Each speck stood out from the grey around it, and no longer does.
    let end = shot.frames.len() - 1;
    for &speck in &SPECKS {
        let at = in_strip(&shot, speck);
        let beside = (at.0 - 12.0, at.1);
        let off = |frame: usize| (0..3).map(|c| (pixel(&shot, frame, at)[c] - pixel(&shot, frame, beside)[c]).abs()).max().unwrap();
        assert!(off(0) > 40 && off(end) < off(0) / 3, "the speck at {speck:?} healed: {} then {}", off(0), off(end));
    }

    // Twin: the same frame and hand, never pressing, shows no ring, no count and no tile.
    let idle = dedust::shoot(&dusty, Ask { still_hand: true, ..Ask::default() }).unwrap();
    assert!(!idle.detected && idle.seen.iter().all(|s| s.rings.is_empty() && s.found == 0 && s.rows.is_empty()));
    assert_eq!(idle.frames[0].pixels, idle.frames.last().unwrap().pixels, "nothing changed");

    // Twin: a clean frame read finds nothing, and REMOVE stays out of the hand's reach.
    let none = dedust::shoot(&clean, Ask { rehearsal: true, ..Ask::default() }).unwrap();
    assert!(none.detected && !none.removed, "DETECT pressed, REMOVE out of reach");
    assert!(none.seen.iter().all(|s| s.rings.is_empty() && s.found == 0 && s.rows.is_empty()));
    let _ = std::fs::remove_dir_all(&dir);
}
