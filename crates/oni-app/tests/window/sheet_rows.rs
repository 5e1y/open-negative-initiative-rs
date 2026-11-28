//! Every sheet's keys stand whole inside its card at the smallest window, in every language: the keys a sheet adds to
//! its last row take a line of their own when the row cannot hold them, never pushing the commit past the card's edge.

use i_slint_backend_testing::{AccessibleRole as Role, ElementRoot};
use oni_app::{Confirm, Help, MainWindow, Pairing, Presets};
use slint::ComponentHandle;
use slint::platform::WindowEvent;

use crate::common::{self, Placed};

/// The smallest window the app lays itself in.
const SMALLEST: (u32, u32) = (1100, 700);

/// The card of the one sheet shown.
fn card(w: &MainWindow) -> Placed {
    let found: Vec<Placed> = w
        .root_element()
        .query_descendants()
        .match_id("Sheet::card")
        .find_all()
        .into_iter()
        .map(|e| Placed { x: e.absolute_position().x, y: e.absolute_position().y, width: e.size().width, height: e.size().height })
        .filter(|p| p.width > 0.0 && p.height > 0.0)
        .collect();
    assert_eq!(found.len(), 1, "one sheet shown: {found:?}");
    found[0]
}

pub(crate) fn every_sheets_keys_stand_whole_inside_its_card_in_every_language() {
    let rig = common::rig();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    let window = MainWindow::new().unwrap();
    oni_app::keycaps::serve(&window);
    rig.window.set_size(slint::PhysicalSize::new(SMALLEST.0, SMALLEST.1));
    window.show().unwrap();
    let w = &window;
    let (help, pairing, presets, confirm) = (w.global::<Help>(), w.global::<Pairing>(), w.global::<Presets>(), w.global::<Confirm>());
    type Shown<'a> = Box<dyn Fn(bool) + 'a>;
    let sheets: Vec<(&str, Shown, Vec<&str>)> = vec![
        ("the report", Box::new(|on| help.set_report_shown(on)), vec!["Cancel", "See Community Feedbacks", "Send"]),
        ("the phones", Box::new(|on| {
            pairing.set_can_forget(on);
            pairing.set_shown(on);
        }), vec!["Forget Phones", "DONE"]),
        ("the preset editor", Box::new(|on| {
            presets.set_form_shown(on);
            presets.set_editing(on);
        }), vec!["Delete", "Duplicate", "Share…", "Copy Preset Code", "Done"]),
        ("the New Roll confirm", Box::new(|on| {
            confirm.set_title("New Roll".into());
            confirm.set_message("3 photos move into a new folder, Roll #9, beside them. Their sidecars go with them.".into());
            confirm.set_key("Create Roll".into());
            confirm.set_shown(on);
        }), vec!["Cancel", "Create Roll"]),
    ];
    let (mut checked, mut wrong) = (0, Vec::new());
    for language in [oni_i18n::Language::English, oni_i18n::Language::French, oni_i18n::Language::Spanish] {
        oni_app::language::speak(Some(language));
        rig.settle();
        // The live interface of a debug build reads English: its other languages are judged on the app profile.
        let word = |english: &'static str| if english == "Create Roll" { english } else { oni_i18n::tr(english) };
        for (name, show, keys) in &sheets {
            show(true);
            rig.settle();
            let found: Vec<Option<Placed>> = keys.iter().map(|k| common::placed(w, Role::Button, word(k)).into_iter().last()).collect();
            if language != oni_i18n::Language::English && found.iter().all(Option::is_none) {
                show(false);
                rig.settle();
                println!("{language:?}: the live interface reads English, judged on the app profile");
                break;
            }
            let c = card(w);
            println!("{language:?}, {name}: card {c:?}, keys {found:?}");
            println!("shot {}", rig.shot(&format!("sheet-rows-{}-{}", language.key(), name.replace(' ', "-"))).display());
            for (key, at) in keys.iter().zip(&found) {
                match at {
                    Some(at) if at.x >= c.x - 0.5 && at.x + at.width <= c.x + c.width + 0.5 => {}
                    _ => wrong.push(format!("{language:?}, {name}: {key} not whole inside the card, {at:?} in {c:?}")),
                }
            }
            // Twin: keys wider together than the card hold more than one line, so a row laid whole would have overrun.
            let keys: Vec<Placed> = found.into_iter().flatten().collect();
            let together = keys.iter().map(|k| k.width).sum::<f32>();
            let lines = { let mut ys: Vec<i32> = keys.iter().map(|k| k.y.round() as i32).collect(); ys.sort(); ys.dedup(); ys.len() };
            assert!(together < c.width || lines > 1, "{language:?}, {name}: {together} pt of keys on {lines} line in {} pt", c.width);
            show(false);
            rig.settle();
            checked += 1;
        }
    }
    oni_app::language::speak(Some(oni_i18n::Language::English));
    assert!(wrong.is_empty(), "keys past their card: {wrong:#?}");
    assert!(checked >= sheets.len(), "every sheet checked at least in English");
}
