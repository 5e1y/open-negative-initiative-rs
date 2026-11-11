//! The cell menu: the line under a right click and what each line does.

use super::*;

/// What a cell's line does, drawn before its words; a project's lines wear the projects' glyph.
fn cell_glyph(action: &CellAction) -> MenuGlyph {
    match action {
        CellAction::Missing => MenuGlyph::Warning,
        CellAction::Copy => MenuGlyph::Copy,
        CellAction::Paste => MenuGlyph::Paste,
        CellAction::Reset => MenuGlyph::Reset,
        CellAction::NewRoll => MenuGlyph::Roll,
        CellAction::Ungroup => MenuGlyph::Ungroup,
        CellAction::NewProject | CellAction::AddToProject(_) | CellAction::RemoveFromProject(_) => MenuGlyph::Project,
        CellAction::AddTo => MenuGlyph::Add,
        CellAction::RemoveFrom => MenuGlyph::Remove,
        CellAction::Export => MenuGlyph::Export,
        CellAction::Reveal => MenuGlyph::Reveal,
        CellAction::Trash => MenuGlyph::Trash,
    }
}

impl Ctl {
    /// The menu a right click opens on the cell under it, selected alone first when outside the
    /// selection, so the photos the menu acts on are the ones marked; the ground opens none.
    pub(in crate::app) fn context(self: &Rc<Self>, x: f32, y: f32) -> bool {
        let (path, outside) = {
            let s = self.state.borrow();
            let Some(i) = s.layout.cell_at(x, y) else { return false };
            let Some(path) = s.order.get(i).cloned() else { return false };
            let outside = !s.shown_picks().selection.contains(&path);
            (path, outside)
        };
        if outside {
            {
                let mut s = self.state.borrow_mut();
                if !s.trash_shown() {
                    s.org.project_ring = None;
                }
                let picks = s.shown_picks_mut();
                picks.selection = HashSet::from([path.clone()]);
                picks.anchor = Some(path.clone());
                picks.cursor = Some(path.clone());
            }
            self.publish_tiles();
            self.publish_organising();
            self.publish_bar();
        }
        let can_paste = self.can_paste();
        let items = {
            let s = self.state.borrow();
            if s.trash_shown() {
                let scope = organise::cell_scope(&path, &s.trash_picks.selection);
                organise::trash_menu(&scope, &s.order)
                    .into_iter()
                    .enumerate()
                    .map(|(i, (text, rule, paths))| {
                        let (glyph, act) = if i == 0 { (MenuGlyph::Restore, Act::Restore(paths)) } else { (MenuGlyph::Reveal, Act::Reveal(paths)) };
                        Item { line: menu_line(glyph, &text, true, rule, false), act }
                    })
                    .collect()
            } else {
                let scope = organise::cell_scope(&path, &s.picks.selection);
                self.cell_items(&s, &path, &scope, false, can_paste)
            }
        };
        self.open_menu(items, Some(path))
    }

    /// A right click on the canvas: the open photo's cell menu, on the frames shown with it.
    pub(in crate::app) fn canvas_menu(self: &Rc<Self>) -> bool {
        let can_paste = self.can_paste();
        let (items, path) = {
            let s = self.state.borrow();
            let Some(path) = s.open.clone() else { return false };
            let scope = organise::cell_scope(&path, &s.picks.selection);
            (self.cell_items(&s, &path, &scope, false, can_paste), path)
        };
        self.open_menu(items, Some(path))
    }

    pub(super) fn can_paste(&self) -> bool {
        self.state.borrow_mut().store.editor().is_some_and(|e| e.has_copied_settings())
    }

    pub(super) fn cell_items(&self, s: &State, path: &str, scope: &HashSet<String>, edit: bool, can_paste: bool) -> Vec<Item> {
        let rolled: HashSet<String> = s.rows.iter().filter(|e| e.film.is_some()).map(|e| e.path.clone()).collect();
        let projects: Vec<(Id, String)> = s.org.projects.iter().map(|p| (p.id, p.name.clone())).collect();
        let rows = s.reach();
        let reach = project::Reach::new(&rows, &s.org.found);
        let member_of: Vec<(Id, String)> =
            project::member_projects(&s.org.projects, &reach, scope).into_iter().map(|p| (p.id, p.name.clone())).collect();
        let facts = CellFacts { missing: &s.org.missing, rolled: &rolled, can_paste, projects, member_of };
        let mut items = Vec::new();
        if edit && scope.len() == 1 {
            items.push(Item { line: menu_line(MenuGlyph::Edit, tr("Edit"), true, false, false), act: Act::Edit(path.to_owned()) });
        }
        for (i, line) in organise::cell_menu(scope, &s.order, &facts).into_iter().enumerate() {
            let opens = matches!(line.action, CellAction::AddTo | CellAction::RemoveFrom);
            let rule = line.rule || (edit && scope.len() == 1 && i == 0);
            let keys = line.action.chord(crate::platform::Platform::THIS).into();
            let glyph = cell_glyph(&line.action);
            items.push(Item { line: MenuLine { keys, ..menu_line(glyph, &line.text, line.enabled, rule, opens) }, act: Act::Cell(line) });
        }
        items
    }

    pub(super) fn open_menu(&self, items: Vec<Item>, path: Option<String>) -> bool {
        let Some(w) = self.window() else { return false };
        if items.is_empty() {
            return false;
        }
        let lines: Vec<MenuLine> = items.iter().map(|i| i.line.clone()).collect();
        {
            let mut s = self.state.borrow_mut();
            let projects: Vec<(Id, String)> = s.org.projects.iter().map(|p| (p.id, p.name.clone())).collect();
            // The menu's whole scope, read off New Project, which leaves no photo out, for the submenus.
            let scope: HashSet<String> = items
                .iter()
                .find_map(|i| match &i.act {
                    Act::Cell(l) if l.action == CellAction::NewProject => Some(l.scope.iter().cloned().collect()),
                    _ => None,
                })
                .unwrap_or_default();
            let rows = s.reach();
            let member: Vec<(Id, String)> = project::member_projects(&s.org.projects, &project::Reach::new(&rows, &s.org.found), &scope)
                .into_iter()
                .map(|p| (p.id, p.name.clone()))
                .collect();
            drop(rows);
            s.org.menu_lists = (projects, member);
            s.org.menu = items;
            s.org.sub_menu.clear();
            s.org.menu_path = path;
        }
        let lib = w.global::<Library>();
        lib.set_menu_lines(ModelRc::new(VecModel::from(lines)));
        lib.set_menu_sub_lines(ModelRc::default());
        true
    }

    /// The submenu an opener's line lays beside the menu, or none for any other line.
    pub(in crate::app) fn menu_opened(self: &Rc<Self>, i: Option<usize>) {
        let opener = i.and_then(|i| self.state.borrow().org.menu.get(i).cloned());
        let sub: Vec<Item> = match opener {
            Some(Item { act: Act::Cell(line), line: shown }) if shown.enabled && matches!(line.action, CellAction::AddTo | CellAction::RemoveFrom) => {
                let (projects, member) = self.state.borrow().org.menu_lists.clone();
                let none = HashSet::new();
                let facts = CellFacts { missing: &none, rolled: &none, can_paste: false, projects, member_of: member };
                organise::submenu(&line, &facts)
                    .into_iter()
                    .map(|l| Item { line: menu_line(cell_glyph(&l.action), &l.text, true, false, false), act: Act::Cell(l) })
                    .collect()
            }
            _ => Vec::new(),
        };
        let lines: Vec<MenuLine> = sub.iter().map(|i| i.line.clone()).collect();
        self.state.borrow_mut().org.sub_menu = sub;
        if let Some(w) = self.window() {
            w.global::<Library>().set_menu_sub_lines(ModelRc::new(VecModel::from(lines)));
        }
    }

    /// A line of the submenu shown, which closes the menu.
    pub(in crate::app) fn menu_sub_chosen(self: &Rc<Self>, i: usize) -> bool {
        let item = self.state.borrow().org.sub_menu.get(i).cloned();
        if let Some(Item { act: Act::Cell(line), .. }) = item {
            self.cell_act(line);
        }
        false
    }

    /// A line of the open menu; answers whether the menu stays, which a submenu's opener does.
    pub(in crate::app) fn menu_chosen(self: &Rc<Self>, i: usize) -> bool {
        let item = self.state.borrow().org.menu.get(i).cloned();
        let Some(item) = item else { return false };
        if !item.line.enabled {
            return true;
        }
        match item.act {
            Act::Cell(line) if matches!(line.action, CellAction::AddTo | CellAction::RemoveFrom) => {
                self.menu_opened(Some(i));
                return true;
            }
            Act::Cell(line) => self.cell_act(line),
            Act::Edit(path) => self.open_in_editor(&path),
            Act::RenameProject(id) => self.rename_project(id),
            Act::DeleteProject(id) => self.ask_delete_project(id),
            Act::Restore(paths) => {
                self.state.borrow_mut().trash_picks.selection = paths.iter().cloned().collect();
                self.restore(&paths);
            }
            Act::Reveal(paths) => system::reveal_all(&paths.iter().map(std::path::PathBuf::from).collect::<Vec<_>>()),
            Act::OpenTrash => self.open_trash(),
            Act::RestoreAll => self.ask_restore_all(),
            Act::Folder(act, path) => self.folder_acted(act, &path, true),
        }
        self.publish_bar();
        false
    }

    /// Copy Settings from the bar and the menu bar: the open frame's; in the gallery, the one photo
    /// picked, as its cell menu's line copies it, so the chord that line prints means one thing.
    pub(in crate::app) fn copy_settings(self: &Rc<Self>) {
        if !self.window().is_some_and(|w| w.global::<App>().get_gallery()) {
            return self.ask_copy(None);
        }
        if let [path] = self.targets().as_slice() {
            self.ask_copy(Some(path.clone()));
        }
    }

    /// File > New Project: the photos the gestures reach now, as the cell menu's line on them; with
    /// none, nothing, as the menu shows it.
    pub(in crate::app) fn new_project_from_targets(self: &Rc<Self>) {
        let targets: HashSet<String> = self.targets().into_iter().collect();
        self.new_project(&targets);
    }

    /// Add to Project, asked first when it takes photos from another project, as a photo sits in one;
    /// then said, what joined and what was there already, with Undo.
    fn ask_add_to_project(self: &Rc<Self>, scope: HashSet<String>, id: Id) {
        let (target, asked) = {
            let s = self.state.borrow();
            let target = s.org.projects.iter().find(|p| p.id == id).map(|p| p.name.clone()).unwrap_or_default();
            let rows = s.reach();
            let leaving = project::leaving(&s.org.projects, &project::Reach::new(&rows, &s.org.found), &scope, id);
            let lines: Vec<String> = leaving
                .iter()
                .map(|(p, n)| trn("One photo leaves “{project}”.", "{n} photos leave “{project}”.", *n as u64).replace("{project}", &p.name))
                .collect();
            (target, (!lines.is_empty()).then(|| lines.join(" ")))
        };
        let named = target.clone();
        let add = move |c: &Rc<Ctl>| {
            let mut joined = project::Joined::default();
            c.regroup(tr("Add to Project"), |projects, reach| {
                joined = project::add_to(projects, reach, &scope, id);
                joined.added > 0
            });
            c.said_joined(&named, joined);
        };
        match asked {
            None => add(self),
            Some(lines) => {
                let message = tr!("{lines} A photo sits in one project at a time; a project left empty stays.", lines = lines);
                self.ask(&tr!("Move to “{project}”?", project = target), &message, tr("Move"), false, Pending::Run(Box::new(add)));
            }
        }
    }

    /// What Add to Project did, in the notice: the photos that joined, with Undo, and those already there.
    fn said_joined(&self, project: &str, joined: project::Joined) {
        let there = |n: usize| trn("One was already in it.", "{n} were already in it.", n as u64);
        let message = match joined {
            project::Joined { added: 0, already } => trn("The photo is already in “{project}”.", "The {n} photos are already in “{project}”.", already as u64),
            project::Joined { added, already: 0 } => trn("One photo added to “{project}”.", "{n} photos added to “{project}”.", added as u64),
            project::Joined { added, already } => {
                format!("{} {}", trn("One photo added to “{project}”.", "{n} photos added to “{project}”.", added as u64), there(already))
            }
        };
        let message = message.replace("{project}", project);
        if joined.added > 0 { self.told_step(tr("Add to Project"), &message) } else { self.told(tr("Add to Project"), &message) }
    }

    /// Remove from Project, said with Undo, as a photo leaving a project leaves no other trace, and
    /// where one still shows, so a filing it reveals is never news later.
    fn remove_from_project(self: &Rc<Self>, scope: &HashSet<String>, id: Id) {
        let name = self.state.borrow().org.projects.iter().find(|p| p.id == id).map(|p| p.name.clone()).unwrap_or_default();
        let mut said = String::new();
        if self.regroup(tr("Remove from Project"), |projects, reach| {
            let gone = project::remove_from(projects, reach, scope, id);
            said = project::removed_said(&name, gone.len(), &project::shown_in(projects, reach, &gone.iter().cloned().collect()));
            !gone.is_empty()
        }) {
            self.told_step(tr("Remove from Project"), &said);
        }
    }

    /// `CellMenu`: each verb selects its scope first, anchored on the cell, then acts on it.
    pub(super) fn cell_act(self: &Rc<Self>, line: CellLine) {
        let scope: HashSet<String> = line.scope.iter().cloned().collect();
        if !matches!(line.action, CellAction::Copy | CellAction::Reveal | CellAction::Trash) {
            let mut s = self.state.borrow_mut();
            let path = s.org.menu_path.clone();
            s.picks.selection = scope.clone();
            s.picks.anchor = path.clone();
            s.picks.cursor = path;
        }
        match line.action {
            CellAction::Copy => {
                if let [path] = line.scope.as_slice() {
                    self.ask_copy(Some(path.clone()));
                }
            }
            CellAction::Paste => self.paste(),
            CellAction::Reset => self.reset_all(),
            CellAction::NewRoll => self.new_roll(),
            CellAction::Ungroup => {
                let paths = line.scope.clone();
                self.gesture(&paths, |store| store.apply_film(&paths, None, tr("Ungroup")));
            }
            CellAction::NewProject => {
                self.new_project(&scope);
            }
            CellAction::AddToProject(id) => self.ask_add_to_project(scope, id),
            CellAction::RemoveFromProject(id) => self.remove_from_project(&scope, id),
            CellAction::Export => self.act(crate::Act::Export),
            CellAction::Reveal => system::reveal_all(&line.scope.iter().map(std::path::PathBuf::from).collect::<Vec<_>>()),
            CellAction::Trash => self.move_paths_to_trash(&line.scope),
            CellAction::AddTo | CellAction::RemoveFrom | CellAction::Missing => {}
        }
        // The photos left out for their originals, said once for the whole verb.
        if line.left > 0 {
            self.told(&line.text, &oni_library::refusal::left_out(line.left));
        }
        self.lay_grid();
        self.publish_bar();
    }
}
