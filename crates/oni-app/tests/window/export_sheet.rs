//! Three photos exported as one contact sheet: one file named for it, the sheet's size, black in its
//! gaps, each frame's number in orange under its picture, a foot of four margins and the logo's band;
//! the same three as positives on white, numbered in dark red, the logo black.

use oni_app::app::export_card::{self, Ask, job::End};
use oni_app::app::{Options, Shell};
use oni_app::{Exporting, Library};
use oni_core::{Archive, EditorStore};
use slint::ComponentHandle;
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
    let names = ["A.tif", "B.tif", "C.tif"];
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
    let targets: Vec<_> = names.iter().map(|n| oni_app::export::Target { path: folder.join(n), settings: settings(n) }).collect();
    ctl.enter_export(targets.clone());
    rig.until("the lines and sizes", LONG, || export_card::card(&ctl).rows.iter().all(Option::is_some) && e.get_dims().as_str() != "—");
    assert_eq!(e.get_dims(), "1500 × 1000", "a batch's size is its first photo's");

    // One sheet: three frames a tile wide, 30 px apart, as tall as B, a 60 px margin around them, four
    // under them, then the 270 px logo band.
    e.invoke_sheet_set(true);
    rig.until("the sheet's size", LONG, || e.get_dims().as_str() == "3252 × 1398");
    assert_eq!(e.get_key_title(), "EXPORT CONTACT SHEET");
    e.invoke_export();
    rig.until("the sheet written", LONG, || export_card::job::job(&ctl).is_some_and(|j| j.2.is_some()));
    assert_eq!(export_card::job::job(&ctl), Some((1, 1, Some(End::Written))));
    assert_eq!(*asked.borrow(), [Ask::Save { name: "Contact Sheet.jpg".into(), extension: "jpg".into() }], "one name asked, no folder");
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 1, "twin: one file, not one a photo");

    let sheet = image::open(out.join("Contact Sheet.jpg")).expect("the sheet decodes").to_rgb8();
    assert_eq!(sheet.dimensions(), (3252, 1398), "the size the panel said");
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
    // The foot under the row's numbers, black; then the white logo across the first column, nothing
    // beside it, and the margin under it.
    assert_eq!(count(0..3252, 896..1064, &|p| !black(p)), 0, "the foot under the numbers");
    assert!(count(60..1084, 1067..1337, &white) > 20_000, "the logo");
    assert_eq!(count(1114..3192, 1067..1337, &|p| !black(p)), 0, "twin: nothing beside the logo");
    assert_eq!(count(0..3252, 1344..1398, &|p| !black(p)) + count(0..3252, 0..56, &|p| !black(p)), 0, "twin: the margins under it and above the row");

    // The same three as positives: a white sheet, the numbers dark red, the logo black.
    *file.borrow_mut() = out.join("Positives.jpg");
    let positive = |mut t: oni_app::export::Target| {
        t.settings.mode = oni_settings::ConversionMode::Positive;
        t
    };
    ctl.enter_export(targets.into_iter().map(positive).collect());
    e.invoke_sheet_set(true);
    rig.until("the positives' sheet", LONG, || e.get_dims().as_str() == "3252 × 1398");
    e.invoke_export();
    rig.until("the positives written", LONG, || export_card::job::job(&ctl).is_some_and(|j| j.2.is_some()));
    assert_eq!(export_card::job::job(&ctl), Some((1, 1, Some(End::Written))));
    let sheet = image::open(out.join("Positives.jpg")).expect("the positives decode").to_rgb8();
    let px = |x: u32, y: u32| sheet.get_pixel(x, y).0;
    let count = |xs: std::ops::Range<u32>, ys: std::ops::Range<u32>, f: &dyn Fn([u8; 3]) -> bool| xs.flat_map(|x| ys.clone().map(move |y| (x, y))).filter(|&(x, y)| f(px(x, y))).count();
    let paper = |p: [u8; 3]| p.iter().all(|v| *v >= 247);
    let dark_red = |p: [u8; 3]| p[0] > 90 && p[0] < 200 && p[1] < 80 && p[2] < 80;
    let dark = |p: [u8; 3]| p.iter().all(|v| *v < 60);
    assert!(paper(px(60 + 1024 + 15, 580)) && paper(px(10, 10)), "the gaps and the margin are white: {:?}", px(60 + 1024 + 15, 580));
    let numbered = count(60..260, bottom..bottom + 60, &dark_red);
    assert!(numbered > 100, "the first number is dark red: {numbered} pixels");
    assert!(count(60..1084, 1067..1337, &dark) > 20_000, "the logo black");
    assert_eq!(count(0..3252, 896..1064, &|p| !paper(p)) + count(60..260, bottom..bottom + 60, &orange), 0, "twin: the foot white, no orange number");
    let _ = std::fs::remove_dir_all(&root);
}
