//! A help mark naming a demo plays it on its card, frame after frame, while the card is open;
//! leaving the mark stops the clip, and the next mark plays its own.

use oni_app::app::{Options, Shell};
use oni_app::{PanelState, Tip};
use oni_core::{Archive, EditorStore};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, LogicalPosition};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::common;

const SHARPENING: &str = "Sharpens edges without adding color fringing.";
const CHROMA: &str = "Smooths color noise from the scan without touching the film grain.";

/// Sweeps the panel under its tab bar until the card saying `note` opens.
pub(crate) fn hover_note(rig: &common::Rig, tip: &Tip, left: f32, from: f32, note: &str) -> bool {
    let mut y = from;
    while y < 790.0 {
        let mut x = left;
        while x < left + 280.0 {
            rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(x, y) });
            rig.pump();
            if tip.get_shown() && tip.get_card() && tip.get_text() == note {
                return true;
            }
            x += 3.0;
        }
        y += 3.0;
    }
    false
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
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the help demo is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-help-demo-{}", std::process::id()));
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
    w.global::<PanelState>().set_tab(3);
    w.global::<PanelState>().invoke_tab_picked(3);
    rig.settle();
    let (left, foot) = (w.get_panel_x() + 4.0, w.get_tabs_foot() + 2.0);

    assert!(hover_note(&rig, &tip, left, foot, SHARPENING), "the Sharpening mark opens its card");
    assert_eq!(tip.get_demo(), "sharpening", "the mark names its topic");
    assert!(tip.get_has_clip(), "the card holds a clip box");
    rig.until("the demo's first frame", Duration::from_secs(5), || clip_pixels(&tip).is_some());
    let first = clip_pixels(&tip).unwrap();
    // The hand rests half a second before it moves: past that, the picture must have changed.
    watch(&rig, Duration::from_millis(1500));
    let later = clip_pixels(&tip).expect("still playing");
    assert_ne!(first, later, "the demo's frame advances while the card is open");
    let with_clip = w.get_tip_box();
    assert!(with_clip.height > with_clip.width, "the square clip stands above the note: {with_clip:?}");
    rig.shot("help-demo-sharpening");

    // Twin: the clip stopped with the card it left, and another mark plays its own demo.
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(10.0, 10.0) });
    rig.pump();
    assert!(!tip.get_has_clip() && clip_pixels(&tip).is_none(), "leaving the mark stops the clip");
    assert!(hover_note(&rig, &tip, left, foot, CHROMA), "the Chroma Denoise mark opens its card");
    rig.pump();
    assert_eq!(tip.get_demo(), "chroma-denoise", "the next mark names its own topic");
    rig.until("its own demo", Duration::from_secs(5), || clip_pixels(&tip).is_some_and(|p| p != later));
    let _ = std::fs::remove_dir_all(&root);
}
