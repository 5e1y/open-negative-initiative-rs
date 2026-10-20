//! The Look Lab in the window: on the Frontier, a drag on the blues' hue turns the drawn image, one undo
//! takes it back to the measured machine, and a double click rests a slider on it.

use oni_app::app::{Options, Shell};
use oni_app::{App, Bar, Lab, Library, LookLab};
use oni_core::{Archive, EditorStore};
use oni_settings::frontier_lab::FrontierLab;
use slint::platform::WindowEvent;
use slint::{ComponentHandle, Model, PhysicalSize};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

const FRAME: &str = "DSC_6442.NEF";

/// The frame as a hard link beside a copy of its sidecar, in a fresh folder.
fn mirror(roll: &Path, root: &Path) -> PathBuf {
    let dir = root.join("Roll");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::hard_link(roll.join(FRAME), dir.join(FRAME)).unwrap();
    let sidecar = format!("{FRAME}.json");
    if roll.join(&sidecar).exists() {
        std::fs::copy(roll.join(&sidecar), dir.join(&sidecar)).unwrap();
    }
    dir
}

/// The canvas's pixels as a shot draws them, once two shots running agree, the panel left out.
fn pixels(rig: &common::Rig, name: &str) -> Vec<u8> {
    let read = || {
        rig.settle();
        let path = rig.shot(name);
        let mut reader = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap())).read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buf).unwrap();
        let (w, row) = (info.width as usize, info.color_type.samples());
        (CANVAS.1..CANVAS.3).flat_map(|y| buf[(y * w + CANVAS.0) * row..(y * w + CANVAS.2) * row].to_vec()).collect::<Vec<u8>>()
    };
    let mut last = read();
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(200));
        let next = read();
        if next == last {
            return next;
        }
        last = next;
    }
    last
}

/// The canvas card in a 1280 × 800 window: left, top, right, bottom.
const CANVAS: (usize, usize, usize, usize) = (270, 130, 930, 760);

/// The widest gap between the two canvases' 16 px block means, so the display's dither averages out.
fn gap(a: &[u8], b: &[u8]) -> f64 {
    let (w, h) = (CANVAS.2 - CANVAS.0, CANVAS.3 - CANVAS.1);
    let n = a.len() / (w * h);
    let mut worst = 0.0f64;
    for (by, bx, c) in (0..h - 16).step_by(16).flat_map(|y| (0..w - 16).step_by(16).flat_map(move |x| (0..3).map(move |c| (y, x, c)))) {
        let at = |y: usize, x: usize| ((by + y) * w + bx + x) * n + c;
        let sum: f64 = (0..16).flat_map(|y| (0..16).map(move |x| (y, x))).map(|(y, x)| f64::from(a[at(y, x)]) - f64::from(b[at(y, x)])).sum();
        worst = worst.max((sum / 256.0).abs());
    }
    worst
}

pub(crate) fn a_drag_in_the_look_lab_turns_the_frontier_and_one_undo_returns_to_the_machine() {
    let roll = oni_fixtures::folder(oni_fixtures::RAW_DIR);
    if !roll.join(FRAME).exists() {
        return eprintln!("roll #8 is absent: the Look Lab is not exercised");
    }
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the window is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-look-lab-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let folder = mirror(&roll, &root);
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    shell.open_folder(&folder);
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    rig.until("the library read", Duration::from_secs(120), || ctl.state.borrow().read_done());
    w.global::<Library>().invoke_tile_opened(0);
    rig.until("the first render", Duration::from_secs(300), || ctl.state.borrow().renders() >= 1);
    w.global::<Bar>().invoke_act("face-scanner".into());
    w.global::<Lab>().invoke_machine_picked(1);
    w.global::<oni_app::Notice>().invoke_close();
    let lab = || ctl.state.borrow().store().settings().frontier_lab;
    let renders = ctl.state.borrow().renders();
    rig.until("the Frontier render", Duration::from_secs(120), || ctl.state.borrow().renders() > renders);
    rig.settle();
    let machine = pixels(&rig, "look-lab-machine");
    assert!(!w.global::<App>().get_gallery() && lab().is_neutral());

    // The blues' hue dragged: the frame's edit, a render, the image turned.
    let g = w.global::<LookLab>();
    g.invoke_pick_sector(8);
    let renders = ctl.state.borrow().renders();
    assert!(!g.invoke_press(0, false), "a first press is no double click");
    for k in 1..=10 {
        g.invoke_drag(0, -k as f32 * 6.0, 300.0, false);
        rig.pump();
        std::thread::sleep(Duration::from_millis(10));
    }
    g.invoke_release(0);
    rig.pump();
    let turned = lab().hue[8];
    assert!((turned + 12.0).abs() < 0.05, "60 px of 300 on ±30° is −12°: {turned}");
    let effect = oni_pipeline::look_lab::FITTED.frontier.hue[8] as f32 + turned;
    assert_eq!(g.get_readouts().row_data(0).unwrap(), format!("{effect:+.1}°"), "the machine's effect, moved");
    assert!(g.get_sector_marked().row_data(8).unwrap() && g.get_modified().row_data(0).unwrap());
    rig.until("the edited render", Duration::from_secs(120), || ctl.state.borrow().renders() > renders);
    rig.settle();
    let edited = pixels(&rig, "look-lab-blues-turned");
    assert!(gap(&machine, &edited) > 2.0, "the drawn image turned: {}", gap(&machine, &edited));

    // One undo returns the whole drag to the machine's render.
    rig.chord(slint::platform::Key::Control, "z");
    assert_eq!(lab(), FrontierLab::default(), "the drag was one step");
    rig.settle();
    let undone = pixels(&rig, "look-lab-undone");
    assert!(gap(&machine, &undone) < 1.0, "back on the machine: {}", gap(&machine, &undone));
    // Twin: a double click rests an edited slider on the machine rather than starting a drag.
    g.invoke_step(0, -10);
    rig.pump();
    assert!(lab().hue[8] < -3.0);
    g.invoke_press(0, false);
    g.invoke_release(0);
    assert!(g.invoke_press(0, false), "the second press within the interval is a double click");
    rig.pump();
    assert_eq!(lab().hue[8], 0.0);

    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
