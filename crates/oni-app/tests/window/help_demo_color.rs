//! The Color tab's marks play their own demos: Lighting's card plays the lighting clip, frame
//! after frame, and Effects' card plays another.

use oni_app::PanelTab;
use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::{PanelState, Tip};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, LogicalPosition};
use std::time::Duration;

use crate::common;
use crate::help_demo::{clip_pixels, press_help, watch};
use crate::panel_bindings::opened;

const LIGHTING: &str = "The negative back to light along a straight line: the black point on pure black, the white on pure white.\n\nEvery slider but Contrast brightens to the right; Contrast bends the print into an S about a middle grey, its ends held. Black point and White point move the ends of the scan itself.";
const EFFECTS: &str = "Vignette darkens the corners in stops, without shifting their colour; below zero it lightens them.\n\nChrome lays a saturation curve over everything, greying the ends and keeping colour in the middle.";

pub(crate) fn the_color_tabs_marks_play_their_own_demos() {
    let Some((rig, shell, root)) = opened("help-demo-color") else { return };
    let w = &shell.window;
    let tip = w.global::<Tip>();
    w.global::<PanelState>().set_tab(PanelTab::Color);
    w.global::<PanelState>().invoke_tab_picked(PanelTab::Color);
    rig.settle();

    press_help(&rig, w, "Lighting");
    assert_eq!(tip.get_open(), LIGHTING, "the Lighting mark opens its card");
    assert_eq!(tip.get_demo(), "lighting", "the mark names its topic");
    assert!(tip.get_has_clip(), "the card holds a clip box");
    rig.until("the demo's first frame", Duration::from_secs(5), || clip_pixels(&tip).is_some());
    let first = clip_pixels(&tip).unwrap();
    // The hand reaches the first knob within a second: past that, the picture must have changed.
    watch(&rig, Duration::from_millis(1500));
    assert_ne!(Some(&first), clip_pixels(&tip).as_ref(), "the demo's frame advances while the card stands");
    rig.shot("help-demo-lighting");

    // Twin: another mark of the tab names its own topic and plays another clip.
    press_help(&rig, w, "Lighting");
    // Effects lies under the fold: the panel is scrolled by the wheel to bring it up.
    let over = LogicalPosition::new(w.get_panel_x() + 120.0, 500.0);
    for _ in 0..12 {
        if common::placed(w, Role::Button, "Help on Effects").first().is_some_and(|m| m.y < 700.0) {
            break;
        }
        rig.window.dispatch_event(WindowEvent::PointerScrolled { position: over, delta_x: 0.0, delta_y: -150.0 });
        rig.settle();
    }
    press_help(&rig, w, "Effects");
    assert_eq!(tip.get_open(), EFFECTS, "the Effects mark opens its card");
    assert_eq!(tip.get_demo(), "effects");
    rig.until("the other demo's first frame", Duration::from_secs(5), || clip_pixels(&tip).is_some());
    assert_ne!(Some(first), clip_pixels(&tip), "the Effects card shows its own clip, its knobs elsewhere");
    let _ = std::fs::remove_dir_all(&root);
}
