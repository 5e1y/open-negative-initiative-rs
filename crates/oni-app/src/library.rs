//! The archive the shell shows: the opened folder surveyed, its photos read through the SQLite
//! index, which is a disposable cache of their own files, and the sidebar's tree and the gallery's
//! groups cut from `oni-library`, never a second truth.

use oni_library::archive_tree::{self, Node, NodeId};
use oni_library::index::{self as library_index, LibraryIndex};
use oni_library::roots::Survey;
use oni_library::{Entry, folders, library, paths, relink, roots};
use oni_settings::Medal;
use oni_settings::film::film_stock;
use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, Instant};

/// `absorb`'s landing pace: rows read for the folder on screen reach the gallery this often.
pub const LANDING: Duration = Duration::from_millis(400);

/// One line of the sidebar, as the Slint `TreeRow` draws it.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub id: NodeId,
    pub depth: usize,
    pub title: String,
    pub caption: String,
    /// 0 a folder, 1 a roll, 2 a photo.
    pub kind: i32,
    pub expandable: bool,
    pub expanded: bool,
}

/// One band of the gallery: its title, its caption and its photos in order.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub title: String,
    pub caption: String,
    pub stock: String,
    /// The roll a folder's marker names, the one a click on the band writes.
    pub film: Option<oni_settings::Film>,
    pub entries: Vec<Entry>,
}

pub struct Archive {
    /// The folder's parent, so the folder itself is the tree's first node.
    pub root: String,
    pub nodes: Vec<Node>,
    pub groups: Vec<Group>,
    pub expanded: HashSet<NodeId>,
}

fn stem(path: &str) -> String {
    Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

fn photos(n: usize) -> String {
    if n == 1 { "1 photo".into() } else { format!("{n} photos") }
}

impl Archive {
    /// Surveys `folder` alone and reads every photo off its own files, through an index in memory.
    pub fn load(folder: &Path) -> Archive {
        let index = LibraryIndex::open(Path::new(":memory:")).expect("an index in memory");
        let mut last = None;
        read(folder, &index, None, &|| true, &mut |a, _, _| last = Some(a));
        last.expect("a read lands at least once")
    }

    /// The tree and groups of `folder`'s photos: `entries` are what the index shows, the survey the
    /// folders and rolls on the disk, so a folder made in the app shows before it holds a photo.
    /// `filter` keeps exactly one medal in the gallery, never a tier above, and leaves the tree whole.
    pub fn from_rows(folder: &str, entries: &[Entry], survey: &Survey, filter: Option<Medal>) -> Archive {
        let root = Path::new(folder).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| folder.to_string());
        let rolls = folders::rolls(&survey.images);
        let nodes = folders::tree(&root, entries, &rolls, &survey.folders, &survey.made);
        let filtered: Vec<Entry> = entries.iter().filter(|e| filter.is_none_or(|m| e.label == m)).cloned().collect();
        let groups = folders::groups(&root, &filtered, &rolls, &survey.made)
            .into_iter()
            .map(|g| {
                let title = g.film.as_ref().map(|f| f.name.clone()).or(g.heading.clone()).unwrap_or_default();
                let stock = g
                    .film
                    .as_ref()
                    .and_then(|f| film_stock(f.stock.as_deref()))
                    .map(|k| k.display_name.to_string())
                    .unwrap_or_default();
                Group { caption: photos(g.entries.len()), title, stock, film: g.film, entries: g.entries }
            })
            .collect();
        // The opened folder starts unfolded, its photos one click away.
        let expanded = nodes.iter().map(Node::id).collect();
        Archive { root, nodes, groups, expanded }
    }

    /// Every photo in gallery order.
    pub fn entries(&self) -> Vec<&Entry> {
        self.groups.iter().flat_map(|g| g.entries.iter()).collect()
    }

    pub fn rows(&self) -> Vec<Row> {
        archive_tree::flatten(&self.nodes, &self.expanded, 0)
            .into_iter()
            .map(|(node, depth)| {
                let id = node.id();
                let expanded = self.expanded.contains(&id);
                let count = node.count();
                match &node {
                    Node::Photo(e) => Row { id, depth, title: stem(&e.path), caption: medal(e.label), kind: 2, expandable: false, expanded },
                    Node::Folder { path, group, .. } => {
                        let roll = group.as_ref().and_then(|g| g.film.as_ref());
                        let title = roll.map(|f| f.name.clone()).unwrap_or_else(|| stem(path));
                        Row { id, depth, title, caption: count.to_string(), kind: if roll.is_some() { 1 } else { 0 }, expandable: count > 0, expanded }
                    }
                    Node::Roll { group, .. } | Node::Day { group, .. } => {
                        let title = group.film.as_ref().map(|f| f.name.clone()).or(group.heading.clone()).unwrap_or_default();
                        Row { id, depth, title, caption: count.to_string(), kind: 1, expandable: count > 0, expanded }
                    }
                }
            })
            .collect()
    }

    /// Opens or closes a row's node; a photo row answers its path instead.
    pub fn toggle(&mut self, id: &NodeId) -> Option<String> {
        if let NodeId::Photo(path) = id {
            return Some(path.clone());
        }
        if !self.expanded.remove(id) {
            self.expanded.insert(id.clone());
        }
        None
    }

    /// Records a label already written to the sidecar, so the tree and the tiles show it.
    pub fn set_label(&mut self, path: &str, label: Medal) {
        fn walk(nodes: &mut [Node], path: &str, label: Medal) {
            for n in nodes {
                match n {
                    Node::Photo(e) if e.path == path => e.label = label,
                    Node::Folder { group, children, .. } => {
                        walk(children, path, label);
                        for e in group.iter_mut().flat_map(|g| g.entries.iter_mut()).filter(|e| e.path == path) {
                            e.label = label;
                        }
                    }
                    Node::Roll { group, .. } | Node::Day { group, .. } => {
                        for e in group.entries.iter_mut().filter(|e| e.path == path) {
                            e.label = label;
                        }
                    }
                    _ => {}
                }
            }
        }
        walk(&mut self.nodes, path, label);
        for e in self.groups.iter_mut().flat_map(|g| g.entries.iter_mut()).filter(|e| e.path == path) {
            e.label = label;
        }
    }
}

/// What a landing hands the interface: the rows to lay, with the photos still unread as waiting
/// rows, whether the read is whole, and the survey they were cut on.
pub type Land<'a> = &'a mut dyn FnMut(Vec<Entry>, bool, &Survey);

/// `rescan` then `absorb` for one folder: the survey, rows a read covering the folder missed or
/// whose size moved dropped, then the members the index lacks read off their files. `land` gets the
/// rows once, again every `LANDING`, then whole with `true`; `alive` going false stops the read.
pub fn read_rows(folder: &str, index: &LibraryIndex, alive: &dyn Fn() -> bool, land: Land) {
    let folder = folder.to_string();
    let survey = roots::survey(std::slice::from_ref(&folder), alive);
    let _ = library::drop_invented(index);
    let kept: HashSet<&str> = survey.images.iter().map(String::as_str).collect();
    for row in index.all().into_iter().filter(|r| paths::holds(&folder, &r.path)) {
        let unread = survey.unreadable.iter().any(|u| paths::holds(u, &row.path));
        let stale = survey.sizes.get(&row.path).is_some_and(|size| relink::is_stale(&row.fingerprint, *size));
        if (!kept.contains(row.path.as_str()) && !unread) || stale {
            let _ = index.remove(&row.path);
        }
    }
    let shown = |index: &LibraryIndex, waiting: &[String]| -> Vec<Entry> {
        let mut entries = shown_under(index, &folder);
        entries.extend(waiting.iter().map(|p| library::waiting(p, survey.dated.get(p).copied())));
        entries
    };
    let mut waiting = library::unresolved(&survey.images, index);
    land(shown(index, &waiting), waiting.is_empty(), &survey);
    if waiting.is_empty() {
        return;
    }
    let mut landed = Instant::now();
    while !waiting.is_empty() && alive() {
        let path = waiting.remove(0);
        if let Some(entry) = library::indexed(&path) {
            let _ = index.upsert(&entry);
        }
        if landed.elapsed() >= LANDING && !waiting.is_empty() {
            land(shown(index, &waiting), false, &survey);
            landed = Instant::now();
        }
    }
    land(shown(index, &waiting), waiting.is_empty(), &survey);
}

/// `read_rows` cut into archives, for a caller with no interface between the landings.
pub fn read(folder: &Path, index: &LibraryIndex, filter: Option<Medal>, alive: &dyn Fn() -> bool, land: &mut dyn FnMut(Archive, bool, &Survey)) {
    let folder = paths::canonical(folder);
    read_rows(&folder, index, alive, &mut |rows, done, survey| land(Archive::from_rows(&folder, &rows, survey, filter), done, survey));
}

/// A row read again from its sidecar alone, its file's stamp kept: what a gesture on the sidecar
/// moves, without the file read a fingerprint costs.
pub fn refreshed(row: &Entry) -> Entry {
    let side = oni_settings::sidecar::url_for(Path::new(&row.path));
    let Some(envelope) = std::fs::read(&side).ok().and_then(|data| oni_settings::codec::envelope(&data)) else {
        return Entry { label: Medal::None, film: None, manual_order: None, pass_of: None, leads_passes: false, trashed_at: None, ..row.clone() };
    };
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

/// Re-reads `paths`' rows from their sidecars into the index; a path it does not hold is read whole.
pub fn refresh(index: &LibraryIndex, paths: &[String]) {
    let rows = index.all();
    for path in paths {
        let entry = match rows.iter().find(|r| r.path == *path) {
            Some(row) => refreshed(row),
            None => match library::indexed(path) {
                Some(e) => e,
                None => continue,
            },
        };
        let _ = index.upsert(&entry);
    }
}

/// The index's rows for the gallery, the ONI Trash's and the composed passes left out.
pub fn shown_under(index: &LibraryIndex, folder: &str) -> Vec<Entry> {
    library_index::shown(&index.all()).into_iter().filter(|e| paths::holds(folder, &e.path)).collect()
}

/// The medal as the tree's caption writes it.
pub fn medal(label: Medal) -> String {
    match label {
        Medal::None => String::new(),
        Medal::Bronze => "Bronze".into(),
        Medal::Silver => "Silver".into(),
        Medal::Gold => "Gold".into(),
    }
}

pub fn medal_index(label: Medal) -> i32 {
    match label {
        Medal::None => 0,
        Medal::Bronze => 1,
        Medal::Silver => 2,
        Medal::Gold => 3,
    }
}

pub fn medal_of(index: i32) -> Medal {
    match index {
        1 => Medal::Bronze,
        2 => Medal::Silver,
        3 => Medal::Gold,
        _ => Medal::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Roll #8's folder, when this machine has it.
    pub(crate) fn roll8() -> Option<std::path::PathBuf> {
        let dir = oni_fixtures::folder(oni_fixtures::RAW_DIR);
        dir.join("DSC_6440.NEF").exists().then_some(dir)
    }

    fn write(path: &Path, bytes: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn a_folder_read_twice_reads_its_files_once_and_drops_a_photo_gone() {
        let root = std::env::temp_dir().join(format!("oni-app-library-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let folder = root.join("Scans");
        for (name, bytes) in [("A.NEF", &b"first"[..]), ("B.NEF", b"second"), ("Sub/C.NEF", b"third")] {
            write(&folder.join(name), bytes);
        }
        let at = root.join("Caches").join("index.sqlite");
        let index = LibraryIndex::open(&at).unwrap();
        let mut lands: Vec<(Vec<String>, bool)> = Vec::new();
        read(&folder, &index, None, &|| true, &mut |a, done, _| lands.push((a.entries().iter().map(|e| e.fingerprint.clone()).collect(), done)));
        let (first, _) = &lands[0];
        assert_eq!(first.len(), 3);
        assert!(first.iter().all(|f| f.starts_with(library::WAITING_MARK)), "unread photos land as waiting rows: {first:?}");
        let (last, done) = lands.last().unwrap();
        assert!(*done && last.iter().all(|f| !f.starts_with(library::WAITING_MARK)), "{last:?}");
        // A second session finds every row in the index: one landing, whole, no file read again.
        let index = LibraryIndex::open(&at).unwrap();
        let mut again = Vec::new();
        read(&folder, &index, None, &|| true, &mut |a, done, _| again.push((a.entries().len(), done)));
        assert_eq!(again, vec![(3, true)]);
        // Twin: a photo gone from the disk leaves the index on the next read covering its folder.
        std::fs::remove_file(folder.join("B.NEF")).unwrap();
        read(&folder, &index, None, &|| true, &mut |_, _, _| {});
        let held: Vec<String> = index.all().into_iter().map(|e| paths::name(&e.path)).collect();
        assert_eq!(held.len(), 2, "{held:?}");
        assert!(!held.contains(&"B.NEF".to_string()));
        // A photo marked in the ONI Trash leaves every listing once its row is read again.
        let a = paths::canonical(&folder.join("A.NEF"));
        oni_library::trash::mark(std::slice::from_ref(&a), Some(oni_settings::Date::now()), &|_: &Path| true);
        refresh(&index, std::slice::from_ref(&a));
        let shown: Vec<String> = shown_under(&index, &paths::canonical(&folder)).into_iter().map(|e| e.path).collect();
        assert!(!shown.contains(&a) && shown.len() == 1, "{shown:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_reached_through_a_link_lists_the_photos_on_its_real_path() {
        let root = std::env::temp_dir().join(format!("oni-app-linked-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let real = root.join("Real").join("Scans");
        for name in ["A.NEF", "B.NEF"] {
            write(&real.join(name), name.as_bytes());
        }
        // `/tmp` is a link to `/private/tmp` on macOS: the folder opened by its link must still group.
        let link = root.join("Link");
        std::os::unix::fs::symlink(root.join("Real"), &link).unwrap();
        let through = link.join("Scans");
        let archive = Archive::load(&through);
        assert_eq!(archive.entries().len(), 2);
        assert!(archive.entries().iter().all(|e| e.path.starts_with(&paths::canonical(&real))));
        // Twin: the same rows laid under the link as written fall outside every group.
        let survey = roots::survey(&[paths::canonical(&real)], &|| true);
        let rows: Vec<Entry> = archive.entries().into_iter().cloned().collect();
        let raw = Archive::from_rows(&through.to_string_lossy(), &rows, &survey, None);
        assert!(raw.entries().is_empty(), "the twin must lose the photos the link hides");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn roll_8_opens_as_one_named_roll_of_its_photos() {
        let Some(dir) = roll8() else {
            eprintln!("roll #8 is absent: the archive is not exercised");
            return;
        };
        // Ten frames in the shared fixture, the whole roll's 38 and its contact sheet where a tester holds it.
        let n = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).filter(|e| { let n = e.file_name().to_string_lossy().into_owned(); !n.starts_with('.') && !n.ends_with(".json") }).count();
        let a = Archive::load(&dir);
        assert_eq!(a.groups.len(), 1);
        assert_eq!((a.groups[0].title.as_str(), a.groups[0].entries.len()), ("Roll #8", n));
        assert_eq!(a.groups[0].stock, "Fujifilm Fujicolor");
        let rows = a.rows();
        assert_eq!(rows[0].title, "Roll #8");
        assert_eq!((rows[0].kind, rows.len()), (1, n + 1), "the roll, then its photos");
        // Twin: folding the roll leaves its row alone.
        let mut folded = a;
        let id = folded.rows()[0].id.clone();
        folded.toggle(&id);
        assert_eq!(folded.rows().len(), 1);
    }
}
