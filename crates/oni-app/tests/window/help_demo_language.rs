//! A demo showing words plays in the app's language: in French, Film Type's card plays the film
//! whose tabs read French; back in English, the English film.

use oni_app::app::{Options, Shell};
use oni_app::tutorial::Frames;
use oni_app::{PanelState, Tip};
use oni_core::{Archive, EditorStore};
use oni_i18n::Language;
use slint::platform::WindowEvent;
use slint::{ComponentHandle, LogicalPosition};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;
use crate::help_demo::clip_pixels;

/// Sweeps the panel under its tab bar until a card playing `topic` opens, whatever its words.
fn hover_topic(rig: &common::Rig, tip: &Tip, left: f32, from: f32, topic: &str) -> bool {
    for y in (0..).map(|k| from + k as f32 * 3.0).take_while(|y| *y < 790.0) {
        for x in (0..94).map(|k| left + k as f32 * 3.0) {
            rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(x, y) });
            rig.pump();
            if tip.get_shown() && tip.get_card() && tip.get_demo() == topic {
                return true;
            }
        }
    }
    false
}

/// Every picture a player shows of `clip`, once round its loop.
fn pictures(clip: &'static [u8]) -> Vec<Vec<u8>> {
    let mut frames = Frames::new(clip).expect("a GIF");
    let mut out: Vec<Vec<u8>> = Vec::new();
    while let Some((picture, _)) = frames.next() {
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
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the demos' languages are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-help-demo-language-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    common::plain_tiff(&folder.join("A.tif"), 96, 64, 0.6);
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, tip) = (w.global::<oni_app::Library>(), w.global::<Tip>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    w.global::<PanelState>().set_tab(0);
    w.global::<PanelState>().invoke_tab_picked(0);
    rig.settle();
    let (left, foot) = (w.get_panel_x() + 4.0, w.get_tabs_foot() + 2.0);
    let english = pictures(oni_app::demos::clip_in("film-type", Language::English).unwrap());
    let french = pictures(oni_app::demos::clip_in("film-type", Language::French).unwrap());

    oni_app::language::speak(Some(Language::French));
    rig.settle();
    assert!(hover_topic(&rig, &tip, left, foot, "film-type"), "the Film Type mark opens its card in French");
    let shown = playing(&rig, &tip);
    assert!(french.contains(&shown), "the card plays the French film");
    assert!(!english.contains(&shown), "not the English one");
    rig.shot("help-demo-film-type-fr");

    // Twin: back in English, the same mark plays the English film.
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(10.0, 10.0) });
    rig.pump();
    oni_app::language::speak(Some(Language::English));
    rig.settle();
    assert!(hover_topic(&rig, &tip, left, foot, "film-type"), "the Film Type mark opens its card in English");
    let shown = playing(&rig, &tip);
    assert!(english.contains(&shown) && !french.contains(&shown), "the card plays the English film");
    let _ = std::fs::remove_dir_all(&root);
}
