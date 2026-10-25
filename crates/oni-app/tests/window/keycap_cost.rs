//! What the keycaps cost: a screen's keys drawn once and never again while still, a press one key a
//! frame; the numbers are printed for the report.

use oni_app::KeyProbe;
use oni_app::app::{self, Options, Shell};
use oni_core::{Archive, EditorStore};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, PhysicalSize};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::common;

const SCALE: f32 = 2.0;

fn shots() -> PathBuf {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    shots
}

fn renders() -> u64 {
    oni_app::keycaps::stats().renders
}

/// `frames` frames drawn 8 ms apart, the timers run between them; the renders each one asked.
fn frames(rig: &common::Rig, frames: usize) -> Vec<u64> {
    (0..frames)
        .map(|_| {
            let before = renders();
            rig.pump();
            rig.drawn();
            std::thread::sleep(Duration::from_millis(8));
            renders() - before
        })
        .collect()
}

pub(crate) fn a_screen_s_keys_are_drawn_once_and_never_again_while_still() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the keys' cost is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-keycap-cost-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    common::plain_tiff(&folder.join("Roll").join("A.tif"), 48, 32, 0.3);
    common::plain_tiff(&folder.join("Roll").join("B.tif"), 48, 32, 0.5);
    let rig = common::rig(shots());
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: SCALE });
    rig.window.set_size(PhysicalSize::new((1440.0 * SCALE) as u32, (900.0 * SCALE) as u32));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    let tile = app::tiles(&shell.window).into_iter().find(|t| t.caption == "A").expect("A is shown");
    shell.window.global::<oni_app::Library>().invoke_tile_opened(tile.index);
    rig.settle();

    // The editor's first frame draws every key it shows.
    let start = Instant::now();
    rig.drawn();
    let (after, wall) = (oni_app::keycaps::stats(), start.elapsed());
    // Layouts read an image's size, so a key may be drawn before the first frame: counted from the start.
    let (drawn, asked) = (after.renders, after.renders + after.hits);
    println!(
        "a screen's keys at {SCALE}×: {asked} asked, {drawn} distinct rendered in {:.1} ms ({:.2} ms each, the frame {:.1} ms), {:.1} MB kept",
        after.time.as_secs_f64() * 1e3,
        after.time.as_secs_f64() * 1e3 / drawn.max(1) as f64,
        wall.as_secs_f64() * 1e3,
        after.bytes as f64 / (1 << 20) as f64,
    );
    assert!(asked > 10 && drawn > 0, "the editor shows its keys as keycaps ({asked} asked)");
    rig.shot("keycap-cost-editor");
    // Still, a hundred frames redraw the screen and render no key.
    let still: u64 = frames(&rig, 100).iter().sum();
    assert_eq!(still, 0, "a still key is never drawn again");
    // A slider's knob is a keycap that slides: dragged, then sprung home by an undo, it is drawn no more.
    let g = shell.window.global::<oni_app::Store>();
    let field = oni_app::fields::Field::Exposure;
    let (before, start) = (app::fraction(&g, field), renders());
    assert!(!g.invoke_press(field.index(), false), "the press starts a drag");
    let dragged: u64 = (1..=20)
        .map(|i| {
            g.invoke_drag(field.index(), i as f32 * 4.0, 300.0, false);
            frames(&rig, 1)[0]
        })
        .sum();
    g.invoke_release(field.index());
    // Twin: the knob did move, so a raster tied to its place would have shown here.
    assert!((app::fraction(&g, field) - before).abs() > 0.05, "the drag moves the knob");
    rig.chord(slint::platform::Key::Control, "z");
    let sprung: u64 = frames(&rig, 60).iter().sum();
    println!("a slider dragged 20 frames and sprung home over 60: {} knob renders", renders() - start);
    assert_eq!(dragged + sprung, 0, "a knob is drawn once, wherever it slides");
}

pub(crate) fn a_press_draws_one_key_a_frame_and_nothing_once_it_rests() {
    let rig = common::rig(shots());
    let probe = KeyProbe::new().unwrap();
    oni_app::keycaps::serve(&probe);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: SCALE });
    rig.window.set_size(PhysicalSize::new((200.0 * SCALE) as u32, (120.0 * SCALE) as u32));
    probe.show().unwrap();
    rig.settle();
    rig.drawn();
    let at = LogicalPosition::new(probe.get_key_x(), probe.get_key_y());
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: at });
    // The hover's sink, then the press and its release, each frame drawn.
    let hover = frames(&rig, 30);
    let start = oni_app::keycaps::stats();
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: at, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: at, button: PointerEventButton::Left });
    let press = frames(&rig, 40);
    let end = oni_app::keycaps::stats();
    let rest = frames(&rig, 30);
    let pressed: u64 = press.iter().sum();
    println!(
        "a press at {SCALE}×: {pressed} renders over {} frames, {:.2} ms each, the most in a frame {}; hover {} renders",
        press.len(),
        (end.time - start.time).as_secs_f64() * 1e3 / pressed.max(1) as f64,
        press.iter().max().unwrap(),
        hover.iter().sum::<u64>(),
    );
    assert!(press.iter().all(|&n| n <= 1), "one key a frame at most: {press:?}");
    // Twin: the press is drawn, not skipped.
    assert!(pressed > 2, "the press sinks the cap frame by frame: {press:?}");
    assert!(hover.iter().sum::<u64>() > 0, "the hover sinks it too");
    assert_eq!(rest[10..].iter().sum::<u64>(), 0, "risen and still, the key is drawn no more: {rest:?}");
    // A window moved to another screen draws its keys again at that screen's scale.
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(PhysicalSize::new(200, 120));
    assert!(frames(&rig, 3).iter().sum::<u64>() > 0, "a new scale draws the key again");
}
