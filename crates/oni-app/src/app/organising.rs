//! The gallery's organising in the window (`GalleryEngine`, `CellMenu`, `InfoSidebar`): picks, the
//! cell menu, Archive and Projects, the ONI Trash laid as a grid, the medals and the info sidebar.

use super::{Ctl, Pending, State};
use crate::organise::{self, CellAction, CellFacts, CellLine, GridKey, Outcome, Picks, Shown};
use crate::{App, Confirm, Library, MenuLine, StockCount, TrashLine, TreeRow};
use crate::{library, picking, system};
use crate::tree_pick::TreePick;
use oni_library::Entry;
use oni_library::project::{self, Project, ProjectsFile};
use oni_library::project_tree::{self, Node, NodeId};
use oni_settings::film::{default_stock_id, film_stock};
use oni_settings::{Date, Film, Id, Medal};
use oni_i18n::{tr, trn};
use slint::{ComponentHandle, Image, ModelRc, SharedString, VecModel};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::Instant;

mod grid;
mod pointer;
mod grid_keys;
mod cell_menu;
mod projects;
mod oni_trash;
mod status_bar;
mod info_sidebar;
mod band_rename;
mod tree_menu;

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
    /// What renaming the heading renames, `None` where a heading takes no name.
    pub naming: Option<Naming>,
    /// `explicitName`: the name the field opens on, empty for a folder no roll names.
    pub explicit: String,
}

/// `GalleryGroup.id` as a rename reaches it: a folder on the disk, or a project.
#[derive(Clone, Debug, PartialEq)]
pub enum Naming {
    Folder(String),
    Project(Id),
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
    RenameFolder(String),
    Roll(String, bool),
    NewFolder(String),
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

/// The lists of the library column a click lands on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Column {
    Archive,
    Projects,
    Trash,
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
    /// The library column's last row clicked, its list and when, which tells a double click's second.
    row_click: Option<(Column, usize, Instant)>,
    /// What the sidebar's fields were last loaded from, so a refresh never overwrites typing.
    info_loaded: Option<String>,
    /// The surface last laid: 0 the archive, 1 the projects, 2 the ONI Trash.
    laid_surface: Option<u8>,
    /// `renaming` and `draft`: the heading typed over, and what its field holds now.
    pub(in crate::app) renaming: Option<(Naming, String)>,
    /// The heading last clicked and when, which tells a double click's second.
    heading_click: Option<(usize, Instant)>,
    /// `TreeEngine`'s own picks on the archive and on the projects, apart from the ring.
    pub(in crate::app) tree_pick: TreePick<oni_library::archive_tree::NodeId>,
    pub(in crate::app) project_pick: TreePick<NodeId>,
    /// The archive tree's rows as published, which a pick and its menu read.
    pub(in crate::app) tree_ids: Vec<oni_library::archive_tree::NodeId>,
    /// What the row typed over holds now, and the project typed over in the projects' tree.
    pub(in crate::app) tree_draft: Option<String>,
    pub(in crate::app) project_renaming: Option<Id>,
    /// A drag in the projects' tree under way.
    pub(in crate::app) project_drag: Option<super::project_drop::Drag>,
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

    /// Slint sends a double click as two clicks then `double-clicked`: the second click, on the row
    /// the first hit within the interval, is the double click's own and does nothing by itself.
    pub(super) fn second_click(&mut self, column: Column, row: usize) -> bool {
        let now = Instant::now();
        let second = self.org.row_click.is_some_and(|(c, r, at)| (c, r) == (column, row) && now - at < system::double_click());
        self.org.row_click = if second { None } else { Some((column, row, now)) };
        second
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
                .map(|g| Band {
                    key: g.id.to_string(),
                    title: g.project.name.clone(),
                    caption: String::new(),
                    entries: g.entries,
                    naming: Some(Naming::Project(g.id)),
                    explicit: g.project.name.clone(),
                })
                .collect();
        }
        if self.org.trash_open {
            let home = home();
            return oni_library::trash::groups(&self.org.trashed, &home)
                .into_iter()
                .map(|g| Band {
                    key: g.heading.clone().unwrap_or_default(),
                    title: g.heading_title(),
                    caption: g.trail().unwrap_or_default(),
                    entries: g.entries,
                    naming: None,
                    explicit: String::new(),
                })
                .collect();
        }
        self.archive
            .as_ref()
            .map(|a| {
                a.groups
                    .iter()
                    .enumerate()
                    .map(|(i, g)| Band {
                        key: format!("{i}:{}", g.title),
                        title: g.header.clone(),
                        caption: g.caption.clone(),
                        entries: g.entries.clone(),
                        naming: g.folder.clone().map(Naming::Folder),
                        explicit: g.film.as_ref().map(|f| f.name.clone()).unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The roll a click or the anchor stands on: the folder's marker, else the roll every
    /// member's sidecar names; a folder of loose photos is no roll.
    /// The stock the info sidebar's roll carries, picked by a hand or none.
    pub(super) fn roll_stock(&self) -> Option<String> {
        self.roll().and_then(|r| r.film.stock)
    }

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
    pub(super) fn wire_organising(me: &Rc<Ctl>, window: &crate::MainWindow) {
        me.load_projects();
        let lib = window.global::<Library>();
        let index = |i: i32| i.max(0) as usize;
        let p = me.clone();
        lib.on_surface_picked(move |i| p.surface_picked(i));
        let p = me.clone();
        lib.on_gallery_shown(move || p.land_gallery());
        Ctl::wire_band_rename(me, &lib);
        let p = me.clone();
        lib.on_row_double_clicked(move |i| p.row_double_clicked(index(i)));
        let p = me.clone();
        lib.on_row_folded(move |i| p.row_folded(index(i)));
        let p = me.clone();
        lib.on_project_row_folded(move |i| p.project_row_folded(index(i)));
        let p = me.clone();
        lib.on_project_row_clicked(move |i, command, shift| p.project_row_clicked(index(i), command, shift));
        let p = me.clone();
        lib.on_row_menu(move |i| p.row_menu(index(i)));
        let p = me.clone();
        lib.on_tree_empty_clicked(move || p.tree_empty_clicked());
        let p = me.clone();
        lib.on_project_row_double_clicked(move |i| p.project_row_double_clicked(index(i)));
        let p = me.clone();
        lib.on_project_row_menu(move |i| p.project_row_menu(index(i)));
        Ctl::wire_project_drop(me, &lib);
        let p = me.clone();
        lib.on_project_fold_all(move || p.project_fold_all());
        let p = me.clone();
        lib.on_project_centre(move || p.project_centre_key());
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
        lib.on_empty_acted(move || p.act(crate::Act::Open));
        let p = me.clone();
        lib.on_medal_picked(move |m| p.medal_picked(m));
        let p = me.clone();
        lib.on_title_committed(move |t| p.title_committed(&t));
        let p = me.clone();
        lib.on_select_roll_type(move || p.act(crate::Act::SelectRollType));
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
