//! The Settings pages' rows of keys at the smallest window, in every language: each key inside its page,
//! a row too long for one line carried onto the next by its layout, a row that fits kept on one, a key
//! opening a sheet closing Settings first. Twins: a row that would run past the page on one line is met,
//! a key opening no sheet leaves Settings open, a wide probe keeps its keys on one line.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::{Act, AppSettings, MainWindow, RowsProbe};
use oni_i18n::{Language, tr};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, PhysicalSize};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::common::{self, Placed};

/// `ShellStyle.min-width` and `min-height`: the smallest window the app opens at.
const SMALLEST: (u32, u32) = (1100, 680);
/// `SheetStyle.settings-width` and `padding`: the card's width and its pages' inset from its right edge.
const CARD: f32 = 640.0;
const PADDING: f32 = 24.0;
/// `ButtonStyle.group-gap` between the keys of a row.
const GROUP_GAP: f32 = 8.0;
/// Each page's rows of keys by its section, the phone's aside as it shows only where pairing is offered.
const ROWS: [(i32, &[&str]); 3] = [
    (2, &["Export\u{2026}", "Use a File\u{2026}"]),
    (4, &["Check for Updates", "What's New"]),
    (4, &["Welcome", "Report a Problem\u{2026}", "Diagnostic Log"]),
];

pub(crate) fn every_settings_key_stays_on_its_page_in_every_language() {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    let window = MainWindow::new().unwrap();
    oni_app::keycaps::serve(&window);
    rig.window.set_size(PhysicalSize::new(SMALLEST.0, SMALLEST.1));
    window.show().unwrap();
    let settings = window.global::<AppSettings>();
    settings.set_shown(true);
    let right = (SMALLEST.0 as f32 + CARD) / 2.0 - PADDING;
    // The rows that would have run past the page on one line, each found on two.
    let mut carried = 0;
    // Whether a language besides English was read, which only the compiled interface translates.
    let mut translated = false;
    for language in Language::ALL {
        oni_app::language::speak(Some(language));
        settings.set_section(4);
        rig.settle();
        if language != Language::English && common::placed(&window, Role::Button, tr("Diagnostic Log")).is_empty() {
            println!("{language:?}: the live interface reads English, judged on the app profile");
            continue;
        }
        translated |= language != Language::English;
        for (i, (section, row)) in ROWS.into_iter().enumerate() {
            settings.set_section(section);
            rig.settle();
            println!("shot {}", rig.shot(&format!("settings-{}-{section}-{i}", language.key())).display());
            let keys: Vec<Placed> = row.iter().map(|key| common::the(&window, Role::Button, tr(key))).collect();
            for (word, key) in row.iter().zip(&keys) {
                assert!(key.x + key.width <= right + 0.5, "{language:?}: {word:?} ends at {}, past the page's edge at {right}", key.x + key.width);
            }
            let line = keys.iter().map(|key| key.width).sum::<f32>() + GROUP_GAP * (keys.len() - 1) as f32;
            let level = keys.iter().all(|key| key.level_with(&keys[0]));
            if keys[0].x + line > right + 0.5 {
                assert!(!level, "{language:?}: {row:?} too long for one line, carried onto the next: {keys:?}");
                carried += 1;
            } else {
                assert!(level, "{language:?}: {row:?} fits on one line and keeps it: {keys:?}");
            }
        }
    }
    oni_app::language::speak(Some(Language::English));
    // Twin: the compiled interface's French Help row is too long for one line, so the reading meets a row
    // that would overflow; the live one reads English alone, every row on one line.
    assert!(!translated || carried > 0, "twin: some language's row would run past the page on one line");

    // A key opening a sheet of its own closes Settings first; twin: Frontier's file keys leave it open.
    let heard = Rc::new(RefCell::new(Vec::new()));
    let (closes, flips) = (heard.clone(), heard.clone());
    settings.on_close(move || closes.borrow_mut().push(None));
    settings.on_flip(move |act| flips.borrow_mut().push(Some(act)));
    for (section, key, expected) in [(4, "Diagnostic Log", vec![None, Some(Act::Console)]), (2, "Use a File\u{2026}", vec![Some(Act::FrontierChoose)])] {
        settings.set_section(section);
        rig.settle();
        heard.borrow_mut().clear();
        common::click(&rig, &window, common::the(&window, Role::Button, key).middle());
        assert_eq!(*heard.borrow(), expected, "{key:?} pressed");
    }
    window.hide().unwrap();
}

/// A row of keys too long for its width lays them on lines a group's gap apart, the words under it below
/// the last; twin: wide enough, the same keys keep one line.
pub(crate) fn a_row_too_long_for_its_width_lays_its_keys_on_lines_apart() {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    let probe = RowsProbe::new().unwrap();
    oni_app::keycaps::serve(&probe);
    let words = ["Welcome", "Report a Problem", "Diagnostic Log"];
    probe.set_keys(slint::ModelRc::from(words.map(slint::SharedString::from)));
    probe.set_words("Below the keys".into());
    let laid = |width: u32| {
        rig.window.set_size(PhysicalSize::new(width, 300));
        rig.settle();
        let keys: Vec<Placed> = words.iter().map(|k| common::the(&probe, Role::Button, k)).collect();
        (keys, common::the(&probe, Role::Text, "Below the keys"))
    };
    probe.show().unwrap();
    let (keys, words) = laid(300);
    println!("shot {} {keys:?} {words:?}", rig.shot("rows-probe-300").display());
    let (first, last) = (&keys[0], &keys[2]);
    assert!(!last.level_with(first), "300 wide: the last key on a line of its own, {keys:?}");
    assert!(last.y >= first.y + first.height + GROUP_GAP - 0.5, "the second line a group's gap under the first: {keys:?}");
    assert!(words.y >= last.y + last.height, "the words below the last line: {words:?} {keys:?}");
    // Twin: wide enough, the same reading finds every key on one line.
    let (keys, _) = laid(600);
    assert!(keys.iter().all(|key| key.level_with(&keys[0])), "twin: 600 wide, one line: {keys:?}");
    probe.hide().unwrap();
}
