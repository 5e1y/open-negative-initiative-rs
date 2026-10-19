//! Sheets are modal, as Swift's: Escape closes one from inside its own field, a stray click on the
//! scrim keeps what was typed, nothing behind a sheet moves, and each sheet's keys reach its answer.

use oni_app::app::export_card;
use oni_app::app::{Options, Shell};
use oni_app::{Confirm, Exporting, Help, Library, Menus};
use oni_core::{Archive, EditorStore};
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, SharedString};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use crate::common;

fn click(rig: &common::Rig, x: f32, y: f32) {
    let position = LogicalPosition::new(x, y);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Left });
    rig.pump();
}

fn shell(rig: &common::Rig, root: &std::path::Path, gpu: &oni_app::render::Device, reporter: oni_app::app::Reporter) -> Shell {
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), reporter, ..Options::default() };
    let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    shell
}

pub(crate) fn a_sheet_is_modal_and_escape_closes_it_from_its_own_field() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the sheets are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-sheets-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let folder = root.join("Scans");
    common::plain_tiff(&folder.join("A.tif"), 96, 64, 0.5);
    common::plain_tiff(&folder.join("B.tif"), 96, 64, 0.6);
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    // The post waits while `hold` is set, so a sheet can close and open again before it answers.
    let (hold, posted) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicUsize::new(0)));
    let (h, p) = (hold.clone(), posted.clone());
    let reporter: oni_app::app::Reporter = Arc::new(move |_: &str, _: Option<String>| {
        while h.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(5));
        }
        p.fetch_add(1, Ordering::Relaxed);
        Ok(())
    });
    let shell = shell(&rig, &root, &gpu, reporter);
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, menus, help) = (w.global::<Library>(), w.global::<Menus>(), w.global::<Help>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    lib.invoke_tile_clicked(0, false, false);
    rig.settle();
    let live = menus.get_live();
    assert!(live.remove && live.medal && live.export, "twin: with no sheet, the selection's items are live");
    assert_eq!(help.get_release().as_str(), "1.2", "the menu and the welcome's heading name one release");

    // Report a Bug: its field takes the keys, the menus go dark behind it.
    ctl.act("report");
    rig.pump();
    println!("shot {}", rig.shot("report-sheet").display());
    assert!(help.get_report_shown());
    assert!(!w.get_keys_focused(), "the message field holds the keys");
    let live = menus.get_live();
    assert!(!live.remove && !live.medal && !live.export && !live.open && !live.undo, "nothing behind the sheet is live");
    rig.key("x");
    assert_eq!(help.get_report_message().as_str(), "x", "typing reaches the field");

    // A stray click on the scrim keeps the typed report; Escape from the field closes the sheet.
    click(&rig, 12.0, 400.0);
    assert!(help.get_report_shown() && help.get_report_message() == "x", "a modal sheet ignores the scrim");
    rig.key("\u{1b}");
    assert!(!help.get_report_shown(), "Escape from the field reaches the sheet");
    rig.pump();
    assert!(w.get_keys_focused() && menus.get_live().remove, "closed, the keys and the menus come back");

    // The welcome sheet loses nothing on a click beside it, so the scrim closes it.
    ctl.act("welcome");
    rig.pump();
    assert!(help.get_welcome_shown());
    click(&rig, 12.0, 400.0);
    assert!(!help.get_welcome_shown(), "twin: a sheet with nothing to lose closes on the scrim");

    // A post answered after its sheet closed and opened again does not thank the new report.
    hold.store(true, Ordering::Relaxed);
    ctl.act("report");
    rig.pump();
    help.set_report_message("The first report.".into());
    help.invoke_report_send();
    assert!(help.get_report_sending());
    help.invoke_report_closed();
    ctl.act("report");
    rig.pump();
    hold.store(false, Ordering::Relaxed);
    rig.until("the first post", Duration::from_secs(10), || posted.load(Ordering::Relaxed) == 1);
    rig.settle();
    assert!(!help.get_report_sent() && !help.get_report_sending(), "the stale answer lands nowhere");
    help.set_report_message("The second report.".into());
    help.invoke_report_send();
    rig.until("the second post", Duration::from_secs(10), || help.get_report_sent());
    assert_eq!(posted.load(Ordering::Relaxed), 2, "twin: the open sheet's own post is thanked");
    help.invoke_report_closed();
    rig.pump();

    // A Confirm over the gallery: ⌘⌫ and a medal key reach nothing behind it; Escape cancels.
    let tiles = || oni_app::app::tiles(w);
    ctl.act("reset-to-default");
    rig.until("the confirm", Duration::from_secs(10), || w.global::<Confirm>().get_shown());
    assert!(w.get_keys_focused());
    let backspace: SharedString = Key::Backspace.into();
    rig.chord(Key::Control, &backspace);
    rig.key("1");
    rig.pump();
    assert_eq!(tiles().len(), 2, "nothing went to the ONI Trash behind the sheet");
    assert!(tiles().iter().all(|t| t.medal == 0), "no medal was laid behind the sheet");
    rig.key("\u{1b}");
    assert!(!w.global::<Confirm>().get_shown());
    rig.pump();
    rig.key("1");
    assert!(tiles().iter().any(|t| t.medal == 1), "twin: with the sheet gone, the key lays the medal");
    rig.chord(Key::Control, &backspace);
    rig.until("the photo trashed", Duration::from_secs(10), || tiles().len() == 1);
    let _ = std::fs::remove_dir_all(&root);
}

/// The export's clash asks with its own sheet, whose Replace reaches `Exporting.confirmed` and
/// writes the batch, never through the generic confirm's answer.
pub(crate) fn the_exports_replace_prompt_answers_the_export() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the export's prompt is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-replace-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let folder = root.join("Scans");
    common::plain_tiff(&folder.join("A.tif"), 96, 64, 0.5);
    common::plain_tiff(&folder.join("B.tif"), 96, 64, 0.6);
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let shell = shell(&rig, &root, &gpu, Arc::new(|_: &str, _: Option<String>| Ok(())));
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, e) = (w.global::<Library>(), w.global::<Exporting>());
    let generic = Rc::new(RefCell::new(Vec::new()));
    let heard = generic.clone();
    w.global::<Confirm>().on_answered(move |go| heard.borrow_mut().push(go));
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(1, true, false);
    ctl.act("export");
    assert!(e.get_shown());
    rig.until("the batch's lines", Duration::from_secs(120), || export_card::lines(w).iter().all(|l| l.ready));
    let names: Vec<String> = export_card::lines(w).iter().map(|l| l.name.to_string()).collect();
    let batch = root.join("batch");
    std::fs::create_dir_all(&batch).unwrap();
    for extension in ["tiff", "tif", "jpg", "png"] {
        std::fs::write(batch.join(format!("{}.{extension}", names[0])), b"kept").unwrap();
    }
    let answer = batch.clone();
    export_card::answer_with(&ctl, Rc::new(move |_| vec![answer.clone()]));
    e.invoke_export();
    rig.pump();
    assert!(e.get_confirm_shown(), "the clash asks");
    rig.settle();
    println!("shot {}", rig.shot("export-replace").display());
    // The prompt's keys sit below the window's middle: the pointer sweeps them, Cancel asked again.
    let (mut cancelled, mut replaced) = (0, false);
    'sweep: for y in (400..560).step_by(4) {
        for x in (440..840).step_by(8) {
            click(&rig, x as f32, y as f32);
            if !e.get_confirm_shown() {
                if export_card::card(&ctl).exporting || !e.get_shown() {
                    replaced = true;
                    break 'sweep;
                }
                cancelled += 1;
                e.invoke_export();
                rig.pump();
            }
        }
    }
    assert!(replaced, "Replace reaches the export");
    assert!(cancelled > 0, "Cancel, beside it, closes the prompt and writes nothing");
    assert!(generic.borrow().is_empty(), "twin: the generic confirm's answer is never the export's");
    rig.until("the batch written", Duration::from_secs(120), || !export_card::card(&ctl).exporting && !e.get_shown());
    assert!(names.iter().all(|n| std::fs::read_dir(&batch).unwrap().filter_map(Result::ok).any(|f| {
        let name = f.file_name().to_string_lossy().to_string();
        name.starts_with(n.as_str()) && std::fs::read(f.path()).unwrap() != b"kept"
    })), "each photo written over the clash");
    let _ = std::fs::remove_dir_all(&root);
}
