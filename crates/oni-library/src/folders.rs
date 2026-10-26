//! The archive's shape, read off the disk: a root's folders newest first, each holding its
//! subfolders and then its own photos in shooting order, the gallery laying them flat the same way.

use crate::archive_tree::Node;
use crate::film_group::{self, FilmGroup, FilmKey};
use crate::index::Entry;
use crate::paths::{self, NameKey, holds};
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
    FilmGroup {
        id: FilmKey::Folder(folder.to_owned()),
        film: roll,
        start,
        entries: in_shooting_order(entries),
        heading: Some(paths::relative(folder, root)),
    }
}

/// `shot`'s order, each photo's time and name read once rather than at every comparison.
fn in_shooting_order(entries: Vec<Entry>) -> Vec<Entry> {
    // `shot` takes 0 and -0 for one time, which `total_cmp` tells apart.
    let at = |e: &Entry| crate::seconds(film_group::capture(e)) + 0.0;
    let mut keyed: Vec<(f64, NameKey, Entry)> = entries.into_iter().map(|e| (at(&e), NameKey::new(&name(&e.path)), e)).collect();
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    keyed.into_iter().map(|(_, _, e)| e).collect()
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
    // Newest first, the name breaking a tie, each folder's stamp and name read once.
    let newest_first = |folders: Option<&BTreeSet<String>>| -> Vec<String> {
        let mut keyed: Vec<(f64, NameKey, &String)> =
            folders.into_iter().flatten().map(|f| (stamp(f), NameKey::new(&name(f)), f)).collect();
        keyed.sort_by(|a, b| if a.0 != b.0 { b.0.total_cmp(&a.0) } else { a.1.cmp(&b.1) });
        keyed.into_iter().map(|(_, _, f)| f.clone()).collect()
    };
    struct Cut<'a> {
        root: &'a str,
        by_folder: HashMap<String, Vec<Entry>>,
        children: &'a HashMap<String, BTreeSet<String>>,
        rolls: &'a HashMap<String, Film>,
    }
    fn node(cut: &mut Cut, folder: String, newest_first: &dyn Fn(Option<&BTreeSet<String>>) -> Vec<String>) -> Node {
        let own = cut.by_folder.remove(&folder).map(|e| group(&folder, cut.root, e, cut.rolls.get(&folder).cloned()));
        let subfolders = newest_first(cut.children.get(&folder)).into_iter().map(|f| node(cut, f, newest_first)).collect();
        Node::Folder { path: folder, group: own, children: subfolders }
    }
    let loose = by_folder.remove(root);
    let mut cut = Cut { root, by_folder, children: &children, rolls };
    let mut out: Vec<Node> = newest_first(children.get(root)).into_iter().map(|f| node(&mut cut, f, &newest_first)).collect();
    if let Some(loose) = loose {
        let loose = group(root, root, loose, rolls.get(root).cloned());
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
    tree_and_groups(root, entries, rolls, &[], made).1
}

/// `tree` and `groups` cut in one walk: the empty folders the survey adds hold no group, so the
/// groups of the whole tree are those of its photos alone.
pub fn tree_and_groups(
    root: &str,
    entries: &[Entry],
    rolls: &HashMap<String, Film>,
    folders: &[String],
    made: &HashMap<String, Date>,
) -> (Vec<Node>, Vec<FilmGroup>) {
    let nodes = tree(root, entries, rolls, folders, made);
    let groups = groups_of(root, &nodes, rolls);
    (nodes, groups)
}

/// The groups a tree cut by `tree` holds, laid as `groups` lays them.
pub fn groups_of(root: &str, nodes: &[Node], rolls: &HashMap<String, Film>) -> Vec<FilmGroup> {
    fn walk(nodes: &[Node], out: &mut Vec<FilmGroup>) {
        for node in nodes {
            if let Node::Folder { group, children, .. } = node {
                walk(children, out);
                out.extend(group.iter().cloned());
            }
        }
    }
    let mut out = Vec::new();
    walk(nodes, &mut out);
    let inside: Vec<Entry> = nodes.iter().filter_map(|n| if let Node::Photo(e) = n { Some(e.clone()) } else { None }).collect();
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
    let ext = paths::extension(path);
    !ext.is_empty() && kinds.iter().any(|k| ext.eq_ignore_ascii_case(k))
}

/// A RAW+JPEG pair as one photo: a JPEG with no sidecar beside a RAW of its stem among `images`, the
/// disk's files (a RAW in the ONI Trash too), is left out and named as the RAW's companion.
pub fn paired(entries: &[Entry], images: &[String], sidecars: &HashMap<String, Date>) -> (Vec<Entry>, HashMap<String, String>) {
    use crate::library::{JPEG_EXTENSIONS, RAW_EXTENSIONS};
    // A folder of RAWs alone, or of scans, pairs nothing: its stems are never cut.
    if !entries.iter().any(|e| is_kind(&e.path, JPEG_EXTENSIONS) && !sidecars.contains_key(&e.path)) {
        return (entries.to_vec(), HashMap::new());
    }
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

    /// One walk cuts the tree and the gallery's groups; the survey's empty folders move no group.
    #[test]
    fn the_groups_cut_with_the_tree_are_the_gallery_s() {
        let made = HashMap::from([("/R/New".to_owned(), Date::since_reference_date(99.0))]);
        let rolls = HashMap::from([("/R/2026/Juin".to_owned(), Film::new("Portra"))]);
        let empty = ["/R/New".to_owned(), "/R/2026/Empty".to_owned()];
        let (nodes, cut) = tree_and_groups("/R", &entries(), &rolls, &empty, &made);
        assert_eq!(cut, groups("/R", &entries(), &rolls, &made));
        assert_eq!(nodes, tree("/R", &entries(), &rolls, &empty, &made));
        // Twin: a photo in the new folder gives it a group, which the comparison sees.
        let mut more = entries();
        more.push(entry("New/n.RW2", 50.0));
        assert_ne!(tree_and_groups("/R", &more, &rolls, &empty, &made).1, cut);
    }

    /// The keys read once order the photos as `shot` does, ties broken by the Finder's name order.
    #[test]
    fn a_group_s_photos_lie_in_shot_s_order() {
        let photos: Vec<Entry> =
            [("f10.RW2", 5.0), ("f2.RW2", 5.0), ("\u{c9}t\u{e9}.RW2", 5.0), ("Etna.RW2", 5.0), ("a.RW2", 1.0), ("z.RW2", -0.0), ("y.RW2", 0.0)]
                .iter()
                .map(|(n, at)| entry(n, *at))
                .collect();
        let mut by_shot = photos.clone();
        by_shot.sort_by(shot);
        let laid = in_shooting_order(photos.clone());
        assert_eq!(laid, by_shot);
        // Twin: the names' bytes would put f10 before f2 and the composed Été last.
        let mut bytes = photos;
        bytes.sort_by(|a, b| crate::seconds(film_group::capture(a)).total_cmp(&crate::seconds(film_group::capture(b))).then_with(|| a.path.cmp(&b.path)));
        assert_ne!(laid, bytes);
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
