//! The export surface as the window lays and draws it: one step between the panel's groups whatever
//! the space's note wraps to, the summary in the panel's words, each field's face and example beside
//! the picture, the date shown dashed while the file keeps EXIF's colons, and the presets in one picker
//! whose choices move no row under it.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::Exporting;
use oni_app::app::export_card;
use slint::ComponentHandle;

use crate::common;
use crate::panel_bindings::{opened, opened_with};

/// The panel column's step between groups, `Space.m`.
const STEP: f32 = 16.0;

/// The gap from `above`'s foot to `below`'s head.
fn gap(above: common::Placed, below: common::Placed) -> f32 {
    below.y - (above.y + above.height)
}

pub(crate) fn the_export_panel_keeps_one_step_between_its_groups() {
    let Some((rig, shell, root)) = opened("export-panel") else { return };
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let e = w.global::<Exporting>();
    ctl.act(oni_app::Act::Export);
    rig.settle();
    let label = common::the(w, Role::Text, "Presets");
    // The space under the one name the canvas's chip opens on.
    common::the(w, Role::Text, "Colour space");

    // Display P3's note runs on two lines, the case that ran the keys under it over Quality; its room
    // holds the tallest note it may say. A space naming its own profile shows no Profile select: the
    // keys stand a step under that room.
    let note = common::the(w, Role::Text, &e.get_space_note());
    assert!(note.height > 1.5 * label.height, "the note wraps: {note:?}");
    assert!(common::placed(w, Role::Text, "Profile").is_empty(), "no Profile select where it cannot change");
    let (keys, quality) = (common::the(w, Role::Button, "Load a Profile…"), common::the(w, Role::Slider, "Quality"));
    assert!(gap(note, keys) >= STEP, "the keys at least one step under the note: {note:?} {keys:?}");
    let edited = common::the(w, Role::Text, &e.get_edited_in());
    assert_eq!(gap(keys, edited), STEP, "the edited-in note one step under the keys: {keys:?} {edited:?}");
    assert!(gap(edited, quality) >= STEP, "Quality at least one step under the note: {edited:?} {quality:?}");

    // The summary one fact a row, in the panel's words, at the panel's foot; twin: the subtitle, a
    // note with its lines opened, measures taller, so the check tells the two faces apart.
    let over = slint::LogicalPosition::new(quality.x + quality.width / 2.0, quality.y);
    rig.window.dispatch_event(slint::platform::WindowEvent::PointerScrolled { position: over, delta_x: 0.0, delta_y: -400.0 });
    rig.settle();
    let rows = ["Pixels", "Bit depth", &e.get_weight_label()].map(|r| common::the(w, Role::Text, r));
    assert!(rows.iter().all(|r| r.height == label.height), "the summary as large as a field's label: {rows:?}");
    assert!(rows.windows(2).all(|p| p[1].y > p[0].y + p[0].height / 2.0), "each fact on a row of its own: {rows:?}");
    rig.window.dispatch_event(slint::platform::WindowEvent::PointerScrolled { position: over, delta_x: 0.0, delta_y: 400.0 });
    rig.settle();
    let subtitle = common::the(w, Role::Text, &e.get_subtitle());
    assert!(subtitle.height > label.height, "{subtitle:?} against {label:?}");

    // Custom: its note under the tabs, and its Profile select, one step above the keys it pushes down.
    e.invoke_space_picked(4);
    rig.settle();
    assert_eq!(common::placed(w, Role::Text, &e.get_space_note()).len(), 1, "Custom's note shows under the tabs");
    let profile = common::the(w, Role::Text, "Profile");
    let moved = common::the(w, Role::Button, "Load a Profile…");
    assert!(profile.y > note.y && moved.y > profile.y, "the Profile select between the note and the keys: {note:?} {profile:?} {moved:?}");
    assert!(moved.y > keys.y, "twin: the select takes room the fixed profile left free: {keys:?} {moved:?}");

    // Each field's face shows on its ground; twin: on the canvas, where the fields stood, it would not.
    let (width, drawn) = rig.drawn();
    let at = |x: f32, y: f32| drawn[y as usize * width + x as usize].0;
    let artist = common::the(w, Role::TextInput, "Artist");
    let middle = artist.y + artist.height / 2.0;
    let face = at(artist.x - 8.0, middle);
    assert_ne!(face, at(artist.x - 20.0, middle), "the Artist field's face shows beside its label");
    let caption = common::the(w, Role::Text, &export_card::lines(w)[0].caption);
    assert_eq!(at(caption.x - 20.0, caption.y + caption.height / 2.0), face, "the canvas is the face's own ground");
    // A lone photo stands in the middle of its room, as much above its fields as under its caption,
    // between the panel's title and its keys; twin: from the top it would stand level with the title.
    let (head, cancel, name) = (common::the(w, Role::Text, "Export"), common::the(w, Role::Button, "Cancel"), common::the(w, Role::TextInput, "Name"));
    let (above, below) = (name.y - head.y, cancel.y + cancel.height - caption.y - caption.height);
    assert!((above - below).abs() < 48.0, "the lone photo centred: {above} above, {below} below");
    assert!(above > 48.0, "the lone photo off the head's level: {above}");

    // An empty field shows an example, never what reads as a value; twin: a name typed hides it.
    let example = "e.g. Jane Doe";
    assert_eq!(common::placed(w, Role::Text, example).len(), 1, "the empty Artist field shows its example");
    common::click(&rig, w, artist.middle());
    rig.key("Ansel Adams");
    rig.settle();
    assert!(common::placed(w, Role::Text, example).is_empty(), "the typed name stands alone");

    // A date typed as shown reaches the file in EXIF's form and comes back dashed.
    e.invoke_edited(0, "date".into(), "2026-09-14 16:46:19".into());
    let kept = export_card::card(&ctl).rows[0].as_ref().map(|r| r.metadata.date.clone());
    assert_eq!(kept.as_deref(), Some("2026:09:14 16:46:19"));
    e.invoke_full_size();
    rig.settle();
    assert_eq!(export_card::lines(w)[0].date, "2026-09-14 16:46:19");
    presets_keep_the_rows_in_place(&rig, &shell);
    let _ = std::fs::remove_dir_all(&root);
}

/// Where each row under the picker lies, from the format's tabs to the summary at the panel's foot.
fn rows(w: &oni_app::MainWindow) -> Vec<(&'static str, f32)> {
    let rows = [
        (Role::Tab, "JPEG"),
        (Role::Tab, "Adobe RGB"),
        (Role::Button, "Load a Profile…"),
        (Role::Text, "Size"),
        (Role::Checkbox, "Brighter on HDR screens"),
        (Role::Text, "Pixels"),
    ];
    rows.into_iter().map(|(role, name)| (name, common::the(w, role, name).y)).collect()
}

/// The picker's face, named by the preset the settings equal and its summary.
fn picker(w: &oni_app::MainWindow) -> String {
    let e = w.global::<Exporting>();
    let summary = e.get_preset_summary();
    if summary.is_empty() { e.get_preset_title().to_string() } else { format!("{} · {summary}", e.get_preset_title()) }
}

/// Whether a popup shows over the window, as the picker's list does once dropped.
fn dropped(w: &oni_app::MainWindow) -> bool {
    !i_slint_core::window::WindowInner::from_pub(w.window()).active_popups().is_empty()
}

/// The picker's list dropped, and `line` of it clicked, which closes it.
fn choose(rig: &common::Rig, w: &oni_app::MainWindow, line: &str) {
    common::click(rig, w, common::the(w, Role::Button, &picker(w)).middle());
    rig.until("the picker's list", std::time::Duration::from_secs(5), || dropped(w));
    common::click(rig, w, common::in_popup(w, Role::Button, line).middle());
    rig.until("the list closed", std::time::Duration::from_secs(5), || !dropped(w));
    rig.settle();
}

/// One picker for every preset, each line a name over its summary; choosing one moves no row under it
/// whatever its space's notes say, and the list ends with what saves, renames and removes the hand's own.
fn presets_keep_the_rows_in_place(rig: &common::Rig, shell: &oni_app::app::Shell) {
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let e = w.global::<Exporting>();
    // Tall enough for the whole panel, so every row is shown to be found.
    rig.window.set_size(slint::PhysicalSize::new(1280, 1240));
    e.invoke_space_picked(1);
    rig.settle();
    assert_eq!(picker(w), "No preset", "the rows as they opened equal no preset");
    let before = rows(w);
    let edited = |e: &Exporting| common::the(w, Role::Text, &e.get_edited_in());
    let (said_p3, purpose_p3) = (e.get_edited_in(), common::the(w, Role::Text, &e.get_space_note()).height);

    // Web by a click: JPEG, sRGB, 2048 on the long edge at 85 %, named at rest with its summary.
    choose(rig, w, "Web");
    let c = export_card::card(&ctl).settings.clone();
    assert_eq!((c.format(), c.resize, c.jpeg_quality), (oni_export::Format::Jpeg, oni_export::Resize::LongestEdge(2048), 0.85));
    assert_eq!((e.get_chip_title().as_str(), e.get_lit_preset()), ("sRGB", 3));
    assert_eq!(picker(w), "Web · JPEG · sRGB · 2048 px");
    assert_eq!(rows(w), before, "Web moved no row under the picker");
    // Twin: the note naming where the photo was edited says another sentence in sRGB, in the same room.
    assert_ne!(e.get_edited_in(), said_p3, "the edited-in note says the canvas now shows another space");
    let room = common::the(w, Role::Slider, "Quality").y - STEP - edited(&e).y;
    assert!(room >= edited(&e).height, "the room {room} holds the words {:?}", edited(&e));
    rig.shot("export-preset-web");

    // Print from the keyboard: the arrows walk the list, Return chooses.
    common::click(rig, w, common::the(w, Role::Button, &picker(w)).middle());
    rig.until("the picker's list", std::time::Duration::from_secs(5), || dropped(w));
    let (down, ret): (slint::SharedString, slint::SharedString) = (slint::platform::Key::DownArrow.into(), slint::platform::Key::Return.into());
    for _ in 0..3 {
        rig.key(&down);
    }
    rig.key(&ret);
    rig.settle();
    assert_eq!((e.get_format(), e.get_chip_title().as_str(), e.get_lit_preset()), (0, "Adobe RGB", 2), "Print wrote its rows");
    assert_eq!(rows(w), before, "Print moved no row under the picker");
    // Twin: the format's row holds another control, Compression's in place of Quality's.
    assert!(common::placed(w, Role::Slider, "Quality").is_empty());
    assert_eq!(common::placed(w, Role::Checkbox, "ZIP, lossless").len(), 1);
    assert_ne!(common::the(w, Role::Text, &e.get_space_note()).height, purpose_p3, "twin: Adobe RGB's purpose takes another height");
    for line in ["Instagram", "Display"] {
        choose(rig, w, line);
        assert_eq!(rows(w), before, "{line} moved no row under the picker");
    }
    assert_eq!(e.get_lit_preset(), 1);
    e.invoke_quality_step(-1);
    rig.pump();
    assert_eq!((picker(w).as_str(), e.get_lit_preset()), ("No preset", -1), "a row set by hand equals no preset");

    // Save as Preset… ends the list and asks a name; its commit waits for one.
    choose(rig, w, "Save as Preset…");
    assert!(e.get_naming_shown() && e.get_naming_title() == "Save as Preset");
    common::click(rig, w, common::the(w, Role::Button, "Save").middle());
    assert!(e.get_naming_shown() && export_card::kept(&ctl).is_empty(), "twin: no name, nothing saved");
    common::click(rig, w, common::the(w, Role::TextInput, "Preset name").middle());
    rig.key("Client JPEG");
    rig.shot("export-preset-name");
    common::click(rig, w, common::the(w, Role::Button, "Save").middle());
    rig.settle();
    let kept = export_card::kept(&ctl);
    assert_eq!(kept.iter().map(|k| k.name.as_str()).collect::<Vec<_>>(), ["Client JPEG"]);
    assert!(!e.get_naming_shown() && picker(w) == "Client JPEG · JPEG · Display P3 · Full resolution", "{}", picker(w));
    common::click(rig, w, common::the(w, Role::Button, &picker(w)).middle());
    rig.until("the picker's list", std::time::Duration::from_secs(5), || dropped(w));
    rig.shot("export-presets");
    common::click(rig, w, common::in_popup(w, Role::Button, "Print").middle());
    rig.until("the list closed", std::time::Duration::from_secs(5), || !dropped(w));
    assert_eq!(e.get_lit_preset(), 2, "Print chosen from the list");
    // The saved line writes every setting back in one choice; Rename opens on its name, Return renames.
    choose(rig, w, "Client JPEG");
    assert_eq!(export_card::card(&ctl).settings, kept[0].settings, "the saved settings back whole");
    choose(rig, w, "Rename “Client JPEG”…");
    assert_eq!(e.get_naming_name(), "Client JPEG");
    rig.key("Lab JPEG");
    rig.key(&ret);
    rig.settle();
    assert_eq!(export_card::kept(&ctl)[0].name, "Lab JPEG", "typed over, the name renames it");
    assert!(picker(w).starts_with("Lab JPEG · "));
    // Remove takes it out of the list and leaves the settings as they are; Escape leaves the name sheet.
    choose(rig, w, "Remove “Lab JPEG”");
    assert!(export_card::kept(&ctl).is_empty() && picker(w) == "No preset");
    assert_eq!(export_card::card(&ctl).settings, kept[0].settings, "Remove changes no setting");
    choose(rig, w, "Save as Preset…");
    rig.key("Kept?");
    rig.key("\u{1b}");
    rig.settle();
    assert!(!e.get_naming_shown() && export_card::kept(&ctl).is_empty(), "Escape keeps nothing");
    rig.shot("export-panel");
}

/// A preset saved before Web was built in, holding Web's settings: its own name stands for them, its
/// line is the one checked, Rename reaches it, and the file keeps the two words 1.5.6 reads it by.
pub(crate) fn a_preset_saved_before_web_keeps_its_name_and_its_file() {
    let web = oni_export::Preset::Web.applied(&oni_export::Settings::default());
    let settings: serde_json::Value = serde_json::from_str(&web.raw_value()).unwrap();
    let file = serde_json::json!({ "exportPresets": [{ "name": "Web 2048", "settings": settings }] });
    let seed = |support: &std::path::Path| std::fs::write(support.join("preferences.json"), file.to_string()).unwrap();
    let Some((rig, shell, root)) = opened_with("export-kept", seed) else { return };
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let e = w.global::<Exporting>();
    ctl.act(oni_app::Act::Export);
    rig.settle();
    assert_eq!(export_card::kept(&ctl).iter().map(|k| k.name.as_str()).collect::<Vec<_>>(), ["Web 2048"]);
    choose(&rig, w, "Web");
    assert_eq!(export_card::card(&ctl).settings, web);
    // Twin: the built-in Web's line, the fourth, is not the one checked, the saved one after it is.
    assert_ne!(e.get_lit_preset(), 3, "the built-in Web's line holds the settings too, yet the saved name stands");
    assert_eq!(e.get_lit_preset(), 4, "the saved line checked, after the four built-in ones");
    assert_eq!(picker(w), "Web 2048 · JPEG · sRGB · 2048 px");
    choose(&rig, w, "Rename “Web 2048”…");
    rig.key("Blog");
    let ret: slint::SharedString = slint::platform::Key::Return.into();
    rig.key(&ret);
    rig.settle();
    assert_eq!(picker(w), "Blog · JPEG · sRGB · 2048 px");
    let path = root.join("Support/preferences.json");
    let read = || serde_json::from_slice::<serde_json::Value>(&std::fs::read(&path).unwrap_or_default()).unwrap_or_default();
    rig.until("the rename kept", std::time::Duration::from_secs(10), || read()["exportPresets"][0]["name"] == "Blog");
    let item = read()["exportPresets"][0].clone();
    let words: Vec<&String> = item.as_object().unwrap().keys().collect();
    assert_eq!(words, ["name", "settings"], "each preset in the two words 1.5.6 lists it by");
    assert_eq!(oni_export::Settings::from_raw_value(&item["settings"].to_string()), Some(web), "its settings in their own words");
    let _ = std::fs::remove_dir_all(&root);
}
