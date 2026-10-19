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
        s.tree_frames = frames;
        s.tree_total = total;
        let lib = w.global::<Library>();
        lib.set_tree_content_height(total);
        lib.set_tree_layout(Layout::ALL.iter().position(|l| *l == layout).unwrap_or(0) as i32);
        lib.set_tree_layout_help(format!("{} — next: {}", layout.name(), layout.next().name()).into());
    }

    /// A thumbnail landed after the rows were laid reaches its row in place.
    pub(super) fn fill_tree_thumbnails(&self) {
        let Some(w) = self.window() else { return };
        let model = w.global::<Library>().get_rows();
        let s = self.state.borrow();
        for (i, path) in s.tree_paths.iter().enumerate() {
            let Some(mut row) = model.row_data(i).filter(|r| r.thumb && r.picture.size().width == 0) else { continue };
            let Some(picture) = path.as_ref().and_then(|p| s.fingerprints.get(p)).and_then(|f| s.thumbs.get(f)) else { continue };
            row.picture = picture;
            model.set_row_data(i, row);
        }
    }

    /// `TreeEngine.reveal`: the column centred on the open photo, else the first picked.
    pub(super) fn centre_tree(&self) {
        let Some(w) = self.window() else { return };
        let s = self.state.borrow();
        let target = s.open.clone().or_else(|| s.picks.anchor.clone());
        let Some(i) = target.and_then(|t| s.tree_paths.iter().position(|p| p.as_deref() == Some(t.as_str()))) else { return };
        let Some(frame) = s.tree_frames.get(i).copied() else { return };
        let offset = sidebar::centred(frame, s.tree_size.1, s.tree_total);
        w.global::<Library>().set_tree_viewport_y(-offset);
    }

    pub(super) fn wire_tree(self: &Rc<Self>, lib: &Library) {
        let me = Rc::downgrade(self);
        lib.on_tree_resized(move |width, height| {
            let Some(c) = me.upgrade() else { return };
            if c.state.borrow().tree_size == (width, height) {
                return;
            }
            c.state.borrow_mut().tree_size = (width, height);
            c.publish_tree();
            c.centre_tree();
        });
        let me = Rc::downgrade(self);
        lib.on_tree_layout_next(move || {
            let Some(c) = me.upgrade() else { return };
            let next = c.state.borrow().tree_layout().next();
            c.remember(LAYOUT_KEY, serde_json::Value::from(next.key()));
            c.publish_tree();
            // A new layout moves every row: the selection comes back to the middle.
            c.centre_tree();
        });
        let me = Rc::downgrade(self);
        lib.on_tree_centre(move || {
            if let Some(c) = me.upgrade() {
                c.centre_tree();
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
