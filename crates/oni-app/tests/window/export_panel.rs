//! The export surface as the window lays and draws it: one step between the panel's groups whatever
//! the space's note wraps to, the summary in the panel's words, each field's face and example under
//! the picture, the date shown dashed while the file keeps EXIF's colons, and the presets as keys.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::Exporting;
use oni_app::app::export_card;
use slint::ComponentHandle;

use crate::common;
use crate::panel_bindings::opened;

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

    // Display P3's note runs on two lines, the case that ran the keys under it over Quality.
    let note = common::the(w, Role::Text, &e.get_space_note());
    assert!(note.height > 1.5 * label.height, "the note wraps: {note:?}");
    let (keys, quality) = (common::the(w, Role::Button, "Load a Profile…"), common::the(w, Role::Slider, "Quality"));
    assert_eq!(gap(note, keys), STEP, "the keys one step under the note: {note:?} {keys:?}");
    assert_eq!(gap(keys, quality), STEP, "Quality one step under the keys: {keys:?} {quality:?}");

    // The summary in the panel's words; twin: the subtitle's smaller face measures smaller, so the
    // check tells the two sizes apart.
    let summary = common::the(w, Role::Text, &e.get_bits());
    assert_eq!(summary.height, label.height, "the summary as large as a field's label");
    let subtitle = common::the(w, Role::Text, &e.get_subtitle());
    assert!(subtitle.height < label.height, "{subtitle:?} against {label:?}");

    // Custom: its note under the tabs, and the keys one step above what follows them.
    e.invoke_space_picked(4);
    rig.settle();
    assert_eq!(common::placed(w, Role::Text, &e.get_space_note()).len(), 1, "Custom's note shows under the tabs");
    let keys = common::the(w, Role::Button, "Load a Profile…");
    let edited = e.get_edited_in().to_string();
    let next = if edited.is_empty() { common::the(w, Role::Slider, "Quality") } else { common::the(w, Role::Text, &edited) };
    assert_eq!(gap(keys, next), STEP, "one step under the keys on Custom: {keys:?} {next:?}");

    // Each field's face shows on its ground; twin: on the canvas, where the fields stood, it would not.
    let (width, drawn) = rig.drawn();
    let at = |x: f32, y: f32| drawn[y as usize * width + x as usize].0;
    let artist = common::the(w, Role::TextInput, "Artist");
    let middle = artist.y + artist.height / 2.0;
    let face = at(artist.x - 8.0, middle);
    assert_ne!(face, at(artist.x - 20.0, middle), "the Artist field's face shows beside its label");
    let caption = common::the(w, Role::Text, &export_card::lines(w)[0].caption);
    assert_eq!(at(caption.x - 20.0, caption.y + caption.height / 2.0), face, "the canvas is the face's own ground");

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
    // The presets are keys that fill the rows, lit while the rows equal one; twin: a row set by hand lights none.
    common::click(&rig, w, common::the(w, Role::Button, "Instagram").middle());
    assert_eq!((e.get_format(), e.get_chip_title().as_str()), (1, "sRGB"), "Instagram wrote its rows");
    let lit = |name: &str| i_slint_backend_testing::ElementHandle::find_by_accessible_label(w, name).any(|k| k.accessible_role() == Some(Role::Button) && k.accessible_checked() == Some(true));
    assert!(lit("Instagram") && !lit("Display") && !lit("Print"));
    e.invoke_quality_step(-1);
    rig.pump();
    assert!(!lit("Instagram"), "a row set by hand lights no preset");
    rig.shot("export-panel");
    let _ = std::fs::remove_dir_all(&root);
}
