//! A help mark naming a demo plays it on its card, frame after frame, while the card stands; its mark
//! pressed again stops the clip, the next mark plays its own, and a card gone with its panel asks no frame.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::{PanelState, Tip};
use slint::ComponentHandle;
use std::time::{Duration, Instant};

use crate::common;
use crate::panel_bindings::opened;

/// The help mark of the block titled `subject`, named as assistive tools read it; a check's build
/// keeps the interface's words English.
pub(crate) fn mark(w: &oni_app::MainWindow, subject: &str) -> common::Placed {
    common::the(w, Role::Button, &format!("Help on {subject}"))
}

/// Presses the help mark of the block titled `subject`.
pub(crate) fn press_help(rig: &common::Rig, w: &oni_app::MainWindow, subject: &str) {
    common::click(rig, w, mark(w, subject).middle());
    rig.settle();
}

pub(crate) fn clip_pixels(tip: &Tip) -> Option<Vec<u8>> {
    tip.get_clip().to_rgba8().map(|b| b.as_bytes().to_vec())
}

/// Pumps the event loop for `span`, as a person watching the card.
pub(crate) fn watch(rig: &common::Rig, span: Duration) {
    let start = Instant::now();
    while start.elapsed() < span {
        rig.pump();
        std::thread::sleep(Duration::from_millis(5));
    }
}

pub(crate) fn a_help_mark_with_a_demo_plays_it_and_leaving_stops_it() {
    let Some((rig, shell, root)) = opened("help-demo") else { return };
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let tip = w.global::<Tip>();
    w.global::<PanelState>().set_tab(3);
    w.global::<PanelState>().invoke_tab_picked(3);
    rig.settle();

    press_help(&rig, w, "Sharpening");
    assert_eq!(tip.get_demo(), "sharpening", "the mark names its topic");
    assert!(tip.get_has_clip(), "the card holds a clip box");
    rig.until("the demo's first frame", Duration::from_secs(5), || clip_pixels(&tip).is_some());
    let first = clip_pixels(&tip).unwrap();
    // The hand rests half a second before it moves: past that, the picture must have changed.
    watch(&rig, Duration::from_millis(1500));
    let later = clip_pixels(&tip).expect("still playing");
    assert_ne!(first, later, "the demo's frame advances while the card stands");
    rig.shot("help-demo-sharpening");

    // Gone with the panel, in the gallery, the card asks no frame; back, it plays on. A frame drawn
    // lays each view out, as the window does after each change.
    ctl.act(oni_app::Act::ToggleMode);
    rig.settle();
    let _ = rig.drawn();
    let held = clip_pixels(&tip);
    watch(&rig, Duration::from_millis(600));
    assert_eq!(held, clip_pixels(&tip), "no card stands: the clip waits");
    ctl.act(oni_app::Act::ToggleMode);
    rig.settle();
    let _ = rig.drawn();
    watch(&rig, Duration::from_millis(600));
    assert_ne!(held, clip_pixels(&tip), "twin: the card back, its frames advance again");

    // Pressed again, the mark stops the clip, and another mark plays its own demo.
    press_help(&rig, w, "Sharpening");
    assert!(tip.get_open().is_empty() && !tip.get_has_clip() && clip_pixels(&tip).is_none(), "its mark again stops the clip");
    press_help(&rig, w, "Chroma Denoise");
    assert_eq!(tip.get_demo(), "chroma-denoise", "the next mark names its own topic");
    rig.until("its own demo", Duration::from_secs(5), || clip_pixels(&tip).is_some_and(|p| p != later));
    let _ = std::fs::remove_dir_all(&root);
}
