//! Sheets one over another: the top one holds the keyboard, and closing it hands the keyboard to the
//! one beneath, whose arrows turn its pages again; the welcome's dots name each page and the one shown.

use i_slint_backend_testing::{AccessibleRole as Role, ElementHandle};
use oni_app::{Help, MainWindow, Upgraded};
use oni_i18n::tr;
use slint::platform::{Key, WindowEvent};
use slint::{ComponentHandle, PhysicalSize, SharedString};
use std::path::PathBuf;

use crate::common;

const PAGES: usize = 4;

fn right(rig: &common::Rig) {
    rig.key(&SharedString::from(Key::RightArrow));
}

/// The dot naming page `n` (from 1) of `count`, as the catalogue says it.
fn dot_name(n: usize, count: usize) -> String {
    tr("Page {} of {}").replacen("{}", &n.to_string(), 1).replacen("{}", &count.to_string(), 1)
}

/// The dots found by their names, each with whether it reads as the shown page.
fn dots(window: &MainWindow, count: usize) -> Vec<Option<bool>> {
    (1..=count)
        .map(|n| {
            let found: Vec<_> = ElementHandle::find_by_accessible_label(window, &dot_name(n, count)).filter(|e| e.accessible_role() == Some(Role::Tab)).collect();
            found.first().map(|e| e.accessible_item_selected() == Some(true))
        })
        .collect()
}

pub(crate) fn a_sheet_closing_over_another_hands_it_the_keyboard() {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    let window = MainWindow::new().unwrap();
    // The interface and the names asked of it in one language, whatever the system's.
    oni_app::language::speak(Some(oni_i18n::Language::English));
    oni_app::keycaps::serve(&window);
    let (help, upgraded) = (window.global::<Help>(), window.global::<Upgraded>());
    help.set_release("1.6".into());
    help.set_welcome_shown(true);
    window.show().unwrap();
    rig.window.set_size(PhysicalSize::new(1280, 800));
    rig.settle();
    right(&rig);
    assert_eq!(help.get_welcome_page(), 1, "alone, the welcome's arrows turn its pages");

    // The dots: each named by its place among the four, the shown one alone selected, following a turn.
    let shown = |page: usize| (0..PAGES).map(|i| Some(i == page)).collect::<Vec<_>>();
    assert_eq!(dots(&window, PAGES), shown(1), "every dot named, page 2 selected");
    right(&rig);
    assert_eq!(dots(&window, PAGES), shown(2), "the selection follows the page");
    // Twin: a name the dots do not carry is not found, so a dot without its name would fail above.
    assert!(dots(&window, PAGES - 1).iter().all(Option::is_none), "no dot names a count of three");

    // A sheet over it takes the keyboard: the arrows no longer reach the welcome.
    upgraded.set_shown(true);
    rig.settle();
    println!("shot {}", rig.shot("sheets-stacked").display());
    right(&rig);
    assert_eq!(help.get_welcome_page(), 2, "twin: under another sheet the welcome hears no arrow");

    // Closed, the top sheet hands the keyboard to the welcome beneath, whose arrows turn it again.
    upgraded.set_shown(false);
    rig.settle();
    right(&rig);
    assert_eq!(help.get_welcome_page(), 3, "the welcome holds the keyboard again once the sheet over it closes");
    window.hide().unwrap();
}
