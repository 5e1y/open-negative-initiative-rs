//! The library column's two trees as `TreeEngine` lays them (`TreeEngine.swift:93-98,332-380`): a
//! photo as a name or a thumbnail by `SidebarLayout`, the open row held under the top while its children pass.

use super::*;
use super::organising::reach;
use crate::TreePin;
use crate::sidebar::{self, Frame, Layout, Metrics};

/// `sidebarLayout.editor` and `sidebarLayout.gallery` (`OpenNegativeApp.swift:253-254`).
const EDITOR_LAYOUT: &str = "sidebarLayout.editor";
const GALLERY_LAYOUT: &str = "sidebarLayout.gallery";

/// The archive's tree, beside a photo only, or the projects' tree.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Which {
    Archive,
    Projects,
}

/// A tree as laid, which a pick, the pinned row and a drop read.
#[derive(Default)]
pub(super) struct Laid {
    /// Each row's photo; none for a folder, a roll or a project.
    pub paths: Vec<Option<String>>,
    /// The column's size in points, each row's place in it and its whole height.
    pub size: (f32, f32),
    pub frames: Vec<Frame>,
    pub total: f32,
    /// Whether each row holds its thumbnail; none for a name. A held one changes only when it is
    /// drawn again (`fill_tree_thumbnails`), so no picture is compared pixel by pixel.
    pub pictures: Vec<Option<bool>>,
    /// Each row's depth, whether it is open, whether it is drawn as a thumbnail.
    pub depths: Vec<usize>,
    pub open: Vec<bool>,
    pub photos: Vec<bool>,
    /// Whether each row is lit as the model holds it, so a pick relights the rows it changed alone.
    pub marked: Vec<bool>,
}

impl State {
    pub(super) fn laid(&self, which: Which) -> &Laid {
        match which {
            Which::Archive => &self.tree,
            Which::Projects => &self.project_tree,
        }
    }

    fn laid_mut(&mut self, which: Which) -> &mut Laid {
        match which {
            Which::Archive => &mut self.tree,
            Which::Projects => &mut self.project_tree,
        }
    }
}

impl Ctl {
    pub(super) fn sidebar_metrics(w: &MainWindow) -> Metrics {
        let ds = w.global::<crate::DS>();
        Metrics {
            row: ds.get_control_film_sidebar_row_height(),
            small: ds.get_space_sm(),
            medium: ds.get_space_md(),
            tiny: ds.get_space_xxs(),
            caption: ds.get_control_caption_height(),
        }
    }

    fn in_gallery(&self) -> bool {
        self.window().is_none_or(|w| w.global::<App>().get_gallery())
    }

    /// `sidebarLayout`: beside a photo the editor's, two thumbnails a line until one is picked; the
    /// projects in the gallery the gallery's, names until one is picked.
    pub(super) fn column_layout(&self, which: Which) -> Layout {
        let gallery = which == Which::Projects && self.in_gallery();
        let (key, default) = if gallery { (GALLERY_LAYOUT, Layout::List) } else { (EDITOR_LAYOUT, Layout::Pairs) };
        self.state.borrow().preferences.string(key).and_then(Layout::of_key).unwrap_or(default)
    }

    /// The tree the column shows, whose layout the key wears.
    fn shown_tree(&self) -> Which {
        if self.state.borrow().org.projects_surface { Which::Projects } else { Which::Archive }
    }

    /// `SidebarKeys`' layout key: the next layout remembered for the mode on screen.
    pub(super) fn next_layout(&self) {
        let which = self.shown_tree();
        let key = if which == Which::Projects && self.in_gallery() { GALLERY_LAYOUT } else { EDITOR_LAYOUT };
        self.remember(key, serde_json::Value::from(self.column_layout(which).next().key()));
    }

    /// The key wears the layout of the tree shown, and names the next.
    pub(super) fn publish_layout_key(&self) {
        let Some(w) = self.window() else { return };
        let layout = self.column_layout(self.shown_tree());
        let lib = w.global::<Library>();
        lib.set_tree_layout(Layout::ALL.iter().position(|l| *l == layout).unwrap_or(0) as i32);
        lib.set_tree_layout_help(tr!("{layout} — next: {next}", layout = layout.name(), next = layout.next().name()).into());
    }

    /// Lays `rows`, whose photos are `paths`, in the tree's column, and asks the thumbnails they lack.
    pub(super) fn lay_column(&self, which: Which, rows: &mut [TreeRow], paths: Vec<Option<String>>) {
        let Some(w) = self.window() else { return };
        let metrics = Self::sidebar_metrics(&w);
        let columns = self.column_layout(which).columns();
        let near = self.near_view(which);
        let mut guard = self.state.borrow_mut();
        let s = &mut *guard;
        let photos: Vec<bool> = rows.iter().map(|r| columns > 0 && r.kind == 2).collect();
        let depths: Vec<usize> = rows.iter().map(|r| r.depth.max(0) as usize).collect();
        let (frames, total) = sidebar::frames(&photos, &depths, s.laid(which).size.0, columns, metrics);
        let flags = Ctl::tile_flags(s);
        let mut asking = Vec::new();
        for (i, (row, frame)) in rows.iter_mut().zip(&frames).enumerate() {
            (row.x, row.y, row.width, row.height) = (frame.x, frame.y, frame.width, frame.height);
            row.thumb = photos[i];
            let Some(path) = paths.get(i).cloned().flatten().filter(|_| row.thumb) else { continue };
            // `TreeLayers.caption`: a thumbnail is captioned by its stem, a name row by the whole name.
            row.title = stem(&path).into();
            let fingerprint = s.fingerprints.get(&path).cloned();
            if let Some(f) = flags.get(&path) {
                (row.medal, row.missing, row.rgb) = *f;
            }
            (row.away, row.note, row.gone_pass) = s.tile_marks(&path);
            let missing = row.missing;
            row.picture = fingerprint.as_ref().filter(|_| near(frame)).and_then(|f| Some(reach::shown(&s.org.seen, f, s.thumbs.get(f)?, missing))).unwrap_or_default();
            if let Some(f) = fingerprint.filter(|f| !f.starts_with(thumbnails::PENDING) && !s.thumbs.contains(f) && !s.asked.contains(f)) {
                asking.push(f);
            }
        }
        for fingerprint in asking {
            s.asked.insert(fingerprint.clone());
            let _ = s.disk.send(fingerprint);
        }
        let laid = s.laid_mut(which);
        laid.pictures = rows.iter().map(|r| r.thumb.then(|| r.picture.size().width > 0)).collect();
        laid.open = rows.iter().map(|r| r.expandable && r.expanded).collect();
        laid.marked = rows.iter().map(|r| r.selected).collect();
        (laid.paths, laid.frames, laid.total, laid.depths, laid.photos) = (paths, frames, total, depths, photos);
        drop(guard);
        let lib = w.global::<Library>();
        match which {
            Which::Archive => lib.set_tree_content_height(total),
            Which::Projects => lib.set_project_content_height(total),
        }
        self.publish_layout_key();
    }

    pub(super) fn column_model(&self, which: Which) -> Option<ModelRc<TreeRow>> {
        let w = self.window()?;
        let lib = w.global::<Library>();
        Some(match which {
            Which::Archive => lib.get_rows(),
            Which::Projects => lib.get_project_rows(),
        })
    }

    /// Whether a row lies within a screen of the column's view, the rows that keep their pictures.
    fn near_view(&self, which: Which) -> impl Fn(&Frame) -> bool + use<> {
        let top = self.column_top(which);
        // A column not laid out yet has no view: every row is near until it has one.
        let height = self.state.borrow().laid(which).size.1;
        move |f: &Frame| height <= 0.0 || (f.y + f.height > top - height && f.y < top + 2.0 * height)
    }

    /// How far the tree has scrolled, in points from its top.
    fn column_top(&self, which: Which) -> f32 {
        let Some(w) = self.window() else { return 0.0 };
        let lib = w.global::<Library>();
        -match which {
            Which::Archive => lib.get_tree_viewport_y(),
            Which::Projects => lib.get_project_viewport_y(),
        }
    }

    fn scroll_column(&self, which: Which, top: f32) {
        let Some(w) = self.window() else { return };
        let lib = w.global::<Library>();
        match which {
            Which::Archive => lib.set_tree_viewport_y(-top),
            Which::Projects => lib.set_project_viewport_y(-top),
        }
    }

    /// A thumbnail landed or drawn again after the rows were laid reaches its row in place; only
    /// the rows whose picture changed are read from the model.
    pub(super) fn fill_tree_thumbnails(&self, redrawn: Option<&str>) {
        for which in [Which::Archive, Which::Projects] {
            let Some(model) = self.column_model(which) else { return };
            let near = self.near_view(which);
            let changed: Vec<(usize, Image, String)> = {
                let mut guard = self.state.borrow_mut();
                let s = &mut *guard;
                let (fingerprints, thumbs) = (&s.fingerprints, &s.thumbs);
                let laid = match which {
                    Which::Archive => &mut s.tree,
                    Which::Projects => &mut s.project_tree,
                };
                let mut changed = Vec::new();
                for (i, (path, held)) in laid.paths.iter().zip(laid.pictures.iter_mut()).enumerate() {
                    let Some(held) = held else { continue };
                    // A row scrolled far away lets its picture go, and with it the texture Slint kept.
                    if !laid.frames.get(i).is_some_and(&near) {
                        if *held {
                            *held = false;
                            changed.push((i, Image::default(), String::new()));
                        }
                        continue;
                    }
                    let Some(fingerprint) = path.as_ref().and_then(|p| fingerprints.get(p)) else { continue };
                    if *held && redrawn != Some(fingerprint.as_str()) {
                        continue;
                    }
                    let Some(picture) = thumbs.get(fingerprint) else { continue };
                    *held = true;
                    changed.push((i, picture, fingerprint.clone()));
                }
                changed
            };
            for (i, picture, fingerprint) in changed {
                if let Some(mut row) = model.row_data(i) {
                    row.picture = reach::shown(&self.state.borrow().org.seen, &fingerprint, picture, row.missing);
                    model.set_row_data(i, row);
                }
            }
        }
    }

    /// The rows laid again at the column's width in place, so no row is rebuilt and the scroll stays.
    pub(super) fn relay_column(&self, which: Which) {
        let Some(model) = self.column_model(which) else { return };
        let paths = self.state.borrow().laid(which).paths.clone();
        if model.row_count() != paths.len() {
            return self.publish_column(which);
        }
        let old: Vec<TreeRow> = model.iter().collect();
        let mut rows = old.clone();
        self.lay_column(which, &mut rows, paths);
        // Pictures compared by whether they are held, not pixel by pixel: a redraw is relayed apart.
        let bare = |r: &TreeRow| TreeRow { picture: Image::default(), ..r.clone() };
        for (i, (row, was)) in rows.into_iter().zip(&old).enumerate() {
            if bare(&row) != bare(was) || (row.picture.size().width > 0) != (was.picture.size().width > 0) {
                model.set_row_data(i, row);
            }
        }
        self.pin_column(which);
    }

    pub(super) fn publish_column(&self, which: Which) {
        match which {
            Which::Archive => self.publish_tree(),
            Which::Projects => self.publish_projects(),
        }
    }

    /// The scroll kept within the content, which a taller column or a narrower layout shortens.
    pub(super) fn clamp_column(&self, which: Which) {
        let (height, total) = {
            let s = self.state.borrow();
            (s.laid(which).size.1, s.laid(which).total)
        };
        let top = self.column_top(which).clamp(0.0, (total - height).max(0.0));
        self.scroll_column(which, top);
    }

    /// `TreeEngine.reveal(_:)`: the row's middle at the column's, held inside the content.
    pub(super) fn centre_row(&self, which: Which, i: usize) {
        let top = {
            let s = self.state.borrow();
            let laid = s.laid(which);
            let Some(frame) = laid.frames.get(i).copied() else { return };
            sidebar::centred(frame, laid.size.1, laid.total)
        };
        self.scroll_column(which, top);
    }

    /// `PinnedRow`: the open row whose children pass under the top, held there, or none.
    pub(super) fn pin_column(&self, which: Which) {
        let (Some(w), Some(model)) = (self.window(), self.column_model(which)) else { return };
        let top = self.column_top(which);
        let found = {
            let s = self.state.borrow();
            let l = s.laid(which);
            let row = Self::sidebar_metrics(&w).row;
            (model.row_count() == l.frames.len()).then(|| sidebar::pinned(top, &l.frames, l.total, &l.depths, &l.open, row)).flatten()
        };
        let pin = found
            .and_then(|(i, push)| model.row_data(i).map(|row| TreePin { shown: true, index: i as i32, push, row }))
            .unwrap_or_default();
        let lib = w.global::<Library>();
        match which {
            Which::Archive if lib.get_tree_pin() != pin => lib.set_tree_pin(pin),
            Which::Projects if lib.get_project_pin() != pin => lib.set_project_pin(pin),
            _ => {}
        }
    }

    /// `PinnedRow.onTap` off the chevron: the row clicked, then revealed in the middle.
    fn pin_clicked(self: &Rc<Self>, which: Which, command: bool, shift: bool) {
        let Some(w) = self.window() else { return };
        let lib = w.global::<Library>();
        let i = match which {
            Which::Archive => lib.get_tree_pin().index,
            Which::Projects => lib.get_project_pin().index,
        }
        .max(0) as usize;
        match which {
            Which::Archive => {
                let id = self.state.borrow().org.tree_ids.get(i).cloned();
                self.row_clicked(i, command, shift);
                let at = self.state.borrow().org.tree_ids.iter().position(|n| Some(n) == id.as_ref());
                if let Some(at) = at {
                    self.centre_row(which, at);
                }
            }
            Which::Projects => {
                let id = self.state.borrow().org.project_nodes.get(i).cloned();
                self.project_row_clicked(i, command, shift);
                let at = self.state.borrow().org.project_nodes.iter().position(|n| Some(n) == id.as_ref());
                if let Some(at) = at {
                    self.centre_row(which, at);
                }
            }
        }
    }

    /// The projects' tree made: laid at its width, then `.task(id: revealed)` centres the ring.
    fn project_shown(&self, width: f32, height: f32) {
        let was = std::mem::replace(&mut self.state.borrow_mut().project_tree.size, (width, height));
        if was.0 != width {
            self.relay_column(Which::Projects);
        }
        let ring = {
            let s = self.state.borrow();
            s.org.project_ring.as_ref().and_then(|r| s.org.project_nodes.iter().position(|n| n == r))
        };
        match ring {
            Some(i) => self.centre_row(Which::Projects, i),
            None => self.pin_column(Which::Projects),
        }
    }

    fn project_resized(&self, width: f32, height: f32) {
        let was = std::mem::replace(&mut self.state.borrow_mut().project_tree.size, (width, height));
        if was.0 != width {
            self.relay_column(Which::Projects);
        }
        self.clamp_column(Which::Projects);
        self.pin_column(Which::Projects);
    }

    pub(super) fn wire_tree_column(self: &Rc<Self>, lib: &Library) {
        let me = Rc::downgrade(self);
        lib.on_tree_scrolled(move || {
            if let Some(c) = me.upgrade() {
                c.pin_column(Which::Archive);
                c.fill_tree_thumbnails(None);
            }
        });
        let me = Rc::downgrade(self);
        lib.on_project_scrolled(move || {
            if let Some(c) = me.upgrade() {
                c.pin_column(Which::Projects);
                c.fill_tree_thumbnails(None);
            }
        });
        let me = Rc::downgrade(self);
        lib.on_tree_pin_clicked(move |command, shift| {
            if let Some(c) = me.upgrade() {
                c.pin_clicked(Which::Archive, command, shift);
            }
        });
        let me = Rc::downgrade(self);
        lib.on_project_pin_clicked(move |command, shift| {
            if let Some(c) = me.upgrade() {
                c.pin_clicked(Which::Projects, command, shift);
            }
        });
        let me = Rc::downgrade(self);
        lib.on_project_shown(move |width, height| {
            if let Some(c) = me.upgrade() {
                c.project_shown(width, height);
            }
        });
        let me = Rc::downgrade(self);
        lib.on_project_resized(move |width, height| {
            if let Some(c) = me.upgrade() {
                c.project_resized(width, height);
            }
        });
        // A mode changed: the projects' tree takes the other mode's layout, and the key wears it.
        let me = Rc::downgrade(self);
        lib.on_tree_moded(move || {
            if let Some(c) = me.upgrade() {
                c.publish_projects();
                c.publish_layout_key();
            }
        });
    }
}
