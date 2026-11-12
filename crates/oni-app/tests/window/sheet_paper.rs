//! A contact sheet on a paper lays each frame's Paper Correction on its own tile alone, as the panel
//! shows it: the other frames, the gaps and the margins keep what they are with no correction at all.

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
#[path = "../../../oni-colour/tests/common/mod.rs"]
mod stand_in;

const LONG: Duration = Duration::from_secs(120);

/// The mean of a block of `sheet` `half` pixels around `at`, 0–255 a channel.
fn block(sheet: &image::RgbImage, at: (u32, u32), half: u32) -> [f64; 3] {
    let mut sum = [0.0; 3];
    for y in at.1 - half..at.1 + half {
        for x in at.0 - half..at.0 + half {
            let p = sheet.get_pixel(x, y).0;
            (0..3).for_each(|c| sum[c] += f64::from(p[c]));
        }
    }
    sum.map(|v| v / f64::from(4 * half * half))
}

fn apart(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|c| (a[c] - b[c]).abs()).fold(0.0, f64::max)
}

pub(crate) fn each_frame_of_a_sheet_on_a_paper_takes_its_own_correction() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the sheet on a paper is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-sheet-paper-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, out, profiles) = (root.join("Scans"), root.join("Out"), root.join("Profiles"));
    std::fs::create_dir_all(&out).unwrap();
    let names = ["A.tif", "B.tif", "C.tif"];
    for (i, n) in names.iter().enumerate() {
        common::graded_tiff(&folder.join(n), 1500, 1000, 0.8 - 0.2 * i as f32);
    }
    let pearl = root.join("Pearl.icc");
    stand_in::write_stand_in(&pearl, "Pearl", 0.02, Some(stand_in::bluish()), false);
    let folder = PathBuf::from(oni_library::paths::canonical(&folder));
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, engine, options).unwrap();
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
    let answer: Rc<RefCell<PathBuf>> = Rc::new(RefCell::new(pearl.clone()));
    let given = answer.clone();
    export_card::answer_with(&ctl, Rc::new(move |_: &Ask| vec![given.borrow().clone()]));
    export_card::profiles_in(&ctl, profiles);
    // A sheet of three on a paper, the first frame's blue a stop down by its Paper Correction.
    let sheet = |first: [f32; 3], name: &str| -> image::RgbImage {
        let targets: Vec<_> = names
            .iter()
            .enumerate()
            .map(|(i, n)| {
                // Positives, bright enough for a stop of blue to tell.
                let mut settings = oni_settings::PipelineSettings { mode: oni_settings::ConversionMode::Positive, ..Default::default() };
                if i == 0 {
                    settings.paper_correction.stops = first;
                }
                oni_app::export::Target { path: folder.join(n), settings }
            })
            .collect();
        ctl.enter_export(targets);
        rig.until("the lines and sizes", LONG, || export_card::card(&ctl).rows.iter().all(Option::is_some) && e.get_dims().as_str() != "—");
        if !e.get_on_paper() {
            *answer.borrow_mut() = pearl.clone();
            e.invoke_load_profile();
            assert!(e.get_on_paper(), "the export lands on the paper");
        }
        e.invoke_sheet_set(true);
        rig.until("the sheet's size", LONG, || e.get_dims().as_str().starts_with("3252 × "));
        *answer.borrow_mut() = out.join(name);
        e.invoke_export();
        rig.until("the sheet written", LONG, || export_card::job::job(&ctl).is_some_and(|j| j.2.is_some()));
        assert_eq!(export_card::job::job(&ctl), Some((1, 1, Some(End::Written))));
        rig.until("the surface left", LONG, || !e.get_shown());
        image::open(out.join(name)).expect("the sheet decodes").to_rgb8()
    };
    let corrected = sheet([0.0, 0.0, -1.0], "Corrected.jpg");
    let neutral = sheet([0.0; 3], "Neutral.jpg");
    assert_eq!(corrected.dimensions(), neutral.dimensions());
    let height = corrected.height();
    // Three tiles 1024 wide and 30 apart in a 60 px margin, a row of 683 under it.
    let (first, second, third) = ((60 + 512, 60 + 341), (60 + 1054 + 512, 60 + 341), (60 + 2108 + 512, 60 + 341));
    let (gap, margin) = ((60 + 1024 + 15, 60 + 341), (30, height / 2));
    for (what, at, half) in [("the second frame", second, 300), ("the third frame", third, 300), ("a gap", gap, 8), ("the margin", margin, 8)] {
        let (c, n) = (block(&corrected, at, half), block(&neutral, at, half));
        println!("{what}: {c:?} against {n:?}");
        assert!(apart(c, n) < 1.0, "{what} takes the first frame's correction: {c:?} against {n:?}");
    }
    // Twin: the first frame's own tile takes its correction, its blue down against its red.
    let (c, n) = (block(&corrected, first, 300), block(&neutral, first, 300));
    println!("the first frame: {c:?} against {n:?}");
    assert!(c[2] / c[0] < n[2] / n[0] - 0.05, "the first frame keeps its blue: {c:?} against {n:?}");
    let _ = std::fs::remove_dir_all(&root);
}
