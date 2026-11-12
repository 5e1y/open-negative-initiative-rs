//! A frame's opening decode overtaken by an edit naming another reading: the decode under the reading
//! left never reaches the canvas, and the one under the new reading is rendered once it lands.

use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::{Canvas, Library, PanelState, Store};
use oni_core::{Archive, EditorStore};
use oni_settings::SourceEncoding;
use slint::ComponentHandle;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::common;

/// The canvas's mean colour, 0 to 255 a channel.
fn mean(c: &Canvas) -> [f64; 3] {
    let frame = c.get_frame().to_rgba8().expect("the canvas holds pixels");
    let n = (frame.width() * frame.height()) as f64;
    let mut sum = [0.0; 3];
    for p in frame.as_bytes().chunks_exact(4) {
        for k in 0..3 {
            sum[k] += f64::from(p[k]);
        }
    }
    sum.map(|s| s / n)
}

fn apart(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|k| (a[k] - b[k]).abs()).fold(0.0, f64::max)
}

pub(crate) fn a_decode_under_a_reading_left_never_lands_on_the_canvas() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the opening's reading is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-opening-reading-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    // Wider than the preview's side, so the opening decode keeps no full resolution, and short so it
    // decodes in a few milliseconds.
    common::plain_tiff(&folder.join("A.tif"), oni_app::render::PREVIEW_SIDE + 48, 300, 0.5);
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, engine, options).unwrap();
    rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (c, panel, store) = (w.global::<Canvas>(), w.global::<PanelState>(), w.global::<Store>());
    let renders = || ctl.state.borrow().renders();
    let rendered_past = |n: usize, what: &str| rig.until(what, Duration::from_secs(60), || renders() > n && c.get_frame().size().width > 0);
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.settle();
    // The thumbnail's preview let go, so the opening decodes; every decode place held, so it waits
    // past its ticket's check while the reading changes.
    ctl.state.borrow().forget_previews();
    let held = [oni_app::decode_gate::slot(), oni_app::decode_gate::slot()];
    w.global::<Library>().invoke_tile_opened(0);
    let pump_for = |ms: u64| {
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(ms) {
            rig.pump();
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    pump_for(100);
    panel.invoke_encoding_picked(1);
    assert_eq!(ctl.state.borrow().store().settings().source_encoding, Some(SourceEncoding::Linear));
    assert_eq!(renders(), 0, "nothing renders before a decode lands");
    drop(held);
    rendered_past(0, "the opening's render");
    // Both decodes landed; the full resolution, built once the frame stayed a second, not yet, as its
    // render under the new reading would hide a preview decoded under the old one.
    pump_for(200);
    assert!(!ctl.state.borrow().full_built(), "the picture is the preview's");
    let shown = mean(&c);

    // The same settings rendered again on the negative the frame holds now: its own reading's.
    let n = renders();
    store.invoke_step(Field::Exposure.index(), 1);
    rendered_past(n, "a notch's render");
    let n = renders();
    store.invoke_step(Field::Exposure.index(), -1);
    rendered_past(n, "the notch back's render");
    rig.settle();
    let own = mean(&c);
    assert!(apart(shown, own) < 1.0, "the canvas shows the new reading's decode: {shown:?} against {own:?}");

    // Twin: the reading left renders another picture, so a decode under it on the canvas is seen.
    let n = renders();
    panel.invoke_encoding_picked(0);
    rendered_past(n, "the sRGB reading's render");
    rig.settle();
    let left = mean(&c);
    assert!(apart(left, own) > 8.0, "twin: the two readings render apart: {left:?} against {own:?}");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
