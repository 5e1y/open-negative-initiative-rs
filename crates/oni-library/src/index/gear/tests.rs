use super::*;
use crate::index::Entry;
use crate::scratch::Scratch;
use crate::{library, paths};
use oni_settings::sidecar::{self, ExifLine};
use oni_settings::{Date, Film};

fn day() -> Date {
    Date::since_reference_date(800_000_000.0)
}

fn roll(body: Option<&str>) -> Film {
    let mut film = Film::new("Roll");
    film.camera_body = body.map(str::to_owned);
    film
}

#[test]
fn the_rolls_bodies_come_out_once_each_the_most_photos_first() {
    let scratch = Scratch::new("index-gear");
    let index = LibraryIndex::open(&scratch.path().join("index.sqlite")).unwrap();
    assert_eq!(index.camera_bodies().unwrap(), None, "an empty index answers nothing yet");
    for (i, body) in [Some("Leica M6"), Some("Nikon F3"), Some(" Nikon F3 "), None, Some("")].iter().enumerate() {
        let path = format!("/negs/{i}.RW2");
        index.upsert(&Entry::new(&path, day(), &format!("f-{i}"))).unwrap();
        index.set_film(Some(&roll(*body)), &path).unwrap();
    }
    assert_eq!(index.camera_bodies().unwrap(), Some(vec!["Nikon F3".to_string(), "Leica M6".to_string()]));
    // Twin: a roll without a body names none, and the index still answers.
    index.set_film(Some(&roll(None)), "/negs/0.RW2").unwrap();
    assert_eq!(index.camera_bodies().unwrap(), Some(vec!["Nikon F3".to_string()]));
}

/// A row of `path` in a roll of `body`, its line keeping `camera` and `lens`.
fn photo(index: &LibraryIndex, path: &str, body: Option<&str>, camera: Option<&str>, lens: Option<&str>) {
    let kept = |v: Option<&str>| v.map(str::to_owned);
    let entry = Entry { film: Some(roll(body)), kept_camera: kept(camera), kept_lens: kept(lens), ..Entry::new(path, day(), path) };
    index.upsert(&entry).unwrap();
}

#[test]
fn a_photo_counts_for_the_camera_its_line_keeps_over_its_roll_s_body() {
    let index = LibraryIndex::open(std::path::Path::new(":memory:")).unwrap();
    photo(&index, "/negs/a.NEF", Some("Nikon F3"), None, None);
    photo(&index, "/negs/b.NEF", Some("Nikon F3"), Some("Leica M6"), Some("Summicron 35"));
    photo(&index, "/negs/c.NEF", Some("Nikon F3"), Some(" "), None);
    photo(&index, "/negs/d.NEF", None, Some("  leica   M6 "), Some("summicron  35"));
    let counts = index.gear_counts().unwrap();
    assert_eq!((counts.body("Nikon F3"), counts.body("Leica M6")), (2, 2), "a blank camera kept leaves the roll's: {counts:?}");
    assert_eq!((counts.body(" LEICA  m6"), counts.lens("Summicron 35")), (2, 2), "names matched trimmed, spaces as one, case aside");
    assert_eq!((counts.body(""), counts.body("Leica M5"), counts.lens("Nikon F3")), (0, 0, 0));
    // Twin: the line's camera cleared, the roll's body counts it again.
    photo(&index, "/negs/b.NEF", Some("Nikon F3"), None, Some("Summicron 35"));
    let twin = index.gear_counts().unwrap();
    assert_eq!((twin.body("Nikon F3"), twin.body("Leica M6")), (3, 1));
}

#[test]
fn the_trash_s_photos_passes_and_contact_sheets_count_for_no_gear() {
    let index = LibraryIndex::open(std::path::Path::new(":memory:")).unwrap();
    for name in ["kept", "trashed", "pass", "sheet"] {
        photo(&index, &format!("/negs/{name}.NEF"), Some("Nikon F3"), None, Some("Nikkor 50"));
    }
    index.set_trashed(Some(day()), "/negs/trashed.NEF").unwrap();
    index.set_passes(Some("kept.NEF"), false, "/negs/pass.NEF").unwrap();
    let sheet = Entry { sheet: true, ..index.row("/negs/sheet.NEF").unwrap() };
    index.upsert(&sheet).unwrap();
    let counts = index.gear_counts().unwrap();
    assert_eq!((counts.body("Nikon F3"), counts.lens("Nikkor 50")), (1, 1));
    // Twin: restored from the ONI Trash and no longer a pass, each counts again.
    index.set_trashed(None, "/negs/trashed.NEF").unwrap();
    index.set_passes(None, false, "/negs/pass.NEF").unwrap();
    let twin = index.gear_counts().unwrap();
    assert_eq!((twin.body("Nikon F3"), twin.lens("Nikkor 50")), (3, 3));
}

/// What an export keeps of a line reaches the counts through the row read off the sidecar, a line
/// kept by format 21 and under, under `exif`, included.
#[test]
fn a_line_kept_in_the_sidecar_reaches_the_counts() {
    let scratch = Scratch::new("index-gear-kept");
    let index = LibraryIndex::open(&scratch.path().join("index.sqlite")).unwrap();
    let (fresh, older) = (scratch.path().join("a.NEF"), scratch.path().join("b.NEF"));
    for frame in [&fresh, &older] {
        std::fs::write(frame, b"raw").unwrap();
    }
    let read = |frame: &std::path::Path| index.upsert(&library::indexed(&paths::canonical(frame)).unwrap()).unwrap();
    read(&fresh);
    // Twin: before the line is kept, nothing names the lens.
    assert_eq!(index.gear_counts().unwrap().lens("Summicron 35"), 0);
    let line = ExifLine { camera: Some("Leica M6".into()), lens: Some("Summicron 35".into()), ..ExifLine::default() };
    sidecar::set_exif(line, &fresh, false).unwrap();
    let stored = r#"{"version":20,"settings":{},"exif":{"artist":"","camera":"Leica M6","copyright":"","date":"","film":"","iso":"","lens":"Summicron 35"}}"#;
    std::fs::write(sidecar::url_for(&older), stored).unwrap();
    read(&fresh);
    read(&older);
    let counts = index.gear_counts().unwrap();
    assert_eq!((counts.body("Leica M6"), counts.lens("Summicron 35")), (2, 2));
}

/// A folder made a roll counts its photos for the body its marker names, over each sidecar's roll, as its band
/// shows it, and the count reads the index alone: a marker changed on disk counts once the index learns it.
#[test]
fn a_folder_roll_s_body_counts_for_its_photos_off_the_index_alone() {
    let scratch = Scratch::new("index-gear-marker");
    let index = LibraryIndex::open(&scratch.path().join("index.sqlite")).unwrap();
    let (marked, loose) = (scratch.path().join("Roll 1"), scratch.path().join("Loose"));
    for folder in [&marked, &loose] {
        std::fs::create_dir_all(folder).unwrap();
    }
    let (marked, loose) = (paths::canonical(&marked), paths::canonical(&loose));
    crate::folders::set_roll(Some(&roll(Some("Nikon FM2"))), &marked).unwrap();
    for name in ["a", "b", "c"] {
        std::fs::write(format!("{marked}/{name}.NEF"), name).unwrap();
    }
    std::fs::write(format!("{loose}/d.NEF"), "d").unwrap();
    let read = |path: &str, body: Option<&str>, camera: Option<&str>| {
        let entry = library::indexed(path).unwrap();
        index.upsert(&Entry { film: Some(roll(body)), kept_camera: camera.map(str::to_owned), ..entry }).unwrap();
    };
    read(&format!("{marked}/a.NEF"), None, None);
    read(&format!("{marked}/b.NEF"), Some("Leica M6"), None);
    read(&format!("{marked}/c.NEF"), None, Some("Pentax MX"));
    read(&format!("{loose}/d.NEF"), Some("Leica M6"), None);
    let counts = index.gear_counts().unwrap();
    assert_eq!((counts.body("Nikon FM2"), counts.body("Leica M6"), counts.body("Pentax MX")), (2, 1, 1), "{counts:?}");
    // The marker gone from the disk: the count reads no disk, so it stands until the index learns it.
    crate::folders::set_roll(None, &marked).unwrap();
    assert_eq!(index.gear_counts().unwrap(), counts);
    index.set_marker(&marked, None).unwrap();
    let twin = index.gear_counts().unwrap();
    assert_eq!((twin.body("Nikon FM2"), twin.body("Leica M6")), (0, 2), "twin: no longer a roll, each sidecar's roll counts");
    // A roll whose marker names no body names none, over the sidecar's.
    index.set_marker(&marked, Some(&roll(None))).unwrap();
    assert_eq!(index.gear_counts().unwrap().body("Leica M6"), 1);
}

/// A marker's body reaches the rows right inside its folder alone.
#[test]
fn a_marker_reaches_its_folder_s_own_rows() {
    let index = LibraryIndex::open(std::path::Path::new(":memory:")).unwrap();
    for path in ["/negs/Roll/a.NEF", "/negs/Roll/Inner/b.NEF", "/negs/Rolled/c.NEF"] {
        index.upsert(&Entry::new(path, day(), path)).unwrap();
    }
    index.set_marker("/negs/Roll", Some(&roll(Some("Nikon FM2")))).unwrap();
    let body = |p: &str| index.row(p).unwrap().marker_body;
    assert_eq!(body("/negs/Roll/a.NEF").as_deref(), Some("Nikon FM2"));
    assert_eq!((body("/negs/Roll/Inner/b.NEF"), body("/negs/Rolled/c.NEF")), (None, None), "twin: a subfolder and a namesake are other folders");
}
