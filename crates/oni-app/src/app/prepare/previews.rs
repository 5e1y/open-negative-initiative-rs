//! The roll's previews, rendered once and drawn on the card and the sheet.

use super::*;

impl Ctl {
    // MARK: - Previews

    pub(in crate::app) fn preview_landed(self: &Rc<Self>, session: u64, out: Out) {
        {
            let r = &mut self.state.borrow_mut().prepare;
            if r.session != session || !r.shown {
                return;
            }
            match out {
                Out::Tile(path, b) => {
                    r.fresh_tiles.insert(path.clone());
                    r.tiles.insert(path, picture(&b));
                }
                Out::Card(path, b) => r.card = Some((path, picture(&b))),
                // A picture drawn again for the same photo is laid again, whatever its size.
                Out::Scan(path, b) => {
                    let size = (b.width, b.height);
                    r.scan = Some((path, picture(&b), size));
                    (r.laid_scan, r.laid_loupe) = (None, None);
                }
                Out::Full(path, b) => {
                    let size = (b.width, b.height);
                    r.full = Some((path, picture(&b), size));
                    r.laid_loupe = None;
                }
            }
        }
        self.publish_prepare();
    }

    /// The thumbnails of the roll's frames asked of the folder, as the grid asks its visible cells.
    pub(in crate::app) fn ask_thumbnails(s: &mut State) {
        let wanted: Vec<String> = s
            .prepare
            .frames
            .iter()
            .map(|f| f.fingerprint.clone())
            .filter(|f| !f.starts_with(crate::thumbnails::PENDING) && !s.thumbs.contains(f) && !s.asked.contains(f) && !s.prepare.asked.contains(f))
            .collect();
        for f in wanted {
            s.asked.insert(f.clone());
            s.prepare.asked.insert(f.clone());
            let _ = s.disk.send(f);
        }
    }
}
