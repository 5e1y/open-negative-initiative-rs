//! Prepare the Roll's two endings through the hand's flow: the roll's first frame alone, the status bar
//! saying how to show every frame together; Settings' switch on, every frame together at once.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::app::{Options, Shell};
use oni_app::preferences::Preferences;
use oni_app::{App, AppSettings, Bar, Canvas, Library, Preparing};
use oni_core::{Archive, EditorStore};
use slint::platform::{Key, WindowEvent};
use slint::{ComponentHandle, Model};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

pub(crate) fn prepare_ends_on_the_first_frame_alone_and_on_the_whole_roll_once_settings_asks() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: Prepare the Roll's endings are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-prepare-ending-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (scans, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for (roll, names) in [("R1", ["A.tif", "B.tif", "C.tif"]), ("R2", ["E.tif", "F.tif", "G.tif"])] {
        for (i, name) in names.iter().enumerate() {
            common::graded_tiff(&scans.join(roll).join(name), 120, 80, 0.8 - 0.2 * i as f32);
        }
    }
    let path = |roll: &str, n: &str| oni_library::paths::canonical(&scans.join(roll).join(n));
    // A preferences file as 1.5.6 leaves it: keys of its own, none for this choice.
    std::fs::create_dir_all(&support).unwrap();
    std::fs::write(Preferences::file_path(&support), br#"{ "keepZoom": false, "soundMuted": true }"#).unwrap();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support: support.clone(), caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, p, app, canvas, settings) = (w.global::<Library>(), w.global::<Preparing>(), w.global::<App>(), w.global::<Canvas>(), w.global::<AppSettings>());
    shell.open_folder(&scans);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done() && ctl.state.borrow().photos() == 6);
    rig.settle();
    let index = |name: &str| ctl.state.borrow().order().iter().position(|p| p.ends_with(name)).unwrap_or_else(|| panic!("{name}")) as i32;
    // The roll of a frame picked in the gallery, walked to its window and measured.
    let measured = |name: &str| {
        lib.invoke_tile_clicked(index(name), false, false);
        rig.pump();
        w.global::<Bar>().invoke_act(oni_app::Act::PrepareRoll);
        rig.until("the roll's surface", Duration::from_secs(10), || p.get_shown() && p.get_cells().row_count() == 3);
        rig.until("every frame read", Duration::from_secs(10), || p.get_capture() == 0);
        p.invoke_step_picked(4);
        rig.until("every frame placed", Duration::from_secs(20), || p.get_readouts().row_data(4).is_some_and(|r| r.ends_with(" / 3")));
        p.invoke_advance();
        let advice = w.global::<oni_app::Advice>();
        rig.until("the measure landed", Duration::from_secs(120), || !p.get_shown() || advice.get_shown());
        assert!(!advice.get_shown(), "{}: {}", advice.get_title(), advice.get_message());
        rig.pump();
        assert!(!app.get_gallery(), "the editor opens once measured");
    };
    let picked = || ctl.state.borrow().picked().clone();
    let hint = format!("Select All ({}A) to edit the whole roll", oni_app::platform::keys(oni_app::platform::Platform::THIS).command);
    let hinted = || common::placed(w, Role::Text, &hint).len();

    // By default, B picked: the editor on the roll's first frame alone, the bar saying how to show them all.
    measured("B.tif");
    assert_eq!(ctl.state.borrow().open_path(), Some(path("R1", "A.tif").as_str()), "the roll's first frame, not the one picked");
    assert_eq!(picked(), HashSet::from([path("R1", "A.tif")]));
    rig.until("the first frame shown", Duration::from_secs(30), || canvas.get_status().contains(" × "));
    assert!(!canvas.get_together(), "twin: no frames together");
    // The hint is the idle bar's: a frame left on screen a second builds its full resolution, the bar
    // busy meanwhile, so the bar is read once that build has landed and the bar has said so.
    rig.until("the frame's full resolution, the bar idle", Duration::from_secs(30), || ctl.state.borrow().full_built() && lib.get_loading().is_empty());
    rig.settle();
    assert_eq!(hinted(), 1, "the status bar says how to edit the whole roll");
    println!("shot {}", rig.shot("prepare-ends-alone").display());
    // ⌘A shows the roll together, and the hint, its work done, goes.
    rig.chord(Key::Control, "a");
    rig.until("the roll together", Duration::from_secs(30), || canvas.get_status().as_str() == "3 frames together");
    rig.settle();
    assert_eq!(hinted(), 0, "twin: the hint is the frame alone's");
    assert!(!p.get_landed_alone());

    // Settings > Editor's switch, off as the file left it, clicked on and kept.
    w.global::<Bar>().invoke_act(oni_app::Act::Settings);
    settings.set_section(1);
    rig.settle();
    let switch = common::the(w, Role::Checkbox, "After Prepare, open the whole roll");
    assert!(!settings.get_whole_roll_after_prepare(), "a file written without the key reads off");
    common::click(&rig, w, switch.middle());
    rig.settle();
    assert!(settings.get_whole_roll_after_prepare() && Preferences::load(&support).whole_roll_after_prepare());
    println!("shot {}", rig.shot("prepare-ending-settings").display());
    rig.key("\u{1b}");
    assert!(!settings.get_shown());

    // Switched on, F picked: every frame of its roll together, led by its first, and no hint.
    ctl.act(oni_app::Act::ToggleMode);
    rig.pump();
    measured("F.tif");
    assert_eq!(ctl.state.borrow().open_path(), Some(path("R2", "E.tif").as_str()));
    assert_eq!(picked(), ["E.tif", "F.tif", "G.tif"].iter().map(|n| path("R2", n)).collect::<HashSet<_>>(), "the whole roll picked");
    rig.until("the roll together", Duration::from_secs(30), || canvas.get_status().as_str() == "3 frames together");
    rig.settle();
    assert!(hinted() == 0 && !p.get_landed_alone(), "the whole roll shown needs no word on it");
    println!("shot {}", rig.shot("prepare-ends-whole").display());
    let _ = std::fs::remove_dir_all(&root);
}
