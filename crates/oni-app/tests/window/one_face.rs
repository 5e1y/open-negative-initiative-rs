//! One face: no Scanner block on the Colour tab; a Scanner photo opens on its legacy print, switched off and on; Settings
//! reads the filters as C, M and Y, the same values as Temperature and Tint. Each state shot at 1512 x 982.

use i_slint_backend_testing::{AccessibleRole as Role, ElementHandle};
use oni_app::app::{Options, Shell};
use oni_app::{Library, MainWindow, PanelState, PanelTab};
use oni_core::{Archive, EditorStore};
use oni_settings::{Document, LegacyMachine, PipelineSettings};
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

/// The Colour tab's blocks on a digital photo, every one the panel lays.
const ONE_FACE: [&str; 9] = ["Film", "Colour", "More Color Settings", "Lighting", "Effects", "Film Effects", "Curves", "Colour Mixer", "Saturation Curve"];
/// What only the Scanner face laid: its machine, its keypad's blocks, its face's line and the bar's switch.
const SCANNER: [&str; 5] = ["Machine", "Density", "Look", "Scanner: a minilab's density and colour, in steps", "Pro: every setting by hand, the place to start"];

/// A format-45 sidecar of a photo the Scanner face printed on the Frontier, its keys moved.
fn scanner_sidecar(original: &Path) {
    let mut v = serde_json::to_value(Document::of(PipelineSettings::rest(oni_settings::ConversionMode::Negative, false))).unwrap();
    v["version"] = 45.into();
    let s = &mut v["settings"];
    s["face"] = "scanner".into();
    s["scannerMachine"] = "frontier".into();
    s["scanner"] = serde_json::json!({"d": 3, "c": -2, "m": 0, "y": 4, "w": 1});
    std::fs::write(oni_settings::sidecar::url_for(original), serde_json::to_vec_pretty(&v).unwrap()).unwrap();
}

fn launch(name: &str, seed: impl FnOnce(&Path)) -> Option<(common::Rig, Shell, PathBuf)> {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => {
            eprintln!("{e}: the one face is not exercised");
            return None;
        }
    };
    let root = std::env::temp_dir().join(format!("oni-app-one-face-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    common::plain_tiff(&folder.join("A.tif"), 960, 640, 0.35);
    common::plain_tiff(&folder.join("B.tif"), 960, 640, 0.35);
    seed(&folder.join("A.tif"));
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let shell = Shell::new(store, engine, Options { support, caches, picker: Box::new(|| None), ..Options::default() }).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 2.0 });
    rig.window.set_size(slint::PhysicalSize::new(2 * 1512, 2 * 982));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    let lib = shell.window.global::<Library>();
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    let panel = shell.window.global::<PanelState>();
    panel.set_tab(PanelTab::Color);
    panel.invoke_tab_picked(PanelTab::Color);
    rig.settle();
    Some((rig, shell, root))
}

fn shown(w: &MainWindow, role: Role, name: &str) -> bool {
    !common::placed(w, role, name).is_empty()
}

fn slider_value(w: &MainWindow, name: &str) -> String {
    let mut found = ElementHandle::find_by_accessible_label(w, name).filter(|e| e.accessible_role() == Some(Role::Slider));
    found.next().unwrap_or_else(|| panic!("a {name} slider shown")).accessible_value().unwrap_or_default().to_string()
}

pub(crate) fn the_colour_tab_lays_one_face_and_nothing_of_the_scanner() {
    let Some((rig, shell, root)) = launch("blocks", |_| {}) else { return };
    let w = &shell.window;
    // Tall enough for every block to stand laid; a digital photo lays the most.
    w.global::<PanelState>().invoke_film_type_picked(3);
    rig.settle();
    println!("shot {}", rig.shot("after-colour").display());
    rig.window.set_size(slint::PhysicalSize::new(2 * 1512, 2 * 3200));
    rig.settle();
    let laid: Vec<&str> = ONE_FACE.into_iter().filter(|t| shown(w, Role::Text, t)).collect();
    assert_eq!(laid, ONE_FACE, "every block of the one face is laid, the probe seeing each title");
    let remnants: Vec<&str> = SCANNER.into_iter().filter(|t| shown(w, Role::Text, t)).collect();
    assert!(remnants.is_empty(), "the Scanner face's blocks: {remnants:?}");
    assert!(!shown(w, Role::Tab, "Scanner") && !shown(w, Role::Tab, "Frontier") && !shown(w, Role::Tab, "Pro"), "no face, no machine");
    // Twin: the same probe sees a title the tab does lay that only a photo with a legacy print would lack.
    assert!(shown(w, Role::Text, "Lighting") && !shown(w, Role::Text, "Scanner ONI look (legacy)"));
    let _ = std::fs::remove_dir_all(&root);
}

pub(crate) fn a_photo_edited_on_the_scanner_opens_on_its_legacy_print_and_switches_it() {
    let Some((rig, shell, root)) = launch("legacy", scanner_sidecar) else { return };
    let (w, ctl) = (&shell.window, shell.ctl.clone());
    let settings = || ctl.state.borrow().store().settings().clone();
    let legacy = settings().legacy_print.expect("the migrated photo keeps its legacy print");
    assert_eq!((legacy.machine, legacy.d, legacy.c, legacy.y, legacy.w, legacy.laid), (LegacyMachine::Frontier, 3, -2, 4, 1, true));
    rig.window.set_size(slint::PhysicalSize::new(2 * 1512, 2 * 3200));
    rig.settle();
    let title = "Scanner Frontier look (legacy)";
    assert!(shown(w, Role::Text, "D +1.5 · C −1.0 · M 0.0 · Y +2.0 · TEMP +0.5"), "its keys as the keypad read them");
    let toggle = common::the(w, Role::Checkbox, title);
    // Off, the photo prints as its sliders say and keeps the legacy print to lay again.
    let renders = ctl.state.borrow().renders();
    common::click(&rig, w, toggle.middle());
    rig.until("the render without it", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    assert!(settings().legacy_print.is_some_and(|l| !l.laid), "kept, not laid");
    assert!(shown(w, Role::Checkbox, title), "its switch stays to lay it again");
    common::click(&rig, w, common::the(w, Role::Checkbox, title).middle());
    rig.settle();
    assert!(settings().legacy_print.is_some_and(|l| l.laid));
    rig.window.set_size(slint::PhysicalSize::new(2 * 1512, 2 * 982));
    rig.settle();
    // The pane scrolled to Lighting's end, where the print's rows stand.
    let over = slint::LogicalPosition::new(1360.0, 700.0);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: over });
    rig.window.dispatch_event(WindowEvent::PointerScrolled { position: over, delta_x: 0.0, delta_y: -560.0 });
    rig.settle();
    println!("shot {}", rig.shot("after-colour-legacy-print").display());
    // Twin: B, never edited on the Scanner face, shows no legacy print.
    let renders = ctl.state.borrow().renders();
    w.global::<Library>().invoke_tile_opened(1);
    rig.until("B rendered", Duration::from_secs(120), || ctl.state.borrow().renders() > renders);
    rig.window.set_size(slint::PhysicalSize::new(2 * 1512, 2 * 3200));
    rig.settle();
    assert!(settings().legacy_print.is_none() && !shown(w, Role::Checkbox, title) && shown(w, Role::Text, "Lighting"));
    drop(ctl);
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}

pub(crate) fn settings_read_the_filters_as_c_m_and_y_with_the_same_values() {
    let Some((rig, shell, root)) = launch("cmy", |_| {}) else { return };
    let (w, ctl) = (&shell.window, shell.ctl.clone());
    let store = w.global::<oni_app::Store>();
    store.invoke_typed(oni_app::fields::Field::Temperature.index(), 100);
    rig.settle();
    let filters = ctl.state.borrow().store().settings().print.filters();
    assert!(shown(w, Role::Slider, "Temperature") && !shown(w, Role::Slider, "C"));
    ctl.act(oni_app::Act::FiltrationCmy);
    rig.settle();
    assert!(!shown(w, Role::Slider, "Temperature") && !shown(w, Role::Slider, "Tint"), "C, M and Y in their place");
    // A stop of Temperature is the red's half a stop up and the blue's down: thirty CC of C over Y, M halfway.
    assert_eq!([slider_value(w, "C"), slider_value(w, "M"), slider_value(w, "Y")], ["30 CC", "15 CC", "0 CC"]);
    assert_eq!(ctl.state.borrow().store().settings().print.filters(), filters, "switching moves no value");
    println!("shot {}", rig.shot("after-colour-cmy").display());
    ctl.act(oni_app::Act::FiltrationCmy);
    rig.settle();
    // Twin: back on Temperature and Tint the same values read as they were typed.
    assert!(shown(w, Role::Slider, "Temperature") && !shown(w, Role::Slider, "C"));
    assert!(slider_value(w, "Temperature").contains("1.00"), "{}", slider_value(w, "Temperature"));
    drop(store);
    drop(ctl);
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
