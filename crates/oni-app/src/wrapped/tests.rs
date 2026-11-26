use super::*;
use oni_library::wrapped::palette::Swatch;
use oni_library::wrapped::{Darkroom, Frames, Palette, Persona};

fn full() -> Year {
    let mut by_month = [0; 12];
    (by_month[2], by_month[6], by_month[10]) = (218, 90, 104);
    let mut by_hour = [0; 24];
    (by_hour[22], by_hour[14]) = (400, 200);
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
        last: Some("/roll/F99.NEF".into()),
        busiest_day: Some((3, 14, 42)),
        paths: vec![("/roll/F00.NEF".into(), "fp0".into()), ("/roll/F99.NEF".into(), "fp99".into())],
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
    Year::new(2026, darkroom, frames, Palette { swatches, warmth: 0.08, chroma: 0.1, sampled: 400 })
}

fn eyebrows(story: &[Scene]) -> Vec<&str> {
    story.iter().map(|s| s.eyebrow.as_str()).collect()
}

fn scene<'a>(story: &'a [Scene], eyebrow: &str) -> &'a Scene {
    story.iter().find(|s| s.eyebrow == eyebrow).unwrap_or_else(|| panic!("no scene {eyebrow}"))
}

#[test]
fn a_full_year_tells_every_scene_in_order() {
    let story = scenes(&full());
    assert_eq!(
        eyebrows(&story),
        [
            "",
            "This year",
            "Your busiest month",
            "Your film of the year",
            "Through the gate",
            "Your faithful companion",
            "The only thing I see is gold",
            "Your most edited photo",
            "Your hands knew the way",
            "Frames rescued",
            "Your signature look",
            "The colours of your year",
            "Time in the darkroom",
            "Out into the world",
            "Your film personality",
            "My 2026 on film",
            "See you next year"
        ]
    );
    assert_eq!((story[0].kind, story[0].figure.as_str()), (Kind::Title, "2026"));
    assert_eq!(story[0].photos, ["/roll/G00.NEF", "/roll/G01.NEF", "/roll/G02.NEF"], "the title's prints are the year's gold");
    let frames = scene(&story, "This year");
    assert_eq!((frames.count, frames.caption.as_str()), (Some(412), "frames brought to light"));
    assert_eq!(frames.note, "300 colour negatives  \u{b7}  60 in black and white  \u{b7}  40 slides  \u{b7}  12 digital photos on film");
    assert_eq!(frames.photos, ["/roll/F00.NEF"]);
    let month = scene(&story, "Your busiest month");
    assert_eq!((month.kind, month.figure.as_str(), month.caption.as_str(), month.lit), (Kind::Bars, "March", "218 frames", Some(2)));
    assert_eq!((month.bars.len(), month.bars[2], month.marks.join("")), (12, 1.0, "JFMAMJJASOND".to_string()));
    assert_eq!(month.note, "March 14 alone holds 42");
    let film = scene(&story, "Your film of the year");
    assert_eq!((film.kind, film.stock.as_deref(), film.caption.as_str()), (Kind::Film, Some("kodak-portra400"), "206 frames, 50 % of your year"));
    assert_eq!(film.figure, film_name("kodak-portra400"));
    assert_eq!((film.lines.len(), film.lines[1].label.as_str(), film.note.as_str()), (2, "no-such-film", "3 films this year"), "a key no file defines is named by itself");
    let longest = &scene(&story, "Through the gate").note;
    assert_eq!(oni_i18n::spoken(longest), "The longest, Lisbon, holds 38 frames");
    assert_ne!(longest, "The longest, Lisbon, holds 38 frames", "twin: the roll's name is marked, so no log would carry it");
    let kit = scene(&story, "Your faithful companion");
    assert_eq!((kit.figure.as_str(), kit.caption.as_str(), kit.lines[0].label.as_str(), kit.lines[1].share), ("Nikon FM2", "310 frames together", "50mm f/1.8", 0.24));
    let gold = scene(&story, "The only thing I see is gold");
    assert_eq!((gold.kind, gold.count, gold.photos.len(), gold.note.as_str()), (Kind::Photos, Some(11), 9, "and 20 silver"));
    let edited = scene(&story, "Your most edited photo");
    assert_eq!((edited.count, edited.photos.as_slice()), (Some(412), ["/roll/M.NEF".to_string()].as_slice()));
    let hands = scene(&story, "Your hands knew the way");
    assert_eq!((hands.count, hands.lines.len(), hands.lines[0].label.clone()), (Some(9_840), 3, row_name(0)));
    // The white balance's two rows are one name, their gestures counted together.
    let rows = oni_core::parameters::all();
    let rank = |id: &str| rows.iter().position(|r| r.id == id).unwrap();
    let merged = named_rows(&[(rank("levelsGlobal"), 5), (rank("gains"), 3), (rank("levelsPerChannel"), 4)]);
    assert_eq!(merged, [("White balance".to_string(), 7), ("Lighting".to_string(), 5)]);
    assert_ne!(row_name(rank("gains")), "gains", "twin: the row's own id is not shown");
    let dust = scene(&story, "Frames rescued");
    assert_eq!((dust.count, dust.note.as_str()), (Some(1_640), "on 120 frames; the dustiest took 164"));
    let look = scene(&story, "Your signature look");
    assert_eq!(look.figure, "Portra warm");
    let labels: Vec<&str> = look.lines.iter().map(|l| l.label.as_str()).collect();
    assert_eq!(labels, ["Preset", "Paper", "Printed on paper", "Grain", "Cropped", "Straightened", "Balance", "Settings pasted"], "no line for the halation no photo has");
    assert_eq!(oni_i18n::spoken(&look.lines[0].value), "Portra warm, laid 212 times");
    let colours = scene(&story, "The colours of your year");
    assert_eq!((colours.figure.as_str(), colours.swatches.len(), colours.caption.as_str()), ("A warm year", 2, "read off 400 of your photos"));
    let clock = scene(&story, "Time in the darkroom");
    assert_eq!((clock.count, clock.unit.as_str(), clock.bars.len(), clock.lit), (Some(162), " h", 24, Some(22)));
    assert_eq!(clock.note, "96 days at the desk  \u{b7}  9 in a row  \u{b7}  mostly around 22:00");
    assert_eq!(scene(&story, "Out into the world").count, Some(1_284));
    let persona = scene(&story, "Your film personality");
    assert_eq!((persona.figure.as_str(), persona.note.as_str()), ("The Night Developer", "67 % of your gestures between 10 pm and 5 am"));
    let summary = scene(&story, "My 2026 on film");
    assert_eq!((summary.kind, summary.figure.as_str(), summary.photos.len()), (Kind::Summary, "The Night Developer", 4));
    let told: Vec<(&str, &str)> = summary.lines.iter().map(|l| (l.label.as_str(), l.value.as_str())).collect();
    assert_eq!(told[0], ("Frames", "412"));
    assert!(told.contains(&("In the darkroom", "162 h")) && told.contains(&("Dust healed", "1,640")) && told.contains(&("Camera", "Nikon FM2")));
    assert_eq!(story.last().map(|s| (s.kind, s.caption.as_str())), Some((Kind::Farewell, "on film.")));
}

#[test]
fn an_empty_year_is_an_invitation_and_a_thin_one_a_short_story() {
    let empty = scenes(&Year { year: 2026, ..Default::default() });
    assert_eq!(eyebrows(&empty), ["", "A fresh roll", "See you next year"]);
    assert_eq!(empty[1].caption, "Your year on film is still unexposed.");
    assert_eq!(empty[0].note, "Read off this computer alone. Nothing was sent anywhere.");
    // Twin: one frame is a year, told without the scenes it has nothing for.
    let frames = Frames { count: 1, negatives: 1, by_month: [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0], first: Some("/a".into()), last: Some("/a".into()), ..Default::default() };
    let thin = scenes(&Year::new(2026, Darkroom::default(), frames, Palette::default()));
    assert_eq!(eyebrows(&thin), ["", "This year", "Your busiest month", "Your film personality", "My 2026 on film", "See you next year"]);
    assert_eq!((thin[1].caption.as_str(), thin[1].note.as_str()), ("frame brought to light", "1 colour negative"));
    assert_eq!((thin[2].figure.as_str(), thin[2].caption.as_str(), thin[2].note.as_str()), ("May", "1 frame", ""), "a day holding one frame is not told");
    assert_eq!(thin[3].figure, "The Film Photographer");
    // A year of gestures on photos dated elsewhere still counts the photos opened.
    let hands = Darkroom { photos: 3, gestures: 2, hours: 0.9, ..Default::default() };
    let worked = scenes(&Year::new(2026, hands, Frames::default(), Palette::default()));
    assert_eq!((worked[1].count, worked[1].caption.as_str()), (Some(3), "photos opened in the darkroom"));
    assert!(!eyebrows(&worked).contains(&"Time in the darkroom"), "under an hour the darkroom is not told");
}

#[test]
fn figures_group_their_thousands_and_rows_are_named_by_their_box() {
    assert_eq!([figure(0), figure(999), figure(1_284), figure(1_234_567)], ["0", "999", "1,284", "1,234,567"]);
    assert_eq!(row_name(usize::MAX), "Settings");
    let lighting = oni_core::parameters::all().iter().position(|r| r.id == "levelsGlobal").unwrap();
    assert_eq!(row_name(lighting), "Lighting");
    // Twin: the row's own id is not what is shown.
    assert_ne!(row_name(lighting), "levelsGlobal");
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
    assert_eq!(persona_words(&loyal).2, format!("62 % of your frames on {}", film_name("kodak-portra400")));
    assert_eq!(scene(&scenes(&loyal), "Your film personality").lines[0].label, "The Regular");
}
