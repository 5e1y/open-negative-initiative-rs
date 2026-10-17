//! The design system's catalogue on the headless window: it opens, and its pages are shot to `ONI_SHOTS`.

use oni_app::Catalogue;
use slint::platform::WindowEvent;
use slint::{ComponentHandle, PhysicalSize};
use std::path::PathBuf;

mod common;

#[test]
fn the_catalogue_shows_every_component() {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let catalogue = Catalogue::new().unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 2.0 });
    rig.window.set_size(PhysicalSize::new(2 * 1440, 2 * 900));
    catalogue.show().unwrap();
    rig.settle();
    println!("shot {}", rig.shot("catalogue").display());
    // The page scrolls past the window: the second shot starts where the first ends.
    catalogue.set_scroll(-860.0);
    rig.settle();
    assert!(catalogue.get_scroll() < 0.0, "the catalogue is taller than the window");
    println!("shot {}", rig.shot("catalogue-2").display());
}
