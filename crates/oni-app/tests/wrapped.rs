//! The year on film on the headless window: each gesture Swift notes lands in the journal, and
//! "Show My Year on Film" reads it back as the deck, turned by a click or the arrows, Esc leaving.

use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::{Library, Menus, PanelState, Store as StoreGlobal, Wrapped};
use oni_core::{Archive, EditorStore};
use oni_library::journal::{Journal, Kind};
use slint::platform::{Key, WindowEvent};
use slint::{ComponentHandle, Model};
use std::path::PathBuf;
use std::time::Duration;

mod common;

fn key(k: Key) -> slint::SharedString {
    k.into()
}

fn eyebrows(w: &Wrapped) -> Vec<String> {
    let cards = w.get_cards();
    (0..cards.row_count()).filter_map(|i| cards.row_data(i)).map(|c| c.eyebrow.to_string()).collect()
}

#[test]
fn every_gesture_is_noted_and_the_year_reads_back_as_its_deck() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the year on film is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-wrapped-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support) = (root.join("Scans"), root.join("Support"));
    for (name, tone) in [("A.tif", 0.9), ("B.tif", 0.6), ("C.tif", 0.4)] {
        common::linear_tiff(&folder.join(name), 96, 64, tone);
    }
    let lighting = oni_core::parameters::all().iter().position(|r| r.id == "levelsGlobal").unwrap();
    let mut parameters = std::collections::BTreeSet::new();
    parameters.insert("levelsGlobal".to_string());
    let preset = oni_core::presets::Preset::new("Portra warm", parameters, oni_core::presets::Tint::Amber, Default::default());
    oni_core::presets::save(&preset, &oni_core::presets::directory(&support)).unwrap();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support: support.clone(), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    let journal = Journal::in_support(&support);
    let kinds = || journal.read().0.iter().map(|r| r.kind).collect::<Vec<_>>();
    let a = oni_library::paths::canonical(&folder.join("A.tif"));

    // Opened, then one record for the one row an Exposure drag moved, written once the hand rests.
    let lib = w.global::<Library>();
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A on the canvas", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    assert_eq!(kinds(), [Kind::Opened]);
    let g = w.global::<StoreGlobal>();
    let field = Field::Exposure.index();
    g.invoke_press(field, false);
    for step in 1..=6 {
        g.invoke_drag(field, step as f32 * 10.0, 300.0, false);
        rig.pump();
    }
    g.invoke_release(field);
    rig.until("the edit settled", Duration::from_secs(10), || kinds().len() == 2);
    rig.settle();
    assert_eq!(kinds().len(), 2, "twin: six notches are one gesture, never six records");
    let (records, names) = journal.read();
    assert_eq!((records[1].kind, usize::from(records[1].detail)), (Kind::Edited, lighting), "one record for six notches");
    assert_eq!(names[records[1].subject as usize], a);

    // A medal, a paste, the balance key, a preset: one record each, as Swift notes them.
    ctl.act("medal-3");
    ctl.act("copy");
    ctl.act("paste");
    w.global::<PanelState>().invoke_auto_balance();
    w.global::<Menus>().invoke_apply_preset(0);
    rig.until("the gestures noted", Duration::from_secs(20), || kinds().len() >= 6);
    rig.settle();
    let (records, names) = journal.read();
    let noted: Vec<Kind> = records.iter().map(|r| r.kind).filter(|k| *k != Kind::Edited).collect();
    assert_eq!(noted, [Kind::Opened, Kind::Medal, Kind::Pasted, Kind::Balanced, Kind::Preset]);
    let medal = records.iter().find(|r| r.kind == Kind::Medal).unwrap();
    assert_eq!(medal.detail, oni_library::wrapped::GOLD);
    assert_eq!(records.iter().find(|r| r.kind == Kind::Pasted).unwrap().value, 1);
    let laid = records.iter().find(|r| r.kind == Kind::Preset).unwrap();
    assert_eq!((names[laid.subject as usize].as_str(), names[laid.value as usize].as_str()), (a.as_str(), "Portra warm"));

    // Show My Year on Film: the title, then the deck at the first click.
    let wrapped = w.global::<Wrapped>();
    ctl.act("wrapped");
    rig.until("the summary", Duration::from_secs(10), || wrapped.get_shown());
    assert!(!wrapped.get_dealt());
    assert_eq!(wrapped.get_year().as_str(), oni_library::wrapped::today().0.to_string());
    // Laid by a first frame, then grown in.
    rig.shot("wrapped-title");
    rig.settle();
    println!("shot {}", rig.shot("wrapped-title").display());
    let deck = eyebrows(&wrapped);
    assert_eq!(deck.first().map(String::as_str), Some("THIS YEAR"));
    assert!(deck.contains(&"THE ONLY THING I SEE IS GOLD".to_string()));
    assert!(deck.contains(&"YOUR SIGNATURE LOOK".to_string()));
    assert_eq!(deck.last().map(String::as_str), Some("SEE YOU NEXT YEAR"));
    let card = |eyebrow: &str| {
        let cards = wrapped.get_cards();
        (0..cards.row_count()).filter_map(|i| cards.row_data(i)).find(|c| c.eyebrow == eyebrow).unwrap()
    };
    assert_eq!((card("THIS YEAR").figure.as_str(), card("THIS YEAR").caption.as_str()), ("1", "negative brought back to life"));
    assert_eq!(card("YOUR MOST EDITED PHOTO").caption.as_str(), "A.tif \u{2014} overcooked?");
    assert!(card("YOUR HANDS KNEW THE WAY").caption.contains("Lighting"));
    assert_eq!(card("YOUR GO-TO BALANCE").caption.as_str(), "pressed 1 times");
    assert_eq!(card("YOUR SIGNATURE LOOK").figure.as_str(), "Portra warm");
    assert_eq!(card("THE ONLY THING I SEE IS GOLD").photos.row_count(), 1);
    rig.key(" ");
    assert!(wrapped.get_dealt());
    assert_eq!(wrapped.get_index(), 0, "twin: the first key deals the deck rather than turning it");
    rig.settle();
    println!("shot {}", rig.shot("wrapped-deck").display());
    rig.key(&key(Key::RightArrow));
    rig.key(&key(Key::RightArrow));
    rig.key(&key(Key::LeftArrow));
    assert_eq!(wrapped.get_index(), 1);
    // Past the last card it closes, as Esc does.
    for _ in 0..deck.len() {
        wrapped.invoke_forward();
    }
    assert!(!wrapped.get_shown());
    ctl.act("wrapped");
    rig.until("the summary again", Duration::from_secs(10), || wrapped.get_shown());
    // Drawn once, as a person sees it before pressing: the view takes the keyboard as it is laid.
    rig.shot("wrapped-again");
    assert!(!shell.window.get_keys_focused(), "the summary holds the keyboard");
    rig.key(&key(Key::Escape));
    assert!(!wrapped.get_shown(), "Esc leaves");

    // A photo in the ONI Trash counts for nothing: A was the only one opened.
    ctl.act("remove");
    rig.settle();
    ctl.act("wrapped");
    rig.until("the summary without A", Duration::from_secs(10), || wrapped.get_shown());
    let kept = eyebrows(&wrapped);
    assert!(!kept.contains(&"THIS YEAR".to_string()) && !kept.contains(&"THE ONLY THING I SEE IS GOLD".to_string()), "{kept:?}");
    wrapped.invoke_close();

    // December offers the year once; any other month offers nothing.
    let december = oni_library::wrapped::today().1 == 12;
    shell.offer_wrapped();
    rig.until("the offer read", Duration::from_secs(5), || !december || wrapped.get_shown());
    assert_eq!(wrapped.get_shown(), december);
    wrapped.invoke_close();
    shell.offer_wrapped();
    rig.settle();
    assert!(!wrapped.get_shown(), "never twice in a year");

    // New Roll from Selection notes the photos it gathered.
    assert!(w.global::<oni_app::App>().get_gallery(), "the trashed photo left the editor");
    lib.invoke_tile_clicked(0, false, false);
    ctl.act("new-roll");
    rig.until("the roll noted", Duration::from_secs(10), || kinds().contains(&Kind::Rolled));
    assert_eq!(journal.read().0.iter().find(|r| r.kind == Kind::Rolled).unwrap().value, 1);
    let _ = std::fs::remove_dir_all(&root);
}
