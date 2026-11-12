//! A flat-field drag on a frame with dust corrections: each patch lies on the decoded scan, so the drag
//! neither drops a patch nor heals the frame again; only another decode does.

use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::{Canvas, Dedust, Library, PanelState, Store};
use oni_core::{Archive, EditorStore};
use oni_settings::cg::Point;
use oni_settings::frame::{Correction, FlatField, FlatFieldMask};
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{ConversionMode, Id, PipelineSettings};
use slint::{ComponentHandle, Model};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::common;

/// Every patch file under the caches, with when it was written.
fn patches(caches: &Path) -> Vec<(PathBuf, SystemTime)> {
    let mut found = Vec::new();
    let mut folders = vec![caches.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                folders.push(path);
            } else if path.extension().is_some_and(|e| e == "tiff") {
                found.push((path, entry.metadata().and_then(|m| m.modified()).unwrap()));
            }
        }
    }
    found.sort();
    found
}

pub(crate) fn a_flat_field_drag_keeps_every_patch() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the flat-field drag over patches is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-flat-drag-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    common::plain_tiff(&folder.join("A.tif"), 320, 240, 0.5);
    let mut settings = PipelineSettings::rest(ConversionMode::Negative, false);
    settings.flat_field = FlatField { mask: FlatFieldMask::ValoiEasy35V1, ..FlatField::default() };
    settings.corrections = (0..3)
        .map(|k| {
            let x = 0.2 + 0.25 * k as f64;
            Correction { id: Id::new(), stroke: vec![Point { x, y: 0.6 }, Point { x: x + 0.04, y: 0.6 }], radius: 4.0 }
        })
        .collect();
    sidecar::write(&settings, &folder.join("A.tif"), WriteOptions::default()).unwrap();
    // SAFETY: set before any thread of this process reads it; the patches land in the check's own folder.
    unsafe { std::env::set_var("ONI_CACHES", &caches) };
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches: caches.clone(), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, engine, options).unwrap();
    rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, c, d) = (w.global::<Library>(), w.global::<Canvas>(), w.global::<Dedust>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A on the canvas", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1 && c.get_frame().size().width > 0);
    let healed = || !ctl.state.borrow().healing() && d.get_rows().iter().all(|r| !r.pending);
    rig.until("A's three patches", Duration::from_secs(60), || healed() && patches(&caches).len() == 3);
    let made = patches(&caches);

    // Twenty notches of Amount, each rendered: no patch dropped, none made again, no heal started.
    let store = w.global::<Store>();
    for notch in 0..20 {
        store.invoke_step(Field::FlatAmount.index(), -1);
        let revision = ctl.state.borrow().store().revision();
        rig.until("the notch rendered", Duration::from_secs(30), || ctl.state.borrow().rendered() >= revision);
        assert!(!ctl.state.borrow().healing(), "notch {notch}: the frame is healed again");
        assert!(d.get_rows().iter().all(|r| !r.pending), "notch {notch}: a patch counts as missing");
        assert_eq!(patches(&caches), made, "notch {notch}: the patch files changed");
    }
    assert!((ctl.state.borrow().store().settings().flat_field.amount - 1.0).abs() > 1e-3, "the drag moved nothing");

    // Twin: another decode of the file cuts its patches again, under its own reading.
    w.global::<PanelState>().invoke_film_type_picked(1);
    rig.until("the positive's patches", Duration::from_secs(120), || healed() && patches(&caches).len() == 6);
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
