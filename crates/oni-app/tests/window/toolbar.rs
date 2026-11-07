//! The top bar as the window widens: its keys keep their widths and only the room between them grows;
//! one layout in every mode, whatever the selection, the picture's right click opening its menu; its groups.

use oni_app::app::{Options, Shell};
use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::{Act, App, Bar, Library, Preparing};
use oni_core::{Archive, EditorStore};
use slint::platform::software_renderer::Rgb565Pixel;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

/// The window drawn at `w` by 800, as rows of RGB565 pixels.
fn drawn(rig: &common::Rig, w: u32) -> (usize, Vec<u16>) {
    rig.window.set_size(slint::PhysicalSize::new(w, 800));
    rig.settle();
    rig.window.request_redraw();
    let (w, h) = (w as usize, 800);
    let mut buffer = vec![Rgb565Pixel(0); w * h];
    rig.window.draw_if_needed(|r| {
        r.render(&mut buffer, w);
    });
    (w, buffer.into_iter().map(|p| p.0).collect())
}

/// Along the first row under the top edges of the bar's last four keys, a keycap and the flat keys
/// standing on its middle, from the window's right edge leftwards, the runs of key and of ground: four
/// keys with their gaps, then the room before them.
fn runs(w: usize, px: &[u16]) -> Vec<usize> {
    let at = |x: usize, y: usize| px[y * w + x];
    let x = w - 30;
    let ground = at(x, 44);
    let top = (44..120).find(|&y| at(x, y) != ground).expect("a key at the bar's end") + 2;
    let along = |y: usize| {
        let key = |i: usize| at(i, y) != ground;
        let mut out = Vec::new();
        let mut i = (x..w).find(|&i| !key(i)).unwrap_or(w);
        while out.len() < 8 && i > 0 {
            let kind = key(i - 1);
            let start = (0..i).rev().find(|&j| key(j) != kind).map_or(0, |j| j + 1);
            out.push(i - start);
            i = start;
        }
        out
    };
    // The cap's neighbour nearer than the cap is wide: a key across the bar is none of the four.
    (top..top + 12).map(along).find(|r| r.len() == 8 && r[1] < r[0]).expect("a row crossing the four keys")
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
    // ADD FOLDER, the gear, the phone and the feedback key with their gaps, then the room before them.
    let keys = |w: u32| {
        let (w, px) = drawn(&rig, w);
        runs(w, &px)
    };
    // Both bars hold every word of the centre, so only the room between the keys can change.
    let (narrow, wide) = (keys(1728), keys(1990));
    println!("shot {}", rig.shot("toolbar-1990").display());
    assert_eq!(narrow[..7], wide[..7], "the keys and their gaps keep their widths: {narrow:?} {wide:?}");
    assert_eq!((narrow[2], narrow[4]), (narrow[2], narrow[2]), "the gear and the phone are square keys alike");
    // Twin: the room before them does grow with the window.
    assert!(wide[7] > narrow[7] + 100, "the room before the keys grows: {narrow:?} {wide:?}");
    drop(shell);

    // The phone switch off: the gear's gap meets the feedback key, with no phone key between them.
    let shell = bar(false);
    let hidden = keys(1728);
    assert_eq!((&hidden[..3], &hidden[3..5]), (&narrow[..3], &narrow[5..7]), "no phone key: {hidden:?} {narrow:?}");
    drop(shell);
}

/// The keys along the bar's middle row, as spans of x from the window's left edge, each by its face,
/// whose strokes stand at most `STROKE_GAP` pixels apart.
fn spans(w: usize, px: &[u16]) -> Vec<(usize, usize)> {
    const STROKE_GAP: usize = 4;
    let at = |x: usize, y: usize| px[y * w + x];
    let ground = at(30, 44);
    let top = (44..120).find(|&y| at(30, y) != ground).expect("the mode key");
    // The cap's last row within its 40 pt: a dished face's top may match the ground.
    let foot = (top..top + 40).rev().find(|&y| at(30, y) != ground).map_or(top + 20, |y| y + 1);
    let row = (top + foot) / 2;
    let ground = at(2, row);
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut x = 0;
    while x < w {
        let start = x;
        while x < w && at(x, row) != ground {
            x += 1;
        }
        if x > start {
            match out.last_mut() {
                Some(last) if start - last.1 <= STROKE_GAP => last.1 = x,
                _ => out.push((start, x)),
            }
        }
        x += 1;
    }
    out
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
    let keys = || {
        let (w, px) = drawn(&rig, 1100);
        spans(w, &px)
    };
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
    assert_eq!((&gallery[..2], &preparing[..2]), (&editor[..2], &editor[..2]), "{gallery:?} {preparing:?} {editor:?}");
    let ends = |s: &[(usize, usize)], slot: usize| s[s.len() - slot - 2..s.len() - slot].to_vec();
    assert_eq!((ends(&gallery, 1), ends(&preparing, 0)), (ends(&editor, 1), ends(&editor, 1)), "{gallery:?} {preparing:?} {editor:?}");
    assert_eq!(gallery.last(), editor.last(), "ADD FOLDER and EXPORT share one slot");
    assert!(editor.last().unwrap().1 <= 1100 - 8, "EXPORT ends inside the window: {editor:?}");
    // Twin: the keys between them are the mode's own, which a bar laid once would not show.
    assert_ne!(gallery[2..gallery.len() - 3], editor[2..editor.len() - 3]);

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

/// Swift's `DS.Space.sm` between the keys of one group, and three times that between the bar's groups.
const GROUP_GAP: f32 = 8.0;
const BAR_GAP: f32 = 24.0;
/// The rule before Reset and Trash: an engraved groove and its lit edge, a bar's gap either side.
const RULED: f32 = 2.0 * BAR_GAP + 2.0;
/// The glyph a key shows before its word.
const GLYPH: f32 = 16.0;

/// The one key of the bar so named, where the layout put it.
fn bar_key(w: &oni_app::MainWindow, label: &str) -> common::Placed {
    let keys: Vec<_> = common::placed(w, Role::Button, label).into_iter().filter(|k| k.y < w.global::<Bar>().get_height()).collect();
    assert_eq!(keys.len(), 1, "one {label:?} on the bar: {keys:?}");
    keys[0]
}

/// How far into its key a key's word starts.
fn word_at(w: &oni_app::MainWindow, key: common::Placed, text: &str) -> f32 {
    let words: Vec<_> = common::placed(w, Role::Text, text).into_iter().filter(|t| key.level_with(t) && t.x >= key.x && t.x < key.x + key.width).collect();
    assert_eq!(words.len(), 1, "{text:?} written once on its key {key:?}: {words:?}");
    words[0].x - key.x
}

/// Whether the bar draws anything at `x` on the middle row of `key`, the window as it stands.
fn inked(rig: &common::Rig, key: common::Placed, x: f32) -> bool {
    let (w, px) = rig.drawn();
    let y = (key.y + key.height / 2.0) as usize;
    px[y * w + x as usize].0 != px[y * w + 2].0
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

    // The gallery: Select All opening the filter's group, PREPARE ROLL, then past a rule Reset naming the
    // photo it reaches and Trash last, one group.
    let (select, all) = (bar_key(w, "Select All"), bar_key(w, "All Photos"));
    let (prepare, reset, trash) = (bar_key(w, "PREPARE ROLL"), bar_key(w, "Reset Photo"), bar_key(w, "Trash"));
    println!("shot {}", rig.shot("toolbar-groups-gallery").display());
    assert!(near(gap(select, all), GROUP_GAP), "Select All and the filter one group: {select:?} {all:?}");
    assert!(near(gap(prepare, reset), RULED) && near(gap(reset, trash), GROUP_GAP), "a rule, then one group: {prepare:?} {reset:?} {trash:?}");
    assert!(inked(&rig, reset, reset.x - BAR_GAP - 1.0), "the rule drawn between PREPARE ROLL and Reset");
    // Twin: the room between two groups with no rule is bare ground.
    assert!(!inked(&rig, prepare, prepare.x - BAR_GAP / 2.0), "the measure sees bare room");
    // Trash wears its can before its word; a key of a word alone starts it at its edge's padding.
    let (can, bare) = (word_at(w, trash, "Trash"), word_at(w, select, "Select All"));
    assert!(can >= bare + GLYPH, "Trash's word after its can: {can} against {bare}");
    // The app's glyphs one group, the commit's slot a bar's gap after them.
    let (report, settings, add) = (bar_key(w, "Report a Bug / Give Feedback"), bar_key(w, "Settings"), bar_key(w, "ADD FOLDER\u{2026}"));
    assert!(near(gap(report, settings), GROUP_GAP) && near(gap(settings, add), BAR_GAP), "{report:?} {settings:?} {add:?}");

    // Select All latches with its lamp; Reset then names both photos, the bar holding still.
    assert!(!bar.get_whole());
    bar.invoke_act(Act::SelectAll);
    rig.settle();
    assert!(bar.get_whole() && bar.get_targets() == 2, "every photo picked, the lamp lit");
    let both = bar_key(w, "Reset 2 Photos");
    assert!(common::placed(w, Role::Button, "Reset Photo").is_empty(), "twin: the one photo's Reset is gone");
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
    assert_eq!((bar_key(w, "Select All").x, both.x, both.width), (select.x, reset.x, reset.width), "the lamp lights in place, Reset keeps its room");
    bar.invoke_act(Act::SelectAll);
    rig.settle();
    assert!(!bar.get_whole() && bar.get_targets() == 1, "twin: one photo again, the lamp dark");

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

    // The editor by job: Select All opening the canvas's views and tools, the faces, the settings carried
    // from photo to photo, then past a rule Reset and Trash, one group, Trash last.
    lib.invoke_tile_opened(tile("A"));
    rig.until("A in the editor", Duration::from_secs(60), || !w.global::<App>().get_gallery() && ctl.state.borrow().renders() >= 1);
    rig.settle();
    let (select, crop, de_dust, compare) = (bar_key(w, "Select All"), bar_key(w, "Crop"), bar_key(w, "De-dust"), bar_key(w, "Compare"));
    let (copy, paste, to_roll) = (bar_key(w, "Copy"), bar_key(w, "Paste"), bar_key(w, "Paste to Roll"));
    let (reset, trash) = (bar_key(w, "Reset Photo"), bar_key(w, "Trash"));
    println!("shot {}", rig.shot("toolbar-groups-editor").display());
    for (a, b) in [(select, crop), (crop, de_dust), (de_dust, compare), (copy, paste), (paste, to_roll), (reset, trash)] {
        assert!(near(gap(a, b), GROUP_GAP), "one group, a group's gap apart: {a:?} {b:?}");
    }
    assert!(near(gap(to_roll, reset), RULED), "a rule before Reset and Trash: {to_roll:?} {reset:?}");
    assert!(inked(&rig, reset, reset.x - BAR_GAP - 1.0), "the rule drawn between Paste to Roll and Reset");
    // Twin: the faces stand between the views and the settings, a bar's gap either side.
    assert!(gap(compare, copy) > 2.0 * BAR_GAP + 100.0, "{compare:?} {copy:?}");
    let (can, bare) = (word_at(w, trash, "Trash"), word_at(w, copy, "Copy"));
    assert!(can >= bare + GLYPH, "twin of the gallery's: Trash's can, Copy's word alone: {can} {bare}");
    assert!(common::placed(w, Role::Button, "Whole Roll").is_empty(), "one name in both modes");
    let _ = std::fs::remove_dir_all(&root);
}
