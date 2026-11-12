//! Prepare's walk by the hand: NEXT alone goes on, past a step left empty, which keeps no mark; a pick on the
//! film base goes straight on, there to see and take again from its step; a window with nothing to measure waits.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::app::{Options, Shell};
use oni_app::{Bar, Clicks, Library, Preparing};
use oni_core::{Archive, EditorStore};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, LogicalPosition, Model};
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use crate::common;

pub(crate) fn next_alone_passes_an_empty_step_and_a_pick_on_the_film_goes_straight_on() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: Prepare's walk is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-prepare-walk-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let folder = root.join("Scans");
    for (name, tone) in [("A.tif", 0.8), ("B.tif", 0.5), ("C.tif", 0.3)] {
        common::graded_tiff(&folder.join(name), 240, 160, tone);
    }
    let rig = common::rig(std::env::temp_dir());
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), require_roll: true, ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let p = w.global::<Preparing>();
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(60), || ctl.state.borrow().read_done());
    let first = oni_app::app::tiles(w).into_iter().min_by_key(|t| t.index).unwrap().index;
    w.global::<Library>().invoke_tile_clicked(first, false, false);
    rig.pump();
    w.global::<Bar>().invoke_act(oni_app::Act::PrepareRoll);
    rig.until("the roll's surface", Duration::from_secs(30), || p.get_shown() && p.get_cells().row_count() == 3);
    rig.until("every frame read", Duration::from_secs(30), || p.get_capture() == 0);
    let presses = Rc::new(Cell::new(0));
    let counted = presses.clone();
    w.global::<Clicks>().on_press(move || counted.set(counted.get() + 1));
    let press = |role: Role, name: &str| {
        rig.settle();
        common::click(&rig, w, common::the(w, role, name).middle());
        rig.pump();
    };
    let marks = || (0..5).map(|i| p.get_done().row_data(i).unwrap()).collect::<Vec<bool>>();
    let no_skip = || assert!(common::placed(w, Role::Button, "Skip").is_empty() && common::placed(w, Role::Button, "NEXT").len() == 1, "NEXT alone goes on");

    // A DSLR's roll walks its flat-field: left empty, NEXT is live and goes on, no mark on the step.
    press(Role::Tab, "DSLR Scan");
    press(Role::Button, "NEXT");
    assert_eq!(p.get_step(), 1);
    rig.settle();
    assert!(!p.get_flat_on() && p.get_action_enabled(), "an empty flat-field may go on");
    no_skip();
    press(Role::Button, "NEXT");
    assert_eq!(p.get_step(), 2, "NEXT passes the empty flat-field");
    assert_eq!(marks(), [true, false, false, false, false], "the flat-field passed empty holds no mark");

    // The base, its scan read: a pick outside it gives no value, and the step stays.
    rig.until("the scan read", Duration::from_secs(60), || p.get_scan_ready());
    rig.settle();
    no_skip();
    assert!(p.get_action_enabled() && !p.get_has_base(), "an empty base may go on");
    let keys = presses.get();
    p.invoke_base_clicked(p.get_scan_x() - 1.0, p.get_scan_y() - 1.0);
    rig.pump();
    assert_eq!((p.get_step(), p.get_has_base(), presses.get()), (2, false, keys), "a pick of no value stays, silent");
    // NEXT passes it empty; Back returns.
    press(Role::Button, "NEXT");
    assert_eq!((p.get_step(), p.get_has_base()), (3, false), "NEXT passes the empty base");
    assert_eq!(marks(), [true, false, false, false, false], "a base passed empty holds no mark");
    press(Role::Button, "Back");
    assert_eq!(p.get_step(), 2);

    // A pick by the hand: a key's click, and straight on to the next step, its value in every draft.
    rig.until("the scan again", Duration::from_secs(60), || p.get_scan_ready());
    rig.settle();
    let on_scan = |fx: f32, fy: f32| (p.get_base_origin_x() + p.get_scan_x() + p.get_scan_width() * fx, p.get_base_origin_y() + p.get_scan_y() + p.get_scan_height() * fy);
    let hover = |at: (f32, f32)| {
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(at.0, at.1) });
        rig.pump();
    };
    let at = on_scan(0.05, 0.1);
    hover(at);
    let keys = presses.get();
    assert!(p.get_loupe_shown() && p.get_step() == 2, "twin: the hand over the scan picks nothing yet");
    common::click(&rig, w, at);
    rig.pump();
    assert_eq!((p.get_step(), p.get_has_base(), presses.get()), (3, true, keys + 1), "a pick goes straight on, with a click");
    assert_eq!(marks(), [true, false, true, false, false], "the base behind holds its mark");
    let drafted = || oni_settings::prepare::read(std::path::Path::new(&oni_library::paths::canonical(&folder))).pop();
    rig.until("the pick in the draft", Duration::from_secs(10), || drafted().is_some_and(|d| d.step == "flags" && d.base.is_some() && d.frames.len() == 3));

    // The pick stays reachable: its step, by the bar's tab, shows it, the loupe gone with the hand until it comes over the scan.
    press(Role::Tab, "Base");
    assert_eq!(p.get_step(), 2);
    assert!(p.get_ring_shown() && p.get_base_note() == "On A.tif", "the pick shown on its step: {}", p.get_base_note());
    assert!(!p.get_loupe_shown(), "the hand left with the step");
    rig.settle();
    let ring = p.get_ring_x();
    let other = on_scan(0.5, 0.5);
    hover(other);
    assert!(p.get_loupe_shown(), "twin: the hand back over the scan brings the loupe");
    // A pick again lays the new one and goes on again.
    let keys = presses.get();
    common::click(&rig, w, other);
    rig.pump();
    assert_eq!((p.get_step(), presses.get()), (3, keys + 1), "picked again, it goes on again");
    p.invoke_step_picked(2);
    rig.pump();
    assert!(p.get_ring_x() != ring, "the new pick replaced the old: {} {ring}", p.get_ring_x());

    // A step that may not go on stays so: every frame flagged, no window to measure, the keycap dark.
    p.invoke_step_picked(3);
    rig.pump();
    (0..3).for_each(|i| p.invoke_sheet_clicked(i));
    p.invoke_step_picked(4);
    rig.pump();
    assert_eq!((p.get_action_title().as_str(), p.get_action_enabled()), ("PREPARE ROLL", false), "nothing to measure, nothing to end on");
    p.invoke_clear_flags();
    rig.pump();
    assert!(p.get_action_enabled(), "twin: a frame to measure lights it");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
