//! Opening an archive as the Swift app does, on the headless window: the folder picker, the gallery
//! first, the index and the remembered folder, the thumbnails pass and its files, an edit's redraw.

use oni_app::app::{self, Options, Shell, WRITE_DELAY};
use oni_app::fields::Field;
use oni_app::preferences::{self, Preferences};
use oni_app::thumbnails;
use oni_app::{App, Library, Store as StoreGlobal};
use oni_core::{Archive, EditorStore};
use oni_library::index::LibraryIndex;
use slint::ComponentHandle;
use slint::platform::{Key, WindowEvent};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

mod common;

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Pumps for `span`, as a hand resting.
fn wait(rig: &common::Rig, span: Duration) {
    let start = Instant::now();
    while start.elapsed() < span {
        rig.pump();
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_folder_opens_on_the_gallery_and_its_thumbnails_are_drawn_once() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the archive is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-archive-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for (name, tone) in [("A.tif", 1.0), ("B.tif", 0.7), ("C.tif", 0.4)] {
        common::linear_tiff(&folder.join(name), 96, 64, tone);
    }
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let chosen = folder.clone();
    let picker = Box::new(move || Some(chosen.clone()));
    let options = Options { support: support.clone(), caches: caches.clone(), picker, ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 2.0 });
    rig.window.set_size(slint::PhysicalSize::new(2 * 1280, 2 * 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;

    // The app opens on the gallery with nothing in the editor; ⌘O asks the picker, then reads behind.
    assert!(w.global::<App>().get_gallery() && ctl.state.borrow().open_path().is_none());
    assert!(ctl.state.borrow().opened().is_none());
    rig.chord(Key::Control, "o");
    rig.until("the library read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    let canonical = oni_library::paths::canonical(&folder);
    assert_eq!(ctl.state.borrow().opened(), Some(canonical.as_str()));
    assert_eq!(app::tiles(w).len(), 3);
    assert!(w.global::<App>().get_gallery() && ctl.state.borrow().open_path().is_none(), "a folder opens nothing in the editor");
    // The folder is remembered for the next launch, and the index lives in the caches.
    assert_eq!(Preferences::load(&support).string(preferences::OPENED_FOLDER), Some(canonical.as_str()));
    let index = LibraryIndex::open(&LibraryIndex::default_path(&caches)).unwrap();
    assert_eq!(index.all().len(), 3);

    // The pass draws every photo lacking a file, named by its fingerprint alone, and the tiles show them.
    rig.until("three thumbnails", Duration::from_secs(120), || ctl.state.borrow().loaded() >= 3);
    rig.until("the tiles drawn", Duration::from_secs(10), || app::tiles(w).iter().all(|t| t.picture.size().width > 0));
    let files: Vec<PathBuf> = ["A.tif", "B.tif", "C.tif"]
        .iter()
        .map(|n| {
            let path = oni_library::paths::canonical(&folder.join(n));
            let fingerprint = ctl.state.borrow().fingerprint(&path).unwrap().to_string();
            thumbnails::file(&support, &fingerprint)
        })
        .collect();
    assert!(files.iter().all(|f| f.exists()), "{files:?}");
    let size = app::tiles(w)[0].picture.size();
    assert_eq!(size.width.max(size.height) as usize, 96.min(thumbnails::SIDE), "a frame smaller than the side keeps its own");
    let stamped: Vec<_> = files.iter().map(|f| modified(f)).collect();

    // A tile in memory dropped is read back from its file, and no render runs for it.
    ctl.state.borrow_mut().forget_tiles();
    w.global::<Library>().invoke_resized(900.0, 700.0);
    let renders = ctl.state.borrow().renders();
    rig.until("the tiles read back", Duration::from_secs(10), || app::tiles(w).iter().all(|t| t.picture.size().width > 0));
    assert_eq!(ctl.state.borrow().renders(), renders);

    // The folder opened again finds every file held: no pass, nothing rewritten.
    shell.open_folder(&folder);
    rig.until("the second read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.until("the second pass", Duration::from_secs(30), || ctl.state.borrow().loaded() >= 3);
    assert!(ctl.state.borrow().load_bar().is_none());
    assert_eq!(files.iter().map(|f| modified(f)).collect::<Vec<_>>(), stamped);
    // Twin: a file removed is drawn again by the next pass, the others left as they are.
    std::fs::remove_file(&files[1]).unwrap();
    shell.open_folder(&folder);
    rig.until("the redrawn file", Duration::from_secs(120), || files[1].exists() && ctl.state.borrow().loaded() >= 3);
    assert_eq!((modified(&files[0]), modified(&files[2])), (stamped[0], stamped[2]));

    // A medal key labels the selection's sidecar and its index row.
    w.global::<Library>().invoke_tile_clicked(1, false, false);
    rig.key("2");
    let b = oni_library::paths::canonical(&folder.join("B.tif"));
    let row = |p: &str| LibraryIndex::open(&LibraryIndex::default_path(&caches)).unwrap().all().into_iter().find(|e| e.path == p).unwrap();
    assert_eq!(row(&b).label, oni_settings::Medal::Silver);
    let a = oni_library::paths::canonical(&folder.join("A.tif"));
    assert_eq!(row(&a).label, oni_settings::Medal::None, "twin: the unselected photo keeps its label");

    // A file dropped opens its folder, then itself in the editor once found.
    shell.add(&folder.join("C.tif"));
    rig.until("C in the editor", Duration::from_secs(30), || ctl.state.borrow().open_path().is_some());
    assert!(ctl.state.borrow().open_path().unwrap().ends_with("C.tif"));
    assert!(!w.global::<App>().get_gallery());
    rig.until("the first render", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    // Opening a photo redraws its thumbnail once the delay has passed, as its view appearing does.
    wait(&rig, thumbnails::REFRESH_DELAY + Duration::from_millis(500));
    rig.until("the opening's redraw", Duration::from_secs(30), || !ctl.state.borrow().drawing());

    // `Sidecar.writeDelay`: a notch writes the sidecar once the hand has rested 400 ms, not before.
    let c = PathBuf::from(ctl.state.borrow().open_path().unwrap());
    let side = oni_settings::sidecar::url_for(&c);
    let before = modified(&side);
    let thumb = files[2].clone();
    let drawn = modified(&thumb);
    w.global::<StoreGlobal>().invoke_step(Field::Exposure.index(), 1);
    let stepped = Instant::now();
    wait(&rig, WRITE_DELAY - Duration::from_millis(100));
    assert_eq!(modified(&side), before, "nothing is written while the hand may still move");
    rig.until("the sidecar written", Duration::from_secs(2), || modified(&side) != before);
    let written = stepped.elapsed();
    assert!(written >= WRITE_DELAY && written < WRITE_DELAY + Duration::from_millis(400), "{written:?}");

    // `Thumbnails.refreshDelay` after the write, the photo's thumbnail is drawn again on its new settings.
    wait(&rig, thumbnails::REFRESH_DELAY - Duration::from_millis(300));
    assert_eq!(modified(&thumb), drawn, "twin: the redraw waits past the write");
    rig.until("the redraw", Duration::from_secs(30), || modified(&thumb) != drawn);
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
