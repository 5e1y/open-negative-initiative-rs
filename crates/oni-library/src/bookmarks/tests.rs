use super::*;
use crate::favourites::Favourites;
use crate::scratch::Scratch;

/// `Favourites.swift` at launch: the file decoded as its two fields, then encoded back.
#[derive(Serialize, Deserialize)]
struct SwiftFavourites {
    version: i64,
    paths: Vec<String>,
}

fn swift_launch(support: &Path) {
    let file = Favourites::file_path(support);
    let read: SwiftFavourites = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    std::fs::write(&file, serde_json::to_vec(&read).unwrap()).unwrap();
}

#[test]
fn a_swift_launch_keeps_the_volume_identities() {
    let scratch = Scratch::new("bookmarks");
    let support = scratch.path().join("support");
    let scans = scratch.path().join("Scans");
    std::fs::create_dir_all(&scans).unwrap();
    let path = crate::paths::canonical(&scans);
    let mut favourites = Favourites { version: 1, paths: vec![path.clone()] };
    favourites.save_merged(&mut Favourites::default(), &support).unwrap();
    Bookmarks::default().noted(&favourites.paths, |p| place(p, &system())).save(&support).unwrap();
    let noted = Bookmarks::load(&support).places[&path].clone();
    assert!(noted.file_id.is_some());
    if cfg!(target_os = "macos") {
        assert!(noted.volume.as_ref().is_some_and(|v| v.uuid.is_some()), "{noted:?}");
    }
    swift_launch(&support);
    assert_eq!(Favourites::load(&support).unwrap().paths, [path.clone()]);
    assert_eq!(Bookmarks::load(&support).places.get(&path), Some(&noted));
    // Twin: the same identity kept inside `favourites.json` is gone after Swift's launch.
    let file = Favourites::file_path(&support);
    std::fs::write(&file, format!(r#"{{"version":1,"paths":["{path}"],"places":{{"{path}":{{"fileId":1}}}}}}"#)).unwrap();
    swift_launch(&support);
    assert!(!std::fs::read_to_string(&file).unwrap().contains("places"));
}

#[test]
fn a_disk_away_or_another_volume_keeps_the_identity_known() {
    let volume = |uuid: &str| Place { volume: Some(Volume { uuid: Some(uuid.into()), mount: "/Volumes/T7".into() }), file_id: Some(7) };
    let known = Bookmarks { version: 1, places: [("/Volumes/T7/Scans".to_string(), volume("A"))].into() };
    let paths = vec!["/Volumes/T7/Scans".to_string()];
    assert_eq!(known.noted(&paths, |_| None), known, "a disk away");
    assert_eq!(known.noted(&paths, |_| Some(volume("B"))), known, "another disk under the same name");
    let refreshed = Place { file_id: Some(8), ..volume("A") };
    assert_eq!(known.noted(&paths, |_| Some(refreshed.clone())).places["/Volumes/T7/Scans"], refreshed);
    // Twin: a path no longer a favourite leaves the file.
    assert!(known.noted(&[], |_| Some(volume("A"))).places.is_empty());
}

/// Each volume of `computer` named by the UUID `uuids` gives its folder's name.
fn fake(computer: &Computer, uuids: &[(&str, &str)]) -> Identify {
    let (computer, uuids): (Computer, Vec<(String, String)>) = (computer.clone(), uuids.iter().map(|(n, u)| (n.to_string(), u.to_string())).collect());
    Arc::new(move |path: &str| {
        let root = crate::absence::volume_root(&computer, path)?;
        let uuid = uuids.iter().find(|(n, _)| *n == paths::name(&root)).map(|(_, u)| u.clone());
        Some(Volume { uuid, mount: root })
    })
}

#[test]
fn a_volume_known_by_its_uuid_under_another_name_is_a_lead() {
    let scratch = Scratch::new("bookmarks-volume");
    let volumes = scratch.path().join("Volumes");
    std::fs::create_dir_all(volumes.join("T7 1/Scans")).unwrap();
    let computer = Computer { home: paths::canonical(scratch.path()), volumes: paths::canonical(&volumes) };
    let (t7, t7_1) = (paths::join(&computer.volumes, "T7"), paths::join(&computer.volumes, "T7 1"));
    let scans = paths::join(&t7, "Scans");
    let known = Bookmarks { version: 1, places: [(scans.clone(), Place { volume: Some(Volume { uuid: Some("A".into()), mount: t7.clone() }), file_id: None })].into() };
    let mounted = vec![computer.home.clone(), t7_1.clone()];
    let lead = known.lead(&paths::join(&scans, "Roll 12"), &computer, &mounted, &fake(&computer, &[("T7 1", "A")]));
    assert_eq!(lead, None, "the folder itself is not on the disk now under that name");
    std::fs::create_dir_all(volumes.join("T7 1/Scans/Roll 12")).unwrap();
    let lead = known.lead(&paths::join(&scans, "Roll 12"), &computer, &mounted, &fake(&computer, &[("T7 1", "A")]));
    assert_eq!(lead, Some(Lead { from: t7.clone(), to: t7_1.clone(), volume: true }));
    // Twins: another disk under the new name, or the disk back under its own name, leads nowhere.
    assert_eq!(known.lead(&scans, &computer, &mounted, &fake(&computer, &[("T7 1", "B")])), None);
    std::fs::create_dir_all(volumes.join("T7/Scans")).unwrap();
    let both = vec![t7.clone(), t7_1.clone()];
    assert_eq!(known.lead(&scans, &computer, &both, &fake(&computer, &[("T7", "A"), ("T7 1", "A")])), None);
}

#[cfg(target_os = "macos")]
#[test]
fn a_favourite_renamed_in_the_finder_is_found_by_its_file_id_and_one_thrown_away_is_not() {
    let scratch = Scratch::new("bookmarks-id");
    let scans = scratch.path().join("Scans");
    std::fs::create_dir_all(scans.join("Roll 12/Day 2")).unwrap();
    std::fs::create_dir_all(scratch.path().join("Archive")).unwrap();
    let computer = Computer { home: paths::canonical(scratch.path()), volumes: "/Volumes".into() };
    let old = paths::canonical(&scans.join("Roll 12"));
    let known = Bookmarks::default().noted(std::slice::from_ref(&old), |p| place(p, &system()));
    let inside = paths::join(&old, "Day 2");
    std::fs::rename(scans.join("Roll 12"), scans.join("2026-03 Lisbon")).unwrap();
    let renamed = paths::canonical(&scans.join("2026-03 Lisbon"));
    assert_eq!(known.lead(&inside, &computer, &[], &system()), Some(Lead { from: old.clone(), to: renamed, volume: false }));
    // Moved elsewhere on the volume, the volume says where.
    std::fs::rename(scans.join("2026-03 Lisbon"), scratch.path().join("Archive/Lisbon")).unwrap();
    let archived = paths::canonical(&scratch.path().join("Archive/Lisbon"));
    assert_eq!(known.lead(&old, &computer, &[], &system()).map(|l| l.to), Some(archived));
    // Twins: thrown into a Trash, or deleted, it leads nowhere.
    std::fs::create_dir_all(scratch.path().join(".Trash")).unwrap();
    std::fs::rename(scratch.path().join("Archive/Lisbon"), scratch.path().join(".Trash/Lisbon")).unwrap();
    assert_eq!(known.lead(&old, &computer, &[], &system()), None);
    std::fs::remove_dir_all(scratch.path().join(".Trash/Lisbon")).unwrap();
    assert_eq!(known.lead(&old, &computer, &[], &system()), None);
}
