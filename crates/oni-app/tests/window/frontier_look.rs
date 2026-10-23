//! The hand's Frontier look from a file: read at launch, read again on each save, a broken save
//! keeping the last, and MEASURED returning to the measured table.

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
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), |_: &Path| false));
    let options = Options { support: support.clone(), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    shell.window.show().unwrap();

    let first = generation().expect("the file's look stands in at launch");
    shell.ctl.act(oni_app::Act::Settings);
    let sheet = shell.window.global::<AppSettings>();
    assert!(sheet.get_frontier_look().contains("My Frontier.cube") && sheet.get_frontier_hand());

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

    shell.ctl.act(oni_app::Act::FrontierMeasured);
    assert_eq!(generation(), None);
    assert!(!sheet.get_frontier_hand() && sheet.get_frontier_look().contains("measured"));
    assert_eq!(Preferences::load(&support).string(preferences::FRONTIER_LOOK), None);
    drop(sheet);
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
