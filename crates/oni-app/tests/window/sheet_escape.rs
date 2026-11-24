//! Escape answers the sheet over the window after a press beside its card, whatever key it holds:
//! a default key, none, or a working stage with no commit.

use oni_app::{Confirm, Converting, DngStage, Unsaved, Upgraded};
use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;

use crate::common;

pub(crate) fn escape_answers_every_sheet_after_a_press_beside_it() {
    let root = common::Root::new("sheet-escape");
    let Some(app) = common::launch(root).show() else { return };
    let common::App { rig, shell, .. } = &app;
    let w = &shell.window;
    rig.settle();
    // What each sheet answered, in order.
    let heard = Rc::new(RefCell::new(Vec::<String>::new()));
    let (u, c, n, k) = (w.global::<Unsaved>(), w.global::<Converting>(), w.global::<Upgraded>(), w.global::<Confirm>());
    let (h, ww) = (heard.clone(), w.as_weak());
    u.on_answered(move |i| {
        h.borrow_mut().push(format!("unsaved {i}"));
        ww.unwrap().global::<Unsaved>().set_shown(false);
    });
    let (h, ww) = (heard.clone(), w.as_weak());
    c.on_cancelled(move || {
        h.borrow_mut().push("converting cancelled".into());
        ww.unwrap().global::<Converting>().set_shown(false);
    });
    let (h, ww) = (heard.clone(), w.as_weak());
    n.on_close(move || {
        h.borrow_mut().push("notice closed".into());
        ww.unwrap().global::<Upgraded>().set_shown(false);
    });
    let (h, ww) = (heard.clone(), w.as_weak());
    k.on_answered(move |go| {
        h.borrow_mut().push(format!("confirm {go}"));
        ww.unwrap().global::<Confirm>().set_shown(false);
    });
    // Shown, drawn, a press beside the card, then Escape.
    let escape_after_a_press = |show: &dyn Fn()| {
        show();
        rig.settle();
        rig.drawn();
        common::click(rig, w, (12.0, 400.0));
        rig.key("\u{1b}");
        rig.pump();
    };

    // The unsaved edits, whose Retry holds the keyboard: Escape keeps them (-1).
    u.set_title("The edits could not be saved".into());
    escape_after_a_press(&|| u.set_shown(true));
    assert!(!u.get_shown(), "Escape answers the unsaved-edits sheet");
    // The DNG conversion at work, its footer holding no commit: Escape cancels it.
    c.set_stage(DngStage::Converting);
    c.set_message("Converting the folder's frames.".into());
    escape_after_a_press(&|| c.set_shown(true));
    assert!(!c.get_shown(), "Escape cancels the conversion at work");
    // A question whose key removes holds no default key: Escape cancels it.
    k.set_title("Remove?".into());
    k.set_key("Remove".into());
    k.set_destructive(true);
    escape_after_a_press(&|| k.set_shown(true));
    assert!(!k.get_shown(), "Escape cancels a removing question");
    // A notice whose commit holds the keyboard: Escape closes it.
    escape_after_a_press(&|| n.set_shown(true));
    assert!(!n.get_shown(), "Escape closes the notice");
    assert_eq!(*heard.borrow(), ["unsaved -1", "converting cancelled", "confirm false", "notice closed"]);

    // Twin: the DNGs written, the sheet's choice is the hand's, so Escape leaves it shown.
    c.set_stage(DngStage::Choice);
    escape_after_a_press(&|| c.set_shown(true));
    assert!(c.get_shown(), "the measure sees a sheet Escape leaves");
    assert_eq!(heard.borrow().len(), 4, "the choice heard nothing");
}
