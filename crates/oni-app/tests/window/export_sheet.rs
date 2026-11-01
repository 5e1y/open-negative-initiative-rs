//! Three photos exported as one contact sheet: one file named for it, the sheet's size, black in its
//! gaps, each frame's number in orange under its picture.

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
        common::graded_tiff(&folder.join(n), 150, 100, 0.8 - 0.2 * i as f32);
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
    let (record, file) = (asked.clone(), out.join("Contact Sheet.jpg"));
    export_card::answer_with(&ctl, Rc::new(move |ask| {
        record.borrow_mut().push(ask.clone());
        vec![file.clone()]
    }));
    export_card::job::reveal_with(&ctl, Rc::new(|_| {}));
    let targets = names.iter().map(|n| oni_app::export::Target { path: folder.join(n), settings: Default::default() }).collect();
    ctl.enter_export(targets);
    rig.until("the lines and sizes", LONG, || export_card::card(&ctl).rows.iter().all(Option::is_some) && e.get_dims().as_str() != "—");
    assert_eq!(e.get_dims(), "150 × 100", "a batch's size is its first photo's");

    // One sheet: three 3:2 frames a tile wide, 30 px apart, a 60 px margin around them.
    e.invoke_sheet_set(true);
    rig.until("the sheet's size", LONG, || e.get_dims().as_str() == "3252 × 803");
    assert_eq!(e.get_key_title(), "EXPORT CONTACT SHEET");
    e.invoke_export();
    rig.until("the sheet written", LONG, || export_card::job::job(&ctl).is_some_and(|j| j.2.is_some()));
    assert_eq!(export_card::job::job(&ctl), Some((1, 1, Some(End::Written))));
    assert_eq!(*asked.borrow(), [Ask::Save { name: "Contact Sheet.jpg".into(), extension: "jpg".into() }], "one name asked, no folder");
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 1, "twin: one file, not one a photo");

    let sheet = image::open(out.join("Contact Sheet.jpg")).expect("the sheet decodes").to_rgb8();
    assert_eq!(sheet.dimensions(), (3252, 803));
    let px = |x: u32, y: u32| sheet.get_pixel(x, y).0;
    let black = |p: [u8; 3]| p.iter().all(|v| *v <= 8);
    assert!(black(px(60 + 1024 + 15, 400)) && black(px(10, 10)), "the gaps and the margin are black: {:?}", px(60 + 1024 + 15, 400));
    assert!(!black(px(60 + 512, 60 + 341)), "twin: a picture is not black");
    // Under the first picture, its number in orange: red over green over blue.
    let band = (60..60 + 200).flat_map(|x| (60 + 683..803).map(move |y| (x, y)));
    let orange = band.map(|(x, y)| px(x, y)).filter(|p| p[0] > 180 && p[1] > 90 && p[1] < 200 && p[2] < 90).count();
    assert!(orange > 100, "the first number is orange under its picture: {orange} pixels");
    let _ = std::fs::remove_dir_all(&root);
}
