//! The Film Effects block of a digital photo through the window's pointer: a press on Grain's or
//! Halation's rail lays it, the block's Reset shows and the frame renders again; a film mode shows none.

use i_slint_backend_testing::AccessibleRole;
use oni_app::app::{Options, Shell};
use oni_app::{Library, PanelState, Store};
use oni_core::{Archive, EditorStore};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

/// The photo shown: the first photo of `ONI_RAW_DIR`, else a graded TIFF.
fn photo(folder: &Path) -> String {
    std::fs::create_dir_all(folder).unwrap();
    if let Some(dir) = std::env::var_os("ONI_RAW_DIR") {
        let kind = |p: &Path| p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let mut photos: Vec<PathBuf> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).filter(|p| ["nef", "rw2", "jpg", "tif"].contains(&kind(p).as_str())).collect();
        photos.sort();
        if let Some(photo) = photos.first() {
            let name = photo.file_name().unwrap().to_string_lossy().into_owned();
            std::fs::copy(photo, folder.join(&name)).unwrap();
            return name;
        }
    }
    common::graded_tiff(&folder.join("A.tif"), 2400, 1600, 0.7);
    "A.tif".into()
}

/// A press on the rail of the slider named `label`, at `share` of its travel.
fn press_rail(rig: &common::Rig, w: &oni_app::MainWindow, label: &str, share: f32) {
    let slider = common::the(w, AccessibleRole::Slider, label);
    let at = LogicalPosition::new(slider.x + 10.0 + share * (slider.width - 20.0), slider.y + slider.height - 13.0);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: at });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: at, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: at, button: PointerEventButton::Left });
    rig.pump();
}

/// A shell on `folder`'s one photo, open in the editor and rendered as a digital photo.
fn opened(gpu: &oni_app::render::Device, root: &Path) -> (common::Rig, Shell) {
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 1600));
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(gpu), options).unwrap();
    shell.window.show().unwrap();
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    w.global::<Library>().invoke_tile_opened(0);
    rig.until("the photo rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    w.global::<oni_app::Notice>().invoke_close();
    rig.settle();
    let n = ctl.state.borrow().renders();
    w.global::<PanelState>().invoke_film_type_picked(3);
    rig.until("the digital photo rendered", Duration::from_secs(60), || ctl.state.borrow().renders() > n);
    rig.settle();
    assert_eq!(ctl.state.borrow().store().settings().mode, oni_settings::ConversionMode::Digital);
    (rig, shell)
}

/// The view stepped up to 100 %.
fn at_full_size(rig: &common::Rig, ctl: &std::rc::Rc<oni_app::app::Ctl>) {
    for _ in 0..oni_app::viewport::ZOOM_STEPS.len() {
        if ctl.state.borrow().viewport().scale == Some(1.0) {
            break;
        }
        let n = ctl.state.borrow().renders();
        ctl.act(oni_app::Act::Larger);
        rig.until("a rung's render", Duration::from_secs(60), || ctl.state.borrow().renders() > n);
    }
    rig.settle();
    assert_eq!(ctl.state.borrow().viewport().scale, Some(1.0), "the view at 100 %");
}

pub(crate) fn a_press_on_grains_rail_lays_the_grain_and_renders_it() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the Film Effects block's grain is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-film-grain-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let name = photo(&root.join("Scans"));
    let (rig, shell) = opened(&gpu, &root);
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    assert!(ctl.state.borrow().open_path().is_some_and(|p| p.ends_with(&name)));

    let grain = || ctl.state.borrow().store().settings().grain;
    let g = w.global::<Store>();
    assert!(grain().amount == 0.0 && !g.get_film_effects_modified(), "the photo opens with no grain");
    let renders = ctl.state.borrow().renders();
    press_rail(&rig, w, "Grain", 0.8);
    rig.until("the grain rendered", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    rig.settle();
    let laid = grain();
    assert!((laid.amount - 0.8).abs() < 0.03, "the press lays Grain at its place on the rail: {laid:?}");
    assert!(g.get_film_effects_modified(), "the block's Reset shows");
    // Twin: the press reached Grain alone, its size at rest.
    assert_eq!(laid.size, 0.5, "Grain size moved: {laid:?}");
    println!("shot {}", rig.shot("film-effects-grain-fit").display());
    // At 100 %, where the grain's cells are a pixel or two.
    at_full_size(&rig, &ctl);
    println!("shot {}", rig.shot("film-effects-grain-100").display());
    // Twin: the same frame as a negative keeps its grain unshown, with no Film Effects block.
    assert_eq!(common::placed(w, AccessibleRole::Text, "Film Effects").len(), 1, "a digital photo lays Film Effects");
    w.global::<PanelState>().invoke_film_type_picked(0);
    rig.settle();
    assert_eq!(grain(), laid, "a film mode keeps the grain for a later Digital");
    assert!(common::placed(w, AccessibleRole::Text, "Film Effects").is_empty(), "a negative lays no Film Effects");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}

pub(crate) fn a_press_on_halations_rail_makes_a_highlight_glow_red() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the Film Effects block's halation is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-film-halation-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    // A night: a lit window, a lamp and a thin wire of light on a dark ground.
    std::fs::create_dir_all(root.join("Scans")).unwrap();
    common::pixels_tiff(&root.join("Scans/A.tif"), 2400, 1600, |x, y| {
        let (fx, fy) = (x as f32, y as f32);
        let window = (900.0..1300.0).contains(&fx) && (500.0..900.0).contains(&fy);
        let lamp = (fx - 1700.0).powi(2) + (fy - 1100.0).powi(2) < 60.0 * 60.0;
        let wire = (fy - 1300.0).abs() < 2.0 && (400.0..1400.0).contains(&fx);
        if window || lamp || wire { [1.0; 3] } else { [0.0; 3] }
    });
    let (rig, shell) = opened(&gpu, &root);
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let halation = || ctl.state.borrow().store().settings().halation;
    let g = w.global::<Store>();
    assert!(halation().amount == 0.0 && !g.get_film_effects_modified(), "the photo opens with no halation");
    let renders = ctl.state.borrow().renders();
    press_rail(&rig, w, "Halation", 0.95);
    press_rail(&rig, w, "Halation size", 0.95);
    rig.until("the halation rendered", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    rig.settle();
    let laid = halation();
    assert!((laid.amount - 0.95).abs() < 0.03 && (laid.size - 0.95).abs() < 0.03, "the presses lay Halation and its size: {laid:?}");
    assert!(g.get_film_effects_modified(), "the block's Reset shows");
    // Twin: the presses reached Halation alone, the grain at rest.
    assert_eq!(ctl.state.borrow().store().settings().grain.amount, 0.0);
    println!("shot {}", rig.shot("film-effects-halation-fit").display());
    at_full_size(&rig, &ctl);
    println!("shot {}", rig.shot("film-effects-halation-100").display());
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
