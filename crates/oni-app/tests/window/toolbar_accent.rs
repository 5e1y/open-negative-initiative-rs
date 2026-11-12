//! The accent on each screen's one primary: ADD FOLDER… on an empty archive, PREPARE ROLL beside a roll
//! never measured, EXPORT at the bar's right end once it can act, in the gallery as in the editor, a tool's
//! DONE while the crop or De-dust stands open, none on the projects or in the ONI Trash. Each is a keycap
//! filled with the accent, as a DONE is.

use oni_app::app::{Options, Shell};
use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::{Act, App, Bar, Library};
use oni_core::{Archive, EditorStore};
use oni_settings::frame::RollReference;
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{ConversionMode, Id, PipelineSettings};
use slint::platform::WindowEvent;
use slint::ComponentHandle;
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

/// The pixels of a key drawn in the accent's blue in the window drawn now, and their share of its face.
fn blue(rig: &common::Rig, key: common::Placed) -> (usize, f32) {
    let (w, px) = rig.drawn();
    let (xs, ys) = (key.x as usize..(key.x + key.width) as usize, key.y as usize..(key.y + key.height) as usize);
    let total = xs.len() * ys.len();
    let lit = ys
        .flat_map(|y| xs.clone().map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let v = px[y * w + x].0;
            let (r, b) = (((v >> 11) & 0x1f) * 255 / 31, (v & 0x1f) * 255 / 31);
            b > r + 80
        })
        .count();
    (lit, lit as f32 / total.max(1) as f32)
}

/// A key filled with the accent: most of its box blue, as a DONE's; a glyph alone in the accent, some tens
/// of pixels, stays under a twentieth of it.
const FILL: f32 = 0.5;

fn filled((_, share): (usize, f32)) -> bool {
    share > FILL
}

pub(crate) fn the_accent_lands_on_each_screens_own_commit() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the bar's accent is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-toolbar-accent-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, measured, support, caches) = (root.join("Scans"), root.join("Measured"), root.join("Support"), root.join("Caches"));
    for (name, tone) in [("A.tif", 0.8), ("B.tif", 0.5)] {
        common::graded_tiff(&folder.join(name), 120, 80, tone);
    }
    // A roll measured once: nothing waits on Prepare Roll.
    let done = measured.join("C.tif");
    common::graded_tiff(&done, 120, 80, 0.6);
    let mut s = PipelineSettings::rest(ConversionMode::Negative, false);
    s.roll_reference = Some(RollReference { id: Id::new(), stops: [0.0; 3], black: [0.0; 3], white: [1.0; 3], base: None, median: None });
    sidecar::write(&s, &done, WriteOptions::default()).unwrap();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1728, 900));
    shell.window.show().unwrap();
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (lib, bar) = (w.global::<Library>(), w.global::<Bar>());
    // The bar's key so named.
    let key = |label: &str| {
        rig.settle();
        let keys: Vec<_> = common::placed(w, Role::Button, label).into_iter().filter(|k| k.y < bar.get_height()).collect();
        assert_eq!(keys.len(), 1, "one {label:?} on the bar: {keys:?}");
        keys[0]
    };
    let face = |label: &str| blue(&rig, key(label));
    // EXPORT ends the bar, the columns' inset from the window's right edge (`ShellStyle.bar-end`), no key after it.
    let at_end = || {
        let export = key("EXPORT");
        let after = common::placed_all(w, Role::Button).into_iter().filter(|k| k.y < bar.get_height() && k.x > export.x).count();
        ((export.x + export.width - (1728.0 - 16.0)).abs() < 0.5, after)
    };

    // An empty archive: a folder to add is all there is to do, EXPORT out of reach at the end.
    let (add, export) = (face("ADD FOLDER…"), face("EXPORT"));
    println!("shot {}", rig.shot("toolbar-accent-archive").display());
    assert!(filled(add) && export.0 == 0, "ADD FOLDER… alone filled with the accent: {add:?} {export:?}");
    assert_eq!(at_end(), (true, 0), "EXPORT at the gallery's right end");
    assert!(key("ADD FOLDER…").x < key("Reset").x, "ADD FOLDER… in the centre, before Reset");
    // A roll never measured: Prepare Roll takes it.
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.until("the roll found unmeasured", Duration::from_secs(10), || lib.get_roll_unmeasured());
    let (prepare, add, export) = (face("PREPARE ROLL"), face("ADD FOLDER…"), face("EXPORT"));
    println!("shot {}", rig.shot("toolbar-accent-unmeasured").display());
    assert!(filled(prepare) && add.0 == 0 && export.0 == 0, "PREPARE ROLL alone filled: {prepare:?} {add:?} {export:?}");
    // One Prepare Roll on screen: the info column's notice points to the bar's, with no key of its own.
    let keys = |label: &str| common::placed(w, Role::Button, label).len();
    assert_eq!((keys("PREPARE ROLL"), keys("Prepare Roll")), (1, 0), "the bar's key alone prepares the roll");
    assert_eq!(common::placed(w, Role::Text, "Not prepared yet").len(), 1, "twin: the notice the key stood under shows");
    // The editor's commit is EXPORT until a tool opens: the tool's DONE takes the accent while it stands.
    let tile = |name: &str| oni_app::app::tiles(w).into_iter().find(|t| t.caption == name).unwrap_or_else(|| panic!("{name}")).index;
    lib.invoke_tile_opened(tile("A"));
    rig.until("A in the editor", Duration::from_secs(60), || !w.global::<App>().get_gallery() && ctl.state.borrow().renders() >= 1);
    let export = face("EXPORT");
    println!("shot {}", rig.shot("toolbar-accent-editor").display());
    assert!(filled(export), "EXPORT filled with the accent: {export:?}");
    assert_eq!(at_end(), (true, 0), "EXPORT at the editor's right end, where it stands in the gallery");
    for name in ["BACK", "Copy", "Settings"] {
        assert_eq!(face(name).0, 0, "twin: {name} wears no accent");
    }
    for (tool, act) in [("crop", Act::Framing), ("De-dust", Act::Correction)] {
        bar.invoke_act(act);
        rig.settle();
        let (done, export) = (blue(&rig, common::the(w, Role::Button, "DONE")), face("EXPORT"));
        println!("shot {}", rig.shot(&format!("toolbar-accent-{tool}")).display());
        assert!(filled(done) && export.0 == 0, "the {tool}'s DONE alone wears the accent: {done:?} {export:?}");
        bar.invoke_act(act);
        let export = face("EXPORT");
        assert!(filled(export) && common::placed(w, Role::Button, "DONE").is_empty(), "twin: the {tool} left, EXPORT takes it back: {export:?}");
    }
    bar.invoke_act(Act::ToggleMode);
    rig.until("the gallery again", Duration::from_secs(10), || w.global::<App>().get_gallery());
    // A measured roll with a photo picked: the gallery's EXPORT is its primary, ADD FOLDER… a key like any other.
    shell.open_folder(&measured);
    rig.until("the measured folder read", Duration::from_secs(30), || ctl.state.borrow().read_done() && oni_app::app::tiles(w).iter().any(|t| t.caption == "C"));
    lib.invoke_tile_clicked(tile("C"), false, false);
    rig.until("the roll found measured", Duration::from_secs(10), || {
        rig.pump();
        !lib.get_roll_unmeasured()
    });
    let (export, add, prepare) = (face("EXPORT"), face("ADD FOLDER…"), face("PREPARE ROLL"));
    println!("shot {}", rig.shot("toolbar-accent-gallery").display());
    assert!(filled(export) && add.0 == 0 && prepare.0 == 0, "the gallery's EXPORT alone filled: {export:?} {add:?} {prepare:?}");
    // The projects and the ONI Trash, where nothing is the screen's to commit: no accent.
    lib.invoke_surface_picked(1);
    let (prepare, add, export) = (face("PREPARE ROLL"), face("ADD FOLDER…"), face("EXPORT"));
    assert!(prepare.0 == 0 && add.0 == 0 && export.0 == 0, "no accent on the projects: {prepare:?} {add:?} {export:?}");
    lib.invoke_surface_picked(0);
    lib.invoke_trash_row_clicked(0);
    let (prepare, add, export) = (face("PREPARE ROLL"), face("ADD FOLDER…"), face("EXPORT"));
    println!("shot {}", rig.shot("toolbar-accent-trash").display());
    assert!(prepare.0 == 0 && add.0 == 0 && export.0 == 0, "no accent in the ONI Trash: {prepare:?} {add:?} {export:?}");
    let _ = std::fs::remove_dir_all(&root);
}
