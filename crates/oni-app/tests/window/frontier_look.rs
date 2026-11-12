//! The hand's Frontier look from a file: read at launch, read again on each save, a broken save
//! keeping the last, and the section's Reset, apart from the file keys, returning to the measured table.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::AppSettings;
use oni_app::app::{Options, Shell};
use oni_app::preferences::{self, Preferences};
use oni_core::{Archive, EditorStore};
use oni_pipeline::frontier_table;
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::common;

fn generation() -> Option<u64> {
    frontier_table::current().map(|(g, _)| g)
}

/// Writes `text` with a date a second past the last, as an editor's save would.
fn save(path: &Path, text: &str, at: u64) {
    std::fs::write(path, text).unwrap();
    let file = std::fs::File::options().write(true).open(path).unwrap();
    file.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(2_000_000_000 + at)).unwrap();
}

pub(crate) fn a_frontier_look_file_is_read_again_on_each_save() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the window is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-frontier-look-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join("My Frontier.cube");
    save(&file, &frontier_table::write(&frontier_table::starting()), 0);
    let support = root.join("Support");
    Preferences::load(&support).set(preferences::FRONTIER_LOOK, file.to_string_lossy().into_owned().into()).unwrap();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), |_: &Path| false));
    let options = Options { support: support.clone(), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();

    let first = generation().expect("the file's look stands in at launch");
    shell.ctl.act(oni_app::Act::Settings);
    rig.settle();
    let w = &shell.window;
    common::click(&rig, w, common::the(w, Role::ListItem, "Frontier Look").middle());
    rig.settle();
    let sheet = w.global::<AppSettings>();
    assert_eq!(sheet.get_frontier_file(), "My Frontier.cube");
    let shown = |text: &str| !common::placed(w, Role::Text, text).is_empty();
    assert!(shown("Table in use") && shown("My Frontier.cube") && !shown("Measured"), "the file named as the table in use");
    // The Reset shown on the section's title row, never on the row of the keys that keep a file.
    let reset = || {
        let titles = common::placed(w, Role::Text, "Frontier Look");
        common::placed(w, Role::Button, "Reset").into_iter().filter(|k| titles.iter().any(|t| t.level_with(k))).collect::<Vec<_>>()
    };
    let keys = reset();
    assert_eq!(keys.len(), 1, "one Reset on the title's row, found {keys:?}");
    let export = common::the(w, Role::Button, "Export…");
    assert!(!export.level_with(&keys[0]), "twin: the Reset stands apart from Export… and Use a File…");
    assert!(common::placed(w, Role::Button, "Measured").is_empty(), "no Measured key beside the file keys");
    rig.shot("settings-frontier-file");

    let mut halved = frontier_table::starting();
    halved.cube_data.iter_mut().for_each(|n| n[..3].iter_mut().for_each(|v| *v *= 0.5));
    save(&file, &frontier_table::write(&halved), 1);
    rig.until("the save read again", Duration::from_secs(10), || generation() > Some(first));
    let saved = generation();
    assert!((frontier_table::current().unwrap().1.cube_data[100][0] - halved.cube_data[100][0]).abs() < 1e-6, "the file's six decimals");

    // Twin: a save cut short keeps the last good look.
    save(&file, "LUT_3D_SIZE 33\n0 0 0\n", 2);
    rig.until("the broken save looked at", Duration::from_secs(10), || {
        rig.pump();
        true
    });
    std::thread::sleep(Duration::from_millis(1200));
    rig.pump();
    rig.settle();
    assert_eq!(generation(), saved, "a broken file never stands in");

    common::click(&rig, w, keys[0].middle());
    rig.settle();
    assert_eq!(generation(), None);
    assert_eq!(Preferences::load(&support).string(preferences::FRONTIER_LOOK), None);
    assert!(shown("Measured") && !shown("My Frontier.cube"), "the measured table named as the one in use");
    // The block's Reset leaves while the measured table is the one in use.
    assert!(reset().is_empty(), "nothing left to reset");
    rig.shot("settings-frontier-measured");
    drop(sheet);
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
