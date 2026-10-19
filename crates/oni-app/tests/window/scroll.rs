//! Dragging is kept for the sliders and the image: every other surface scrolls by the wheel alone.

use oni_app::Catalogue;
use slint::platform::WindowEvent;
use slint::{ComponentHandle, PhysicalSize};
use std::path::PathBuf;

use crate::common;

pub(crate) fn a_surface_scrolls_by_the_wheel_and_never_by_a_drag() {
    use slint::platform::PointerEventButton;
    use slint::LogicalPosition;
    let rig = common::rig(PathBuf::from(env!("CARGO_TARGET_TMPDIR")));
    let catalogue = Catalogue::new().unwrap();
    rig.window.set_size(PhysicalSize::new(1440, 900));
    catalogue.show().unwrap();
    rig.settle();
    // A press on the colour column's swatches, away from every control, then a long upward drag.
    let start = LogicalPosition::new(200.0, 600.0);
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: start, button: PointerEventButton::Left });
    for k in 1..=20 {
        rig.pump();
        std::thread::sleep(std::time::Duration::from_millis(16));
        let at = LogicalPosition::new(200.0, 600.0 - 25.0 * k as f32);
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: at });
    }
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: LogicalPosition::new(200.0, 100.0), button: PointerEventButton::Left });
    rig.settle();
    assert_eq!(catalogue.get_scroll(), 0.0, "a drag moved the page");

    // Twin: the wheel at the same place does scroll, so the drag reached a scrollable page.
    rig.window.dispatch_event(WindowEvent::PointerScrolled { position: start, delta_x: 0.0, delta_y: -300.0 });
    rig.settle();
    assert!(catalogue.get_scroll() < 0.0, "the wheel did not scroll the page");
}

/// Pumps for `span`, as a hand resting.
fn wait(rig: &common::Rig, span: std::time::Duration) {
    let start = std::time::Instant::now();
    while start.elapsed() < span {
        rig.pump();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// AppKit's overlay knob: on a surface that overflows it lights while it scrolls, a moment after, and
/// while the pointer is on the edge; a surface that fits has none, and a vertical one never slides.
pub(crate) fn the_knob_shows_only_where_the_content_overflows_and_only_while_it_moves() {
    use slint::LogicalPosition;
    use std::time::Duration;
    let rig = common::rig(PathBuf::from(env!("CARGO_TARGET_TMPDIR")));
    let catalogue = Catalogue::new().unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(PhysicalSize::new(1440, 900));
    catalogue.show().unwrap();
    rig.settle();
    assert!(catalogue.get_page_overflows(), "the catalogue is taller than the window");
    assert!(!catalogue.get_page_lit(), "the knob shows at rest");
    assert!(!catalogue.get_short_overflows(), "a scroller whose content fits counts as overflowing");
    let short = LogicalPosition::new(catalogue.get_short_x(), catalogue.get_short_y());
    assert!(short.y > 0.0 && short.y < 900.0, "the short scroller is off the window");

    // The wheel over the short scroller: it cannot move, so the page under it does.
    rig.window.dispatch_event(WindowEvent::PointerScrolled { position: short, delta_x: 0.0, delta_y: -120.0 });
    wait(&rig, Duration::from_millis(100));
    assert!(catalogue.get_scroll() < 0.0, "the wheel did not reach the page");
    assert!(catalogue.get_page_lit(), "the page's knob stays dark while it scrolls");
    // Twin: the same wheel event lit nothing on the surface that fits.
    assert!(!catalogue.get_short_lit(), "a scroller whose content fits shows a knob");

    // A moment after the last scroll, with the pointer away from the edge, the knob goes out.
    wait(&rig, Duration::from_millis(1000));
    assert!(!catalogue.get_page_lit(), "the knob outlasts its linger");

    // On the edge strip it lights; back over the content it goes out again.
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(1436.0, 450.0) });
    rig.pump();
    assert!(catalogue.get_page_lit(), "the pointer on the edge does not light the knob");
    wait(&rig, Duration::from_millis(300));
    println!("shot {}", rig.shot("scroll-knob").display());
    // Twin: on the edge of the scroller whose content fits, nothing lights.
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(catalogue.get_short_right() - 4.0, catalogue.get_short_y()) });
    rig.pump();
    assert!(!catalogue.get_short_lit(), "the edge of a scroller whose content fits lights a knob");
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(700.0, 450.0) });
    rig.pump();
    assert!(!catalogue.get_page_lit(), "the pointer over the content lights the knob");

    // A diagonal trackpad swipe moves the vertical page down only.
    let before = catalogue.get_scroll();
    rig.window.dispatch_event(WindowEvent::PointerScrolled { position: LogicalPosition::new(700.0, 450.0), delta_x: -200.0, delta_y: -60.0 });
    rig.pump();
    assert_eq!(catalogue.get_page_x(), 0.0, "a vertical page slid sideways");
    // Twin: the swipe did reach the page.
    assert!(catalogue.get_scroll() < before, "the swipe did not reach the page");
}
