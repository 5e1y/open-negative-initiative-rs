//! The Roll tab's marks play their own demos: Film Type's card plays the film type's clip, frame
//! after frame, and Measured Roll's another one, so each card shows the topic it names.

use oni_app::app::{Options, Shell};
use oni_app::{PanelState, Tip};
use oni_core::{Archive, EditorStore};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, LogicalPosition};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::common;

const FILM_TYPE: &str = "What the film is: a colour negative, a slide or black and white. A slide is read as a plain RAW: its roll is never measured nor its dyes separated; Prepare the Roll places each slide's own black and white inside the roll's measuring frame, and Auto Levels places them again on a click.";
const MEASURED_ROLL: &str = "Every setting here is the roll's: moved on this photo, it reaches every photo measured with it, each keeping its own look. A photo of the folder left out of that measurement can join it, or the measured ones can become a roll of their own.";

/// Sweeps the panel under its tab bar until the card saying `note` opens.
fn hover(rig: &common::Rig, tip: &Tip, left: f32, from: f32, note: &str) -> bool {
    for y in (0..).map(|k| from + k as f32 * 3.0).take_while(|y| *y < 790.0) {
        for x in (0..94).map(|k| left + k as f32 * 3.0) {
            rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(x, y) });
            rig.pump();
            if tip.get_shown() && tip.get_card() && tip.get_text() == note {
                return true;
            }
        }
    }
    false
}

fn clip_pixels(tip: &Tip) -> Option<Vec<u8>> {
    tip.get_clip().to_rgba8().map(|b| b.as_bytes().to_vec())
}

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
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the Roll tab's demos are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-help-demo-roll-{}", std::process::id()));
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

    assert!(hover(&rig, &tip, left, foot, FILM_TYPE), "the Film Type mark opens its card");
    assert_eq!(tip.get_demo(), "film-type", "the mark names its topic");
    assert!(tip.get_has_clip(), "the card holds a clip box");
    // The hand leaves its rest at 0.4 s and clicks at 1 s: past 1.5 s the picture has changed.
    let (film_type, later) = played(&rig, &tip, Duration::from_millis(1500));
    assert_ne!(film_type, later, "the demo's frame advances while the card is open");
    rig.shot("help-demo-film-type");

    // Twin: another mark of the tab plays another clip, so the card above showed its own topic's.
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(10.0, 10.0) });
    rig.pump();
    assert!(clip_pixels(&tip).is_none(), "leaving the mark stops the clip");
    assert!(hover(&rig, &tip, left, foot, MEASURED_ROLL), "the Measured Roll mark opens its card");
    assert_eq!(tip.get_demo(), "measured-roll");
    let (measured, _) = played(&rig, &tip, Duration::ZERO);
    assert_ne!(film_type, measured, "two topics show two clips");
    let _ = std::fs::remove_dir_all(&root);
}
