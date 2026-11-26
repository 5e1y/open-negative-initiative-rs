//! The welcome's sheet with no stage behind it: in every language each page's headline and key inside the smallest
//! window, the dots a hand's least target, the Collection's gift taken out. The stage's film is `welcome_reel`.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::{Help, MainWindow};
use oni_i18n::{Language, tr};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, PhysicalSize};

use crate::common::{self, Placed};

/// `WelcomeStyle.words-width`, `SheetStyle.margin` and `DS.control-grab-size`.
const WORDS: f32 = 470.0;
const MARGIN: f32 = 24.0;
const REACH: f32 = 24.0;
const PAGES: usize = 6;
/// Each page's headline; the huge gift's page, all stage, names the step it shows.
const HEADLINES: [&str; PAGES] = ["ONI {}", "A livelier interface", "The Collection, with The Lens DB", "Your photo", "Your year on film", "I have a message for you"];
fn headline(window: &MainWindow, page: usize) -> Placed {
    common::the(window, Role::Text, &tr(HEADLINES[page]).replace("{}", "2.0"))
}

fn commit(window: &MainWindow, page: usize) -> Placed {
    // With no stage the overture never ends, and no gift is unwrapped.
    let word = match page {
        0 => "Skip the Opening",
        3 => "Next Step",
        5 => "Enter ONI",
        _ => "Next Gift",
    };
    common::the(window, Role::Button, tr(word))
}

fn dot(window: &MainWindow, page: usize) -> Placed {
    let label = tr("Page {} of {}").replacen("{}", &(page + 1).to_string(), 1).replacen("{}", &PAGES.to_string(), 1);
    common::the(window, Role::Tab, &label)
}

fn tap(rig: &common::Rig, (x, y): (f32, f32)) {
    let position = LogicalPosition::new(x, y);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Left });
    rig.pump();
}

fn sized(rig: &common::Rig, (w, h): (u32, u32)) {
    rig.window.set_size(PhysicalSize::new(w, h));
    rig.settle();
}

pub(crate) fn the_welcome_fills_the_window_in_every_language() {
    let rig = common::rig();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    let window = MainWindow::new().unwrap();
    oni_app::keycaps::serve(&window);
    let help = window.global::<Help>();
    help.set_release("2.0".into());
    help.set_contest_tag("#ONIsplash".into());
    help.set_welcome_shown(true);
    let closed = std::rc::Rc::new(std::cell::Cell::new(false));
    let ear = closed.clone();
    help.on_welcome_closed(move || ear.set(true));
    window.show().unwrap();

    // The smallest window, in each language the interface speaks here: every page's headline and key in it.
    let (w, h) = (1100.0f32, 680.0f32);
    for language in Language::ALL {
        oni_app::language::speak(Some(language));
        help.set_welcome_page(0);
        sized(&rig, (w as u32, h as u32));
        if common::placed(&window, Role::Button, tr("Skip the Opening")).is_empty() {
            println!("{language:?}: the live interface reads English, judged on the app profile");
            continue;
        }
        for page in 0..PAGES {
            help.set_welcome_page(page as i32);
            rig.settle();
            println!("shot {}", rig.shot(&format!("welcome-{}-1100x680-{page}", language.key())).display());
            let (title, key) = (headline(&window, page), commit(&window, page));
            assert!(title.y >= 0.0 && title.y + title.height <= h, "{language:?} page {page}: the headline in the window, {title:?}");
            assert!(key.x + key.width <= w - MARGIN + 0.5 && key.y + key.height <= h - MARGIN + 0.5, "{language:?} page {page}: the key in the window, {key:?}");
            if [1, 2, 4].contains(&page) {
                assert!(title.x >= w - WORDS, "{language:?} page {page}: the words right of the stage, at {}", title.x);
            }
        }
    }
    oni_app::language::speak(Some(Language::English));

    // Each dot's touch is the hand's least target, and a click inside it turns to its page.
    for page in 0..PAGES {
        let touch = dot(&window, page);
        assert!(touch.width >= REACH && touch.height >= REACH, "dot {page}: a touch of {REACH} at the least, {touch:?}");
    }
    let (x, y) = dot(&window, 2).middle();
    tap(&rig, (x, y - REACH / 2.0 - 1.0));
    assert_eq!(help.get_welcome_page(), 0, "twin: a click past the dot's reach turns no page");
    tap(&rig, (x, y - REACH / 2.0 + 1.0));
    assert_eq!(help.get_welcome_page(), 2, "a click at the edge of the dot's reach turns to its page");

    // A short window: the message's words scroll alone and its key stays in reach and answers.
    sized(&rig, (900, 440));
    help.set_welcome_page(PAGES as i32 - 1);
    rig.settle();
    println!("shot {}", rig.shot("welcome-en-900x440-5").display());
    let before = headline(&window, 5);
    let position = LogicalPosition::new(before.x + 40.0, before.y + 10.0);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position });
    rig.window.dispatch_event(WindowEvent::PointerScrolled { position, delta_x: 0.0, delta_y: -2000.0 });
    rig.pump();
    let moved = common::placed(&window, Role::Text, tr(HEADLINES[5])).first().map_or(f32::INFINITY, |after| before.y - after.y);
    assert!(moved > 20.0, "900×440: the message's words scroll, {moved}");
    let key = commit(&window, 5);
    assert!(key.y + key.height <= 440.0 - MARGIN + 0.5, "900×440: the key in the window, {key:?}");
    tap(&rig, key.middle());
    assert!(closed.get(), "the last page's key leaves the welcome");

    // Without the Collection's gift its page is skipped both ways and the dots count five; twin: with it, six.
    sized(&rig, (w as u32, h as u32));
    help.set_welcome_page(1);
    rig.settle();
    dot(&window, 5);
    help.set_kit_gift(false);
    rig.settle();
    rig.key("\n");
    assert_eq!(help.get_welcome_page(), 3, "Return skips the Collection's page");
    rig.key(&slint::SharedString::from(slint::platform::Key::LeftArrow));
    assert_eq!(help.get_welcome_page(), 1, "and the left arrow skips it back");
    let five = tr("Page {} of {}").replacen("{}", "5", 1).replacen("{}", "5", 1);
    common::the(&window, Role::Tab, &five);
    help.set_kit_gift(true);
    window.hide().unwrap();
}
