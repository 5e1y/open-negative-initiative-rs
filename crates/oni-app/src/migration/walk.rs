//! Where a sweep looks for sidecars of an earlier format: the trees a person keeps (the folder last
//! opened, the favourites, the roots) walked whole, and the folders the indexes and projects name.

use crate::preferences::{OPENED_FOLDER, Preferences};
use oni_library::index::listed;
use oni_library::{migrate, paths, places::Places, project, roots::Roots};
use std::collections::BTreeSet;
use std::path::Path;

/// What a sweep walks, built fresh at each run.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Plan {
    /// Walked recursively, with the survey's folder rules.
    pub trees: Vec<String>,
    /// Each alone, not below it.
    pub folders: Vec<String>,
}

/// The trees and the folders of `support` and `caches`, the ledger's pending ones among the trees, each
/// by the spelling a survey gives it; a tree inside another and a folder inside a tree are left to the tree.
/// `alive` is read before each path is spelt, as spelling one on a mount that stopped answering waits.
pub fn planned(support: &Path, caches: &Path, pending: &[String], alive: &dyn Fn() -> bool) -> Plan {
    let mut trees: Vec<String> = Vec::new();
    trees.extend(Preferences::load(support).string(OPENED_FOLDER).map(str::to_owned));
    let places = Places::load(support).unwrap_or_else(|| migrate::places_held(support));
    trees.extend(places.paths());
    trees.extend(migrate::places_held(support).paths());
    trees.extend(Roots::load(support).paths);
    trees.extend(pending.iter().cloned());
    let mut folders: BTreeSet<String> = BTreeSet::new();
    for file in listed::files(caches) {
        folders.extend(listed::listed(&file).iter().map(|p| paths::parent(p)));
    }
    for p in project::load(support).projects {
        folders.extend(p.members.iter().filter_map(|m| m.path.as_deref()).map(paths::parent));
    }
    let spelt = |p: &String| alive().then(|| paths::canonical(Path::new(p)));
    plan(trees.iter().map_while(spelt).collect(), folders.iter().map_while(spelt).collect())
}

fn plan(trees: Vec<String>, folders: BTreeSet<String>) -> Plan {
    let mut kept: Vec<String> = Vec::new();
    for tree in trees.into_iter().filter(|t| !t.is_empty()) {
        if !kept.iter().any(|k| paths::holds(k, &tree) || paths::same(k, &tree)) {
            kept.retain(|k| !paths::holds(&tree, k));
            kept.push(tree);
        }
    }
    let folders = folders.into_iter().filter(|f| !f.is_empty() && !kept.iter().any(|t| paths::holds(t, f) || paths::same(t, f))).collect();
    Plan { trees: kept, folders }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tree_takes_what_lies_inside_it_and_the_first_named_leads() {
        let trees = ["/Scans/Roll 12", "/Scans", "/Volumes/T7/Rolls", "/Scans/Roll 3", ""].map(String::from).to_vec();
        let folders = ["/Scans/Roll 9", "/Users/me/Desktop", "/Volumes/T7/Rolls"].map(String::from).into();
        let plan = plan(trees, folders);
        assert_eq!(plan.trees, ["/Scans", "/Volumes/T7/Rolls"]);
        assert_eq!(plan.folders, ["/Users/me/Desktop"]);
        // Twin: a folder beside a tree, not inside it, stays a folder of its own.
        let beside = super::plan(vec!["/Scans".into()], ["/Scans 2".to_string()].into());
        assert_eq!(beside.folders, ["/Scans 2"]);
    }
}
