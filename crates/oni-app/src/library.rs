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
    /// The heading's trail, the folders above it, and the name its band is headed by.
    pub caption: String,
    pub header: String,
    pub stock: String,
    /// The roll a folder's marker names, the one a click on the band writes.
    pub film: Option<oni_settings::Film>,
    /// The folder the band lays, which renaming its heading renames.
    pub folder: Option<String>,
    pub entries: Vec<Entry>,
}

pub struct Archive {
    /// The folder's parent, so the folder itself is the tree's first node.
    pub root: String,
    pub nodes: Vec<Node>,
    pub groups: Vec<Group>,
    pub expanded: HashSet<NodeId>,
}

use oni_library::paths::stem;

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
        let root = Some(oni_library::paths::parent(folder)).filter(|p| p != folder).unwrap_or_else(|| folder.to_string());
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
                // `RollHeaderContent`: the folders above the roll, then its own name.
                let (caption, header) = (g.trail().unwrap_or_default(), g.heading_title());
                let folder = match g.id {
                    oni_library::film_group::FilmKey::Folder(path) => Some(path),
                    _ => None,
                };
                Group { caption, header, title, stock, film: g.film, folder, entries: g.entries }
            })
            .collect();
        // `openFolder`: every folder under the opened one starts unfolded, the opened one included.
        let mut expanded = HashSet::new();
        folder_ids(&nodes, &mut expanded);
        Archive { root, nodes, groups, expanded }
    }

    /// `expandAncestors`: every folder above `path` up to the opened one unfolds; answers whether
    /// a row was added.
    pub fn unfold_to(&mut self, path: &str) -> bool {
        let mut added = false;
        let mut folder = paths::parent(path);
        while folder != self.root && paths::holds(&self.root, &folder) {
            added |= self.expanded.insert(NodeId::Folder(folder.clone()));
            let up = paths::parent(&folder);
            if up == folder {
                break;
            }
            folder = up;
        }
        added
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

fn folder_ids(nodes: &[Node], out: &mut HashSet<NodeId>) {
    for node in nodes {
        if let Node::Folder { children, .. } = node {
            out.insert(node.id());
            folder_ids(children, out);
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
mod tests;
