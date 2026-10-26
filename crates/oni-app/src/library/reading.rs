//! `rescan` then `absorb`: a folder surveyed, its rows checked against what the disk says of each
//! file, and the members the index lacks read off their files, landing as they come.

use super::{Archive, LANDING};
use oni_library::index::{self as library_index, LibraryIndex, Stamped};
use oni_library::navigator::Computer;
use oni_library::roots::Survey;
use oni_library::{Entry, kept, library, paths, relink, roots};
use oni_settings::{Date, Medal};
use std::collections::{HashSet, VecDeque};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

/// What a landing hands the interface: the rows to lay, with the photos still unread as waiting
/// rows, whether the read is whole, and the survey they were cut on, shared by every landing.
pub type Land<'a> = &'a mut dyn FnMut(Vec<Entry>, bool, &Arc<Survey>);

/// What a read knows beyond its folder: the volumes that tell offline from missing, and the
/// caches' folder where each folder's last shape is kept.
pub struct Place<'a> {
    pub computer: &'a Computer,
    pub kept: Option<&'a Path>,
}

/// A row whose file or sidecar moved since it was written, looked at again after the first landing.
enum Again {
    /// The original changed date or size kept: stamped again, read whole when its print moved.
    Original(Entry),
    /// The sidecar changed outside this app, or went: its envelope read again.
    Sidecar(Entry),
}

/// What the survey says of `path` now, the stamp its row is checked against.
fn stamped(survey: &Survey, path: &str) -> Option<Stamped> {
    let at = |d: &Date| d.time_interval_since_reference_date();
    Some(Stamped {
        size: *survey.sizes.get(path)?,
        modified: survey.dated.get(path).map(at),
        sidecar: survey.sidecars.get(path).map(at),
    })
}

/// One folder surveyed, rows gone or resized dropped, those moved looked at again, the members missing
/// read; `land` gets the rows once, every `LANDING`, then whole; `alive` false stops the read.
pub fn read_rows(folder: &str, index: &LibraryIndex, place: &Place, alive: &dyn Fn() -> bool, land: Land) {
    let folder = folder.to_string();
    let mut survey = roots::surveyed(std::slice::from_ref(&folder), alive, place.computer);
    // A read superseded drops nothing: what its walk did not reach is unknown, never gone.
    if survey.stopped || !alive() {
        return;
    }
    if let Some(dir) = place.kept {
        let _ = kept::keep(dir, &folder, &survey);
        kept::worn(dir, &folder, &mut survey);
    }
    let survey = Arc::new(survey);
    let _ = library::drop_invented(index);
    let listed: HashSet<&str> = survey.images.iter().map(String::as_str).collect();
    let stamps = index.stamps_under(&folder);
    let (mut again, mut adopted) = (VecDeque::new(), Vec::new());
    // A row under a folder the disk refused is kept, offline or missing alike: a disk away loses no row.
    for row in index.under(&folder) {
        // Superseded, the survey may predate a move whose rows were just written under new names.
        if !alive() {
            return;
        }
        let unread = survey.unreadable.iter().any(|u| paths::holds(u, &row.path));
        let stale = survey.sizes.get(&row.path).is_some_and(|size| relink::is_stale(&row.fingerprint, *size));
        if (!listed.contains(row.path.as_str()) && !unread) || stale {
            let _ = index.remove(&row.path);
            continue;
        }
        // A file still being written keeps its row as it was until it rests.
        let Some(now) = stamped(&survey, &row.path).filter(|_| !survey.settling.contains(&row.path)) else { continue };
        match stamps.get(&row.path) {
            // A row written before its stamp was kept takes the disk's word once.
            None => adopted.push((row.path.clone(), now)),
            Some(was) if was.original_moved(now.size, now.modified) => again.push_back(Again::Original(row)),
            Some(was) if was.sidecar_moved(now.sidecar) => again.push_back(Again::Sidecar(row)),
            Some(_) => {}
        }
    }
    let _ = index.note_stamps(&adopted);
    let mut unreadable: Vec<String> = Vec::new();
    let shown = |index: &LibraryIndex, waiting: &VecDeque<String>, unreadable: &[String]| -> Vec<Entry> {
        let mut entries = shown_under(index, &folder);
        entries.extend(waiting.iter().chain(unreadable).map(|p| library::waiting(p, survey.dated.get(p).copied())));
        entries
    };
    let held = index.paths_under(&folder);
    let mut waiting: VecDeque<String> = survey.images.iter().filter(|m| !held.contains(*m)).cloned().collect();
    land(shown(index, &waiting, &unreadable), waiting.is_empty() && again.is_empty(), &survey);
    if waiting.is_empty() && again.is_empty() {
        return;
    }
    let mut landed = Instant::now();
    let mut noted: Vec<(String, Stamped)> = Vec::new();
    while alive() {
        if let Some(path) = waiting.pop_front() {
            // A file that cannot be read whole, that the cloud keeps away or that is still being
            // written, waits with no print.
            let read = if survey.settling.contains(&path) { None } else { library::indexed(&path) };
            match read {
                Some(entry) => {
                    let _ = index.upsert(&entry);
                    noted.extend(stamped(&survey, &path).map(|s| (path, s)));
                }
                None => unreadable.push(path),
            }
        } else if let Some(row) = again.pop_front() {
            noted.extend(look_again(index, row, &survey));
        } else {
            break;
        }
        if landed.elapsed() >= LANDING && !(waiting.is_empty() && again.is_empty()) {
            let _ = index.note_stamps(&std::mem::take(&mut noted));
            land(shown(index, &waiting, &unreadable), false, &survey);
            landed = Instant::now();
        }
    }
    let _ = index.note_stamps(&noted);
    land(shown(index, &waiting, &unreadable), waiting.is_empty() && again.is_empty(), &survey);
}

/// A row whose file or sidecar moved, read again: a print that holds keeps the row, one that moved
/// reads it whole. The stamp to keep, `None` while the file cannot be read.
fn look_again(index: &LibraryIndex, again: Again, survey: &Survey) -> Option<(String, Stamped)> {
    let row = match again {
        Again::Sidecar(row) => {
            let _ = index.upsert(&refreshed(&row));
            row
        }
        Again::Original(row) => {
            let print = relink::stamp(Path::new(&row.path))?;
            if print.fingerprint == row.fingerprint {
                let _ = index.upsert(&refreshed(&row));
            } else {
                let _ = index.upsert(&library::indexed(&row.path)?);
            }
            row
        }
    };
    stamped(survey, &row.path).map(|s| (row.path, s))
}

/// `read_rows` cut into archives, for a caller with no interface between the landings.
pub fn read(folder: &Path, index: &LibraryIndex, filter: Option<Medal>, alive: &dyn Fn() -> bool, land: &mut dyn FnMut(Archive, bool, &Survey)) {
    let folder = paths::canonical(folder);
    let place = Place { computer: &Computer::none(), kept: None };
    read_rows(&folder, index, &place, alive, &mut |rows, done, survey| land(Archive::from_rows(&folder, &rows, survey, filter), done, survey));
}

/// A row read again from its sidecar alone, its file's stamp kept: what a gesture on the sidecar
/// moves, without the file read a fingerprint costs. A row whose sidecar cannot be seen stays as it was.
pub fn refreshed(row: &Entry) -> Entry {
    let original = Path::new(&row.path);
    let envelope = match std::fs::read(oni_settings::sidecar::url_for(original)) {
        Ok(data) => oni_settings::sidecar::identity::envelope(&data, original),
        // No sidecar beside a present original is a photo at rest; beside a missing one, a disk away.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && original.exists() => {
            return Entry { label: Medal::None, film: None, manual_order: None, pass_of: None, leads_passes: false, trashed_at: None, ..row.clone() };
        }
        Err(_) => None,
    };
    let Some(envelope) = envelope else { return row.clone() };
    Entry {
        label: envelope.label,
        film: envelope.film,
        manual_order: envelope.manual_order,
        mode: Some(envelope.mode),
        pass_of: envelope.pass_of,
        leads_passes: envelope.leads_passes,
        trashed_at: envelope.trashed_at,
        ..row.clone()
    }
}

/// Re-reads `paths`' rows from their sidecars into the index, each row found by its key and all
/// written in one transaction; a path it does not hold is read whole.
pub fn refresh(index: &LibraryIndex, paths: &[String]) {
    let entries: Vec<Entry> = paths
        .iter()
        .filter_map(|path| match index.row(path) {
            Some(row) => Some(refreshed(&row)),
            None => library::indexed(path),
        })
        .collect();
    let _ = index.upsert_all(&entries);
}

/// The index's rows for the gallery, the ONI Trash's and the composed passes left out: a pass's
/// lead lies beside it, so the folder's rows alone tell which passes it composes.
pub fn shown_under(index: &LibraryIndex, folder: &str) -> Vec<Entry> {
    library_index::shown(&index.under(folder))
}
