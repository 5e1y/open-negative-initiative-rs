//! Each schema's own file: 1.5.6's `index.sqlite` beside this build's, neither touching the other's,
//! and nothing the shared file held that a read of the photos' own files and sidecars does not give again.

use super::*;
use crate::scratch::Scratch;
use crate::{library, paths};
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::PipelineSettings;

/// `index.sqlite` as 1.5.6 (eac39604) leaves it: schema 9, its photos table without `sheet`, one row.
fn shipped(at: &Path) {
    let raw = Connection::open(at).unwrap();
    raw.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(())).unwrap();
    raw.execute_batch(
        "CREATE TABLE photos (path TEXT PRIMARY KEY NOT NULL, imported_at REAL NOT NULL, captured_at REAL,
            fingerprint TEXT NOT NULL, label TEXT NOT NULL DEFAULT 'none', film_id TEXT, film_name TEXT,
            film_order REAL, film_stock TEXT, film_shot_date REAL, film_camera_body TEXT, manual_order REAL,
            mode TEXT, pass_of TEXT, leads_passes INTEGER NOT NULL DEFAULT 0, trashed_at REAL);
        CREATE TABLE stamps (path TEXT PRIMARY KEY NOT NULL, size INTEGER NOT NULL, modified REAL, sidecar REAL);
        CREATE TABLE rebuilt (at REAL NOT NULL);
        INSERT INTO photos (path, imported_at, fingerprint) VALUES ('/Roll 9/a.NEF', 8e8, '1-a');
        PRAGMA user_version = 9;",
    )
    .unwrap();
}

fn rows(at: &Path) -> i64 {
    Connection::open(at).unwrap().query_row("SELECT COUNT(*) FROM photos", [], |r| r.get(0)).unwrap()
}

#[test]
fn this_build_keeps_its_schema_in_its_own_file_and_leaves_1_5_6_s_as_written() {
    let scratch = Scratch::new("index-files");
    let caches = scratch.path();
    let theirs = caches.join("index.sqlite");
    shipped(&theirs);
    let written = std::fs::read(&theirs).unwrap();
    let ours = LibraryIndex::default_path(caches);
    assert_eq!(ours, caches.join(format!("index-{SCHEMA_VERSION}.sqlite")), "named by its schema");
    let index = LibraryIndex::open(&ours).unwrap();
    assert_eq!(index.count(), 0, "made as at a first launch");
    assert!(index.rebuilt().unwrap().elapsed().unwrap() < std::time::Duration::from_secs(60), "dated now, so no sweep trusts it yet");
    index.upsert(&Entry::new("/Roll 9/a.NEF", day(), "1-a")).unwrap();
    drop(index);
    assert_eq!(std::fs::read(&theirs).unwrap(), written, "1.5.6's file as 1.5.6 wrote it");
    assert_eq!(rows(&theirs), 1);
    assert_eq!(LibraryIndex::open(&ours).unwrap().count(), 1, "ours kept for the next launch");
    // Twin: one file shared, as before, the open of a schema that differs dropped 1.5.6's row.
    let former = Connection::open(&theirs).unwrap();
    former.execute_batch("DROP TABLE IF EXISTS photos; DROP TABLE IF EXISTS stamps; PRAGMA user_version = 10").unwrap();
    drop(former);
    assert_ne!(std::fs::read(&theirs).unwrap(), written);
}

fn day() -> Date {
    Date::since_reference_date(8e8)
}

/// The shared file at this schema, as a build of it left it, held each row as that build read it off the
/// photo's file and sidecar: a first read into the new file gives every column of every row again.
#[test]
fn a_read_into_the_new_file_gives_every_row_the_shared_one_held() {
    let scratch = Scratch::new("index-anew");
    let roll = scratch.path().join("Roll 9");
    std::fs::create_dir_all(&roll).unwrap();
    let mut members = Vec::new();
    for (name, label, trashed) in [("a.NEF", Medal::Gold, None), ("b.NEF", Medal::None, Some(day()))] {
        let frame = roll.join(name);
        std::fs::write(&frame, name).unwrap();
        let settings = PipelineSettings { film: Some(Film::new("Roll 9")), ..PipelineSettings::default() };
        sidecar::write(&settings, &frame, WriteOptions { imported_at: Some(day()), label: Some(label), trashed_at: Some(trashed), ..WriteOptions::default() }).unwrap();
        members.push(paths::canonical(&frame));
    }
    let read = |index: &LibraryIndex| members.iter().for_each(|m| index.upsert(&library::indexed(m).unwrap()).unwrap());
    let shared = LibraryIndex::open(&scratch.path().join("index.sqlite")).unwrap();
    read(&shared);
    let ours = LibraryIndex::open(&LibraryIndex::default_path(scratch.path())).unwrap();
    read(&ours);
    assert_eq!(ours.all(), shared.all());
    assert!(ours.all().iter().any(|e| e.label == Medal::Gold && e.film.is_some()) && ours.trashed_frames().len() == 1);
    // Twin: a value held in the index alone, which no sidecar carries, is what the new file would lack.
    shared.set_manual_order(Some(2.0), &members[0]).unwrap();
    assert_ne!(ours.all(), shared.all());
}
