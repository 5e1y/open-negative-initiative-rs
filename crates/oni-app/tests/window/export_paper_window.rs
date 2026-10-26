//! A paper profile end to end on the headless window: loaded through Load a Profile…, the canvas
//! proofed onto the paper, Paper Correction solved by its key, and laid by the export on a paper alone.

use oni_app::app::export_card::{self, Ask};
use oni_app::app::{Options, Shell};
use oni_app::display::Screen;
use oni_app::{Exporting, MainWindow};
use oni_core::{Archive, EditorStore};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, PhysicalSize};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use crate::common;
#[path = "../../../oni-colour/tests/common/mod.rs"]
mod stand_in;

const LONG: Duration = Duration::from_secs(600);

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oni-app-paper-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Roll #8's frames as hard links beside copies of their sidecars and marker, so no sidecar is written
/// beside the originals and a contact sheet made in the roll never takes the first tile.
fn mirror(roll: &Path, into: &Path) -> PathBuf {
    let dir = into.join("Roll #8");
    std::fs::create_dir_all(&dir).unwrap();
    for entry in std::fs::read_dir(roll).unwrap() {
        let from = entry.unwrap().path();
        let name = from.file_name().unwrap().to_string_lossy();
        if !name.starts_with("DSC_") && name != ".oni-roll.json" {
            continue;
        }
        let to = dir.join(from.file_name().unwrap());
        if from.extension().is_some_and(|e| e == "json") {
            std::fs::copy(&from, &to).unwrap();
        } else {
            std::fs::hard_link(&from, &to).unwrap();
        }
    }
    dir
}

/// Pumps until no render has landed for four seconds, longer than one render takes here.
fn quiet(rig: &common::Rig, renders: impl Fn() -> usize) {
    loop {
        let n = renders();
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(4) {
            rig.pump();
            std::thread::sleep(Duration::from_millis(20));
        }
        if renders() == n {
            return;
        }
    }
}

fn frame(w: &MainWindow) -> Vec<u8> {
    w.global::<oni_app::Canvas>().get_frame().to_rgba8().expect("the canvas holds pixels").as_bytes().to_vec()
}

/// Pixels whose channels differ by more than the dither's reach.
fn differing(a: &[u8], b: &[u8]) -> usize {
    a.chunks_exact(4).zip(b.chunks_exact(4)).filter(|(p, q)| (0..3).any(|c| p[c].abs_diff(q[c]) > 3)).count()
}

/// A JPEG's size, profile and RGB pixels, read by an independent decoder.
fn jpeg(path: &Path) -> (u16, u16, Vec<u8>, Vec<u8>) {
    let mut d = jpeg_decoder::Decoder::new(std::fs::File::open(path).unwrap());
    let pixels = d.decode().unwrap();
    let info = d.info().unwrap();
    (info.width, info.height, d.icc_profile().expect("the JPEG declares a profile"), pixels)
}

/// The largest mean difference over a 12 × 12 grid of blocks, 0–1.
fn worst_block(w: usize, h: usize, a: &[u8], b: &[u8]) -> f64 {
    let n = 12;
    let mut worst: f64 = 0.0;
    for by in 0..n {
        for bx in 0..n {
            let mut sum = [0.0f64; 3];
            let mut count = 0.0;
            for y in by * h / n..(by + 1) * h / n {
                for x in bx * w / n..(bx + 1) * w / n {
                    let i = (y * w + x) * 3;
                    (0..3).for_each(|c| sum[c] += f64::from(a[i + c]) - f64::from(b[i + c]));
                    count += 1.0;
                }
            }
            worst = sum.iter().fold(worst, |m, s| m.max((s / count / 255.0).abs()));
        }
    }
    worst
}

pub(crate) fn a_paper_proofs_the_canvas_and_its_correction_is_laid_on_the_paper_alone() {
    let roll = oni_fixtures::folder(oni_fixtures::RAW_DIR);
    if !roll.join("DSC_6432.NEF").exists() {
        eprintln!("roll #8 is absent: the paper proof is not exercised");
        return;
    }
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the paper proof is not exercised"),
    };
    let shots = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots");
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let window = rig.window.clone();
    let root = scratch();
    let folder = mirror(&roll, &root);
    let support = root.join("support");
    let profiles = root.join("Profiles");
    let out = root.join("out");
    std::fs::create_dir_all(&out).unwrap();
    let pearl = root.join("Pearl.icc");
    stand_in::write_stand_in(&pearl, "Pearl", 0.02, Some(stand_in::bluish()), false);

    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { screen: Screen::DisplayP3, canvas: false, support: support.clone(), caches: root.join("caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    window.set_size(PhysicalSize::new(1440, 900));
    shell.window.show().unwrap();
    shell.open_folder(&folder);
    let ctl = shell.ctl.clone();
    let w: &MainWindow = &shell.window;
    let e = w.global::<Exporting>();
    let asked: Rc<RefCell<Vec<Ask>>> = Rc::default();
    let answer: Rc<RefCell<Vec<PathBuf>>> = Rc::default();
    {
        let (asked, answer) = (asked.clone(), answer.clone());
        export_card::answer_with(&ctl, Rc::new(move |a: &Ask| {
            asked.borrow_mut().push(a.clone());
            answer.borrow().clone()
        }));
    }
    export_card::profiles_in(&ctl, profiles.clone());
    let renders = || ctl.state.borrow().renders();
    // A change is seen once a render after it has landed and none follows.
    let settled_after = |n: usize, what: &str| {
        rig.until(what, LONG, || renders() > n);
        quiet(&rig, renders);
    };
    let stops = || ctl.state.borrow().store().settings().paper_correction.stops;

    // The app opens on the gallery: the first photo is ringed and taken into the editor by ⌘G.
    rig.until("the library read", LONG, || ctl.state.borrow().read_done());
    w.global::<oni_app::Library>().invoke_tile_clicked(0, false, false);
    rig.pump();
    rig.chord(slint::platform::Key::Control, "g");
    assert!(ctl.state.borrow().open_path().is_some_and(|p| p.ends_with("DSC_6432.NEF")), "the first photo opens");
    rig.until("the first render", LONG, || renders() >= 1);
    quiet(&rig, renders);
    let screen = frame(w);
    let pixels = screen.len() / 4;
    assert!(!e.get_on_paper(), "a built-in space shows no Paper Correction");
    // Twin: the key does nothing off a paper.
    e.invoke_auto_paper();
    assert_eq!(stops(), [0.0; 3]);

    // Load a Profile… installs the file in the user's profiles and lands on it.
    *answer.borrow_mut() = vec![pearl.clone()];
    let n = renders();
    e.invoke_load_profile();
    assert_eq!(asked.borrow().last(), Some(&Ask::Profiles));
    assert!(profiles.join("Pearl.icc").exists(), "the profile is installed where the profiles live");
    assert!(e.get_on_paper() && e.get_on_profile());
    assert_eq!(e.get_space_tab(), 4);
    assert!(e.get_chip_title().contains("Pearl"), "{}", e.get_chip_title());
    settled_after(n, "the paper proof");
    let paper = frame(w);
    let moved = differing(&screen, &paper);
    println!("the paper proof moves {moved} of {pixels} pixels");
    assert!(moved * 10 > pixels, "the canvas is not proofed onto the paper: {moved}");

    // Colour only drops the paper's white and black to the screen's, and is remembered.
    let n = renders();
    e.invoke_colour_only_set(true);
    settled_after(n, "colour only");
    let colour_only = frame(w);
    assert!(differing(&paper, &colour_only) * 10 > pixels, "colour only shows the paper as it was");
    let n = renders();
    e.invoke_colour_only_set(false);
    settled_after(n, "colour only off");
    assert!(differing(&paper, &frame(w)) * 1000 < pixels, "colour only off is the paper proof again");

    // Auto Paper Correction reads the profile alone and writes the gains it shows.
    let n = renders();
    e.invoke_auto_paper();
    let solved = stops();
    println!("the paper's correction: {solved:?}");
    assert!(solved.iter().any(|s| *s < -0.01), "{solved:?}");
    assert!(solved.iter().any(|s| *s == 0.0), "the strongest channel is held at 0: {solved:?}");
    settled_after(n, "the corrected proof");
    let corrected = frame(w);
    assert!(differing(&paper, &corrected) * 10 > pixels, "the correction moves nothing on the paper");

    // The export on the paper: a JPEG declaring the paper's own profile, the correction laid.
    // 2000 px on the long edge unless a preset sized it: a full frame does not fit the check's memory.
    let export = |name: &str, sized: bool| -> PathBuf {
        ctl.act(oni_app::Act::Export);
        assert!(e.get_shown());
        rig.until("the lines", LONG, || export_card::lines(w).iter().all(|l| l.ready) && e.get_size_enabled());
        if sized {
            e.invoke_size_typed("2000".into(), e.get_height_text());
            assert_eq!(e.get_width_text(), "2000");
        }
        let destination = out.join(name);
        *answer.borrow_mut() = vec![destination.clone()];
        e.invoke_export();
        rig.until(name, LONG, || !export_card::card(&ctl).exporting && !e.get_shown());
        assert!(destination.exists(), "{name} written");
        destination
    };
    assert_eq!(e.get_format(), 1, "the export opens on a JPEG");
    let on_paper = export("paper.jpg", true);
    let (pw, ph, icc, paper_pixels) = jpeg(&on_paper);
    assert_eq!(icc, std::fs::read(&pearl).unwrap(), "the file declares the paper's profile");

    // Instagram writes every row: a JPEG in sRGB, 1440 across the narrow way, the proof following.
    ctl.act(oni_app::Act::Export);
    let n = renders();
    e.invoke_preset(0);
    assert_eq!((e.get_format(), e.get_space_tab()), (1, 0));
    assert!(!e.get_on_paper());
    e.invoke_cancel();
    settled_after(n, "the sRGB proof");
    let srgb_corrected = frame(w);
    let in_srgb = export("srgb.jpg", false);
    let (sw, sh, srgb_icc, srgb_pixels) = jpeg(&in_srgb);
    assert_eq!(sw.min(sh), 1440);
    assert_eq!(Some(srgb_icc), oni_colour::Space::Srgb.icc_data());

    // The correction undone: the sRGB canvas and file stay as they were, the paper's move.
    let n = renders();
    ctl.act(oni_app::Act::Undo);
    assert_eq!(stops(), [0.0; 3]);
    settled_after(n, "the correction undone");
    assert!(differing(&srgb_corrected, &frame(w)) * 1000 < pixels, "the correction moved a canvas off the paper");
    let (_, _, _, srgb_plain) = jpeg(&export("srgb-plain.jpg", false));
    let off_paper = worst_block(sw as usize, sh as usize, &srgb_pixels, &srgb_plain);
    println!("sRGB with and without the correction: worst block {off_paper:.4}");
    assert!(off_paper < 0.002, "the correction is laid off the paper: {off_paper}");
    ctl.act(oni_app::Act::Export);
    e.invoke_space_picked(4);
    assert!(e.get_on_paper(), "Custom lands on the profile last chosen");
    e.invoke_cancel();
    let (qw, qh, _, paper_plain) = jpeg(&export("paper-plain.jpg", true));
    assert_eq!((qw, qh), (pw, ph));
    let laid = worst_block(pw as usize, ph as usize, &paper_pixels, &paper_plain);
    println!("the paper with and without the correction: worst block {laid:.4}");
    assert!(laid > 0.01, "the export does not lay the correction on the paper: {laid}");
    let _ = std::fs::remove_dir_all(&root);
}
