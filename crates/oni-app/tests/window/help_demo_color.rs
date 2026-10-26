//! The Color tab's marks play their own demos: Lighting's card plays the lighting clip, frame
//! after frame, and Effects' card plays another.

use oni_app::app::{Options, Shell};
use oni_app::{PanelState, Tip};
use oni_core::{Archive, EditorStore};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, LogicalPosition};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;
use crate::help_demo::{clip_pixels, hover_note, watch};

const LIGHTING: &str = "The negative back to light along a straight line: the black point on pure black, the white on pure white. Every slider brightens to the right; Black point and White point move the ends of the scan itself.";
const EFFECTS: &str = "Contrast bends the print into an S about a middle grey, its ends held. Vignette darkens the corners in stops, without shifting their color; below zero it lightens them. Chrome lays a saturation curve over everything, greying the ends and keeping color in the middle.";

pub(crate) fn the_color_tabs_marks_play_their_own_demos() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the Color tab's demos are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-help-demo-color-{}", std::process::id()));
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
    w.global::<PanelState>().set_tab(2);
    w.global::<PanelState>().invoke_tab_picked(2);
    rig.settle();
    let (left, foot) = (w.get_panel_x() + 4.0, w.get_tabs_foot() + 2.0);

    assert!(hover_note(&rig, &tip, left, foot, LIGHTING), "the Lighting mark opens its card");
    assert_eq!(tip.get_demo(), "lighting", "the mark names its topic");
    assert!(tip.get_has_clip(), "the card holds a clip box");
    rig.until("the demo's first frame", Duration::from_secs(5), || clip_pixels(&tip).is_some());
    let first = clip_pixels(&tip).unwrap();
    // The hand reaches the first knob within a second: past that, the picture must have changed.
    watch(&rig, Duration::from_millis(1500));
    assert_ne!(Some(&first), clip_pixels(&tip).as_ref(), "the demo's frame advances while the card is open");
    rig.shot("help-demo-lighting");

    // Twin: another mark of the tab names its own topic and plays another clip.
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(10.0, 10.0) });
    rig.pump();
    // Effects lies under the fold: the panel is scrolled by the wheel to bring it up.
    let over = LogicalPosition::new(left + 120.0, 500.0);
    for _ in 0..4 {
        rig.window.dispatch_event(WindowEvent::PointerScrolled { position: over, delta_x: 0.0, delta_y: -150.0 });
        rig.pump();
    }
    rig.settle();
    assert!(hover_note(&rig, &tip, left, foot, EFFECTS), "the Effects mark opens its card");
    assert_eq!(tip.get_demo(), "effects");
    rig.until("the other demo's first frame", Duration::from_secs(5), || clip_pixels(&tip).is_some());
    assert_ne!(Some(first), clip_pixels(&tip), "the Effects card shows its own clip, its knobs elsewhere");
    let _ = std::fs::remove_dir_all(&root);
}
