//! What a project reaches: the opened folder's rows and every member the index holds beyond them,
//! so the Projects tab and every project gesture read the whole library, whatever folder is open.

use super::*;
use std::borrow::Cow;

impl State {
    /// The rows projects resolve against: the opened folder's first, then the members beyond it.
    pub(in crate::app) fn reach(&self) -> Cow<'_, [Entry]> {
        if self.org.standing.is_empty() {
            return Cow::Borrowed(&self.rows);
        }
        Cow::Owned(self.rows.iter().chain(&self.org.standing).cloned().collect())
    }

    /// Every roll marker known, by folder: the opened folder's survey, then those read for the
    /// projects' members beyond it.
    pub(in crate::app) fn roll_markers(&self) -> HashMap<String, Film> {
        let beyond = self.org.member_rolls.iter().filter_map(|(folder, film)| Some((folder.clone(), film.clone()?)));
        beyond.chain(self.survey.rolls.iter().map(|(folder, film)| (folder.clone(), film.clone()))).collect()
    }
}

/// Whether the opened folder's survey covers `folder`, which reads its marker afresh.
fn surveyed(opened: Option<&str>, folder: &str) -> bool {
    opened.is_some_and(|opened| oni_library::paths::holds(opened, folder))
}

impl Ctl {
    /// `checkProjectPhotos`: the members beyond the opened folder read again off the index, one look
    /// per member, the originals' states looked at behind; a Swift print found is written as its path.
    pub(in crate::app) fn read_standing(&self) {
        let found = {
            let mut s = self.state.borrow_mut();
            let standing = match &s.index {
                Some(index) if !s.org.projects.is_empty() => oni_library::project_standing::standing(&s.org.projects, &s.rows, index),
                _ => Default::default(),
            };
            s.org.standing = standing.rows;
            if !standing.found.is_empty() {
                s.org.projects = project::moved(&s.org.projects, &standing.found);
            }
            !standing.found.is_empty()
        };
        if found {
            self.save_projects();
        }
        self.read_member_rolls();
    }

    /// The roll markers of the members' folders beyond the opened one, read once each off this
    /// thread; one the survey covers is read again once left, its marker perhaps changed meanwhile.
    fn read_member_rolls(&self) {
        let unread: Vec<String> = {
            let mut guard = self.state.borrow_mut();
            let s = &mut *guard;
            let opened = s.opened.as_deref();
            s.org.member_rolls.retain(|folder, _| !surveyed(opened, folder));
            let members = s.org.projects.iter().flat_map(|p| &p.members).filter(|m| !project::is_print(m));
            let folders: HashSet<String> = members.map(|m| oni_library::paths::parent(m)).collect();
            folders.into_iter().filter(|f| !surveyed(opened, f) && !s.org.member_rolls.contains_key(f)).collect()
        };
        if unread.is_empty() {
            return;
        }
        let _ = std::thread::Builder::new().name("member rolls".into()).spawn(move || {
            let read = unread.into_iter().map(|folder| {
                let film = oni_library::folders::roll(&folder);
                (folder, film)
            });
            let read: Vec<(String, Option<Film>)> = read.collect();
            crate::app::with_shell(move |c| {
                c.state.borrow_mut().org.member_rolls.extend(read);
                c.publish_info();
            });
        });
    }

    /// The ONI Trash and the projects' members, read for the whole library: a launch with no
    /// folder open lists them as one that opens a folder does.
    pub(in crate::app) fn read_library(&self) {
        self.read_trashed();
        self.read_standing();
    }
}
