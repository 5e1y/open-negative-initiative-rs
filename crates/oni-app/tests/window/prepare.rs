//! Prepare the Roll on the headless window, every step by the hand: left keeping nothing, resumed from a
//! quit's draft, measured as one gallery step, asked for by a bare frame opened again, from the Roll tab.

use oni_app::app::{Options, Shell};
use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::{App, Bar, Canvas, Clicks, Confirm, Library, Menus, Preparing};
use oni_core::{Archive, EditorStore};
use oni_settings::frame::RollReference;
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{ConversionMode, Id, PipelineSettings};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use crate::common;

fn loaded(path: &Path) -> sidecar::Loaded {
    sidecar::read(path, false, true).unwrap()
}

/// Whether a picture of a graded scan reads as the scan does: darker at its top left than at its
/// bottom right, as the gradient was written; an inverted picture reads the other way.
fn as_scanned(rgba: &[u8], width: usize, height: usize) -> bool {
    let luma = |fx: f32, fy: f32| {
        let at = ((fy * height as f32) as usize * width + (fx * width as f32) as usize) * 4;
        rgba[at..at + 3].iter().map(|v| f32::from(*v)).sum::<f32>()
    };
    luma(0.1, 0.1) + 6.0 < luma(0.9, 0.9)
}

fn click(rig: &common::Rig, x: f32, y: f32) {
    let at = LogicalPosition::new(x, y);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: at });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: at, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: at, button: PointerEventButton::Left });
    rig.pump();
}

pub(crate) fn a_roll_is_prepared_left_resumed_from_its_draft_and_measured_as_one_gallery_step() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: Prepare the Roll is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-prepare-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for (name, tone) in [("A.tif", 0.9), ("B.tif", 0.7), ("C.tif", 0.5), ("D.tif", 0.3)] {
        common::graded_tiff(&folder.join(name), 120, 80, tone);
    }
    let canonical = |n: &str| PathBuf::from(oni_library::paths::canonical(&folder.join(n)));
    // D stands on another roll: a frame of this folder the measure leaves out.
    let mut other = PipelineSettings::rest(ConversionMode::Negative, false);
    other.roll_reference = Some(RollReference { id: Id::new(), stops: [0.0; 3], black: [0.0; 3], white: [1.0; 3], base: None, median: None });
    sidecar::write(&other, &canonical("D.tif"), WriteOptions::default()).unwrap();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support: support.clone(), caches, picker: Box::new(|| None), require_roll: true, ..Options::default() };
    let radius = || oni_app::preferences::Preferences::load(&support).value(oni_measure::roll::BASE_RADIUS_KEY).and_then(serde_json::Value::as_f64);
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, menus, p, app) = (w.global::<Library>(), w.global::<Menus>(), w.global::<Preparing>(), w.global::<App>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.settle();
    let tile = |name: &str| {
        let mut tiles = oni_app::app::tiles(w);
        tiles.sort_by_key(|t| t.index);
        tiles.iter().find(|t| t.caption == name).unwrap_or_else(|| panic!("{name}")).index
    };
    let names = || (0..p.get_cells().row_count()).count();
    let action = || (p.get_footer_x() + p.get_footer_width() - 12.0, p.get_footer_y() + p.get_footer_height() / 2.0);

    // Nothing picked, the roll shown is the one to prepare; one frame picked prepares its own roll.
    assert!(menus.get_live().measure_roll && w.global::<Bar>().get_can_prepare());
    lib.invoke_tile_clicked(tile("A"), false, false);
    rig.pump();
    assert!(menus.get_live().measure_roll && w.global::<Bar>().get_can_prepare());
    assert!(menus.get_live().export && menus.get_live().medal, "twin: in the gallery the selection's items are live");
    w.global::<Bar>().invoke_act(oni_app::Act::PrepareRoll);
    rig.until("the roll's surface", Duration::from_secs(10), || p.get_shown() && names() == 3);
    rig.until("every frame read", Duration::from_secs(10), || p.get_capture() == 0);
    assert_eq!(p.get_left_out().as_str(), "1 frame of this folder, prepared with another roll, is left out.");
    assert_eq!(p.get_heading().as_str(), "Select the Film Type");
    assert!(!p.get_has_tutorial(), "the film step offers no demo");
    assert!(p.get_skipped().row_data(1).unwrap(), "a scanner's roll skips the flat-field");
    assert!(!menus.get_live().toggle_mode && !menus.get_live().measure_roll, "⌘G and the item itself stay dark meanwhile");
    let live = menus.get_live();
    let behind = [live.export, live.framing, live.correction, live.mirror, live.smaller, live.larger, live.remove, live.undo, live.redo, live.reset];
    let more = [live.copy_settings, live.paste_settings, live.paste_to_roll, live.medal, live.new_roll, live.save_preset, live.apply_preset];
    assert!(behind.iter().chain(more.iter()).all(|on| !on), "nothing reaches the editor the roll hides: {live:?}");
    assert!(loaded(&canonical("B.tif")).roll_draft.is_none(), "twin: the film step writes no draft");
    rig.settle();
    println!("shot {}", rig.shot("prepare-film").display());

    // The card shows the frame as its scan, as the base step will: one look across the steps.
    rig.until("the previews", Duration::from_secs(60), || ctl.state.borrow().roll_previewed() == 3);
    let card = if p.get_card_second() { p.get_card_b() } else { p.get_card_a() };
    let card = card.to_rgba8().expect("the card holds pixels");
    let (cw, ch) = (card.width() as usize, card.height() as usize);
    assert!(as_scanned(card.as_bytes(), cw, ch), "the card reads as the scan, never inverted");
    let inverted: Vec<u8> = card.as_bytes().chunks(4).flat_map(|p| [255 - p[0], 255 - p[1], 255 - p[2], p[3]]).collect();
    assert!(!as_scanned(&inverted, cw, ch), "twin: its complement reads the other way");

    // The step bar: each step's words as wide as they are, the room left shared, so the gaps between them are even.
    let steps = ["Film Type", "Flat-Field", "Base", "Flags", "Window"];
    let words: Vec<common::Placed> = steps
        .iter()
        .map(|n| {
            let found = common::placed(w, Role::Text, n).into_iter().filter(|t| t.y < w.global::<Bar>().get_height() && t.width > 0.0);
            found.min_by(|a, b| a.width.total_cmp(&b.width)).unwrap_or_else(|| panic!("the step {n:?} in the bar"))
        })
        .collect();
    let gaps: Vec<f32> = words.windows(2).map(|w| w[1].x - (w[0].x + w[0].width)).collect();
    let spread = |g: &[f32]| g.iter().copied().fold(f32::MIN, f32::max) - g.iter().copied().fold(f32::MAX, f32::min);
    assert!(spread(&gaps) < 1.0, "even gaps between the steps: {gaps:?}");
    let tabs: Vec<common::Placed> = steps.iter().map(|n| common::the(w, Role::Tab, n)).collect();
    let cell = (tabs[4].x + tabs[4].width - tabs[0].x) / 5.0;
    let centred: Vec<f32> = words.windows(2).map(|w| cell - (w[0].width + w[1].width) / 2.0).collect();
    assert!(spread(&centred) > 4.0, "twin: the words centred in equal cells leave uneven gaps: {centred:?}");

    // NEXT walks past the skipped flat-field to the base; the step reached rides in every sidecar.
    let first = (common::the(w, Role::Button, "Cancel"), p.get_footer_height());
    let (x, y) = action();
    click(&rig, x, y);
    assert_eq!(p.get_step(), 2);
    rig.until("the drafts written", Duration::from_secs(10), || ["A.tif", "B.tif", "C.tif"].iter().all(|n| loaded(&canonical(n)).roll_draft.is_some_and(|d| d.step == "base")));
    assert!(loaded(&canonical("D.tif")).roll_draft.is_none(), "the frame left out carries no draft");
    assert_eq!((p.get_base_note().as_str(), p.get_action_title().as_str(), p.get_action_enabled(), p.get_skippable()), ("No base picked yet.", "NEXT", false, true));
    let done = || (0..5).map(|i| p.get_done().row_data(i).unwrap()).collect::<Vec<bool>>();
    assert_eq!(done(), [true, false, false, false, false], "the film step behind is done; the flat-field not walked and the current base are not");
    // Cancel and Skip at the left; Back and the step's keycap a group's gap apart at the right, the
    // footer laid as on the film step, its keys on the same line.
    rig.settle();
    let key = |name: &str| common::the(w, Role::Button, name);
    let (cancel, skip, back, next) = (key("Cancel"), key("Skip"), key("Back"), key("NEXT"));
    let group = next.x - (back.x + back.width);
    assert!((group - 8.0).abs() < 1.0 && (skip.x - (cancel.x + cancel.width) - group).abs() < 1.0, "two groups: {cancel:?} {skip:?} {back:?} {next:?}");
    assert!(back.x - (skip.x + skip.width) >= 2.0 * group, "twin: Skip stands far from the keycap: {skip:?} {back:?}");
    assert_eq!((cancel.y, cancel.height, p.get_footer_height()), (first.0.y, first.0.height, first.1), "one footer on every step");
    assert!(cancel.height < p.get_footer_height(), "twin: the keycap lays the footer, taller than a flat key");
    // At the keycap's place, a click passes no empty base: Skip is a choice apart.
    let (x, y) = action();
    click(&rig, x, y);
    assert_eq!(p.get_step(), 2, "the keycap waits for the base");

    // The base's demo waits for Show How, nothing moving beside the scan unasked; pressed again, it goes.
    let demo = || p.get_tutorial().size().width;
    assert!(p.get_has_tutorial() && demo() == 0, "offered, never played unasked");
    p.invoke_tutorial_toggled();
    rig.pump();
    assert!(demo() > 0, "Show How plays it");
    p.invoke_tutorial_toggled();
    rig.pump();
    assert_eq!(demo(), 0, "twin: pressed again, it goes");
    p.invoke_tutorial_toggled();
    rig.pump();

    // Picker Size: held in memory while it slides, written once let go.
    let before = radius();
    assert!(!p.invoke_press(3, false));
    for t in [10.0, 20.0, 30.0] {
        p.invoke_drag(3, t, 200.0, false);
    }
    rig.pump();
    assert_eq!(radius(), before, "a drag writes no preference at each notch");
    p.invoke_release(3);
    assert!(radius().is_some_and(|r| Some(r) != before), "the release writes the size: {:?}", radius());
    // The wheel along it moves it too, written once the scroll rests; a scroll across is the pane's.
    let dragged = radius();
    assert!(!p.invoke_scroll(3, 0.0, 80.0, 200.0, false), "a scroll across the track is the pane's");
    assert!(p.invoke_scroll(3, 80.0, 0.0, 200.0, false));
    rig.pump();
    assert_eq!(radius(), dragged, "a scroll writes no preference at each event");
    std::thread::sleep(Duration::from_millis(250));
    rig.pump();
    assert!(radius().is_some_and(|r| Some(r) != dragged), "the rest writes the size: {:?}", radius());

    // The base step's scan reads as the card did: the same look on every step.
    rig.until("the scan read", Duration::from_secs(60), || p.get_scan_ready());
    let scan = p.get_scan().to_rgba8().expect("the scan holds pixels");
    assert!(as_scanned(scan.as_bytes(), scan.width() as usize, scan.height() as usize), "the base's scan reads as the card");
    rig.settle();
    println!("shot {}", rig.shot("prepare-base").display());

    // A click on the scan picks the bare film there: its ring lands with a key's click, and the step
    // stays, the key risen to NEXT, so the hand sees its pick before it goes on.
    let (pressed, released) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let (pc, rc) = (pressed.clone(), released.clone());
    w.global::<Clicks>().on_press(move || pc.set(pc.get() + 1));
    w.global::<Clicks>().on_release(move || rc.set(rc.get() + 1));
    let on_scan = (p.get_base_origin_x() + p.get_scan_x() + p.get_scan_width() * 0.05, p.get_base_origin_y() + p.get_scan_y() + p.get_scan_height() * 0.1);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(on_scan.0, on_scan.1) });
    rig.pump();
    assert_eq!(pressed.get(), 0, "twin: the pointer over the scan picks nothing and clicks nothing");
    click(&rig, on_scan.0, on_scan.1);
    assert_eq!((p.get_step(), p.get_base_picks(), pressed.get()), (2, 1, 1), "a pick stays on its step, its ring landing with a click");
    assert!(p.get_ring_shown());
    assert_eq!((p.get_base_note().as_str(), p.get_action_title().as_str(), p.get_action_enabled(), p.get_skippable()), ("On A.tif", "NEXT", true, false));
    assert!(common::placed(w, Role::Button, "Skip").is_empty(), "a base picked: nothing left to skip");
    // The pick shows the colour it sampled: within the scan's own colours in its disc, never the far corner's.
    let (sx, sy, r) = (scan.width() as usize, scan.height() as usize, oni_measure::pipette::radius(radius().unwrap_or(oni_measure::roll::BASE_RADIUS)));
    let span = |fx: f32, fy: f32| {
        let (cx, cy) = (f64::from(fx) * sx as f64, f64::from(fy) * sy as f64);
        let inside = (0..sx * sy).filter(|i| ((i % sx) as f64 + 0.5 - cx).powi(2) + ((i / sx) as f64 + 0.5 - cy).powi(2) <= r * r);
        inside.fold([(255u8, 0u8); 3], |mut b, i| {
            for (c, v) in b.iter_mut().zip(&scan.as_bytes()[i * 4..i * 4 + 3]) {
                *c = (c.0.min(*v), c.1.max(*v));
            }
            b
        })
    };
    let within = |c: [u8; 3], b: [(u8, u8); 3]| c.iter().zip(b).all(|(v, (lo, hi))| (lo..=hi).contains(v));
    let ring = p.get_ring_colour();
    let (pick, corner) = (span(0.05, 0.1), span(0.95, 0.9));
    println!("the pick's colour {ring:?}, its disc {pick:?}, the far corner's {corner:?}");
    assert!(within([ring.red(), ring.green(), ring.blue()], pick), "the pick's colour is the scan's in its disc");
    assert!(!within([ring.red(), ring.green(), ring.blue()], corner), "twin: it is not the far corner's");
    println!("shot {}", rig.shot("prepare-base-landing").display());
    // The hand gone, the loupe with it: the pick's mark alone shows where and what it sampled.
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(p.get_footer_x(), p.get_footer_y() - 40.0) });
    rig.settle();
    println!("shot {}", rig.shot("prepare-base-picked").display());
    let (x, y) = action();
    click(&rig, x, y);
    assert_eq!(p.get_step(), 3, "NEXT goes on to the flags");
    assert!(demo() == 0 && p.get_has_tutorial(), "the base's demo goes with its step; the flags' waits to be asked");
    assert_eq!(done(), [true, false, true, false, false], "the base behind is done once it holds its pick");

    // A click on the sheet flags C; Clear Flags takes every flag off; a second click flags it again.
    rig.settle();
    let cell = |i: usize| {
        let (pad, gap) = (24.0, 16.0);
        let width = (p.get_sheet_width() - 2.0 * pad - 5.0 * gap) / 6.0;
        (p.get_sheet_origin_x() + pad + i as f32 * (width + gap) + width / 2.0, p.get_sheet_origin_y() + pad + width / 3.0)
    };
    let (x, y) = cell(2);
    click(&rig, x, y);
    assert_eq!(p.get_flags_note().as_str(), "2 measured, 1 flagged");
    p.invoke_clear_flags();
    rig.pump();
    assert_eq!(p.get_flags_note().as_str(), "3 measured, 0 flagged", "twin: the flag is the click's, not the sheet's");
    click(&rig, x, y);
    assert!(p.get_cells().row_data(2).unwrap().flagged);
    // The flag rides in the draft only at a step change, so closing the window writes it.
    rig.until("the flags step's drafts", Duration::from_secs(10), || loaded(&canonical("C.tif")).roll_draft.is_some_and(|d| d.step == "flags"));
    assert!(!loaded(&canonical("C.tif")).roll_draft.unwrap().flagged, "twin: the click alone writes nothing");
    rig.window.dispatch_event(WindowEvent::CloseRequested);
    rig.pump();
    assert!(loaded(&canonical("C.tif")).roll_draft.unwrap().flagged, "closing wrote the step as it stands");
    assert!(p.get_shown());
    rig.settle();
    println!("shot {}", rig.shot("prepare-flags").display());

    // The window step: two to measure, the arrows walk them, the body drags the window.
    p.invoke_step_picked(4);
    rig.pump();
    let walked = || p.get_readouts().row_data(4).unwrap_or_default();
    assert_eq!((p.get_action_title().as_str(), p.get_skippable(), walked().as_str()), ("PREPARE ROLL", false, "1 / 2"), "the bar's own key ends the walk over two frames");
    assert_eq!(p.get_frame_name().as_str(), "A.tif");
    rig.key("\u{F703}");
    assert_eq!(p.get_frame_name().as_str(), "B.tif");
    rig.key("\u{F703}");
    assert_eq!(p.get_frame_name().as_str(), "B.tif", "twin: the last frame holds");
    rig.settle();
    let (wx, wy) = (p.get_window_x(), p.get_window_y());
    let (cx, cy) = (p.get_card_origin_x() + wx + p.get_window_width() / 2.0, p.get_card_origin_y() + wy + p.get_window_height() / 2.0);
    let clicks = (pressed.get(), released.get());
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(cx, cy) });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: LogicalPosition::new(cx, cy), button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(cx - 10.0, cy - 6.0) });
    rig.pump();
    assert_eq!((pressed.get(), released.get()), (clicks.0 + 1, clicks.1), "the window taken up clicks as a key goes down");
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: LogicalPosition::new(cx - 10.0, cy - 6.0), button: PointerEventButton::Left });
    rig.pump();
    assert_eq!((pressed.get(), released.get()), (clicks.0 + 1, clicks.1 + 1), "set down, it clicks as a key comes up");
    assert!((p.get_window_x() - (wx - 10.0)).abs() < 1.0 && (p.get_window_y() - (wy - 6.0)).abs() < 1.0, "the body moves the window");
    println!("shot {}", rig.shot("prepare-window").display());

    // Each step reached rides in every frame's sidecar, a quit's to resume from.
    let roll = ["A.tif", "B.tif", "C.tif"];
    rig.until("the drafts written", Duration::from_secs(10), || roll.iter().all(|n| loaded(&canonical(n)).roll_draft.is_some_and(|d| d.step == "window")));
    let drafts: Vec<_> = roll.iter().map(|n| loaded(&canonical(n)).roll_draft.unwrap()).collect();
    assert!(drafts[2].flagged && drafts[2].ticked && drafts[2].base.as_ref().is_some_and(|b| b.path.ends_with("A.tif")));

    // Escape asks before it drops a roll holding a pick, a flag and a drawn window; the sheet's Cancel keeps it.
    let confirm = w.global::<Confirm>();
    rig.key("\u{1b}");
    assert!(confirm.get_shown() && p.get_shown(), "a roll holding choices asks before it is left");
    assert_eq!((confirm.get_title().as_str(), confirm.get_key().as_str()), ("Discard This Roll's Preparation?", "Discard"));
    rig.key("\u{1b}");
    assert!(!confirm.get_shown() && p.get_shown(), "twin: answered Cancel, the roll stays with its steps");
    // The footer's Cancel asks the same; Discard leaves for the gallery and keeps nothing: every frame's draft dropped.
    rig.settle();
    common::click(&rig, w, common::the(w, Role::Button, "Cancel").middle());
    assert!(confirm.get_shown() && p.get_shown());
    confirm.invoke_answered(true);
    rig.pump();
    assert!(!p.get_shown() && app.get_gallery());
    rig.until("the drafts dropped", Duration::from_secs(10), || roll.iter().all(|n| loaded(&canonical(n)).roll_draft.is_none()));
    // The roll as a quit leaves it, its drafts on disk.
    for (n, d) in roll.iter().zip(drafts) {
        sidecar::set_roll_draft(Some(d), &canonical(n), false).unwrap();
    }

    // B picked brings the same roll back where it stood.
    lib.invoke_tile_clicked(tile("B"), false, false);
    rig.pump();
    ctl.act(oni_app::Act::PrepareRoll);
    rig.until("the roll resumed", Duration::from_secs(10), || p.get_shown() && p.get_step() == 4);
    rig.until("every frame placed", Duration::from_secs(10), || walked().ends_with(" / 2"));
    assert!(p.get_cells().row_data(2).unwrap().flagged);

    // MEASURE: off this thread, then the editor on the whole roll together; one gallery step lands it.
    let (x, y) = action();
    click(&rig, x, y);
    let advice = w.global::<oni_app::Advice>();
    rig.until("the measure landed", Duration::from_secs(120), || !p.get_shown() || advice.get_shown());
    assert!(!advice.get_shown(), "{}: {}", advice.get_title(), advice.get_message());
    rig.pump();
    assert!(!app.get_gallery(), "the editor opens once measured");
    assert!(ctl.state.borrow().open_path().is_some_and(|o| o.ends_with("A.tif")));
    let roll: std::collections::HashSet<String> = ["A.tif", "B.tif", "C.tif"].iter().map(|n| canonical(n).to_string_lossy().into_owned()).collect();
    assert_eq!(*ctl.state.borrow().picked(), roll, "the whole roll picked, D left out");
    rig.until("the roll together", Duration::from_secs(30), || w.global::<Canvas>().get_status().as_str() == "3 frames together");
    let ids: Vec<Option<Id>> = ["B.tif", "C.tif"].iter().map(|n| loaded(&canonical(n)).settings.roll_reference.map(|r| r.id)).collect();
    assert!(ids[0].is_some() && ids[0] == ids[1], "B and the flagged C stand on one roll: {ids:?}");
    assert!(["B.tif", "C.tif"].iter().all(|n| loaded(&canonical(n)).roll_draft.is_none()), "a measure clears the draft");
    assert_eq!(ctl.state.borrow().store().settings().roll_reference.as_ref().map(|r| r.id), ids[0], "A, open, landed in memory");
    assert!(loaded(&canonical("D.tif")).settings.roll_reference.map(|r| r.id) != ids[0], "twin: D keeps its own roll");
    ctl.act(oni_app::Act::ToggleMode);
    rig.pump();
    assert_eq!(menus.get_titles().undo.as_str(), "Undo Prepare the roll of 3 photos");
    ctl.act(oni_app::Act::Undo);
    rig.pump();
    assert!(loaded(&canonical("B.tif")).settings.roll_reference.is_none(), "one undo takes the whole measure back");
    rig.until("A back at rest", Duration::from_secs(5), || loaded(&canonical("A.tif")).settings.roll_reference.is_none());

    // A frame never measured, opened in the editor, goes to Prepare the Roll; Escape leaves for the gallery.
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    lib.invoke_tile_clicked(tile("C"), false, false);
    lib.invoke_tile_clicked(tile("C"), false, false);
    lib.invoke_tile_opened(tile("C"));
    rig.until("the roll asked for", Duration::from_secs(30), || p.get_shown());
    assert_eq!(names(), 3);
    rig.key("\u{1b}");
    assert!(app.get_gallery(), "Cancel leaves for the gallery, not the editor");
    // C, still the photo open, opened again goes to Prepare the Roll again, on its first step.
    lib.invoke_tile_opened(tile("C"));
    rig.until("the roll asked for again", Duration::from_secs(30), || p.get_shown());
    assert_eq!(p.get_step(), 0, "the roll left keeps no step");
    rig.key("\u{1b}");
    assert!(app.get_gallery());
    // Twin: a frame on a roll opens in the editor and stays there.
    lib.invoke_tile_opened(tile("D"));
    rig.until("D open", Duration::from_secs(30), || ctl.state.borrow().open_path().is_some_and(|o| o.ends_with("D.tif")));
    rig.settle();
    assert!(!p.get_shown() && !app.get_gallery());

    // The Roll tab's key prepares from the editor, D with the frames never measured; Escape goes back to it.
    let panel = w.global::<oni_app::PanelState>();
    assert!(panel.get_can_prepare());
    panel.invoke_prepare_roll();
    rig.until("the roll from the editor", Duration::from_secs(10), || p.get_shown() && names() == 4);
    // The measure lays no contact sheet, so nothing of the folder stands on another roll.
    assert_eq!(p.get_left_out().as_str(), "");
    rig.key("\u{1b}");
    assert!(!p.get_shown() && !app.get_gallery(), "Cancel goes back to the editor it came from");
    let _ = std::fs::remove_dir_all(&root);
}
