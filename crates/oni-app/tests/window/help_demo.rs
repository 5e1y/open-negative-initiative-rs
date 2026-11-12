//! A help mark naming a demo plays it on its card, frame after frame, while the hand rests on the mark;
//! the hand leaving stops the clip, the next mark plays its own, and a card closed with its panel asks no frame.

use oni_app::PanelTab;
use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::{PanelState, Tip};
use slint::ComponentHandle;
use std::time::{Duration, Instant};

use crate::common;
use crate::panel_bindings::opened;

const SHARPENING: &str = "Sharpens edges without adding colour fringing.\n\nHalo suppression keeps a sharpened edge within the light of its neighbours, clamping the overshoot that draws a bright or dark rim along it.";
const CHROMA: &str = "Smooths colour noise from the scan without touching the film grain.";
/// The help mark of the block titled `subject`, named as assistive tools read it: in English where the
/// interface is read live, as on the Mac, in the app's language where it is compiled, as in the CI.
pub(crate) fn mark(w: &oni_app::MainWindow, subject: &'static str) -> common::Placed {
    let english = format!("Help on {subject}");
    if !common::placed(w, Role::Button, &english).is_empty() {
        return common::the(w, Role::Button, &english);
    }
    common::the(w, Role::Button, &oni_i18n::tr("Help on {}").replace("{}", oni_i18n::tr(subject)))
}

/// Rests the hand on the help mark of the block titled `subject` and presses it, as the keyboard would.
pub(crate) fn press_help(rig: &common::Rig, w: &oni_app::MainWindow, subject: &'static str) {
    common::click(rig, w, mark(w, subject).middle());
    rig.settle();
}

/// Takes the hand off every help mark, onto the canvas, as a person moving on.
pub(crate) fn leave_help(rig: &common::Rig, w: &oni_app::MainWindow) {
    let position = slint::LogicalPosition::new(w.get_panel_x() - 300.0, 400.0);
    rig.window.dispatch_event(slint::platform::WindowEvent::PointerMoved { position });
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
    w.global::<PanelState>().set_tab(PanelTab::Details);
    w.global::<PanelState>().invoke_tab_picked(PanelTab::Details);
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

    // Closed with the panel, in the gallery, the card asks no frame; back, its mark plays it again.
    ctl.act(oni_app::Act::ToggleMode);
    rig.settle();
    let _ = rig.drawn();
    assert!(tip.get_open().is_empty() && clip_pixels(&tip).is_none(), "no card stands: no clip");
    watch(&rig, Duration::from_millis(600));
    assert!(clip_pixels(&tip).is_none(), "no card stands: no frame asked");
    ctl.act(oni_app::Act::ToggleMode);
    rig.settle();
    let _ = rig.drawn();
    press_help(&rig, w, "Sharpening");
    rig.until("twin: its mark again, its frames", Duration::from_secs(5), || clip_pixels(&tip).is_some());

    // The hand leaving the mark stops the clip, and another mark plays its own demo.
    leave_help(&rig, w);
    assert!(tip.get_open().is_empty() && !tip.get_has_clip() && clip_pixels(&tip).is_none(), "the hand leaving its mark stops the clip");
    press_help(&rig, w, "Chroma Denoise");
    assert_eq!(tip.get_demo(), "chroma-denoise", "the next mark names its own topic");
    rig.until("its own demo", Duration::from_secs(5), || clip_pixels(&tip).is_some_and(|p| p != later));
    let _ = std::fs::remove_dir_all(&root);
}
