//! Prepare the Roll's masks on the headless window: Load… and Use This Frame as Flat-Field as Swift
//! sets them, the frames' previews and the measure laid through the roll's custom mask.

use oni_app::app::{Options, Shell};
use oni_app::flat_fields;
use oni_app::{Advice, Bar, Library, MaskDialog, Preparing};
use oni_core::{Archive, EditorStore};
use oni_settings::sidecar;
use slint::{ComponentHandle, Model};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use crate::common;

fn stops(path: &Path) -> [f32; 3] {
    sidecar::read(path, false, true).unwrap().settings.roll_reference.expect("measured").stops
}

/// The mean distance of two pictures' bytes, over their colour channels.
fn apart(a: &slint::Image, b: &slint::Image) -> f64 {
    let (a, b) = (a.to_rgba8().expect("pixels"), b.to_rgba8().expect("pixels"));
    assert_eq!((a.width(), a.height()), (b.width(), b.height()));
    let sum: f64 = a.as_slice().iter().zip(b.as_slice()).map(|(p, q)| [p.r, p.g, p.b].iter().zip([q.r, q.g, q.b]).map(|(x, y)| f64::from(x.abs_diff(y))).sum::<f64>()).sum();
    sum / (3 * a.as_slice().len()) as f64
}

pub(crate) fn the_roll_loads_and_takes_its_mask_and_shows_and_measures_through_it_as_swift_does() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: Prepare the Roll's masks are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-prepare-masks-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (scans, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for roll in ["R1", "R2", "R3"] {
        for (name, tone) in [("A.tif", 0.9), ("B.tif", 0.7), ("C.tif", 0.5)] {
            common::graded_tiff(&scans.join(roll).join(name), 120, 80, tone);
        }
    }
    // The panel falls off from its centre, as a light panel's does.
    let panel_file = root.join("Rig/panel.tif");
    common::pixels_tiff(&panel_file, 120, 80, |x, y| {
        let r = ((x as f32 - 60.0).powi(2) + (y as f32 - 40.0).powi(2)) / (60.0f32.powi(2) + 40.0f32.powi(2));
        [0.9 - 0.4 * r; 3]
    });
    let noise = root.join("Rig/noise.tif");
    std::fs::write(&noise, b"no picture in here").unwrap();
    let path = |roll: &str, n: &str| PathBuf::from(oni_library::paths::canonical(&scans.join(roll).join(n)));
    let chosen: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let asked = chosen.clone();
    let options = Options { support: support.clone(), caches, picker: Box::new(|| None), mask_picker: Box::new(move || asked.borrow().clone()), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, p, advice, dialog) = (w.global::<Library>(), w.global::<Preparing>(), w.global::<Advice>(), w.global::<MaskDialog>());
    shell.open_folder(&scans);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done() && ctl.state.borrow().photos() == 9);
    rig.settle();
    let index = |roll: &str, name: &str| {
        let wanted = path(roll, name).to_string_lossy().into_owned();
        ctl.state.borrow().order().iter().position(|p| *p == wanted).unwrap_or_else(|| panic!("{roll}/{name}")) as i32
    };
    // A camera's roll, so the flat-field step stands; its frames' previews landed.
    let prepare = |roll: &str| {
        lib.invoke_tile_clicked(index(roll, "A.tif"), false, false);
        rig.pump();
        w.global::<Bar>().invoke_act("prepare-roll".into());
        rig.until("the roll's surface", Duration::from_secs(10), || p.get_shown() && p.get_cells().row_count() == 3);
        rig.until("every frame read", Duration::from_secs(10), || p.get_capture() == 0);
        p.invoke_capture_picked(1);
        p.invoke_step_picked(1);
        rig.pump();
        rig.until("the previews", Duration::from_secs(60), || ctl.state.borrow().roll_previewed() == 3);
    };
    let tile = |i: usize| p.get_cells().row_data(i).unwrap().picture;
    let line = |title: &str| (0..p.get_mask_lines().row_count()).find(|&i| p.get_mask_lines().row_data(i).unwrap().text == title).unwrap_or_else(|| panic!("{title}")) as i32;
    let measure = || {
        p.invoke_step_picked(4);
        rig.pump();
        p.invoke_advance();
        rig.until("the measure landed", Duration::from_secs(120), || !p.get_shown() || advice.get_shown());
        assert!(!advice.get_shown(), "{}: {}", advice.get_title(), advice.get_message());
    };

    prepare("R1");
    assert_eq!(p.get_step(), 1);
    assert!(!p.get_can_take_flat() && !p.get_loading_flat(), "no frame picked yet");
    let bare = tile(0);

    // Load…: a cancelled panel does nothing; a file no reader takes says so; the panel's picture
    // becomes the roll's mask, its Amount as the hand left it, and no offer to move it out.
    p.invoke_load_flat();
    rig.pump();
    assert!(!p.get_loading_flat() && flat_fields::entries(&support).is_empty());
    assert!(!p.invoke_press(0, false));
    p.invoke_drag(0, -60.0, 200.0, false);
    p.invoke_release(0);
    rig.pump();
    let dialled = p.get_readouts().row_data(0).unwrap();
    assert_ne!(dialled.as_str(), "100%");
    *chosen.borrow_mut() = Some(noise.clone());
    p.invoke_load_flat();
    rig.until("the noise refused", Duration::from_secs(60), || !p.get_loading_flat());
    assert_eq!((advice.get_title().as_str(), advice.get_message().as_str()), ("No Mask Loaded", "This file could not be decoded as a picture."));
    advice.invoke_closed();
    *chosen.borrow_mut() = Some(panel_file.clone());
    p.invoke_load_flat();
    assert!(p.get_loading_flat(), "Loading… while the decode runs");
    rig.until("the panel loaded", Duration::from_secs(60), || !p.get_loading_flat());
    assert_eq!(flat_fields::entries(&support).iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["panel"]);
    assert!(p.get_flat_on() && p.get_flat_masked() && p.get_mask_title() == "panel");
    assert_eq!(p.get_readouts().row_data(0).unwrap(), dialled, "a mask loaded keeps the Amount");
    assert!(!dialog.get_shown() && !advice.get_shown());

    // The frames' previews follow the mask; switched off, they come back as they were.
    rig.until("the previews through the mask", Duration::from_secs(30), || apart(&tile(0), &bare) > 4.0);
    println!("through the mask: {:.2}", apart(&tile(0), &bare));
    rig.settle();
    println!("shot {}", rig.shot("prepare-masks-loaded").display());
    p.invoke_mask_chosen(line("None selected"));
    rig.until("twin: the previews without it", Duration::from_secs(30), || apart(&tile(0), &bare) < 1.0);
    println!("without: {:.2}", apart(&tile(0), &bare));

    // Use This Frame as Flat-Field: B picked, decoded as the mask at the whole Amount, then left out.
    assert_eq!(p.get_readouts().row_data(0).unwrap(), dialled);
    p.invoke_cell_clicked(1);
    rig.pump();
    assert!(p.get_can_take_flat());
    p.invoke_take_flat();
    assert!(p.get_taking_flat(), "Setting… while the decode runs");
    rig.until("B taken", Duration::from_secs(60), || !p.get_taking_flat());
    assert!(!advice.get_shown(), "{}", advice.get_message());
    assert!(p.get_mask_title() == "B" && p.get_readouts().row_data(0).unwrap() == "100%", "taken at the whole Amount");
    assert!(!p.get_cells().row_data(1).unwrap().ticked, "the mask's frame leaves the measure");
    assert!(p.get_cells().row_data(0).unwrap().ticked && p.get_cells().row_data(2).unwrap().ticked, "twin: the others stay");
    assert!(!dialog.get_shown() && path("R1", "B.tif").exists(), "the roll offers no move out");
    // A frame no reader takes says so under Swift's title, and the mask stays.
    std::fs::write(path("R1", "C.tif"), b"no longer a picture").unwrap();
    p.invoke_cell_clicked(2);
    p.invoke_take_flat();
    rig.until("C refused", Duration::from_secs(60), || !p.get_taking_flat());
    assert_eq!((advice.get_title().as_str(), advice.get_message().as_str()), ("No Mask Set", "This file could not be decoded as a picture."));
    advice.invoke_closed();
    assert!(p.get_mask_title() == "B" && p.get_cells().row_data(2).unwrap().ticked);
    common::graded_tiff(&path("R1", "C.tif"), 120, 80, 0.5);

    // The measure reads through the roll's mask: R1 on the panel, R2 bare, R3 on the panel again,
    // A and C voting on each, B left out or flagged.
    p.invoke_mask_chosen(line("panel"));
    measure();
    let flag_b = || {
        p.invoke_step_picked(3);
        rig.pump();
        p.invoke_sheet_clicked(1);
        assert!(p.get_cells().row_data(1).unwrap().flagged);
    };
    prepare("R2");
    flag_b();
    measure();
    prepare("R3");
    p.invoke_mask_chosen(line("panel"));
    flag_b();
    measure();
    let (masked, bare, again) = (stops(&path("R1", "A.tif")), stops(&path("R2", "A.tif")), stops(&path("R3", "A.tif")));
    println!("stops on the mask {masked:?}, bare {bare:?}, again {again:?}");
    assert!((0..3).any(|c| (masked[c] - bare[c]).abs() > 1e-3), "the mask reaches the measure");
    assert!((0..3).all(|c| (masked[c] - again[c]).abs() < 1e-4), "twin: the same mask measures the same");
    let _ = std::fs::remove_dir_all(&root);
}
