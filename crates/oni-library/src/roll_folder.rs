//! A roll made on the disk the Finder's way: a folder beside the photos, the photos moved in with
//! their sidecars, and the same moves walked back. The disk is the truth; the app only reads it.

use crate::folders::{self, name};
use crate::gallery_history::{Disk, Move};
use crate::{paths, rgb_passes};
use chrono::{DateTime, Local};
use oni_i18n::trn;
use oni_settings::{Date, Film, roll_folder, sidecar};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// The earliest capture as year, month, day in local time, numbered past the names taken beside it.
pub fn name_for(earliest: Date, parent: &str) -> String {
    let base = dated(earliest);
    let mut name = base.clone();
    let mut rank = 2;
    while Path::new(&paths::join(parent, &name)).exists() {
        name = format!("{base} {rank}");
        rank += 1;
    }
    name
}

pub fn dated(date: Date) -> String {
    DateTime::from_timestamp(date.unix_seconds().floor() as i64, 0)
        .unwrap_or_default()
        .with_timezone(&Local)
        .format("%Y-%m-%d")
        .to_string()
}

/// The folder made and marked a roll; an error when the disk refuses, leaving nothing behind.
pub fn make(name: &str, parent: &str, film: &Film) -> std::io::Result<String> {
    roll_folder::make(name, Path::new(parent), film).map(|p| crate::paths::normal(&p.to_string_lossy()).into_owned())
}

/// Each photo to its new path with its sidecar; a path already taken keeps that photo where it
/// was. The moves made, in order.
pub fn move_items(moves: &[Move]) -> Vec<Move> {
    moves
        .iter()
        .filter(|m| {
            let (from, to) = (Path::new(&m.from), Path::new(&m.to));
            let (side_from, side_to) = (sidecar::url_for(from), sidecar::url_for(to));
            if to.exists() {
                return false;
            }
            // The settings go first and come back if the photo cannot follow, so a photo never
            // lands without them.
            let carries = side_from.exists();
            if carries && std::fs::rename(&side_from, &side_to).is_err() {
                return false;
            }
            if std::fs::rename(from, to).is_err() {
                if carries {
                    let _ = std::fs::rename(&side_to, &side_from);
                }
                return false;
            }
            true
        })
        .cloned()
        .collect()
}

/// Every item back where it was, then the roll's marker and the folder the gesture made, only
/// once nothing else lies in it: a file added there meanwhile keeps the folder.
pub fn take_back(disk: &Disk) -> (Vec<Move>, bool) {
    let reversed: Vec<Move> =
        disk.moves.iter().rev().map(|m| Move { from: m.to.clone(), to: m.from.clone() }).collect();
    let back = move_items(&reversed);
    let Some(folder) = &disk.folder else { return (back, false) };
    let left: Vec<String> = std::fs::read_dir(folder)
        .map(|l| l.filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned())).collect())
        .unwrap_or_default();
    if !left.iter().all(|n| n == folders::ROLL_MARKER) {
        return (back, false);
    }
    let _ = folders::set_roll(None, folder);
    // Only an empty folder goes: `remove_dir` refuses one that still holds anything.
    let removed = std::fs::remove_dir(folder).is_ok();
    (back, removed)
}

/// What an undo or a redo of a roll says for the `missed` items it could not move, as Swift's `takeBack`.
pub fn shortfall(missed: usize, undoing: bool) -> String {
    if undoing {
        trn(
            "One item could not be put back: its folder moved or a file took its name.",
            "{n} of the items could not be put back: their folder moved or a file took their name.",
            missed as u64,
        )
    } else {
        trn(
            "One item could not be moved into the roll again: its folder moved or a file took its name.",
            "{n} of the items could not be moved into the roll again: their folder moved or a file took their name.",
            missed as u64,
        )
    }
}

pub fn lay(disk: &Disk) -> Vec<Move> {
    if let Some(folder) = &disk.folder {
        let _ = std::fs::create_dir(folder);
        let _ = folders::set_roll(disk.film.as_ref(), folder);
    }
    move_items(&disk.moves)
}

/// Each item into `folder`, a name taken there or by another item of the batch numbered as the
/// Finder numbers a copy: `Tokyo 2`, `P107 2.RW2`.
pub fn moves(items: &[String], folder: &str) -> Vec<Move> {
    let mut claimed: HashSet<String> = HashSet::new();
    let exists = |n: &str| Path::new(&paths::join(folder, n)).exists();
    // A pass keeps its name or stays: its frame finds the other two by name, so one taken name
    // keeps the whole triplet where it is.
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for item in items {
        if let Some(group) = rgb_passes::group(item) {
            groups.entry(item.clone()).or_insert(group);
        }
    }
    let blocked: HashSet<String> =
        groups.values().filter(|g| g.iter().any(|p| exists(&name(p)))).flatten().cloned().collect();
    items
        .iter()
        .filter_map(|item| {
            if groups.contains_key(item) {
                if blocked.contains(item) {
                    return None;
                }
                claimed.insert(name(item));
                return Some(Move { from: item.clone(), to: paths::join(folder, &name(item)) });
            }
            let file = name(item);
            let ext = if Path::new(item).is_dir() { String::new() } else { paths::extension(&file) };
            let stem = if ext.is_empty() { file.clone() } else { paths::stem(&file) };
            let mut free = file.clone();
            let mut rank = 2;
            while claimed.contains(&free) || exists(&free) {
                free = if ext.is_empty() { format!("{stem} {rank}") } else { format!("{stem} {rank}.{ext}") };
                rank += 1;
            }
            claimed.insert(free.clone());
            Some(Move { from: item.clone(), to: paths::join(folder, &free) })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;
    use oni_settings::PipelineSettings;
    use oni_settings::frame::RgbPasses;
    use oni_settings::sidecar::WriteOptions;

    fn touch(path: &str, bytes: &[u8]) {
        std::fs::create_dir_all(paths::parent(path)).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    fn exists(path: &str) -> bool {
        Path::new(path).exists()
    }

    fn side(path: &str) -> String {
        crate::paths::normal(&sidecar::url_for(Path::new(path)).to_string_lossy()).into_owned()
    }

    #[test]
    fn a_roll_folder_takes_its_photos_and_gives_them_back() {
        let scratch = Scratch::new("roll-folder");
        let parent = crate::paths::normal(&scratch.path().to_string_lossy()).into_owned();
        let photos: Vec<String> = ["a.RW2", "b.tif"].iter().map(|n| paths::join(&parent, n)).collect();
        for p in &photos {
            touch(p, p.as_bytes());
            touch(&side(p), b"{}");
        }
        let day = crate::relink::date("2026:01:14 12:00:00").unwrap();
        std::fs::create_dir(paths::join(&parent, "2026-01-14")).unwrap();
        let name = name_for(day, &parent);
        assert_eq!(name, "2026-01-14 2");

        let film = Film::new(&name);
        let folder = make(&name, &parent, &film).unwrap();
        let made = move_items(&moves(&photos, &folder));
        assert_eq!(made.len(), 2);
        assert!(made.iter().all(|m| exists(&m.to) && exists(&side(&m.to))));
        assert_eq!(folders::roll(&folder).map(|f| f.id), Some(film.id));
        assert!(!photos.iter().any(|p| exists(p)));

        let disk = Disk { folder: Some(folder.clone()), film: Some(film.clone()), moves: made };
        let (back, removed) = take_back(&disk);
        assert_eq!(back.len(), 2);
        assert!(photos.iter().all(|p| exists(p) && exists(&side(p))) && removed && !exists(&folder));

        let redone = lay(&disk);
        assert_eq!(redone.len(), 2);
        assert_eq!(folders::roll(&folder).map(|f| f.id), Some(film.id));
        // Twin: a file put in the folder since keeps it, the photos still go back.
        let kept = paths::join(&folder, "notes.txt");
        touch(&kept, b"mine");
        let (again, removed) = take_back(&disk);
        assert!(!removed && exists(&kept) && again.len() == 2);
    }

    #[test]
    fn a_taken_name_is_numbered_as_the_finder_numbers_a_copy() {
        let scratch = Scratch::new("roll-folder-names");
        let root = crate::paths::normal(&scratch.path().to_string_lossy()).into_owned();
        let p = |r: &str| paths::join(&root, r);
        for dir in ["Dest/Trip", "Elsewhere", "Other", "Trip"] {
            std::fs::create_dir_all(p(dir)).unwrap();
        }
        for file in ["Dest/x.RW2", "Elsewhere/x.RW2", "Elsewhere/y.RW2", "Other/y.RW2", "Trip/t.RW2"] {
            touch(&p(file), file.as_bytes());
        }
        touch(&side(&p("Trip/t.RW2")), b"{}");
        let items = [p("Elsewhere/x.RW2"), p("Elsewhere/y.RW2"), p("Other/y.RW2"), p("Trip")];
        let planned = moves(&items, &p("Dest"));
        assert_eq!(planned.iter().map(|m| name(&m.to)).collect::<Vec<_>>(), ["x 2.RW2", "y.RW2", "y 2.RW2", "Trip 2"]);
        // Twin: kept on their own names, two collide and one is taken.
        let plain: Vec<String> = items.iter().map(|i| paths::join(&p("Dest"), &name(i))).collect();
        assert!(plain.iter().collect::<HashSet<_>>().len() < plain.len() && plain.iter().any(|x| exists(x)));
        let carried = move_items(&planned);
        assert_eq!(carried.len(), 4);
        assert!(exists(&p("Dest/Trip 2/t.RW2")) && exists(&side(&p("Dest/Trip 2/t.RW2"))));
        let (back, removed) = take_back(&Disk { moves: carried, ..Disk::default() });
        assert!(back.len() == 4 && !removed && exists(&p("Dest/x.RW2")) && items.iter().all(|i| exists(i)));
    }

    #[test]
    fn a_triplet_moves_whole_on_its_names_or_stays_whole() {
        let scratch = Scratch::new("roll-folder-passes");
        let root = crate::paths::normal(&scratch.path().to_string_lossy()).into_owned();
        let p = |r: &str| paths::join(&root, r);
        for dir in ["Passes", "Clear", "Dest"] {
            std::fs::create_dir_all(p(dir)).unwrap();
        }
        let triplet = [p("Passes/b.ARW"), p("Passes/g.ARW"), p("Passes/r.ARW")];
        for f in &triplet {
            touch(f, f.as_bytes());
        }
        let lead = PipelineSettings {
            rgb_passes: Some(RgbPasses {
                red: "r.ARW".into(),
                green: "g.ARW".into(),
                blue: "b.ARW".into(),
                registration: None,
            }),
            ..PipelineSettings::default()
        };
        sidecar::write(&lead, Path::new(&triplet[0]), WriteOptions::default()).unwrap();
        for f in &triplet[1..] {
            let pass = PipelineSettings { pass_of: Some("b.ARW".into()), ..PipelineSettings::default() };
            sidecar::write(&pass, Path::new(f), WriteOptions::default()).unwrap();
        }
        let order = vec![triplet[2].clone(), triplet[1].clone(), triplet[0].clone()];
        assert_eq!(rgb_passes::expanded(&[triplet[0].clone()]), order);
        assert_eq!(rgb_passes::expanded(&[triplet[2].clone()]), order);
        touch(&p("Dest/g.ARW"), b"taken");
        assert!(moves(&triplet, &p("Dest")).is_empty(), "one taken name keeps the three where they are");
        // Twin: with every name free, the three move on their own names.
        let clear = moves(&triplet, &p("Clear"));
        assert_eq!(clear.iter().map(|m| name(&m.to)).collect::<Vec<_>>(), ["b.ARW", "g.ARW", "r.ARW"]);
    }

    /// A photo whose settings cannot follow it stays with them; the old order moved the photo first
    /// and left it without its settings. A stale file in the settings' place is replaced, as before.
    #[test]
    fn a_photo_never_lands_without_its_settings() {
        let scratch = Scratch::new("roll-folder-sidecars");
        let root = crate::paths::normal(&scratch.path().to_string_lossy()).into_owned();
        let (a, b) = (paths::join(&root, "a.RW2"), paths::join(&root, "b.RW2"));
        for (p, settings) in [(&a, &b"{\"a\": 1}"[..]), (&b, &b"{\"b\": 1}"[..])] {
            touch(p, b"frame");
            touch(&side(p), settings);
        }
        let into = paths::join(&root, "Roll");
        // A folder where a's settings would go, which no rename may replace; a stale file where b's go.
        std::fs::create_dir_all(side(&paths::join(&into, "a.RW2"))).unwrap();
        touch(&side(&paths::join(&into, "b.RW2")), b"{\"stale\": 1}");
        let asked = [Move { from: a.clone(), to: paths::join(&into, "a.RW2") }, Move { from: b.clone(), to: paths::join(&into, "b.RW2") }];
        let done = move_items(&asked);
        assert!(exists(&a) && std::fs::read(side(&a)).unwrap() == b"{\"a\": 1}", "a stayed with its settings");
        assert!(!exists(&paths::join(&into, "a.RW2")), "twin: a never landed without them");
        assert_eq!(done.len(), 1);
        assert_eq!(std::fs::read(side(&paths::join(&into, "b.RW2"))).unwrap(), b"{\"b\": 1}", "b took its settings");
    }
}
