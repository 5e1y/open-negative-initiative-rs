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
    Bookmarks::default().noted(&favourites.paths, place).save(&support).unwrap();
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
