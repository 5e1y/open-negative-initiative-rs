//! Roll #8's archive proofs on a roll the test makes itself, so they run without the reference roll:
//! a marker, a sidecar a frame naming the roll's reference, an empty placeholder standing for each RAW.

use oni_library::{LibraryIndex, folders, index, library, roll_membership, roots};
use oni_settings::frame::RollReference;
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{Film, Id, PipelineSettings};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

const FRAMES: usize = 6;

struct Made {
    root: PathBuf,
    roll: PathBuf,
    film: Film,
    reference: Id,
}

impl Drop for Made {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A folder named apart from its roll, so a heading read off the folder cannot pass for the roll's.
fn made(label: &str) -> Made {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let root = std::env::temp_dir().join(format!("oni-library-made-{label}-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let root = PathBuf::from(library::canonical(&root));
    let roll = root.join("made-copy");
    std::fs::create_dir(&roll).unwrap();
    let film = Film { stock: Some("fujifilm-fujicolor".into()), ..Film::new("Roll A") };
    folders::set_roll(Some(&film), &roll.to_string_lossy()).unwrap();
    let reference = Id::new();
    let rolled = RollReference { id: reference, stops: [0.0; 3], black: [0.0; 3], white: [1.0; 3], base: None, median: None };
    for i in 0..FRAMES {
        let frame = roll.join(format!("DSC_{i:04}.NEF"));
        std::fs::write(&frame, b"").unwrap();
        let settings = PipelineSettings { roll_reference: Some(rolled.clone()), ..PipelineSettings::default() };
        sidecar::write(&settings, &frame, WriteOptions::default()).unwrap();
    }
    Made { root, roll, film, reference }
}

fn listing(folder: &Path) -> BTreeMap<String, u64> {
    fn walk(base: &Path, folder: &Path, out: &mut BTreeMap<String, u64>) {
        for entry in std::fs::read_dir(folder).unwrap().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                walk(base, &path, out);
            } else {
                out.insert(path.strip_prefix(base).unwrap().to_string_lossy().into_owned(), entry.metadata().unwrap().len());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(folder, folder, &mut out);
    out
}

fn indexed_all(members: &[String]) -> Vec<index::Entry> {
    members.iter().filter_map(|m| library::indexed(m)).collect()
}

#[test]
fn an_index_rebuilt_from_the_sidecars_finds_every_frame_and_a_disk_away_invents_none() {
    let made = made("rebuild");
    let root = made.root.to_string_lossy().into_owned();
    let members = roots::scan(std::slice::from_ref(&root));
    assert_eq!(members.len(), FRAMES);
    assert!(members.iter().all(|m| m.ends_with(".NEF")));
    // Twin: every file of the folder, sidecars and marker included, is not the membership.
    assert_eq!(listing(&made.root).len(), 2 * FRAMES + 1);

    let index = LibraryIndex::open(&made.root.join("index.sqlite")).unwrap();
    index.rebuild(&indexed_all(&members)).unwrap();
    assert_eq!(index.count(), FRAMES);
    assert!(library::unresolved(&members, &index).is_empty());
    let paths: HashSet<String> = index.all().into_iter().map(|e| e.path).collect();
    assert_eq!(paths, members.iter().cloned().collect());

    // The roll is the folder's marker, read off the disk, never the index.
    let rolls = folders::rolls(&members);
    let film = rolls.get(&*made.roll.to_string_lossy()).expect("the folder carries its roll");
    assert_eq!((film.id, film.name.as_str(), film.stock.as_deref()), (made.film.id, "Roll A", Some("fujifilm-fujicolor")));
    let groups = folders::groups(&root, &index.shown(), &rolls, &Default::default());
    assert_eq!((groups.len(), groups[0].display_title(), groups[0].entries.len()), (1, "Roll A".to_owned(), FRAMES));
    // Twin: without the marker the folder heads with its own name.
    let bare = folders::groups(&root, &index.shown(), &Default::default(), &Default::default());
    assert_eq!(bare[0].display_title(), "made-copy");

    // A disk gone with one frame: no row is invented, and putting it back is the repair.
    let away = &members[3];
    let hidden = made.root.join("away.bin");
    std::fs::rename(away, &hidden).unwrap();
    assert!(library::indexed(away).is_none());
    index.rebuild(&indexed_all(&members)).unwrap();
    assert_eq!(index.count(), FRAMES - 1);
    assert_eq!(library::unresolved(&members, &index), std::slice::from_ref(away));
    std::fs::rename(&hidden, away).unwrap();
    for path in library::unresolved(&members, &index) {
        index.upsert(&library::indexed(&path).unwrap()).unwrap();
    }
    assert_eq!(index.count(), FRAMES);
    assert!(library::unresolved(&members, &index).is_empty());
}

#[test]
fn roll_membership_reads_the_reference_each_sidecar_names() {
    let made = made("membership");
    let members = roots::scan(&[made.root.to_string_lossy().into_owned()]);
    assert_eq!(roll_membership::measured(made.reference, &members).len(), FRAMES);
    // Twin: another id names no frame of the roll.
    assert!(roll_membership::measured(Id::new(), &members).is_empty());

    // Twin: one sidecar moved onto another reference leaves the roll, and only it.
    let strayed = &members[2];
    let side = sidecar::url_for(Path::new(strayed));
    let mut document: serde_json::Value = serde_json::from_slice(&std::fs::read(&side).unwrap()).unwrap();
    let other = Id::new();
    document["settings"]["rollReference"]["id"] = serde_json::Value::String(other.to_string());
    std::fs::write(&side, serde_json::to_vec(&document).unwrap()).unwrap();
    let measured = roll_membership::measured(made.reference, &members);
    assert_eq!(measured.len(), FRAMES - 1);
    assert!(!measured.contains(strayed));
    assert_eq!(roll_membership::reference(strayed), Some(other));
    // A frame with no sidecar stands on no reference.
    let lone = made.roll.join("lone.NEF");
    std::fs::write(&lone, b"").unwrap();
    assert_eq!(roll_membership::reference(&lone.to_string_lossy()), None);
}
