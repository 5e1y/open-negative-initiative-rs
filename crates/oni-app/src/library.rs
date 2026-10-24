//! The archive the shell shows: the opened folder surveyed, its photos read through the SQLite
//! index, which is a disposable cache of their own files, and the sidebar's tree and the gallery's
//! groups cut from `oni-library`, never a second truth.

use oni_library::archive_tree::{self, Node, NodeId};
use oni_library::index::LibraryIndex;
use oni_library::roots::Survey;
use oni_library::film_group::FilmGroup;
use oni_library::{Entry, folders, paths};
use oni_i18n::tr;
use oni_settings::Medal;
use oni_settings::film::film_stock;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

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
    /// `TreeRowChrome.defaultStock`: a roll's stock, picked or its mode's generic one.
    pub stock: Option<&'static str>,
}

/// `TreeRowChrome.defaultStock`: the stock a hand picked for the roll, else its first photo's mode's.
pub fn default_stock(group: &FilmGroup) -> &'static str {
    let picked = group.film.as_ref().and_then(|f| film_stock(f.stock.as_deref())).map(|k| k.id);
    let mode = group.entries.first().and_then(|e| e.mode).unwrap_or(oni_settings::ConversionMode::Negative);
    picked.unwrap_or_else(|| oni_settings::film::default_stock_id(mode))
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
    /// Each RAW whose JPEG of a RAW+JPEG pair stands behind it, by path: one tile, marked.
    pub companions: HashMap<String, String>,
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
        let root = Some(oni_library::paths::parent(folder)).filter(|p| p != folder).unwrap_or_else(|| folder.to_string());
        let rolls = &survey.rolls;
        let (entries, companions) = folders::paired(entries, &survey.images, &survey.sidecars);
        let entries = &entries[..];
        let nodes = folders::tree(&root, entries, rolls, &survey.folders, &survey.made);
        let filtered: Vec<Entry> = entries.iter().filter(|e| filter.is_none_or(|m| e.label == m)).cloned().collect();
        let groups = folders::groups(&root, &filtered, rolls, &survey.made)
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
        Archive { root, nodes, groups, expanded, companions }
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

    /// Every node a hand left folded, a folder at any depth included.
    pub fn folded(&self) -> HashSet<NodeId> {
        let mut folders = HashSet::new();
        folder_ids(&self.nodes, &mut folders);
        self.nodes.iter().map(Node::id).chain(folders).filter(|id| !self.expanded.contains(id)).collect()
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
                // `FilmRow.title`: a photo's whole file name, a folder's own name, extensions kept.
                match &node {
                    Node::Photo(e) => Row { id, depth, title: paths::name(&e.path), caption: medal(e.label), kind: 2, expandable: false, expanded, stock: None },
                    Node::Folder { path, group, .. } => {
                        let roll = group.as_ref().filter(|g| g.film.is_some());
                        let title = roll.and_then(|g| g.film.as_ref()).map(|f| f.name.clone()).unwrap_or_else(|| paths::name(path));
                        Row { id, depth, title, caption: count.to_string(), kind: if roll.is_some() { 1 } else { 0 }, expandable: count > 0, expanded, stock: roll.map(default_stock) }
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

/// The medal as the tree's caption writes it.
pub fn medal(label: Medal) -> String {
    match label {
        Medal::None => String::new(),
        Medal::Bronze => tr("Bronze").into(),
        Medal::Silver => tr("Silver").into(),
        Medal::Gold => tr("Gold").into(),
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

mod reading;
pub use reading::{Land, Place, read_rows, read, refreshed, refresh, shown_under};
pub mod tidy;

#[cfg(test)]
mod tests;
