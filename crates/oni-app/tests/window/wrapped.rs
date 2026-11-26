//! The year on film on the headless window: each gesture lands in the journal, and "Show My Year on
//! Film" reads it and the library back as a strip of scenes, advanced by a key or by itself, each saved as a card.

use oni_app::fields::Field;
use oni_app::{Library, Menus, PanelState, Store, Wrapped};
use oni_library::journal::{Journal, Kind};
use slint::platform::Key;
use slint::{ComponentHandle, Model};
use std::time::Duration;

use crate::common;

fn key(k: Key) -> slint::SharedString {
    k.into()
}

fn eyebrows(w: &Wrapped) -> Vec<String> {
    let scenes = w.get_scenes();
    (0..scenes.row_count()).filter_map(|i| scenes.row_data(i)).map(|c| c.eyebrow.to_string()).collect()
}

pub(crate) fn every_gesture_is_noted_and_the_year_reads_back_as_its_story() {
    let root = common::Root::new("wrapped");
    let (folder, support) = (root.join("Scans"), root.join("Support"));
    for (name, tone) in [("A.tif", 0.9), ("B.tif", 0.6), ("C.tif", 0.4)] {
        common::linear_tiff(&folder.join(name), 96, 64, tone);
    }
    let lighting = oni_core::parameters::all().iter().position(|r| r.id == "levelsGlobal").unwrap();
    let mut parameters = std::collections::BTreeSet::new();
    parameters.insert("levelsGlobal".to_string());
    let preset = oni_core::presets::Preset::new("Portra warm", parameters, oni_core::presets::Tint::Amber, Default::default());
    oni_core::presets::save(&preset, &oni_core::presets::directory(&support)).unwrap();
    let Some(app) = common::launch(root).show() else { return };
    let common::App { rig, shell, .. } = &app;
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    app.read(&folder);
    let journal = Journal::in_support(&support);
    let kinds = || journal.read().0.iter().map(|r| r.kind).collect::<Vec<_>>();
    let a = oni_library::paths::canonical(&folder.join("A.tif"));

    // Opened, then one record for the one row an Exposure drag moved, written once the hand rests.
    let lib = w.global::<Library>();
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A on the canvas", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    assert_eq!(kinds(), [Kind::Opened]);
    let g = w.global::<Store>();
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
    ctl.act(oni_app::Act::MedalGold);
    ctl.act(oni_app::Act::Copy);
    w.global::<oni_app::Presets>().invoke_save_confirmed();
    ctl.act(oni_app::Act::Paste);
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

    // Show My Year on Film: the strip's first frame is the title, then one scene a frame.
    let wrapped = w.global::<Wrapped>();
    let year = oni_library::wrapped::today().0;
    ctl.show_wrapped(year);
    rig.until("the summary", Duration::from_secs(20), || wrapped.get_shown());
    wrapped.set_playing(false);
    assert_eq!((wrapped.get_index(), wrapped.get_year().as_str()), (0, year.to_string().as_str()));
    // Laid by a first frame, then run in.
    rig.shot("wrapped-title");
    rig.settle();
    println!("shot {}", rig.shot("wrapped-title").display());
    let story = eyebrows(&wrapped);
    let mine = format!("MY {year} ON FILM");
    assert_eq!(
        story,
        ["", "THIS YEAR", "YOUR BUSIEST MONTH", "THE ONLY THING I SEE IS GOLD", "YOUR MOST EDITED PHOTO", "YOUR HANDS KNEW THE WAY", "YOUR SIGNATURE LOOK", "YOUR FILM PERSONALITY", mine.as_str(), "See you next year"]
    );
    let scene = |eyebrow: &str| {
        let scenes = wrapped.get_scenes();
        (0..scenes.row_count()).filter_map(|i| scenes.row_data(i)).find(|c| c.eyebrow == eyebrow).unwrap()
    };
    let lines = |eyebrow: &str| scene(eyebrow).lines.iter().map(|l| (l.label.to_string(), l.value.to_string())).collect::<Vec<_>>();
    assert_eq!((scene("THIS YEAR").count, scene("THIS YEAR").caption.as_str(), scene("THIS YEAR").note.as_str()), (3, "frames brought to light", "3 colour negatives"));
    assert_eq!(scene("YOUR BUSIEST MONTH").bars.row_count(), 12);
    assert_eq!((scene("YOUR MOST EDITED PHOTO").photos.row_count(), scene("THE ONLY THING I SEE IS GOLD").photos.row_count()), (1, 1));
    assert!(lines("YOUR HANDS KNEW THE WAY").iter().any(|(label, _)| label == "Lighting"));
    assert_eq!(scene("YOUR SIGNATURE LOOK").figure.as_str(), "Portra warm");
    assert!(lines("YOUR SIGNATURE LOOK").contains(&("Balance".to_string(), "pressed 1 times".to_string())));
    assert_eq!(lines(&mine).first(), Some(&("Frames".to_string(), "3".to_string())));
    assert_eq!(wrapped.invoke_counted(1_284).as_str(), "1,284");
    rig.key(" ");
    assert_eq!(wrapped.get_index(), 1);
    rig.settle();
    println!("shot {}", rig.shot("wrapped-frames").display());
    rig.key(&key(Key::RightArrow));
    rig.key(&key(Key::RightArrow));
    rig.key(&key(Key::LeftArrow));
    assert_eq!(wrapped.get_index(), 2);
    rig.settle();
    println!("shot {}", rig.shot("wrapped-month").display());
    // The film advances by itself after a scene's hold, and only while it plays.
    rig.advance(Duration::from_secs(8));
    assert_eq!(wrapped.get_index(), 2, "twin: held while it does not play");
    wrapped.set_playing(true);
    rig.pump();
    rig.advance(Duration::from_millis(3_000));
    assert_eq!(wrapped.get_index(), 2, "twin: not before the hold is over");
    rig.advance(Duration::from_millis(4_500));
    assert_eq!(wrapped.get_index(), 3);
    wrapped.set_playing(false);

    // Every scene saved as a card: a 9 by 16 JPEG of its frame alone, the story left where it stood.
    let cards = app.root.join("Cards");
    std::fs::create_dir_all(&cards).unwrap();
    let saved = ctl.save_wrapped_into(&cards, true).unwrap();
    assert_eq!(saved.len(), story.len());
    assert_eq!(saved[0].file_name().unwrap().to_string_lossy(), format!("ONI Wrapped {year} - 01.jpg"));
    assert_eq!((wrapped.get_index(), wrapped.get_still()), (3, false));
    let size = w.window().size();
    for path in &saved {
        let mut decoder = jpeg_decoder::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
        let pixels = decoder.decode().unwrap();
        let info = decoder.info().unwrap();
        assert_eq!(u32::from(info.width) * 16, u32::from(info.height) * 9, "9 by 16");
        assert!(u32::from(info.height) == size.height / 16 * 16 || u32::from(info.width) == size.width / 9 * 9, "as large as the window holds");
        // The frame's ground fills the card to its corners; the black around the strip is not in it.
        let corner = &pixels[..3];
        assert!(corner.iter().any(|c| *c > 12), "{corner:?}");
    }
    for (i, path) in saved.iter().enumerate() {
        std::fs::copy(path, rig.shots.join(format!("wrapped-card-{:02}.jpg", i + 1))).unwrap();
    }
    let one = ctl.save_wrapped_into(&cards, false).unwrap();
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].file_name().unwrap().to_string_lossy(), format!("ONI Wrapped {year} - 04 (2).jpg"), "twin: the card saved before is not replaced");

    // Past the last scene it closes, as Esc does.
    for _ in 0..story.len() {
        wrapped.invoke_forward();
    }
    assert!(!wrapped.get_shown());
    ctl.act(oni_app::Act::Wrapped);
    rig.until("the summary again", Duration::from_secs(20), || wrapped.get_shown());
    wrapped.set_playing(false);
    // Drawn once, as a person sees it before pressing: the view takes the keyboard as it is laid.
    rig.shot("wrapped-again");
    assert!(!shell.window.get_keys_focused(), "the summary holds the keyboard");
    rig.key(&key(Key::Escape));
    assert!(!wrapped.get_shown(), "Esc leaves");

    // A photo in the ONI Trash counts for nothing: A held the gold and every gesture. Trashed from the
    // gallery, as the editor would go on to open B.
    ctl.act(oni_app::Act::ToggleMode);
    rig.pump();
    ctl.act(oni_app::Act::Remove);
    rig.landed(&ctl);
    rig.settle();
    ctl.show_wrapped(year);
    rig.until("the summary without A", Duration::from_secs(20), || wrapped.get_shown());
    let kept = eyebrows(&wrapped);
    assert!(!kept.contains(&"THE ONLY THING I SEE IS GOLD".to_string()) && !kept.contains(&"YOUR MOST EDITED PHOTO".to_string()), "{kept:?}");
    assert_eq!(scene("THIS YEAR").count, 2);
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
    assert!(w.global::<oni_app::App>().get_gallery());
    lib.invoke_tile_clicked(0, false, false);
    ctl.act(oni_app::Act::NewRoll);
    rig.roll_confirmed(&ctl, w);
    rig.until("the roll noted", Duration::from_secs(10), || kinds().contains(&Kind::Rolled));
    assert_eq!(journal.read().0.iter().find(|r| r.kind == Kind::Rolled).unwrap().value, 1);
}
