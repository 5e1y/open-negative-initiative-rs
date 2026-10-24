//! Gestures that move files on the disk, New Roll, and what follows any move: every path the app
//! recorded goes where the disk moved it, so nothing is looked for where it was.

use super::*;
use oni_library::archive_tree::NodeId as TreeId;
use oni_library::project_tree::NodeId as ProjectNode;

impl Ctl {
    /// `rollFolder(from:)`: a dated folder beside the first target, the photos, their passes and
    /// sidecars moved in, one gallery step that ⌘Z walks back.
    pub(in crate::app) fn new_roll(self: &Rc<Self>) {
        let targets: HashSet<String> = self.targets().into_iter().collect();
        self.new_roll_from(&targets);
    }

    /// `rollFolder(from:)`: the photos moved into a dated roll folder beside the first, one step.
    pub(in crate::app) fn new_roll_from(self: &Rc<Self>, targets: &HashSet<String>) {
        let (ordered, earliest) = {
            let s = self.state.borrow();
            let ordered: Vec<String> = s.order.iter().filter(|p| targets.contains(*p)).cloned().collect();
            let dates = s.rows.iter().filter(|e| targets.contains(&e.path)).filter_map(|e| e.captured_at);
            (ordered, dates.reduce(|a, b| if b < a { b } else { a }))
        };
        let Some(first) = ordered.first() else { return };
        let parent = oni_library::paths::parent(first);
        let name = oni_library::roll_folder::name_for(earliest.unwrap_or_else(oni_settings::Date::now), &parent);
        let film = oni_settings::Film::new(&name);
        let folder = match oni_library::roll_folder::make(&name, &parent, &film) {
            Ok(folder) => folder,
            Err(e) => return self.told(tr("New Roll"), &tr!("The roll's folder could not be made: {e}", e = oni_library::refusal::cause(&e))),
        };
        let files = oni_library::rgb_passes::expanded(&ordered);
        let failure = self.state.borrow_mut().store.flush();
        if let Some(message) = failure {
            self.told(tr("New Roll"), &message);
        }
        let made = oni_library::roll_folder::move_items(&oni_library::roll_folder::moves(&files, &folder));
        if made.len() < files.len() {
            self.told(tr("New Roll"), &tr!("{count} of the files could not be moved into the roll.", count = files.len() - made.len()));
        }
        {
            let mut s = self.state.borrow_mut();
            if let Some(archive) = s.archive.as_mut() {
                archive.expanded.insert(TreeId::Folder(parent.clone()));
                archive.expanded.insert(TreeId::Folder(folder.clone()));
            }
            s.rename_request = Some(folder.clone());
            if let Some(store) = s.store.editor() {
                store.record_disk(oni_core::store::Disk { folder: Some(folder), film: Some(film), moves: made.clone() }, tr("New Roll"));
            }
        }
        self.journal_count(oni_library::journal::Kind::Rolled, targets.len());

        self.followed(&made);
    }

    /// `followed(_:)`: the index, the rows held, the picks, the rings, the folds, the projects'
    /// recorded paths and the open frame go where the disk moved them; the folder is read again.
    pub(in crate::app) fn followed(self: &Rc<Self>, moves: &[Move]) {
        let moved = |path: &str| {
            moves.iter().find(|m| oni_library::paths::holds(&m.from, path)).map(|m| oni_library::paths::moved(path, &m.from, &m.to))
        };
        let carry = |path: &mut String| {
            if let Some(to) = moved(path) {
                *path = to;
            }
        };
        let (reopen, folded, recorded) = {
            let mut guard = self.state.borrow_mut();
            let s = &mut *guard;
            if let Some(index) = &s.index {
                for row in index.all() {
                    if let Some(to) = moved(&row.path) {
                        let _ = index.remove(&row.path);
                        let _ = index.upsert(&oni_library::Entry { path: to, ..row });
                    }
                }
            }
            for row in s.rows.iter_mut().chain(s.org.standing.iter_mut()).chain(s.org.trashed.iter_mut()) {
                carry(&mut row.path);
            }
            let fingerprints = s.rows.iter().map(|e| (e.path.clone(), e.fingerprint.clone())).collect();
            s.fingerprints = fingerprints;
            s.order.iter_mut().for_each(carry);
            // `selection` and `ring` follow too, so a photo moved is never opened where it was.
            let picks = &mut s.picks;
            picks.selection = picks.selection.drain().map(|p| moved(&p).unwrap_or(p)).collect();
            [&mut picks.anchor, &mut picks.cursor].into_iter().flatten().for_each(carry);
            let tree = &mut s.org.tree_pick;
            tree.selection = tree.selection.drain().map(|id| tree_moved(id, &moved)).collect();
            tree.anchor = tree.anchor.take().map(|id| tree_moved(id, &moved));
            let projects = &mut s.org.project_pick;
            projects.selection = projects.selection.drain().map(|id| project_moved(id, &moved)).collect();
            projects.anchor = projects.anchor.take().map(|id| project_moved(id, &moved));
            s.org.project_ring = s.org.project_ring.take().map(|id| project_moved(id, &moved));
            // The folders a hand folded stay folded under their new names.
            let folded: HashSet<TreeId> = s.archive.as_ref().map(|a| a.folded().into_iter().map(|id| tree_moved(id, &moved)).collect()).unwrap_or_default();
            // `Project.moved` then `saveProjects`: a member no read reaches is found at its new path.
            let recorded = s.org.projects.iter().any(|p| p.paths.is_some());
            if recorded {
                let pairs: Vec<(String, String)> = moves.iter().map(|m| (m.from.clone(), m.to.clone())).collect();
                s.org.projects = oni_library::project::moved(&s.org.projects, &pairs);
            }
            (s.open.as_deref().and_then(|p| moved(p)), folded, recorded)
        };
        if recorded {
            self.save_projects();
        }
        self.lay_archive();
        if !folded.is_empty() {
            if let Some(archive) = self.state.borrow_mut().archive.as_mut() {
                archive.expanded.retain(|id| !folded.contains(id));
            }
            self.publish_tree();
        }
        if let Some(to) = reopen {
            self.open_photo(&to);
        }
        self.read_again();
    }
}

fn tree_moved(id: TreeId, moved: &impl Fn(&str) -> Option<String>) -> TreeId {
    match id {
        TreeId::Folder(p) => TreeId::Folder(moved(&p).unwrap_or(p)),
        TreeId::Photo(p) => TreeId::Photo(moved(&p).unwrap_or(p)),
        other => other,
    }
}

fn project_moved(id: ProjectNode, moved: &impl Fn(&str) -> Option<String>) -> ProjectNode {
    match id {
        ProjectNode::Photo { path, project } => ProjectNode::Photo { path: moved(&path).unwrap_or(path), project },
        other => other,
    }
}
