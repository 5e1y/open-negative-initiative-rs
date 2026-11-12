//! A Swift user's rolls opened by this app: `ONI_UPGRADE_ROLLS` holds one folder per roll, mirrored
//! here (sidecars copied, originals linked), so neither the folder nor its originals are ever touched.

use oni_app::app::{self, Options, Shell, WRITE_DELAY};
use oni_app::{App, Library, Notice};
use oni_core::{Archive, EditorStore};
use slint::platform::{Key, WindowEvent};
use slint::ComponentHandle;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::common;

fn mirror(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if src.is_dir() {
            mirror(&src, &dst);
        } else if src.extension().is_some_and(|e| e == "json") || std::fs::hard_link(&src, &dst).is_err() {
            std::fs::copy(&src, &dst).unwrap();
        }
    }
}

fn sidecars(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "json")).collect();
    out.sort();
    out
}

/// What a sidecar's bytes say, of whatever format: its settings, its label and its import date.
fn said(bytes: &[u8], mosaic: bool) -> Option<(oni_settings::PipelineSettings, oni_settings::Medal, Option<oni_settings::Date>)> {
    let (c, _) = oni_settings::migrate::reading(bytes, &oni_settings::migrate::Facts::bare(mosaic), true).ok()?;
    Some((c.document.settings, c.document.label, c.document.imported_at))
}

/// Whether `now` says another thing than `was`: a migration rewrites the bytes, never their meaning.
fn edited(was: &[u8], now: &[u8], mosaic: bool) -> Option<String> {
    (said(was, mosaic) != said(now, mosaic)).then(|| "the settings, the label or the import date moved".into())
}

fn settle(rig: &common::Rig, span: Duration) {
    let start = Instant::now();
    while start.elapsed() < span {
        rig.pump();
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Each roll opened, its first frame edited by nothing but a double click and left again: a sidecar
/// is rewritten by its migration alone, which keeps every setting the earlier file says.
pub(crate) fn a_swift_roll_opened_and_left_rewrites_no_sidecar() {
    let Some(source) = std::env::var_os("ONI_UPGRADE_ROLLS").map(PathBuf::from) else {
        return eprintln!("ONI_UPGRADE_ROLLS is unset: the upgrade is not exercised");
    };
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the upgrade is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-upgrade-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (rolls, support, caches) = (root.join("Rolls"), root.join("Support"), root.join("Caches"));
    mirror(&source, &rolls);
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let lib = w.global::<Library>();
    let mut folders: Vec<PathBuf> = std::fs::read_dir(&rolls).unwrap().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    folders.sort();
    let (mut rewritten, mut moved, mut seen, mut notices) = (Vec::new(), Vec::new(), 0, 0);
    for folder in &folders {
        let files = sidecars(folder);
        let before: Vec<Vec<u8>> = files.iter().map(|f| std::fs::read(f).unwrap()).collect();
        shell.open_folder(folder);
        rig.until("the roll read", Duration::from_secs(60), || ctl.state.borrow().read_done());
        let tiles = app::tiles(w).len();
        rig.until("the thumbnails", Duration::from_secs(300), || ctl.state.borrow().loaded() >= tiles);
        let renders = ctl.state.borrow().renders();
        std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
        lib.invoke_tile_clicked(0, false, false);
        lib.invoke_tile_clicked(0, false, false);
        rig.pump();
        assert!(!w.global::<App>().get_gallery(), "{}: a double click opens the editor", folder.display());
        rig.until("the frame rendered", Duration::from_secs(120), || ctl.state.borrow().renders() > renders);
        settle(&rig, WRITE_DELAY + Duration::from_millis(600));
        // The RAW notice Swift's own preferences had dismissed shows once to the upgraded user, never twice.
        if w.global::<Notice>().get_shown() {
            notices += 1;
            w.global::<Notice>().invoke_close();
            rig.pump();
        }
        rig.chord(Key::Control, "g");
        assert!(w.global::<App>().get_gallery(), "⌘G goes back to the gallery");
        settle(&rig, WRITE_DELAY + Duration::from_millis(600));
        for (file, was) in files.iter().zip(&before) {
            seen += 1;
            let now = std::fs::read(file).unwrap();
            if &now == was {
                continue;
            }
            let original = file.with_extension("");
            let mosaic = oni_decode::decodes_via_mosaic(&original);
            if file.file_name().is_some_and(|n| n == oni_settings::film::roll_folder::ROLL_MARKER) {
                rewritten.push(format!("{}: the roll marker", file.display()));
            } else if let Some(why) = edited(was, &now, mosaic) {
                moved.push(format!("{}: {why}", file.display()));
            }
        }
    }
    eprintln!("upgrade: {} rolls, {seen} sidecars, {} rewritten {rewritten:?}", folders.len(), rewritten.len());
    assert!(seen > 0, "twin: the mirror holds sidecars");
    assert!(notices <= 1, "the RAW notice came back {notices} times");
    assert!(moved.is_empty(), "settings moved by a rewrite: {moved:?}");
    assert!(rewritten.is_empty(), "rewritten on a mere opening: {rewritten:?}");

    // Twin: a notch on the frame the last roll opened is an edit the comparison sees.
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    let renders = ctl.state.borrow().renders();
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("the frame rendered again", Duration::from_secs(120), || ctl.state.borrow().renders() > renders);
    let frame = oni_settings::sidecar::url_for(Path::new(ctl.state.borrow().open_path().unwrap()));
    let was = std::fs::read(&frame).unwrap();
    w.global::<oni_app::Store>().invoke_step(oni_app::fields::Field::Exposure.index(), 1);
    let mosaic = oni_decode::decodes_via_mosaic(Path::new(ctl.state.borrow().open_path().unwrap()));
    rig.until("the notch written", Duration::from_secs(5), || edited(&was, &std::fs::read(&frame).unwrap(), mosaic).is_some());
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
