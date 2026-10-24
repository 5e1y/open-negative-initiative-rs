//! The navigator's rows: Computer, the base's folders and the favourites, each one's state, a click
//! unfolding and a second opening.

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
            let lines: Vec<FolderLine> = rows
                .iter()
                .map(|(path, depth)| {
                    let presence = s.row_presence(path);
                    let (caption, help) = reach::row_words(presence, path, &s.nav.computer);
                    FolderLine {
                    title: paths::name(path).into(),
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
                ancestors.iter().map(|a| FolderChoice { title: walk::title(a.as_deref()).into(), current: *a == base }).collect();
            let header = (
                model.can_go_up(),
                model.favourites_only,
                if model.favourites_only { tr("Favourites").to_string() } else { walk::title(base.as_deref()) },
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
                    let title = if away {
                        let state = reach::row_words(Presence::Offline(Away::Disconnected), p, &s.nav.computer).0;
                        tr!("{name} — {state}", name = paths::name(p), state = state)
                    } else {
                        tr!("Add {name}", name = paths::name(p)).to_uppercase()
                    };
                    WelcomeLine { title: title.into(), path: p.clone().into(), enabled: !away }
                })
                .collect();
            let welcome_shown = s.opened.is_none() && !s.nav.welcome.is_empty();
            let opened_pill = (s.opened.as_deref().map(paths::name).unwrap_or_default(), s.opened.clone().unwrap_or_default());
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

    /// The listings asked and not under way, read on a few threads; each thread lands its share at once.
    pub(super) fn list_wanted(&self) {
        let wanted = self.state.borrow_mut().nav.model.take_wanted();
        if wanted.is_empty() {
            return;
        }
        let share = wanted.len().div_ceil(LISTERS);
        let computer = self.state.borrow().nav.computer.clone();
        for chunk in wanted.chunks(share) {
            let (chunk, computer) = (chunk.to_vec(), computer.clone());
            let _ = std::thread::Builder::new().name("navigator".into()).spawn(move || {
                let listed: Vec<(String, Option<Vec<String>>)> = chunk.into_iter().map(|f| {
                    let children = walk::children(&f);
                    (f, children)
                }).collect();
                // A folder refused is told offline or missing here, off the interface thread.
                let found: Vec<(String, Presence)> =
                    listed.iter().map(|(f, c)| (f.clone(), if c.is_some() { Presence::Present } else { presence::of(f, &computer) })).collect();
                with_shell(move |c| {
                    c.state.borrow_mut().noted(found);
                    c.listed(listed);
                });
            });
        }
    }

    pub(super) fn listed(self: &Rc<Self>, listed: Vec<(String, Option<Vec<String>>)>) {
        let (opening, moved) = {
            let mut s = self.state.borrow_mut();
            let mut moved = false;
            let mut opening = Vec::new();
            for (folder, children) in listed {
                let landed = match children {
                    Some(children) => s.nav.model.listed(&folder, children),
                    None => s.nav.model.refused(&folder),
                };
                moved |= landed != Landed::Same;
                if let Landed::Open(folder) = landed {
                    opening.push(folder);
                }
            }
            (opening, moved)
        };
        if moved {
            self.publish_navigator();
        }
        for folder in opening {
            self.open_folder(Path::new(&folder));
        }
    }

    /// `NavigatorRow.onTapGesture`: the click's count read as AppKit counts it, then `click`.
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
            let roll = s.nav.rolls().contains(&path);
            s.org.trash_picked = None;
            // Out of reach, a click shows its state in the gallery, as no listing will answer it.
            if s.row_presence(&path).is_present() {
                s.nav.model.click(&path, count, roll)
            } else {
                s.nav.model.selected = Some(path.clone());
                (count == 1).then_some(path)
            }
        };
        self.publish_trash_rows();
        self.publish_navigator();
        if let Some(folder) = open {
            self.open_folder(Path::new(&folder));
        }
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
