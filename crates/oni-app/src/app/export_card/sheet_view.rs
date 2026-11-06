//! The contact sheet the export writes, shown in its column in place of the lines: each frame's
//! preview at its place, its number under it and the logo's band, laid as the file lays them.

use super::*;
use super::super::gallery_view::together;
use crate::{SheetBox, SheetTile};

impl Ctl {
    /// The sheet on the sizes measured so far, a frame still unmeasured on 3:2, fitted to the room the
    /// column gives it; none while the batch is written photo by photo.
    pub(super) fn publish_sheet_view(&self) {
        let Some(w) = self.window() else { return };
        let e = w.global::<Exporting>();
        let (paths, sizes, previews, room) = {
            let c = self.export.borrow();
            let Some(room) = c.sheet_room.filter(|_| c.on_sheet()) else {
                e.set_sheet_tiles(ModelRc::default());
                return;
            };
            let paths: Vec<String> = c.targets.iter().map(|t| oni_library::paths::normal(&t.path.to_string_lossy()).into_owned()).collect();
            let previews: Vec<Image> = (0..c.targets.len())
                .map(|i| match c.previews.get(i) {
                    Some(Preview::Ready(img)) => img.clone(),
                    _ => Image::default(),
                })
                .collect();
            (paths, c.sizes.clone(), previews, room)
        };
        let numbers = together::ranks(&paths, &self.roll_folders(&paths));
        let (pictures, band, size) = together::sheet_places(&sizes);
        let k = (room.0 / size.0).min(room.1 / size.1);
        let (left, top) = ((room.0 - size.0 * k) / 2.0, (room.1 - size.1 * k) / 2.0);
        let placed = |(x, y, w, h): (f64, f64, f64, f64)| SheetBox { x: (left + x * k) as f32, y: (top + y * k) as f32, width: (w * k) as f32, height: (h * k) as f32 };
        let tiles: Vec<SheetTile> = pictures
            .iter()
            .zip(previews)
            .zip(&numbers)
            .map(|((&area, preview), n)| {
                let b = placed(area);
                SheetTile { x: b.x, y: b.y, width: b.width, height: b.height, preview, number: n.map(|n| n.to_string()).unwrap_or_default().into() }
            })
            .collect();
        e.set_sheet_tiles(ModelRc::new(VecModel::from(tiles)));
        e.set_sheet_ground(placed((0.0, 0.0, size.0, size.1)));
        e.set_sheet_logo_box(placed(band.unwrap_or_default()));
        e.set_sheet_number_size((together::NUMBER as f64 * k) as f32);
        e.set_sheet_number_band((together::MARGIN as f64 * k) as f32);
        // A sheet of fewer than a row carries no logo.
        e.set_sheet_logo(band.and_then(|b| self.sheet_logo(b.2 as usize)).unwrap_or_default());
    }

    /// The logo's band for a sheet `width` wide, drawn once for each width.
    fn sheet_logo(&self, width: usize) -> Option<Image> {
        if let Some((key, logo)) = self.export.borrow().logo.as_ref()
            && *key == width
        {
            return Some(logo.clone());
        }
        let inks = self.inks()?;
        let logo = together::logo_image(width, inks.logo);
        self.export.borrow_mut().logo = Some((width, logo.clone()));
        Some(logo)
    }
}
