//! The design system's catalogue on the headless window: it opens, and its pages are shot to `ONI_SHOTS`.

use oni_app::Catalogue;
use slint::platform::WindowEvent;
use slint::{ComponentHandle, PhysicalSize};
use std::path::PathBuf;

use crate::common;

pub(crate) fn the_catalogue_shows_every_component() {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let catalogue = Catalogue::new().unwrap();
    oni_app::language::speak(None);
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
    // A help mark's note is a card: the clip's 4:3 place above, the note under it.
    catalogue.global::<oni_app::Tips>().invoke_note(true, "Contrast bends the print into an S about a middle grey, its ends held.".into(), 400.0, 300.0);
    rig.settle();
    assert!(catalogue.global::<oni_app::Tip>().get_card(), "the note is a card");
    println!("shot {}", rig.shot("help-card").display());
    catalogue.global::<oni_app::Tips>().invoke_hover(true, "Zoom in (⌘+)".into(), 400.0, 300.0);
    assert!(!catalogue.global::<oni_app::Tip>().get_card(), "twin: a tooltip stays a line");
}
