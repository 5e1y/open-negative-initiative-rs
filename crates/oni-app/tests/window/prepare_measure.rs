//! The measure's screens: each frame's channels and negative drawn as it lands and morphed into the
//! next one's, the roll's histogram summed frame after frame with its ends, every screen dark until
//! the first lands, a measure stopped and started again included.

use oni_app::{Library, Preparing};
use slint::{ComponentHandle, Model};
use std::collections::HashSet;
use std::time::{Duration, Instant};

use crate::common;

const FRAMES: usize = 8;

fn edges(p: &Preparing) -> String {
    p.get_trace_curves().iter().map(|c| c.edge.to_string()).collect()
}

fn roll_edges(p: &Preparing) -> String {
    p.get_roll_curves().iter().map(|c| c.edge.to_string()).collect()
}

/// Every screen dark: no channel, no echo, no negative, no roll.
fn dark(p: &Preparing) -> bool {
    !p.get_traced()
        && p.get_trace_curves().row_count() == 0
        && p.get_trace_trail().row_count() == 0
        && p.get_negative_a().size().width == 0
        && p.get_negative_b().size().width == 0
        && p.get_roll_curves().row_count() == 0
        && p.get_roll_voted() == 0
}

pub(crate) fn each_frame_measured_morphs_its_screens_the_roll_sums_and_a_measure_opens_dark() {
    let root = common::Root::new("prepare-measure");
    let folder = root.scans();
    for i in 0..FRAMES {
        common::graded_tiff(&folder.join(format!("F{i}.tif")), 1500, 1000, 0.2 + 0.7 * i as f32 / FRAMES as f32);
    }
    let Some(app) = common::launch(root).with(|o| o.require_roll = true).show() else { return };
    let common::App { rig, shell, .. } = &app;
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, p) = (w.global::<Library>(), w.global::<Preparing>());
    app.read(&folder);
    rig.settle();
    let first = oni_app::app::tiles(w).iter().map(|t| t.index).min().unwrap();
    lib.invoke_tile_clicked(first, false, false);
    rig.pump();
    ctl.act(oni_app::Act::PrepareRoll);
    rig.until("the roll's surface", Duration::from_secs(10), || p.get_shown() && p.get_cells().row_count() == FRAMES);
    p.invoke_step_picked(4);
    rig.until("every frame placed", Duration::from_secs(20), || p.get_readouts().row_data(4).is_some_and(|r| r.ends_with(&format!(" / {FRAMES}"))));
    rig.settle();

    // Started: every screen is dark until a frame lands, which only the event loop brings.
    p.invoke_advance();
    assert!(p.get_measuring());
    assert!(dark(&p), "the first measure opens on dark screens");
    // Drawn once, so the screens are laid and tell their sizes.
    let _ = rig.drawn();
    let (mut names, mut drawn, mut rolls) = (HashSet::new(), HashSet::new(), HashSet::new());
    let mut voted = Vec::new();
    let start = Instant::now();
    while p.get_measuring() && (names.len() < 3 || voted.last().copied().unwrap_or(0) < 3) {
        assert!(start.elapsed() < Duration::from_secs(120), "timed out waiting for the frames' histograms");
        rig.pump();
        if p.get_traced() {
            assert_eq!(p.get_trace_curves().row_count(), 3, "red, green and blue, each on its screen");
            assert!(p.get_trace_name().starts_with('F') && p.get_trace_count().ends_with(&format!("/{FRAMES:02}")), "{}", p.get_trace_count());
            assert!(p.get_negative_a().size().width > 0 || p.get_negative_b().size().width > 0, "the frame's negative shows");
            names.insert(p.get_trace_name().to_string());
            let e = edges(&p);
            assert!(e.starts_with("M 0 "), "the curves are laid in the screens' points: {e:.40}");
            drawn.insert(e);
        }
        if p.get_roll_curves().row_count() > 0 {
            assert_eq!(p.get_roll_curves().row_count(), 3);
            assert!(p.get_roll_curves().iter().all(|c| c.black < c.white), "each channel's black stands left of its white");
            rolls.insert(roll_edges(&p));
            voted.push(p.get_roll_voted());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    println!("shot {}", rig.shot("prepare-measuring").display());
    // Twin: the histogram the checks below find gone was there, and moved between landings.
    assert!(names.len() >= 2, "the histogram follows the frames as they land: {names:?}");
    assert!(drawn.len() > names.len(), "a landing morphs over several frames, never jumps: {} drawn for {} frames", drawn.len(), names.len());
    assert!(p.get_trace_trail().row_count() >= 3, "the frames before trail behind, on each channel's screen");
    // The roll sums every frame read: its count only grows, and its histogram moves as it does.
    // Twin: a roll that showed the last frame alone would stay at one frame.
    assert!(voted.windows(2).all(|w| w[0] <= w[1]), "the roll never shrinks back: {voted:?}");
    assert!(voted.last().copied().unwrap_or(0) >= 2, "the roll gathers frames: {voted:?}");
    assert!(rolls.len() >= 2, "the roll's histogram moves as frames join it");

    // Stopped, then started again: no histogram of the measure before shows.
    p.invoke_cancel();
    rig.until("the measure stopped", Duration::from_secs(60), || !p.get_measuring());
    assert!(p.get_shown(), "a stopped measure keeps the roll open");
    assert!(dark(&p), "a stopped measure leaves its screens dark");
    p.invoke_advance();
    assert!(p.get_measuring());
    assert!(dark(&p), "the measure again opens on dark screens, not stale ones");
    rig.until("the measure landed", Duration::from_secs(180), || !p.get_shown() || w.global::<oni_app::Advice>().get_shown());
    assert!(dark(&p), "a landed measure leaves nothing drawn for the next one");
}
