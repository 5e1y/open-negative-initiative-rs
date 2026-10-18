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

use crate::common;

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

/// A little-endian LinearRaw TIFF of `w` by `h`, 8-bit RGB in one strip, as a scanner writes it.
fn linear_raw_8_bit(w: u32, h: u32) -> Vec<u8> {
    let data = vec![128u8; (w * h * 3) as usize];
    let entries: [(u16, u16, u32); 9] = [
        (256, 4, w),
        (257, 4, h),
        (258, 3, 8),
        (259, 3, 1),
        (262, 3, 34892),
        (273, 4, 8),
        (277, 3, 3),
        (278, 4, h),
        (279, 4, data.len() as u32),
    ];
    let mut out = b"II".to_vec();
    out.extend(42u16.to_le_bytes());
    out.extend((8 + data.len() as u32).to_le_bytes());
    out.extend(&data);
    out.extend((entries.len() as u16).to_le_bytes());
    for (tag, kind, value) in entries {
        out.extend(tag.to_le_bytes());
        out.extend(kind.to_le_bytes());
        out.extend(1u32.to_le_bytes());
        if kind == 3 {
            out.extend((value as u16).to_le_bytes());
            out.extend(0u16.to_le_bytes());
        } else {
            out.extend(value.to_le_bytes());
        }
    }
    out.extend(0u32.to_le_bytes());
    out
}

pub(crate) fn a_point_and_a_handle_dragged_in_the_window_reach_the_store_as_one_step_each() {
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

    // A double click on a point: the second press lands inside the settle delay, so the move the
    // first made and the removal are one step, and one undo puts the point back where it stood.
    let undo = || {
        w.global::<Bar>().invoke_act("undo".into());
        rig.settle();
    };
    press(&rig, start);
    release(&rig, start);
    rig.settle();
    let off = (start.0 + 3.0, start.1);
    for _ in 0..2 {
        press(&rig, off);
        release(&rig, off);
    }
    assert!(curves().is_neutral(), "the double click removed the point");
    rig.settle();
    undo();
    assert!((curves().points[1].x - 0.5).abs() < 1e-6, "the move went with the removal: {:?}", curves().points);
    undo();
    assert!(curves().is_neutral());
    // Twin: the delay running out between the two presses leaves the move a step of its own.
    press(&rig, start);
    release(&rig, start);
    rig.settle();
    press(&rig, off);
    release(&rig, off);
    let rest = std::time::Instant::now();
    rig.until("the settle delay", Duration::from_secs(1), || rest.elapsed() > Duration::from_millis(250));
    press(&rig, off);
    release(&rig, off);
    assert!(curves().is_neutral(), "still a double click at 250 ms");
    rig.settle();
    undo();
    let moved = curves().points[1].x;
    assert!((moved - (off.0 - origin.0) / size.w).abs() < 1e-3, "the undo stopped at the move: {moved}");
    undo();
    undo();
    assert!(curves().is_neutral());

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
    let white_x = || f64::from(g.get_levels_knobs().row_data(4).unwrap().x);
    let x_of = |v: f32| f64::from(position(v)) * size.w;
    assert!((white_x() - x_of(after.white)).abs() < 1e-3, "the handle let go stands where the hand left it");
    // The undo is a value from elsewhere: the handle springs back over frames instead of jumping.
    w.global::<Bar>().invoke_act("undo".into());
    rig.pump();
    let flying = white_x();
    assert!((flying - x_of(before.white)).abs() > 1.0, "the handle jumped back: {flying}");
    rig.settle();
    assert_eq!(red(), before, "one undo takes the handle back");
    assert!((white_x() - x_of(before.white)).abs() < 1e-3, "the spring came to rest on the value");

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
    // The band slider follows the wheel by its spring, a value from elsewhere.
    let knob = || g.get_band_fractions().row_data(1).unwrap();
    let range = oni_settings::spectrogram::Spectrogram::SATURATION_RANGE;
    let target = oni_app::track::position(0.04, &(range.0..=range.1), None);
    rig.settle();
    assert!((knob() - target).abs() < 1e-5, "the saturation knob rests on the value: {}", knob());

    // A press off the wheel releases it: the point unlights and the arrows go back to the window,
    // as the window's own press hook sends it before the press lands.
    let lit = || g.get_wheel_dots().iter().any(|d| d.halo > 0.0);
    assert!(lit(), "the focused wheel lights its point");
    editors::pressed_anywhere(&ctl, at);
    press(&rig, at);
    release(&rig, at);
    rig.key(&up);
    let (_, twice, _) = red();
    assert!((twice - 0.08).abs() < 1e-6, "twin: a press on the wheel keeps it: {twice}");
    let outside = (origin.0 - 4.0, origin.1 + size.h / 2.0);
    editors::pressed_anywhere(&ctl, outside);
    press(&rig, outside);
    release(&rig, outside);
    rig.settle();
    assert!(!lit(), "a press off the wheel left its point lit");
    rig.key(&up);
    assert_eq!(red().1, twice, "the arrow still reached the wheel");

    // A swatch tab names its band in the tooltip; a channel tab already shows its name.
    let tip = w.global::<oni_app::Tip>();
    let mut named = None;
    for dy in 0..80 {
        let at = LogicalPosition::new((origin.0 + 12.0) as f32, (origin.1 + size.h + dy as f64) as f32);
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: at });
        rig.pump();
        if tip.get_shown() {
            named = Some(tip.get_text().to_string());
            break;
        }
    }
    assert_eq!(named.as_deref(), Some(Band::ALL[0].label()), "the first swatch named no band");

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
    second.window.global::<PanelState>().set_tab(2);
    g2.set_spectrogram_folded(false);
    rig.shot("editors-monochrome");
    rig.settle();
    assert!(g2.get_monochrome());
    assert_eq!(editors::size(&second.ctl, Plot::Wheel).w, 0.0, "a monochrome panel laid out the wheel");
    assert!(!g.get_low_precision(), "twin: a RAW is never flagged 8-bit");

    // An 8-bit scan is flagged under Per-Channel Levels.
    let scan = dir.join("scan.tif");
    std::fs::write(&scan, linear_raw_8_bit(4, 4)).unwrap();
    let scan = scan.to_string_lossy().into_owned();
    let mut eight = EditorStore::new(Archive::new(vec![scan.clone()], |_: &Path| false));
    eight.open(&scan).unwrap();
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support: dir.join("support"), ..Options::default() };
    let third = Shell::new(Box::new(eight), engine, options).unwrap();
    third.window.show().unwrap();
    third.window.global::<App>().set_gallery(false);
    let g3 = third.window.global::<Editors>();
    third.window.global::<PanelState>().set_tab(0);
    g3.set_levels_folded(false);
    rig.shot("editors-eight-bit");
    rig.settle();
    assert!(g3.get_low_precision(), "an 8-bit scan carries no badge");
    assert!(editors::size(&third.ctl, Plot::Levels).w > 200.0, "the Pro face lays out Per-Channel Levels");

    // The Scanner face has no Per-Channel Levels: the block is never laid out.
    // The face switch takes a frame opened from the archive, as a hand opens one.
    let faces = dir.join("faces");
    std::fs::create_dir_all(&faces).unwrap();
    let raw = faces.join("frame.NEF");
    std::fs::write(&raw, b"").unwrap();
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let picker: Box<dyn Fn() -> Option<PathBuf>> = Box::new(|| None);
    let options = Options { support: dir.join("faces-support"), caches: dir.join("faces-caches"), picker, ..Options::default() };
    let fourth = Shell::new(Box::new(store), engine, options).unwrap();
    fourth.window.show().unwrap();
    fourth.add(&raw);
    rig.until("the frame in the editor", Duration::from_secs(60), || {
        !fourth.window.global::<App>().get_gallery() && fourth.ctl.state.borrow().open_path().is_some()
    });
    fourth.window.global::<Bar>().invoke_act("face-scanner".into());
    let g4 = fourth.window.global::<Editors>();
    fourth.window.global::<PanelState>().set_tab(0);
    g4.set_levels_folded(false);
    rig.shot("editors-scanner-face");
    rig.settle();
    assert!(g4.get_scanner_face());
    assert_eq!(editors::size(&fourth.ctl, Plot::Levels).w, 0.0, "the Scanner face laid out Per-Channel Levels");
    let _ = std::fs::remove_dir_all(&dir);
}
