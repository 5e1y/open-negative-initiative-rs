//! The navigator's rows: Computer, the base's folders and the favourites, each one's state, a click
//! opening and the chevron folding.

use super::*;

impl Ctl {
    /// The rows, the header and the welcome as they stand; every row's own listing is asked so
    /// its chevron knows whether it holds a folder.
    pub(in crate::app) fn publish_navigator(&self) {
        let Some(w) = self.window() else { return };
        let published = {
            let mut s = self.state.borrow_mut();
            let favourites = s.nav.favourites.paths.clone();
            let rows = s.nav.model.rows(&favourites);
            for (path, _) in &rows {
                s.nav.model.list(path, false);
            }
            let rolls = s.nav.rolls();
            let opened = if s.org.trash_open { None } else { s.opened.clone() };
            let model = &s.nav.model;
            // Two favourites named alike carry the folders above them, only there: siblings never are.
            let trails = if model.favourites_only { walk::trails(&favourites) } else { HashMap::new() };
            let lines: Vec<FolderLine> = rows
                .iter()
                .map(|(path, depth)| {
                    let presence = s.row_presence(path);
                    let (caption, help) = reach::row_words(presence, path, &s.nav.computer);
                    let title = s.nav.computer.name(path);
                    let (head, tail) = walk::split_end(&title);
                    FolderLine {
                    title: title.into(),
                    head: head.into(),
                    tail: tail.into(),
                    trail: trails.get(path).filter(|_| *depth == 0).cloned().unwrap_or_default().into(),
                    path: path.clone().into(),
                    depth: *depth as i32,
                    // Out of reach, it promises no folder inside.
                    chevron: presence.is_present() && model.listings.get(path).is_none_or(|c| !c.is_empty()),
                    unfolded: model.unfolded.contains(path),
                    kind: if rolls.contains(path) {
                        1
                    } else if opened.as_deref() == Some(path.as_str()) {
                        2
                    } else {
                        0
                    },
                    picked: model.selected.as_deref() == Some(path.as_str()),
                    opened: opened.as_deref() == Some(path.as_str()),
                    favourite: s.nav.favourites.contains(path),
                    present: presence.is_present(),
                    reach: reach::reach(presence),
                    caption: caption.into(),
                    help: help.into(),
                    renaming: model.renaming.as_deref() == Some(path.as_str()),
                }})
                .collect();
            let blank = s.blank_words();
            let base = model.base.clone();
            let mut ancestors: Vec<Option<String>> =
                base.as_deref().map(|b| s.nav.computer.ancestors(b)).unwrap_or_default().into_iter().map(Some).collect();
            ancestors.push(None);
            let choices: Vec<FolderChoice> =
                ancestors.iter().map(|a| FolderChoice { title: walk::title(a.as_deref(), &s.nav.computer).into(), current: *a == base }).collect();
            let header = (
                model.can_go_up(),
                model.favourites_only,
                if model.favourites_only { tr("Favourites").to_string() } else { walk::title(base.as_deref(), &s.nav.computer) },
                base.as_deref().map(|b| tr!("{folder} — click to jump to a folder above", folder = b)).unwrap_or_else(|| tr("The home folder and every volume").into()),
                model.favourites_only && favourites.is_empty(),
                !model.favourites_only && base.is_none(),
            );
            let welcome: Vec<WelcomeLine> = s
                .nav
                .welcome
                .iter()
                .map(|p| {
                    // A folder on a disk away is shown for what it is, never offered to open on nothing.
                    let away = !s.present(p);
                    let name = s.nav.computer.name(p);
                    let title = if away {
                        let state = reach::row_words(Presence::Offline(Away::Disconnected), p, &s.nav.computer).0;
                        tr!("{name} — {state}", name = name, state = state)
                    } else {
                        tr!("Add {name}", name = name).to_uppercase()
                    };
                    WelcomeLine { title: title.into(), path: p.clone().into(), enabled: !away }
                })
                .collect();
            let welcome_shown = s.opened.is_none() && !s.nav.welcome.is_empty();
            let opened_pill = (s.opened.as_deref().map(|o| s.nav.computer.name(o)).unwrap_or_default(), s.opened.clone().unwrap_or_default());
            s.nav.rows = rows.into_iter().map(|(p, _)| p).collect();
            let mut refused: Vec<String> = s.nav.model.refused.iter().cloned().collect();
            refused.extend(s.opened.clone().filter(|o| s.survey.unreadable.contains(o)));
            *s.nav.refused.lock().expect("refused") = refused;
            s.remember();
            (lines, choices, header, welcome, welcome_shown, opened_pill, blank)
        };
        self.list_wanted();
        let (lines, choices, header, welcome, welcome_shown, opened_pill, blank) = published;
        let n = w.global::<crate::Navigator>();
        n.set_rows(ModelRc::new(VecModel::from(lines)));
        n.set_ancestors(ModelRc::new(VecModel::from(choices)));
        n.set_can_go_up(header.0);
        n.set_favourites_only(header.1);
        n.set_pill_title(header.2.into());
        n.set_pill_help(header.3.into());
        n.set_no_favourites(header.4);
        n.set_at_computer(header.5);
        n.set_blank_title(blank.0.into());
        n.set_blank_detail(blank.1.into());
        n.set_welcome(ModelRc::new(VecModel::from(welcome)));
        n.set_welcome_shown(welcome_shown);
        n.set_opened_title(opened_pill.0.into());
        n.set_opened_help(opened_pill.1.into());
        self.publish_catch_up();
        // A volume mounted under another name moves no tile, so the banner's lead is looked for here too.
        self.look_for_lead();
        self.rewatch();
    }

    /// The listings asked and not under way, each read apart (`list_apart`).
    pub(super) fn list_wanted(&self) {
        let wanted = self.state.borrow_mut().nav.model.take_wanted();
        self.list_apart(wanted);
    }

    /// The listings landed lay their rows and open nothing: the folder opened last stays opened.
    pub(super) fn listed(self: &Rc<Self>, listed: Vec<(String, Option<Vec<String>>)>) {
        let moved = {
            let mut s = self.state.borrow_mut();
            let mut moved = false;
            for (folder, children) in listed {
                let landed = match children {
                    Some(children) => s.nav.model.listed(&folder, children),
                    None => s.nav.model.refused(&folder),
                };
                moved |= landed != Landed::Same;
            }
            moved
        };
        if moved {
            self.publish_navigator();
        }
    }

    /// `NavigatorRow.onTapGesture`: a click opens its folder, out of reach too, where the gallery
    /// says its state; the second of a double click opens nothing more. The arrows follow it.
    pub(super) fn navigator_row_clicked(self: &Rc<Self>, i: usize) {
        let open = {
            let mut s = self.state.borrow_mut();
            let Some(path) = s.nav.rows.get(i).cloned() else { return };
            let now = Instant::now();
            let count = match &s.nav.clicks {
                Some((last, at, n)) if *last == path && now.duration_since(*at) <= crate::system::double_click() => n + 1,
                _ => 1,
            };
            s.nav.clicks = Some((path.clone(), now, count));
            s.nav.keys = true;
            s.org.trash_picked = None;
            s.nav.model.click(&path, count)
        };
        self.publish_trash_rows();
        self.publish_navigator();
        if let Some(folder) = open {
            self.open_asked(&folder);
        }
    }

    /// The chevron alone: the row folds or unfolds, and nothing opens.
    pub(super) fn navigator_row_folded(self: &Rc<Self>, i: usize) {
        {
            let mut s = self.state.borrow_mut();
            let Some(path) = s.nav.rows.get(i).cloned() else { return };
            s.nav.keys = true;
            s.nav.model.toggle(&path);
        }
        self.publish_navigator();
    }

    /// A folder opened by the hand; the home or a whole disk is unfolded and asked about first, as
    /// opening one reads every folder on the disk.
    pub(in crate::app) fn open_asked(self: &Rc<Self>, folder: &str) {
        let wide = self.state.borrow().nav.computer.is_wide(folder);
        if !wide {
            return self.open_folder(Path::new(folder));
        }
        {
            let mut s = self.state.borrow_mut();
            if !s.nav.model.unfolded.contains(folder) {
                s.nav.model.toggle(folder);
            }
        }
        self.publish_navigator();
        let name = self.state.borrow().nav.computer.name(folder);
        let target = folder.to_owned();
        self.ask(
            &tr!("Show Every Photo on “{name}”?", name = name),
            &tr("ONI reads every folder on it, which takes long on a full disk. A folder of scans inside it opens at once: pick one under it."),
            tr("Show Every Photo"),
            false,
            Pending::Run(Box::new(move |c| c.open_folder(Path::new(&target)))),
        );
    }

    pub(super) fn ancestor_chosen(&self, i: usize) {
        {
            let mut s = self.state.borrow_mut();
            let base = s.nav.model.base.clone();
            let mut ancestors: Vec<Option<String>> =
                base.as_deref().map(|b| s.nav.computer.ancestors(b)).unwrap_or_default().into_iter().map(Some).collect();
            ancestors.push(None);
            let Some(chosen) = ancestors.get(i).cloned() else { return };
            s.nav.model.go(chosen);
        }
        self.publish_navigator();
    }
}

impl State {
    /// What the column says in place of its rows: the base out of reach, or holding no folder.
    fn blank_words(&self) -> (String, String) {
        let model = &self.nav.model;
        let Some(base) = model.base.as_deref().filter(|_| !model.favourites_only) else { return Default::default() };
        let name = paths::name(base);
        let presence = self.row_presence(base);
        if !presence.is_present() {
            let (state, _) = reach::row_words(presence, base, &self.nav.computer);
            return (tr!("“{name}” Cannot Be Reached", name = name), state);
        }
        match model.listings.get(base) {
            Some(children) if children.is_empty() => (tr!("No Folder Inside “{name}”", name = name), tr("Parent shows the folders around it.").into()),
            _ => Default::default(),
        }
    }
}
