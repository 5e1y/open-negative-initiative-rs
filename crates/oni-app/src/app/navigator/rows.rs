//! The navigator's rows: the favourites' section and Computer's, each folder's state, a click opening,
//! the chevron folding and a section's heading folding its rows.

use super::*;

impl Ctl {
    /// The rows and the welcome as they stand; every folder's own listing is asked so its chevron
    /// knows whether it holds a folder.
    pub(in crate::app) fn publish_navigator(&self) {
        let Some(w) = self.window() else { return };
        let published = {
            let mut s = self.state.borrow_mut();
            let favourites = s.nav.favourites.paths.clone();
            let rows = s.nav.model.rows(&favourites);
            for (path, _) in rows.iter().filter(|(p, _)| !walk::is_heading(p)) {
                s.nav.model.list(path, false);
            }
            let rolls = s.nav.rolls();
            let opened = if s.org.trash_open { None } else { s.opened.clone() };
            let model = &s.nav.model;
            let side = crate::stocks::row_side(&w);
            // Two favourites named alike carry the folders above them, in their own section alone.
            let trails = walk::trails(&favourites);
            let mut section = walk::COMPUTER_ROW;
            let paths: Vec<String> = rows.iter().map(|(p, _)| p.clone()).collect();
            // A folder both sections show is picked and typed over in one row alone.
            let picked = model.picked_row(&paths);
            let typed = model.renaming.as_deref().and_then(|r| model.row_of(&paths, r));
            // Beside a photo, its folder's row, which a list made anew scrolls to as it first draws.
            let reveal = s.nav.current.as_deref().and_then(|c| model.row_of(&paths, c)).map_or(-1, |i| i as i32);
            let lines: Vec<FolderLine> = rows
                .iter()
                .enumerate()
                .map(|(i, (path, depth))| {
                    if walk::is_heading(path) {
                        section = if path == walk::FAVOURITES_ROW { walk::FAVOURITES_ROW } else { walk::COMPUTER_ROW };
                        let title = if section == walk::FAVOURITES_ROW { tr("Favourites") } else { tr("Computer") };
                        return FolderLine { title: title.into(), depth: 0, chevron: true, unfolded: !model.folded.contains(path), kind: 3, present: true, ..FolderLine::default() };
                    }
                    let presence = s.row_presence(path);
                    let (caption, help) = reach::row_words(presence, path, &s.nav.computer);
                    let title = s.nav.computer.name(path);
                    let (head, tail) = walk::split_end(&title);
                    let roll = rolls.contains(path);
                    let stock = roll.then(|| crate::stocks::row_icon(s.nav.stock(path), side)).flatten();
                    FolderLine {
                    title: title.into(),
                    head: head.into(),
                    tail: tail.into(),
                    trail: trails.get(path).filter(|_| *depth == 0 && section == walk::FAVOURITES_ROW).cloned().unwrap_or_default().into(),
                    path: path.clone().into(),
                    depth: *depth as i32,
                    // Out of reach, it promises no folder inside.
                    chevron: presence.is_present() && model.listings.get(path).is_none_or(|c| !c.is_empty()),
                    unfolded: model.is_unfolded(section, path),
                    kind: if roll {
                        1
                    } else if opened.as_deref() == Some(path.as_str()) {
                        2
                    } else {
                        0
                    },
                    picked: picked == Some(i),
                    opened: opened.as_deref() == Some(path.as_str()),
                    favourite: s.nav.favourites.contains(path),
                    present: presence.is_present(),
                    reach: reach::reach(presence),
                    caption: caption.into(),
                    help: help.into(),
                    renaming: typed == Some(i),
                    has_stock: stock.is_some(),
                    stock: stock.unwrap_or_default(),
                    unprepared: presence.is_present() && s.nav.unprepared.contains(path),
                }})
                .collect();
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
                        tr!("Add {name}", name = name)
                    };
                    WelcomeLine { title: title.into(), path: p.clone().into(), enabled: !away }
                })
                .collect();
            let welcome_shown = s.opened.is_none() && !s.nav.welcome.is_empty();
            s.nav.rows = paths;
            let mut refused: Vec<String> = s.nav.model.refused.iter().cloned().collect();
            refused.extend(s.opened.clone().filter(|o| s.survey.unreadable.contains(o)));
            *s.nav.refused.lock().expect("refused") = refused;
            s.remember();
            (lines, welcome, welcome_shown, reveal)
        };
        self.list_wanted();
        let (lines, welcome, welcome_shown, reveal) = published;
        let n = w.global::<crate::Navigator>();
        n.set_rows(ModelRc::new(VecModel::from(lines)));
        n.set_reveal_row(reveal);
        n.set_welcome(ModelRc::new(VecModel::from(welcome)));
        n.set_welcome_shown(welcome_shown);
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
    /// says its state; the second of a double click opens nothing more. A heading folds its section.
    pub(super) fn navigator_row_clicked(self: &Rc<Self>, i: usize) {
        if self.state.borrow().nav.rows.get(i).is_some_and(|p| walk::is_heading(p)) {
            return self.navigator_row_folded(i);
        }
        let open = {
            let mut s = self.state.borrow_mut();
            let Some(path) = s.nav.rows.get(i).cloned() else { return };
            let now = Instant::now();
            let count = match &s.nav.clicks {
                Some((last, at, n)) if *last == path && now.duration_since(*at) <= crate::system::double_click() => n + 1,
                _ => 1,
            };
            s.nav.clicks = Some((path.clone(), now, count));
            s.org.trash_picked = None;
            let s = &mut *s;
            s.nav.model.pick(&s.nav.rows, i);
            s.nav.model.click(&path, count)
        };
        self.publish_trash_rows();
        self.publish_navigator();
        if let Some(folder) = open {
            self.open_asked(&folder);
        }
    }

    /// The chevron alone: the row folds or unfolds, and nothing opens; a heading folds its section.
    pub(super) fn navigator_row_folded(self: &Rc<Self>, i: usize) {
        {
            let mut s = self.state.borrow_mut();
            let Some(path) = s.nav.rows.get(i).cloned() else { return };
            if walk::is_heading(&path) {
                s.nav.model.toggle_section(&path);
            } else {
                let heading = walk::heading_above(&s.nav.rows, i);
                s.nav.model.toggle_in(heading, &path);
            }
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
}
