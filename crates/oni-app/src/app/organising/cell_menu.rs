//! The cell menu: the line under a right click and what each line does.

use super::*;

impl Ctl {
    /// The menu a right click opens on the cell under it; the ground opens none.
    pub(in crate::app) fn context(self: &Rc<Self>, x: f32, y: f32) -> bool {
        let path = {
            let s = self.state.borrow();
            let Some(i) = s.layout.index_at(x, y) else { return false };
            let Some(path) = s.order.get(i).cloned() else { return false };
            path
        };
        let can_paste = self.can_paste();
        let items = {
            let s = self.state.borrow();
            if s.trash_shown() {
                let scope = organise::cell_scope(&path, &s.trash_picks.selection);
                organise::trash_menu(&scope, &s.order)
                    .into_iter()
                    .enumerate()
                    .map(|(i, (text, rule, paths))| Item {
                        line: menu_line(&text, true, rule, false),
                        act: if i == 0 { Act::Restore(paths) } else { Act::Reveal(paths) },
                    })
                    .collect()
            } else {
                let scope = organise::cell_scope(&path, &s.picks.selection);
                self.cell_items(&s, &path, &scope, false, can_paste)
            }
        };
        self.open_menu(items, Some(path))
    }

    pub(super) fn can_paste(&self) -> bool {
        self.state.borrow_mut().store.editor().is_some_and(|e| e.has_copied_settings())
    }

    pub(super) fn cell_items(&self, s: &State, path: &str, scope: &HashSet<String>, edit: bool, can_paste: bool) -> Vec<Item> {
        let rolled: HashSet<String> = s.rows.iter().filter(|e| e.film.is_some()).map(|e| e.path.clone()).collect();
        let projects: Vec<(Id, String)> = s.org.projects.iter().map(|p| (p.id, p.name.clone())).collect();
        let member_of: Vec<(Id, String)> =
            project::member_projects(&s.org.projects, &s.reach(), scope).into_iter().map(|p| (p.id, p.name.clone())).collect();
        let facts = CellFacts { missing: &s.org.missing, rolled: &rolled, can_paste, projects, member_of };
        let mut items = Vec::new();
        if edit && scope.len() == 1 {
            items.push(Item { line: menu_line(tr("Edit"), true, false, false), act: Act::Edit(path.to_owned()) });
        }
        for (i, line) in organise::cell_menu(scope, &s.order, &facts).into_iter().enumerate() {
            let opens = matches!(line.action, CellAction::AddTo | CellAction::RemoveFrom);
            let rule = line.rule || (edit && scope.len() == 1 && i == 0);
            items.push(Item { line: menu_line(&line.text, line.enabled, rule, opens), act: Act::Cell(line) });
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
            let member: Vec<(Id, String)> =
                project::member_projects(&s.org.projects, &s.reach(), &scope).into_iter().map(|p| (p.id, p.name.clone())).collect();
            s.org.menu_lists = (projects, member);
            s.org.menu = items;
            s.org.menu_path = path;
        }
        w.global::<Library>().set_menu_lines(ModelRc::new(VecModel::from(lines)));
        true
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
                let (projects, member) = self.state.borrow().org.menu_lists.clone();
                let none = HashSet::new();
                let facts = CellFacts { missing: &none, rolled: &none, can_paste: false, projects, member_of: member };
                let sub: Vec<Item> = organise::submenu(&line, &facts)
                    .into_iter()
                    .map(|l| Item { line: menu_line(&l.text, true, false, false), act: Act::Cell(l) })
                    .collect();
                let path = self.state.borrow().org.menu_path.clone();
                return self.open_menu(sub, path);
            }
            Act::Cell(line) => self.cell_act(line),
            Act::Edit(path) => self.open_in_editor(&path),
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

    /// Add to Project, asked first when it takes photos from another project, as a photo sits in one.
    fn ask_add_to_project(self: &Rc<Self>, scope: HashSet<String>, id: Id) {
        let asked = {
            let s = self.state.borrow();
            let rows = s.reach();
            let target = s.org.projects.iter().find(|p| p.id == id).map(|p| p.name.clone()).unwrap_or_default();
            let leaving = project::leaving(&s.org.projects, &rows, &scope, id);
            let lines: Vec<String> = leaving
                .iter()
                .map(|(p, n)| trn("One photo leaves “{project}”.", "{n} photos leave “{project}”.", *n as u64).replace("{project}", &p.name))
                .collect();
            (!lines.is_empty()).then(|| (target, lines.join(" ")))
        };
        let add = move |c: &Rc<Ctl>| {
            let added = {
                let mut s = c.state.borrow_mut();
                let rows = s.reach().into_owned();
                project::add_to(&mut s.org.projects, &rows, &scope, id)
            };
            if added {
                c.save_projects();
            }
            c.lay_grid();
            c.publish_bar();
        };
        match asked {
            None => add(self),
            Some((target, lines)) => {
                let message = tr!("{lines} A photo sits in one project at a time; a project left empty stays.", lines = lines);
                self.ask(&tr!("Move to “{project}”?", project = target), &message, tr("Move"), false, Pending::Run(Box::new(add)));
            }
        }
    }

    /// `CellMenu`: each verb selects its scope first, anchored on the cell, then acts on it.
    pub(super) fn cell_act(self: &Rc<Self>, line: CellLine) {
        let scope: HashSet<String> = line.scope.iter().cloned().collect();
        if !matches!(line.action, CellAction::Reveal | CellAction::Trash) {
            let mut s = self.state.borrow_mut();
            let path = s.org.menu_path.clone();
            s.picks.selection = scope.clone();
            s.picks.anchor = path.clone();
            s.picks.cursor = path;
        }
        match line.action {
            CellAction::Paste => self.paste(),
            CellAction::Reset => self.reset_all(),
            CellAction::ResetToDefault => self.act(crate::Act::ResetToDefault),
            CellAction::Medal(m) => self.label_paths(&line.scope, m),
            CellAction::NewRoll => self.new_roll(),
            CellAction::Ungroup => {
                let paths = line.scope.clone();
                self.gesture(&paths, |store| store.set_film(&paths, None, tr("Ungroup")));
            }
            CellAction::NewProject => self.new_project(&scope),
            CellAction::AddToProject(id) => self.ask_add_to_project(scope, id),
            CellAction::RemoveFromProject(id) => {
                {
                    let mut s = self.state.borrow_mut();
                    let rows = s.reach().into_owned();
                    project::remove_from(&mut s.org.projects, &rows, &scope, id);
                }
                self.save_projects();
            }
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
