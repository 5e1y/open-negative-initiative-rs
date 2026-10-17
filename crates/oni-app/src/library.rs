//! The archive the shell shows: one folder surveyed, its photos indexed from their own files, the
//! sidebar's tree and the gallery's groups cut from `oni-library`, never a second truth.

use oni_library::archive_tree::{self, Node, NodeId};
use oni_library::{Entry, folders, library, roots};
use oni_settings::Medal;
use oni_settings::film::film_stock;
use std::collections::HashSet;
use std::path::Path;

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
    /// Surveys `folder` alone, so a roll opened from a crowded parent reads nothing beside it.
    pub fn load(folder: &Path) -> Archive {
        let folder = folder.to_string_lossy().trim_end_matches('/').to_string();
        let root = Path::new(&folder).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| folder.clone());
        let survey = roots::survey(std::slice::from_ref(&folder), &|| true);
        let entries: Vec<Entry> = survey.images.iter().filter_map(|p| library::indexed(p)).collect();
        let rolls = folders::rolls(&survey.images);
        let nodes = folders::tree(&root, &entries, &rolls, &survey.folders, &survey.made);
        let groups = folders::groups(&root, &entries, &rolls, &survey.made)
            .into_iter()
            .map(|g| {
                let title = g.film.as_ref().map(|f| f.name.clone()).or(g.heading.clone()).unwrap_or_default();
                let stock = g
                    .film
                    .as_ref()
                    .and_then(|f| film_stock(f.stock.as_deref()))
                    .map(|k| k.display_name.to_string())
                    .unwrap_or_default();
                Group { caption: photos(g.entries.len()), title, stock, entries: g.entries }
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

    #[test]
    fn roll_8_opens_as_one_named_roll_of_ten_photos() {
        let Some(dir) = roll8() else {
            eprintln!("roll #8 is absent: the archive is not exercised");
            return;
        };
        let a = Archive::load(&dir);
        assert_eq!(a.groups.len(), 1);
        assert_eq!((a.groups[0].title.as_str(), a.groups[0].entries.len()), ("Roll #8", 10));
        assert_eq!(a.groups[0].stock, "Fujifilm Fujicolor");
        let rows = a.rows();
        assert_eq!(rows[0].title, "Roll #8");
        assert_eq!((rows[0].kind, rows.len()), (1, 11), "the roll, then its photos");
        // Twin: folding the roll leaves its row alone.
        let mut folded = a;
        let id = folded.rows()[0].id.clone();
        folded.toggle(&id);
        assert_eq!(folded.rows().len(), 1);
    }
}
