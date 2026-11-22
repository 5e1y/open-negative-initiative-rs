//! More Color Settings through the window's pointer: folded under the Colour block until its title is
//! clicked, one channel a tab, each of a tab's four sliders moves its own channel's value and the render
//! with it, the tab's mark follows, its Reset clears them, and black and white shows none of it.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::fields::Field;
use oni_app::{PanelState, PanelTab, Store};
use oni_core::Knob;
use slint::{ComponentHandle, Model};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::LogicalPosition;
use std::time::Duration;

use crate::common;
use crate::panel_bindings::opened;

const TITLE: &str = "More Color Settings";
/// Each tab's name and its sliders, in the order the owner named them.
const TABS: [(&str, [(Field, &str); 4]); 3] = [
    ("R", [(Field::RedWhitePoint, "Red white point"), (Field::RedBlackPoint, "Red black point"), (Field::RedMid, "Red mid"), (Field::RedContrast, "Red contrast")]),
    ("G", [(Field::GreenWhitePoint, "Green white point"), (Field::GreenBlackPoint, "Green black point"), (Field::GreenMid, "Green mid"), (Field::GreenContrast, "Green contrast")]),
    ("B", [(Field::BlueWhitePoint, "Blue white point"), (Field::BlueBlackPoint, "Blue black point"), (Field::BlueMid, "Blue mid"), (Field::BlueContrast, "Blue contrast")]),
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
    assert!(folded < 100.0, "the sliders lie folded: Lighting {folded} pt below the title");
    rig.shot("channel-points-folded");
    click(&rig, title()[0].middle());
    rig.settle();
    let open = lighting() - title()[0].y;
    assert!(open > folded + 4.0 * 30.0, "the title's click lays the tabs and a tab's four sliders: {folded} then {open} pt");
    println!("shot {}", rig.shot("channel-points-open").display());
    // The block's own tabs, between its title and Lighting: the Curves' further down share their names.
    let tab = |name: &str| {
        let (top, bottom) = (title()[0].y, lighting());
        let found: Vec<_> = common::placed(w, Role::Tab, name).into_iter().filter(|t| t.y > top && t.y < bottom).collect();
        assert_eq!(found.len(), 1, "one {name} tab in {TITLE}: {found:?}");
        found[0]
    };
    let marks = || -> Vec<bool> { (0..3).map(|c| g.get_channel_marked().row_data(c).unwrap_or(false)).collect() };
    let every: Vec<Field> = TABS.iter().flat_map(|(_, sliders)| sliders.map(|(f, _)| f)).collect();

    // Each tab shows its own channel's four sliders alone; each slider by its rail moves its own value
    // alone, through the field the markup wires, marks its tab alone, and the block's Reset clears it.
    for (c, (name, sliders)) in TABS.iter().enumerate() {
        click(&rig, tab(name).middle());
        rig.settle();
        for (other, (_, theirs)) in TABS.iter().enumerate() {
            for (_, label) in theirs {
                assert_eq!(common::placed(w, Role::Slider, label).len(), usize::from(other == c), "{label} under the {name} tab");
            }
        }
        for (field, label) in sliders {
            let rail = common::rail(w, label);
            let knob = field.knob();
            std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
            click(&rig, (rail.x + rail.width * 0.92, rail.y + rail.height - 13.0));
            rig.settle();
            let s = settings();
            assert!(knob.get(&s).abs() > 0.05, "{label} moved its value: {}", knob.get(&s));
            let others: Vec<f32> = every.iter().filter(|f| *f != field).map(|f| f.knob().get(&s)).collect();
            assert!(others.iter().all(|v| *v == 0.0), "{label} moved another value: {others:?}");
            assert!(g.get_channel_points_modified(), "{label}: the block's Reset shows");
            assert_eq!(marks(), (0..3).map(|k| k == c).collect::<Vec<_>>(), "{label} marks its tab alone");
            if *field == Field::RedMid {
                println!("shot {}", rig.shot("channel-points-red-mid").display());
            }
            // Back to rest by the block's own key, beside its title.
            let reset: Vec<_> = common::placed(w, Role::Button, "Reset").into_iter().filter(|k| title()[0].level_with(k)).collect();
            assert_eq!(reset.len(), 1, "one Reset on the row of {TITLE}");
            click(&rig, reset[0].middle());
            rig.settle();
            assert_eq!(knob.get(&settings()), 0.0, "{label}: the Reset took it back");
            assert!(!g.get_channel_points_modified() && marks() == [false; 3], "{label}: no mark once reset");
        }
    }
    // Twin: green's white point is not laid while the red tab is picked, so a walk of one tab sees one channel.
    click(&rig, tab("R").middle());
    rig.settle();
    assert!(common::placed(w, Role::Slider, "Green white point").is_empty());

    // The render follows: red's white point lifts red's histogram, green's stays near where it was.
    rig.until("a histogram", Duration::from_secs(60), || ctl.state.borrow().histogram().is_some());
    let before = means(&ctl);
    let renders = ctl.state.borrow().renders();
    let red = common::rail(w, "Red white point");
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
    // Twin: the same panel on a positive lays it, and so does a digital photo's.
    assert_eq!(title().len(), 1, "a positive lays More Color Settings");
    panel.invoke_film_type_picked(3);
    rig.settle();
    assert_eq!(ctl.state.borrow().store().settings().mode, oni_settings::ConversionMode::Digital);
    assert_eq!(title().len(), 1, "a digital photo lays More Color Settings");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
