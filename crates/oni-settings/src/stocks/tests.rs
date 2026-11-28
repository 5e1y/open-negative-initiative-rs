use super::file::{self, File};
use super::*;
use crate::{DyeSet, PaperStock, VirtualStock};
use std::cmp::Ordering;

fn shipped_json() -> impl Iterator<Item = (&'static str, &'static [u8])> {
    shipped_files().iter().copied().filter(|(n, _)| n.ends_with(".json"))
}

fn value(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).unwrap()
}

/// A person's file of `json`, named `name`.
fn own(name: &str, json: &str) -> Result<Loaded, String> {
    let file = file::read(json.as_bytes())?;
    Ok(Loaded { name: name.into(), origin: Origin::Own(name.into()), file, hash: hash(json.as_bytes()) })
}

fn with_own(extra: Vec<Loaded>) -> (Catalogue, Vec<Refusal>) {
    let (mut files, canisters) = shipped();
    files.extend(extra);
    Catalogue::build(files, canisters)
}

/// The shipped file `name` with `edit` laid on its JSON.
fn edited(name: &str, edit: impl FnOnce(&mut serde_json::Value)) -> String {
    let bytes = shipped_files().iter().find(|(n, _)| *n == name).map(|(_, b)| *b).expect("shipped");
    let mut v = value(bytes);
    edit(&mut v);
    serde_json::to_string(&v).unwrap()
}

/// Every shipped file reads, and written back holds every field it read: nothing a file says is lost.
#[test]
fn every_shipped_file_reads_and_round_trips() {
    let mut count = 0;
    for (name, bytes) in shipped_json() {
        let file = file::read(bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(value(file.to_json().as_bytes()), value(bytes), "{name} round-trips");
        assert_eq!(format!("{}.json", file.key()), name, "a file is named by its key");
        count += 1;
    }
    assert_eq!(count, 111, "103 film stocks and 8 papers");
    // Twin: a field dropped on the way is seen.
    let file = file::read(shipped_json().next().unwrap().1).unwrap();
    let mut lost = value(file.to_json().as_bytes());
    lost.as_object_mut().unwrap().remove("brand");
    assert_ne!(lost, value(shipped_json().next().unwrap().1));
}

/// A file missing a field, holding one unknown, of a later format or keyed with a space is refused with why; the
/// shipped one it was made from reads.
#[test]
fn a_broken_file_is_refused_with_why() {
    let name = "kodak-gold200.json";
    let cases: [(&str, Box<dyn FnOnce(&mut serde_json::Value)>); 6] = [
        ("missing field `name`", Box::new(|v| drop(v.as_object_mut().unwrap().remove("name")))),
        ("unknown field `isos`", Box::new(|v| drop(v.as_object_mut().unwrap().insert("isos".into(), 200.into())))),
        ("format 2", Box::new(|v| v["format"] = 2.into())),
        ("letters, digits and hyphens", Box::new(|v| v["key"] = "kodak gold".into())),
        ("unknown variant", Box::new(|v| v["type"] = "colour negative".into())),
        ("not an SVG", Box::new(|v| v["canister"] = "../kodak-gold.svg".into())),
    ];
    for (why, edit) in cases {
        let err = file::read(edited(name, edit).as_bytes()).err().unwrap_or_default();
        assert!(err.contains(why), "{why}: {err}");
    }
    assert!(file::read(b"{\"oni\":\"film\"").is_err(), "a file cut short");
    // Twin: the file unedited reads.
    assert!(file::read(edited(name, |_| {}).as_bytes()).is_ok());
}

/// The shipped catalogue holds every stock, film, dye set and paper, the defaults among them, each key read back.
#[test]
fn the_shipped_catalogue_holds_every_stock_and_its_defaults() {
    let (files, canisters) = shipped();
    assert_eq!((files.len(), canisters.len()), (111, 41), "every shipped file reads");
    let (_, refused) = Catalogue::build(files, canisters);
    assert!(refused.is_empty(), "every shipped file is taken: {refused:?}");
    let c = current();
    assert_eq!((c.films().len(), c.virtuals().len(), c.dye_sets().len(), c.papers().len()), (105, 80, 47, 8));
    assert!(VirtualStock::default().is_known() && PaperStock::of("kodak-endura-premier").is_known());
    assert!(DyeSet::default().is_generic() && c.dye_set(DyeSet::default()).is_some());
    assert_eq!(c.virtual_film(VirtualStock::of("kodak-portra-800-push-2")).map(|v| (v.catalogue, v.dyes)), Some(("kodak-portra800-push2", DyeSet::of("kodak-portra-800"))));
    for v in c.virtuals() {
        assert!(c.dye_set(v.dyes).is_some() && c.film(v.catalogue).is_some_and(|k| k.film == Some(v.stock)), "{:?}", v.stock);
    }
    for k in c.films() {
        assert!(k.dyes.is_none_or(|d| c.dye_set(d).is_some()), "{}", k.id);
    }
    // Retired keys read as their entry's and are written under it.
    let twin: VirtualStock = serde_json::from_str("\"kodak-gold-200-datasheet\"").unwrap();
    assert_eq!(serde_json::to_string(&twin).unwrap(), "\"kodak-gold-200\"");
    // Twin: an unknown key is kept as stored, never read as another.
    let lost: VirtualStock = serde_json::from_str("\"kodak-gold-900\"").unwrap();
    assert_eq!((lost.key(), lost.is_known(), lost.reference()), ("kodak-gold-900", false, VirtualStock::default().reference()));
}

/// A person's file adds a stock, or stands for the shipped one of its key; one clashing with another, dropping a key
/// its shipped file defined or naming dyes no file defines is refused, the rest kept.
#[test]
fn a_persons_file_adds_or_stands_in_and_a_clashing_one_is_refused() {
    let added = edited("kodak-gold200.json", |v| {
        v["key"] = "kodak-gold200-mine".into();
        v["name"] = "Kodak Gold 200 (Mine)".into();
        v["film"]["key"] = "kodak-gold-200-mine".into();
        v["film"]["dyes"] = "kodak-gold-200".into();
        v["film"].as_object_mut().unwrap().remove("aliases");
    });
    let (c, refused) = with_own(vec![own("kodak-gold200-mine.json", &added).unwrap()]);
    assert!(refused.is_empty(), "{refused:?}");
    assert_eq!(c.films().len(), 106);
    let mine = c.virtual_film(VirtualStock::of("kodak-gold-200-mine")).expect("its film is listed");
    assert_eq!((mine.name, mine.dyes), ("Kodak Gold 200 (Mine)", DyeSet::of("kodak-gold-200")));
    let place = |c: &Catalogue, id: &str| c.films().iter().position(|k| k.id == id).unwrap();
    assert_eq!(place(&c, "kodak-gold200-mine"), place(&c, "kodak-gold200") + 1, "listed by its name, after Gold 200");

    let renamed = edited("kodak-gold200.json", |v| v["name"] = "Kodak Gold 200 (Edited)".into());
    let (c, refused) = with_own(vec![own("kodak-gold200.json", &renamed).unwrap()]);
    assert!(refused.is_empty() && c.films().len() == 105, "{refused:?}");
    assert_eq!(c.film("kodak-gold200").map(|k| k.display_name), Some("Kodak Gold 200 (Edited)"));

    let dropped = edited("kodak-gold200.json", |v| drop(v.as_object_mut().unwrap().remove("film")));
    let clash = edited("kodak-gold200.json", |v| v["key"] = "kodak-gold200-again".into());
    let dangling = edited("kodak-gold200.json", |v| {
        v["key"] = "kodak-gold200-dangling".into();
        v["dyes"] = "kodak-gold-900".into();
        v.as_object_mut().unwrap().remove("film");
    });
    let files = vec![own("kodak-gold200.json", &dropped).unwrap(), own("again.json", &clash).unwrap(), own("dangling.json", &dangling).unwrap()];
    let (c, refused) = with_own(files);
    let why = |name: &str| refused.iter().find(|r| r.name == name).map(|r| r.reason.clone()).unwrap_or_default();
    assert!(why("kodak-gold200.json").contains("drops the key \"kodak-gold-200\""), "{refused:?}");
    assert!(why("again.json").contains("as kodak-gold200.json does"), "{refused:?}");
    assert!(why("dangling.json").contains("kodak-gold-900"), "{refused:?}");
    // Twin: the shipped Gold 200 stays in use, whole.
    assert_eq!(c.film("kodak-gold200").map(|k| k.display_name), Some("Kodak Gold 200"));
    assert!(c.virtual_film(VirtualStock::of("kodak-gold-200")).is_some() && c.films().len() == 105);
}

#[test]
fn names_sort_as_a_person_reads_them() {
    let mut names = vec!["Kodak Vision3 500T", "Kodak Vision3 50D", "Kodak Vision3 250D", "kodak vision3 200T", "Kodak Portra 800 (Push 1)", "Kodak Portra 800"];
    names.sort_by(|a, b| natural(a, b));
    assert_eq!(names, ["Kodak Portra 800", "Kodak Portra 800 (Push 1)", "Kodak Vision3 50D", "kodak vision3 200T", "Kodak Vision3 250D", "Kodak Vision3 500T"]);
    // Twin: compared as bytes, 500T would come before 50D and lowercase after every capital.
    assert_eq!("Kodak Vision3 500T".cmp("Kodak Vision3 50D"), Ordering::Less);
    assert_eq!(natural("Kodak Vision3 500T", "Kodak Vision3 50D"), Ordering::Greater);
}

/// A file read, by whatever name: a stock's and a paper's both.
#[test]
fn a_file_says_what_it_describes() {
    let film = shipped_json().find(|(n, _)| *n == "kodak-portra800.json").unwrap().1;
    let paper = shipped_json().find(|(n, _)| *n == "fujifilm-frontier-sp-3000.json").unwrap().1;
    assert!(matches!(file::read(film), Ok(File::Film(f)) if f.developments.len() == 2));
    assert!(matches!(file::read(paper), Ok(File::Paper(p)) if p.profile.is_none()));
    // Twin: a paper with no look, or one holding a field no paper holds, is refused.
    let lookless = String::from_utf8_lossy(paper).replacen("\"look\":", "\"seen\":", 1);
    assert!(file::read(lookless.as_bytes()).is_err());
}

/// Every shipped film is offered, each colour negative stating its couplers as the scanner reads it uncoupled: a digital
/// photo on any, a black and white one or a key no file defines stays as stored.
#[test]
fn every_shipped_film_is_offered_and_a_photo_on_one_stays() {
    use crate::{ConversionMode, PipelineSettings, VirtualStock};
    let on = |key: &'static str| PipelineSettings::rest(ConversionMode::Digital, true).on_stock(VirtualStock::of(key));
    assert!(VirtualStock::all().into_iter().all(|v| v.entry().offered()), "every shipped film offered");
    // Twin: a colour film whose file states no couplers is offered no more.
    let mut unstated = VirtualStock::of("kodak-vision3-50d").entry();
    unstated.couplers = None;
    assert!(!unstated.offered());
    for key in ["kodak-vision3-50d", "kodak-portra-400", "ilford-hp5-plus"] {
        assert_eq!(on(key).on_offered_film(), on(key), "{key}");
    }
    let gone = PipelineSettings { virtual_film: Some(crate::VirtualFilm { stock: VirtualStock::of("kodak-gold-900"), exposure: 0.0 }), ..on("kodak-portra-400") };
    assert_eq!(gone.clone().on_offered_film(), gone);
}
