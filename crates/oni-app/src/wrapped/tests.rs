use super::*;
use oni_library::wrapped::palette::Swatch;
use oni_library::wrapped::{Darkroom, Frames, Palette, Persona, Picks, Trait};

fn full() -> Year {
    let mut by_month = [0; 12];
    (by_month[2], by_month[6], by_month[10]) = (218, 90, 104);
    let mut by_hour = [0; 24];
    (by_hour[22], by_hour[14]) = (400, 200);
    let paths: Vec<(String, String)> = (0..48).map(|i| (format!("/roll/F{i:02}.NEF"), format!("fp{i}"))).collect();
    let frames = Frames {
        count: 412,
        by_month,
        negatives: 300,
        monochromes: 60,
        slides: 40,
        digitals: 12,
        rolls: 14,
        longest_roll: Some(("Lisbon".into(), 38)),
        stocks: vec![("kodak-portra400".into(), 206), ("ilford-hp5".into(), 60), ("no-such-film".into(), 9)],
        papers: vec![("oni-standard".into(), 30)],
        bodies: vec![("Nikon FM2".into(), 310)],
        lenses: vec![("50mm f/1.8".into(), 250), ("28mm f/2.8".into(), 60)],
        gold: (0..11).map(|i| format!("/roll/G{i:02}.NEF")).collect(),
        silver: 20,
        bronze: 0,
        dust: 1_640,
        dusted: 120,
        dustiest: Some(("/roll/D.NEF".into(), 164)),
        cropped: 300,
        straightened: 44,
        grained: 12,
        haloed: 0,
        first: Some("/roll/F00.NEF".into()),
        last: Some("/roll/F47.NEF".into()),
        busiest_day: Some((3, 14, 142)),
        paths,
        picks: Picks { month: Some("/roll/March.NEF".into()), stock: Some("/roll/Portra.NEF".into()), roll: Some("/roll/Lisbon.NEF".into()), body: Some("/roll/FM2.NEF".into()) },
    };
    let darkroom = Darkroom {
        photos: 380,
        gestures: 9_840,
        most_edited: Some(("/roll/M.NEF".into(), 412)),
        favourite_rows: vec![(0, 180), (1, 120), (2, 60)],
        favourite_preset: Some(("Portra warm".into(), 212)),
        balanced: 318,
        pasted: 640,
        exported: 1_284,
        rolls: 3,
        measured: 400,
        hours: 162.4,
        longest_sitting: 3.0,
        by_hour,
        days: 96,
        streak: 9,
        busiest_day: Some((3, 14, 400)),
    };
    let swatches = vec![Swatch { rgb: [230, 150, 60], share: 0.6 }, Swatch { rgb: [50, 110, 220], share: 0.4 }];
    let palette = Palette { swatches, warmth: 0.08, chroma: 0.1, sampled: 400, picks: vec![1, 0] };
    Year::new(2026, darkroom, frames, palette, &["/roll/blue.NEF".to_string(), "/roll/orange.NEF".to_string()])
}

fn eyebrows(story: &[Scene]) -> Vec<&str> {
    story.iter().map(|s| s.eyebrow.as_str()).collect()
}

fn scene<'a>(story: &'a [Scene], eyebrow: &str) -> &'a Scene {
    story.iter().find(|s| s.eyebrow == eyebrow).unwrap_or_else(|| panic!("no scene {eyebrow}"))
}

#[test]
fn a_full_year_tells_every_scene_in_order_each_on_a_photo_of_its_own() {
    let story = scenes(&full());
    assert_eq!(
        eyebrows(&story),
        [
            "",
            "THIS YEAR",
            "YOUR BUSIEST MONTH",
            "YOUR FILM OF THE YEAR",
            "THROUGH THE GATE",
            "YOUR PARTNER IN CRIME",
            "THE ONLY THING I SEE IS GOLD",
            "YOUR MOST EDITED PHOTO",
            "YOUR HANDS KNEW THE WAY",
            "FRAMES RESCUED",
            "YOUR SIGNATURE LOOK",
            "THE COLOURS OF YOUR YEAR",
            "TIME IN THE DARKROOM",
            "OUT INTO THE WORLD",
            "YOUR FILM PERSONALITY",
            "MY YEAR ON FILM",
            ""
        ]
    );
    // The owner's rule: a photo of the library on every frame.
    assert!(story.iter().all(|s| !s.photos.is_empty()), "{:?}", story.iter().filter(|s| s.photos.is_empty()).map(|s| &s.eyebrow).collect::<Vec<_>>());
    // A scene with one photo shows one no scene before it showed, while the year holds others.
    let heroes: Vec<&String> = story.iter().filter(|s| s.kind == Kind::Hero).map(|s| &s.photos[0]).collect();
    let distinct: std::collections::HashSet<&String> = heroes.iter().copied().collect();
    assert_eq!(distinct.len(), heroes.len(), "{heroes:?}");
    assert_eq!((story[0].kind, story[0].figure.as_str(), story[0].headline.as_str(), story[0].count), (Kind::Title, "2026", "YOUR YEAR\nON FILM", None));
    assert_eq!(story[0].photos, ["/roll/G00.NEF"], "the title's photo is the year's first gold");
    let frames = scene(&story, "THIS YEAR");
    assert_eq!((frames.count, frames.figure.as_str(), frames.headline.as_str()), (Some(412), "412", "FRAMES BROUGHT TO LIGHT"));
    assert_eq!(frames.note, "300 colour negatives  \u{b7}  60 in black and white  \u{b7}  40 slides  \u{b7}  12 digital photos on film");
    assert_eq!(frames.photos, ["/roll/F00.NEF"], "the year's first frame");
    let month = scene(&story, "YOUR BUSIEST MONTH");
    assert_eq!((month.figure.as_str(), month.headline.as_str(), month.note.as_str()), ("MARCH", "218 FRAMES", "March 14 alone holds 142. Did you sleep?"));
    assert_eq!(month.photos, ["/roll/March.NEF"], "a photo of that month");
    let film = scene(&story, "YOUR FILM OF THE YEAR");
    assert_eq!(film.figure.replace('\n', " "), upper(&film_name("kodak-portra400")));
    assert_eq!((film.headline.as_str(), film.photos[0].as_str()), ("206 FRAMES, 50 % OF YOUR YEAR", "/roll/Portra.NEF"));
    assert_eq!(film.note, format!("Ahead of {}, no-such-film.", film_name("ilford-hp5")), "a key no file defines is named by itself");
    let rolls = scene(&story, "THROUGH THE GATE");
    assert_eq!((rolls.count, rolls.headline.as_str(), rolls.photos[0].as_str()), (Some(14), "ROLLS", "/roll/Lisbon.NEF"));
    let longest = &rolls.note;
    assert_eq!(oni_i18n::spoken(longest), "The longest, Lisbon, holds 38 frames");
    assert_ne!(longest, "The longest, Lisbon, holds 38 frames", "twin: the roll's name is marked, so no log would carry it");
    let kit = scene(&story, "YOUR PARTNER IN CRIME");
    assert_eq!((kit.figure.replace('\n', " ").as_str(), kit.headline.as_str(), kit.note.as_str()), ("NIKON FM2", "310 FRAMES TOGETHER", "Mostly through the 50mm f/1.8."));
    let gold = scene(&story, "THE ONLY THING I SEE IS GOLD");
    assert_eq!((gold.kind, gold.count, gold.photos.len(), gold.note.as_str()), (Kind::Mosaic, Some(11), GOLD_SHOWN, "and 20 silver"));
    let edited = scene(&story, "YOUR MOST EDITED PHOTO");
    assert_eq!(
        (edited.kind, edited.count, edited.photos.as_slice(), edited.note.as_str()),
        (Kind::Print, Some(412), ["/roll/M.NEF".to_string()].as_slice(), "Overcooked? Slow-cooked."),
        "the photo it tells is laid whole"
    );
    let hands = scene(&story, "YOUR HANDS KNEW THE WAY");
    assert_eq!((hands.figure.replace('\n', " "), hands.headline.as_str()), (upper(&row_name(0)), "MOVED 180 TIMES"));
    assert_eq!(hands.note, format!("Then {} and {}.", row_name(1), row_name(2)));
    let dust = scene(&story, "FRAMES RESCUED");
    assert_eq!((dust.count, dust.note.as_str(), dust.photos[0].as_str()), (Some(1_640), "on 120 frames; the dustiest took 164. Clean that scanner.", "/roll/D.NEF"));
    let look = scene(&story, "YOUR SIGNATURE LOOK");
    assert_eq!((look.figure.replace('\n', " ").as_str(), look.headline.as_str()), ("PORTRA WARM", "LAID ON 212 PHOTOS"));
    assert_eq!(look.note, "grain on 12 photos  \u{b7}  300 photos cropped  \u{b7}  44 photos straightened", "no word of the halation no photo has, and three habits at most");
    let colours = scene(&story, "THE COLOURS OF YOUR YEAR");
    assert_eq!((colours.kind, colours.figure.replace('\n', " ").as_str(), colours.swatches.len()), (Kind::Palette, "A WARM YEAR", 2));
    assert_eq!(colours.photos, ["/roll/orange.NEF", "/roll/blue.NEF"], "each colour's slice is the photo nearest it");
    let clock = scene(&story, "TIME IN THE DARKROOM");
    assert_eq!((clock.count, clock.unit.as_str(), clock.figure.as_str()), (Some(162), " H", "162 H"));
    assert_eq!(clock.note, "96 days at the desk  \u{b7}  9 in a row  \u{b7}  mostly around 22:00. We don't judge.");
    assert_eq!((scene(&story, "OUT INTO THE WORLD").count, scene(&story, "OUT INTO THE WORLD").note.as_str()), (Some(1_284), "Now post them."));
    let persona = scene(&story, "YOUR FILM PERSONALITY");
    assert_eq!((persona.figure.replace('\n', " ").as_str(), persona.headline.as_str()), ("THE NIGHT DEVELOPER", "You work when the house sleeps."));
    let touch = trait_name(full().persona.also[0]);
    assert_eq!(persona.note, format!("67 % of your gestures between 10 pm and 5 am. With a touch of {touch}."));
    let summary = scene(&story, "MY YEAR ON FILM");
    assert_eq!((summary.kind, summary.figure.as_str(), summary.headline.as_str(), summary.photos.len()), (Kind::Summary, "2026", "THE NIGHT DEVELOPER", 1));
    let told: Vec<(&str, &str)> = summary.lines.iter().map(|l| (l.label.as_str(), l.value.as_str())).collect();
    assert_eq!(told, [("FRAMES", "412"), ("ROLLS", "14"), ("FILM", film_name("kodak-portra400").to_uppercase().as_str()), ("IN THE DARKROOM", "162 H")]);
    let last = story.last().unwrap();
    assert_eq!(
        (last.kind, last.figure.replace('\n', " ").as_str(), last.headline.as_str(), last.photos.as_slice()),
        (Kind::Hero, "SEE YOU NEXT YEAR", "ON FILM, OBVIOUSLY.", ["/roll/F47.NEF".to_string()].as_slice())
    );
    // Every figure is set to hold in its frame.
    assert!(story.iter().all(|s| s.size > 0.0 && s.size <= staging::MOST), "{:?}", story.iter().map(|s| s.size).collect::<Vec<_>>());
}

#[test]
fn an_empty_year_is_an_invitation_and_a_thin_one_a_short_story() {
    let empty = scenes(&Year { year: 2026, ..Default::default() });
    assert_eq!(eyebrows(&empty), ["", "A FRESH ROLL"]);
    assert_eq!((empty[1].figure.replace('\n', " ").as_str(), empty[1].headline.as_str()), ("FRAME 0", "YOUR YEAR ON FILM IS STILL UNEXPOSED."));
    assert_eq!(empty[0].note, "Read off this computer alone. Nothing was sent anywhere.");
    // Twin: one frame is a year, told without the scenes it has nothing for, that frame on each of them.
    let frames = Frames {
        count: 1,
        negatives: 1,
        by_month: [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
        first: Some("/a".into()),
        last: Some("/a".into()),
        paths: vec![("/a".into(), "fp".into())],
        picks: Picks { month: Some("/a".into()), ..Default::default() },
        ..Default::default()
    };
    let thin = scenes(&Year::new(2026, Darkroom::default(), frames, Palette::default(), &[]));
    assert_eq!(eyebrows(&thin), ["", "THIS YEAR", "YOUR BUSIEST MONTH", "YOUR FILM PERSONALITY", "MY YEAR ON FILM", ""]);
    assert!(thin.iter().all(|s| s.photos.first().map(String::as_str) == Some("/a")), "the year's one photo stands on every frame");
    assert_eq!((thin[1].headline.as_str(), thin[1].note.as_str()), ("FRAME BROUGHT TO LIGHT", "1 colour negative"));
    assert_eq!((thin[2].figure.as_str(), thin[2].headline.as_str(), thin[2].note.as_str()), ("MAY", "1 FRAME", ""), "a day holding one frame is not told");
    assert_eq!(thin[3].figure.replace('\n', " "), "THE FILM PHOTOGRAPHER");
    // A year of gestures on photos dated elsewhere still counts the photos opened, on the photo they were made on.
    let hands = Darkroom { photos: 3, gestures: 2, hours: 0.9, most_edited: Some(("/m".into(), 2)), ..Default::default() };
    let worked = scenes(&Year::new(2026, hands, Frames::default(), Palette::default(), &[]));
    assert_eq!((worked[1].count, worked[1].headline.as_str(), worked[1].photos.as_slice()), (Some(3), "PHOTOS OPENED IN THE DARKROOM", ["/m".to_string()].as_slice()));
    assert!(!eyebrows(&worked).contains(&"TIME IN THE DARKROOM"), "under an hour the darkroom is not told");
}

#[test]
fn figures_group_their_thousands_and_rows_are_named_by_their_box() {
    assert_eq!([figure(0), figure(999), figure(1_284), figure(1_234_567)], ["0", "999", "1,284", "1,234,567"]);
    assert_eq!(row_name(usize::MAX), "Settings");
    let rows = oni_core::parameters::all();
    let rank = |id: &str| rows.iter().position(|r| r.id == id).unwrap();
    assert_eq!(row_name(rank("levelsGlobal")), "Lighting");
    // The white balance's two rows are one name, their gestures counted together.
    let merged = named_rows(&[(rank("levelsGlobal"), 5), (rank("gains"), 3), (rank("levelsPerChannel"), 4)]);
    assert_eq!(merged, [("White balance".to_string(), 7), ("Lighting".to_string(), 5)]);
    assert_ne!(row_name(rank("gains")), "gains", "twin: the row's own id is not shown");
}

#[test]
fn every_trait_has_its_name_its_line_and_its_reason() {
    let mut seen = std::collections::HashSet::new();
    for kind in [
        Trait::NightDeveloper,
        Trait::SilverSoul,
        Trait::Projectionist,
        Trait::TimeTraveller,
        Trait::Loyalist,
        Trait::Explorer,
        Trait::DustHunter,
        Trait::Curator,
        Trait::Tinkerer,
        Trait::Purist,
        Trait::Marathoner,
        Trait::FilmPhotographer,
    ] {
        let year = Year { persona: Persona { kind, measure: 0.5, also: Vec::new() }, ..full() };
        let (name, line, reason) = persona_words(&year);
        assert_eq!(name, trait_name(kind));
        assert!(seen.insert(name) && !line.is_empty(), "{kind:?}");
        assert_eq!(reason.is_empty(), kind == Trait::FilmPhotographer, "{kind:?}: {reason}");
        assert!(!reason.contains('{'), "{kind:?}: {reason}");
    }
    let loyal = Year { persona: Persona { kind: Trait::Loyalist, measure: 0.62, also: vec![Trait::Marathoner] }, ..full() };
    assert_eq!(scene(&scenes(&loyal), "YOUR FILM PERSONALITY").note, format!("62 % of your frames on {}. With a touch of The Regular.", film_name("kodak-portra400")));
}

#[test]
fn a_camera_or_a_paper_on_a_handful_of_frames_is_no_headline() {
    let mut few = full();
    few.frames.bodies = vec![("Leica M6".into(), 1)];
    few.frames.papers = vec![("oni-standard".into(), 3)];
    few.frames.grained = 3;
    few.darkroom.favourite_preset = None;
    let story = scenes(&few);
    assert!(!eyebrows(&story).contains(&"YOUR PARTNER IN CRIME") && !eyebrows(&story).contains(&"YOUR SIGNATURE LOOK"));
    // Twin: the full year's camera, on 310 frames, and its preset are told; grain on enough photos is a look of its own.
    assert!(eyebrows(&scenes(&full())).contains(&"YOUR PARTNER IN CRIME"));
    few.frames.grained = 161;
    let look = scenes(&few);
    assert_eq!((scene(&look, "YOUR SIGNATURE LOOK").figure.as_str(), scene(&look, "YOUR SIGNATURE LOOK").headline.as_str()), ("GRAIN", "ON 161 PHOTOS"));
}
