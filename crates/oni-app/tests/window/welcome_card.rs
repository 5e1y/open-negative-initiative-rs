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
const TITLES: [&str; 5] = [
    "ONI's interface comes alive",
    "Import your photo kit into ONI, with The Lens DB",
    "Your digital photos, on film",
    "Your Wrapped is here",
    "You already paid for your film. You shouldn't have to pay just to look at it.",
];
/// Each gift's tag while it is wrapped.
const TAGS: [&str; 4] = ["A small gift", "Another small gift", "The huge gift", "The mystery gift"];

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
    help.set_scroll((page - 1) as f32);
    help.set_welcome_from(page as i32);
    help.set_message_turn(if page == 5 { 1.0 } else { 0.0 });
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
    common::the(&window, Role::Button, tr("Open the Gift"));
    assert!(common::placed(&window, Role::Button, tr("Next Gift")).is_empty(), "no way on past a wrapped gift");

    // A short window: the message stands inside it and its key answers; first its announcement alone, which the key
    // turns to what it says.
    sized(&rig, (900, 440));
    shown(&rig, &help, 5);
    help.set_message_turn(0.0);
    rig.settle();
    let asked = std::rc::Rc::new(std::cell::Cell::new(false));
    let ear = asked.clone();
    help.on_message_on(move || ear.set(true));
    tap(&rig, common::the(&window, Role::Button, tr("Continue")).middle());
    assert!(asked.get() && !closed.get(), "the announcement's key turns the message, never leaves");
    help.set_message_turn(1.0);
    rig.settle();
    println!("shot {}", rig.shot("welcome-en-900x440-5").display());
    let hook = title(&window, 5);
    assert!(hook.y >= 0.0, "900×440: the message's hook in the window, {hook:?}");
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

/// A frame from the middle of gift `gift`'s film, as the welcome shows it.
fn film_frame(gift: usize) -> slint::Image {
    let name = ["interface", "collection", "huge", "wrapped"][gift];
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../oni-ui/ui/welcome");
    let list = std::fs::read_to_string(dir.join(format!("film-{name}.txt"))).unwrap();
    let bytes = std::fs::read(dir.join(format!("film-{name}.webps"))).unwrap();
    let lengths: Vec<usize> = list.lines().skip(1).map(|l| l.split(' ').nth(1).unwrap().parse().unwrap()).collect();
    let at: usize = lengths[..lengths.len() / 2].iter().sum();
    let mut decoder = image_webp::WebPDecoder::new(std::io::Cursor::new(&bytes[at..at + lengths[lengths.len() / 2]])).unwrap();
    let (width, height) = decoder.dimensions();
    let mut rgb = vec![0; decoder.output_buffer_size().unwrap()];
    decoder.read_image(&mut rgb).unwrap();
    slint::Image::from_rgb8(slint::SharedPixelBuffer::clone_from_slice(&rgb, width, height))
}

/// The rectangle every text and key shown in a column starting at `left` takes, above `below`: a screen's block of
/// words, from its eyebrow to its last key.
fn block(window: &MainWindow, left: f32, below: f32) -> Placed {
    let parts: Vec<Placed> = [Role::Text, Role::Button]
        .into_iter()
        .flat_map(|role| common::placed_all(window, role))
        .filter(|p| (p.x - left).abs() < 1.5 && p.y + p.height <= below && p.height > 0.0)
        .collect();
    let (top, bottom) = parts.iter().fold((f32::MAX, f32::MIN), |(t, b), p| (t.min(p.y), b.max(p.y + p.height)));
    let right = parts.iter().fold(left, |r, p| r.max(p.x + p.width));
    Placed { x: left, y: top, width: right - left, height: bottom - top }
}

/// How far `part`'s middle lies off the middle of the sheet, which stands `top` down the window and is `tall`.
fn off_middle(part: &Placed, top: f32, tall: f32) -> f32 {
    part.y + part.height / 2.0 - (top + tall / 2.0)
}

/// Every gift wrapped and opened, and the message's two screens, at the smallest window, a middle one and a whole
/// screen: one column of words for every screen, on the window's middle line beside the film, never under the keys.
/// `ONI_WELCOME_SHEET` names where each screen is kept as a picture.
pub(crate) fn the_welcome_stands_on_the_windows_middle_line_at_every_size() {
    let rig = common::rig();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    let window = MainWindow::new().unwrap();
    oni_app::keycaps::serve(&window);
    let help = window.global::<Help>();
    help.set_release("2.0".into());
    help.set_made_with("#madewithoni".into());
    help.set_welcome_shown(true);
    help.set_stage_live(false);
    help.set_film_count(oni_settings::VirtualStock::sheets().len() as i32);
    // The sheet's own size, below the window's menu where the window draws one: its middle is the one that counts.
    let size = std::rc::Rc::new(std::cell::Cell::new((0.0f32, 0.0f32)));
    let ear = size.clone();
    help.on_welcome_sized(move |w, h, _, _| ear.set((w, h)));
    window.show().unwrap();
    let sheet = std::env::var_os("ONI_WELCOME_SHEET").map(std::path::PathBuf::from);
    let keep = |name: &str| {
        if let Some(dir) = &sheet {
            let shot = rig.shot(name);
            std::fs::create_dir_all(dir).unwrap();
            std::fs::copy(&shot, dir.join(format!("{name}.png"))).unwrap();
        }
    };
    let language = std::env::var("ONI_WELCOME_SHEET_LANGUAGE").unwrap_or_else(|_| "en".into());
    oni_app::language::speak(Language::ALL.into_iter().find(|l| l.key() == language));
    for (w, h) in [(1000u32, 620u32), (1100, 680), (1728, 1117)] {
        sized(&rig, (w, h));
        let mut columns = Vec::new();
        for page in 1..=4usize {
            for opened in [false, true] {
                shown(&rig, &help, page);
                help.set_gift_shown(page as i32 - 1);
                if opened {
                    help.set_reel_gift(page as i32 - 1);
                    help.set_reel(film_frame(page - 1));
                } else {
                    help.set_opened_time(-1.0);
                    help.set_reel_gift(-1);
                }
                rig.settle();
                keep(&format!("{language}-{w}x{h}-{page}-{}", if opened { "opened" } else { "wrapped" }));
                let key = if opened { commit(&window, page) } else { common::the(&window, Role::Button, tr("Open the Gift")) };
                let words = if opened { title(&window, page) } else { common::placed(&window, Role::Text, tr(TAGS[page - 1]))[0] };
                columns.push(words.x);
                // The whole block of words, eyebrow to last key, its middle on the sheet's; the film's too.
                let top = h as f32 - size.get().1;
                let held = block(&window, words.x, key.y - MARGIN / 2.0);
                let off = off_middle(&held, top, size.get().1);
                let (state, film) = (if opened { "opened" } else { "wrapped" }, common::placed(&window, Role::Image, tr(TITLES[page - 1])));
                println!("{language} {w}×{h} screen {page} {state}: block {held:?} off the middle by {off:.1}, sheet {:?}, film {film:?}", size.get());
                assert!(off.abs() <= 1.0, "{language} {w}×{h} screen {page}: the words' block on the middle, off by {off:.1}: {held:?}");
                if opened {
                    let film = common::the(&window, Role::Image, tr(TITLES[page - 1]));
                    let off = off_middle(&film, top, size.get().1);
                    assert!(off.abs() <= 1.0, "{language} {w}×{h} screen {page}: the film on the middle, off by {off:.1}: {film:?}");
                    // Twin: the same block hung by its title, the title's middle on the sheet's, is told off the middle.
                    let hung = Placed { y: held.y - off_middle(&words, top, size.get().1), ..held };
                    assert!(off_middle(&hung, top, size.get().1).abs() > 1.0, "{w}×{h} screen {page}: a block hung by its title is caught");
                }
                assert!(words.y >= key.height, "{w}×{h} screen {page}: the words stand clear of the window's top, {words:?}");
                assert!(words.x > w as f32 / 2.0 - 60.0, "{w}×{h} screen {page}: the words right of the film, {words:?}");
                if opened && page == 4 {
                    let open = common::the(&window, Role::Button, tr("Open My Wrapped"));
                    assert!(open.y + open.height < key.y - MARGIN / 2.0, "{w}×{h}: the Wrapped's key above the keys' band, {open:?} {key:?}");
                }
            }
        }
        // One column for every screen, wrapped or opened; twin: the message's words are not in it.
        assert!(columns.iter().all(|x| (x - columns[0]).abs() < 1.0), "{w}×{h}: one column of words, {columns:?}");
        for (turn, name) in [(0.0, "announced"), (1.0, "told")] {
            shown(&rig, &help, 5);
            help.set_message_turn(turn);
            rig.settle();
            keep(&format!("{language}-{w}x{h}-5-{name}"));
            let top = h as f32 - size.get().1;
            let key = common::the(&window, Role::Button, tr(if turn == 0.0 { "Continue" } else { "Enter ONI" }));
            let told = if turn == 0.0 {
                common::the(&window, Role::Text, tr("I have a message for you"))
            } else {
                let hook = common::the(&window, Role::Text, tr(TITLES[4]));
                let keys = common::the(&window, Role::Button, tr("Enter the Contest"));
                Placed { x: hook.x, y: hook.y, width: hook.width, height: keys.y + keys.height - hook.y }
            };
            let off = off_middle(&told, top, size.get().1);
            println!("{language} {w}×{h} message {name}: block {told:?} off the middle by {off:.1}");
            if turn == 1.0 {
                // Its three keys, each drawn whole inside the window, side by side or on two lines, none over another.
                let keys: Vec<Placed> = ["Follow ONI on Instagram", "Support ONI on Ko-fi", "Enter the Contest"].iter().map(|k| common::the(&window, Role::Button, tr(k))).collect();
                for (i, k) in keys.iter().enumerate() {
                    let whole = k.width > 40.0 && k.x >= 0.0 && k.x + k.width <= w as f32;
                    assert!(whole && k.y >= top && k.y + k.height < key.y - MARGIN / 2.0, "{language} {w}×{h}: key {i} whole in the window, {k:?}");
                    for other in &keys[i + 1..] {
                        let apart = k.x + k.width <= other.x || other.x + other.width <= k.x || k.y + k.height <= other.y || other.y + other.height <= k.y;
                        assert!(apart, "{language} {w}×{h}: keys apart, {k:?} {other:?}");
                    }
                }
            }
            assert!(off.abs() <= 1.0 && told.y + told.height < key.y - MARGIN / 2.0, "{language} {w}×{h} message {name}: on the middle, clear of the keys, off by {off:.1}: {told:?}");
        }
        let made = common::the(&window, Role::Text, "#madewithoni");
        assert!((made.x + made.width / 2.0 - w as f32 / 2.0).abs() < 2.0 && made.x > columns[0] - w as f32, "{w}×{h}: the tag in the middle, {made:?}");
    }
    oni_app::language::speak(Some(Language::English));
    window.hide().unwrap();
}
