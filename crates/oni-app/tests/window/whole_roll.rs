//! The whole roll shown together ends when another roll opens beside the photo: what a step reaches
//! stays what the column shows.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::{Canvas, Library, Store};
use oni_core::{Archive, EditorStore};
use oni_settings::sidecar;
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

fn exposure(path: &Path) -> f32 {
    sidecar::read(path, false, true).map(|l| l.settings.print.exposure).unwrap_or_default()
}

pub(crate) fn the_whole_roll_together_ends_when_another_roll_opens() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the whole roll is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-whole-roll-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (support, caches) = (root.join("Support"), root.join("Caches"));
    // Two rolls whose frames share their names, as two scans of one camera's counter do.
    let rolls = ["Roll 10", "Roll 8"].map(|r| {
        for (i, n) in ["A", "B", "C"].iter().enumerate() {
            common::graded_tiff(&root.join(r).join(format!("{n}.tif")), 150, 100, 0.3 + 0.1 * i as f32);
        }
        PathBuf::from(oni_library::paths::canonical(&root.join(r)))
    });
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1728, 1017));
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    shell.window.show().unwrap();
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (canvas, g) = (w.global::<Canvas>(), w.global::<Store>());
    let open = |folder: &Path| {
        shell.open_folder(folder);
        rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
        rig.settle();
    };
    let step = || {
        g.invoke_step(Field::Exposure.index(), 1);
        rig.pump();
        shell.flush();
    };
    open(&rolls[0]);
    w.global::<Library>().invoke_tile_opened(0);
    rig.pump();
    common::click(&rig, w, common::the(w, Role::Button, "Sheet").middle());
    rig.until("the roll together", Duration::from_secs(30), || canvas.get_status().as_str() == "3 frames together");
    let b = rolls[0].join("B.tif");
    let rest = exposure(&b);
    // Twin: together, a step reaches every frame of the roll, so the check sees one left behind.
    step();
    let together = exposure(&b);
    assert_ne!(together, rest, "the step reached the frames together");

    // Another roll opened beside the photo: the group ends, a step reaches the open frame alone.
    open(&rolls[1]);
    rig.until("the open frame alone", Duration::from_secs(30), || canvas.get_status().contains(" × "));
    let was = ctl.state.borrow().store().settings().print.exposure;
    let others = || ["A", "B", "C"].map(|n| exposure(&rolls[1].join(format!("{n}.tif"))));
    let others_were = others();
    step();
    assert_ne!(ctl.state.borrow().store().settings().print.exposure, was, "the step moved the open frame");
    assert_eq!(exposure(&b), together, "a frame of the roll left keeps its own");
    assert_eq!(others(), others_were, "the roll opened takes nothing of a step on another");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
