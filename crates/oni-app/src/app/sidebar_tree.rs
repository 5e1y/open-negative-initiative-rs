//! The library column beside a photo: its rows laid as names or thumbnails, `SidebarLayout`'s key,
//! and the scroll that brings the selection back to its middle.

use super::*;
use crate::sidebar::{self, Layout, Metrics};

/// `sidebarLayout.editor`: the layout beside a photo, two thumbnails a line until one is picked.
const LAYOUT_KEY: &str = "sidebarLayout.editor";

impl State {
    pub(super) fn tree_layout(&self) -> Layout {
        self.preferences.string(LAYOUT_KEY).and_then(Layout::of_key).unwrap_or(Layout::Pairs)
    }
}

impl Ctl {
    fn sidebar_metrics(w: &MainWindow) -> Metrics {
        let ds = w.global::<crate::DS>();
        Metrics {
            row: ds.get_control_film_sidebar_row_height(),
            small: ds.get_space_sm(),
            medium: ds.get_space_md(),
            tiny: ds.get_space_xxs(),
            caption: ds.get_control_caption_height(),
        }
    }

    /// Lays `rows`, whose photos are `s.tree_paths`, in the column, and asks the thumbnails they lack.
    pub(super) fn lay_tree(&self, rows: &mut [TreeRow]) {
        let Some(w) = self.window() else { return };
        let metrics = Self::sidebar_metrics(&w);
        let mut s = self.state.borrow_mut();
        let layout = s.tree_layout();
        let photo: Vec<bool> = rows.iter().map(|r| r.kind == 2).collect();
        let depth: Vec<usize> = rows.iter().map(|r| r.depth.max(0) as usize).collect();
        let (frames, total) = sidebar::frames(&photo, &depth, s.tree_size.0, layout.columns(), metrics);
        let flags = Ctl::tile_flags(&s);
        let mut asking = Vec::new();
        for (i, (row, frame)) in rows.iter_mut().zip(&frames).enumerate() {
            (row.x, row.y, row.width, row.height) = (frame.x, frame.y, frame.width, frame.height);
            let Some(path) = s.tree_paths.get(i).cloned().flatten() else { continue };
            row.thumb = layout.columns() > 0 && row.kind == 2;
            if !row.thumb {
                continue;
            }
            let fingerprint = s.fingerprints.get(&path).cloned();
            row.picture = fingerprint.as_ref().and_then(|f| s.thumbs.get(f)).unwrap_or_default();
            (row.medal, row.missing) = flags.get(&path).map_or((0, false), |f| (f.0, f.1));
            if let Some(f) = fingerprint.filter(|f| !f.starts_with(thumbnails::PENDING) && !s.thumbs.contains(f) && !s.asked.contains(f)) {
                asking.push(f);
            }
        }
        for fingerprint in asking {
            s.asked.insert(fingerprint.clone());
            let _ = s.disk.send(fingerprint);
        }
        s.tree_pictures = rows.iter().map(|r| r.thumb.then(|| r.picture.clone())).collect();
        s.tree_frames = frames;
        s.tree_total = total;
        let lib = w.global::<Library>();
        lib.set_tree_content_height(total);
        lib.set_tree_layout(Layout::ALL.iter().position(|l| *l == layout).unwrap_or(0) as i32);
        lib.set_tree_layout_help(format!("{} — next: {}", layout.name(), layout.next().name()).into());
    }

    /// A thumbnail landed or drawn again after the rows were laid reaches its row in place; only
    /// the rows whose picture changed are read from the model.
    pub(super) fn fill_tree_thumbnails(&self) {
        let Some(w) = self.window() else { return };
        let changed: Vec<(usize, Image)> = {
            let mut guard = self.state.borrow_mut();
            let s = &mut *guard;
            let mut changed = Vec::new();
            for (i, (path, shown)) in s.tree_paths.iter().zip(s.tree_pictures.iter_mut()).enumerate() {
                let Some(shown) = shown else { continue };
                let Some(picture) = path.as_ref().and_then(|p| s.fingerprints.get(p)).and_then(|f| s.thumbs.get(f)) else { continue };
                if *shown != picture {
                    *shown = picture.clone();
                    changed.push((i, picture));
                }
            }
            changed
        };
        let model = w.global::<Library>().get_rows();
        for (i, picture) in changed {
            if let Some(mut row) = model.row_data(i) {
                row.picture = picture;
                model.set_row_data(i, row);
            }
        }
    }

    /// The rows laid again at the column's width in place, so no row is rebuilt and the scroll stays.
    fn relay_tree(&self) {
        let Some(w) = self.window() else { return };
        let model = w.global::<Library>().get_rows();
        if model.row_count() != self.state.borrow().tree_paths.len() {
            return self.publish_tree();
        }
        let old: Vec<TreeRow> = model.iter().collect();
        let mut rows = old.clone();
        self.lay_tree(&mut rows);
        for (i, (row, was)) in rows.into_iter().zip(&old).enumerate() {
            if row != *was {
                model.set_row_data(i, row);
            }
        }
    }

    /// The scroll kept within the content, which a taller column or a narrower layout shortens.
    fn clamp_tree(&self) {
        let Some(w) = self.window() else { return };
        let lib = w.global::<Library>();
        let (height, total) = {
            let s = self.state.borrow();
            (s.tree_size.1, s.tree_total)
        };
        let offset = (-lib.get_tree_viewport_y()).clamp(0.0, (total - height).max(0.0));
        lib.set_tree_viewport_y(-offset);
    }

    /// The photo the column follows: the open one, else the first picked.
    pub(super) fn tree_target(&self) -> Option<String> {
        let s = self.state.borrow();
        s.open.clone().or_else(|| s.picks.anchor.clone())
    }

    /// `TreeEngine.reveal`: the column centred on the photo it follows.
    pub(super) fn centre_tree(&self) {
        let Some(w) = self.window() else { return };
        if self.unfold_archive() {
            self.publish_tree();
        }
        let target = self.tree_target();
        let mut s = self.state.borrow_mut();
        s.tree_centred = target.clone();
        let Some(i) = target.and_then(|t| s.tree_paths.iter().position(|p| p.as_deref() == Some(t.as_str()))) else { return };
        let Some(frame) = s.tree_frames.get(i).copied() else { return };
        let offset = sidebar::centred(frame, s.tree_size.1, s.tree_total);
        drop(s);
        w.global::<Library>().set_tree_viewport_y(-offset);
    }

    /// Centres the column once the photo it follows changes, never on an edit, so a scroll by
    /// the wheel stays while the photo is worked on.
    pub(super) fn follow_tree(&self) {
        if self.state.borrow().tree_centred != self.tree_target() {
            self.centre_tree();
        }
    }

    pub(super) fn wire_tree(self: &Rc<Self>, lib: &Library) {
        let me = Rc::downgrade(self);
        lib.on_tree_resized(move |width, height| {
            let Some(c) = me.upgrade() else { return };
            let was = std::mem::replace(&mut c.state.borrow_mut().tree_size, (width, height));
            if was == (width, height) {
                return;
            }
            // A column first given its size centres on the photo; a resize keeps the scroll.
            if was.0 <= 0.0 || was.1 <= 0.0 {
                c.state.borrow_mut().tree_centred = None;
            }
            if was.0 != width {
                c.relay_tree();
            }
            c.clamp_tree();
            c.follow_tree();
        });
        // The column shown again, beside a photo or back on the archive: centred on the photo.
        let me = Rc::downgrade(self);
        lib.on_tree_shown(move |width, height| {
            let Some(c) = me.upgrade() else { return };
            let was = std::mem::replace(&mut c.state.borrow_mut().tree_size, (width, height));
            if was.0 != width {
                c.relay_tree();
            }
            c.centre_tree();
        });
        let me = Rc::downgrade(self);
        lib.on_tree_layout_next(move || {
            let Some(c) = me.upgrade() else { return };
            let next = c.state.borrow().tree_layout().next();
            c.remember(LAYOUT_KEY, serde_json::Value::from(next.key()));
            c.publish_tree();
            // A new layout moves every row: the selection comes back to the middle.
            c.centre_key();
        });
        let me = Rc::downgrade(self);
        lib.on_tree_centre(move || {
            if let Some(c) = me.upgrade() {
                c.centre_key();
            }
        });
        let me = Rc::downgrade(self);
        lib.on_tree_reload(move || {
            if let Some(c) = me.upgrade() {
                c.act("read-folders");
            }
        });
    }
}
