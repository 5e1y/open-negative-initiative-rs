//! The gallery: its layout, the visible tiles, opening a tile and the medals written from it.

use super::*;

#[path = "together.rs"]
pub(super) mod together;

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

    /// `land(through:)` as the grid mounts: the open photo centred, else the top; the ONI Trash
    /// names no open photo.
    pub(super) fn land_gallery(&self) {
        {
            let mut s = self.state.borrow_mut();
            // The archive's tree beside the photo is gone, and its pick with it.
            s.org.tree_pick = crate::tree_pick::TreePick::default();
            let open = s.open.clone().filter(|_| !s.trash_shown());
            let at = open.and_then(|p| s.order.iter().position(|o| *o == p));
            s.scroll = s.layout.landing(at, s.grid.1);
        }
        self.publish_tiles();
    }

    /// A thumbnail drawn again onto the cells already showing its photo, the only ones it changes.
    pub(super) fn relay_thumbnail(&self, fingerprint: &str) {
        let Some(w) = self.window() else { return };
        let model = w.global::<Library>().get_tiles();
        let Some(rows) = model.as_any().downcast_ref::<VecModel<Tile>>() else { return };
        let s = self.state.borrow();
        let Some(picture) = s.thumbs.get(fingerprint) else { return };
        for at in 0..rows.row_count() {
            let Some(mut row) = rows.row_data(at) else { continue };
            let shows = s.order.get(row.index as usize).and_then(|p| s.fingerprints.get(p)).is_some_and(|f| f == fingerprint);
            if shows && row.width > 0.0 {
                row.picture = picture.clone();
                rows.set_row_data(at, row);
            }
        }
    }

    /// The cells in view and one row either side, the only tiles Slint is given.
    pub(super) fn publish_tiles(&self) {
        // Every change of the picks lays the tiles: the sidebar's rows follow them here.
        self.mark_tree();
        self.fill_tree_thumbnails();
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
        // One field at a time: the pinned heading carries it while it covers its band's own.
        let pinned = s.layout.pinned(s.scroll).map(|(i, _)| i);
        let headings: Vec<Heading> = headers
            .map(|i| {
                let b = &s.layout.bands[i];
                let renaming = s.heading_renamed(i) && pinned != Some(i);
                Heading { y: b.top, title: b.title.clone().into(), caption: b.caption.clone().into(), renaming }
            })
            .collect();
        let slots = range.len();
        s.visible = range;
        let (height, scroll) = (s.layout.height, s.scroll);
        drop(s);
        lay_tiles(&lib, tiles, slots);
        set_headings(&lib, headings);
        self.publish_pinned();
        lib.set_content_height(height);
        lib.set_scroll(scroll);
    }

    /// The heading held at the grid's top, which follows every scroll, the offset's alone too.
    pub(super) fn publish_pinned(&self) {
        let Some(w) = self.window() else { return };
        let pinned = {
            let s = self.state.borrow();
            s.layout.pinned(s.scroll).map(|(i, y)| (y, s.layout.bands[i].title.clone(), s.layout.bands[i].caption.clone(), s.heading_renamed(i)))
        };
        let lib = w.global::<Library>();
        lib.set_pinned_shown(pinned.is_some());
        if let Some((y, title, caption, renaming)) = pinned {
            lib.set_pinned_y(y);
            lib.set_pinned_title(title.into());
            lib.set_pinned_caption(caption.into());
            lib.set_pinned_renaming(renaming);
        }
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

    /// `takeOff`: `load(_:)` unless the photo is the one open, then `mode = .editor`.
    pub(super) fn open_in_editor(self: &Rc<Self>, path: &str) {
        let reopened = self.state.borrow().open.as_deref() == Some(path);
        let together = self.state.borrow().selected();
        let together = if together.len() > 1 && together.iter().any(|p| p == path) { together } else { Vec::new() };
        self.state.borrow_mut().designate(path);
        if !reopened {
            self.open_photo(path);
        }
        self.edit_together(together);
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

/// Row by row into the model shown: a heading typed over keeps its field while the tiles are laid again.
fn set_headings(lib: &Library, headings: Vec<Heading>) {
    let model = lib.get_headings();
    if let Some(shown) = model.as_any().downcast_ref::<VecModel<Heading>>()
        && shown.row_count() == headings.len()
    {
        for (i, h) in headings.into_iter().enumerate() {
            if shown.row_data(i).as_ref() != Some(&h) {
                shown.set_row_data(i, h);
            }
        }
        return;
    }
    lib.set_headings(ModelRc::new(VecModel::from(headings)));
}

/// The grid's tiles in one kept model, each photo in the slot its index keeps while in view: a scroll
/// or a pick rewrites only the cells that changed, and Slint uploads no unchanged thumbnail again.
fn lay_tiles(lib: &Library, tiles: Vec<Tile>, slots: usize) {
    let slots = slots.max(1);
    let model = lib.get_tiles();
    let kept = model.as_any().downcast_ref::<VecModel<Tile>>().filter(|m| m.row_count() == slots);
    let Some(kept) = kept else {
        let mut rows = vec![Tile::default(); slots];
        for tile in tiles {
            let at = tile.index as usize % slots;
            rows[at] = tile;
        }
        return lib.set_tiles(ModelRc::new(VecModel::from(rows)));
    };
    let mut rows: Vec<Option<Tile>> = vec![None; slots];
    for tile in tiles {
        let at = tile.index as usize % slots;
        rows[at] = Some(tile);
    }
    for (at, row) in rows.into_iter().enumerate() {
        let new = row.unwrap_or_default();
        let old = kept.row_data(at).unwrap_or_default();
        if !same_tile(&old, &new) {
            kept.set_row_data(at, new);
        }
    }
}

/// The same cell but for its picture, which only a landing thumbnail changes (`relay_thumbnail`)
/// once the cell holds one; comparing pictures would read every pixel of every tile.
fn same_tile(old: &Tile, new: &Tile) -> bool {
    let held = old.picture.size().width > 0 || new.picture.size().width == 0;
    let place = (old.index, old.x, old.y, old.width, old.height) == (new.index, new.x, new.y, new.width, new.height);
    let marks = (old.selected, old.medal, old.missing, old.rgb) == (new.selected, new.medal, new.missing, new.rgb);
    held && place && marks && old.caption == new.caption
}


#[cfg(test)]
mod tests {
    use super::*;

    fn picture(side: u32) -> Image {
        Image::from_rgba8(slint::SharedPixelBuffer::new(side, side))
    }

    #[test]
    fn a_cell_is_rewritten_only_when_more_than_its_held_picture_changes() {
        let tile = Tile { index: 7, width: 100.0, picture: picture(4), caption: "DSC_1".into(), ..Tile::default() };
        // Its picture drawn again goes through the relay, and a pending one never blanks a held one.
        assert!(same_tile(&tile, &Tile { picture: picture(8), ..tile.clone() }));
        assert!(same_tile(&tile, &Tile { picture: Image::default(), ..tile.clone() }));
        // Twins: a pick, a move or a first picture each rewrite the cell.
        assert!(!same_tile(&tile, &Tile { selected: true, ..tile.clone() }));
        assert!(!same_tile(&tile, &Tile { index: 8, ..tile.clone() }));
        let pending = Tile { picture: Image::default(), ..tile.clone() };
        assert!(!same_tile(&pending, &tile));
    }
}
