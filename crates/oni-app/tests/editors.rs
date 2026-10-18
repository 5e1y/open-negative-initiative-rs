//! The editors in the window, driven by pointer events: a point dragged on the curves plot and a
//! handle dragged on the levels track reach the store at the hand's pace, as one history step each.

use oni_app::app::{Options, Shell};
use oni_app::editors::{self, Plot};
use oni_app::{App, Bar, Editors, MainWindow, PanelState};
use oni_core::{Archive, EditorStore};
use oni_settings::spectrogram::{Band, Spectrogram};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model, PhysicalSize};
use std::path::{Path, PathBuf};
use std::time::Duration;

mod common;

fn press(rig: &common::Rig, at: (f64, f64)) {
    let position = LogicalPosition::new(at.0 as f32, at.1 as f32);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
    rig.pump();
}

fn drag(rig: &common::Rig, from: (f64, f64), by: (f64, f64), steps: usize) {
    for k in 1..=steps {
        std::thread::sleep(Duration::from_millis(4));
        let t = k as f64 / steps as f64;
        let position = LogicalPosition::new((from.0 + by.0 * t) as f32, (from.1 + by.1 * t) as f32);
        rig.window.dispatch_event(WindowEvent::PointerMoved { position });
        rig.pump();
    }
}

fn release(rig: &common::Rig, at: (f64, f64)) {
    let position = LogicalPosition::new(at.0 as f32, at.1 as f32);
    rig.window.dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Left });
    rig.pump();
}

#[test]
fn a_point_and_a_handle_dragged_in_the_window_reach_the_store_as_one_step_each() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the window is not exercised"),
    };
    let shots = std::env::var_os("ONI_SHOTS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let dir = std::env::temp_dir().join(format!("oni-app-editors-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let frame = dir.join("frame.NEF").to_string_lossy().into_owned();
    let mut store = EditorStore::new(Archive::new(vec![frame.clone()], |_: &Path| true));
    store.open(&frame).unwrap();
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support: dir.join("support"), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.set_size(PhysicalSize::new(1280, 2200));
    shell.window.show().unwrap();
    // The app launches on the gallery; the store's open frame is shown in the editor.
    shell.window.global::<App>().set_gallery(false);
    let w: &MainWindow = &shell.window;
    let ctl = shell.ctl.clone();
    let g = w.global::<Editors>();

    // The Color tab with the Curves block open.
    w.global::<PanelState>().set_tab(2);
    g.set_curves_folded(false);
    // A drawn frame lays out the panel the editor brings in, as the app's next frame does.
    rig.shot("editors-opened");
    rig.settle();
    let origin = editors::origin(&ctl, Plot::Curves);
    let size = editors::size(&ctl, Plot::Curves);
    assert!(size.w > 200.0 && (size.w - size.h).abs() < 0.5, "the curves plot is a square: {size:?}");
    assert!(origin.1 + size.h < 2200.0, "the plot is on screen: {origin:?} {size:?}");
    println!("shot {}", rig.shot("editors-curves").display());

    // A press at the middle adds a point there; the drag carries it up by a fifth of the plot.
    let start = (origin.0 + size.w / 2.0, origin.1 + size.h / 2.0);
    let writes = editors::writes(&ctl);
    press(&rig, start);
    let curves = || ctl.state.borrow().store().settings().manual_curves.linked.clone();
    assert_eq!(curves().points.len(), 3, "the press added a point");
    // The editor draws the hand's value at once, ahead of the store's cadence.
    let lift = size.h / 5.0;
    rig.window.dispatch_event(WindowEvent::PointerMoved {
        position: LogicalPosition::new(start.0 as f32, (start.1 - lift / 2.0) as f32),
    });
    let drawn = g.get_curve_dots().row_data(1).unwrap();
    assert!((f64::from(drawn.y) - (size.h / 2.0 - lift / 2.0)).abs() < 0.5, "the dot follows the pointer: {}", drawn.y);
    assert!((curves().points[1].y - 0.5).abs() < 1e-3, "the store waits for the hand's cadence");
    // Thirty moves as fast as the pointer sends them: the store takes one per hand interval.
    let moves = 30;
    let began = std::time::Instant::now();
    for k in 1..=moves {
        let y = start.1 - lift / 2.0 - lift / 2.0 * k as f64 / moves as f64;
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(start.0 as f32, y as f32) });
        rig.pump();
    }
    let elapsed = began.elapsed();
    release(&rig, (start.0, start.1 - lift));
    rig.settle();
    let p = curves().points[1];
    assert!((p.x - 0.5).abs() < 1e-3 && (p.y - 0.7).abs() < 1e-3, "the point lands under the pointer: {p:?}");
    let written = editors::writes(&ctl) - writes;
    let paced = (elapsed.as_secs_f64() / oni_app::track::HAND_INTERVAL.as_secs_f64()).ceil() as usize + 3;
    assert!(written >= 2 && written <= paced, "{written} writes for {moves} moves in {elapsed:?}");
    // Twin: a write per pointer event would have reached the store once per move.
    assert!(written < moves, "{written} writes for {moves} moves in {elapsed:?}");
    assert!(g.get_curves_modified() && g.get_curve_marked().row_data(0) == Some(true), "the block shows its reset");

    // One undo takes the press and its drag back; a second finds nothing of theirs.
    w.global::<Bar>().invoke_act("undo".into());
    rig.settle();
    assert!(curves().is_neutral(), "one step took the whole drag back");
    assert!(!ctl.state.borrow().store().can_undo(), "the drag was a single step");

    // Twin: a press on the folded block's place reaches no editor.
    g.set_curves_folded(true);
    rig.settle();
    press(&rig, start);
    release(&rig, start);
    rig.settle();
    assert!(curves().is_neutral(), "a folded plot took a press");

    // The levels track on the Roll tab: the white handle dragged left by forty points.
    w.global::<PanelState>().set_tab(0);
    g.set_levels_folded(false);
    rig.settle();
    let origin = editors::origin(&ctl, Plot::Levels);
    let size = editors::size(&ctl, Plot::Levels);
    assert!(size.w > 200.0, "the levels track is laid out: {size:?}");
    println!("shot {}", rig.shot("editors-levels").display());
    let knob = g.get_levels_knobs().row_data(4).unwrap();
    assert_eq!(knob.label.as_str(), "White");
    let red = || ctl.state.borrow().store().settings().levels.red;
    let before = red();
    let grab = (origin.0 + f64::from(knob.x) - 4.0, origin.1 + size.h / 2.0);
    press(&rig, grab);
    drag(&rig, grab, (-40.0, 0.0), 20);
    release(&rig, (grab.0 - 40.0, grab.1));
    rig.settle();
    let after = red();
    let position = |v: f32| oni_settings::levels::LevelsChannel::Red.position_of_value(v, oni_settings::ConversionMode::Negative);
    let travel = (position(before.white) - position(after.white)) * size.w as f32;
    assert!((travel - 40.0).abs() < 0.5, "the white moved by the pointer's travel: {travel}");
    assert_eq!((after.black, after.mid), (before.black, before.mid), "the other handles stay");
    assert!(g.get_levels_modified());
    w.global::<Bar>().invoke_act("undo".into());
    rig.settle();
    assert_eq!(red(), before, "one undo takes the handle back");

    // The wheel: unfocused, an arrow is the window's; a press selects its point and takes the arrows.
    w.global::<PanelState>().set_tab(2);
    g.set_spectrogram_folded(false);
    rig.settle();
    let origin = editors::origin(&ctl, Plot::Wheel);
    let size = editors::size(&ctl, Plot::Wheel);
    assert!(size.w > 200.0 && (size.w - size.h).abs() < 0.5, "the wheel is a square: {size:?}");
    assert!(origin.1 + size.h < 2200.0, "the wheel is on screen: {origin:?}");
    let red = || ctl.state.borrow().store().settings().spectrogram.band(Band::Red);
    let up: slint::SharedString = slint::platform::Key::UpArrow.into();
    rig.key(&up);
    assert_eq!(red(), (0.0, 0.0, 0.0), "an unfocused wheel left the arrow alone");
    let wheel = editors::spectrogram::Wheel { side: size.w };
    let tip = wheel.position(editors::spectrogram::Grip::default(), &Spectrogram::default());
    let at = (origin.0 + tip.0 + 2.0, origin.1 + tip.1);
    press(&rig, at);
    release(&rig, at);
    rig.key(&up);
    let (_, saturation, _) = red();
    assert!((saturation - 0.04).abs() < 1e-6, "the arrow stepped the selected point: {saturation}");
    println!("shot {}", rig.shot("editors-wheel").display());

    // A monochrome photo has no grade blocks: the wheel's plot is never laid out.
    let mut mono = EditorStore::new(Archive::new(vec![frame.clone()], |_: &Path| true));
    mono.open(&frame).unwrap();
    mono.apply(oni_core::Command::Mode(oni_settings::ConversionMode::Monochrome));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support: dir.join("support"), ..Options::default() };
    let second = Shell::new(Box::new(mono), engine, options).unwrap();
    second.window.show().unwrap();
    second.window.global::<App>().set_gallery(false);
    let g2 = second.window.global::<Editors>();
    g2.set_spectrogram_folded(false);
    rig.shot("editors-monochrome");
    rig.settle();
    assert!(g2.get_monochrome());
    assert_eq!(editors::size(&second.ctl, Plot::Wheel).w, 0.0, "a monochrome panel laid out the wheel");
    let _ = std::fs::remove_dir_all(&dir);
}
