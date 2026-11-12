//! The preset deck by the pointer: its tab opens it and the pointer leaving closes it, the "+" card
//! keeps its hit while it lifts, a wheel keeps the deck up and scrolls it, two automatic cards each
//! preview their balance, and the preview leaves with the deck or under a sheet.

use oni_app::PanelTab;
use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::{Library, Menus, PanelState, Presets};
use oni_core::preset_parameters::{self as carries, WhiteBalance};
use oni_core::presets::{self, Preset, Tint};
use oni_core::{Archive, EditorStore};
use oni_settings::PipelineSettings;
use i_slint_backend_testing::AccessibleRole as Role;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

/// `DS.Control.cardLift`, `PresetCarousel.cardWidth` and `cardHeight`, `DS.Space.sm`.
const LIFT: f32 = 6.0;
const CARD_W: f32 = 96.0;
const CARD_H: f32 = 64.0;
const GAP: f32 = 8.0;
const LOOKS: usize = 16;

fn move_to(rig: &common::Rig, x: f32, y: f32) {
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(x, y) });
    rig.settle();
}

fn click(rig: &common::Rig, x: f32, y: f32) {
    let position = LogicalPosition::new(x, y);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Left });
    rig.pump();
}

fn wheel(rig: &common::Rig, x: f32, y: f32, dx: f32, dy: f32) {
    rig.window.dispatch_event(WindowEvent::PointerScrolled { position: LogicalPosition::new(x, y), delta_x: dx, delta_y: dy });
    rig.settle();
}

pub(crate) fn the_deck_opens_on_its_tab_and_answers_the_pointer() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the deck is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-preset-deck-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    // A cast the automatic balance measures away, so its preview differs from the preset laid alone.
    common::pixels_tiff(&folder.join("A.tif"), 240, 160, |x, y| {
        let (u, v) = (x as f32 / 240.0, y as f32 / 160.0);
        [0.3 + 0.4 * u, 0.25 + 0.4 * v, 0.2 + 0.15 * (u + v)]
    });
    let shelf = presets::directory(&support);
    // The first two carry the key alone, the rest a brighter Lighting.
    let bright = Field::Exposure.knob().set(&PipelineSettings::default(), 0.8);
    for i in 0..LOOKS {
        let parameters = if i < 2 { carries::with_white_balance(WhiteBalance::Automatic, &BTreeSet::new()) } else { carries::proposed(&bright) };
        let preset = Preset::new(&format!("Look {i:02}"), parameters, Tint::ALL[i % Tint::ALL.len()], bright.clone());
        presets::save(&preset, &shelf).unwrap();
    }
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let deck = w.global::<Presets>();
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    let lib = w.global::<Library>();
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    assert_eq!(deck.get_cards().row_count(), LOOKS);

    // Two automatic cards in a row, the first still measuring: the second previews the balance.
    let held = presets::load(&shelf);
    let settings = ctl.state.borrow().store().settings().clone();
    let laid = |i: usize| presets::previewed(Some(&held[i]), &settings);
    let exposure = |s: &PipelineSettings| Field::Exposure.knob().get(s);
    deck.invoke_hover(0);
    deck.invoke_hover(1);
    rig.until("the second card's balance", Duration::from_secs(30), || {
        ctl.state.borrow().shown_settings().is_some_and(|s| *s != laid(1) && *s != settings)
    });
    deck.invoke_hover(2);
    rig.until("twin: a card without the key previews bare", Duration::from_secs(30), || ctl.state.borrow().shown_settings() == Some(&laid(2)));
    assert!((exposure(&laid(2)) - exposure(&settings)).abs() > 0.1, "the third card's preview shows");
    deck.invoke_hover(-1);
    rig.settle();
    rig.shot("preset-deck-rest");
    let first = || (deck.get_first_x(), deck.get_first_y());
    let (x0, rest_y) = first();
    let x = x0 + CARD_W / 2.0;
    let plus = x0 - GAP - CARD_W / 2.0;
    let tab_at = common::the(w, Role::Button, "Presets");
    let tab = tab_at.middle();
    let mid = LIFT + CARD_H / 2.0;

    // The tab is a 24 pt target: its foot opens the deck, the canvas just above it does not.
    assert!(tab_at.height >= 24.0, "{tab_at:?}");
    click(&rig, tab.0, tab_at.y - 3.0);
    rig.settle();
    assert_eq!(first().1, rest_y, "twin: a click above the tab opens nothing");
    click(&rig, tab.0, tab_at.y + tab_at.height - 2.0);
    rig.settle();
    assert!(first().1 < rest_y - CARD_H, "the tab's foot opens the deck");
    click(&rig, tab.0, tab.1);
    rig.settle();
    assert_eq!(first().1, rest_y);

    // The tab alone answers while the deck is closed: neither the canvas's edge nor the tab's hover opens it.
    move_to(&rig, x, rest_y - 200.0);
    move_to(&rig, x, tab.1);
    move_to(&rig, tab.0, tab.1);
    assert_eq!(first().1, rest_y, "a hover on the tab leaves the deck closed");
    click(&rig, tab.0, tab.1);
    rig.settle();
    let up_y = first().1;
    assert!(up_y < rest_y - CARD_H, "twin: a click on the tab opens it: {up_y} against {rest_y}");
    assert_eq!(common::the(w, Role::Button, "Presets").middle(), tab, "the tab stays under the pointer");
    assert!(!deck.get_saving(), "the tab saves nothing");
    click(&rig, tab.0, tab.1);
    rig.settle();
    assert_eq!(first().1, rest_y, "a second click on the tab closes it");
    click(&rig, tab.0, tab.1);
    rig.settle();
    move_to(&rig, x, up_y + mid);
    assert_eq!(first().1, up_y, "the pointer onto the cards keeps it open");
    move_to(&rig, x, up_y - 200.0);
    assert_eq!(first().1, rest_y, "the pointer off the deck closes it");

    // The "+" card lifts under the pointer in its bottom band and still takes the click there.
    click(&rig, tab.0, tab.1);
    rig.settle();
    let bottom = up_y + LIFT + CARD_H - 3.0;
    move_to(&rig, plus, bottom);
    rig.shot("preset-deck-plus");
    click(&rig, plus, bottom + 0.5);
    rig.settle();
    assert!(deck.get_saving(), "the \"+\" card's bottom band opens the save sheet");
    deck.invoke_save_cancelled();
    // The sheet took the pointer and closed the deck: the tab opens it again.
    move_to(&rig, x, rest_y - 200.0);
    click(&rig, tab.0, tab.1);
    rig.settle();
    move_to(&rig, x0 - GAP / 2.0, up_y + mid);
    click(&rig, x0 - GAP / 2.0, up_y + mid);
    rig.settle();
    assert!(!deck.get_saving(), "twin: the gap beside the card opens nothing");

    // A wheel over a card keeps the deck up and the card hovered; a vertical one scrolls the roll.
    move_to(&rig, x, up_y + mid);
    assert_eq!(deck.get_hovered(), 0);
    wheel(&rig, x, up_y + mid, 0.0, 40.0);
    assert_eq!(first(), (x0, up_y), "twin: the roll's start holds against a wheel towards it");
    assert_eq!(deck.get_hovered(), 0, "the wheel keeps the card hovered");
    wheel(&rig, x, up_y + mid, 0.0, -40.0);
    assert_eq!(first().1, up_y, "the wheel keeps the deck up");
    assert!(first().0 < x0 - 20.0, "a vertical wheel scrolls the roll: {} against {x0}", first().0);
    wheel(&rig, x, up_y + mid, 0.0, 400.0);
    assert_eq!(first().0, x0);
    wheel(&rig, x, up_y + mid, -40.0, 0.0);
    assert!(first().0 < x0 - 20.0, "a sideways wheel still scrolls the roll: {} against {x0}", first().0);
    assert_eq!(first().1, up_y, "and keeps the deck up");
    wheel(&rig, x, up_y + mid, 400.0, 0.0);
    move_to(&rig, x, rest_y - 200.0);
    assert_eq!(first().1, rest_y);

    // The preview leaves with the deck, under Crop, and under the save sheet.
    deck.invoke_hover(2);
    w.global::<PanelState>().set_tab(PanelTab::Crop);
    w.global::<PanelState>().invoke_tab_picked(PanelTab::Crop);
    rig.pump();
    assert_eq!(deck.get_hovered(), -1, "Crop takes the deck and its preview");
    rig.until("the preview gone under Crop", Duration::from_secs(30), || ctl.state.borrow().shown_settings().is_some_and(|s| exposure(s) == exposure(&settings)));
    w.global::<PanelState>().set_tab(PanelTab::Color);
    w.global::<PanelState>().invoke_tab_picked(PanelTab::Color);
    deck.invoke_hover(2);
    rig.until("twin: the preview back", Duration::from_secs(30), || ctl.state.borrow().shown_settings() == Some(&laid(2)));
    let menus = w.global::<Menus>();
    assert!(menus.get_live().save_preset);
    ctl.act(oni_app::Act::SavePreset);
    rig.pump();
    assert_eq!(deck.get_hovered(), -1, "the save sheet takes the preview");
    rig.until("the preview gone under the sheet", Duration::from_secs(30), || ctl.state.borrow().shown_settings().is_some_and(|s| exposure(s) == exposure(&settings)));
    assert!(!menus.get_live().save_preset, "Save as Preset… is dark while a sheet is open");
    rig.settle();
    println!("shot {}", rig.shot("preset-save-sheet").display());

    // The proposed name stands selected, so typing replaces it; Save takes a name, never blanks.
    rig.key("Z");
    assert_eq!(deck.get_form_name().as_str(), "Z", "typing replaces the proposed name");
    rig.key("Y");
    assert_eq!(deck.get_form_name().as_str(), "ZY", "twin: the next key adds to it");
    assert!(deck.get_form_valid());
    deck.invoke_form_named("   ".into());
    assert!(!deck.get_form_valid(), "blanks are no name");
    deck.invoke_save_confirmed();
    assert!(deck.get_saving(), "Save does nothing on blanks");
    deck.invoke_form_named(" Kept ".into());
    assert!(deck.get_form_valid(), "twin: a name among blanks is one");

    // Each swatch rings in its own tint's ink; the balance menu ticks the current choice.
    let inks = deck.get_tint_inks();
    let (black, white) = (slint::Color::from_rgb_u8(0, 0, 0), slint::Color::from_rgb_u8(255, 255, 255));
    for (i, tint) in Tint::ALL.iter().enumerate() {
        assert_eq!(inks.row_data(i), Some(if tint.wants_dark_ink() { black } else { white }), "{tint:?}");
    }
    assert!(Tint::ALL.iter().any(|t| t.wants_dark_ink()) && Tint::ALL.iter().any(|t| !t.wants_dark_ink()), "twin: one ink for all would miss a tint");
    assert_eq!(deck.get_form_balance(), 0, "the photo's own balance proposes Not carried");
    // The menu drops under its field: its third line, Automatic, is where the pointer finds it.
    let field = common::the(w, Role::Button, "Not carried").middle();
    click(&rig, field.0, field.1);
    rig.settle();
    println!("shot {}", rig.shot("preset-balance-menu").display());
    let automatic = common::in_popup(w, Role::Button, "Automatic WB (Temperature, Tint)");
    assert!(automatic.y > field.1, "the menu drops under its field: {automatic:?}");
    click(&rig, automatic.middle().0, automatic.middle().1);
    rig.settle();
    assert_eq!(deck.get_form_balance(), 2, "Automatic picked under the field");
    deck.invoke_save_cancelled();
    rig.pump();
    assert!(menus.get_live().save_preset, "twin: and lit once it is closed");

    // The editor updates its rows in place as a name is typed, so hover and scroll hold.
    ctl.act(oni_app::Act::EditPresets);
    rig.settle();
    println!("shot {}", rig.shot("preset-editor").display());
    let rows = deck.get_rows();
    deck.invoke_form_named("Renamed".into());
    rig.pump();
    assert!(deck.get_rows() == rows, "a keystroke keeps the rows' model");
    assert_eq!(deck.get_rows().row_data(0).unwrap().name.as_str(), "Renamed", "twin: the row shows the name typed");
    let name = common::the(w, Role::TextInput, "Preset name").middle();
    click(&rig, name.0, name.1);
    rig.key("\n");
    assert!(!deck.get_editing(), "Return in a field is Done");
    let _ = std::fs::remove_dir_all(&root);
}
