//! The Roll tab's marks play their own demos: Film Type's card plays the film type's clip, frame
//! after frame, and Prepared Roll's another one, so each card shows the topic it names.

use oni_app::{PanelState, Tip};
use slint::ComponentHandle;
use std::time::{Duration, Instant};

use crate::common;
use crate::help_demo::{clip_pixels, leave_help, press_help};
use crate::panel_bindings::opened;

const FILM_TYPE: &str = "What the film is: a colour negative, a slide or black and white. A slide is read as a plain RAW, its dyes never separated; Prepare the Roll balances a slide roll on a white or grey you click, its colour alone; without a click, the slides keep the camera's 5500 K.";
const PREPARED_ROLL: &str = "Every setting here is the roll's: moved on this photo, it reaches every photo prepared with the roll, each keeping its own look. A photo left out when the roll was prepared can be included, or the prepared ones moved into a roll of their own.";

/// The card's first frame, then one after `span` of watching.
fn played(rig: &common::Rig, tip: &Tip, span: Duration) -> (Vec<u8>, Vec<u8>) {
    rig.until("the demo's first frame", Duration::from_secs(5), || clip_pixels(tip).is_some());
    let first = clip_pixels(tip).unwrap();
    let start = Instant::now();
    while start.elapsed() < span {
        rig.pump();
        std::thread::sleep(Duration::from_millis(5));
    }
    (first, clip_pixels(tip).expect("still playing"))
}

pub(crate) fn the_roll_tabs_marks_play_each_its_own_demo() {
    let Some((rig, shell, root)) = opened("help-demo-roll") else { return };
    let w = &shell.window;
    let tip = w.global::<Tip>();
    w.global::<PanelState>().set_tab(0);
    w.global::<PanelState>().invoke_tab_picked(0);
    rig.settle();

    press_help(&rig, w, "Film Type");
    assert_eq!(tip.get_open(), FILM_TYPE, "the Film Type mark opens its card");
    assert_eq!(tip.get_demo(), "film-type", "the mark names its topic");
    assert!(tip.get_has_clip(), "the card holds a clip box");
    // The hand leaves its rest at 0.4 s and clicks at 1 s: past 1.5 s the picture has changed.
    let (film_type, later) = played(&rig, &tip, Duration::from_millis(1500));
    assert_ne!(film_type, later, "the demo's frame advances while the card stands");
    rig.shot("help-demo-film-type");

    // Twin: another mark of the tab plays another clip, so the card above showed its own topic's.
    leave_help(&rig, w);
    assert!(clip_pixels(&tip).is_none(), "the hand leaving its mark stops the clip");
    press_help(&rig, w, "Prepared Roll");
    assert_eq!(tip.get_open(), PREPARED_ROLL, "the Prepared Roll mark opens its card");
    assert_eq!(tip.get_demo(), "measured-roll");
    let (measured, _) = played(&rig, &tip, Duration::ZERO);
    assert_ne!(film_type, measured, "two topics show two clips");
    let _ = std::fs::remove_dir_all(&root);
}
