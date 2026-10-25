//! Where a sheet's pieces sit, read by the pointer as a hand finds them: the welcome as tall as its
//! content in a tall window and scrolled to reach GET STARTED in a short one, a notice's key on the
//! right of its message, and no sheet closed by a click beside it.

use oni_app::{Advice, Help, MainWindow};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, PhysicalSize};
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::common;

fn click(rig: &common::Rig, x: f32, y: f32) {
    let position = LogicalPosition::new(x, y);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Left });
    rig.pump();
}

/// The first point of the rectangle, row by row from the top, whose click fires `heard`.
fn find(rig: &common::Rig, heard: &Cell<bool>, xs: (f32, f32), ys: (f32, f32)) -> Option<(f32, f32)> {
    heard.set(false);
    let mut y = ys.0;
    while y < ys.1 {
        let mut x = xs.0;
        while x < xs.1 {
            click(rig, x, y);
            if heard.get() {
                return Some((x, y));
            }
            x += 6.0;
        }
        y += 4.0;
    }
    None
}

fn wheel(rig: &common::Rig, x: f32, y: f32, by: f32) {
    let position = LogicalPosition::new(x, y);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position });
    rig.window.dispatch_event(WindowEvent::PointerScrolled { position, delta_x: 0.0, delta_y: by });
    rig.pump();
}

pub(crate) fn the_welcome_fits_or_scrolls_and_a_notices_key_sits_right() {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    let window = MainWindow::new().unwrap();
    oni_app::keycaps::serve(&window);
    let help = window.global::<Help>();
    help.set_release("1.2".into());
    help.set_welcome_line("V1.2.1".into());
    help.set_welcome_credit("©fllauh".into());
    let closed = Rc::new(Cell::new(false));
    let heard = closed.clone();
    help.on_welcome_closed(move || heard.set(true));
    help.set_welcome_shown(true);

    // Tall: GET STARTED in the window as it opens, the card's foot a margin above the window's.
    let (w, h) = (1400.0, 1300.0);
    rig.window.set_size(PhysicalSize::new(w as u32, h as u32));
    window.show().unwrap();
    rig.settle();
    println!("shot {}", rig.shot("welcome-tall").display());
    // The key's band: the card's right part, clear of Website and Feedback on the left.
    let keys = |w: f32| (w / 2.0 + 120.0, w / 2.0 + 280.0 - 24.0);
    let tall = find(&rig, &closed, keys(w), (0.0, h)).expect("GET STARTED shows in a tall window");
    assert!(tall.1 < h - 24.0 - 24.0, "the card stops a margin above the window's foot");
    // Nothing to scroll: the wheel leaves the key where it was.
    wheel(&rig, w / 2.0, h / 2.0, -2000.0);
    let still = find(&rig, &closed, keys(w), (0.0, h)).expect("GET STARTED still there");
    assert_eq!(still.1, tall.1, "a sheet as tall as its content does not scroll");
    // A click beside the card reaches nothing: the sheet is modal.
    closed.set(false);
    click(&rig, 40.0, h / 2.0);
    assert!(!closed.get(), "a click beside the welcome leaves it open");

    // Short: the key below the window's foot until the wheel brings it up inside the card.
    let (w, h) = (1280.0, 700.0);
    rig.window.set_size(PhysicalSize::new(w as u32, h as u32));
    rig.settle();
    println!("shot {}", rig.shot("welcome-short").display());
    assert!(find(&rig, &closed, keys(w), (0.0, h)).is_none(), "twin: in a short window GET STARTED is out of reach unscrolled");
    for _ in 0..4 {
        wheel(&rig, w / 2.0, h / 2.0, -600.0);
    }
    rig.settle();
    println!("shot {}", rig.shot("welcome-short-scrolled").display());
    let short = find(&rig, &closed, keys(w), (0.0, h)).expect("scrolled, GET STARTED is reached");
    assert!(short.1 < h - 24.0, "the scrolled card still stops a margin above the window's foot");
    help.set_welcome_shown(false);

    // A notice: its title and message on the left, OK on the right of the card.
    let advice = window.global::<Advice>();
    let ok = Rc::new(Cell::new(false));
    let heard = ok.clone();
    advice.on_closed(move || heard.set(true));
    advice.set_title("No Preset Imported".into());
    advice.set_message("The file holds no preset ONI can read, or one it already has.".into());
    advice.set_shown(true);
    rig.settle();
    println!("shot {}", rig.shot("notice-ok").display());
    let card = (w / 2.0 - 180.0, w / 2.0 + 180.0);
    assert!(find(&rig, &ok, (card.0, w / 2.0), (0.0, h / 2.0)).is_none(), "twin: the card's left half holds no key");
    let key = find(&rig, &ok, card, (0.0, h / 2.0)).expect("OK answers a click");
    assert!(key.0 > w / 2.0 + 60.0, "OK sits at the card's right, not under its middle");
    assert!(key.1 < h / 3.0, "the notice hangs from the window's top");
}
