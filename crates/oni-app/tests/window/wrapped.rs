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
    let told = ["", "THIS YEAR", "YOUR BUSIEST MONTH", "THE ONLY THING I SEE IS GOLD", "YOUR MOST EDITED PHOTO", "YOUR HANDS KNEW THE WAY"];
    assert_eq!(story[..6], told);
    assert_eq!(story[6..], ["YOUR SIGNATURE LOOK", "YOUR FILM PERSONALITY", "MY YEAR ON FILM", ""]);
    let scene = |eyebrow: &str| {
        let scenes = wrapped.get_scenes();
        (0..scenes.row_count()).filter_map(|i| scenes.row_data(i)).find(|c| c.eyebrow == eyebrow).unwrap()
    };
    let lines = |eyebrow: &str| scene(eyebrow).lines.iter().map(|l| (l.label.to_string(), l.value.to_string())).collect::<Vec<_>>();
    let flat = |eyebrow: &str| scene(eyebrow).figure.replace('\n', " ");
    let this_year = scene("THIS YEAR");
    assert_eq!((this_year.count, this_year.headline.as_str(), this_year.note.as_str()), (3, "FRAMES BROUGHT TO LIGHT", "3 colour negatives"));
    assert!(scene("YOUR BUSIEST MONTH").headline.contains("FRAMES"), "the month's frames are its headline");
    assert_eq!((scene("YOUR MOST EDITED PHOTO").photos.row_count(), scene("THE ONLY THING I SEE IS GOLD").photos.row_count()), (1, 1));
    assert!(flat("YOUR HANDS KNEW THE WAY").contains("LIGHTING") && scene("YOUR HANDS KNEW THE WAY").headline.starts_with("MOVED "));
    assert_eq!(flat("YOUR SIGNATURE LOOK"), "PORTRA WARM");
    assert!(scene("YOUR SIGNATURE LOOK").note.contains("Balance pressed 1 times"));
    assert_eq!(lines("MY YEAR ON FILM").first(), Some(&("FRAMES".to_string(), "3".to_string())));
    assert_eq!(wrapped.invoke_counted(1_284).as_str(), "1,284");
    rig.key(&key(Key::Return));
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
    rig.key(" ");
    rig.advance(Duration::from_secs(8));
    assert_eq!(wrapped.get_index(), 2, "twin: Space pauses the film");
    rig.key(" ");
    rig.advance(Duration::from_millis(3_000));
    assert_eq!(wrapped.get_index(), 2, "twin: not before the hold is over");
    rig.advance(Duration::from_millis(4_500));
    assert_eq!(wrapped.get_index(), 3);
    wrapped.set_playing(false);

    // Every scene saved as a card: a 1080 by 1920 JPEG of its frame in its piece of the strip, taken a
    // window's worth at a time, the story left where it stood.
    let cards = app.root.join("Cards");
    // One card a turn of the loop: the window stays live while they are taken, encoded and written.
    let written = |scene: Option<usize>| {
        let (tx, rx) = std::sync::mpsc::channel();
        ctl.save_wrapped_into(&cards, scene, move |written| tx.send(written).unwrap());
        let got = std::cell::RefCell::new(None);
        rig.until("the cards written", Duration::from_secs(120), || {
            let mut got = got.borrow_mut();
            *got = got.take().or_else(|| rx.try_recv().ok());
            got.is_some()
        });
        got.into_inner().unwrap().unwrap()
    };
    let saved = written(None);
    assert_eq!(saved.len(), story.len());
    assert_eq!(saved[0].file_name().unwrap().to_string_lossy(), format!("ONI Wrapped {year} - 01.jpg"));
    assert_eq!((wrapped.get_index(), wrapped.get_still()), (3, false));
    let size = w.window().size();
    assert!(size.height < 1920, "the card is taller than the window: it is laid from several pictures of it");
    assert!(wrapped.get_view_y() > 0.0, "the window holds its menu bar above the view, which no card shows");
    for (i, path) in saved.iter().enumerate() {
        let mut decoder = jpeg_decoder::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
        let pixels = decoder.decode().unwrap();
        let info = decoder.info().unwrap();
        assert_eq!((info.width, info.height), (1080, 1920), "a story's picture");
        let at = |x: usize, y: usize| &pixels[(y * 1080 + x) * 3..(y * 1080 + x) * 3 + 3];
        // The film's edge is lit at the card's head by the scene's photo and darker at its foot, under the
        // veil: no window's worth of black above.
        let lit = |p: &[u8]| p.iter().map(|c| u32::from(*c)).sum::<u32>();
        let (head, foot) = (lit(at(6, 8)), lit(at(6, 1912)));
        assert!(head > 30 && head > foot, "card {}: head {head}, foot {foot}", i + 1);
        // The brand's band lies inside a story's safe area, above its last 20 %: the mark's ink is there.
        let band = (1440..1530).flat_map(|y| (80..400).map(move |x| (x, y))).filter(|(x, y)| lit(at(*x, *y)) > 450).count();
        assert!(band > 200, "card {}: ONI's mark, {band} pixels of its ink", i + 1);
        std::fs::copy(path, rig.shots.join(format!("wrapped-card-{:02}.jpg", i + 1))).unwrap();
    }
    let one = written(Some(3));
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].file_name().unwrap().to_string_lossy(), format!("ONI Wrapped {year} - 04.jpg"), "saved again, it takes its own place");
    assert_eq!(std::fs::read_dir(&cards).unwrap().count(), story.len(), "twin: no second file beside the first");

    // A card copied is on the pasteboard as a picture any app pastes.
    assert_eq!(ctl.board_picture(), None);
    ctl.copy_wrapped(3);
    rig.until("the card on the pasteboard", Duration::from_secs(60), || ctl.board_picture().is_some());
    assert_eq!(&ctl.board_picture().unwrap()[..4], b"II*\0", "a TIFF");
    assert_eq!(wrapped.get_notice().as_str(), "Card copied: paste it where you post.");

    // Past the last scene every card is laid out to share; back returns to the story, forward leaves.
    for _ in wrapped.get_index() as usize..story.len() {
        wrapped.invoke_forward();
    }
    assert!(wrapped.get_shown() && wrapped.get_sharing());
    assert_eq!(wrapped.get_index() as usize, story.len() - 1);
    rig.settle();
    println!("shot {}", rig.shot("wrapped-share").display());
    rig.key(&key(Key::LeftArrow));
    assert!(!wrapped.get_sharing(), "twin: back from the share screen is the last scene again");
    rig.key(&key(Key::RightArrow));
    assert!(wrapped.get_sharing());
    rig.key(&key(Key::RightArrow));
    assert!(!wrapped.get_shown());
    ctl.act(oni_app::Act::Wrapped);
    rig.until("the summary again", Duration::from_secs(20), || wrapped.get_shown());
    wrapped.set_playing(false);
    assert!(!wrapped.get_sharing(), "it opens on its story");
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

    // December offers the year once, and a year already shown in its December is that offer made; any
    // other month offers nothing.
    shell.offer_wrapped();
    rig.settle();
    assert!(!wrapped.get_shown(), "shown above already, or not December");

    // The gallery's column opens on the year through December and the January after it, which tells the
    // year just ended; a click on it shows the year.
    assert!(w.global::<oni_app::App>().get_gallery());
    let entry = || {
        use i_slint_backend_testing::{AccessibleRole, ElementHandle};
        ElementHandle::find_by_accessible_label(w, "Your year on film").find(|e| e.accessible_role() == Some(AccessibleRole::Button))
    };
    for (today, told) in [((2026, 12), "2026"), ((2027, 1), "2026")] {
        ctl.wrapped_season(today);
        rig.settle();
        assert_eq!(wrapped.get_season().as_str(), told, "{today:?}");
        assert!(entry().is_some(), "{today:?}: the entry heads the gallery's column");
    }
    println!("shot {}", rig.shot("wrapped-entry").display());
    entry().unwrap().invoke_accessible_default_action();
    rig.until("the year shown from the gallery", Duration::from_secs(20), || wrapped.get_shown());
    wrapped.invoke_close();
    rig.settle();
    // Twin: the months on either side have no entry.
    for today in [(2026, 11), (2027, 2)] {
        ctl.wrapped_season(today);
        rig.settle();
        assert!(wrapped.get_season().is_empty() && entry().is_none(), "{today:?}");
    }

    // New Roll from Selection notes the photos it gathered.
    assert!(w.global::<oni_app::App>().get_gallery());
    lib.invoke_tile_clicked(0, false, false);
    ctl.act(oni_app::Act::NewRoll);
    rig.roll_confirmed(&ctl, w);
    rig.until("the roll noted", Duration::from_secs(10), || kinds().contains(&Kind::Rolled));
    assert_eq!(journal.read().0.iter().find(|r| r.kind == Kind::Rolled).unwrap().value, 1);
}
