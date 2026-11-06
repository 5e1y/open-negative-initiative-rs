//! Three photos exported as one contact sheet: one file named for it, the sheet's size, black in its
//! gaps, each frame's number in orange under its picture, no logo under less than a row; the same
//! three as positives on the same black; five photos with the logo's band under a foot of four
//! margins; the sheet shown in the column, and Export As laying the gallery's pick on one sheet.

use oni_app::app::export_card::{self, Ask, job::End};
use oni_app::app::{Options, Shell};
use oni_app::{Exporting, Library};
use oni_core::{Archive, EditorStore};
use slint::{ComponentHandle, Model};
use slint::platform::WindowEvent;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use crate::common;

const LONG: Duration = Duration::from_secs(120);

pub(crate) fn a_selection_exports_as_one_contact_sheet_black_and_numbered() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the contact sheet's export is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-export-sheet-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, out) = (root.join("Scans"), root.join("Out"));
    std::fs::create_dir_all(&out).unwrap();
    let names = ["A.tif", "B.tif", "C.tif", "D.tif", "E.tif"];
    for (i, n) in names.iter().enumerate() {
        // B is a 4:3 scan, as a Lumix writes one.
        let (w, h) = if *n == "B.tif" { (2592, 1944) } else { (1500, 1000) };
        common::graded_tiff(&folder.join(n), w, h, 0.8 - 0.2 * i as f32);
    }
    let folder = PathBuf::from(oni_library::paths::canonical(&folder));
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    let lib = w.global::<Library>();
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A open", Duration::from_secs(60), || ctl.state.borrow().open_path().is_some());

    let e = w.global::<Exporting>();
    let asked: Rc<RefCell<Vec<Ask>>> = Rc::default();
    let file = Rc::new(RefCell::new(out.join("Contact Sheet.jpg")));
    let (record, answer) = (asked.clone(), file.clone());
    export_card::answer_with(&ctl, Rc::new(move |ask| {
        record.borrow_mut().push(ask.clone());
        vec![answer.borrow().clone()]
    }));
    export_card::job::reveal_with(&ctl, Rc::new(|_| {}));
    // B, a 4:3 scan cropped as a roll's frame was: its row grows to its height, the file to the size said.
    let mut cropped = oni_settings::PipelineSettings::default();
    cropped.geometry.crop = oni_settings::cg::Rect { x: 0.151131, y: 0.159713, width: 0.730397, height: 0.729485 };
    let settings = |n: &str| if n == "B.tif" { cropped.clone() } else { Default::default() };
    let all: Vec<_> = names.iter().map(|n| oni_app::export::Target { path: folder.join(n), settings: settings(n) }).collect();
    let targets = all[..3].to_vec();
    ctl.enter_export(targets.clone());
    rig.until("the lines and sizes", LONG, || export_card::card(&ctl).rows.iter().all(Option::is_some) && e.get_dims().as_str() != "—");
    assert_eq!(e.get_dims(), "1500 × 1000 px", "a batch's size is its first photo's");

    // One sheet: three frames a tile wide, 30 px apart, as tall as B, a 60 px margin around them, and
    // under the row the band of their numbers and the margin; no logo under less than a row.
    assert_eq!(e.get_sheet_tiles().row_count(), 0, "twin: separate photos show their lines");
    e.invoke_sheet_set(true);
    rig.until("the sheet's size", LONG, || e.get_dims().as_str() == "3252 × 948 px");
    assert_eq!(e.get_key_title(), "EXPORT CONTACT SHEET");
    // The column shows the sheet as the file lays it, fitted to its room: three tiles in a row,
    // numbered, on the sheet's proportions, no logo under them.
    // Laid out once drawn: the room the column gives the sheet is read then.
    let _ = rig.drawn();
    rig.until("the sheet in the column", LONG, || e.get_sheet_tiles().row_count() == 3);
    let tiles: Vec<_> = (0..3).filter_map(|i| e.get_sheet_tiles().row_data(i)).collect();
    let ground = e.get_sheet_ground();
    assert!(((ground.width / ground.height) as f64 - 3252.0 / 948.0).abs() < 0.01, "{ground:?}");
    assert!(tiles[0].x < tiles[1].x && tiles[1].x < tiles[2].x && (tiles[0].width - tiles[2].width).abs() < 0.5, "one row, a tile wide each");
    assert_eq!(tiles.iter().map(|t| t.number.to_string()).collect::<Vec<_>>(), ["1", "2", "3"]);
    assert!(e.get_sheet_logo().size().width == 0 && e.get_sheet_logo_box().height == 0.0, "no logo under three photos");
    e.invoke_export();
    rig.until("the sheet written", LONG, || export_card::job::job(&ctl).is_some_and(|j| j.2.is_some()));
    assert_eq!(export_card::job::job(&ctl), Some((1, 1, Some(End::Written))));
    assert_eq!(*asked.borrow(), [Ask::Save { name: "Contact Sheet.jpg".into(), extension: "jpg".into() }], "one name asked, no folder");
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 1, "twin: one file, not one a photo");

    let sheet = image::open(out.join("Contact Sheet.jpg")).expect("the sheet decodes").to_rgb8();
    assert_eq!(sheet.dimensions(), (3252, 948), "the size the panel said");
    let px = |x: u32, y: u32| sheet.get_pixel(x, y).0;
    let black = |p: [u8; 3]| p.iter().all(|v| *v <= 8);
    assert!(black(px(60 + 1024 + 15, 580)) && black(px(10, 10)), "the gaps and the margin are black: {:?}", px(60 + 1024 + 15, 580));
    assert!(!black(px(60 + 512, 640)), "twin: a picture is not black");
    let count = |xs: std::ops::Range<u32>, ys: std::ops::Range<u32>, f: &dyn Fn([u8; 3]) -> bool| xs.flat_map(|x| ys.clone().map(move |y| (x, y))).filter(|&(x, y)| f(px(x, y))).count();
    let orange = |p: [u8; 3]| p[0] > 180 && p[1] > 90 && p[1] < 200 && p[2] < 90;
    let white = |p: [u8; 3]| p.iter().all(|v| *v > 200);
    // Under the first picture, centred in B's row, its number in orange: red over green over blue.
    let bottom = 60 + (767 - 683) / 2 + 683;
    let numbered = count(60..260, bottom..bottom + 60, &orange);
    assert!(numbered > 100, "the first number is orange under its picture: {numbered} pixels");
    // Under the row's numbers the margin alone, black, and above the row.
    assert_eq!(count(0..3252, 892..948, &|p| !black(p)) + count(0..3252, 0..56, &|p| !black(p)), 0, "the margins under the numbers and above the row");

    // The same three as positives: the same black, the numbers orange.
    *file.borrow_mut() = out.join("Positives.jpg");
    let positive = |mut t: oni_app::export::Target| {
        t.settings.mode = oni_settings::ConversionMode::Positive;
        t
    };
    ctl.enter_export(targets.into_iter().map(positive).collect());
    e.invoke_sheet_set(true);
    rig.until("the positives' sheet", LONG, || e.get_dims().as_str() == "3252 × 948 px");
    e.invoke_export();
    rig.until("the positives written", LONG, || export_card::job::job(&ctl).is_some_and(|j| j.2.is_some()));
    assert_eq!(export_card::job::job(&ctl), Some((1, 1, Some(End::Written))));
    let sheet = image::open(out.join("Positives.jpg")).expect("the positives decode").to_rgb8();
    let px = |x: u32, y: u32| sheet.get_pixel(x, y).0;
    let count = |xs: std::ops::Range<u32>, ys: std::ops::Range<u32>, f: &dyn Fn([u8; 3]) -> bool| xs.flat_map(|x| ys.clone().map(move |y| (x, y))).filter(|&(x, y)| f(px(x, y))).count();
    assert!(black(px(60 + 1024 + 15, 580)) && black(px(10, 10)), "positives lie on black too: {:?}", px(60 + 1024 + 15, 580));
    let numbered = count(60..260, bottom..bottom + 60, &orange);
    assert!(numbered > 100, "numbered in orange: {numbered} pixels");

    // Five photos fill a row: the white logo four margins under it, across the first column, nothing
    // beside it, and the margin under it.
    *file.borrow_mut() = out.join("Five.jpg");
    ctl.enter_export(all.clone());
    e.invoke_sheet_set(true);
    rig.until("the five's sheet", LONG, || e.get_dims().as_str() == "5360 × 1398 px");
    let _ = rig.drawn();
    rig.until("the five's logo in the column", LONG, || e.get_sheet_logo().size().width > 0);
    e.invoke_export();
    rig.until("the five written", LONG, || export_card::job::job(&ctl).is_some_and(|j| j.2.is_some()));
    assert_eq!(export_card::job::job(&ctl), Some((1, 1, Some(End::Written))));
    let sheet = image::open(out.join("Five.jpg")).expect("the five decode").to_rgb8();
    let px = |x: u32, y: u32| sheet.get_pixel(x, y).0;
    let count = |xs: std::ops::Range<u32>, ys: std::ops::Range<u32>, f: &dyn Fn([u8; 3]) -> bool| xs.flat_map(|x| ys.clone().map(move |y| (x, y))).filter(|&(x, y)| f(px(x, y))).count();
    assert_eq!(count(0..5360, 896..1064, &|p| !black(p)), 0, "the foot under the numbers");
    assert!(count(60..1084, 1067..1337, &white) > 20_000, "the logo");
    assert_eq!(count(1114..5300, 1067..1337, &|p| !black(p)), 0, "twin: nothing beside the logo");
    assert_eq!(count(0..5360, 1344..1398, &|p| !black(p)), 0, "twin: the margin under it");

    // Export on the gallery's five photos opens on separate photos; Export As lays them on one sheet,
    // and the next export opens on separate photos again.
    rig.until("the surface left", LONG, || !e.get_shown());
    if !w.global::<oni_app::App>().get_gallery() {
        ctl.act(oni_app::Act::ToggleMode);
    }
    rig.until("the gallery", LONG, || w.global::<oni_app::App>().get_gallery());
    lib.invoke_tile_clicked(0, false, false);
    for i in 1..names.len() as i32 {
        lib.invoke_tile_clicked(i, true, false);
    }
    rig.pump();
    assert_eq!(w.global::<oni_app::Bar>().get_targets(), names.len() as i32, "the bar counts the five");
    ctl.act(oni_app::Act::Export);
    rig.until("the export on separate photos", LONG, || e.get_shown());
    assert!(!e.get_sheet(), "export writes each photo first");
    e.invoke_sheet_set(true);
    rig.until("the export on one sheet", LONG, || e.get_sheet());
    ctl.leave_export();
    ctl.act(oni_app::Act::Export);
    rig.until("the export on separate photos again", LONG, || e.get_shown());
    assert!(!e.get_sheet(), "twin: the sheet chosen once is not the next export's");
    // The contact-sheet key on the roll picked whole leaves the photo last picked alone, then picks the
    // roll again.
    ctl.leave_export();
    w.global::<oni_app::Bar>().invoke_act(oni_app::Act::SelectAll);
    assert_eq!(ctl.state.borrow().picked().len(), 1);
    w.global::<oni_app::Bar>().invoke_act(oni_app::Act::SelectAll);
    assert_eq!(ctl.state.borrow().picked().len(), names.len(), "twin: one photo picked, the key picks its roll");
    let _ = std::fs::remove_dir_all(&root);
}
