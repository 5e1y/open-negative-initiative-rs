//! The welcome's third and fourth scenes, held at the start, the middle and the end of their loops:
//! each settings card of the edits scene changes while every photo's pixels stay put, and the fixes
//! scene ticks and counts. Each assertion carries a twin that looks where the change is and must see it.

use oni_app::{Help, MainWindow, SceneClock};
use slint::platform::software_renderer::Rgb565Pixel;
use slint::{ComponentHandle, PhysicalSize};
use std::path::PathBuf;

use crate::common;

type Frame = Vec<Rgb565Pixel>;
type Box4 = (usize, usize, usize, usize);

/// The smallest box holding every pixel that differs between two frames of `width`, if any does.
fn changes(a: &Frame, b: &Frame, width: usize, within: Box4) -> Option<Box4> {
    let (x0, y0, x1, y1) = within;
    let mut found: Option<Box4> = None;
    for y in y0..y1 {
        for x in x0..x1 {
            if a[y * width + x].0 != b[y * width + x].0 {
                found = Some(match found {
                    None => (x, y, x + 1, y + 1),
                    Some((l, t, r, d)) => (l.min(x), t.min(y), r.max(x + 1), d.max(y + 1)),
                });
            }
        }
    }
    found
}

pub(crate) fn the_edits_scene_turns_its_cards_and_never_its_photos_and_the_fixes_tick() {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    let window = MainWindow::new().unwrap();
    oni_app::keycaps::serve(&window);
    let help = window.global::<Help>();
    help.set_release("1.6".into());
    help.set_welcome_line("V1.6.0".into());
    help.set_welcome_credit("".into());
    help.set_welcome_page(3);
    help.set_welcome_shown(true);
    let (w, h) = (1400u32, 1000u32);
    rig.window.set_size(PhysicalSize::new(w, h));
    window.show().unwrap();
    rig.settle();
    let clock = window.global::<SceneClock>();
    let width = w as usize;
    let whole = (0, 0, width, h as usize);
    // A moment of the scene, drawn and kept.
    let at = |held: f32, name: &str| -> Frame {
        clock.set_held(held);
        rig.pump();
        rig.pump();
        println!("shot {}", rig.shot(name).display());
        rig.drawn().1
    };

    let start = at(0.06, "welcome-edits-start");
    let middle = at(0.42, "welcome-edits-middle");
    let end = at(1.0, "welcome-edits-end");
    let again = at(1.0, "welcome-edits-end-again");

    // The cards: what differs from the start to the end, five new cards and a status line.
    let cards = changes(&start, &end, width, whole).expect("the cards turn between the start and the end");
    assert!(changes(&end, &again, width, whole).is_none(), "a held moment draws the same twice");
    assert!(changes(&start, &middle, width, whole).is_some() && changes(&middle, &end, width, whole).is_some(), "the middle is neither the start nor the end");
    // The photos lie above the cards: 36 px of link and 54 px of photo, across the row of five.
    let (left, top) = (cards.0 - 12, cards.1 - 90);
    let photos = (left, top, left + 496, top + 54);
    for (name, frame) in [("middle", &middle), ("end", &end)] {
        assert!(changes(&start, frame, width, photos).is_none(), "a photo's pixels stay put to the {name}");
    }
    // Twin: the same box moved down onto the cards sees them change.
    let wrong = (left, top + 90, left + 496, top + 144);
    assert!(changes(&start, &end, width, wrong).is_some(), "a box on the cards must see the turn");

    // The fixes: the boxes tick one after another and the count rises.
    help.set_welcome_page(4);
    rig.settle();
    let first = at(0.06, "welcome-fixes-start");
    let half = at(0.4, "welcome-fixes-middle");
    let last = at(1.0, "welcome-fixes-end");
    let ticked = changes(&first, &last, width, whole).expect("the fixes change between the start and the end");
    let so_far = changes(&first, &half, width, whole).expect("the fixes change by the middle");
    assert!(so_far.3 < ticked.3 || so_far.1 > ticked.1 || so_far != ticked, "the middle has ticked part of the list");
    assert!(changes(&half, &last, width, whole).is_some(), "the end ticks what the middle has not");
    // Twin: a frame held at the end twice shows no change, so the diff above is the clock's.
    assert!(changes(&last, &at(1.0, "welcome-fixes-end-again"), width, whole).is_none());
    clock.set_held(-1.0);
}
