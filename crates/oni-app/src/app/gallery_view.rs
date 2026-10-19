//! The gallery: its layout, the visible tiles, opening a tile and the medals written from it.

use super::*;

impl Ctl {
    // MARK: - The gallery

    pub(super) fn relayout(&self) {
        {
            let mut s = self.state.borrow_mut();
            let groups: Vec<(String, String, usize)> =
                s.org.bands.iter().map(|b| (b.title.clone(), b.caption.clone(), b.entries.len())).collect();
            s.layout = Layout::new(&groups, s.columns, s.grid.0, Metrics::GALLERY);
            s.scroll = s.layout.clamped(s.scroll, s.grid.1);
        }
        self.publish_tiles();
    }

    /// The cells in view and one row either side, the only tiles Slint is given.
    pub(super) fn publish_tiles(&self) {
        // Every change of the picks lays the tiles: the sidebar's rows follow them here.
        self.mark_tree();
        // A thumbnail landed reaches the roll's cells too, which read the same memory.
        if self.state.borrow().prepare.shown {
            self.publish_prepare();
        }
        let Some(w) = self.window() else { return };
        let lib = w.global::<Library>();
        let mut s = self.state.borrow_mut();
        let range = s.layout.cells(s.scroll, s.grid.1);
        let flags = Ctl::tile_flags(&s);
        // A cell reads the folder, never renders: a photo with no file waits for the pass or an edit.
        let mut asking = Vec::new();
        for i in range.clone() {
            let Some(fingerprint) = s.order.get(i).and_then(|p| s.fingerprints.get(p)) else { continue };
            if !fingerprint.starts_with(thumbnails::PENDING) && !s.thumbs.contains(fingerprint) && !s.asked.contains(fingerprint) {
                asking.push(fingerprint.clone());
            }
        }
        for fingerprint in asking {
            s.asked.insert(fingerprint.clone());
            let _ = s.disk.send(fingerprint);
        }
        let tiles: Vec<Tile> = range
            .clone()
            .filter_map(|i| {
                let frame = s.layout.frame(i)?;
                let path = s.order.get(i)?;
                let picture = s.fingerprints.get(path).and_then(|f| s.thumbs.get(f)).unwrap_or_default();
                Some(Tile {
                    index: i as i32,
                    x: frame.x,
                    y: frame.y,
                    width: frame.width,
                    height: s.layout.tile.1,
                    picture,
                    caption: stem(path).into(),
                    selected: s.shown_picks().selection.contains(path),
                    medal: flags.get(path).map_or(0, |f| f.0),
                    missing: flags.get(path).is_some_and(|f| f.1),
                    rgb: flags.get(path).is_some_and(|f| f.2),
                })
            })
            .collect();
        let headers = s.layout.headers(s.scroll, s.grid.1);
        let headings: Vec<Heading> = s.layout.bands[headers]
            .iter()
            .map(|b| Heading { y: b.top, title: b.title.clone().into(), caption: b.caption.clone().into() })
            .collect();
        s.visible = range;
        let (height, scroll) = (s.layout.height, s.scroll);
        drop(s);
        lib.set_tiles(ModelRc::new(VecModel::from(tiles)));
        lib.set_headings(ModelRc::new(VecModel::from(headings)));
        lib.set_content_height(height);
        lib.set_scroll(scroll);
    }

    pub(super) fn tile_clicked(self: &Rc<Self>, i: usize, toggle: bool, extend: bool) {
        self.click(i, toggle, extend);
    }

    pub(super) fn tile_opened(self: &Rc<Self>, i: usize) {
        let path = self.state.borrow().order.get(i).cloned();
        if let Some(path) = path {
            self.open_in_editor(&path);
        }
    }

    /// `load(_:)` then `mode = .editor`.
    pub(super) fn open_in_editor(self: &Rc<Self>, path: &str) {
        self.state.borrow_mut().designate(path);
        self.open_photo(path);
        if let Some(w) = self.window() {
            w.global::<App>().set_gallery(false);
        }
        self.land_tree_request();
        self.publish_bar();
        self.publish_tiles();
    }

    /// `applyMedal`: the selection's sidecars and index rows, in the gallery and the editor alike.
    pub(super) fn label(self: &Rc<Self>, medal: i32) {
        let targets = self.state.borrow().selected();
        self.label_paths(&targets, library::medal_of(medal));
    }

    pub(super) fn label_paths(self: &Rc<Self>, targets: &[String], label: Medal) {
        self.journal_medal(targets, label);
        let mut written = Vec::new();
        for path in targets {
            match oni_settings::sidecar::set_label(label, Path::new(path), oni_decode::decodes_via_mosaic(Path::new(path))) {
                Ok(()) => written.push(path.clone()),
                Err(e) => self.say(&e.to_string()),
            }
        }
        self.reread(&written);
    }

    /// Rows whose sidecar a gesture wrote, read again into the index, then the gallery laid from
    /// the index, so a photo marked or restored leaves or joins it.
    pub(super) fn reread(self: &Rc<Self>, paths: &[String]) {
        if paths.is_empty() {
            return;
        }
        {
            let mut s = self.state.borrow_mut();
            let Some(folder) = s.opened.clone() else { return };
            let Some(index) = &s.index else { return };
            library::refresh(index, paths);
            let mut rows = library::shown_under(index, &folder);
            let held: HashSet<String> = rows.iter().map(|e| e.path.clone()).collect();
            let waiting: Vec<oni_library::Entry> =
                s.rows.iter().filter(|r| r.fingerprint.starts_with(thumbnails::PENDING) && !held.contains(&r.path)).cloned().collect();
            rows.extend(waiting);
            s.fingerprints = rows.iter().map(|e| (e.path.clone(), e.fingerprint.clone())).collect();
            s.rows = rows;
        }
        self.lay_archive();
    }
}
