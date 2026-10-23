//! The editor's routes on the headless window: a frame never balanced opens with its measuring
//! window armed, Return and the crop's way out, De-dust leaving the Lab, the brush kept, Space and
//! ⌘A beside a photo, and a preset's automatic balance that asks nothing.

use oni_app::app::{Options, Shell};
use oni_app::preferences::{self, Preferences};
use oni_app::{App, Canvas, Choice, Framing, Lab, Library, Menus, PanelState, Presets};
use oni_core::presets::{self, Preset, Tint};
use oni_core::{Archive, EditorStore};
use oni_settings::{ConversionMode, PipelineSettings};
use slint::ComponentHandle;
use slint::platform::{Key, WindowEvent};
use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

pub(crate) fn the_panels_routes_and_the_measuring_window_behave_as_swifts() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the editor's routes are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-routes-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for (name, tone) in [("A.tif", 0.8), ("B.tif", 0.6), ("C.tif", 0.5), ("D.tif", 0.4)] {
        common::graded_tiff(&folder.join(name), 600, 400, tone);
    }
    let names: Vec<String> = ["A.tif", "B.tif", "C.tif", "D.tif"].iter().map(|n| oni_library::paths::canonical(&folder.join(n))).collect();
    // The brush the last session left, and a preset naming the balance key.
    Preferences::load(&support).set(preferences::BRUSH_RADIUS, 40.0.into()).unwrap();
    let auto: BTreeSet<String> = BTreeSet::from([oni_core::parameters::AUTO_BALANCE_NAME.to_string()]);
    presets::save(&Preset::new("Auto", auto, Tint::Amber, PipelineSettings::rest(ConversionMode::Negative, false)), &presets::directory(&support)).unwrap();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support: support.clone(), caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, panel, framing, canvas) = (w.global::<Library>(), w.global::<PanelState>(), w.global::<Framing>(), w.global::<Canvas>());
    let pick = |tab: i32| {
        panel.set_tab(tab);
        panel.invoke_tab_picked(tab);
        rig.pump();
    };
    let open = |i: i32| {
        std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
        lib.invoke_tile_clicked(i, false, false);
        lib.invoke_tile_clicked(i, false, false);
        rig.pump();
    };
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    open(0);
    rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    rig.pump();

    // `adopt`: A, never balanced, opens with the window its key proposes, armed and drawn on Color.
    let g = ctl.state.borrow().store().settings().geometry.clone();
    assert_eq!(g.measure_window, Some(oni_settings::geometry::proposal(g.crop, false)));
    assert!(framing.get_armed());
    rig.until("the window drawn", Duration::from_secs(30), || {
        rig.pump();
        canvas.get_window_shown()
    });
    pick(3);
    assert!(!canvas.get_window_shown() && framing.get_armed(), "Details hides the window, still armed");
    rig.key("\r");
    assert!(framing.get_armed(), "twin: Return on Details applies nothing");
    pick(2);
    assert!(canvas.get_window_shown());
    rig.key("\r");
    assert!(!framing.get_armed() && !canvas.get_window_shown(), "Return applies the window");
    rig.key("\r");
    assert_eq!(panel.get_tab(), 2, "Return on Color leaves nothing");

    // ⌘K opens the crop and stays; Return applies the window first, then leaves for the last panel.
    rig.chord(Key::Control, "k");
    rig.chord(Key::Control, "k");
    assert_eq!(panel.get_tab(), 1, "⌘K never leaves the crop");
    framing.invoke_act(oni_app::Act::WindowArm);
    assert!(framing.get_armed() && canvas.get_window_shown());
    rig.key("\r");
    assert_eq!((panel.get_tab(), framing.get_armed()), (1, false));
    rig.key("\r");
    assert_eq!(panel.get_tab(), 2, "the second Return leaves the crop");
    // The last panel is recorded on the tab bar's route too.
    pick(3);
    rig.chord(Key::Control, "k");
    pick(2);
    pick(1);
    rig.key("\r");
    assert_eq!(panel.get_tab(), 2, "twin: the panel picked last, not Details where ⌘K was pressed");
    // Leaving the crop disarms the window, leaving Color does not.
    pick(1);
    framing.invoke_act(oni_app::Act::WindowArm);
    pick(2);
    assert!(!framing.get_armed(), "leaving the crop disarms");
    framing.invoke_act(oni_app::Act::WindowArm);
    pick(3);
    assert!(framing.get_armed(), "twin: leaving Color keeps it armed");
    pick(2);
    rig.key("\r");

    // De-dust leaves the Lab first; the brush is the size the last session left, and kept.
    assert_eq!(ctl.state.borrow().brush(), 40.0, "not the default 12");
    w.global::<App>().invoke_menu(oni_app::Act::Emulator);
    rig.pump();
    assert!(w.global::<Lab>().get_shown());
    ctl.act(oni_app::Act::Correction);
    rig.pump();
    assert!(!w.global::<Lab>().get_shown() && panel.get_tab() == 4, "De-dust on Pro's face");
    rig.key("]");
    assert_eq!(ctl.state.borrow().brush(), 44.0);
    let kept = Preferences::load(&support).value(preferences::BRUSH_RADIUS).and_then(serde_json::Value::as_f64);
    assert_eq!(kept, Some(44.0), "twin: the 40 read at launch is written over");
    ctl.act(oni_app::Act::Correction);
    rig.pump();
    assert_eq!(panel.get_tab(), 2);

    // ⌘A beside a photo completes the folder the selection touches.
    assert_eq!(ctl.state.borrow().picked().len(), 1, "twin: the open photo alone before the key");
    rig.chord(Key::Control, "a");
    assert_eq!(*ctl.state.borrow().picked(), names.iter().cloned().collect::<HashSet<_>>());

    // Space leaves the Lab for the gallery, the menu item lit beside a photo only.
    w.global::<App>().invoke_menu(oni_app::Act::Emulator);
    rig.pump();
    assert!(w.global::<Menus>().get_live().emulator);
    rig.key(" ");
    assert!(w.global::<App>().get_gallery() && !w.global::<Menus>().get_live().emulator);

    // A preset naming the balance key balances two photos Each, asking nothing.
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    lib.invoke_tile_clicked(1, false, false);
    lib.invoke_tile_clicked(2, true, false);
    rig.pump();
    assert_eq!(ctl.state.borrow().picked().len(), 2);
    w.global::<Presets>().invoke_apply(0);
    assert!(panel.get_balancing() && !w.global::<Choice>().get_shown(), "the preset balances, asking no strategy");
    rig.pump();
    rig.until("the preset's balance", Duration::from_secs(120), || {
        rig.pump();
        !panel.get_balancing()
    });
    // Twin: the key pressed on the same two asks Each or Average.
    panel.invoke_auto_balance();
    rig.pump();
    assert!(w.global::<Choice>().get_shown());
    w.global::<Choice>().invoke_chosen(-1);
    rig.pump();

    // The balance key disarms the window D opened armed.
    open(3);
    rig.until("D open", Duration::from_secs(60), || ctl.state.borrow().open_path() == Some(names[3].as_str()));
    assert!(framing.get_armed());
    panel.invoke_auto_balance();
    assert!(panel.get_balancing() && !framing.get_armed(), "a balance key pressed ends the window's work");
    rig.until("D's balance", Duration::from_secs(120), || {
        rig.pump();
        !panel.get_balancing()
    });
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
