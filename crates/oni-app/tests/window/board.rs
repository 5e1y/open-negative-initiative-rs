//! The design system's state board on the headless window: each family's page is drawn at its own
//! size and shot to `ONI_SHOTS` as `board-<nn>-<name>.png`, for a review of every state side by side.

use oni_app::Board;
use slint::platform::WindowEvent;
use slint::{ComponentHandle, Model, PhysicalSize};
use std::path::PathBuf;

use crate::common;

/// A page is shot whole up to this many logical points a side, past which it is cut.
const CAP: (f32, f32) = (2400.0, 4000.0);
const SCALE: f32 = 2.0;
/// Fewer distinct pixels than this is a blank page; any drawn page antialiases far more.
const INKS: usize = 64;

/// How many distinct pixels the window draws now.
fn inks(rig: &common::Rig) -> usize {
    let (_, drawn) = rig.drawn();
    drawn.iter().map(|p| p.0).collect::<std::collections::HashSet<u16>>().len()
}

pub(crate) fn the_board_shows_every_family_in_every_state() {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let board = Board::new().unwrap();
    // The board reads English whatever the machine speaks, as the checks do.
    oni_app::language::speak(None);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: SCALE });
    rig.window.set_size(PhysicalSize::new(3200, 2000));
    board.show().unwrap();
    let families: Vec<String> = board.get_families().iter().map(|s| s.to_string()).collect();
    assert_eq!(families.len(), 13, "one page a family");
    let mut lowest = f32::MAX;
    for (i, name) in families.iter().enumerate() {
        board.set_family(i as i32);
        // A page is made at the next draw, so its size is read after one.
        rig.drawn();
        let (w, h) = (board.get_content_width(), board.get_content_height());
        assert!(w > 0.0 && h > 0.0, "{name} draws a page");
        lowest = lowest.min(h);
        rig.window.set_size(PhysicalSize::new((w.min(CAP.0) * SCALE).ceil() as u32, (h.min(CAP.1) * SCALE).ceil() as u32));
        rig.settle();
        let path = rig.shot(&format!("board-{i:02}-{name}"));
        println!("shot {} ({w:.0} × {h:.0} pt)", path.display());
        assert!(inks(&rig) > INKS, "{name}'s page is drawn, not blank");
    }
    // Twin: a family the board lacks lays no page and draws the bare ground, which both checks see.
    board.set_family(99);
    rig.drawn();
    assert!(board.get_content_height() < lowest, "an unknown family lays nothing");
    rig.window.set_size(PhysicalSize::new(800, 800));
    rig.settle();
    assert!(inks(&rig) <= INKS, "a blank page is seen as blank");
}
