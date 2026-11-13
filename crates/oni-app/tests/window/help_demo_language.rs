//! A demo showing words plays in the app's language: in French, Film Type's card plays the film
//! whose tabs read French; back in English, the English film.

use oni_app::PanelTab;
use oni_app::tutorial::Frames;
use oni_app::{PanelState, Tip};
use oni_i18n::Language;
use slint::ComponentHandle;
use std::time::Duration;

use crate::common;
use crate::help_demo::{clip_pixels, press_help};
use crate::panel_bindings::opened;

/// Every picture a player shows of `clip`, once round its loop.
fn pictures(clip: &'static [u8]) -> Vec<Vec<u8>> {
    let mut frames = Frames::new(clip).expect("a GIF");
    let mut out: Vec<Vec<u8>> = Vec::new();
    while let Some((picture, _, _)) = frames.next() {
        if out.first().is_some_and(|p| p == picture) || out.len() > 400 {
            break;
        }
        out.push(picture.to_vec());
    }
    out
}

/// The card's picture once the film is on its way.
fn playing(rig: &common::Rig, tip: &Tip) -> Vec<u8> {
    rig.until("the demo's first frame", Duration::from_secs(5), || clip_pixels(tip).is_some());
    clip_pixels(tip).expect("playing")
}

pub(crate) fn a_worded_demo_plays_in_the_app_s_language() {
    let Some((rig, shell, root)) = opened("help-demo-language") else { return };
    let w = &shell.window;
    let tip = w.global::<Tip>();
    w.global::<PanelState>().set_tab(PanelTab::Roll);
    w.global::<PanelState>().invoke_tab_picked(PanelTab::Roll);
    rig.settle();
    let english = pictures(oni_app::demos::clip_in("film-type", Language::English).unwrap());
    let french = pictures(oni_app::demos::clip_in("film-type", Language::French).unwrap());

    oni_app::language::speak(Some(Language::French));
    rig.settle();
    press_help(&rig, w, "Film Type");
    assert_eq!(tip.get_demo(), "film-type", "the Film Type mark opens its card in French");
    let shown = playing(&rig, &tip);
    assert!(french.contains(&shown), "the card plays the French film");
    assert!(!english.contains(&shown), "not the English one");
    rig.shot("help-demo-film-type-fr");

    // Twin: back in English, the card standing again on the same topic plays the English film; a frame
    // drawn lays each view out, as the window does after each change.
    oni_app::language::speak(Some(Language::English));
    let ctl = shell.ctl.clone();
    for _ in 0..2 {
        ctl.act(oni_app::Act::ToggleMode);
        rig.settle();
        let _ = rig.drawn();
    }
    // The interface's own words stay English in a check's build; translated, the note changed and closed it.
    if common::placed(w, i_slint_backend_testing::AccessibleRole::Groupbox, "Help on Film Type").is_empty() {
        press_help(&rig, w, "Film Type");
    }
    assert_eq!(tip.get_demo(), "film-type", "the Film Type card stands in English");
    rig.until("the English film", Duration::from_secs(5), || clip_pixels(&tip).is_some_and(|p| english.contains(&p)));
    let _ = std::fs::remove_dir_all(&root);
}
