use super::*;
use crate::navigator::Computer;
use crate::scratch::Scratch;
use std::sync::Arc;

#[test]
fn a_folder_s_identity_is_its_volume_and_its_file_id() {
    let scratch = Scratch::new("places-identity");
    let path = crate::paths::canonical(scratch.path());
    let noted = place(&path, &system()).expect("the folder answers");
    assert!(noted.path == path && noted.file_id.is_some(), "{noted:?}");
    if cfg!(any(target_os = "macos", windows)) {
        assert!(noted.volume.as_ref().is_some_and(|v| v.uuid.is_some()), "{noted:?}");
    }
    // Twin: a folder gone answers nothing, which keeps the identity known.
    assert_eq!(place(&format!("{path}/gone"), &system()), None);
}

#[test]
fn a_disk_away_or_another_volume_keeps_the_identity_known() {
    let scans = "/Volumes/T7/Scans";
    let volume = |uuid: &str| Place { path: scans.into(), volume: Some(Volume { uuid: Some(uuid.into()), mount: "/Volumes/T7".into() }), file_id: Some(7) };
    let known = Places { version: VERSION, places: vec![volume("A")] };
    assert_eq!(known.noted(|_| None), known, "a disk away");
    assert_eq!(known.noted(|_| Some(volume("B"))), known, "another disk under the same name");
    let refreshed = Place { file_id: Some(8), ..volume("A") };
    assert_eq!(known.noted(|_| Some(refreshed.clone())).places, std::slice::from_ref(&refreshed));
    // Laid on the places held: a star taken off meanwhile stays off, the others take what was read.
    let mut held = Places { version: VERSION, places: vec![Place::at("/Volumes/T7/Other"), volume("A")] };
    assert!(held.take_identities(&known.noted(|_| Some(refreshed.clone()))));
    assert_eq!(held.places, [Place::at("/Volumes/T7/Other"), refreshed.clone()]);
    // Twin: what was read already held changes nothing, so nothing is written.
    assert!(!held.take_identities(&known.noted(|_| Some(refreshed.clone()))));
}

/// Each volume of `computer` named by the UUID `uuids` gives its folder's name.
fn fake(computer: &Computer, uuids: &[(&str, &str)]) -> Identify {
    let (computer, uuids): (Computer, Vec<(String, String)>) = (computer.clone(), uuids.iter().map(|(n, u)| (n.to_string(), u.to_string())).collect());
    Arc::new(move |path: &str| {
        let root = computer.volume_root(path)?;
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
    let known = Places { version: VERSION, places: vec![Place { path: scans.clone(), volume: Some(Volume { uuid: Some("A".into()), mount: t7.clone() }), file_id: None }] };
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

#[cfg(any(target_os = "macos", windows))]
#[test]
fn a_favourite_renamed_in_the_finder_is_found_by_its_file_id_and_one_thrown_away_is_not() {
    let scratch = Scratch::new("bookmarks-id");
    let scans = scratch.path().join("Scans");
    std::fs::create_dir_all(scans.join("Roll 12/Day 2")).unwrap();
    std::fs::create_dir_all(scratch.path().join("Archive")).unwrap();
    let computer = Computer { home: paths::canonical(scratch.path()), volumes: "/Volumes".into() };
    let old = paths::canonical(&scans.join("Roll 12"));
    let known = Places { version: VERSION, places: vec![Place::at(&old)] }.noted(|p| place(p, &system()));
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
