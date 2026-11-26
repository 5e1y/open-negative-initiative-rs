//! Sheets one over another: the top one holds the keyboard, and closing it hands the keyboard to the
//! one beneath, whose keys reach it again; the welcome's rail names the screen shown.

use i_slint_backend_testing::ElementHandle;
use oni_app::{Help, MainWindow, Upgraded};
use oni_i18n::tr;
use slint::platform::{Key, WindowEvent};
use slint::{ComponentHandle, PhysicalSize, SharedString};

use crate::common;

/// The gifts' screens the rail counts.
const SCREENS: usize = 4;

fn down(rig: &common::Rig) {
    rig.key(&SharedString::from(Key::DownArrow));
}

/// Whether the rail names screen `n` (from 1) of `count`, as the catalogue says it.
fn rail(window: &MainWindow, n: usize, count: usize) -> bool {
    let name = tr("Screen {} of {}").replacen("{}", &n.to_string(), 1).replacen("{}", &count.to_string(), 1);
    ElementHandle::find_by_accessible_label(window, &name).next().is_some()
}

pub(crate) fn a_sheet_closing_over_another_hands_it_the_keyboard() {
    let rig = common::rig();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    let window = MainWindow::new().unwrap();
    // The interface and the names asked of it in one language, whatever the system's.
    oni_app::language::speak(Some(oni_i18n::Language::English));
    oni_app::keycaps::serve(&window);
    let (help, upgraded) = (window.global::<Help>(), window.global::<Upgraded>());
    help.set_release("2.0".into());
    help.set_welcome_shown(true);
    // The app moves the screens; here each step the keys ask turns the screen at once.
    let turned = window.as_weak();
    help.on_welcome_go(move |by| {
        let window = turned.unwrap();
        let help = window.global::<Help>();
        help.set_welcome_page((help.get_welcome_page() + by).clamp(1, SCREENS as i32));
        help.set_scroll((help.get_welcome_page() - 1) as f32);
    });
    window.show().unwrap();
    rig.window.set_size(PhysicalSize::new(1280, 800));
    rig.settle();
    // Its default key holds the keyboard for Return, unringed: no key was pressed to bring it there.
    let rings = || ElementHandle::find_by_element_type_name(&window, "FocusRing").count();
    assert_eq!(rings(), 0, "the default key holds the keyboard without a ring");
    down(&rig);
    assert_eq!(help.get_welcome_page(), 2, "alone, the welcome hears its keys");
    // Twin: Tab brings the keyboard to a key, which rings; the first, from the sheet's last key, only
    // crosses the window's wall back into the sheet.
    rig.key("\t");
    rig.key("\t");
    assert_eq!(rings(), 1, "Tab rings the key it gives the keyboard to");

    // The rail names the screen shown, following a turn; twin: it names no other count.
    rig.settle();
    assert!(rail(&window, 2, SCREENS), "the rail names screen 2 of 4");
    down(&rig);
    rig.settle();
    assert!(rail(&window, 3, SCREENS) && !rail(&window, 3, SCREENS - 1), "the rail follows the screen");

    // A sheet over it takes the keyboard: the keys no longer reach the welcome.
    upgraded.set_shown(true);
    rig.settle();
    println!("shot {}", rig.shot("sheets-stacked").display());
    down(&rig);
    assert_eq!(help.get_welcome_page(), 3, "twin: under another sheet the welcome hears no key");

    // Closed, the top sheet hands the keyboard to the welcome beneath, whose keys reach it again.
    upgraded.set_shown(false);
    rig.settle();
    down(&rig);
    assert_eq!(help.get_welcome_page(), 4, "the welcome holds the keyboard again once the sheet over it closes");
    window.hide().unwrap();
}
