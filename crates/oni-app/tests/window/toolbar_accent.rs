//! The accent on each screen's own commit alone: ADD FOLDER… or PREPARE ROLL in the gallery, EXPORT in the
//! editor, a tool's DONE while the crop or De-dust stands open, none on the projects or in the ONI Trash.

use oni_app::app::{Options, Shell};
use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::{Act, App, Bar, Library};
use oni_core::{Archive, EditorStore};
use slint::platform::WindowEvent;
use slint::ComponentHandle;
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

/// The share of a key's face drawn in the accent's blue, in the window drawn now.
fn blue(rig: &common::Rig, key: common::Placed) -> f32 {
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
    lit as f32 / total.max(1) as f32
}

pub(crate) fn the_accent_lands_on_each_screens_own_commit() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the bar's accent is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-toolbar-accent-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for (name, tone) in [("A.tif", 0.8), ("B.tif", 0.5)] {
        common::graded_tiff(&folder.join(name), 120, 80, tone);
    }
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1728, 900));
    shell.window.show().unwrap();
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (lib, bar) = (w.global::<Library>(), w.global::<Bar>());
    // The bar's key so named.
    let face = |label: &str| {
        rig.settle();
        let keys: Vec<_> = common::placed(w, Role::Button, label).into_iter().filter(|k| k.y < bar.get_height()).collect();
        assert_eq!(keys.len(), 1, "one {label:?} on the bar: {keys:?}");
        blue(&rig, keys[0])
    };
    // An empty archive: a folder to add is all there is to do.
    let add = face("ADD FOLDER…");
    assert!(add > 0.5, "ADD FOLDER… wears the accent on the archive: {add}");
    // A roll never measured: Prepare Roll takes it, and the slot keeps its cap without it.
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.until("the roll found unmeasured", Duration::from_secs(10), || lib.get_roll_unmeasured());
    let (prepare, add) = (face("PREPARE ROLL"), face("ADD FOLDER…"));
    println!("shot {}", rig.shot("toolbar-accent-unmeasured").display());
    assert!(prepare > 0.5 && add < 0.05, "PREPARE ROLL alone wears the accent: {prepare} {add}");
    // One Prepare Roll on screen: the info column's notice points to the bar's, with no key of its own.
    let keys = |label: &str| common::placed(w, Role::Button, label).len();
    assert_eq!((keys("PREPARE ROLL"), keys("Prepare Roll")), (1, 0), "the bar's key alone prepares the roll");
    assert_eq!(common::placed(w, Role::Text, "Not prepared yet").len(), 1, "twin: the notice the key stood under shows");
    // The editor's commit is EXPORT until a tool opens: the tool's DONE takes the accent while it stands.
    let tile = |name: &str| oni_app::app::tiles(w).into_iter().find(|t| t.caption == name).unwrap_or_else(|| panic!("{name}")).index;
    lib.invoke_tile_opened(tile("A"));
    rig.until("A in the editor", Duration::from_secs(60), || !w.global::<App>().get_gallery() && ctl.state.borrow().renders() >= 1);
    let export = face("EXPORT");
    assert!(export > 0.5, "EXPORT wears the accent in the editor: {export}");
    for (tool, act) in [("crop", Act::Framing), ("De-dust", Act::Correction)] {
        bar.invoke_act(act);
        rig.settle();
        let (done, export) = (blue(&rig, common::the(w, Role::Button, "DONE")), face("EXPORT"));
        println!("shot {}", rig.shot(&format!("toolbar-accent-{tool}")).display());
        assert!(done > 0.5 && export < 0.05, "the {tool}'s DONE alone wears the accent: {done} {export}");
        bar.invoke_act(act);
        let export = face("EXPORT");
        assert!(export > 0.5 && common::placed(w, Role::Button, "DONE").is_empty(), "twin: the {tool} left, EXPORT takes it back: {export}");
    }
    bar.invoke_act(Act::ToggleMode);
    rig.until("the gallery again", Duration::from_secs(10), || w.global::<App>().get_gallery());
    // The projects and the ONI Trash, where neither key is the screen's: no accent.
    lib.invoke_surface_picked(1);
    let (prepare, add) = (face("PREPARE ROLL"), face("ADD FOLDER…"));
    assert!(prepare < 0.05 && add < 0.05, "no accent on the projects: {prepare} {add}");
    lib.invoke_surface_picked(0);
    lib.invoke_trash_row_clicked(0);
    let (prepare, add) = (face("PREPARE ROLL"), face("ADD FOLDER…"));
    println!("shot {}", rig.shot("toolbar-accent-trash").display());
    assert!(prepare < 0.05 && add < 0.05, "no accent in the ONI Trash: {prepare} {add}");
    let _ = std::fs::remove_dir_all(&root);
}
