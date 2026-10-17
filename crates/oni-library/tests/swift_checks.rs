//! The Swift app's self-checks on the archive, each on its own adversarial values and with its twin,
//! a variant built to fail when the property it guards is broken.

use oni_library::film_group::{self, FilmGroup, FilmKey};
use oni_library::{Entry, LibraryIndex, library, relink};
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{Date, Film, PipelineSettings, roll_folder};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("oni-library-swift-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    PathBuf::from(library::canonical(&dir))
}

// MARK: - Film.selfCheck

fn base() -> Date {
    Date::since_reference_date(8e8)
}

fn at(hours: f64, path: &str, film: Option<&Film>) -> Entry {
    let mut e = Entry::new(path, base(), path);
    e.captured_at = Some(oni_library::later(base(), hours * 3600.0));
    e.film = film.cloned();
    e
}

fn placing(film: Option<&Film>, paths: &[&str], entries: &[Entry]) -> Vec<Entry> {
    entries
        .iter()
        .map(|e| {
            let mut copy = e.clone();
            if paths.contains(&e.path.as_str()) {
                copy.film = film.cloned();
            }
            copy
        })
        .collect()
}

fn paths(group: &FilmGroup) -> Vec<&str> {
    group.entries.iter().map(|e| e.path.as_str()).collect()
}

#[test]
fn rolls_take_their_frames_out_of_the_days_and_clearing_one_gives_them_back() {
    let all = [at(1.0, "a", None), at(2.0, "b", None), at(3.0, "c", None), at(30.0, "d", None), at(31.0, "e", None)];
    let by_day = film_group::grouped(&all);
    assert!(by_day.len() == 2 && paths(&by_day[0]) == ["a", "b", "c"] && paths(&by_day[1]) == ["d", "e"]);
    assert!(by_day.iter().all(|g| !g.is_explicit()));

    let roll = Film::new("Portra 400");
    let crossed = film_group::grouped(&placing(Some(&roll), &["b", "d"], &all));
    assert_eq!(crossed.len(), 3, "forming a roll leaves what it did not take");
    let made = crossed.iter().position(|g| g.is_explicit()).unwrap();
    assert_eq!(paths(&crossed[made]), ["b", "d"]);
    let starts: Vec<f64> = crossed.iter().map(|g| g.start.time_interval_since_reference_date()).collect();
    assert!(starts.windows(2).all(|w| w[0] <= w[1]), "the feed stays in capture order");
    // A roll follows the day that still starts before it.
    assert!(made == 1 && paths(&crossed[0]) == ["a", "c"] && paths(&crossed[2]) == ["e"]);

    let cleared = film_group::grouped(&placing(None, &["b", "d"], &placing(Some(&roll), &["b", "d"], &all)));
    assert_eq!(cleared, by_day, "clearing the roll gives every frame back to its day");
    // Twin: two rolls sharing a name are two rolls, identity being the id.
    let twin = Film::new("Portra 400");
    let named = film_group::grouped(&placing(Some(&twin), &["e"], &placing(Some(&roll), &["a"], &all)));
    assert_eq!(named.iter().filter(|g| g.is_explicit()).count(), 2);

    // Equal starts are decided by the first frame, or the sort shuffles the feed between passes.
    let tied = film_group::grouped(&[at(1.0, "a", Some(&roll)), at(1.0, "b", None), at(2.0, "c", None)]);
    assert!(tied.len() == 2 && tied[0].start == tied[1].start && paths(&tied[0])[0] == "a");
}

#[test]
fn a_roll_is_offered_the_lowest_free_number() {
    assert_eq!(film_group::untitled(&[]), "Roll 1");
    assert_eq!(film_group::untitled(&[Film::new("Roll 1"), Film::new("Portra 400")]), "Roll 2");
    assert_eq!(film_group::untitled(&[Film::new("Roll 1"), Film::new("Roll 3")]), "Roll 2");
    // Twin: a count of rolls would answer 3 here and clash.
    assert_eq!(film_group::untitled(&[Film::new("Roll 2"), Film::new("Roll 3")]), "Roll 1");
}

#[test]
fn the_day_is_computed_once_per_frame() {
    let many: Vec<Entry> =
        (0..2000).map(|i| Entry::new(&format!("/perf/{i}.RW2"), oni_library::later(base(), i as f64), &format!("p{i}"))).collect();
    // Twin: the same buckets keyed twice per frame, the cost this cut must not pay.
    let twice = |entries: &[Entry]| {
        let mut buckets: HashMap<FilmKey, Vec<Entry>> = HashMap::new();
        for entry in entries {
            let first = film_group::key(entry);
            if first != film_group::key(entry) {
                continue;
            }
            buckets.entry(first).or_default().push(entry.clone());
        }
        buckets
    };
    let best = |f: &dyn Fn()| {
        (0..9)
            .map(|_| {
                let start = Instant::now();
                f();
                start.elapsed().as_secs_f64()
            })
            .fold(f64::MAX, f64::min)
    };
    let once = best(&|| drop(film_group::grouped(&many)));
    let doubled = best(&|| drop(twice(&many)));
    println!("grouped {:.2} ms, keyed twice {:.2} ms, {:.2}x", once * 1e3, doubled * 1e3, doubled / once);
    assert!(doubled / once.max(1e-9) >= 1.3, "keying twice costs only {:.2}x the cut", doubled / once);
}

#[test]
fn a_hand_placed_frame_reorders_inside_its_roll_and_never_walks_the_roll() {
    let mut reordered = vec![at(1.0, "x", None), at(2.0, "y", None), at(3.0, "z", None)];
    reordered[2].manual_order = Some(film_group::order(&reordered[0]) - 1000.0);
    let regrouped = film_group::grouped(&reordered);
    assert!(regrouped.len() == 1 && paths(&regrouped[0]) == ["z", "x", "y"]);
    // Twin: the roll's place still reads the earliest capture, not the frame moved ahead.
    assert_eq!(regrouped[0].start, film_group::capture(&reordered[0]));

    assert_eq!(film_group::behind(100.0, Some(50.0)), 50.0);
    assert_eq!(film_group::behind(100.0, Some(100.0)), 100.0 - 86_400.0);
    assert_eq!(film_group::behind(100.0, Some(150.0)), 100.0 - 86_400.0);
    assert_eq!(film_group::behind(100.0, None), 100.0 - 86_400.0);

    let same: Vec<Entry> = ["a", "b", "c", "d", "e"].iter().map(|p| at(0.0, p, None)).collect();
    let untied = film_group::detied(&same, 1);
    assert_eq!(untied.iter().map(|u| u.0.as_str()).collect::<Vec<_>>(), ["a", "b"]);
    assert!(untied[0].1 < untied[1].1 && untied[1].1 < film_group::order(&same[1]));
    assert!(film_group::detied(&same, 0).is_empty());
    let staggered = [at(-1.0, "a", None), at(0.0, "b", None), at(1.0, "c", None)];
    assert!(film_group::detied(&staggered, 2).is_empty(), "a distinct predecessor costs nothing");
}

#[test]
fn a_dragged_roll_leads_the_feed_and_a_day_reads_its_own_start() {
    let group = |film: Option<Film>, start: f64| FilmGroup {
        id: film.as_ref().map(|f| FilmKey::Roll(f.id)).unwrap_or(FilmKey::Day(0)),
        film,
        start: oni_library::later(base(), start),
        entries: Vec::new(),
        heading: None,
    };
    let earlier = group(Some(Film::new("Earlier")), 3600.0);
    let mut later_roll = Film::new("Later");
    let later = group(Some(later_roll.clone()), 7200.0);
    assert!(film_group::root_order(&earlier) < film_group::root_order(&later));
    later_roll.manual_order = Some(film_group::root_order(&earlier) - 1.0);
    let moved = FilmGroup { film: Some(later_roll), ..later.clone() };
    assert!(film_group::root_order(&moved) < film_group::root_order(&earlier));
    // Twin: a day has no roll to carry an override.
    let day = group(None, 1.0);
    assert_eq!(film_group::root_order(&day), day.start.time_interval_since_reference_date());

    let untouched = Film::new("Untouched");
    let mut pulled = Film::new("Pulled ahead");
    let before = film_group::grouped(&[at(1.0, "p", Some(&untouched)), at(2.0, "q", Some(&pulled))]);
    assert_eq!(paths(&before[0])[0], "p");
    pulled.manual_order = Some(film_group::root_order(&before[0]) - 1.0);
    let after = film_group::grouped(&[at(1.0, "p", Some(&untouched)), at(2.0, "q", Some(&pulled))]);
    assert_eq!(paths(&after[0])[0], "q", "a dragged roll leads `grouped`'s own output");
}

#[test]
fn a_rolls_three_inspector_fields_move_equality_apart_and_round_trip() {
    let bare = Film::new("Bare");
    let with_stock = Film { stock: Some("kodak-gold-200".into()), ..bare.clone() };
    let with_date = Film { shot_date: Some(base()), ..bare.clone() };
    let with_body = Film { camera_body: Some("Leica M6".into()), ..bare.clone() };
    assert!(with_stock != bare && with_date != bare && with_body != bare);
    // Twin: the three fields are independent, not one flag.
    assert!(with_stock != with_date && with_date != with_body && with_stock != with_body);
    assert_eq!(roll_folder::decode(roll_folder::encode(&with_stock).as_bytes()), Some(with_stock));
}

// MARK: - Library.selfCheck

#[test]
fn a_disk_that_is_away_invents_nothing_and_plugging_it_back_is_the_repair() {
    let sandbox = scratch("library");
    let now = Date::since_reference_date(800_000_000.0);
    let real = sandbox.join("real");
    std::fs::create_dir_all(&real).unwrap();
    let link = sandbox.join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    std::fs::write(real.join("f.RW2"), [1]).unwrap();
    let (through, direct) = (link.join("f.RW2"), real.join("f.RW2"));
    assert_eq!(library::canonical(&through), library::canonical(&direct), "a link and its real path name one photo");
    // Twin: the two paths as written differ.
    assert_ne!(through, direct);

    let raw = sandbox.join("P1072392.RW2");
    std::fs::write(&raw, vec![0x42; 4096]).unwrap();
    sidecar::write(&PipelineSettings::default(), &raw, WriteOptions { imported_at: Some(now), ..WriteOptions::default() }).unwrap();
    let mut edited = PipelineSettings::default();
    edited.gains.stops = [1.0, 0.0, 0.0];
    sidecar::write(&edited, &raw, WriteOptions::default()).unwrap();
    let after = sidecar::read(&raw, true, true).unwrap();
    assert!(after.imported_at == Some(now) && after.settings == edited, "a later setting keeps the import date");

    let away = sandbox.join("partie2.RW2");
    std::fs::write(&away, vec![0x37; 2048]).unwrap();
    let away_path = library::canonical(&away);
    sidecar::write(&PipelineSettings::default(), &away, WriteOptions { imported_at: Some(now), ..WriteOptions::default() }).unwrap();
    // The original and its sidecar both, as an unplugged disk takes both.
    let (hidden, hidden_side) = (sandbox.join("cachee.bin"), sandbox.join("c.bin"));
    std::fs::rename(&away, &hidden).unwrap();
    std::fs::rename(sidecar::url_for(&away), &hidden_side).unwrap();
    assert!(library::indexed(&away_path).is_none(), "an unreachable original yields no row");
    // Twin: the retired shape wrote a row whatever happened, with a mark that lets it heal.
    let invented = Entry::new(&away_path, oni_library::distant_past(), &format!("{}{away_path}", library::INVENTED_MARK));
    assert!(relink::fingerprint(&away).is_none() && invented.fingerprint.starts_with(library::INVENTED_MARK));

    let index = LibraryIndex::open(&sandbox.join("repair.sqlite")).unwrap();
    let rows: Vec<Entry> = [away_path.clone()].iter().filter_map(|p| library::indexed(p)).collect();
    index.rebuild(&rows).unwrap();
    assert!(index.count() == 0 && library::unresolved(std::slice::from_ref(&away_path), &index) == [away_path.clone()]);

    std::fs::rename(&hidden, &away).unwrap();
    std::fs::rename(&hidden_side, sidecar::url_for(&away)).unwrap();
    for path in library::unresolved(std::slice::from_ref(&away_path), &index) {
        if let Some(row) = library::indexed(&path) {
            index.upsert(&row).unwrap();
        }
    }
    let repaired = index.all();
    assert!(index.count() == 1 && repaired[0].imported_at == now);
    assert!(Some(&repaired[0].fingerprint) == relink::fingerprint(&away).as_ref() && !repaired[0].fingerprint.starts_with(library::INVENTED_MARK));
    assert!(library::unresolved(&[away_path], &index).is_empty(), "nothing is left owing");
}

// MARK: - OniTrash.sweepCheck

/// The names a photograph, its sheet or its sidecar goes by.
const MEMBER_NAMES: [&str; 25] = [
    "url", "urls", "path", "paths", "original", "originals", "photo", "photos", "member", "members", "sheet", "sheets",
    "frame", "frames", "file", "files", "target", "targets", "lead", "pass", "passes", "entry", "entries", "source",
    "sources",
];

/// Deletions of a file that is no member, by source and target: a transfer's part file.
const SCRATCH: [&str; 1] = ["oni-localsend/src/inbox.rs: &self.path"];

const DELETIONS: [&str; 4] = ["remove_file(", "remove_dir_all(", "remove_dir(", "trash::delete("];

/// Every deletion in `text` whose first argument names a member; a sidecar's path is not one.
fn member_deletions(text: &str) -> Vec<String> {
    let code: String = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut out = Vec::new();
    let mut rest = code.as_str();
    while let Some((at, call)) = DELETIONS.iter().filter_map(|d| rest.find(d).map(|i| (i, *d))).min_by_key(|(i, _)| *i) {
        let body = &rest[at + call.len()..];
        let mut depth = 0;
        let mut end = body.len();
        for (i, c) in body.char_indices() {
            match c {
                '(' => depth += 1,
                ')' if depth == 0 => {
                    end = i;
                    break;
                }
                ')' => depth -= 1,
                ',' if depth == 0 => {
                    end = i;
                    break;
                }
                _ => {}
            }
        }
        let target = body[..end].trim().to_string();
        let bare = target.replace("sidecar::url_for(", "(").replace("url_for(", "(");
        let mut names = Vec::new();
        let chars: Vec<char> = bare.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i].is_ascii_alphabetic() || chars[i] == '_' {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                let called = chars[i..].iter().find(|c| !c.is_whitespace()) == Some(&'(');
                if !called {
                    names.push(word);
                }
            } else {
                i += 1;
            }
        }
        if names.iter().any(|n| MEMBER_NAMES.contains(&n.as_str())) {
            out.push(target);
        }
        rest = &body[end..];
    }
    out
}

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_crate_trashes_or_removes_a_library_member() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut files = Vec::new();
    for krate in std::fs::read_dir(crates).unwrap().flatten() {
        sources(&krate.path().join("src"), &mut files);
    }
    let mut offenders = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap();
        for target in member_deletions(&text) {
            let named = format!("{}: {target}", file.strip_prefix(crates).unwrap().display());
            if !SCRATCH.contains(&named.as_str()) {
                offenders.push(named);
            }
        }
    }
    assert!(files.len() > 50, "only {} sources scanned", files.len());
    assert!(offenders.is_empty(), "{offenders:?}");
    // Twin: the lines a system-trash route would be written in, spelt in pieces so no source holds them.
    let (trash, remove) = ("trash::".to_owned() + "delete(", "std::fs::".to_owned() + "remove_file(");
    let retired = format!(
        "if {trash}&url).is_err() {{ continue; }}\n{remove}Path::new(&path))?;\nlet _ = std::fs::remove_dir_all(&sandbox);\nlet _ = {remove}sidecar::url_for(&away));"
    );
    assert_eq!(member_deletions(&retired), ["&url", "Path::new(&path)"]);
}
