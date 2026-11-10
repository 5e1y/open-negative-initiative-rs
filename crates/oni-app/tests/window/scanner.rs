//! The Scanner face in the window, headless: the face tab, the keypad's keys as the phone's own
//! commands, the Lab screen reading them back, the step keycaps, the group resets, Esc, the walk
//! keys and the face remembered across launches.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::app::{Options, Shell};
use oni_app::{App, Bar, Lab, Library, MainWindow, PanelState};
use oni_core::{Archive, EditorStore};
use oni_link::scanner::Letter;
use oni_settings::scanner::ScannerClicks;
use oni_settings::{Face, ScannerMachine};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, Model};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

fn cells(w: &MainWindow) -> Vec<String> {
    let model = w.global::<Lab>().get_cells();
    (0..model.row_count()).map(|i| model.row_data(i).unwrap().value.to_string()).collect()
}

fn modified(w: &MainWindow) -> Vec<bool> {
    let model = w.global::<Lab>().get_modified();
    (0..model.row_count()).map(|i| model.row_data(i).unwrap()).collect()
}

/// A folder opens on the gallery, as Swift's does; the first tile opened brings the editor.
fn open_first(rig: &common::Rig, shell: &Shell) {
    rig.until("the library read", Duration::from_secs(30), || shell.ctl.state.borrow().read_done());
    shell.window.global::<Library>().invoke_tile_opened(0);
    rig.pump();
    assert!(!shell.window.global::<App>().get_gallery(), "the first tile opens in the editor");
}

/// A click on `key` as winit hands it over, ⇧ held around it when `shift`.
fn press(rig: &common::Rig, w: &MainWindow, key: common::Placed, shift: bool) {
    let held: slint::SharedString = slint::platform::Key::Shift.into();
    if shift {
        rig.window.dispatch_event(WindowEvent::KeyPressed { text: held.clone() });
    }
    common::click(rig, w, key.middle());
    if shift {
        rig.window.dispatch_event(WindowEvent::KeyReleased { text: held });
    }
    rig.pump();
}

/// The keycaps drawn anew while `act` runs and over the 40 frames after it, 8 ms apart: a cap's sink
/// and rise are drawn frame by frame, a flat key never asks one.
fn caps_drawn(rig: &common::Rig, act: impl FnOnce()) -> u64 {
    rig.drawn();
    let before = oni_app::keycaps::stats().renders;
    act();
    for _ in 0..40 {
        rig.pump();
        rig.drawn();
        std::thread::sleep(Duration::from_millis(8));
    }
    oni_app::keycaps::stats().renders - before
}

fn shell(gpu: &oni_app::render::Device, support: &Path) -> Shell {
    let store = EditorStore::new(Archive::new(Vec::new(), |_: &Path| true));
    let options = Options { support: support.to_path_buf(), ..Options::default() };
    Shell::new(Box::new(store), oni_app::render::Engine::shared(gpu), options).unwrap()
}

pub(crate) fn the_keypad_presses_the_phone_s_commands_and_the_lab_screen_reads_them_back() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the window is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-scanner-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support) = (root.join("Roll"), root.join("Support"));
    std::fs::create_dir_all(&folder).unwrap();
    // Two frames LibRaw refuses: the store opens them all the same, and the walk has a neighbour.
    for name in ["A.NEF", "B.NEF"] {
        std::fs::write(folder.join(name), b"not a raw").unwrap();
    }
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 2.0 });
    // Tall enough for every block of the keypad, Density's first.
    rig.window.set_size(slint::PhysicalSize::new(2 * 1280, 2 * 1000));

    let first = shell(&gpu, &support);
    first.window.show().unwrap();
    let (ctl, w) = (first.ctl.clone(), &first.window);
    first.open_folder(&folder);
    open_first(&rig, &first);
    let (lab, bar, panel) = (w.global::<Lab>(), w.global::<Bar>(), w.global::<PanelState>());
    let settings = || ctl.state.borrow().store().settings().clone();
    assert!(!lab.get_shown() && bar.get_face() == 0, "the editor opens on Pro, never asked otherwise");
    assert!(panel.get_separation_shown(), "Pro's Roll tab holds Dye Separation");

    rig.settle();
    println!("shot {}", rig.shot("pro-face").display());
    // Each face says what it holds at the Colour tab's head, in the same place on both.
    let (pro_caption, scanner_caption) = ("Pro: every setting by hand, the place to start", "Scanner: a minilab's density and colour, in steps");
    let caption = common::the(w, i_slint_backend_testing::AccessibleRole::Text, pro_caption);
    // The bar's Scanner tab: the face, a view and no step, remembered.
    bar.invoke_act(oni_app::Act::FaceScanner);
    rig.pump();
    assert!(lab.get_shown() && bar.get_face() == 1);
    assert_eq!(common::the(w, i_slint_backend_testing::AccessibleRole::Text, scanner_caption).y, caption.y);
    // Twin: Pro's words leave with its face.
    assert!(common::placed(w, i_slint_backend_testing::AccessibleRole::Text, pro_caption).is_empty());
    assert_eq!(settings().face, Face::Scanner);
    assert_eq!(std::fs::read_to_string(support.join("lastFaceScanner")).unwrap(), "1");
    // Dye Separation acts on this face's render too: its Roll tab holds it.
    assert!(panel.get_separation_shown(), "the Scanner face's Roll tab hides Dye Separation");
    assert_eq!(lab.get_frame(), "FRAME 01/02");
    assert_eq!(lab.get_machine_line(), "ONI · NEGATIVE");

    // The keypad's keys, the same commands the phone sends: replayed on a second store, they land alike.
    let pro_contrast = settings().print.contrast;
    let presses = [(0, 3), (1, 1), (1, 1), (1, 1), (2, -1), (2, -1), (4, 3), (4, 1)];
    for (letter, by) in presses {
        lab.invoke_press(letter, by);
    }
    rig.pump();
    assert_eq!(settings().scanner, ScannerClicks::new(3, 3, -2, 0, 4));
    let mut phone = EditorStore::new(Archive::new(Vec::new(), |_: &Path| true));
    let twin_dir = root.join("Phone");
    std::fs::create_dir_all(&twin_dir).unwrap();
    std::fs::write(twin_dir.join("A.NEF"), b"").unwrap();
    phone.open(&twin_dir.join("A.NEF").to_string_lossy()).unwrap();
    for (letter, by) in presses {
        phone.apply(oni_app::link::command_of(oni_link::Command::Step { letter: Letter::ALL[letter as usize], by: by as i8 }).unwrap());
    }
    assert_eq!(phone.settings().scanner, settings().scanner, "the phone's presses and the desktop's keys are one command");
    assert_eq!(cells(w), ["+1.5", "+1.5", "−1", "0", "+2"], "the Lab screen reads every letter back, a step half a unit");
    let readout = |i: usize| lab.get_readouts().row_data(i).unwrap().to_string();
    // Each letter's slider reads the points its keypad and the screen count, beside them.
    assert_eq!((0..5).map(readout).collect::<Vec<_>>(), cells(w));
    assert_eq!((lab.get_lows().row_data(0), lab.get_highs().row_data(4)), (Some(-48), Some(20)));
    // Twin: D read in Pro's stops, the light its key moves, says another number than the screen.
    assert_ne!(readout(0), "+0.17 EV");
    // Pro's Temperature keeps its stops.
    assert!(w.global::<oni_app::Store>().get_temperature_readout().ends_with(" EV"));
    // A key moves half a unit, ⇧ one and a half; twin: whole steps would read +1 and +3.
    lab.invoke_press(3, 1);
    rig.pump();
    assert_eq!((cells(w)[3].as_str(), readout(3).as_str()), ("+0.5", "+0.5"));
    lab.invoke_press(3, 3);
    rig.pump();
    assert_eq!(cells(w)[3], "+2");
    assert_ne!(cells(w)[3], "+4");
    lab.invoke_clear(3);
    rig.pump();
    assert_eq!(modified(w), [true, true, true, false]);

    // A letter's own key clears it alone; the Color block's reset clears its three as one step.
    lab.invoke_clear(0);
    rig.pump();
    assert_eq!(settings().scanner, ScannerClicks::new(0, 3, -2, 0, 4));
    lab.invoke_reset_group(1);
    rig.pump();
    assert_eq!(settings().scanner, ScannerClicks::new(0, 0, 0, 0, 4));
    w.global::<App>().invoke_menu(oni_app::Act::Undo);
    rig.pump();
    assert_eq!(settings().scanner, ScannerClicks::new(0, 3, -2, 0, 4), "the block's reset is one undo step");

    // The Effects keys write the face's own finishing; Pro's Contrast stays where it was.
    lab.invoke_step_effect(0, 1);
    lab.invoke_step_effect(1, -3);
    rig.pump();
    let s = settings();
    assert!((s.scanner_finish.contrast - 0.05).abs() < 1e-6 && (s.scanner_finish.vignette.stops + 0.3).abs() < 1e-6);
    assert_eq!(s.print.contrast, pro_contrast, "a Scanner key never moves Pro's Contrast");
    assert_eq!((readout(5), readout(6)), ("+5%".to_string(), "−0.30 EV".to_string()));
    assert!(modified(w)[3]);
    lab.invoke_clear_effect(1);
    lab.invoke_reset_group(3);
    rig.pump();
    assert_eq!((settings().scanner_finish.contrast, settings().scanner_finish.vignette.stops), (0.0, 0.0));
    assert!(!modified(w)[3]);

    // The machine picker: the Frontier, on the screen.
    lab.invoke_machine_picked(1);
    rig.pump();
    assert_eq!(settings().scanner_machine, ScannerMachine::Frontier);
    assert_eq!((lab.get_machine(), lab.get_machine_line().to_string()), (1, "FRONTIER · NEGATIVE".to_string()));
    // The screen sizes its counts for the widest a letter reaches, which the app gives it.
    assert!(["+23.5", "−23.5"].contains(&lab.get_widest().as_str()), "{}", lab.get_widest());
    rig.settle();
    println!("shot {}", rig.shot("scanner-face").display());

    // The RAW notice is modal: closed first, so the keys reach the Lab.
    w.global::<oni_app::Notice>().invoke_close();
    rig.pump();
    // The hand on C's rail shows its red and cyan whole; the other rails rest faint.
    let c = common::the(w, i_slint_backend_testing::AccessibleRole::Slider, "C");
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: slint::LogicalPosition::new(c.x + c.width / 3.0, c.y + c.height - 13.0) });
    rig.settle();
    println!("shot {}", rig.shot("scanner-face-hand").display());
    // D's rail slid by the hand, a pixel a move: it lands on every step, half a unit apart, and never
    // jumps a whole unit; the readout follows each step in the screen's points.
    // The rail is the slider's lowest row, its knob's height; the hand runs past both its ends.
    let d = common::the(w, i_slint_backend_testing::AccessibleRole::Slider, "D");
    let y = d.y + d.height - 13.0;
    let pressed = slint::LogicalPosition::new(d.x + d.width / 2.0, y);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: pressed });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: pressed, button: PointerEventButton::Left });
    let mut landed = Vec::new();
    for x in (d.x - 20.0) as i32..=(d.x + d.width + 20.0) as i32 {
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: slint::LogicalPosition::new(x as f32, y) });
        rig.pump();
        landed.push((settings().scanner.d, readout(0)));
    }
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: pressed, button: PointerEventButton::Left });
    rig.pump();
    let steps: Vec<i64> = landed.iter().map(|l| l.0).collect();
    assert_eq!((steps[0], *steps.last().unwrap()), (-48, 48), "the rail's ends are ±24 units");
    assert!(steps.windows(2).all(|p| p[1] - p[0] == 0 || p[1] - p[0] == 1), "every move lands on the next step at most");
    assert_eq!(steps.iter().collect::<std::collections::BTreeSet<_>>().len(), 97, "97 places, half a unit apart");
    assert!(landed.iter().any(|(d, r)| *d == 1 && r == "+0.5") && landed.iter().any(|(d, r)| *d == -3 && r == "−1.5"));
    // Twin: a rail on whole units would skip the odd steps.
    assert!(steps.iter().any(|d| d % 2 != 0));
    // The rail holds at rest as Pro's do: twelve points of the hand more than a step beside it.
    let at = |v: i64| steps.iter().filter(|d| **d == v).count();
    assert!(at(0) >= at(1) + 10, "the drag holds at zero: {} moves there, {} at +1", at(0), at(1));
    // Twin: the steps beside rest take their own width alone.
    assert!(at(1) < 8 && at(-1) < 8, "{} and {}", at(-1), at(1));
    lab.invoke_clear(0);
    rig.pump();
    // The rail slid by the hand leaves the keyboard with the photos, whose arrows walk the frames.
    assert!(w.get_keys_focused(), "the photos hold the keyboard after the slide");
    // D's keys stand at its rail's ends, grey keycaps: a press steps once, ⇧ three times.
    let (minus, plus) = (common::the(w, Role::Button, "D −: toward dark"), common::the(w, Role::Button, "D +: toward light"));
    let row = |k: &common::Placed| (k.middle().1 - y).abs() < 1.0;
    assert!(row(&minus) && row(&plus) && (minus.x - d.x).abs() < 1.0 && (plus.x + plus.width - d.x - d.width).abs() < 1.0, "{minus:?} {plus:?} {d:?}");
    let sunk = caps_drawn(&rig, || press(&rig, w, minus, false));
    assert_eq!(settings().scanner.d, -1, "a press is one step");
    press(&rig, w, plus, true);
    assert_eq!(settings().scanner.d, 2, "⇧ makes a press three steps");
    // Twin: a press deaf to ⇧ would stand back at zero.
    assert_ne!(settings().scanner.d, -1 + 1);
    assert!(sunk > 2, "the key is a keycap, its cap drawn as it sinks: {sunk} renders");
    // Twin: the Density block's flat Reset, the hand resting on it first, clears D and draws no cap.
    let reset = common::placed(w, Role::Button, "Reset").into_iter().filter(|r| r.y < d.y).max_by(|a, b| a.y.total_cmp(&b.y)).expect("Density's Reset");
    caps_drawn(&rig, || rig.window.dispatch_event(WindowEvent::PointerMoved { position: slint::LogicalPosition::new(reset.middle().0, reset.middle().1) }));
    let flat = caps_drawn(&rig, || press(&rig, w, reset, false));
    assert_eq!((flat, settings().scanner.d), (0, 0), "a flat key's press draws no cap");
    // A bare arrow walks the group on the Scanner face, which the next frame opens on.
    rig.key("\u{F703}");
    assert!(ctl.state.borrow().open_path().unwrap().ends_with("B.NEF"));
    assert_eq!((settings().face, lab.get_frame().to_string()), (Face::Scanner, "FRAME 02/02".to_string()));
    rig.key("\u{F700}");
    assert!(ctl.state.borrow().open_path().unwrap().ends_with("A.NEF"));
    // Twin: with ⌘ held the arrow is not a walk.
    rig.chord(slint::platform::Key::Control, "\u{F703}");
    assert!(ctl.state.borrow().open_path().unwrap().ends_with("A.NEF"));

    // Esc leaves the Lab; a second Esc on Pro leaves the editor, the face kept.
    rig.key("\u{1b}");
    assert!(!lab.get_shown() && settings().face == Face::Pro && bar.get_face() == 0);
    assert_eq!(std::fs::read_to_string(support.join("lastFaceScanner")).unwrap(), "0");
    rig.key("\u{1b}");
    assert!(w.global::<App>().get_gallery() && settings().face == Face::Pro);
    w.global::<App>().invoke_menu(oni_app::Act::ToggleMode);
    rig.pump();
    // On Pro the same Effects gesture moves Pro's, the Scanner's finishing left where the keys put it.
    w.global::<oni_app::Store>().invoke_step(oni_app::fields::Field::Contrast.index(), 1);
    rig.pump();
    assert!(settings().print.contrast != pro_contrast && settings().scanner_finish.contrast == 0.0);

    // Space takes Pro's editor to the gallery and back; View > Scanner Emulator toggles the face.
    rig.key(" ");
    assert!(w.global::<App>().get_gallery());
    w.global::<App>().invoke_menu(oni_app::Act::ToggleMode);
    rig.pump();
    assert!(!w.global::<App>().get_gallery());
    w.global::<App>().invoke_menu(oni_app::Act::Emulator);
    rig.pump();
    assert!(lab.get_shown());
    // `columnKeys`: a bare Space leaves the Lab for the gallery; the menu item is lit beside a photo only.
    assert!(w.global::<oni_app::Menus>().get_live().emulator, "Scanner Emulator is lit in the Lab");
    rig.key(" ");
    assert!(w.global::<App>().get_gallery() && !lab.get_shown(), "Space leaves the Lab, which is not on screen over the gallery");
    assert!(!w.global::<oni_app::Menus>().get_live().emulator, "twin: the item is dark in the gallery");
    w.global::<App>().invoke_menu(oni_app::Act::Emulator);
    rig.pump();
    assert_eq!(bar.get_face(), 1, "the menu item is off in the gallery, the face kept");
    drop(first);

    // The next launch opens on the face last left.
    let second = shell(&gpu, &support);
    second.window.show().unwrap();
    second.open_folder(&folder);
    open_first(&rig, &second);
    let lab = second.window.global::<Lab>();
    assert!(lab.get_shown());
    assert_eq!(second.ctl.state.borrow().store().settings().face, Face::Scanner);
    drop(second);
    let _ = std::fs::remove_dir_all(&root);
}

/// The Lab screen, narrow, every count at the widest a letter reaches: glass at each column's edges, each
/// count whole; sized for a one-glyph count, at the screen's full size, the same counts run over them.
pub(crate) fn the_lab_screen_holds_its_widest_count_whole() {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let probe = oni_app::ScreenProbe::new().unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 2.0 });
    rig.window.set_size(slint::PhysicalSize::new(2 * 240, 2 * 140));
    probe.show().unwrap();
    rig.settle();
    // Ink over the glass, on the screen's every row, within a point and a half either side of each column's
    // edge: a count too wide overflows the edge where the interface is read live, and is cut just short of it
    // where it is compiled; one that fits keeps over two points clear.
    let inked = || {
        let (width, px) = rig.drawn();
        let lit = |x: f32, y: f32| {
            let v = px[(y * 2.0) as usize * width + (x * 2.0) as usize].0;
            ((v >> 11) & 0x1f) * 255 / 31 + ((v >> 5) & 0x3f) * 255 / 63 + (v & 0x1f) * 255 / 31 > 150
        };
        let edges: Vec<f32> = (0..=5).map(|i| probe.get_left() + i as f32 * probe.get_column()).collect();
        let rows = (probe.get_top() * 2.0) as i32..(probe.get_bottom() * 2.0) as i32;
        let band = [-1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5];
        rows.map(|r| r as f32 / 2.0).any(|y| edges.iter().any(|x| band.iter().any(|d| lit(x + d, y))))
    };
    println!("shot {}", rig.shot("lab-screen-widest").display());
    assert!(!inked(), "every count stands whole in its column");
    // Twin: sized for a one-glyph count, counts twice as long run over their edges whatever the font.
    probe.set_widest("0".into());
    probe.set_count("23.523.5".into());
    rig.settle();
    println!("shot {}", rig.shot("lab-screen-full-size").display());
    assert!(inked(), "a count at full size would be seen cut");
}
