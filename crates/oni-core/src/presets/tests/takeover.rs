//! The single-file list of earlier versions taken over: once, whole in its archive, never twice.

use super::*;

#[test]
fn the_single_file_list_is_taken_over_once_then_archived() {
    let support = folder("migrate");
    std::fs::create_dir_all(&support).unwrap();
    let mut element: Value = serde_json::from_str(&codec::encode_settings(&PipelineSettings::default()).unwrap()).unwrap();
    element["name"] = "Old one".into();
    element["groups"] = serde_json::json!(["detail"]);
    std::fs::write(legacy_file(&support), serde_json::to_vec(&serde_json::json!([element, {"nothing": 1}])).unwrap()).unwrap();
    let shelf = load_shelf(&support);
    assert_eq!(shelf.len(), 1);
    assert_eq!(shelf[0].parameters, BTreeSet::from(["chroma".to_string(), "sharpen".to_string()]));
    assert!(!legacy_file(&support).exists() && support.join("presets.json.migrated").exists());
    // Twin: a second load finds no list and keeps the one file.
    assert_eq!(load_shelf(&support).len(), 1);
    assert_eq!(files(&directory(&support)).len(), 1);
    let _ = std::fs::remove_dir_all(&support);
}

fn named(name: &str) -> Preset {
    Preset::new(name, BTreeSet::from(["sharpen".to_string()]), Tint::Moss, PipelineSettings::default())
}

#[test]
fn a_failed_takeover_keeps_the_list() {
    let support = folder("takeover");
    std::fs::create_dir_all(&support).unwrap();
    std::fs::write(directory(&support), b"a file where the folder goes").unwrap();
    let element: Value = serde_json::from_str(&encode(&sample()).unwrap()).unwrap();
    std::fs::write(legacy_file(&support), serde_json::to_vec(&serde_json::json!([element])).unwrap()).unwrap();
    assert!(load_shelf(&support).is_empty());
    assert!(listed(&legacy_file(&support)) == [element.clone()] && listed(&support.join("presets.json.migrated")) == [element.clone()]);
    // Twin: with the folder writable the preset comes in and the list goes, its archive kept.
    std::fs::remove_file(directory(&support)).unwrap();
    assert_eq!(load_shelf(&support).len(), 1);
    assert!(!legacy_file(&support).exists() && listed(&support.join("presets.json.migrated")) == [element]);
    let _ = std::fs::remove_dir_all(&support);
}

#[test]
fn a_takeover_rerun_keeps_the_edits_and_deletions_made_since() {
    let support = folder("rerun");
    let shelf = directory(&support);
    let (a, b, c) = (named("A"), named("B"), named("C"));
    let blocked = shelf.join(file_name(&c));
    std::fs::create_dir_all(&blocked).unwrap();
    let list: Vec<Value> = [&a, &b, &c].iter().map(|p| serde_json::from_str(&encode(p).unwrap()).unwrap()).collect();
    std::fs::write(legacy_file(&support), serde_json::to_vec(&list).unwrap()).unwrap();
    assert_eq!(load_shelf(&support).len(), 2);
    save(&Preset { name: "A edited".into(), ..a.clone() }, &shelf).unwrap();
    delete(&b, &shelf, &mut |f| std::fs::remove_file(f));
    let names = |s: &Path| load_shelf(s).into_iter().map(|p| p.name).collect::<Vec<_>>();
    assert_eq!(names(&support), ["A edited"]);
    // Twin: what never reached the shelf still comes in once the way is clear, and the list goes.
    std::fs::remove_dir(&blocked).unwrap();
    assert_eq!(names(&support), ["A edited", "C"]);
    assert!(!legacy_file(&support).exists());
    let _ = std::fs::remove_dir_all(&support);
}

fn elements(presets: &[&Preset]) -> Vec<Value> {
    presets.iter().map(|p| serde_json::from_str(&encode(p).unwrap()).unwrap()).collect()
}

/// The list a file holds, or none.
fn listed(path: &Path) -> Vec<Value> {
    std::fs::read(path).ok().and_then(|d| serde_json::from_slice(&d).ok()).unwrap_or_default()
}

type Rename<'a> = &'a dyn Fn(&Path, &Path) -> io::Result<()>;
type Write<'a> = &'a dyn Fn(&Path, &[u8]) -> io::Result<()>;

/// A takeover of `[A, B, C]` refused somewhere, every preset it showed then deleted.
struct Refused {
    support: PathBuf,
    blocked: PathBuf,
    list: Vec<Value>,
    deleted: Vec<String>,
    after: Vec<String>,
}

/// `[A, B, C]` taken over through `rename` and `write`, C's path blocked if asked; then every preset
/// shown is deleted and the shelf reloaded as the app does.
fn refused_pass(label: &str, block_c: bool, rename: Rename, write: Write) -> Refused {
    let support = folder(label);
    let shelf = directory(&support);
    let (a, b, c) = (named("A"), named("B"), named("C"));
    let blocked = shelf.join(file_name(&c));
    std::fs::create_dir_all(if block_c { &blocked } else { &shelf }).unwrap();
    let list = elements(&[&a, &b, &c]);
    std::fs::write(legacy_file(&support), serde_json::to_vec(&list).unwrap()).unwrap();
    shelf::taken_over(&legacy_file(&support), &shelf, rename, write);
    let shown = load(&shelf);
    for preset in &shown {
        delete(preset, &shelf, &mut |f| std::fs::remove_file(f));
    }
    let after = load_shelf(&support).into_iter().map(|p| p.name).collect();
    Refused { support, blocked, list, deleted: shown.into_iter().map(|p| p.name).collect(), after }
}

#[test]
fn a_preset_taken_then_deleted_stays_deleted_when_the_disk_refuses_the_rest() {
    let full = |_: &Path, _: &[u8]| Err(io::Error::other("disk full"));
    let held = |_: &Path, _: &Path| Err(io::Error::from_raw_os_error(32));
    let archive = |s: &Path| s.join("presets.json.migrated");
    // Disk full: C's save and the list's rewrite refused. Then the list held open, its rename refused.
    for (label, block_c, rename, write) in [("full", true, &oni_settings::atomic::renamed as Rename, &full as Write), ("held", false, &held, &oni_settings::atomic::write_atomically)] {
        let run = refused_pass(label, block_c, rename, write);
        assert!(run.after.iter().all(|n| !run.deleted.contains(n)), "{label}: deleted {:?}, back {:?}", run.deleted, run.after);
        assert!(listed(&legacy_file(&run.support)) == run.list || listed(&archive(&run.support)) == run.list, "{label}: the list kept whole");
        let _ = std::fs::remove_dir_all(&run.support);
    }
    // Twin: with the rewrite allowed, C is kept for a later run and comes in once its path is clear.
    let run = refused_pass("rewritten", true, &oni_settings::atomic::renamed, &oni_settings::atomic::write_atomically);
    assert_eq!((run.deleted, run.after), (vec!["A".to_string(), "B".into()], vec![]));
    std::fs::remove_dir(&run.blocked).unwrap();
    assert_eq!(load_shelf(&run.support).into_iter().map(|p| p.name).collect::<Vec<_>>(), ["C"]);
    assert!(listed(&archive(&run.support)) == run.list, "the archive holds the list as it came");
    let _ = std::fs::remove_dir_all(&run.support);
}

#[test]
fn an_element_the_takeover_cannot_read_stays_in_the_archive() {
    let support = folder("unreadable");
    let shelf = directory(&support);
    let (a, c) = (named("A"), named("C"));
    let blocked = shelf.join(file_name(&c));
    std::fs::create_dir_all(&blocked).unwrap();
    let [a_element, c_element] = <[Value; 2]>::try_from(elements(&[&a, &c])).unwrap();
    let list = vec![a_element, serde_json::json!({"nothing": 1}), c_element.clone()];
    std::fs::write(legacy_file(&support), serde_json::to_vec(&list).unwrap()).unwrap();
    assert_eq!(load_shelf(&support).len(), 1);
    // Twin: what goes back to the list is what failed alone, never the element no build has read yet.
    assert!(listed(&legacy_file(&support)) == [c_element], "the list holds C alone");
    std::fs::remove_dir(&blocked).unwrap();
    assert_eq!(load_shelf(&support).into_iter().map(|p| p.name).collect::<Vec<_>>(), ["A", "C"]);
    assert!(!legacy_file(&support).exists());
    assert!(listed(&support.join("presets.json.migrated")) == list, "the archive holds the list as it came");
    let _ = std::fs::remove_dir_all(&support);
}

#[test]
fn a_list_written_again_only_adds_to_the_shelf() {
    let support = folder("again");
    std::fs::create_dir_all(&support).unwrap();
    let (a, d) = (named("A"), named("D"));
    let first = elements(&[&a]);
    std::fs::write(legacy_file(&support), serde_json::to_vec(&first).unwrap()).unwrap();
    assert_eq!(load_shelf(&support).len(), 1);
    save(&Preset { name: "A edited".into(), ..a.clone() }, &directory(&support)).unwrap();
    std::fs::write(legacy_file(&support), serde_json::to_vec(&elements(&[&a, &d])).unwrap()).unwrap();
    assert_eq!(load_shelf(&support).into_iter().map(|p| p.name).collect::<Vec<_>>(), ["A edited", "D"]);
    // Twin: the list taken goes, and the archive keeps the first list as it came.
    assert!(!legacy_file(&support).exists() && !support.join("presets.json.taking").exists());
    assert!(listed(&support.join("presets.json.migrated")) == first, "the archive holds the first list");
    let _ = std::fs::remove_dir_all(&support);
}
