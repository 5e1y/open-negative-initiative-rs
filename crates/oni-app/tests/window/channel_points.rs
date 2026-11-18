//! More Color Settings through the window's pointer: folded under the Colour block until its title is
//! clicked, each of its six sliders moves its own channel's point and the render with it, its Reset
//! clears them, and black and white shows none of it.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::fields::Field;
use oni_app::{PanelState, PanelTab, Store};
use oni_core::Knob;
use slint::ComponentHandle;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::LogicalPosition;
use std::time::Duration;

use crate::common;
use crate::panel_bindings::opened;

const TITLE: &str = "More Color Settings";
const SLIDERS: [(Field, &str); 6] = [
    (Field::RedWhitePoint, "Red white point"),
    (Field::GreenWhitePoint, "Green white point"),
    (Field::BlueWhitePoint, "Blue white point"),
    (Field::RedBlackPoint, "Red black point"),
    (Field::GreenBlackPoint, "Green black point"),
    (Field::BlueBlackPoint, "Blue black point"),
];

fn click(rig: &common::Rig, (x, y): (f32, f32)) {
    let at = LogicalPosition::new(x, y);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: at });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: at, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: at, button: PointerEventButton::Left });
    rig.pump();
}

/// The histogram's mean place on its bins, per channel, of the picture last rendered.
fn means(ctl: &oni_app::app::Ctl) -> [f32; 3] {
    let state = ctl.state.borrow();
    let h = state.histogram().expect("a histogram of the render");
    std::array::from_fn(|c| {
        let counts = h.channel(c);
        let total: f32 = counts.iter().sum();
        counts.iter().enumerate().map(|(i, n)| i as f32 * n).sum::<f32>() / total.max(1e-6)
    })
}

pub(crate) fn more_color_settings_move_each_its_own_channel() {
    let Some((rig, shell, root)) = opened("channel-points") else { return };
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (panel, g) = (w.global::<PanelState>(), w.global::<Store>());
    rig.window.set_size(slint::PhysicalSize::new(1280, 1600));
    panel.set_tab(PanelTab::Color);
    panel.invoke_tab_picked(PanelTab::Color);
    rig.settle();
    let settings = || ctl.state.borrow().store().settings().clone();
    let title = || common::placed(w, Role::Text, TITLE);
    let lighting = || common::the(w, Role::Text, "Lighting").y;

    // Folded at first, right under Saturation: Lighting follows its title a row below.
    assert_eq!(title().len(), 1, "one title on the Colour block of a negative");
    let folded = lighting() - title()[0].y;
    assert!(folded < 100.0, "the six sliders lie folded: Lighting {folded} pt below the title");
    rig.shot("channel-points-folded");
    click(&rig, title()[0].middle());
    rig.settle();
    let open = lighting() - title()[0].y;
    assert!(open > folded + 6.0 * 30.0, "the title's click lays the six sliders: {folded} then {open} pt");
    println!("shot {}", rig.shot("channel-points-open").display());

    // Each slider by its rail: its own channel's point alone moves, through the field the markup wires.
    for (field, label) in SLIDERS {
        let rail = common::the(w, Role::Slider, label);
        let knob = field.knob();
        std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
        click(&rig, (rail.x + rail.width * 0.92, rail.y + rail.height - 13.0));
        rig.settle();
        let s = settings();
        assert!(knob.get(&s).abs() > 0.05, "{label} moved its point: {}", knob.get(&s));
        let others: Vec<f32> = SLIDERS.iter().filter(|(f, _)| *f != field).map(|(f, _)| f.knob().get(&s)).collect();
        assert!(others.iter().all(|v| *v == 0.0), "{label} moved another point: {others:?}");
        assert!(g.get_channel_points_modified(), "{label}: the block's Reset shows");
        // Back to rest by the block's own key, beside its title.
        let reset: Vec<_> = common::placed(w, Role::Button, "Reset").into_iter().filter(|k| title()[0].level_with(k)).collect();
        assert_eq!(reset.len(), 1, "one Reset on the row of {TITLE}");
        click(&rig, reset[0].middle());
        rig.settle();
        assert_eq!(knob.get(&settings()), 0.0, "{label}: the Reset took it back");
        assert!(!g.get_channel_points_modified());
    }

    // The render follows: red's white point lifts red's histogram, green's stays near where it was.
    rig.until("a histogram", Duration::from_secs(60), || ctl.state.borrow().histogram().is_some());
    let before = means(&ctl);
    let renders = ctl.state.borrow().renders();
    let red = common::the(w, Role::Slider, "Red white point");
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    click(&rig, (red.x + red.width * 0.95, red.y + red.height - 13.0));
    rig.until("the render of red's point", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    rig.settle();
    let after = means(&ctl);
    let moved = [0, 1, 2].map(|c| after[c] - before[c]);
    println!("red's white point moves the histogram's means by {moved:?} bins");
    assert!(Knob::ChannelWhite(oni_core::knob::Channel::Red).get(&settings()) > 0.1);
    assert!(moved[0] > 1.0 && moved[0] > 2.0 * moved[1].abs(), "red's point lifted red: {moved:?}");
    // Twin: the histogram's red read as green misses the move.
    assert!(moved[1].abs() < moved[0] / 2.0);

    // Black and white shows no channel, so no More Color Settings; a positive does.
    panel.invoke_film_type_picked(2);
    rig.settle();
    assert!(panel.get_monochrome() && title().is_empty(), "black and white lays More Color Settings");
    assert!(common::placed(w, Role::Slider, "Red white point").is_empty());
    panel.invoke_film_type_picked(1);
    rig.settle();
    // Twin: the same panel on a positive lays it.
    assert_eq!(title().len(), 1, "a positive lays More Color Settings");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
