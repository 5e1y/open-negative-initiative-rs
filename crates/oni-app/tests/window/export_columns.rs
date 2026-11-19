//! The export's pictures first: each line's EXIF folded in a card that a key left of its picture opens for
//! every line at once, each label over its field, the picture taking what the key and the card leave; the
//! file's name under its picture whether the card is open or not.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::Exporting;
use oni_app::app::export_card;
use oni_app::app::{Options, Shell};
use oni_core::{Archive, EditorStore};
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

/// `Space.l`: the column's and the panel's padding.
const GAP: f32 = 24.0;
/// `Space.m`: from the EXIF's key or its card to the picture.
const NEAR: f32 = 16.0;
/// `Space.s`: the EXIF card's padding round its fields.
const PAD: f32 = 8.0;
/// `CardStyle.inset`: the card's room off the window's ground.
const INSET: f32 = 8.0;
/// `FieldStyle.across`, a field's padding on either side of its words.
const ACROSS: f32 = 16.0;
/// `Space.s`: the name's band around its field.
const BAND: f32 = 8.0;
/// `FormStyle.fields-width`: the EXIF's card.
const CARD: f32 = 260.0;

/// The EXIF typed in, by the names assistive tools read; the film's picker is named by its value.
const TYPED: [&str; 6] = ["Camera", "Lens", "ISO", "Date", "Artist", "Copyright"];

/// The `k`-th from the top of what is so named.
fn nth(w: &oni_app::MainWindow, role: Role, name: &str, k: usize) -> common::Placed {
    let mut found = common::placed(w, role, name);
    found.sort_by(|a, b| a.y.total_cmp(&b.y));
    assert!(found.len() > k, "a {name:?} on line {k}: {found:?}");
    found[k]
}

/// Each EXIF field of line `k`, from the top: the typed fields and the film's picker.
fn fields(w: &oni_app::MainWindow, k: usize) -> Vec<(&'static str, common::Placed)> {
    let mut out: Vec<_> = TYPED.iter().map(|&name| (name, nth(w, Role::TextInput, name, k))).collect();
    out.push(("Film stock", nth(w, Role::Button, "Choose…", k)));
    out.sort_by(|a, b| a.1.y.total_cmp(&b.1.y));
    out
}

pub(crate) fn the_exif_folds_beside_each_picture_and_the_name_stays_under_it() {
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
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    // Tall enough for both lines, each picture as wide as the room leaves it.
    rig.window.set_size(slint::PhysicalSize::new(1280, 2600));
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
    // A picture's left edge, read off its name's band under it, and its right edge off its size's end.
    let picture = |k: usize| nth(w, Role::TextInput, "Name", k).x - BAND - ACROSS;
    let end = |k: usize| caption(k).x + caption(k).width;
    // The room's edge, up to the column's padding.
    let room = presets.x - GAP - INSET - GAP;
    // The picture in the middle of its slot, from `left` to the room's edge, its name and size under it.
    let centred = |k: usize, left: f32| picture(k) >= left - 0.5 && ((picture(k) - left) - (room - end(k))).abs() < 1.0;

    // Closed as the screen opens: no EXIF, each picture right after its key, its name under it.
    assert!(common::placed(w, Role::TextInput, "Camera").is_empty(), "the EXIF closed until asked for");
    let mut keys = common::placed(w, Role::Button, "Show the EXIF");
    keys.sort_by(|a, b| a.y.total_cmp(&b.y));
    assert_eq!(keys.len(), 2, "one EXIF key a line: {keys:?}");
    let closed: Vec<f32> = (0..2).map(picture).collect();
    for k in 0..2 {
        assert!(centred(k, keys[k].x + keys[k].width + NEAR), "line {k}'s picture after its key, its name under it: {:?} {} {}", keys[k], closed[k], end(k));
        assert!(nth(w, Role::TextInput, "Name", k).y > keys[k].y + keys[k].height, "line {k}'s name under its picture");
    }
    println!("shot {}", rig.shot("export-columns").display());

    // The key opens the card on every line in its place: each label over its field, the fields one above the
    // other, left of the picture, which gives the card what the key took less.
    common::click(&rig, w, keys[0].middle());
    rig.settle();
    assert!(w.global::<Exporting>().get_exif_shown(), "the key opens the EXIF");
    for k in 0..2 {
        let laid = fields(w, k);
        let face = |(n, f): &(&str, common::Placed)| if *n == "Film stock" { f.x } else { f.x - ACROSS };
        assert!(laid.iter().all(|f| (face(f) - face(&laid[0])).abs() < 1.0), "line {k}'s fields one above the other: {laid:?}");
        let open = picture(k);
        assert!(laid.iter().all(|(_, f)| f.x + f.width + NEAR <= open), "line {k}'s fields left of its picture: {laid:?} {open}");
        // The card's face ends a padding past its fields; the picture's slot starts a gap after it.
        let card = laid.iter().map(|(_, f)| f.x + f.width).fold(0.0, f32::max) + PAD;
        assert!(centred(k, card + NEAR), "line {k}'s picture in the room the card leaves: {open} {}", end(k));
        assert!((keys[k].x + CARD - card).abs() < 1.5, "line {k}'s card in its key's place: {card} {:?}", keys[k]);
        assert!(common::placed(w, Role::Button, "Show the EXIF").is_empty(), "open, the card's cross stands for the key");
        let mut labels: Vec<_> = common::placed(w, Role::Text, "Camera").into_iter().filter(|t| t.x < presets.x).collect();
        labels.sort_by(|a, b| a.y.total_cmp(&b.y));
        let (label, field) = (labels[k], nth(w, Role::TextInput, "Camera", k));
        assert!(label.y + label.height <= field.y && (label.x - (field.x - ACROSS)).abs() < 1.0, "line {k}'s label over its field: {label:?} {field:?}");
        // Twin: the panel's Presets, right of the pictures, fails the same test.
        assert!(presets.x + presets.width + NEAR > open, "twin: the panel's Presets is not left of the picture");
    }
    let (above, below) = (fields(w, 0), fields(w, 1));
    let foot = above.last().map(|(_, f)| f.y + f.height).unwrap().max(caption(0).y + caption(0).height);
    assert!(below[0].1.y > foot, "the second photo's line under the first's: {foot} {:?}", below[0].1);
    println!("shot {}", rig.shot("export-columns-exif").display());

    // Closed again by the card's cross, at its top right: the card gone, the picture back to its width.
    let hide = common::placed(w, Role::Button, "Hide the EXIF");
    assert_eq!(hide.len(), 2, "a cross on each card: {hide:?}");
    let first = fields(w, 0);
    assert!(hide.iter().any(|x| x.y < first[0].1.y && x.x > first[0].1.x), "the cross over the first field, at its right: {hide:?} {:?}", first[0].1);
    common::click(&rig, w, hide[0].middle());
    rig.settle();
    assert!(common::placed(w, Role::TextInput, "Camera").is_empty(), "the card closes on every line");
    assert!((picture(0) - closed[0]).abs() < 1.0, "the picture back to its width: {} {}", picture(0), closed[0]);
    // The narrowest window the app opens at still lays each picture whole beside its key.
    rig.window.set_size(slint::PhysicalSize::new(1100, 680));
    rig.settle();
    println!("shot {}", rig.shot("export-columns-narrowest").display());

    // An upright frame alone, its height the room's: it stands in the middle of its slot, its name and its
    // size under it, not under the slot's edge; twin: off the key's side by a good part of the slot.
    ctl.leave_export();
    rig.window.set_size(slint::PhysicalSize::new(1280, 900));
    rig.settle();
    lib.invoke_tile_clicked(1, false, false);
    ctl.act(oni_app::Act::Export);
    rig.until("the upright frame's line", Duration::from_secs(120), || export_card::lines(w).len() == 1 && shown());
    rig.settle();
    let key = common::the(w, Role::Button, "Show the EXIF");
    let lone = export_card::lines(w)[0].caption.to_string();
    let size = common::placed(w, Role::Text, &lone).into_iter().find(|c| c.x < presets.x).expect("its size");
    let (left, slot) = (nth(w, Role::TextInput, "Name", 0).x - BAND - ACROSS, key.x + key.width + NEAR);
    assert!(((left - slot) - (room - (size.x + size.width))).abs() < 1.0, "the upright frame's name and size under it: {left} {slot} {size:?}");
    assert!(left - slot > 40.0, "twin: off the slot's edge: {left} {slot}");
    println!("shot {}", rig.shot("export-columns-upright").display());
    let _ = std::fs::remove_dir_all(&root);
}
