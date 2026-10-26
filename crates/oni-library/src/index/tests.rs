use super::*;
use crate::scratch::Scratch;

fn day1() -> Date {
    Date::since_reference_date(800_000_000.0)
}

fn at(base: Date, seconds: f64) -> Date {
    crate::later(base, seconds)
}

fn roll_entries() -> Vec<Entry> {
    let day1 = day1();
    let batch = at(day1, 10_800.0);
    let day2 = at(day1, 86_400.0);
    let mut b = Entry::new("/negs/b.RW2", batch, "b-100-x");
    b.captured_at = Some(at(day1, 7200.0));
    let mut a = Entry::new("/negs/a.RW2", batch, "a-100-x");
    a.captured_at = Some(at(day1, 3600.0));
    let undated = Entry::new("/negs/sansdate.RW2", batch, "s-100-x");
    let mut c = Entry::new("/negs/c.RW2", day2, "c-100-x");
    c.captured_at = Some(at(day2, 60.0));
    let mut relinked = Entry::new("/negs/relinked.RW2", at(day2, 3600.0), "r-100-x");
    relinked.captured_at = Some(at(day1, 1800.0));
    vec![b, a, undated, c, relinked]
}

fn names(index: &LibraryIndex) -> Vec<String> {
    index.all().iter().map(|e| crate::paths::name(&e.path)).collect()
}

#[test]
fn the_index_orders_on_capture_and_rebuilds_whole() {
    let scratch = Scratch::new("index");
    let index = LibraryIndex::open(&scratch.path().join("index.sqlite")).unwrap();
    assert_eq!(index.count(), 0);
    let entries = roll_entries();
    for e in &entries {
        index.upsert(e).unwrap();
    }
    for e in &entries {
        index.upsert(e).unwrap();
    }
    assert_eq!(index.count(), 5, "a second import duplicates nothing");
    assert_eq!(names(&index), ["relinked.RW2", "a.RW2", "b.RW2", "sansdate.RW2", "c.RW2"]);
    index.remove("/negs/b.RW2").unwrap();
    assert_eq!(index.count(), 4);
    index.rebuild(&entries).unwrap();
    assert_eq!(index.all(), {
        let mut sorted = entries.clone();
        let key = |e: &Entry| crate::seconds(e.captured_at.unwrap_or(e.imported_at));
        sorted.sort_by(|a, b| key(a).total_cmp(&key(b)).then_with(|| a.path.cmp(&b.path)));
        sorted
    });

    index.set_label(Medal::Gold, "/negs/a.RW2").unwrap();
    let all = index.all();
    assert!(all.iter().all(|e| (e.label == Medal::Gold) == (e.path == "/negs/a.RW2")));

    let mut roll = Film::new("Portra 400");
    roll.stock = Some("kodak-portra".into());
    roll.shot_date = Some(Date::since_reference_date(700_000_000.0));
    index.set_film(Some(&roll), "/negs/a.RW2").unwrap();
    let joined = index.all().into_iter().find(|e| e.path == "/negs/a.RW2").unwrap();
    assert_eq!(joined.film.as_ref(), Some(&roll));
    // Twin: a namesake is another roll, so the id is what came back.
    assert_ne!(Some(&Film::new("Portra 400")), joined.film.as_ref());
    index.set_film(None, "/negs/a.RW2").unwrap();
    assert!(index.all().iter().all(|e| e.film.is_none()));

    let mut moded = entries.clone();
    moded[0].mode = Some(ConversionMode::Positive);
    moded[1].mode = Some(ConversionMode::Monochrome);
    index.rebuild(&moded).unwrap();
    let mode = |p: &str| index.all().into_iter().find(|e| e.path == p).unwrap().mode;
    assert_eq!(mode("/negs/b.RW2"), Some(ConversionMode::Positive));
    assert_eq!(mode("/negs/a.RW2"), Some(ConversionMode::Monochrome));
    assert_eq!(mode("/negs/c.RW2"), None);
}

#[test]
fn a_triplet_draws_as_its_lead_and_its_passes_show_once_it_is_gone() {
    let index = LibraryIndex::open(Path::new(":memory:")).unwrap();
    let mut lead = Entry::new("/rgb/0821.ARW", day1(), "a");
    lead.leads_passes = true;
    let pass = |p: &str, f: &str| {
        let mut e = Entry::new(p, day1(), f);
        e.pass_of = Some("0821.ARW".into());
        e
    };
    for e in [lead, pass("/rgb/0822.ARW", "b"), pass("/rgb/0823.ARW", "c"), pass("/other/0821-like.ARW", "d")] {
        index.upsert(&e).unwrap();
    }
    let drawn: HashSet<String> = index.shown().into_iter().map(|e| e.path).collect();
    assert_eq!(drawn, HashSet::from(["/other/0821-like.ARW".to_owned(), "/rgb/0821.ARW".to_owned()]));
    index.remove("/rgb/0821.ARW").unwrap();
    // Twin: with the lead gone, the passes show again.
    let drawn: HashSet<String> = index.shown().into_iter().map(|e| e.path).collect();
    assert_eq!(drawn.len(), 3);
}

/// A row with every column set to a value no other column holds.
fn full_row() -> Entry {
    let mut film = Film::new("Roll #8");
    film.manual_order = Some(-4.5);
    film.stock = Some("fujifilm-fujicolor".into());
    film.shot_date = Some(Date::since_reference_date(700_000_001.0));
    film.camera_body = Some("Nikon F3".into());
    Entry {
        path: "/negs/DSC_6432.NEF".into(),
        imported_at: Date::since_reference_date(812_000_002.0),
        captured_at: Some(Date::since_reference_date(811_000_003.0)),
        fingerprint: "30005388-2026:09:14 16:46:19-58c5f1206c22f7cb".into(),
        label: Medal::Silver,
        film: Some(film),
        manual_order: Some(7.25),
        mode: Some(ConversionMode::Monochrome),
        pass_of: Some("DSC_6431.NEF".into()),
        leads_passes: true,
        trashed_at: Some(Date::since_reference_date(813_000_004.0)),
    }
}

#[test]
fn every_column_reads_back_into_its_own_field() {
    let index = LibraryIndex::open(Path::new(":memory:")).unwrap();
    let row = full_row();
    index.upsert(&row).unwrap();
    assert_eq!(index.all(), std::slice::from_ref(&row));
    // Twin: the same row with two same-typed columns crossed is another row, so a crossed read fails above.
    let mut crossed = row.clone();
    crossed.manual_order = row.film.as_ref().and_then(|f| f.manual_order);
    crossed.film.as_mut().unwrap().manual_order = row.manual_order;
    assert_ne!(crossed, row);
    let mut swapped = row.clone();
    swapped.film.as_mut().unwrap().stock = row.film.as_ref().unwrap().camera_body.clone();
    swapped.film.as_mut().unwrap().camera_body = row.film.as_ref().unwrap().stock.clone();
    assert_ne!(swapped, row);

    // Dates are stored as seconds since 2001, as the Swift index stores them.
    let (imported, trashed): (f64, f64) = index
        .connection
        .query_row("SELECT imported_at, trashed_at FROM photos", [], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap();
    assert_eq!((imported, trashed), (812_000_002.0, 813_000_004.0));
    // Twin: the same instant in seconds since 1970 is not what the column holds.
    assert_ne!(imported, row.imported_at.unix_seconds());

    assert_eq!(index.in_trash().len(), 1, "a trashed row is the ONI Trash's");
    index.set_passes(None, false, &row.path).unwrap();
    index.set_trashed(None, &row.path).unwrap();
    assert!(index.in_trash().is_empty(), "twin: restored, it leaves the ONI Trash's rows");
    index.set_manual_order(None, &row.path).unwrap();
    let cleared = index.all().remove(0);
    assert_eq!(
        (cleared.pass_of, cleared.leads_passes, cleared.trashed_at, cleared.manual_order),
        (None, false, None, None)
    );
    assert_eq!(cleared.film, row.film, "a write to one column leaves the roll alone");
}

#[test]
fn a_schema_of_another_version_is_thrown_away() {
    let scratch = Scratch::new("index-version");
    let path = scratch.path().join("index.sqlite");
    {
        let index = LibraryIndex::open(&path).unwrap();
        index.upsert(&Entry::new("/a", day1(), "a")).unwrap();
    }
    // Twin: reopened at the same version, the row is still there.
    assert_eq!(LibraryIndex::open(&path).unwrap().count(), 1);
    Connection::open(&path).unwrap().execute_batch("PRAGMA user_version = 99").unwrap();
    assert_eq!(LibraryIndex::open(&path).unwrap().count(), 0);
}

/// Rows an earlier build keyed as the disk listed them take the archive's spelling at the next open:
/// a row alone is respelt, two rows for one file become one holding what either held.
#[cfg(any(target_os = "macos", windows))]
#[test]
fn an_index_row_keyed_in_another_spelling_is_respelt_and_never_lost() {
    let scratch = Scratch::new("index-spelling");
    let file = scratch.path().join("index.sqlite");
    let stale = |name: &str| {
        if cfg!(windows) { format!("C:\\Scans\\\u{c9}t\u{e9}\\{name}") } else { format!("/Scans/E\u{301}te\u{301}/{name}") }
    };
    let spelt = |name: &str| if cfg!(windows) { format!("C:/Scans/\u{c9}t\u{e9}/{name}") } else { format!("/Scans/\u{c9}t\u{e9}/{name}") };
    let index = LibraryIndex::open(&file).unwrap();
    index.upsert(&Entry::new(&spelt("b.NEF"), day1(), "b-1-x")).unwrap();
    drop(index);
    let raw = Connection::open(&file).unwrap();
    let insert = "INSERT INTO photos (path, imported_at, fingerprint, label, manual_order, trashed_at) VALUES (?, 0, ?, ?, ?, NULL)";
    raw.execute(insert, params![stale("b.NEF"), "b-1-x", "gold", 2.0]).unwrap();
    raw.execute(insert, params![stale("c.NEF"), "c-1-x", "silver", Option::<f64>::None]).unwrap();
    // Twin: keyed byte for byte, the stale row is not found under the spelling the survey hands.
    let found: i64 = raw.query_row("SELECT COUNT(*) FROM photos WHERE path = ?", [spelt("c.NEF")], |r| r.get(0)).unwrap();
    assert_eq!(found, 0);
    drop(raw);
    let index = LibraryIndex::open(&file).unwrap();
    let all = index.all();
    let by = |name: &str| all.iter().find(|e| e.path == spelt(name)).cloned().unwrap();
    assert_eq!(all.len(), 2, "one row per file, none lost: {all:?}");
    assert_eq!((by("b.NEF").label, by("b.NEF").manual_order), (Medal::Gold, Some(2.0)));
    assert_eq!(by("c.NEF").label, Medal::Silver);
    index.set_label(Medal::Bronze, &stale("c.NEF")).unwrap();
    assert_eq!(index.all().iter().find(|e| e.path == spelt("c.NEF")).unwrap().label, Medal::Bronze);
}

/// The Swift app shares the file and sets no busy handler: a write of ours meeting its lock waits.
#[test]
fn a_write_waits_for_the_other_apps_write() {
    let scratch = Scratch::new("index-busy");
    let path = scratch.path().join("index.sqlite");
    let index = LibraryIndex::open(&path).unwrap();
    let hold = |raw: Connection, held: std::sync::mpsc::Sender<()>| {
        std::thread::spawn(move || {
            raw.execute_batch("BEGIN IMMEDIATE; INSERT INTO photos (path, imported_at, fingerprint) VALUES ('/swift', 0, 's')").unwrap();
            held.send(()).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(300));
            raw.execute_batch("COMMIT").unwrap();
        })
    };
    let (held, taken) = std::sync::mpsc::channel();
    let other = hold(Connection::open(&path).unwrap(), held);
    taken.recv().unwrap();
    index.upsert(&Entry::new("/ours", day1(), "o")).unwrap();
    other.join().unwrap();
    assert_eq!(index.count(), 2);
    let check: String = Connection::open(&path).unwrap().query_row("PRAGMA integrity_check", [], |r| r.get(0)).unwrap();
    assert_eq!(check, "ok");
    // Twin: with no busy handler, as Swift opens it, the same write is refused at once.
    let (held, taken) = std::sync::mpsc::channel();
    let raw = Connection::open(&path).unwrap();
    raw.execute_batch("DELETE FROM photos").unwrap();
    let other = hold(raw, held);
    taken.recv().unwrap();
    let bare = Connection::open(&path).unwrap();
    bare.busy_timeout(std::time::Duration::ZERO).unwrap();
    let refused = bare.execute("INSERT INTO photos (path, imported_at, fingerprint) VALUES ('/bare', 0, 'b')", []);
    other.join().unwrap();
    assert!(refused.is_err());
}

#[test]
fn the_trash_lists_a_triplet_once_whichever_folder_holds_it() {
    let index = LibraryIndex::open(Path::new(":memory:")).unwrap();
    let when = at(day1(), 1e7);
    let mut lead = Entry::new("/A/t-r.ARW", day1(), "r");
    lead.leads_passes = true;
    index.upsert(&lead).unwrap();
    for (path, print) in [("/A/t-g.ARW", "g"), ("/A/t-b.ARW", "b")] {
        let mut pass = Entry::new(path, day1(), print);
        pass.pass_of = Some("t-r.ARW".into());
        index.upsert(&pass).unwrap();
    }
    index.upsert(&Entry::new("/B/kept.RW2", day1(), "k")).unwrap();
    for path in ["/A/t-r.ARW", "/A/t-g.ARW", "/A/t-b.ARW"] {
        index.set_trashed(Some(when), path).unwrap();
    }
    let listed: Vec<String> = index.trashed_frames().into_iter().map(|e| e.path).collect();
    assert_eq!(listed, ["/A/t-r.ARW"], "a triplet as its lead");
    assert_eq!(listed, trashed(&index.all()).into_iter().map(|e| e.path).collect::<Vec<_>>());

    // The lead restored while its passes' rows still carry the mark: no pass is listed alone.
    index.set_trashed(None, "/A/t-r.ARW").unwrap();
    assert!(index.trashed_frames().is_empty());
    // Twin: cut on the trashed rows alone, the passes would stand in the trash without their lead.
    let alone: Vec<String> = trashed(&index.in_trash()).into_iter().map(|e| e.path).collect();
    assert_eq!(alone.len(), 2, "{alone:?}");
}

#[test]
fn a_schema_drop_dates_the_rebuild_and_a_plain_open_keeps_its_date() {
    let scratch = Scratch::new("rebuilt");
    let at = scratch.path().join("index.sqlite");
    let index = LibraryIndex::open(&at).unwrap();
    let made = index.rebuilt().expect("a file made here is dated");
    assert!(made.elapsed().unwrap() < std::time::Duration::from_secs(60));
    let long_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(10 * 86_400);
    index.note_rebuilt(long_ago).unwrap();
    drop(index);
    // Twin: an open at the same version throws nothing away and keeps the date.
    let index = LibraryIndex::open(&at).unwrap();
    let kept = index.rebuilt().unwrap();
    assert!(kept.duration_since(long_ago).unwrap_or_default() < std::time::Duration::from_secs(1), "{kept:?}");
    index.connection.execute_batch(&format!("PRAGMA user_version = {}", SCHEMA_VERSION - 1)).unwrap();
    drop(index);
    let index = LibraryIndex::open(&at).unwrap();
    assert!(index.rebuilt().unwrap().elapsed().unwrap() < std::time::Duration::from_secs(60), "the drop is dated now");
}
