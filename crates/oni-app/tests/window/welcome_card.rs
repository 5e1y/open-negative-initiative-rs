//! The welcome's sheet with no stage behind it (that is `welcome_reel`): each screen's title and key inside the smallest
//! window in every language, a gift's words right of it, the message scrolling, the Collection taken out.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::{Help, MainWindow};
use oni_i18n::{Language, tr};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, PhysicalSize};

use crate::common::{self, Placed};

/// `SheetStyle.margin`.
const MARGIN: f32 = 24.0;
/// Each screen's title, a gift's once it is opened.
const TITLES: [&str; 5] = ["A livelier interface", "Import your photo kit into ONI, with The Lens DB", "Your digital photos, on film", "Your Wrapped is here", "Tell your friends about ONI."];

fn title(window: &MainWindow, page: usize) -> Placed {
    common::the(window, Role::Text, tr(TITLES[page - 1]))
}

fn commit(window: &MainWindow, page: usize) -> Placed {
    let word = match page {
        4 => "Continue",
        5 => "Enter ONI",
        _ => "Next Gift",
    };
    common::the(window, Role::Button, tr(word))
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

/// Screen `page` with its gift opened long ago, its words in place, the scroll standing on it.
fn shown(rig: &common::Rig, help: &Help, page: usize) {
    help.set_welcome_page(page as i32);
    help.set_scroll((page.min(4) - 1) as f32);
    help.set_message_shown(if page == 5 { 1.0 } else { 0.0 });
    help.set_opened_time(30.0);
    help.set_words_at(0.0);
    help.set_may_go_on(true);
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
    help.set_made_with("#madewithoni".into());
    help.set_welcome_shown(true);
    let closed = std::rc::Rc::new(std::cell::Cell::new(false));
    let ear = closed.clone();
    help.on_welcome_closed(move || ear.set(true));
    window.show().unwrap();

    // The smallest window, in each language the interface speaks here: every screen's title and key in it.
    let (w, h) = (1100.0f32, 680.0f32);
    for language in Language::ALL {
        oni_app::language::speak(Some(language));
        sized(&rig, (w as u32, h as u32));
        shown(&rig, &help, 1);
        if common::placed(&window, Role::Button, tr("Next Gift")).is_empty() {
            println!("{language:?}: the live interface reads English, judged on the app profile");
            continue;
        }
        for page in 1..=5 {
            shown(&rig, &help, page);
            println!("shot {}", rig.shot(&format!("welcome-{}-1100x680-{page}", language.key())).display());
            let (title, key) = (title(&window, page), commit(&window, page));
            assert!(title.y >= 0.0 && title.y + title.height <= h, "{language:?} screen {page}: the title in the window, {title:?}");
            assert!(key.x + key.width <= w - MARGIN + 0.5 && key.y + key.height <= h - MARGIN + 0.5, "{language:?} screen {page}: the key in the window, {key:?}");
            if page <= 4 {
                assert!(title.x >= w / 2.0 && title.x + title.width <= w, "{language:?} screen {page}: the words right of the gift, at {}", title.x);
            }
        }
    }
    oni_app::language::speak(Some(Language::English));

    // Wrapped, a gift's key unwraps it; twin: opened, it goes on.
    help.set_opened_time(-1.0);
    rig.settle();
    common::the(&window, Role::Button, tr("Unwrap"));
    assert!(common::placed(&window, Role::Button, tr("Next Gift")).is_empty(), "no way on past a wrapped gift");

    // A short window: the message's words scroll alone and its key stays in reach and answers.
    sized(&rig, (900, 440));
    shown(&rig, &help, 5);
    println!("shot {}", rig.shot("welcome-en-900x440-5").display());
    let before = title(&window, 5);
    let position = LogicalPosition::new(before.x + 40.0, before.y + 10.0);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position });
    rig.window.dispatch_event(WindowEvent::PointerScrolled { position, delta_x: 0.0, delta_y: -2000.0 });
    rig.pump();
    let moved = common::placed(&window, Role::Text, tr(TITLES[4])).first().map_or(f32::INFINITY, |after| before.y - after.y);
    assert!(moved > 20.0, "900×440: the message's words scroll, {moved}");
    let key = commit(&window, 5);
    assert!(key.y + key.height <= 440.0 - MARGIN + 0.5, "900×440: the key in the window, {key:?}");
    tap(&rig, key.middle());
    assert!(closed.get(), "the last screen's key leaves the welcome");

    // Without the Collection's gift the rail counts three gifts; twin: with it, four.
    sized(&rig, (w as u32, h as u32));
    shown(&rig, &help, 1);
    common::the(&window, Role::Text, &tr("Screen {} of {}").replacen("{}", "1", 1).replacen("{}", "4", 1));
    help.set_kit_gift(false);
    rig.settle();
    common::the(&window, Role::Text, &tr("Screen {} of {}").replacen("{}", "1", 1).replacen("{}", "3", 1));
    help.set_kit_gift(true);
    window.hide().unwrap();
}
