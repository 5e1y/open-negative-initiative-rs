//! The Swift app's self-checks on the archive, each on its own adversarial values and with its twin,
//! a variant built to fail when the property it guards is broken.

use oni_library::{Entry, LibraryIndex, library, relink};
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{Date, Film, PipelineSettings, roll_folder};
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("oni-library-swift-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    PathBuf::from(library::canonical(&dir))
}

// MARK: - Film

fn base() -> Date {
    Date::since_reference_date(8e8)
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
    // A link needs a privilege on Windows: the canonical path is checked where one can be made.
    #[cfg(unix)]
    {
        let real = sandbox.join("real");
        std::fs::create_dir_all(&real).unwrap();
        let link = sandbox.join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        std::fs::write(real.join("f.RW2"), [1]).unwrap();
        let (through, direct) = (link.join("f.RW2"), real.join("f.RW2"));
        assert_eq!(library::canonical(&through), library::canonical(&direct), "a link and its real path name one photo");
        // Twin: the two paths as written differ.
        assert_ne!(through, direct);
    }

    let raw = sandbox.join("P1072392.RW2");
    std::fs::write(&raw, vec![0x42; 4096]).unwrap();
    sidecar::write(&PipelineSettings::default(), &raw, WriteOptions { imported_at: Some(now), ..WriteOptions::default() }).unwrap();
    let mut edited = PipelineSettings::default();
    edited.gains.stops = [1.0, 0.0, 0.0];
    sidecar::write(&edited, &raw, WriteOptions::default()).unwrap();
    let after = sidecar::read(&raw, true).unwrap();
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
            let named = format!("{}: {target}", file.strip_prefix(crates).unwrap().display()).replace('\\', "/");
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

// MARK: - RollSettings.resetReachCheck

/// Two rolls in one folder and a frame on none: a frame picked reaches its whole roll, never the
/// neighbour's.
#[test]
fn a_reset_to_default_reaches_whole_rolls_never_a_neighbour() {
    use oni_library::roll_membership::reset_reach;
    use oni_settings::frame::RollReference;
    use std::collections::BTreeSet;
    let dir = scratch("reset-reach");
    let roll = || RollReference { id: oni_settings::Id::new(), stops: [0.0; 3], black: [0.0; 3], white: [1.0; 3], base: None, median: None };
    let (a, b) = (roll(), roll());
    let file = |name: &str, reference: Option<&RollReference>| -> String {
        let path = dir.join(name);
        std::fs::write(&path, b"").unwrap();
        let s = PipelineSettings { roll_reference: reference.cloned(), ..PipelineSettings::default() };
        sidecar::write(&s, &path, WriteOptions::default()).unwrap();
        path.to_string_lossy().into_owned()
    };
    let (a1, a2) = (file("a1.ARW", Some(&a)), file("a2.ARW", Some(&a)));
    let (b1, loose) = (file("b1.ARW", Some(&b)), file("loose.ARW", None));
    let library = vec![a1.clone(), a2.clone(), b1.clone(), loose.clone()];
    let set = |items: &[&String]| items.iter().map(|s| s.to_string()).collect::<BTreeSet<String>>();
    let from_frame = reset_reach(std::slice::from_ref(&a1), &library);
    assert_eq!(from_frame.frames, set(&[&a1, &a2]));
    assert_eq!(from_frame.references, BTreeSet::from([a.id]));
    // Twin, adverse by construction: a frame on no roll reaches itself alone, beside two rolls.
    let alone = reset_reach(std::slice::from_ref(&loose), &library);
    assert_eq!(alone.frames, set(&[&loose]));
    assert!(alone.references.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
