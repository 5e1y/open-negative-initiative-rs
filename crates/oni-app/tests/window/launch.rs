//! What a launch shows before the editor: the splash, then the recalibration notice hung in the window;
//! both shot to `ONI_SHOTS` on the headless window.

use oni_app::{MainWindow, Notice, SplashWindow};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, PhysicalSize};
use std::path::PathBuf;

use crate::common;

pub(crate) fn the_splash_names_the_version_and_the_notice_covers_the_window() {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 2.0 });
    let splash = SplashWindow::new().unwrap();
    let picture = oni_app::splash::picture(env!("CARGO_PKG_VERSION"));
    splash.set_picture(picture as i32);
    splash.set_line("V0.1.0 Loading".into());
    splash.set_credit("©fllauh".into());
    rig.window.set_size(PhysicalSize::new(2 * 720, 2 * 480));
    splash.show().unwrap();
    rig.settle();
    println!("shot {}", rig.shot("splash").display());
    splash.hide().unwrap();
    let window = MainWindow::new().unwrap();
    rig.window.set_size(PhysicalSize::new(2 * 1440, 2 * 900));
    window.global::<Notice>().set_shown(true);
    window.show().unwrap();
    rig.settle();
    println!("shot {}", rig.shot("notice").display());
    let closed = std::rc::Rc::new(std::cell::Cell::new(false));
    let seen = closed.clone();
    window.global::<Notice>().on_close(move || seen.set(true));
    let click = |x: f32, y: f32| {
        let position = slint::LogicalPosition::new(x, y);
        rig.window.dispatch_event(WindowEvent::PointerPressed { position, button: slint::platform::PointerEventButton::Left });
        rig.window.dispatch_event(WindowEvent::PointerReleased { position, button: slint::platform::PointerEventButton::Left });
        rig.pump();
    };
    // Modal as Swift's sheet: a click beside the card leaves it open.
    click(40.0, 450.0);
    assert!(!closed.get(), "a click beside the notice leaves it open");
    // GOT IT sits at the card's right, under its bullets: the sweep finds it right of the middle.
    let mut found = None;
    'sweep: for y in (24..400).step_by(4) {
        for x in (720 - 210..720 + 210).step_by(6) {
            click(x as f32, y as f32);
            if closed.get() {
                found = Some(x);
                break 'sweep;
            }
        }
    }
    let x = found.expect("GOT IT closes the notice");
    assert!(x > 720 + 60, "twin: GOT IT is not under the card's middle but at its right, x {x}");
}
