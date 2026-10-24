//! The archive's shape, read off the disk: a root's folders newest first, each holding its
//! subfolders and then its own photos in shooting order, the gallery laying them flat the same way.

use crate::archive_tree::Node;
use crate::film_group::{self, FilmGroup, FilmKey};
use crate::index::Entry;
use crate::paths::{self, holds};
use oni_settings::{Date, Film, roll_folder};
use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// A folder made a roll carries its roll in this hidden file beside its photos, so the roll
/// follows the folder wherever it moves.
pub const ROLL_MARKER: &str = roll_folder::ROLL_MARKER;

pub fn marker(folder: &str) -> PathBuf {
    roll_folder::marker(Path::new(folder))
}

pub fn roll(folder: &str) -> Option<Film> {
    roll_folder::roll(Path::new(folder))
}

pub fn set_roll(film: Option<&Film>, folder: &str) -> std::io::Result<()> {
    roll_folder::set_roll(film, Path::new(folder))
}

/// The roll each folder holding one of `paths` carries. Reads the disk.
pub fn rolls(holding: &[String]) -> HashMap<String, Film> {
    let folders: HashSet<String> = holding.iter().map(|p| paths::parent(p)).collect();
    folders.into_iter().filter_map(|folder| roll(&folder).map(|film| (folder, film))).collect()
}

pub use crate::paths::{name, parent};

/// Two paths in the Finder's order of their names.
pub fn ordered(a: &str, b: &str) -> bool {
    paths::standard_order(&name(a), &name(b)) == Ordering::Less
}

fn by_name(a: &str, b: &str) -> Ordering {
    paths::standard_order(&name(a), &name(b))
}

/// Shooting order, the name breaking a tie.
pub fn shot(a: &Entry, b: &Entry) -> Ordering {
    let (ta, tb) = (film_group::capture(a), film_group::capture(b));
    if ta != tb { crate::seconds(ta).total_cmp(&crate::seconds(tb)) } else { by_name(&a.path, &b.path) }
}

pub fn group(folder: &str, root: &str, entries: Vec<Entry>, roll: Option<Film>) -> FilmGroup {
    let start = entries
        .iter()
        .map(film_group::capture)
        .min_by(|a, b| crate::seconds(*a).total_cmp(&crate::seconds(*b)))
        .unwrap_or(crate::distant_past());
    let mut entries = entries;
    entries.sort_by(shot);
    FilmGroup {
        id: FilmKey::Folder(folder.to_owned()),
        film: roll,
        start,
        entries,
        heading: Some(paths::relative(folder, root)),
    }
}

/// The root's children: every folder holding a photo at any depth, newest first by its latest
/// photo (an empty one by its `made` date), then the root's own photos as leaves.
pub fn tree(
    root: &str,
    entries: &[Entry],
    rolls: &HashMap<String, Film>,
    folders: &[String],
    made: &HashMap<String, Date>,
) -> Vec<Node> {
    let mut by_folder: HashMap<String, Vec<Entry>> = HashMap::new();
    for entry in entries.iter().filter(|e| holds(root, &e.path)) {
        by_folder.entry(parent(&entry.path)).or_default().push(entry.clone());
    }
    let mut children: HashMap<String, BTreeSet<String>> = HashMap::new();
    let known: HashSet<String> =
        by_folder.keys().cloned().chain(folders.iter().filter(|f| holds(root, f)).cloned()).collect();
    for folder in known {
        let mut walk = folder;
        while walk != root && holds(root, &walk) {
            let up = parent(&walk);
            if up == walk {
                break;
            }
            children.entry(up.clone()).or_default().insert(walk);
            walk = up;
        }
    }
    let mut latest: HashMap<String, Date> = HashMap::new();
    for (folder, photos) in &by_folder {
        let Some(newest) =
            photos.iter().map(film_group::capture).max_by(|a, b| crate::seconds(*a).total_cmp(&crate::seconds(*b)))
        else {
            continue;
        };
        let mut walk = folder.clone();
        while holds(root, &walk) {
            if latest.get(&walk).is_some_and(|known| *known >= newest) {
                break;
            }
            latest.insert(walk.clone(), newest);
            if walk == root {
                break;
            }
            walk = parent(&walk);
        }
    }
    let stamp = |f: &str| {
        latest.get(f).or_else(|| made.get(f)).copied().map(crate::seconds).unwrap_or(crate::DISTANT_PAST_SECONDS)
    };
    let newest_first = |a: &String, b: &String| {
        let (da, db) = (stamp(a), stamp(b));
        if da != db { db.total_cmp(&da) } else { by_name(a, b) }
    };
    fn node(
        folder: &str,
        root: &str,
        by_folder: &HashMap<String, Vec<Entry>>,
        children: &HashMap<String, BTreeSet<String>>,
        rolls: &HashMap<String, Film>,
        newest_first: &dyn Fn(&String, &String) -> Ordering,
    ) -> Node {
        let own = by_folder.get(folder).map(|e| group(folder, root, e.clone(), rolls.get(folder).cloned()));
        let mut subfolders: Vec<&String> = children.get(folder).map(|c| c.iter().collect()).unwrap_or_default();
        subfolders.sort_by(|a, b| newest_first(a, b));
        let subfolders =
            subfolders.into_iter().map(|f| node(f, root, by_folder, children, rolls, newest_first)).collect();
        Node::Folder { path: folder.to_owned(), group: own, children: subfolders }
    }
    let mut top: Vec<&String> = children.get(root).map(|c| c.iter().collect()).unwrap_or_default();
    top.sort_by(|a, b| newest_first(a, b));
    let mut out: Vec<Node> =
        top.into_iter().map(|f| node(f, root, &by_folder, &children, rolls, &newest_first)).collect();
    if let Some(loose) = by_folder.get(root) {
        let loose = group(root, root, loose.clone(), rolls.get(root).cloned());
        out.extend(loose.entries.into_iter().map(Node::Photo));
    }
    out
}

/// The gallery's groups: the tree laid flat, subfolders before their parent's own photos, the
/// root's own photos last.
pub fn groups(
    root: &str,
    entries: &[Entry],
    rolls: &HashMap<String, Film>,
    made: &HashMap<String, Date>,
) -> Vec<FilmGroup> {
    fn walk(nodes: Vec<Node>) -> Vec<FilmGroup> {
        nodes
            .into_iter()
            .flat_map(|node| match node {
                Node::Folder { group, children, .. } => {
                    let mut out = walk(children);
                    out.extend(group);
                    out
                }
                _ => Vec::new(),
            })
            .collect()
    }
    let inside: Vec<Entry> = entries.iter().filter(|e| parent(&e.path) == root).cloned().collect();
    let mut out = walk(tree(root, entries, rolls, &[], made));
    if !inside.is_empty() {
        out.push(group(root, root, inside, rolls.get(root).cloned()));
    }
    out
}

/// A name as a case-insensitive, composed disk compares it, its extension cut.
fn stem_key(path: &str) -> String {
    format!("{}/{}", parent(path), paths::composed(&paths::stem(path)).to_lowercase())
}

fn is_kind(path: &str, kinds: &[&str]) -> bool {
    let ext = paths::extension(path).to_ascii_lowercase();
    !ext.is_empty() && kinds.contains(&ext.as_str())
}

/// A RAW+JPEG pair as one photo: a JPEG with no sidecar beside a RAW of its stem among `images`, the
/// disk's files (a RAW in the ONI Trash too), is left out and named as the RAW's companion.
pub fn paired(entries: &[Entry], images: &[String], sidecars: &HashMap<String, Date>) -> (Vec<Entry>, HashMap<String, String>) {
    use crate::library::{JPEG_EXTENSIONS, RAW_EXTENSIONS};
    let mut raws: HashMap<String, &String> = HashMap::new();
    for image in images.iter().filter(|i| is_kind(i, RAW_EXTENSIONS)) {
        raws.entry(stem_key(image)).and_modify(|r| *r = (*r).min(image)).or_insert(image);
    }
    let mut companions = HashMap::new();
    let kept = entries
        .iter()
        .filter(|e| {
            if !is_kind(&e.path, JPEG_EXTENSIONS) || sidecars.contains_key(&e.path) {
                return true;
            }
            let Some(raw) = raws.get(&stem_key(&e.path)) else { return true };
            companions.entry((*raw).clone()).or_insert_with(|| e.path.clone());
            false
        })
        .cloned()
        .collect();
    (kept, companions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive_tree::NodeId;

    fn entry(path: &str, at: f64) -> Entry {
        let mut e = Entry::new(&format!("/R/{path}"), crate::distant_past(), path);
        e.captured_at = Some(Date::since_reference_date(at));
        e
    }

    fn entries() -> Vec<Entry> {
        vec![
            entry("b.RW2", 5.0),
            entry("2026/Juin/f10.RW2", 30.0),
            entry("2026/Juin/f2.RW2", 40.0),
            entry("2026/a.RW2", 20.0),
            entry("2025/x.tif", 10.0),
            Entry::new("/Elsewhere/y.RW2", crate::distant_past(), "y"),
        ]
    }

    #[test]
    fn folders_lay_newest_first_subfolders_before_their_parent() {
        let rolls = HashMap::from([("/R/2026/Juin".to_owned(), Film::new("Portra"))]);
        let groups = groups("/R", &entries(), &rolls, &HashMap::new());
        let headings: Vec<String> = groups.iter().map(FilmGroup::display_title).collect();
        assert_eq!(headings, ["Portra", "2026", "2025", "R"]);
        let juin = groups.iter().find(|g| g.id == FilmKey::Folder("/R/2026/Juin".into())).unwrap();
        assert_eq!(juin.entries.iter().map(|e| name(&e.path)).collect::<Vec<_>>(), ["f10.RW2", "f2.RW2"]);
        // Twin: without its roll the folder heads with its path.
        let plain: Vec<String> = super::groups("/R", &entries(), &HashMap::new(), &HashMap::new())
            .iter()
            .map(FilmGroup::display_title)
            .collect();
        assert!(plain.contains(&"2026/Juin".to_owned()) && !plain.contains(&"Portra".to_owned()));
        assert!(!groups.iter().flat_map(|g| &g.entries).any(|e| e.path == "/Elsewhere/y.RW2"));
    }

    #[test]
    fn a_raw_jpeg_pair_is_one_tile() {
        let rows: Vec<Entry> = ["Roll/DSC_1.NEF", "Roll/dsc_1.JPG", "Roll/DSC_2.JPG", "Roll/DSC_3.nef", "Roll/DSC_3.jpeg", "Other/DSC_1.JPG"]
            .iter()
            .map(|p| entry(p, 1.0))
            .collect();
        let images: Vec<String> = rows.iter().map(|e| e.path.clone()).collect();
        let (kept, companions) = paired(&rows, &images, &HashMap::new());
        let names: Vec<String> = kept.iter().map(|e| e.path[3..].to_owned()).collect();
        assert_eq!(names, ["Roll/DSC_1.NEF", "Roll/DSC_2.JPG", "Roll/DSC_3.nef", "Other/DSC_1.JPG"], "a JPEG alone, or in another folder, stays");
        assert_eq!(companions.get("/R/Roll/DSC_1.NEF").map(String::as_str), Some("/R/Roll/dsc_1.JPG"));
        assert_eq!(companions.len(), 2);
        // A RAW in the ONI Trash, out of the rows, still keeps its JPEG behind it.
        let shown: Vec<Entry> = rows.iter().filter(|e| !e.path.ends_with("DSC_1.NEF")).cloned().collect();
        assert!(!paired(&shown, &images, &HashMap::new()).0.iter().any(|e| e.path == "/R/Roll/dsc_1.JPG"));
        // Twin: a JPEG with settings of its own, edited before, stays a photo of its own.
        let edited = HashMap::from([("/R/Roll/dsc_1.JPG".to_owned(), Date::since_reference_date(1.0))]);
        let (kept, companions) = paired(&rows, &images, &edited);
        assert!(kept.iter().any(|e| e.path == "/R/Roll/dsc_1.JPG") && !companions.contains_key("/R/Roll/DSC_1.NEF"));
        // Twin: unpaired, every file is a tile.
        assert_eq!(paired(&rows, &[], &HashMap::new()).0.len(), rows.len());
    }

    #[test]
    fn the_tree_opens_on_its_folders_then_its_photos_and_an_empty_folder_by_its_birth() {
        let top: Vec<NodeId> =
            tree("/R", &entries(), &HashMap::new(), &[], &HashMap::new()).iter().map(Node::id).collect();
        assert_eq!(
            top,
            [NodeId::Folder("/R/2026".into()), NodeId::Folder("/R/2025".into()), NodeId::Photo("/R/b.RW2".into())]
        );
        let made = HashMap::from([("/R/New".to_owned(), Date::since_reference_date(99.0))]);
        let fresh = tree("/R", &entries(), &HashMap::new(), &["/R/New".to_owned()], &made);
        assert_eq!(fresh[0].id(), NodeId::Folder("/R/New".into()));
        // Twin: born before every photo, it falls behind them.
        let made = HashMap::from([("/R/New".to_owned(), Date::since_reference_date(1.0))]);
        let old = tree("/R", &entries(), &HashMap::new(), &["/R/New".to_owned()], &made);
        assert_eq!(old[2].id(), NodeId::Folder("/R/New".into()));
    }
}
