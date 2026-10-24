//! The design system's board on the headless window: each component's page is drawn at its own size,
//! the page list beside it, and shot to `ONI_SHOTS` as `board-<nn>-<name>.png` for a review side by side.

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

/// How many distinct pixels the window draws now right of `from`, in physical pixels: the page alone.
fn inks(rig: &common::Rig, from: usize) -> usize {
    let (w, drawn) = rig.drawn();
    drawn.chunks(w).flat_map(|row| row[from.min(w)..].iter().map(|p| p.0)).collect::<std::collections::HashSet<u16>>().len()
}

pub(crate) fn the_board_shows_every_component_in_every_state() {
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
    assert_eq!(families.len(), 14, "one page a base component");
    let from = (board.get_page_x() * SCALE).ceil() as usize;
    let mut lowest = f32::MAX;
    for (i, name) in families.iter().enumerate() {
        board.set_family(i as i32);
        // A page is made at the next draw, so its size is read after one.
        rig.drawn();
        let (w, h) = (board.get_content_width(), board.get_content_height());
        assert!(w > 0.0 && h > 0.0, "{name} draws a page");
        lowest = lowest.min(board.get_page_height());
        rig.window.set_size(PhysicalSize::new((w.min(CAP.0) * SCALE).ceil() as u32, (h.min(CAP.1) * SCALE).ceil() as u32));
        rig.settle();
        let path = rig.shot(&format!("board-{i:02}-{name}"));
        println!("shot {} ({w:.0} × {h:.0} pt)", path.display());
        assert!(inks(&rig, from) > INKS, "{name}'s page is drawn, not blank");
    }
    // Twin: a page the board lacks lays nothing and draws the bare ground beside the list, which both
    // checks see.
    board.set_family(99);
    rig.drawn();
    assert!(board.get_page_height() < lowest, "an unknown page lays nothing");
    rig.window.set_size(PhysicalSize::new(1600, 800));
    rig.settle();
    assert!(inks(&rig, from) <= INKS, "a blank page is seen as blank");

    // A help mark's note is a card: the clip's 4:3 place above, the note under it.
    board.global::<oni_app::Tips>().invoke_note(true, "Contrast bends the print into an S about a middle grey, its ends held.".into(), 400.0, 100.0);
    rig.settle();
    assert!(board.global::<oni_app::Tip>().get_card(), "the note is a card");
    println!("shot {}", rig.shot("help-card").display());
    board.global::<oni_app::Tips>().invoke_hover(true, "Zoom in (⌘+)".into(), 400.0, 100.0);
    assert!(!board.global::<oni_app::Tip>().get_card(), "twin: a tooltip stays a line");
}
