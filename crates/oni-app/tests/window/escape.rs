//! Escape on the headless window steps out of one thing at a time: a field's typing, a control's hold,
//! Compare, a tool, the editor; each tool ends by its DONE or Return, and the grid's Return opens a photo.
//! A press on Compare's key takes a slider's hold away, so one Escape leaves Compare. A paste on the
//! roll says how many photos it rewrote, its Undo beside.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::fields::Field;
use oni_app::{App, Canvas, PanelState, Told};
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, SharedString};

use crate::common;

fn click(rig: &common::Rig, (x, y): (f32, f32)) {
    let position = LogicalPosition::new(x, y);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Left });
    rig.pump();
}

/// A key as winit hands it over: the window's key hook first, then Slint.
fn key(rig: &common::Rig, w: &oni_app::MainWindow, text: &str) {
    oni_app::outside::keyed(w.window(), || w.invoke_refocus());
    rig.key(text);
}

pub(crate) fn one_escape_leaves_compare_its_key_turned_on_after_a_slider_held_the_keyboard() {
    let Some((rig, shell, root)) = crate::panel_bindings::opened("escape-compare") else { return };
    let w = &shell.window;
    let canvas = w.global::<Canvas>();
    let escape: SharedString = Key::Escape.into();
    // Its word, or its glyph and help on a bar short of room.
    let compare = || {
        let worded = common::placed(w, Role::Button, "Compare");
        if worded.is_empty() { common::the(w, Role::Button, "Compare Before/After") } else { worded[0] }
    };
    // A press the window's hook never sees, as an evaluation tool's, and the hand's, through the hook:
    // the slider holds the keyboard either way, and one Escape still leaves Compare.
    for hooked in [false, true] {
        let rail = common::the(w, Role::Slider, "Exposure");
        common::click(&rig, w, (rail.x + rail.width / 2.0, rail.y + rail.height - 4.0));
        // The pressed slider holds the keyboard, and shows it holds it.
        assert!(w.get_keys_held(), "the pressed slider holds the keyboard");
        if hooked { common::click(&rig, w, compare().middle()) } else { click(&rig, compare().middle()) }
        assert!(canvas.get_split() >= 0.0, "Compare's key turns it on");
        key(&rig, w, &escape);
        assert!(canvas.get_split() < 0.0, "one Escape leaves Compare, the hook seeing the press or not");
        assert!(!w.global::<App>().get_gallery(), "and only Compare");
    }
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}

pub(crate) fn escape_steps_out_one_thing_at_a_time_and_each_tool_ends_by_its_done() {
    let Some((rig, shell, root)) = crate::panel_bindings::opened("escape") else { return };
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (app, panel, canvas) = (w.global::<App>(), w.global::<PanelState>(), w.global::<Canvas>());
    let escape: SharedString = Key::Escape.into();
    assert!(!app.get_gallery() && panel.get_tab() == 2);

    // With nothing open the photos' Escape leaves the editor; Return on the grid opens the photo again.
    rig.key(&escape);
    assert!(app.get_gallery(), "Escape leaves the editor for the gallery");
    // A field's typing is the innermost thing: Escape ends it and goes no further.
    click(&rig, common::the(w, Role::TextInput, "Camera Body").middle());
    assert!(!w.get_keys_held(), "the field holds the keyboard");
    rig.key(&escape);
    assert!(w.get_keys_focused() && app.get_gallery(), "Escape gives the photos the keyboard back, and only that");
    rig.key("\r");
    assert!(!app.get_gallery(), "Return opens the photo the grid holds");

    // Compare, then the crop: one Escape each, the innermost first.
    rig.chord(Key::Control, "k");
    ctl.act(oni_app::Act::Compare);
    rig.pump();
    rig.key(&escape);
    assert!(canvas.get_split() < 0.0 && panel.get_tab() == 1, "Compare closes first");
    // The crop names itself where the tab bar stands, beside the DONE that leaves it.
    assert!(common::placed(w, Role::Tab, "Crop").is_empty(), "the crop is a tool, no tab");
    click(&rig, common::the(w, Role::Button, "DONE").middle());
    assert_eq!(panel.get_tab(), 2, "DONE leaves the crop for the last panel");
    assert!(common::placed(w, Role::Button, "DONE").is_empty(), "twin: no DONE beside a pane");

    // A slider pressed in the crop leaves the keyboard with the photos: one Escape leaves the crop.
    rig.chord(Key::Control, "k");
    let rail = common::the(w, Role::Slider, "Angle");
    click(&rig, (rail.x + rail.width / 2.0, rail.y + rail.height - 4.0));
    assert!(w.get_keys_focused(), "the pressed slider takes no keyboard");
    rig.key(&escape);
    assert_eq!(panel.get_tab(), 2, "Escape leaves the crop");

    // Return is De-dust's DONE as it is the crop's.
    ctl.act(oni_app::Act::Correction);
    rig.pump();
    assert_eq!(panel.get_tab(), 4);
    rig.key("\r");
    assert_eq!(panel.get_tab(), 2, "Return leaves De-dust");

    // Paste to Roll says how many photos it rewrote, its key taking the paste back.
    let told = w.global::<Told>();
    let exposure = || oni_app::store::get(ctl.state.borrow().store(), Field::Exposure);
    ctl.act(oni_app::Act::Copy);
    w.global::<oni_app::Presets>().invoke_save_confirmed();
    ctl.act(oni_app::Act::PasteToRoll);
    rig.pump();
    assert!(!told.get_shown(), "twin: a paste that rewrites nothing says nothing");
    let copied = exposure();
    w.global::<oni_app::Store>().invoke_step(Field::Exposure.index(), 5);
    rig.pump();
    let moved = exposure();
    assert!((moved - copied).abs() > 1e-4);
    ctl.act(oni_app::Act::PasteToRoll);
    rig.pump();
    assert!((exposure() - copied).abs() < 1e-5, "the copied exposure is back");
    let said = (told.get_shown(), told.get_title().to_string(), told.get_message().to_string(), told.get_action().to_string());
    assert_eq!(said, (true, "Paste to Roll".into(), "Settings pasted on one photo.".into(), "Undo".into()));
    told.invoke_acted();
    rig.pump();
    assert!((exposure() - moved).abs() < 1e-5, "Undo takes the paste back");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
