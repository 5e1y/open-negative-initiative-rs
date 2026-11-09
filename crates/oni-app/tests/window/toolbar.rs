//! The top bar as the window widens: its keys keep their widths and only the room between them grows;
//! one layout in every mode, whatever the selection, the picture's right click opening its menu; its
//! groups, each group's keys touching, the mode key naming its action, and the gallery's EXPORT at the
//! bar's end opening the sheet on what is picked.

use oni_app::app::{Options, Shell, export_card};
use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::{Act, App, Bar, Library, Preparing};
use oni_core::{Archive, EditorStore};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

/// The bar's keys left to right in the window `w` points wide, each as the span of x it takes: read off
/// the keys the window shows, never off their faces, which the bar's flat keys show under the hand alone.
fn spans(rig: &common::Rig, window: &oni_app::MainWindow, w: u32) -> Vec<(f32, f32)> {
    rig.window.set_size(slint::PhysicalSize::new(w, 800));
    rig.settle();
    let mut keys: Vec<_> = common::placed_all(window, Role::Button).into_iter().filter(|k| k.y < window.global::<Bar>().get_height()).map(|k| (k.x, k.x + k.width)).collect();
    // A key is found once for each component it is built of.
    keys.sort_by(|a, b| a.0.total_cmp(&b.0));
    keys.dedup();
    keys
}

pub(crate) fn the_bars_keys_keep_their_widths_as_the_window_widens() {
    let root = std::env::temp_dir().join(format!("oni-app-toolbar-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (support, caches) = (root.join("Support"), root.join("Caches"));
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the bar is not exercised"),
    };
    let engine = oni_app::render::Engine::shared(&gpu);
    let bar = |phone: bool| {
        let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
        let options = Options { support: support.clone(), caches: caches.clone(), picker: Box::new(|| None), phone, ..Options::default() };
        let shell = Shell::new(Box::new(store), engine.clone(), options).unwrap();
        rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
        shell.window.show().unwrap();
        rig.until("the bar laid", Duration::from_secs(5), || true);
        shell
    };
    let shell = bar(true);
    // From the window's right edge leftwards: EXPORT, Settings, Phone and Feedback, each as its distance
    // from that edge and its width, then the room before them, past Trash.
    let keys = |w: u32| {
        let keys = spans(&rig, &shell.window, w);
        let at = |label: &str| bar_key(&shell.window, label);
        let mut out: Vec<(f32, f32)> = ["EXPORT", "Settings", "Phone", "Feedback"].iter().map(|l| at(l)).map(|k| (w as f32 - k.x, k.width)).collect();
        let trash = at("Trash");
        out.push((at("Feedback").x - (trash.x + trash.width), 0.0));
        assert!(keys.last().is_some_and(|k| (k.1 - (w as f32 - 8.0)).abs() < 0.5), "EXPORT ends the bar: {keys:?}");
        out
    };
    // Both bars hold every word of the centre, so only the room between the keys can change.
    let (narrow, wide) = (keys(1728), keys(1990));
    println!("shot {}", rig.shot("toolbar-1990").display());
    assert_eq!(narrow[..4], wide[..4], "the keys keep their widths and their places from the edge: {narrow:?} {wide:?}");
    // Each worded, so as wide as its word: Settings' longer than Phone's; the app's keys one group, touching.
    assert!(narrow[1].1 > narrow[2].1, "Settings and Phone sized by their words: {narrow:?}");
    let touch = |a: (f32, f32), b: (f32, f32)| (a.0 + b.1 - b.0).abs() < 0.5;
    assert!(touch(narrow[1], narrow[2]) && touch(narrow[2], narrow[3]), "Feedback, Phone and Settings touch: {narrow:?}");
    // Twin: the room before them does grow with the window.
    assert!(wide[4].0 > narrow[4].0 + 100.0, "the room before the keys grows: {narrow:?} {wide:?}");
    drop(shell);

    // The phone switch off: Settings meets the feedback key, with no phone key between them.
    let shell = bar(false);
    let keys = spans(&rig, &shell.window, 1728);
    assert!(common::placed(&shell.window, Role::Button, "Phone").is_empty(), "no phone key: {keys:?}");
    let (settings, feedback) = (bar_key(&shell.window, "Settings"), bar_key(&shell.window, "Feedback"));
    assert!(1728.0 - settings.x == narrow[1].0 && (feedback.x + feedback.width - settings.x).abs() < 0.5, "Settings in its place, Feedback against it: {settings:?} {feedback:?}");
    drop(shell);
}

pub(crate) fn every_mode_lays_the_bar_alike_and_the_picture_opens_its_menu() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the bar's modes are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-toolbar-modes-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for (name, tone) in [("A.tif", 0.8), ("B.tif", 0.5), ("C.tif", 0.3)] {
        common::graded_tiff(&folder.join(name), 120, 80, tone);
    }
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1100, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, bar, prep) = (w.global::<Library>(), w.global::<Bar>(), w.global::<Preparing>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.settle();
    let tile = |name: &str| oni_app::app::tiles(w).into_iter().find(|t| t.caption == name).unwrap_or_else(|| panic!("{name}")).index;
    // The window at its narrowest, where the words give way to glyphs.
    let keys = || spans(&rig, w, 1100);
    lib.invoke_tile_clicked(tile("A"), false, false);
    rig.pump();
    let gallery = keys();
    bar.invoke_act(Act::SelectAll);
    rig.pump();
    assert_eq!(bar.get_targets(), 3, "the whole roll picked");
    assert_eq!(keys(), gallery, "picking the whole roll adds no key and moves none");
    lib.invoke_tile_clicked(tile("A"), false, false);
    bar.invoke_act(Act::PrepareRoll);
    rig.until("the roll's surface", Duration::from_secs(10), || prep.get_shown());
    let preparing = keys();
    println!("shot {}", rig.shot("toolbar-prepare").display());
    ctl.act(Act::LeaveRoll);
    rig.until("the roll left", Duration::from_secs(10), || !prep.get_shown());
    lib.invoke_tile_opened(tile("A"));
    rig.until("A in the editor", Duration::from_secs(60), || !w.global::<App>().get_gallery() && ctl.state.borrow().renders() >= 1);
    let editor = keys();
    println!("shot {}", rig.shot("toolbar-editor").display());
    // The mode key and the history at the left, feedback, settings and the commit at the right.
    assert_eq!((&gallery[..3], &preparing[..3]), (&editor[..3], &editor[..3]), "{gallery:?} {preparing:?} {editor:?}");
    let ends = |s: &[(f32, f32)], slot: usize| s[s.len() - slot - 2..s.len() - slot].to_vec();
    assert_eq!((ends(&gallery, 1), ends(&preparing, 0)), (ends(&editor, 1), ends(&editor, 1)), "{gallery:?} {preparing:?} {editor:?}");
    assert_eq!(gallery.last(), editor.last(), "EXPORT stands in one place in both screens");
    assert!(editor.last().unwrap().1 <= 1100.0 - 8.0, "EXPORT ends inside the window: {editor:?}");
    // Twin: the keys between them are the mode's own, which a bar laid once would not show.
    assert_ne!(gallery[3..gallery.len() - 3], editor[3..editor.len() - 3]);

    // A right click on the canvas lays the photo's own menu, as its tile's does; one on the bar lays none.
    // Above the picture: the measuring window, armed on an unmeasured frame, takes the press on it.
    let press = |x: f32, y: f32| {
        let position = LogicalPosition::new(x, y);
        rig.window.dispatch_event(WindowEvent::PointerMoved { position });
        rig.window.dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Right });
        rig.window.dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Right });
        rig.pump();
    };
    let lines = || lib.get_menu_lines().iter().map(|l| l.text.to_string()).collect::<Vec<_>>();
    press(1090.0, 70.0);
    assert!(lines().is_empty(), "twin: a right click on the bar lays no menu: {:?}", lines());
    press(510.0, 200.0);
    assert!(lines().iter().any(|l| l == "Export…") && lines().iter().any(|l| l == "Move to ONI Trash"), "{:?}", lines());
    let _ = std::fs::remove_dir_all(&root);
}

/// Three times Swift's `DS.Space.sm` between the bar's groups, whose keys touch.
const BAR_GAP: f32 = 24.0;
/// The rule before Reset: one flat line a point thick, a bar's gap either side.
const RULED: f32 = 2.0 * BAR_GAP + 1.0;

/// The one key of the bar so named, where the layout put it.
fn bar_key(w: &oni_app::MainWindow, label: &str) -> common::Placed {
    let keys: Vec<_> = common::placed(w, Role::Button, label).into_iter().filter(|k| k.y < w.global::<Bar>().get_height()).collect();
    assert_eq!(keys.len(), 1, "one {label:?} on the bar: {keys:?}");
    keys[0]
}

/// Whether the bar draws anything at `x` on the middle row of `key`, the window as it stands.
fn inked(rig: &common::Rig, key: common::Placed, x: f32) -> bool {
    let (w, px) = rig.drawn();
    let y = (key.y + key.height / 2.0) as usize;
    px[y * w + x as usize].0 != px[y * w + 2].0
}

/// The most colour on a key under the hand: its face, its glyph and its word.
fn reached_chroma(rig: &common::Rig, key: common::Placed) -> i32 {
    let position = LogicalPosition::new(key.x + key.width / 2.0, key.y + key.height / 2.0);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position });
    rig.settle();
    let (w, drawn) = rig.drawn();
    let px = &drawn;
    let (xs, ys) = (key.x as usize..(key.x + key.width) as usize, key.y as usize..(key.y + key.height) as usize);
    ys.flat_map(|y| xs.clone().map(move |x| px[y * w + x].0))
        .map(|v| {
            let (r, g, b) = (((v >> 11) & 0x1f) as i32 * 255 / 31, ((v >> 5) & 0x3f) as i32 * 255 / 63, (v & 0x1f) as i32 * 255 / 31);
            r.max(g).max(b) - r.min(g).min(b)
        })
        .max()
        .unwrap_or(0)
}

pub(crate) fn the_bar_reads_by_groups_reset_names_its_scope_and_trash_stands_last() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the bar's groups are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-toolbar-groups-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for (name, tone) in [("A.tif", 0.8), ("B.tif", 0.5)] {
        common::graded_tiff(&folder.join(name), 120, 80, tone);
    }
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1728, 900));
    shell.window.show().unwrap();
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (lib, bar) = (w.global::<Library>(), w.global::<Bar>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    let tile = |name: &str| oni_app::app::tiles(w).into_iter().find(|t| t.caption == name).unwrap_or_else(|| panic!("{name}")).index;
    lib.invoke_tile_clicked(tile("A"), false, false);
    rig.settle();
    let right = |k: common::Placed| k.x + k.width;
    // Two gaps on the bar alone: a group's between its keys, a bar's between groups.
    let gap = |a: common::Placed, b: common::Placed| b.x - right(a);
    let near = |g: f32, want: f32| (g - want).abs() <= 1.0;

    // The gallery: EDIT, Select All, PREPARE ROLL and ADD FOLDER…, then past a rule Reset naming the face
    // it resets and Trash last, one group; the rating filter is the library's, off the bar.
    let (edit, select) = (bar_key(w, "EDIT"), bar_key(w, "Select All"));
    let (prepare, add, reset, trash) = (bar_key(w, "PREPARE ROLL"), bar_key(w, "ADD FOLDER\u{2026}"), bar_key(w, "Reset Pro"), bar_key(w, "Trash"));
    println!("shot {}", rig.shot("toolbar-groups-gallery").display());
    let on_bar = |label: &str| common::placed(w, Role::Button, label).into_iter().filter(|k| k.y < bar.get_height()).count();
    assert_eq!((on_bar("All Photos"), on_bar("EDITOR"), on_bar("BACK")), (0, 0, 0), "no filter on the bar; EDIT names the action");
    assert!(edit.x < select.x, "twin: the measure finds the bar's keys");
    assert!(near(gap(select, prepare), BAR_GAP) && near(gap(prepare, add), BAR_GAP), "{select:?} {prepare:?} {add:?}");
    assert!(near(gap(add, reset), RULED) && near(gap(reset, trash), 0.0), "a rule, then one group, touching: {add:?} {reset:?} {trash:?}");
    assert!(inked(&rig, reset, reset.x - BAR_GAP - 0.5), "the rule drawn between ADD FOLDER… and Reset");
    // Twin: the room between two groups with no rule is bare ground.
    assert!(!inked(&rig, prepare, prepare.x - BAR_GAP / 2.0), "the measure sees bare room");
    // Under the hand Reset lights as any key, PREPARE ROLL's neutral ink; twin: Trash shows its danger's red.
    let (plain_lit, reset_lit, trash_lit) = (reached_chroma(&rig, prepare), reached_chroma(&rig, reset), reached_chroma(&rig, trash));
    assert!(reset_lit <= plain_lit + 6 && trash_lit > 80, "Reset as PREPARE ROLL under the hand, Trash red: {plain_lit} {reset_lit} {trash_lit}");
    // The app's keys one group, touching, EXPORT a bar's gap after them at the bar's end.
    let (report, settings, export) = (bar_key(w, "Feedback"), bar_key(w, "Settings"), bar_key(w, "EXPORT"));
    assert!(near(gap(report, settings), 0.0) && near(gap(settings, export), BAR_GAP), "{report:?} {settings:?} {export:?}");

    // Select All holds lit; Reset still names the face, whatever the count, the bar holding still.
    assert!(!bar.get_whole());
    bar.invoke_act(Act::SelectAll);
    rig.settle();
    assert!(bar.get_whole() && bar.get_targets() == 2, "every photo picked, the key lit");
    let (both, sent) = (bar_key(w, "Reset Pro"), bar_key(w, "EXPORT"));
    let menu_on = |name: &str| {
        let t = oni_app::app::tiles(w).into_iter().find(|t| t.caption == name).unwrap();
        let position = LogicalPosition::new(lib.get_grid_x() + t.x + t.width / 2.0, lib.get_grid_y() + t.y - lib.get_scroll() + t.height / 2.0);
        rig.window.dispatch_event(WindowEvent::PointerMoved { position });
        rig.window.dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Right });
        rig.window.dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Right });
        rig.settle();
    };
    // The photo's menu on both: as wide as its widest line, word and chord, which never elides.
    menu_on("A");
    println!("shot {}", rig.shot("toolbar-groups-menu-two").display());
    let line_width = |text: &str| common::placed(w, Role::Button, text).first().unwrap_or_else(|| panic!("{text}")).width;
    let two = line_width("Move into a New Roll\u{2026} (2 photos)");
    rig.key("\u{1b}");
    assert_eq!((bar_key(w, "Select All").x, both.x, both.width), (select.x, reset.x, reset.width), "the key lights in place, Reset keeps its room");
    assert_eq!((sent.x, sent.width), (export.x, export.width), "EXPORT keeps its room as the count changes");
    bar.invoke_act(Act::SelectAll);
    rig.settle();
    assert!(!bar.get_whole() && bar.get_targets() == 1, "twin: one photo again, the key bare");

    // The photo's menu: Reset in a section of its own, the ONI Trash last and alone, each with the menu bar's chord.
    menu_on("A");
    println!("shot {}", rig.shot("toolbar-groups-menu").display());
    let one = line_width("Move into a New Roll\u{2026}");
    assert!(two > one, "twin: the one photo's menu, its lines fitting, keeps its rest width: {one} {two}");
    let lines: Vec<_> = lib.get_menu_lines().iter().collect();
    let reveal = oni_app::platform::reveal(oni_app::platform::Platform::THIS);
    let foot: Vec<(&str, bool)> = lines[lines.len() - 3..].iter().map(|l| (l.text.as_str(), l.rule)).collect();
    assert_eq!(foot, [(reveal, false), ("Reset Settings", true), ("Move to ONI Trash", true)], "a rule above Reset and above the ONI Trash");
    let k = oni_app::platform::keys(oni_app::platform::Platform::THIS);
    let keys = |text: &str| lines.iter().find(|l| l.text == text).map(|l| l.keys.to_string()).unwrap_or_else(|| panic!("{text}"));
    assert_eq!((keys("Reset Settings"), keys("Move to ONI Trash")), (format!("{}R", k.command_shift), format!("{}{}", k.command, k.delete)));
    assert_eq!(keys("Copy Settings\u{2026}"), format!("{}C", k.command_shift));
    assert!(keys(reveal).is_empty(), "twin: a line with no chord prints none");
    rig.key("\u{1b}");
    // A click on EXPORT opens the sheet on what is picked, the one photo, then both.
    let exported = |key: common::Placed| {
        common::click(&rig, w, key.middle());
        let names: Vec<String> = export_card::card(&ctl).targets.iter().map(|t| t.path.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert!(export_card::card(&ctl).shown, "the sheet opened");
        ctl.leave_export();
        rig.settle();
        names
    };
    assert_eq!(exported(bar_key(w, "EXPORT")), ["A.tif"], "twin: EXPORT sends the one photo picked");
    bar.invoke_act(Act::SelectAll);
    rig.settle();
    assert_eq!(exported(bar_key(w, "EXPORT")), ["A.tif", "B.tif"]);
    bar.invoke_act(Act::SelectAll);
    rig.settle();

    // The editor by job: Select All opening the canvas's views and tools, the faces, the settings carried
    // from photo to photo, then past a rule Reset alone, the ONI Trash off the bar beside a photo.
    assert_eq!(on_bar("Trash"), 1, "twin: the gallery's bar keeps its Trash");
    lib.invoke_tile_opened(tile("A"));
    rig.until("A in the editor", Duration::from_secs(60), || !w.global::<App>().get_gallery() && ctl.state.borrow().renders() >= 1);
    rig.settle();
    let (back, select, crop, de_dust, compare) = (bar_key(w, "BACK"), bar_key(w, "Select All"), bar_key(w, "Crop"), bar_key(w, "De-dust"), bar_key(w, "Compare"));
    assert_eq!((back.x, back.width, on_bar("EDIT"), on_bar("GALLERY")), (edit.x, edit.width, 0, 0), "BACK in EDIT's place, as wide");
    let (copy, paste, to_roll) = (bar_key(w, "Copy"), bar_key(w, "Paste"), bar_key(w, "Paste to Roll"));
    let reset = bar_key(w, "Reset Pro");
    println!("shot {}", rig.shot("toolbar-groups-editor").display());
    // On the Scanner face the key names the Scanner, in the same room; twin: Pro's word is gone.
    bar.invoke_act(Act::FaceScanner);
    rig.settle();
    let scanner = bar_key(w, "Reset Scanner");
    assert_eq!((scanner.x, scanner.width), (reset.x, reset.width), "Reset names the face in the same room");
    assert!(common::placed(w, Role::Button, "Reset Pro").is_empty(), "twin: Pro's Reset is gone on the Scanner face");
    bar.invoke_act(Act::FacePro);
    rig.settle();
    for (a, b) in [(select, crop), (crop, de_dust), (de_dust, compare), (copy, paste), (paste, to_roll)] {
        assert!(near(gap(a, b), 0.0), "one group, its keys touching: {a:?} {b:?}");
    }
    assert_eq!(on_bar("Trash"), 0, "no Trash on the editor's bar");
    assert!(near(gap(to_roll, reset), RULED), "a rule before Reset, alone where the group stood: {to_roll:?} {reset:?}");
    assert!(inked(&rig, reset, reset.x - BAR_GAP - 0.5), "the rule drawn between Paste to Roll and Reset");
    // Beside Reset, where Trash stood, bare room up to the app's keys.
    assert!(!inked(&rig, reset, right(reset) + 12.0), "nothing drawn past Reset");
    // Twin: the faces stand between the views and the settings, a bar's gap either side.
    assert!(gap(compare, copy) > 2.0 * BAR_GAP + 100.0, "{compare:?} {copy:?}");
    assert!(common::placed(w, Role::Button, "Whole Roll").is_empty(), "one name in both modes");

    // Off the bar the ONI Trash stays in reach: the menu bar's chord trashes the frame open, the editor going on.
    let trashed = |name: &str| oni_settings::sidecar::read(&folder.join(name), false, true).unwrap().trashed_at.is_some();
    assert!(!trashed("A.tif"), "twin: nothing trashed before the chord");
    let backspace: slint::SharedString = slint::platform::Key::Backspace.into();
    rig.chord(slint::platform::Key::Control, &backspace);
    rig.landed(&ctl);
    assert!(trashed("A.tif") && !trashed("B.tif"), "the frame open alone moved to the ONI Trash");
    assert!(!w.global::<App>().get_gallery() && ctl.state.borrow().open_path().is_some_and(|o| o.ends_with("B.tif")), "the editor goes on to B");
    let _ = std::fs::remove_dir_all(&root);
}
