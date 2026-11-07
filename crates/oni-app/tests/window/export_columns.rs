//! The export's EXIF in one column on the left of each photo's preview, a line a photo, the preview
//! taking the room the fields leave; and every word a field may hold whole, in English and in French.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::Exporting;
use oni_app::app::export_card;
use oni_app::app::{Options, Shell};
use oni_core::{Archive, EditorStore};
use oni_i18n::Language;
use slint::{ComponentHandle, Model};
use slint::platform::WindowEvent;
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

/// `Space.l`: the gap between the fields and the preview, the column's and the panel's padding.
const GAP: f32 = 24.0;
/// `CardStyle.inset`: the card's room off the window's ground.
const INSET: f32 = 8.0;
/// `FieldStyle.across`, a field's padding on either side of its words.
const ACROSS: f32 = 16.0;
/// `FormStyle.label-width`, the room a field's label takes before it.
const LABEL: f32 = 72.0;
/// A picker's chevron, `FormStyle.chip-icon`, and its gap before it, `Space.s`.
const CHEVRON: f32 = 24.0 * 0.7 + 8.0;
/// A column too narrow for the longest words, `DS.control-export-fields-width`.
const NARROW: f32 = 260.0;

/// The fields typed in, by the names assistive tools read; the film's picker is named by its value.
const TYPED: [&str; 7] = ["Name", "Camera", "Lens", "ISO", "Date", "Artist", "Copyright"];

/// A long camera as a scan names it, the maker before the model, and a long lens.
const CAMERA: &str = "NIKON CORPORATION NIKON Z f";
const LENS: &str = "NIKKOR Z MC 105mm f/2.8 VR S";

/// Each labelled thing of line `k`, from the top: the typed fields and the film's picker.
fn fields(w: &oni_app::MainWindow, k: usize, film: &str) -> Vec<(&'static str, common::Placed)> {
    let nth = |role, name: &str| {
        let mut found = common::placed(w, role, name);
        found.sort_by(|a, b| a.y.total_cmp(&b.y));
        assert!(found.len() > k, "a {name:?} on line {k}: {found:?}");
        found[k]
    };
    let mut out: Vec<_> = TYPED.iter().map(|&name| (name, nth(Role::TextInput, name))).collect();
    out.push(("Film stock", nth(Role::Button, film)));
    out.sort_by(|a, b| a.1.y.total_cmp(&b.1.y));
    out
}

/// Line 0's TextInput named `field`, where the layout put it.
fn input(w: &oni_app::MainWindow, field: &str) -> common::Placed {
    let mut found = common::placed(w, Role::TextInput, field);
    found.sort_by(|a, b| a.y.total_cmp(&b.y));
    found[0]
}

/// Line 0 as the window shows it, `field` holding `text`.
fn hold(w: &oni_app::MainWindow, field: &str, text: &str) {
    let model = w.global::<Exporting>().get_lines();
    let mut line = model.row_data(0).unwrap();
    let slot = match field {
        "Name" => &mut line.name,
        "Camera" => &mut line.camera,
        "Lens" => &mut line.lens,
        "ISO" => &mut line.iso,
        "Date" => &mut line.date,
        "Artist" => &mut line.artist,
        _ => &mut line.copyright,
    };
    *slot = text.into();
    model.set_row_data(0, line);
}

/// How wide `text` runs as a typed field's value: its words' input grows past the field's room once
/// they are longer than it, which the words at rest then end on an ellipsis.
fn typed_width(rig: &common::Rig, w: &oni_app::MainWindow, field: &str, text: &str) -> f32 {
    hold(w, field, text);
    rig.pump();
    let wide = input(w, field).width;
    hold(w, field, "");
    rig.pump();
    wide
}

/// How wide `text` runs as a label's or a picker's words, read off the window: the extension after
/// line 0's empty name takes its words' width out of the name's room, which must keep some.
fn width_of(rig: &common::Rig, w: &oni_app::MainWindow, text: &str) -> f32 {
    let e = w.global::<Exporting>();
    e.set_extension("".into());
    rig.pump();
    let whole = input(w, "Name").width;
    e.set_extension(text.into());
    rig.pump();
    let left = input(w, "Name").width;
    assert!(left > 1.0, "{text:?} took the name's whole field, so it was not measured");
    whole - left
}

/// The words every field may show, an example while empty or a long value, each in its field's room,
/// and every label in its own; `language` is the one the words are read in.
fn every_word_whole(rig: &common::Rig, w: &oni_app::MainWindow, rooms: &[(&str, f32)], language: Language) {
    oni_i18n::set(language);
    let tr = oni_i18n::tr;
    let typed: Vec<(&str, String)> = vec![
        ("Camera", tr("e.g. Leica M6").into()),
        ("Camera", CAMERA.into()),
        ("Lens", tr("e.g. Summicron 50 mm").into()),
        ("Lens", LENS.into()),
        ("ISO", tr("e.g. 400").into()),
        ("Date", tr("e.g. 2026-09-05 10:00:00").into()),
        ("Artist", tr("e.g. Jane Doe").into()),
        ("Copyright", tr("e.g. © 2026 Jane Doe").into()),
    ];
    let mut named: Vec<(&str, String)> = vec![("Film stock", tr("Choose…").into())];
    named.extend(oni_settings::film::FILM_STOCKS.iter().map(|k| ("Film stock", k.display_name.to_owned())));
    let labels = ["Name", "Camera", "Lens", "Film stock", "ISO", "Date", "Artist", "Copyright"];
    named.extend(labels.iter().map(|&l| ("label", tr(l).to_owned())));
    oni_i18n::set(Language::English);
    let room = |field: &str| if field == "label" { LABEL } else { rooms.iter().find(|(n, _)| *n == field).map(|(_, r)| *r).unwrap() };
    for (field, text) in typed {
        let wide = typed_width(rig, w, field, &text);
        assert!(wide <= room(field) + 0.5, "{language:?}: {text:?} runs {wide} in the {field} field's {}", room(field));
    }
    for (field, text) in named {
        let wide = width_of(rig, w, &text);
        assert!(wide <= room(field), "{language:?}: {text:?} runs {wide} in the {field}'s {}", room(field));
    }
}

pub(crate) fn the_exif_stands_left_of_each_preview_and_no_word_is_cut() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the export's columns are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-export-columns-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let folder = root.join("Scans");
    common::plain_tiff(&folder.join("A.tif"), 96, 64, 0.5);
    common::plain_tiff(&folder.join("B.tif"), 64, 96, 0.6);
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 1100));
    shell.window.show().unwrap();
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let lib = w.global::<oni_app::Library>();
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(1, true, false);
    ctl.act(oni_app::Act::Export);
    let shown = || export_card::lines(w).iter().all(|l| l.ready && !l.caption.is_empty() && l.state == oni_app::PreviewState::Shown);
    rig.until("both lines and their previews", Duration::from_secs(120), shown);
    rig.settle();
    let lines = export_card::lines(w);
    assert_eq!(lines.len(), 2, "one line a photo");
    let presets = common::the(w, Role::Text, "Presets");
    let caption = |k: usize| {
        let found: Vec<_> = common::placed(w, Role::Text, &lines[k].caption).into_iter().filter(|c| c.x < presets.x).collect();
        assert_eq!(found.len(), 1, "line {k}'s caption under its preview: {found:?}");
        found[0]
    };

    // Each line's fields in one column, all left of its preview, the preview beside them from their
    // top; twin: the panel's own field, right of the previews, fails the same test.
    let left_of = |a: common::Placed, b: common::Placed| a.x + a.width + GAP <= b.x;
    for k in 0..2 {
        let (laid, under) = (fields(w, k, "Choose…"), caption(k));
        let first = laid[0].1;
        // A typed field's words start a padding into its face, the picker's face at its own edge.
        let face = |(n, f): &(&str, common::Placed)| if *n == "Film stock" { f.x } else { f.x - ACROSS };
        assert!(laid.iter().all(|f| (face(f) - face(&laid[0])).abs() < 1.0), "line {k}'s fields one above the other: {laid:?}");
        assert!(laid.iter().all(|(_, f)| left_of(*f, under)), "line {k}'s fields left of its preview: {laid:?} {under:?}");
        assert!(first.y < under.y, "line {k}'s preview beside its fields, its caption under it: {first:?} {under:?}");
        // The preview takes what the fields leave of the room, up to the column's padding.
        let room = presets.x - GAP - INSET - GAP;
        assert!((under.x + under.width - room).abs() < 1.0, "line {k}'s preview reaches the room's edge: {under:?} {room}");
        assert!((first.x + first.width - room).abs() >= 1.0, "twin: the fields' own edge fails the same test: {first:?}");
    }
    assert!(!left_of(presets, caption(0)), "twin: the panel's Presets, right of the preview, fails the same test");
    let (above, below) = (fields(w, 0, "Choose…"), fields(w, 1, "Choose…"));
    let foot = above.last().map(|(_, f)| f.y + f.height).unwrap().max(caption(0).y + caption(0).height);
    assert!(below[0].1.y > foot, "the second photo's line under the first's: {foot} {:?}", below[0].1);
    println!("shot {}", rig.shot("export-columns").display());

    // Each field's room for its words: a typed one's, as its short value fills it; the picker's, less its chevron.
    let rooms: Vec<(&str, f32)> = above.iter().map(|(n, f)| (*n, if *n == "Film stock" { f.width - 2.0 * ACROSS - CHEVRON } else { f.width })).collect();
    let (e, name) = (w.global::<Exporting>(), lines[0].name.to_string());
    let extension = e.get_extension();
    hold(w, "Name", "");
    every_word_whole(&rig, w, &rooms, Language::English);
    every_word_whole(&rig, w, &rooms, Language::French);
    // Twins: a value longer than its field grows past it, and a 260 pt column cuts the camera's name
    // and the longest film stock's.
    let camera = rooms.iter().find(|(n, _)| *n == "Camera").unwrap().1;
    assert!(typed_width(&rig, w, "Camera", &format!("{CAMERA} {LENS}")) > camera + 0.5, "a value longer than its field reads as cut");
    let cramped = NARROW - LABEL - 8.0 - 2.0 * ACROSS;
    assert!(width_of(&rig, w, CAMERA) > cramped, "the camera's name fits {cramped}: the check cannot tell a cramped column");
    assert!(width_of(&rig, w, "Lomography LomoChrome Turquoise") > cramped - CHEVRON, "the stock's name fits a cramped picker: the check cannot tell it");
    e.set_extension(extension);
    hold(w, "Name", &name);
    // The narrowest window the app opens at still lays each preview beside its fields.
    rig.window.set_size(slint::PhysicalSize::new(1100, 680));
    rig.settle();
    println!("shot {}", rig.shot("export-columns-narrowest").display());
    let _ = std::fs::remove_dir_all(&root);
}
