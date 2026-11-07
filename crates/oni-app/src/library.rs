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
use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
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
    /// A photo's place on its roll from 1, as the contact sheet numbers it.
    pub number: Option<usize>,
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
    expanded: HashSet<NodeId>,
    /// Each RAW whose JPEG of a RAW+JPEG pair stands behind it, by path: one tile, marked.
    pub companions: HashMap<String, String>,
    basis: Basis,
    /// The rows as last laid flat, kept until a fold or a medal moves them.
    laid: OnceCell<Vec<Row>>,
}

fn band(g: FilmGroup) -> Group {
    let title = g.film.as_ref().map(|f| f.name.clone()).or(g.heading.clone()).unwrap_or_default();
    let stock = g.film.as_ref().and_then(|f| film_stock(f.stock.as_deref())).map(|k| k.display_name.to_string()).unwrap_or_default();
    // `RollHeaderContent`: the folders above the roll, then its own name.
    let (caption, header) = (g.trail().unwrap_or_default(), g.heading_title());
    let folder = match g.id {
        oni_library::film_group::FilmKey::Folder(path) => Some(path),
        _ => None,
    };
    Group { caption, header, title, stock, film: g.film, folder, entries: g.entries }
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
    /// `filter` keeps exactly one medal, never a tier above, in the gallery and the tree's rows alike.
    pub fn from_rows(folder: &str, entries: &[Entry], survey: &Arc<Survey>, filter: Option<Medal>) -> Archive {
        let root = Some(oni_library::paths::parent(folder)).filter(|p| p != folder).unwrap_or_else(|| folder.to_string());
        let (paired, companions) = folders::paired(entries, &survey.images, &survey.sidecars);
        let nodes = folders::tree(&root, &paired, &survey.rolls, &survey.folders, &survey.made);
        let basis = Basis { folder: folder.to_owned(), rows: entries.to_vec(), survey: survey.clone(), filter };
        let mut archive = Archive { root, nodes, groups: Vec::new(), expanded: HashSet::new(), companions, basis, laid: OnceCell::new() };
        archive.groups = archive.cut_groups(&paired);
        // `openFolder`: every folder under the opened one starts unfolded, the opened one included.
        archive.unfold_every();
        archive
    }

    /// The gallery's bands under the basis's filter: the tree's own groups unfiltered, else those
    /// of the photos wearing the medal alone.
    fn cut_groups(&self, paired: &[Entry]) -> Vec<Group> {
        let survey = &self.basis.survey;
        let groups = match self.basis.filter {
            None => folders::groups_of(&self.root, &self.nodes, &survey.rolls),
            Some(m) => {
                let filtered: Vec<Entry> = paired.iter().filter(|e| e.label == m).cloned().collect();
                folders::groups(&self.root, &filtered, &survey.rolls, &survey.made)
            }
        };
        groups.into_iter().map(band).collect()
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
        if added {
            self.laid.take();
        }
        added
    }

    /// Every folder unfolded, the opened one included, as a folder just opened lays them.
    pub fn unfold_every(&mut self) {
        let mut every = HashSet::new();
        folder_ids(&self.nodes, &mut every);
        self.set_expanded(every);
    }

    /// The nodes unfolded, folders at any depth.
    pub fn expanded(&self) -> &HashSet<NodeId> {
        &self.expanded
    }

    pub fn set_expanded(&mut self, expanded: HashSet<NodeId>) {
        if self.expanded != expanded {
            self.expanded = expanded;
            self.laid.take();
        }
    }

    /// One node unfolded more.
    pub fn unfold(&mut self, id: NodeId) {
        if self.expanded.insert(id) {
            self.laid.take();
        }
    }

    /// The nodes in `folded` folded again, as a hand left them.
    pub fn keep_folded(&mut self, folded: &HashSet<NodeId>) {
        let before = self.expanded.len();
        self.expanded.retain(|id| !folded.contains(id));
        if self.expanded.len() != before {
            self.laid.take();
        }
    }

    /// Every node a hand left folded, a folder at any depth included.
    pub fn folded(&self) -> HashSet<NodeId> {
        let mut folders = HashSet::new();
        folder_ids(&self.nodes, &mut folders);
        self.nodes.iter().map(Node::id).chain(folders).filter(|id| !self.expanded.contains(id)).collect()
    }

    /// Each roll folder's stock as its tree row draws it (`default_stock`), by path.
    pub fn roll_stocks(&self) -> HashMap<String, &'static str> {
        let mut out = HashMap::new();
        roll_stocks(&self.nodes, &mut out);
        out
    }

    /// A folder's photos in shooting order, whatever medal the gallery keeps: the roll its frames
    /// are numbered on.
    pub fn folder_photos(&self, folder: &str) -> Vec<String> {
        own(&self.nodes, folder).map(|g| g.entries.iter().map(|e| e.path.clone()).collect()).unwrap_or_default()
    }

    /// `folder`'s own photos as a band of the whole tree, whatever medal the gallery keeps: the
    /// roll a folder is, which a filter never cuts; the root's own are the photos loose in it.
    pub fn band(&self, folder: &str) -> Option<Group> {
        if !paths::same(folder, &self.root) {
            return own(&self.nodes, folder).cloned().map(band);
        }
        let loose: Vec<Entry> = self.nodes.iter().filter_map(|n| if let Node::Photo(e) = n { Some(e.clone()) } else { None }).collect();
        let roll = self.basis.survey.rolls.get(&self.root).cloned();
        (!loose.is_empty()).then(|| band(folders::group(&self.root, &self.root, loose, roll)))
    }

    /// `path`'s place on its roll from 1 and the roll's count of frames, as the tree and the contact
    /// sheet number it; a photo loose beside the folders has none.
    pub fn place(&self, path: &str) -> Option<(usize, usize)> {
        let roll = own(&self.nodes, &paths::parent(path))?;
        roll.entries.iter().position(|e| e.path == path).map(|i| (i + 1, roll.entries.len()))
    }

    /// Every photo in gallery order.
    pub fn entries(&self) -> Vec<&Entry> {
        self.groups.iter().flat_map(|g| g.entries.iter()).collect()
    }

    /// The rows laid flat, laid again only once a fold or the tree moved.
    pub fn laid(&self) -> &[Row] {
        self.laid.get_or_init(|| self.lay())
    }

    /// The row at `i` as laid.
    pub fn row(&self, i: usize) -> Option<&Row> {
        self.laid().get(i)
    }

    pub fn rows(&self) -> Vec<Row> {
        self.laid().to_vec()
    }

    /// Whether the filter shows `entry`: every photo, or those wearing its one medal.
    pub fn shows(&self, entry: &Entry) -> bool {
        self.basis.filter.is_none_or(|m| entry.label == m)
    }

    /// The first photo a node's row stands for under the filter: a photo itself, a folder's own
    /// first shown before its children's.
    pub fn first_shown(&self, id: &NodeId) -> Option<String> {
        fn first(a: &Archive, node: &Node) -> Option<String> {
            match node {
                Node::Photo(e) => a.shows(e).then(|| e.path.clone()),
                Node::Folder { group, children, .. } => group
                    .iter()
                    .flat_map(|g| &g.entries)
                    .find(|e| a.shows(e))
                    .map(|e| e.path.clone())
                    .or_else(|| children.iter().find_map(|c| first(a, c))),
            }
        }
        first(self, archive_tree::find(&self.nodes, id)?)
    }

    /// The photos under `node` the filter shows, summed as `Node::count` sums them all.
    fn shown_count(&self, node: &Node) -> usize {
        match node {
            Node::Photo(e) => usize::from(self.shows(e)),
            Node::Folder { group, children, .. } => {
                children.iter().map(|c| self.shown_count(c)).sum::<usize>() + group.iter().flat_map(|g| &g.entries).filter(|e| self.shows(e)).count()
            }
        }
    }

    fn lay(&self) -> Vec<Row> {
        ROW_LISTS.fetch_add(1, Ordering::Relaxed);
        let mut rows = Vec::new();
        archive_tree::walk(&self.nodes, &self.expanded, 0, &mut |laid, depth| {
            if let archive_tree::Laid::Photo(e, _) = laid
                && !self.shows(e)
            {
                return;
            }
            // `FilmRow.title`: a photo's whole file name, a folder's own name, extensions kept.
            rows.push(match laid {
                archive_tree::Laid::Photo(e, number) => Row {
                    id: NodeId::Photo(e.path.clone()),
                    depth,
                    title: paths::name(&e.path),
                    caption: medal(e.label),
                    number,
                    kind: 2,
                    expandable: false,
                    expanded: false,
                    stock: None,
                },
                archive_tree::Laid::Folder(node) => {
                    let id = node.id();
                    let Node::Folder { path, group, .. } = node else { unreachable!("a folder's row lends its folder") };
                    let count = self.shown_count(node);
                    let roll = group.as_ref().filter(|g| g.film.is_some());
                    let title = roll.and_then(|g| g.film.as_ref()).map(|f| f.name.clone()).unwrap_or_else(|| paths::name(path));
                    let expanded = self.expanded.contains(&id);
                    Row { id, depth, title, caption: count.to_string(), number: None, kind: if roll.is_some() { 1 } else { 0 }, expandable: count > 0, expanded, stock: roll.map(default_stock) }
                }
            });
        });
        rows
    }

    /// Opens or closes a row's node; a photo row answers its path instead.
    pub fn toggle(&mut self, id: &NodeId) -> Option<String> {
        if let NodeId::Photo(path) = id {
            return Some(path.clone());
        }
        if !self.expanded.remove(id) {
            self.expanded.insert(id.clone());
        }
        self.laid.take();
        None
    }

}

/// `folder`'s own photos, found at any depth: the roll a folder is.
fn own<'a>(nodes: &'a [Node], folder: &str) -> Option<&'a FilmGroup> {
    nodes.iter().find_map(|node| match node {
        Node::Folder { path, group, .. } if paths::same(path, folder) => group.as_ref(),
        Node::Folder { children, .. } => own(children, folder),
        Node::Photo(_) => None,
    })
}

fn roll_stocks(nodes: &[Node], out: &mut HashMap<String, &'static str>) {
    for node in nodes {
        if let Node::Folder { path, group, children } = node {
            if let Some(group) = group.as_ref().filter(|g| g.film.is_some()) {
                out.insert(path.clone(), default_stock(group));
            }
            roll_stocks(children, out);
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

/// The medal's place on the podium, the figure struck in it and the key that lays it: gold 1, silver
/// 2, bronze 3, none 0.
pub fn medal_index(label: Medal) -> i32 {
    match label {
        Medal::None => 0,
        Medal::Gold => 1,
        Medal::Silver => 2,
        Medal::Bronze => 3,
    }
}

pub fn medal_of(index: i32) -> Medal {
    match index {
        1 => Medal::Gold,
        2 => Medal::Silver,
        3 => Medal::Bronze,
        _ => Medal::None,
    }
}

mod reading;
mod relaid;
pub use relaid::{Relaid, cuts};
use relaid::{Basis, ROW_LISTS};
pub use reading::{Land, Place, read_rows, read, refreshed, refresh, shown_under};
pub mod tidy;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod filtered;
#[cfg(test)]
mod bench;
