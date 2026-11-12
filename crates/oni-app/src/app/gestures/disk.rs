//! Gestures that move files on the disk, New Roll, and what follows any move: every path the app
//! recorded goes where the disk moved it, so nothing is looked for where it was.

use super::*;
use oni_library::archive_tree::NodeId as TreeId;
use oni_library::roll_folder;
use oni_library::project_tree::NodeId as ProjectNode;

impl Ctl {
    /// `rollFolder(from:)`, asked first: a dated folder in the photos' folder, the photos, their
    /// passes and every file beside them moved in, one gallery step that ⌘Z walks back.
    pub(in crate::app) fn new_roll(self: &Rc<Self>) {
        let targets: HashSet<String> = self.targets().into_iter().collect();
        self.new_roll_from(&targets);
    }

    /// The photos of one folder laid out off this thread, then the confirm naming the folder made
    /// and counting the files moved; photos of several folders are refused, with the reason.
    pub(in crate::app) fn new_roll_from(self: &Rc<Self>, targets: &HashSet<String>) {
        self.new_roll_named(targets, None);
    }

    /// `new_roll_from`, under the name and the stock the roll guide typed, when it did.
    pub(in crate::app) fn new_roll_named(self: &Rc<Self>, targets: &HashSet<String>, named: Option<(String, Option<String>)>) {
        if self.busy_said(tr("New Roll")) {
            return;
        }
        let (ordered, earliest) = {
            let s = self.state.borrow();
            let ordered: Vec<String> = s.order.iter().filter(|p| targets.contains(*p)).cloned().collect();
            let dates = s.rows.iter().filter(|e| targets.contains(&e.path)).filter_map(|e| e.captured_at);
            (ordered, dates.reduce(|a, b| if b < a { b } else { a }))
        };
        if ordered.is_empty() {
            return;
        }
        let parent = match roll_folder::one_folder(&ordered) {
            Ok(parent) => parent,
            Err(why) => return self.told(tr("New Roll"), &why),
        };
        let earliest = earliest.unwrap_or_else(oni_settings::Date::now);
        let _ = std::thread::Builder::new().name("planning a roll".into()).spawn(move || {
            let plan = match named {
                Some((name, stock)) => {
                    let mut plan = roll_folder::plan_named(&ordered, &name, &parent);
                    plan.film.stock = stock;
                    (plan, true)
                }
                None => (roll_folder::plan(&ordered, earliest, &parent), false),
            };
            with_shell(move |c| c.ask_new_roll(plan.0, plan.1));
        });
    }

    fn ask_new_roll(self: &Rc<Self>, plan: roll_folder::Plan, named: bool) {
        if plan.units.is_empty() {
            let why = if plan.absent > 0 { oni_library::refusal::left_out(plan.absent) } else { tr("Nothing could move: every name is taken in the roll's folder.").into() };
            return self.told(tr("New Roll"), &why);
        }
        let title = trn("Move This Photo into a New Roll?", "Move {n} Photos into a New Roll?", plan.picked as u64);
        let files = trn("one file", "{n} files", plan.files as u64);
        let mut message = tr!(
            "ONI makes the folder “{folder}” in “{parent}” and moves {files} into it: the photos, their sidecars and what other apps keep beside them. Undo puts them back.",
            folder = plan.name,
            parent = oni_library::paths::name(&plan.parent),
            files = files
        );
        if plan.absent > 0 {
            message.push(' ');
            message.push_str(&trn("One photo stays out: its original cannot be found.", "{n} photos stay out: their originals cannot be found.", plan.absent as u64));
        }
        let key = trn("Move Photo", "Move {n} Photos", plan.picked as u64);
        self.ask(&title, &message, &key, false, Pending::NewRoll(Box::new(plan), named));
    }

    /// The confirmed roll: its folder made and each frame moved whole, off this thread.
    pub(in crate::app) fn lay_new_roll(self: &Rc<Self>, plan: roll_folder::Plan, named: bool) {
        if self.busy_said(tr("New Roll")) {
            return;
        }
        self.drain_drafts();
        let moving: Vec<Move> = plan.units.iter().flat_map(|u| u.moves.iter().cloned()).collect();
        if self.unsaved_among(tr("New Roll"), &moving) {
            return;
        }
        let parent = plan.parent.clone();
        self.on_disk(
            tr("Moving the photos into the roll"),
            move || roll_folder::carry_out(&plan, &roll_folder::rename_new).map_err(|e| oni_library::refusal::cause(&e)),
            move |c, made| c.new_roll_landed(&parent, made, named),
        );
    }

    /// A roll the guide named takes no rename in the tree; its stock's dyes follow as a step of their own.
    fn new_roll_landed(self: &Rc<Self>, parent: &str, made: Result<roll_folder::Made, String>, named: bool) {
        let made = match made {
            Ok(made) => made,
            Err(e) => return self.told(tr("New Roll"), &tr!("The roll's folder could not be made: {e}", e = e)),
        };
        if let Some(said) = roll_folder::said(&made.carried) {
            self.told_with(tr("New Roll"), &said, Some(parent.to_owned()));
        }
        let moves = made.disk.moves.clone();
        let Some(folder) = made.disk.folder.clone().filter(|_| !moves.is_empty()) else { return };
        let stock = made.disk.film.as_ref().and_then(|f| f.stock.clone());
        {
            let mut s = self.state.borrow_mut();
            if let Some(archive) = s.archive.as_mut() {
                archive.unfold(TreeId::Folder(parent.to_owned()));
                archive.unfold(TreeId::Folder(folder.clone()));
            }
            if !named {
                s.rename_request = Some(folder.clone());
            }
            s.store.record_disk(made.disk, tr("New Roll"));
            s.store.forget_measured();
        }
        self.journal_count(oni_library::journal::Kind::Rolled, moves.len());
        self.followed(&moves);
        if let Some(stock) = stock {
            self.dyes_follow(&folder, &stock);
        }
        self.publish_bar();
    }

    /// The open frame's settings written before its file moves; a refused write refuses the move,
    /// said under `title`, or its edits would stay behind.
    pub(in crate::app) fn unsaved_among(&self, title: &str, moves: &[Move]) -> bool {
        let failure = self.state.borrow_mut().store.flush();
        let Some(error) = failure else { return false };
        let open = self.state.borrow().store.path().map(str::to_string);
        if !open.is_some_and(|o| moves.iter().any(|m| oni_library::paths::holds(&m.from, &o))) {
            self.told(title, &error);
            return false;
        }
        self.told(title, &tr!("Nothing was moved, as the open photo's settings could not be saved: {error}", error = error));
        true
    }

    /// `followed`, for a move no gallery step made, a folder renamed or found elsewhere: the steps
    /// follow it too, so an undo reaches the files under their new names.
    pub(in crate::app) fn followed_outside(self: &Rc<Self>, moves: &[Move]) {
        self.state.borrow_mut().store.follow_history(moves);
        self.followed(moves);
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
            // A read in flight surveyed the old names: superseded, its rows would drop the moved picks.
            s.generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            // The store first: the open frame's next write and the edits kept unsaved go to the new path.
            s.store.follow(moves);
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
            let fingerprints = s.rows.iter().map(|e| (e.path.clone(), s.tile_key(e))).collect();
            s.fingerprints = fingerprints;
            survey_moved(Arc::make_mut(&mut s.survey), &moved);
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
            // `Project.moved` then `saveProjects`: a member is found at its new path, its print kept.
            let pairs: Vec<(String, String)> = moves.iter().map(|m| (m.from.clone(), m.to.clone())).collect();
            let carried = oni_library::project::moved(&s.org.projects, &pairs);
            let recorded = carried != s.org.projects;
            s.org.projects = carried;
            let found = std::mem::take(&mut s.org.found).into_iter();
            s.org.found = found.map(|(m, at)| (oni_library::project::moved_member(&m, &pairs), oni_library::project::carried(&at, &pairs))).collect();
            // A photo that stays where it was is not opened again.
            (s.open_path().and_then(|p| moved(p).filter(|to| to != p)), folded, recorded)
        };
        if recorded {
            self.save_projects();
        }
        self.lay_archive();
        if !folded.is_empty() {
            if let Some(archive) = self.state.borrow_mut().archive.as_mut() {
                archive.keep_folded(&folded);
            }
            self.publish_tree();
        }
        if let Some(to) = reopen {
            self.open_photo(&to);
        }
        self.read_again();
    }
}

/// The survey's paths where the disk moved them, so the archive laid before the next read keeps
/// each photo's pair and its roll.
fn survey_moved(survey: &mut Survey, moved: &impl Fn(&str) -> Option<String>) {
    let carry = |path: String| moved(&path).unwrap_or(path);
    fn keys<V>(map: &mut HashMap<String, V>, carry: &impl Fn(String) -> String) {
        *map = std::mem::take(map).into_iter().map(|(k, v)| (carry(k), v)).collect();
    }
    for list in [&mut survey.images, &mut survey.folders, &mut survey.unreadable] {
        *list = std::mem::take(list).into_iter().map(carry).collect();
    }
    for set in [&mut survey.cloud, &mut survey.settling] {
        *set = std::mem::take(set).into_iter().map(carry).collect();
    }
    keys(&mut survey.made, &carry);
    keys(&mut survey.dated, &carry);
    keys(&mut survey.sizes, &carry);
    keys(&mut survey.away, &carry);
    keys(&mut survey.rolls, &carry);
    keys(&mut survey.sidecars, &carry);
}

fn tree_moved(id: TreeId, moved: &impl Fn(&str) -> Option<String>) -> TreeId {
    match id {
        TreeId::Folder(p) => TreeId::Folder(moved(&p).unwrap_or(p)),
        TreeId::Photo(p) => TreeId::Photo(moved(&p).unwrap_or(p)),
    }
}

fn project_moved(id: ProjectNode, moved: &impl Fn(&str) -> Option<String>) -> ProjectNode {
    match id {
        ProjectNode::Photo { path, project } => ProjectNode::Photo { path: moved(&path).unwrap_or(path), project },
        other => other,
    }
}

#[cfg(test)]
#[test]
fn a_survey_follows_a_move_with_each_path_it_records() {
    let mut survey = Survey::default();
    survey.images = vec!["/s/r/a.nef".into(), "/s/b.nef".into()];
    survey.sidecars.insert("/s/r/a.nef".into(), oni_settings::Date::now());
    survey.rolls.insert("/s/r".into(), oni_settings::Film::new("r"));
    let moved = |p: &str| oni_library::paths::holds("/s/r", p).then(|| oni_library::paths::moved(p, "/s/r", "/s/t"));
    // Twin: the survey left as it was pairs no sidecar with the photo at its new path.
    assert!(!survey.sidecars.contains_key("/s/t/a.nef"));
    survey_moved(&mut survey, &moved);
    assert_eq!(survey.images, ["/s/t/a.nef", "/s/b.nef"]);
    assert!(survey.sidecars.contains_key("/s/t/a.nef") && survey.rolls.contains_key("/s/t"));
    assert_eq!(survey.sidecars.len() + survey.rolls.len(), 2, "nothing left under the old names");
}
