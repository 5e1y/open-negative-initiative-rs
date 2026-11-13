//! The welcome's card at the smallest window and below: the stage on the left of the words, the
//! footer inside the card and the window, the words still in every language; in a short window the
//! words scroll alone and NEXT stays in reach. Twins: a page too long scrolls, a short window's card
//! is shorter than the stage.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::{Help, MainWindow};
use oni_i18n::{Language, tr};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, PhysicalSize};
use std::path::PathBuf;

use crate::common::{self, Placed};

/// `WelcomeStyle`: the stage, the words beside it, the card they make; `SheetStyle`'s margin and rest.
const STAGE: f32 = 560.0;
const CARD: (f32, f32) = (960.0, 502.0);
const MARGIN: f32 = 24.0;
const REST: f32 = 0.42;
/// A word each page shows once, at the top of its words.
const FIRSTS: [&str; 4] = [
    "A quieter interface around your scans, dust found on its own, and about 70 fixes.",
    "A calmer interface",
    "Automatic De-dust",
    "About 70 fixes, and what changes",
];

fn first(window: &MainWindow, page: usize) -> Placed {
    common::the(window, Role::Text, tr(FIRSTS[page]))
}

fn commit(window: &MainWindow, page: usize) -> Placed {
    common::the(window, Role::Button, if page == 3 { tr("GET STARTED") } else { tr("Next") })
}

/// How far the page's first words move under a wheel turned down over the words, then back to the top.
fn scrolled(rig: &common::Rig, window: &MainWindow, page: usize) -> f32 {
    let before = first(window, page);
    let position = LogicalPosition::new(before.x + 40.0, before.y + 10.0);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position });
    rig.window.dispatch_event(WindowEvent::PointerScrolled { position, delta_x: 0.0, delta_y: -2000.0 });
    rig.pump();
    // Scrolled out of the card, the first words are no longer shown at all.
    let moved = common::placed(window, Role::Text, tr(FIRSTS[page])).first().map_or(f32::INFINITY, |after| before.y - after.y);
    rig.window.dispatch_event(WindowEvent::PointerScrolled { position, delta_x: 0.0, delta_y: 4000.0 });
    rig.pump();
    moved
}

fn sized(rig: &common::Rig, (w, h): (u32, u32)) {
    rig.window.set_size(PhysicalSize::new(w, h));
    rig.settle();
}

pub(crate) fn the_welcome_card_fits_the_window_and_only_its_words_scroll() {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    let window = MainWindow::new().unwrap();
    oni_app::keycaps::serve(&window);
    let help = window.global::<Help>();
    help.set_release("1.6".into());
    help.set_welcome_line("V1.6.0".into());
    help.set_welcome_credit("©ultra_detente".into());
    help.set_welcome_shown(true);
    window.show().unwrap();

    // The smallest window, in each language the interface speaks here: the card whole, nothing to scroll.
    let (w, h) = (1100.0f32, 680.0f32);
    let (left, top) = ((w - CARD.0) / 2.0, (h - CARD.1) * REST);
    for language in Language::ALL {
        oni_app::language::speak(Some(language));
        help.set_welcome_page(0);
        sized(&rig, (w as u32, h as u32));
        if common::placed(&window, Role::Button, tr("Next")).is_empty() {
            println!("{language:?}: the live interface reads English, judged on the app profile");
            continue;
        }
        for page in 0..4 {
            help.set_welcome_page(page as i32);
            rig.settle();
            println!("shot {}", rig.shot(&format!("welcome-{}-1100x680-{page}", language.key())).display());
            let (words, key) = (first(&window, page), commit(&window, page));
            if page < 3 {
                assert!(words.x >= left + STAGE, "{language:?} page {page}: the words right of the stage, at {}", words.x);
            } else {
                assert!(words.x < left + STAGE / 4.0, "{language:?}: the last page's words take the card, at {}", words.x);
            }
            assert!(key.x + key.width <= left + CARD.0 && key.y + key.height <= top + CARD.1 + 0.5, "{language:?} page {page}: the key inside the card, {key:?}");
            let moved = scrolled(&rig, &window, page);
            assert!(moved.abs() < 0.5, "{language:?} page {page}: the words fit, a wheel moved them {moved}");
        }
    }
    oni_app::language::speak(Some(Language::English));

    // Twin: a heading far longer than any page's does scroll, so the wheel above could see it.
    help.set_welcome_page(0);
    help.set_release("1.6 ".repeat(80).into());
    rig.settle();
    assert!(scrolled(&rig, &window, 0) > 20.0, "twin: a page too long for the card scrolls");
    help.set_release("1.6".into());

    // Short windows: the stage crops, the words scroll alone, the key stays in the window and answers.
    for (w, h) in [(1728u32, 1017u32), (900, 560), (900, 440)] {
        help.set_welcome_page(0);
        sized(&rig, (w, h));
        for page in 0..4 {
            help.set_welcome_page(page as i32);
            rig.settle();
            println!("shot {}", rig.shot(&format!("welcome-en-{w}x{h}-{page}")).display());
            let key = commit(&window, page);
            assert!(key.y + key.height <= h as f32 - MARGIN + 0.5 && key.x + key.width <= w as f32 - MARGIN, "{w}×{h} page {page}: the key in the window, {key:?}");
        }
    }
    help.set_welcome_page(3);
    rig.settle();
    assert!(scrolled(&rig, &window, 3) > 20.0, "900×440: the last page's words scroll");
    help.set_welcome_page(0);
    rig.settle();
    let (x, y) = commit(&window, 0).middle();
    let position = LogicalPosition::new(x, y);
    rig.window.dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Left });
    rig.pump();
    assert_eq!(help.get_welcome_page(), 1, "900×440: NEXT answers a click inside the window");

    // Twin: in a window shorter than the stage, the card gives up height between its margins.
    sized(&rig, (1100, 300));
    let (words, key) = (first(&window, 1), commit(&window, 1));
    assert!(words.y >= MARGIN && key.y + key.height <= 300.0 - MARGIN + 0.5, "the card between the margins: {words:?} {key:?}");
    assert!(key.y + key.height - words.y < CARD.1 - 2.0 * MARGIN, "twin: the card is shorter than the stage");
    window.hide().unwrap();
}
