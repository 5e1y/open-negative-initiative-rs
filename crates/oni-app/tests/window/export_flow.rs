//! An export from the editor unfolds beside it: the surface closes, the status bar counts the frames
//! to the last with its Stop, then the notice says how it ended and shows the files written.
//! Under a rating filter Export opens on the photos it keeps, and the estimate weighs the file written.

use oni_app::app::export_card::{self, job::End};
use oni_app::app::{Options, Shell};
use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::{App, Confirm, Exporting, Library, Menus, Told};
use oni_core::{Archive, EditorStore};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::common;

const LONG: Duration = Duration::from_secs(120);

pub(crate) fn an_export_closes_its_surface_and_counts_its_frames_in_the_status_bar() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the export's card is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-export-flow-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, out) = (root.join("Scans"), root.join("Out"));
    std::fs::create_dir_all(&out).unwrap();
    let names = ["A.tif", "B.tif", "C.tif"];
    for (i, n) in names.iter().enumerate() {
        common::linear_tiff(&folder.join(n), 96, 64, 0.4 + 0.1 * i as f32);
    }
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    let (ww, wh) = (1280.0f32, 800.0f32);
    rig.window.set_size(slint::PhysicalSize::new(ww as u32, wh as u32));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    let lib = w.global::<Library>();
    let e = w.global::<Exporting>();

    // Gold on A and C, the filter on Gold and nothing picked: Export opens on the two it keeps.
    for tile in [0, 2] {
        lib.invoke_tile_clicked(tile, false, false);
        rig.key("1");
        rig.landed(&ctl);
    }
    lib.invoke_tree_empty_clicked();
    lib.invoke_filter_picked(1);
    rig.pump();
    let menus = w.global::<Menus>();
    assert_eq!(menus.get_titles().export, "Export 2 Photos…");
    rig.chord(slint::platform::Key::Control, "e");
    assert!(e.get_shown(), "⌘E opens the surface on the filter's photos");
    let opened: Vec<String> = export_card::card(&ctl).targets.iter().map(|t| t.path.file_name().unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(opened, ["A.tif", "C.tif"]);
    ctl.leave_export();
    // Twin: every photo shown and none picked, Export waits for a pick.
    lib.invoke_filter_picked(0);
    rig.pump();
    assert!(!menus.get_live().export, "the whole gallery is no set the hand chose");
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));

    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A open", Duration::from_secs(60), || ctl.state.borrow().open_path().is_some());
    assert!(!w.global::<App>().get_gallery());

    let chosen = Rc::new(RefCell::new(out.clone()));
    let answer = chosen.clone();
    export_card::answer_with(&ctl, Rc::new(move |_| vec![answer.borrow().clone()]));
    let targets = || names.iter().map(|n| oni_app::export::Target { path: folder.join(n), settings: Default::default() }).collect::<Vec<_>>();
    let told = w.global::<Told>();

    // The key pressed: the surface closes onto the editor at once, the status bar counts and offers Stop.
    ctl.enter_export(targets());
    rig.until("the lines", LONG, || export_card::card(&ctl).rows.iter().all(Option::is_some));
    assert!(e.get_shown());
    e.invoke_export();
    assert!(!e.get_shown() && !w.global::<App>().get_gallery(), "the surface closed onto the editor");
    assert!(e.get_job_shown(), "the status bar follows the export");
    assert_eq!(e.get_job_count(), "0 of 3");
    assert_eq!(common::placed(w, Role::Button, "Stop").len(), 1, "the status bar's Stop shows");
    assert_eq!(common::placed(w, Role::Text, "Exporting…").len(), 1, "the status bar says what runs");
    assert!(!told.get_shown(), "nothing told while it runs");
    rig.shot("export-status-bar");

    // One at a time: the surface reopened meanwhile says so and its key starts nothing.
    ctl.enter_export(targets());
    assert!(e.get_exporting(), "the reopened surface's key waits");
    assert!(e.get_subtitle().starts_with("An export is under way, 0 of 3"), "{}", e.get_subtitle());
    e.invoke_export();
    assert_eq!(export_card::job::job(&ctl).map(|j| j.1), Some(3), "no second export began");
    ctl.leave_export();

    // Quitting meanwhile asks first; Cancel keeps the export going.
    rig.window.dispatch_event(WindowEvent::CloseRequested);
    let confirm = w.global::<Confirm>();
    assert!(confirm.get_shown() && confirm.get_title() == "An export is under way", "quitting mid-export asks");
    confirm.invoke_answered(false);

    // The count advances to the last frame, then the bar goes back to its line and the notice offers
    // the files written.
    let mut seen = Vec::new();
    let start = Instant::now();
    while export_card::job::job(&ctl).is_some_and(|j| j.2.is_none()) {
        assert!(start.elapsed() < LONG, "the export never ended");
        rig.pump();
        seen.push(e.get_job_count().to_string());
        std::thread::sleep(Duration::from_millis(5));
    }
    seen.dedup();
    assert_eq!(seen.last().map(String::as_str), Some("3 of 3"), "{seen:?}");
    assert_eq!(export_card::job::job(&ctl), Some((3, 3, Some(End::Written))));
    assert!(!e.get_job_shown(), "the status bar is the editor's again");
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 3, "every frame written");
    rig.settle();
    assert!(common::placed(w, Role::Button, "Stop").is_empty(), "no Stop once it ended");
    assert_eq!((told.get_title().as_str(), told.get_message().as_str()), ("Export", "3 photos exported"));
    assert_eq!(told.get_action(), common::reveal());
    assert_eq!(common::placed(w, Role::Button, common::reveal()).len(), 1, "its key shows over the editor");
    rig.shot("export-told");
    told.invoke_closed();

    // A folder named but missing is made and written into, as a save makes it.
    let made = root.join("Gone").join("Here");
    *chosen.borrow_mut() = made.clone();
    ctl.enter_export(targets());
    rig.until("the lines", LONG, || export_card::card(&ctl).rows.iter().all(Option::is_some));
    e.invoke_export();
    rig.until("the export into the folder made", LONG, || export_card::job::job(&ctl).is_some_and(|j| j.2.is_some()));
    assert_eq!(export_card::job::job(&ctl), Some((3, 3, Some(End::Written))));
    assert_eq!(std::fs::read_dir(&made).unwrap().count(), 3, "every frame written in the folder made");
    told.invoke_closed();

    // Stop, clicked in the status bar, finishes the frame under way and begins none after it: frames
    // large enough that the click lands while the first is written.
    *chosen.borrow_mut() = root.join("Stopped");
    let large: Vec<_> = (0..3).map(|i| root.join("Large").join(format!("L{i}.tif"))).collect();
    for path in &large {
        common::linear_tiff(path, 3000, 2000, 0.5);
    }
    ctl.enter_export(large.iter().map(|path| oni_app::export::Target { path: path.clone(), settings: Default::default() }).collect());
    rig.until("the lines", LONG, || export_card::card(&ctl).rows.iter().all(Option::is_some));
    e.invoke_export();
    rig.drawn();
    let stop = common::the(w, Role::Button, "Stop").middle();
    for event in [WindowEvent::PointerPressed { position: LogicalPosition::new(stop.0, stop.1), button: PointerEventButton::Left }, WindowEvent::PointerReleased { position: LogicalPosition::new(stop.0, stop.1), button: PointerEventButton::Left }] {
        rig.window.dispatch_event(event);
    }
    assert!(e.get_job_stopping() && e.get_job_title() == "Stopping after this photo…", "the click reached Stop");
    rig.until("the export stopped", LONG, || export_card::job::job(&ctl).is_some_and(|j| j.2.is_some()));
    let (done, _, end) = export_card::job::job(&ctl).unwrap();
    assert_eq!(end, Some(End::Stopped));
    assert!(done < 3, "a frame after the stop was begun: {done}");
    assert!(told.get_message().starts_with("Stopped after"), "{}", told.get_message());
    told.invoke_closed();

    // Twin: a folder that cannot be made, in a read-only one, says why over the surface, which stays.
    let locked = root.join("Locked");
    std::fs::create_dir_all(&locked).unwrap();
    let mut permissions = std::fs::metadata(&locked).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&locked, permissions.clone()).unwrap();
    *chosen.borrow_mut() = locked.join("Here");
    ctl.enter_export(targets());
    rig.until("the lines", LONG, || export_card::card(&ctl).rows.iter().all(Option::is_some));
    e.invoke_export();
    assert!(e.get_notice_shown() && e.get_notice_title() == "Export Failed", "{}", e.get_notice_message());
    assert!(e.get_shown() && export_card::job::job(&ctl).is_some_and(|j| j.2 == Some(End::Stopped)), "no export began; the surface waits");
    e.invoke_notice_closed();

    // A folder that cannot be written fails in the notice, naming the frame, with nothing to show.
    *chosen.borrow_mut() = locked.clone();
    e.invoke_export();
    rig.until("the failure", LONG, || export_card::job::job(&ctl).is_some_and(|j| j.2.is_some() && j.2 != Some(End::Stopped)));
    assert!(matches!(export_card::job::job(&ctl).and_then(|j| j.2), Some(End::Failed(_))));
    assert_eq!(told.get_title(), "Export Failed");
    assert!(told.get_message().starts_with("A.tif: "), "the failure names its frame: {}", told.get_message());
    assert_eq!(told.get_action(), "", "no file written, none to show");
    assert!(!e.get_notice_shown(), "the failure is the notice's, no sheet over the window");
    told.invoke_closed();
    // Twin: once no export runs, quitting asks nothing.
    rig.window.dispatch_event(WindowEvent::CloseRequested);
    assert!(!confirm.get_shown(), "no export, no question on quitting");
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    let _ = std::fs::set_permissions(&locked, permissions);
    let _ = std::fs::remove_dir_all(&root);
}

/// The estimate on a frame resized for the web weighs the file the export then writes.
pub(crate) fn the_estimate_weighs_the_file_the_export_writes() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the estimate is not weighed"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-export-estimate-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (scan, out) = (root.join("Scans").join("Grain.tif"), root.join("Out").join("Grain.jpg"));
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    // Detail at every scale the sample and the file are drawn at, none finer than either keeps.
    common::pixels_tiff(&scan, 3000, 2000, |x, y| {
        let (x, y) = (x as f32, y as f32);
        let v = 0.45 + 0.15 * (x / 41.0).sin() * (y / 37.0).cos() + 0.08 * ((x + 2.0 * y) / 19.0).sin() + 0.05 * ((3.0 * x - y) / 23.0).cos();
        [v, 0.9 * v + 0.05 * (y / 53.0).sin(), 0.8 * v + 0.05 * (x / 61.0).cos()]
    });
    let rig = common::rig(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { canvas: false, support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(&gpu), options).unwrap();
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let e = shell.window.global::<Exporting>();
    let answer = out.clone();
    export_card::answer_with(&ctl, Rc::new(move |_| vec![answer.clone()]));

    ctl.enter_export(vec![oni_app::export::Target { path: scan, settings: Default::default() }]);
    e.invoke_preset(0);
    rig.until("the size", LONG, || e.get_size_enabled());
    e.invoke_size_typed("1500".into(), e.get_height_text());
    assert_eq!(e.get_dims(), "1500 × 1000 px");
    rig.until("the estimate", LONG, || export_card::estimate(&ctl).0.is_some());
    let estimated = export_card::estimate(&ctl).0.unwrap() as f64;
    e.invoke_export();
    rig.until("the file written", LONG, || export_card::job::job(&ctl).is_some_and(|j| j.2.is_some()));
    let written = std::fs::metadata(&out).unwrap().len() as f64;
    println!("estimated {estimated:.0} bytes, written {written:.0}");
    assert!((estimated / written - 1.0).abs() < 0.05, "estimated {estimated:.0} bytes for a file of {written:.0}");
    // Twin: weighed over the frame's full pixels, as the panel weighed it, the figure reads four times the file.
    let former = estimated * (3000.0 * 2000.0) / (1500.0 * 1000.0);
    assert!(former / written > 3.0, "the full frame's pixels weigh {former:.0} bytes");
    rig.settle();
    let _ = std::fs::remove_dir_all(&root);
}
