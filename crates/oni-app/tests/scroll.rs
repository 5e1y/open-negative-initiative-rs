//! Dragging is kept for the sliders and the image: every other surface scrolls by the wheel alone.

use oni_app::Catalogue;
use slint::platform::WindowEvent;
use slint::{ComponentHandle, PhysicalSize};
use std::path::PathBuf;

mod common;

#[test]
fn a_surface_scrolls_by_the_wheel_and_never_by_a_drag() {
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
