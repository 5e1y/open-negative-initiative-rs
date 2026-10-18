//! The gallery's organising in the window (`GalleryEngine`, `CellMenu`, `InfoSidebar`): picks, the
//! cell menu, Archive and Projects, the ONI Trash laid as a grid, the medals and the info sidebar.

use super::{Ctl, Pending, State, stem};
use crate::organise::{self, CellAction, CellFacts, CellLine, GridKey, Outcome, Picks, Shown};
use crate::{App, Confirm, Library, MenuLine, StockCount, TrashLine, TreeRow};
use crate::{library, picking, system};
use oni_library::Entry;
use oni_library::project::{self, Project, ProjectsFile};
use oni_library::project_tree::{self, Node, NodeId};
use oni_settings::film::{default_stock_id, film_stock};
use oni_settings::{Date, Film, Id, Medal};
use slint::{ComponentHandle, Image, ModelRc, SharedString, VecModel};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::Instant;

/// `DragGesture(minimumDistance: 4)`: a press moved this far is a drag, never a click.
pub const DRAG_DISTANCE: f32 = 4.0;
/// The drop line's thickness, `DS.Stroke.emphasis`.
const DROP_LINE: f32 = 2.0;

/// One band of the grid as shown: its key, its heading and its photos in order.
#[derive(Clone, Debug)]
pub struct Band {
    pub key: String,
    pub title: String,
    pub caption: String,
    pub entries: Vec<Entry>,
}

/// What a line of the open context menu does.
#[derive(Clone, Debug)]
enum Act {
    Cell(CellLine),
    Edit(String),
    DeleteProject(Id),
    Restore(Vec<String>),
    Reveal(Vec<String>),
    OpenTrash,
    RestoreAll,
}

#[derive(Clone, Debug)]
struct Item {
    line: MenuLine,
    act: Act,
}

/// A reorder under way: the photos it carries, the one under the pointer, the boundary it would
/// land before, the ghost's place in document coordinates.
#[derive(Clone, Debug)]
struct Reorder {
    sources: HashSet<String>,
    lead: String,
    size: (f32, f32),
    boundary: Option<usize>,
    at: (f32, f32),
}

/// The roll the info sidebar stands on, and where writing it goes: the folder's marker, or every
/// member's sidecar.
#[derive(Clone, Debug, PartialEq)]
struct Roll {
    film: Film,
    marker: Option<String>,
    paths: Vec<String>,
    first: Option<Entry>,
}

#[derive(Default)]
pub struct Organising {
    pub projects_surface: bool,
    pub trash_open: bool,
    pub trash_unfolded: bool,
    /// The ONI Trash's picked row: `""` the trash itself, else a folder's path.
    pub trash_picked: Option<String>,
    pub trashed: Vec<Entry>,
    pub projects: Vec<Project>,
    pub project_ring: Option<NodeId>,
    pub project_expanded: HashSet<NodeId>,
    pub project_nodes: Vec<NodeId>,
    pub bands: Vec<Band>,
    pub missing: HashSet<String>,
    menu: Vec<Item>,
    menu_path: Option<String>,
    menu_lists: (Vec<(Id, String)>, Vec<(Id, String)>),
    press: Option<(f32, f32)>,
    reorder: Option<Reorder>,
    /// What the sidebar's fields were last loaded from, so a refresh never overwrites typing.
    info_loaded: Option<String>,
}

fn photos(n: usize) -> String {
    if n == 1 { "1 photo".into() } else { format!("{n} photos") }
}

/// Seconds on a clock that only goes forward, as `systemUptime` is.
fn uptime() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

fn home() -> String {
    std::env::var("HOME").unwrap_or_default()
}

fn menu_line(text: &str, enabled: bool, rule: bool, opens: bool) -> MenuLine {
    MenuLine { text: text.into(), enabled, rule, opens }
}

impl State {
    /// The picks the grid shows: the ONI Trash keeps its own, which no other gesture reaches.
    pub(super) fn shown_picks(&self) -> &Picks {
        if self.trash_shown() { &self.trash_picks } else { &self.picks }
    }

    fn shown_picks_mut(&mut self) -> &mut Picks {
        if self.trash_shown() { &mut self.trash_picks } else { &mut self.picks }
    }

    /// `case .library where trashOpen`: the trash is the archive's, never the projects'.
    pub(super) fn trash_shown(&self) -> bool {
        self.org.trash_open && !self.org.projects_surface
    }

    /// The photos selected, in the order the grid shows them.
    pub(super) fn selected(&self) -> Vec<String> {
        let picks = &self.picks.selection;
        let mut out: Vec<String> = self.order.iter().filter(|p| picks.contains(*p)).cloned().collect();
        let mut rest: Vec<String> = picks.iter().filter(|p| !out.contains(p)).cloned().collect();
        rest.sort();
        out.extend(rest);
        out
    }

    fn band_keys(&self) -> Vec<(String, Vec<String>)> {
        self.org.bands.iter().map(|b| (b.key.clone(), b.entries.iter().map(|e| e.path.clone()).collect())).collect()
    }

    fn entry(&self, path: &str) -> Option<&Entry> {
        self.rows.iter().chain(&self.org.trashed).find(|e| e.path == path)
    }

    fn bands(&self) -> Vec<Band> {
        if self.org.projects_surface {
            // On the rows, never the medal-filtered ones: a project is hand-picked.
            return project::grouped(&self.org.projects, &self.rows)
                .into_iter()
                .filter(|g| !g.entries.is_empty())
                .map(|g| Band { key: g.id.to_string(), title: g.project.name.clone(), caption: photos(g.entries.len()), entries: g.entries })
                .collect();
        }
        if self.org.trash_open {
            let home = home();
            return oni_library::trash::groups(&self.org.trashed, &home)
                .into_iter()
                .map(|g| Band {
                    key: g.heading.clone().unwrap_or_default(),
                    title: g.heading.clone().unwrap_or_default(),
                    caption: photos(g.entries.len()),
                    entries: g.entries,
                })
                .collect();
        }
        self.archive
            .as_ref()
            .map(|a| {
                a.groups
                    .iter()
                    .enumerate()
                    .map(|(i, g)| Band { key: format!("{i}:{}", g.title), title: g.title.clone(), caption: g.caption.clone(), entries: g.entries.clone() })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The roll a click or the anchor stands on: the folder's marker, else the roll every
    /// member's sidecar names; a folder of loose photos is no roll.
    fn roll(&self) -> Option<Roll> {
        let anchor = self.picks.anchor.as_deref()?;
        let group = self.archive.as_ref()?.groups.iter().find(|g| g.entries.iter().any(|e| e.path == anchor))?;
        let paths: Vec<String> = group.entries.iter().map(|e| e.path.clone()).collect();
        let first = group.entries.first().cloned();
        if let Some(film) = &group.film {
            return Some(Roll { film: film.clone(), marker: Some(oni_library::paths::parent(anchor)), paths, first });
        }
        let film = group.entries.first()?.film.clone()?;
        group.entries.iter().all(|e| e.film.as_ref().is_some_and(|f| f.id == film.id)).then_some(Roll { film, marker: None, paths, first })
    }

    fn project_group(&self) -> Option<project::ProjectGroup> {
        let ring = match &self.org.project_ring {
            Some(NodeId::Project(id)) => Some(*id),
            Some(NodeId::Photo { project, .. }) => Some(*project),
            None => None,
        };
        oni_library::info::resolved_project(&self.org.projects, &self.rows, ring, self.picks.anchor.as_deref())
    }
}

impl Ctl {
    // MARK: - The grid as shown

    /// The bands the surface shows, the selection kept to them, then the tiles, the tree's other
    /// rows, the status bar, the sidebar and the empty state.
    pub(super) fn lay_grid(self: &Rc<Self>) {
        {
            let mut s = self.state.borrow_mut();
            let bands = s.bands();
            let order: Vec<String> = bands.iter().flat_map(|b| b.entries.iter().map(|e| e.path.clone())).collect();
            for e in bands.iter().flat_map(|b| &b.entries) {
                s.fingerprints.entry(e.path.clone()).or_insert_with(|| e.fingerprint.clone());
            }
            s.org.missing = order.iter().filter(|p| !Path::new(p).exists()).cloned().collect();
            let shown: HashSet<String> = order.iter().cloned().collect();
            s.shown_picks_mut().selection.retain(|p| shown.contains(p));
            s.order = order;
            s.org.bands = bands;
        }
        self.relayout();
        self.publish_organising();
    }

    pub(super) fn publish_organising(&self) {
        self.publish_projects();
        self.publish_trash_rows();
        self.publish_status();
        self.publish_info();
        self.publish_empty();
    }

    fn publish_empty(&self) {
        let Some(w) = self.window() else { return };
        let s = self.state.borrow();
        let (title, detail, action) = if !s.order.is_empty() {
            (String::new(), String::new(), String::new())
        } else if s.org.projects_surface {
            ("No Photos in a Project".into(), "Right-click photos in the library and choose New Project from them.".into(), String::new())
        } else if s.org.trash_open {
            ("ONI Trash Is Empty".into(), "A photo deleted in ONI waits here, its file untouched, until it is restored.".into(), String::new())
        } else if let Some(m) = s.filter {
            (format!("No {} Photos", library::medal(m)), "Cycle the filter (⌘L) to see the others.".into(), String::new())
        } else if s.opened.is_none() {
            (
                "No Folder Open".into(),
                "Double-click a folder in the column to open it: every photo under it shows here. Nothing is copied or moved.".into(),
                "Open Folder…".into(),
            )
        } else {
            ("No Photos".into(), "No photo under this folder.".into(), "Open Folder…".into())
        };
        let lib = w.global::<Library>();
        lib.set_empty_title(title.into());
        lib.set_empty_detail(detail.into());
        lib.set_empty_action(action.into());
        lib.set_projects_empty(if s.org.projects.is_empty() { "No Projects" } else { "" }.into());
    }

    /// The tile flags the bands carry: medal, missing original, a triplet's lead.
    pub(super) fn tile_flags(s: &State) -> HashMap<String, (i32, bool, bool)> {
        s.org
            .bands
            .iter()
            .flat_map(|b| &b.entries)
            .map(|e| (e.path.clone(), (library::medal_index(e.label), s.org.missing.contains(&e.path), e.leads_passes)))
            .collect()
    }

    // MARK: - The pointer

    pub(super) fn pointer_down(self: &Rc<Self>, x: f32, y: f32) {
        let mut s = self.state.borrow_mut();
        s.org.press = Some((x, y));
        s.org.reorder = None;
    }

    /// Only a project rearranges its photos by drag; the archive and the ONI Trash move nothing.
    pub(super) fn pointer_moved(self: &Rc<Self>, x: f32, y: f32) {
        {
            let mut s = self.state.borrow_mut();
            let Some((px, py)) = s.org.press else { return };
            if !s.org.projects_surface {
                return;
            }
            if s.org.reorder.is_none() {
                if (x - px).hypot(y - py) < DRAG_DISTANCE {
                    return;
                }
                let Some(start) = s.layout.index_at(px, py) else { return };
                let Some(lead) = s.order.get(start).cloned() else { return };
                let sources = if s.picks.selection.contains(&lead) { s.picks.selection.clone() } else { HashSet::from([lead.clone()]) };
                let width = s.layout.frame(start).map_or(0.0, |f| f.width);
                let size = (width, s.layout.tile.1);
                s.org.reorder = Some(Reorder { sources, lead, size, boundary: Some(start), at: (x, y) });
            }
            let boundary = s.layout.boundary(x, y);
            if let Some(r) = s.org.reorder.as_mut() {
                r.at = (x, y);
                if boundary.is_some() {
                    r.boundary = boundary;
                }
            }
        }
        self.publish_reorder();
    }

    pub(super) fn pointer_up(self: &Rc<Self>, x: f32, y: f32, command: bool, shift: bool) {
        let (press, reorder) = {
            let mut s = self.state.borrow_mut();
            (s.org.press.take(), s.org.reorder.take())
        };
        if let Some(r) = reorder {
            self.publish_reorder();
            return self.commit_reorder(r);
        }
        let Some((px, py)) = press else { return };
        // The document is as tall as its cells: a click under the last row reaches no gesture.
        if (x - px).hypot(y - py) >= DRAG_DISTANCE || y >= self.state.borrow().layout.height {
            return;
        }
        let index = self.state.borrow().layout.index_at(x, y);
        match index {
            Some(i) => self.click(i, command, shift),
            None => self.click_empty(),
        }
    }

    fn publish_reorder(&self) {
        let Some(w) = self.window() else { return };
        let lib = w.global::<Library>();
        let s = self.state.borrow();
        let Some(r) = &s.org.reorder else {
            lib.set_ghost_shown(false);
            lib.set_drop_shown(false);
            return;
        };
        let picture = s.fingerprints.get(&r.lead).and_then(|f| s.thumbs.get(f)).unwrap_or_default();
        lib.set_ghost(picture);
        lib.set_ghost_x(r.at.0);
        lib.set_ghost_y(r.at.1);
        lib.set_ghost_width(r.size.0);
        lib.set_ghost_height(r.size.1);
        lib.set_ghost_shown(true);
        match r.boundary.and_then(|b| s.layout.boundary_line(b, DROP_LINE)) {
            Some(line) => {
                lib.set_drop_x(line.x);
                lib.set_drop_y(line.y);
                lib.set_drop_width(line.width);
                lib.set_drop_height(line.height);
                lib.set_drop_shown(true);
            }
            None => lib.set_drop_shown(false),
        }
    }

    /// `commitReorder`: the boundary past the last cell reads back the last one.
    fn commit_reorder(self: &Rc<Self>, r: Reorder) {
        let moved = {
            let mut s = self.state.borrow_mut();
            let Some(boundary) = r.boundary else { return };
            if s.order.is_empty() {
                return;
            }
            let at = boundary.min(s.order.len() - 1);
            let Some(target) = s.entry(&s.order[at].clone()).cloned() else { return };
            let rows = s.rows.clone();
            project::reorder_drop(&mut s.org.projects, &rows, &r.sources, &target)
        };
        if moved {
            self.save_projects();
            self.lay_grid();
        }
    }

    /// `click(_:)` on a cell: the picks, then the second plain click within the system's interval opens.
    pub(super) fn click(self: &Rc<Self>, index: usize, command: bool, shift: bool) {
        let (outcome, trash) = {
            let mut s = self.state.borrow_mut();
            let keys = s.band_keys();
            let order = s.order.clone();
            let columns = s.layout.columns;
            let shown = Shown { order: &order, groups: &keys, columns };
            let interval = system::double_click().as_secs_f64();
            let trash = s.trash_shown();
            if !trash {
                s.org.project_ring = None;
            }
            (s.shown_picks_mut().click(index, &shown, command, shift, uptime(), interval), trash)
        };
        match outcome {
            // The ONI Trash laid as a folder is; a tile opens nothing.
            Outcome::Open(path) if !trash => self.open_in_editor(&path),
            _ => {}
        }
        self.publish_tiles();
        self.publish_organising();
        self.publish_bar();
    }

    /// `onClickEmpty`: the ground clears the selection and the ring; the ONI Trash's, its selection.
    fn click_empty(self: &Rc<Self>) {
        {
            let mut s = self.state.borrow_mut();
            if !s.trash_shown() {
                s.picks.anchor = None;
            }
            s.shown_picks_mut().clear();
            s.org.project_ring = None;
        }
        self.publish_tiles();
        self.publish_organising();
        self.publish_bar();
    }

    // MARK: - The keys

    /// `GalleryEngine.key(stroke:)`: the arrows, Return, Space and ⌘A, while the gallery shows.
    pub(super) fn grid_key(self: &Rc<Self>, text: &str, command: bool, shift: bool, bare: bool, repeat: bool) -> bool {
        let key = match text {
            "\u{F702}" if !command => GridKey::Left,
            "\u{F703}" if !command => GridKey::Right,
            "\u{F700}" if !command => GridKey::Up,
            "\u{F701}" if !command => GridKey::Down,
            "\r" | "\n" if !command => GridKey::Return,
            " " => GridKey::Space { repeat, modified: !bare },
            "a" | "A" if command && !shift => GridKey::SelectAll,
            _ => return false,
        };
        let (outcome, trash) = {
            let mut s = self.state.borrow_mut();
            let keys = s.band_keys();
            let order = s.order.clone();
            let shown = Shown { order: &order, groups: &keys, columns: s.layout.columns };
            let current = s.open.clone();
            let focus = current.as_deref().and_then(|c| keys.iter().find(|(_, p)| p.iter().any(|x| x == c))).map(|(k, _)| k.clone());
            let trash = s.trash_shown();
            let shift_held = shift && !matches!(key, GridKey::SelectAll);
            (s.shown_picks_mut().key(key, shift_held, &shown, current.as_deref(), focus.as_deref()), trash)
        };
        let Some(outcome) = outcome else { return false };
        match outcome {
            Outcome::Open(path) if !trash => self.open_in_editor(&path),
            Outcome::Reveal(at) => {
                let mut s = self.state.borrow_mut();
                if let Some(top) = s.layout.reveal(at, s.scroll, s.grid.1) {
                    s.scroll = top;
                }
            }
            _ => {}
        }
        self.publish_tiles();
        self.publish_organising();
        self.publish_bar();
        true
    }

    // MARK: - The cell menu

    /// The menu a right click opens on the cell under it; the ground opens none.
    pub(super) fn context(self: &Rc<Self>, x: f32, y: f32) -> bool {
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

    fn can_paste(&self) -> bool {
        self.state.borrow_mut().store.editor().is_some_and(|e| e.has_copied_settings())
    }

    fn cell_items(&self, s: &State, path: &str, scope: &HashSet<String>, edit: bool, can_paste: bool) -> Vec<Item> {
        let rolled: HashSet<String> = s.rows.iter().filter(|e| e.film.is_some()).map(|e| e.path.clone()).collect();
        let projects: Vec<(Id, String)> = s.org.projects.iter().map(|p| (p.id, p.name.clone())).collect();
        let member_of: Vec<(Id, String)> =
            project::member_projects(&s.org.projects, &s.rows, scope).into_iter().map(|p| (p.id, p.name.clone())).collect();
        let facts = CellFacts { missing: &s.org.missing, rolled: &rolled, can_paste, projects, member_of };
        let mut items = Vec::new();
        if edit && scope.len() == 1 {
            items.push(Item { line: menu_line("Edit", true, false, false), act: Act::Edit(path.to_owned()) });
        }
        for (i, line) in organise::cell_menu(scope, &s.order, &facts).into_iter().enumerate() {
            let opens = matches!(line.action, CellAction::AddTo | CellAction::RemoveFrom);
            let rule = line.rule || (edit && scope.len() == 1 && i == 0);
            items.push(Item { line: menu_line(&line.text, line.enabled, rule, opens), act: Act::Cell(line) });
        }
        items
    }

    fn open_menu(&self, items: Vec<Item>, path: Option<String>) -> bool {
        let Some(w) = self.window() else { return false };
        if items.is_empty() {
            return false;
        }
        let lines: Vec<MenuLine> = items.iter().map(|i| i.line.clone()).collect();
        {
            let mut s = self.state.borrow_mut();
            let projects: Vec<(Id, String)> = s.org.projects.iter().map(|p| (p.id, p.name.clone())).collect();
            // The scope every verb of a cell menu shares, read off its first line, for the submenus.
            let scope: HashSet<String> = items
                .iter()
                .find_map(|i| match &i.act {
                    Act::Cell(l) if l.action == CellAction::Paste => Some(l.scope.iter().cloned().collect()),
                    _ => None,
                })
                .unwrap_or_default();
            let member: Vec<(Id, String)> =
                project::member_projects(&s.org.projects, &s.rows, &scope).into_iter().map(|p| (p.id, p.name.clone())).collect();
            s.org.menu_lists = (projects, member);
            s.org.menu = items;
            s.org.menu_path = path;
        }
        w.global::<Library>().set_menu_lines(ModelRc::new(VecModel::from(lines)));
        true
    }

    /// A line of the open menu; answers whether the menu stays, which a submenu's opener does.
    pub(super) fn menu_chosen(self: &Rc<Self>, i: usize) -> bool {
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
            Act::Reveal(paths) => {
                if let Some(first) = paths.first() {
                    system::reveal(Path::new(first));
                }
            }
            Act::OpenTrash => self.open_trash(),
            Act::RestoreAll => self.ask_restore_all(),
        }
        self.publish_bar();
        false
    }

    /// `CellMenu`: each verb selects its scope first, anchored on the cell, then acts on it.
    fn cell_act(self: &Rc<Self>, line: CellLine) {
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
            CellAction::ResetToDefault => self.act("reset-to-default"),
            CellAction::Medal(m) => self.label_paths(&line.scope, m),
            CellAction::NewRoll => self.new_roll(),
            CellAction::Ungroup => {
                let paths = line.scope.clone();
                self.gesture(&paths, |store| store.set_film(&paths, None, "Ungroup"));
            }
            CellAction::NewProject => self.new_project(&scope),
            CellAction::AddToProject(id) => {
                let added = {
                    let mut s = self.state.borrow_mut();
                    let rows = s.rows.clone();
                    project::add_to(&mut s.org.projects, &rows, &scope, id)
                };
                if added {
                    self.save_projects();
                }
            }
            CellAction::RemoveFromProject(id) => {
                {
                    let mut s = self.state.borrow_mut();
                    let rows = s.rows.clone();
                    project::remove_from(&mut s.org.projects, &rows, &scope, id);
                }
                self.save_projects();
            }
            CellAction::Export => self.act("export"),
            CellAction::Reveal => {
                if let Some(first) = line.scope.first() {
                    system::reveal(Path::new(first));
                }
            }
            CellAction::Trash => self.move_paths_to_trash(&line.scope),
            CellAction::AddTo | CellAction::RemoveFrom => {}
        }
        self.lay_grid();
        self.publish_bar();
    }

    // MARK: - Projects

    pub(super) fn load_projects(&self) {
        let mut s = self.state.borrow_mut();
        s.org.projects = project::load(&s.support).projects;
    }

    /// `saveProjects`: every member's path written down, then the one file.
    fn save_projects(&self) {
        let failure = {
            let mut s = self.state.borrow_mut();
            let located = project::located(&s.org.projects, &s.rows);
            s.org.projects = located.clone();
            project::save(&s.support, &ProjectsFile { version: 1, projects: located }).err()
        };
        if let Some(e) = failure {
            self.say(&format!("The projects could not be saved: {e}"));
        }
    }

    fn new_project(self: &Rc<Self>, paths: &HashSet<String>) {
        let made = {
            let mut s = self.state.borrow_mut();
            let rows = s.rows.clone();
            project::new_from(&mut s.org.projects, &rows, paths, Date::now())
        };
        if made.is_some() {
            self.save_projects();
        }
    }

    /// `deleteProjectRequested`: real curation work asks before it goes.
    fn ask_delete_project(self: &Rc<Self>, id: Id) {
        let prompt = self.state.borrow().org.projects.iter().find(|p| p.id == id).map(project::delete_prompt);
        if let Some((title, message, key)) = prompt {
            self.ask(&title, &message, key, Pending::DeleteProject(id));
        }
    }

    pub(super) fn delete_project(self: &Rc<Self>, id: Id) {
        {
            let mut s = self.state.borrow_mut();
            project::delete(&mut s.org.projects, id);
            if matches!(s.org.project_ring, Some(NodeId::Project(r)) if r == id) {
                s.org.project_ring = None;
            }
        }
        self.save_projects();
        self.lay_grid();
    }

    pub(super) fn surface_picked(self: &Rc<Self>, i: i32) {
        {
            let mut s = self.state.borrow_mut();
            s.org.projects_surface = i == 1;
            s.org.reorder = None;
            s.org.info_loaded = None;
        }
        if let Some(w) = self.window() {
            w.global::<Library>().set_surface(i);
        }
        self.lay_grid();
        self.publish_bar();
    }

    fn publish_projects(&self) {
        let Some(w) = self.window() else { return };
        let rows: Vec<TreeRow> = {
            let mut s = self.state.borrow_mut();
            let nodes = project_tree::flatten(&project_tree::roots(&s.org.projects, &s.rows), &s.org.project_expanded, 0);
            s.org.project_nodes = nodes.iter().map(|(n, _)| n.id()).collect();
            nodes
                .iter()
                .map(|(node, depth)| {
                    let id = node.id();
                    let ringed = s.org.project_ring.as_ref() == Some(&id);
                    match node {
                        Node::Project(g) => TreeRow {
                            depth: *depth as i32,
                            title: g.project.name.clone().into(),
                            caption: g.entries.len().to_string().into(),
                            kind: 1,
                            expandable: !g.entries.is_empty(),
                            expanded: s.org.project_expanded.contains(&id),
                            selected: ringed,
                        },
                        Node::Photo { entry, .. } => TreeRow {
                            depth: *depth as i32,
                            title: stem(&entry.path).into(),
                            caption: library::medal(entry.label).into(),
                            kind: 2,
                            expandable: false,
                            expanded: false,
                            selected: ringed || s.picks.selection.contains(&entry.path),
                        },
                    }
                })
                .collect()
        };
        w.global::<Library>().set_project_rows(ModelRc::new(VecModel::from(rows)));
    }

    /// `selectProject`: a project row rings it and clears the photos, a photo row selects it; in
    /// the editor a photo row opens it.
    pub(super) fn project_row_clicked(self: &Rc<Self>, i: usize) {
        let gallery = self.window().is_some_and(|w| w.global::<App>().get_gallery());
        let open = {
            let mut s = self.state.borrow_mut();
            let Some(id) = s.org.project_nodes.get(i).cloned() else { return };
            let open = match &id {
                NodeId::Project(_) => {
                    if !s.org.project_expanded.remove(&id) {
                        s.org.project_expanded.insert(id.clone());
                    }
                    s.picks.selection.clear();
                    s.picks.cursor = None;
                    None
                }
                NodeId::Photo { path, .. } => {
                    s.picks.selection = HashSet::from([path.clone()]);
                    s.picks.cursor = Some(path.clone());
                    s.picks.anchor = Some(path.clone());
                    (!gallery).then(|| path.clone())
                }
            };
            s.org.project_ring = Some(id);
            open
        };
        if let Some(path) = open {
            self.open_photo(&path);
        }
        self.publish_tiles();
        self.publish_organising();
        self.publish_bar();
    }

    /// `takeOff` from the archive's tree: a photo row's double click opens it in the editor.
    pub(super) fn row_double_clicked(self: &Rc<Self>, i: usize) {
        let path = {
            let s = self.state.borrow();
            s.archive.as_ref().and_then(|a| a.rows().get(i).cloned()).and_then(|r| match r.id {
                oni_library::archive_tree::NodeId::Photo(p) => Some(p),
                _ => None,
            })
        };
        if let Some(path) = path {
            self.open_in_editor(&path);
        }
    }

    /// `takeOff`: a photo row's double click opens it in the editor.
    pub(super) fn project_row_double_clicked(self: &Rc<Self>, i: usize) {
        let path = match self.state.borrow().org.project_nodes.get(i) {
            Some(NodeId::Photo { path, .. }) => Some(path.clone()),
            _ => None,
        };
        if let Some(path) = path {
            self.open_in_editor(&path);
        }
    }

    /// `projectContextMenu`: Delete Project on a project, Edit then the cell menu on a photo.
    pub(super) fn project_row_menu(self: &Rc<Self>, i: usize) -> bool {
        let id = self.state.borrow().org.project_nodes.get(i).cloned();
        match id {
            Some(NodeId::Project(id)) => {
                let item = Item { line: menu_line("Delete Project", true, false, false), act: Act::DeleteProject(id) };
                self.open_menu(vec![item], None)
            }
            Some(NodeId::Photo { path, .. }) => {
                let can_paste = self.can_paste();
                let items = {
                    let s = self.state.borrow();
                    let scope = organise::cell_scope(&path, &s.picks.selection);
                    self.cell_items(&s, &path, &scope, true, can_paste)
                };
                self.open_menu(items, Some(path))
            }
            None => false,
        }
    }

    // MARK: - The ONI Trash

    /// The ONI Trash's photos under the opened folder, latest deletion first.
    pub(super) fn read_trashed(&self) {
        let mut s = self.state.borrow_mut();
        let (Some(index), Some(folder)) = (&s.index, s.opened.clone()) else { return };
        let all = index.all();
        let trashed: Vec<Entry> =
            oni_library::index::trashed(&all).into_iter().filter(|e| oni_library::paths::holds(&folder, &e.path)).collect();
        s.org.trashed = trashed;
    }

    fn publish_trash_rows(&self) {
        let Some(w) = self.window() else { return };
        let lines: Vec<TrashLine> = {
            let s = self.state.borrow();
            let count = s.org.trashed.len();
            let unfolded = count > 0 && s.org.trash_unfolded;
            let mut lines = vec![TrashLine {
                title: "ONI Trash".into(),
                trail: SharedString::new(),
                count: if count > 0 { count.to_string().into() } else { SharedString::new() },
                depth: 0,
                foldable: count > 0,
                unfolded,
                picked: s.org.trash_picked.as_deref() == Some(""),
                opened: s.org.trash_open,
                present: count > 0,
                restore_help: if count > 0 { "Restore every photo in the ONI Trash" } else { "" }.into(),
            }];
            if unfolded {
                let home = home();
                for folder in oni_library::trash::folders(&s.org.trashed) {
                    let heading = oni_library::trash::heading(&folder.path, &home);
                    let (trail, title) = match heading.rfind('/') {
                        Some(cut) => (format!("{}/", &heading[..cut]), heading[cut + 1..].to_string()),
                        None => (String::new(), heading.clone()),
                    };
                    lines.push(TrashLine {
                        title: title.into(),
                        trail: trail.into(),
                        count: folder.count.to_string().into(),
                        depth: 1,
                        foldable: false,
                        unfolded: false,
                        picked: s.org.trash_picked.as_deref() == Some(folder.path.as_str()),
                        opened: false,
                        present: Path::new(&folder.path).exists(),
                        restore_help: if folder.count == 1 { "Restore this photo".to_string() } else { format!("Restore these {} photos", folder.count) }
                            .into(),
                    });
                }
            }
            lines
        };
        w.global::<Library>().set_trash_rows(ModelRc::new(VecModel::from(lines)));
    }

    fn trash_folder(&self, i: usize) -> Option<(String, usize)> {
        let s = self.state.borrow();
        oni_library::trash::folders(&s.org.trashed).into_iter().nth(i.checked_sub(1)?).map(|f| (f.path, f.count))
    }

    /// `onClick(1)`: the trash row folds, a folder row is picked.
    pub(super) fn trash_row_clicked(self: &Rc<Self>, i: usize) {
        {
            let folder = self.trash_folder(i);
            let mut s = self.state.borrow_mut();
            match folder {
                None if i == 0 => {
                    s.org.trash_picked = Some(String::new());
                    if !s.org.trashed.is_empty() {
                        s.org.trash_unfolded = !s.org.trash_unfolded;
                    }
                }
                Some((path, _)) => s.org.trash_picked = Some(path),
                None => {}
            }
        }
        self.publish_trash_rows();
    }

    /// `onClick(2)`: the second click opens the ONI Trash; the fold the first click turned is put back.
    pub(super) fn trash_row_double_clicked(self: &Rc<Self>, i: usize) {
        if i == 0 {
            let mut s = self.state.borrow_mut();
            if !s.org.trashed.is_empty() {
                s.org.trash_unfolded = !s.org.trash_unfolded;
            }
        }
        self.open_trash();
    }

    pub(super) fn trash_row_restore(self: &Rc<Self>, i: usize) {
        if i == 0 {
            return self.ask_restore_all();
        }
        let Some((folder, _)) = self.trash_folder(i) else { return };
        let frames: Vec<String> = {
            let s = self.state.borrow();
            let mut f: Vec<String> = oni_library::trash::frames(&folder, &s.org.trashed).into_iter().collect();
            f.sort();
            f
        };
        self.restore(&frames);
    }

    pub(super) fn trash_row_menu(self: &Rc<Self>, i: usize) -> bool {
        let count = self.state.borrow().org.trashed.len();
        let mut items = vec![Item { line: menu_line("Open", true, false, false), act: Act::OpenTrash }];
        if i == 0 {
            if count > 0 {
                items.push(Item { line: menu_line("Restore All…", true, false, false), act: Act::RestoreAll });
            }
        } else if let Some((folder, n)) = self.trash_folder(i) {
            let frames: Vec<String> = {
                let s = self.state.borrow();
                oni_library::trash::frames(&folder, &s.org.trashed).into_iter().collect()
            };
            let word = if n == 1 { "Restore Photo".to_string() } else { format!("Restore {n} Photos") };
            items.push(Item { line: menu_line(&word, true, false, false), act: Act::Restore(frames) });
            let present = Path::new(&folder).exists();
            items.push(Item { line: menu_line("Show in Finder", present, true, false), act: Act::Reveal(vec![folder]) });
        }
        self.open_menu(items, None)
    }

    /// `openTrash`: the grid lays the ONI Trash, both selections cleared, in the gallery.
    pub(super) fn open_trash(self: &Rc<Self>) {
        {
            let mut s = self.state.borrow_mut();
            s.org.trash_open = true;
            s.org.projects_surface = false;
            s.trash_picks = Picks::default();
            s.picks.selection.clear();
        }
        if let Some(w) = self.window() {
            w.global::<App>().set_gallery(true);
            w.global::<Library>().set_surface(0);
        }
        self.lay_grid();
        self.publish_bar();
    }

    pub(super) fn leave_trash(&self) {
        let mut s = self.state.borrow_mut();
        s.org.trash_open = false;
        s.trash_picks = Picks::default();
    }

    /// `restoreFromOniTrash`: the marks cleared on the trashed among `paths`, one gallery step.
    pub(super) fn restore(self: &Rc<Self>, paths: &[String]) {
        let targets: Vec<String> = {
            let s = self.state.borrow();
            let trashed: HashSet<&str> = s.org.trashed.iter().map(|e| e.path.as_str()).collect();
            paths.iter().filter(|p| trashed.contains(p.as_str())).cloned().collect()
        };
        if targets.is_empty() {
            return;
        }
        self.gesture(&targets, |store| store.restore_from_trash(&targets));
        let gone: HashSet<String> = targets.into_iter().collect();
        self.state.borrow_mut().trash_picks.forget(&gone);
        self.lay_grid();
    }

    /// `askRestoreAll`: every photo in the ONI Trash, once a key confirms.
    fn ask_restore_all(self: &Rc<Self>) {
        let count = self.state.borrow().org.trashed.len();
        if count == 0 {
            return;
        }
        let (title, key) =
            if count > 1 { (format!("Restore {count} Photos?"), format!("Restore {count} Photos")) } else { ("Restore This Photo?".into(), "Restore Photo".into()) };
        self.ask(&title, "Every photo in the ONI Trash goes back to its folder, roll and project.", &key, Pending::RestoreAll);
    }

    pub(super) fn restore_all(self: &Rc<Self>) {
        let all: Vec<String> = self.state.borrow().org.trashed.iter().map(|e| e.path.clone()).collect();
        self.restore(&all);
    }

    /// `DSDialogRequest.confirm`: the sheet over the window until a key answers it.
    pub(super) fn ask(&self, title: &str, message: &str, key: &str, pending: Pending) {
        self.state.borrow_mut().pending = Some(pending);
        if let Some(w) = self.window() {
            let c = w.global::<Confirm>();
            c.set_title(title.into());
            c.set_message(message.into());
            c.set_key(key.to_uppercase().into());
            c.set_shown(true);
        }
    }

    // MARK: - The status bar

    fn publish_status(&self) {
        let Some(w) = self.window() else { return };
        let (leading, target) = {
            let s = self.state.borrow();
            let shown = s.archive.as_ref().map_or(0, |a| a.groups.iter().map(|g| g.entries.len()).sum());
            if s.trash_shown() {
                (picking::status_leading(Some((s.trash_picks.selection.len(), s.org.trashed.len())), None, 0, shown), None)
            } else {
                let path = picking::status_path(&s.picks.selection, false, s.open.as_deref());
                let status = path.as_deref().map(|p| picking::status_label(p, None));
                let leading = picking::status_leading(None, status, s.picks.selection.len(), shown);
                let label = |p: &str| s.entry(p).map_or(Medal::None, |e| e.label);
                (leading, picking::medal_target(path.as_deref(), &s.picks.selection, &label))
            }
        };
        let lib = w.global::<Library>();
        lib.set_status(leading.into());
        lib.set_medal_shown(target.is_some());
        lib.set_medal_current(target.and_then(|t| t.0).map_or(-1, library::medal_index));
    }

    /// `DSMedalPicker`'s tap: one photo toggles its own medal off, several all take it.
    pub(super) fn medal_picked(self: &Rc<Self>, medal: i32) {
        let target = {
            let s = self.state.borrow();
            let path = picking::status_path(&s.picks.selection, false, s.open.as_deref());
            let label = |p: &str| s.entry(p).map_or(Medal::None, |e| e.label);
            picking::medal_target(path.as_deref(), &s.picks.selection, &label)
        };
        let Some((current, paths)) = target else { return };
        let applied = picking::medal_tapped(current, paths.len(), library::medal_of(medal));
        self.label_paths(&paths, applied);
    }

    // MARK: - The info sidebar

    fn publish_info(&self) {
        let Some(w) = self.window() else { return };
        let lib = w.global::<Library>();
        let mut s = self.state.borrow_mut();
        if s.org.projects_surface {
            let Some(group) = s.project_group() else {
                lib.set_info_kind(0);
                lib.set_info_empty("Select a project".into());
                s.org.info_loaded = None;
                return;
            };
            lib.set_info_kind(2);
            lib.set_info_title(group.project.name.clone().into());
            let stocks: Vec<StockCount> = oni_library::info::stocks(&group.entries)
                .into_iter()
                .map(|(k, n)| StockCount { name: k.display_name.into(), count: n.to_string().into() })
                .collect();
            lib.set_stocks(ModelRc::new(VecModel::from(stocks)));
            let stack: Vec<Image> = group
                .entries
                .iter()
                .take(3)
                .map(|e| s.thumbs.get(&e.fingerprint).unwrap_or_default())
                .collect();
            lib.set_stack(ModelRc::new(VecModel::from(stack)));
            let key = format!("project {}", group.id);
            if s.org.info_loaded.as_deref() != Some(key.as_str()) {
                lib.set_description(group.project.description.clone().into());
                s.org.info_loaded = Some(key);
            }
            return;
        }
        let Some(roll) = s.roll().filter(|_| !s.trash_shown()) else {
            lib.set_info_kind(0);
            lib.set_info_empty("Select a roll".into());
            s.org.info_loaded = None;
            return;
        };
        let film = &roll.film;
        lib.set_info_kind(1);
        lib.set_info_title(film.name.clone().into());
        // `heroStock`: the stock a hand picked, else the first photo's mode's default, said so.
        let mode = roll.first.as_ref().and_then(|e| e.mode);
        let explicit = film_stock(film.stock.as_deref());
        let fallback = mode.and_then(|m| film_stock(Some(default_stock_id(m))));
        let (stock, note) = match (explicit, fallback, mode) {
            (Some(k), _, _) => (k.display_name.to_string(), String::new()),
            (None, Some(k), Some(m)) => (k.display_name.to_string(), format!("Default, from this roll's {} mode", m.label())),
            _ => (String::new(), "No stock set".to_string()),
        };
        lib.set_stock(stock.into());
        lib.set_stock_note(note.into());
        let default = if film.shot_date.is_none() { roll.first.as_ref().and_then(|e| e.captured_at) } else { None };
        lib.set_shot_date_set(film.shot_date.is_some());
        lib.set_shot_date_default(default.is_some());
        let key = format!("roll {} {:?} {:?} {:?}", film.id, film.shot_date, film.camera_body, default);
        if s.org.info_loaded.as_deref() != Some(key.as_str()) {
            let (y, m, d) = oni_library::info::ymd(film.shot_date.or(default).unwrap_or_else(Date::now));
            lib.set_month(m.to_string().into());
            lib.set_day(d.to_string().into());
            lib.set_year(y.to_string().into());
            lib.set_camera_body(film.camera_body.clone().unwrap_or_default().into());
            s.org.info_loaded = Some(key);
        }
    }

    /// `updateRoll`: a folder's roll is its marker; a roll its sidecars name is written on each
    /// member as one gallery step.
    fn update_roll(self: &Rc<Self>, label: &str, mutate: impl FnOnce(&mut Film)) {
        let Some(roll) = self.state.borrow().roll() else { return };
        let mut film = roll.film.clone();
        mutate(&mut film);
        if film != roll.film {
            match &roll.marker {
                Some(folder) => {
                    if let Err(e) = oni_library::folders::set_roll(Some(&film), folder) {
                        self.say(&format!("The roll could not be written in its folder: {e}"));
                    }
                    self.lay_archive();
                }
                None => {
                    let paths = roll.paths.clone();
                    let label = label.to_string();
                    self.gesture(&paths, |store| store.set_film(&paths, Some(film), &label));
                }
            }
        }
        self.state.borrow_mut().org.info_loaded = None;
        self.publish_info();
    }

    pub(super) fn title_committed(self: &Rc<Self>, draft: &str) {
        if self.state.borrow().org.projects_surface {
            let renamed = {
                let mut s = self.state.borrow_mut();
                let Some(group) = s.project_group() else { return };
                let Some(name) = picking::named(draft, &group.project.name) else { return };
                project::rename(&mut s.org.projects, group.id, &name)
            };
            if renamed {
                self.save_projects();
                self.lay_grid();
            }
            return;
        }
        let Some(roll) = self.state.borrow().roll() else { return };
        let Some(name) = picking::named(draft, &roll.film.name) else { return };
        self.update_roll("Rename Roll", |f| f.name = name);
    }

    /// `DateFields.commit`: month and year must read as numbers, the day falls back on the first.
    pub(super) fn shot_date_committed(self: &Rc<Self>, month: &str, day: &str, year: &str) {
        let made = match (month.trim().parse::<u32>(), year.trim().parse::<i32>()) {
            (Ok(m), Ok(y)) => oni_library::info::made(y, m, day.trim().parse().unwrap_or(1)),
            _ => None,
        };
        match made {
            Some(date) => self.update_roll("Set Shot Date", |f| f.shot_date = Some(date)),
            None => {
                self.state.borrow_mut().org.info_loaded = None;
                self.publish_info();
            }
        }
    }

    pub(super) fn shot_date_cleared(self: &Rc<Self>) {
        self.update_roll("Set Shot Date", |f| f.shot_date = None);
    }

    /// Written on Return or when the focus leaves: each keystroke would rewrite the whole roll.
    pub(super) fn camera_body_committed(self: &Rc<Self>, draft: &str) {
        let wanted = draft.trim().to_string();
        let Some(roll) = self.state.borrow().roll() else { return };
        if wanted == roll.film.camera_body.clone().unwrap_or_default() {
            return;
        }
        self.update_roll("Set Camera Body", |f| f.camera_body = (!wanted.is_empty()).then_some(wanted));
    }

    pub(super) fn description_committed(self: &Rc<Self>, text: &str) {
        let set = {
            let mut s = self.state.borrow_mut();
            let Some(group) = s.project_group() else { return };
            if group.project.description == text {
                return;
            }
            project::set_description(&mut s.org.projects, group.id, text)
        };
        if set {
            self.save_projects();
        }
    }
}

impl Ctl {
    pub(super) fn wire_organising(me: &Rc<Ctl>, window: &crate::MainWindow) {
        me.load_projects();
        let lib = window.global::<Library>();
        let index = |i: i32| i.max(0) as usize;
        let p = me.clone();
        lib.on_surface_picked(move |i| p.surface_picked(i));
        let p = me.clone();
        lib.on_row_double_clicked(move |i| p.row_double_clicked(index(i)));
        let p = me.clone();
        lib.on_project_row_clicked(move |i| p.project_row_clicked(index(i)));
        let p = me.clone();
        lib.on_project_row_double_clicked(move |i| p.project_row_double_clicked(index(i)));
        let p = me.clone();
        lib.on_project_row_menu(move |i| p.project_row_menu(index(i)));
        let p = me.clone();
        lib.on_trash_row_clicked(move |i| p.trash_row_clicked(index(i)));
        let p = me.clone();
        lib.on_trash_row_double_clicked(move |i| p.trash_row_double_clicked(index(i)));
        let p = me.clone();
        lib.on_trash_row_restore(move |i| p.trash_row_restore(index(i)));
        let p = me.clone();
        lib.on_trash_row_menu(move |i| p.trash_row_menu(index(i)));
        let p = me.clone();
        lib.on_pointer_down(move |x, y, _, _| p.pointer_down(x, y));
        let p = me.clone();
        lib.on_pointer_moved(move |x, y| p.pointer_moved(x, y));
        let p = me.clone();
        lib.on_pointer_up(move |x, y, command, shift| p.pointer_up(x, y, command, shift));
        let p = me.clone();
        lib.on_context(move |x, y| p.context(x, y));
        let p = me.clone();
        lib.on_menu_chosen(move |i| p.menu_chosen(index(i)));
        let p = me.clone();
        lib.on_empty_acted(move || p.act("open"));
        let p = me.clone();
        lib.on_medal_picked(move |m| p.medal_picked(m));
        let p = me.clone();
        lib.on_title_committed(move |t| p.title_committed(&t));
        let p = me.clone();
        lib.on_select_roll_type(move || p.act("select-roll-type"));
        let p = me.clone();
        lib.on_shot_date_committed(move |m, d, y| p.shot_date_committed(&m, &d, &y));
        let p = me.clone();
        lib.on_shot_date_cleared(move || p.shot_date_cleared());
        let p = me.clone();
        lib.on_camera_body_committed(move |t| p.camera_body_committed(&t));
        let p = me.clone();
        lib.on_description_committed(move |t| p.description_committed(&t));
    }
}
