//! The person's stock files as the pipeline reads them: one added bakes on its derived reference, an edit renders otherwise,
//! a broken edit is refused with the version before kept, a removed file is gone. Each verdict has a twin.

use oni_fixtures::scratch;
use oni_pipeline::film::{self, GREY};
use oni_pipeline::stocks::Loader;
use oni_pipeline::{conversion, paper};
use oni_render::cpu;
use oni_settings::stocks::{self, shipped_files};
use oni_settings::{ConversionMode, PaperStock, PipelineSettings, VirtualStock};
use serde_json::Value;
use std::path::Path;

fn shipped(name: &str) -> Value {
    let bytes = shipped_files().iter().find(|(n, _)| *n == name).map(|(_, b)| *b).expect("shipped");
    serde_json::from_slice(bytes).unwrap()
}

fn write(dir: &Path, name: &str, v: &Value) {
    std::fs::write(dir.join(name), serde_json::to_vec_pretty(v).unwrap()).unwrap();
}

/// Gold 200's file as a stock of the person's own, its reference left out.
fn mine(name: &str) -> Value {
    let mut v = shipped("kodak-gold200.json");
    v["key"] = "kodak-gold200-mine".into();
    v["name"] = name.into();
    let film = v["film"].as_object_mut().unwrap();
    film.insert("key".into(), "kodak-gold-200-mine".into());
    film.insert("dyes".into(), "kodak-gold-200".into());
    film.remove("aliases");
    film.remove("reference");
    v
}

const MINE: VirtualStock = VirtualStock::of("kodak-gold-200-mine");

/// A ramp of scene greys through `stock`'s negative and print on the CPU.
fn printed(stock: VirtualStock) -> Vec<[f32; 4]> {
    let s = PipelineSettings::rest(ConversionMode::Digital, true).per_pixel_only().on_stock(stock);
    let baked = film::stock(stock);
    let k = conversion::stage47(&s);
    (-8..=8).map(|h| GREY * 2f32.powf(h as f32 / 2.0)).map(|v| cpu::stage47(baked.scanned(cpu::virtual_negative([v, v, v, 1.0], &baked.kernel, &baked.table)), &k)).collect()
}

#[test]
fn a_persons_stock_is_added_edited_refused_and_removed_as_its_file_is() {
    let dir = scratch("stock-files-films");
    let mut loader = Loader::new(dir.clone());
    let first = loader.reload().expect("the first read builds");
    assert!(first.refused.is_empty());
    assert_eq!(first.catalogue.films().len(), 104);
    stocks::install(first.catalogue);
    assert!(loader.reload().is_none(), "a folder as it was builds nothing");

    write(&dir, "mine.json", &mine("Kodak Gold 200 (Mine)"));
    let added = loader.reload().expect("a file added is read");
    assert!(added.refused.is_empty(), "{:?}", added.refused);
    stocks::install(added.catalogue);
    assert_eq!((oni_settings::film::stocks().len(), MINE.name(), MINE.is_known()), (105, "Kodak Gold 200 (Mine)", true));
    let gold = VirtualStock::of("kodak-gold-200");
    assert_eq!(MINE.reference(), gold.reference(), "its reference is the one its profile derives, Gold 200's");
    assert!(printed(MINE).iter().all(|p| p.iter().all(|v| v.is_finite())));
    assert_eq!(printed(MINE), printed(gold), "Gold 200's profile renders as Gold 200");

    // An edit of its profile bakes anew and renders otherwise.
    let mut edited = mine("Kodak Gold 200 (Mine)");
    edited["film"]["balance"] = "tungsten".into();
    write(&dir, "mine.json", &edited);
    let moved = loader.reload().expect("an edit is read");
    stocks::install(moved.catalogue);
    assert_ne!(printed(MINE), printed(gold), "a tungsten balance renders otherwise");
    assert_ne!(film::stock(MINE).revision, film::stock(gold).revision);

    // A broken edit is refused, told once, the version before staying in use.
    std::fs::write(dir.join("mine.json"), b"{\"oni\": \"film\", \"format\": 1, \"key\": \"kodak-gold200-mine\"").unwrap();
    let broken = loader.reload().expect("a broken edit is read");
    assert_eq!(broken.refused.len(), 1);
    assert!(broken.refused[0].earlier_kept && broken.refused[0].refusal.name == "mine.json", "{:?}", broken.refused);
    stocks::install(broken.catalogue);
    assert_eq!(MINE.balance(), oni_settings::virtual_film::Balance::Tungsten, "the last version that read stays");
    let mut spoiled = edited.clone();
    spoiled["film"]["profile"]["data"]["log_exposure"] = Value::Array(vec![]);
    write(&dir, "mine.json", &spoiled);
    let unbaked = loader.reload().expect("a profile that does not read is read");
    assert!(unbaked.refused.iter().any(|r| r.refusal.reason.contains("kodak-gold-200-mine")), "{:?}", unbaked.refused);
    // Twin: the same refusal read again is not told again.
    std::fs::write(dir.join("other.svg"), b"<svg/>").unwrap();
    assert!(loader.reload().expect("a new canister is read").refused.is_empty());

    std::fs::remove_file(dir.join("mine.json")).unwrap();
    let removed = loader.reload().expect("a file removed is read");
    stocks::install(removed.catalogue);
    assert!(!MINE.is_known() && oni_settings::film::stocks().len() == 104);
    assert_eq!(MINE.reference(), VirtualStock::default().reference(), "a key no file defines renders on the default stock");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_persons_paper_is_listed_and_bakes() {
    let dir = scratch("stock-files-papers");
    let mut v = shipped("kodak-royal-digital.json");
    v["key"] = "kodak-royal-digital-mine".into();
    v["name"] = "Kodak Royal Digital (Mine)".into();
    v.as_object_mut().unwrap().remove("aliases");
    write(&dir, "mine.json", &v);
    let mut loader = Loader::new(dir.clone());
    let read = loader.reload().expect("the first read builds");
    assert!(read.refused.is_empty(), "{:?}", read.refused);
    stocks::install(read.catalogue);
    let mine = PaperStock::of("kodak-royal-digital-mine");
    assert!(PaperStock::all().contains(&mine) && mine.brand() == "Kodak");
    let (own, royal) = (paper::sheet(mine), paper::sheet(PaperStock::default()));
    assert_eq!(own.curves, royal.curves, "the same profile bakes the same sheet");
    // Twin: a fit that is no fit is refused.
    let mut fit = shipped("fujifilm-frontier-sp-3000.json");
    fit["key"] = "frontier-mine".into();
    fit["frontier"]["curves"][1] = Value::Array(vec![]);
    write(&dir, "fit.json", &fit);
    let refused = loader.reload().expect("read").refused;
    assert!(refused.iter().any(|r| r.refusal.name == "fit.json" && !r.earlier_kept), "{refused:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
