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
        let (paths, sizes, previews, positive, room) = {
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
            (paths, c.sizes.clone(), previews, together::positive(c.targets.iter().map(|t| &t.settings)), room)
        };
        let numbers = together::ranks(&paths, &self.roll_folders(&paths), oni_library::roll_membership::is_sheet);
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
        e.set_sheet_logo_box(placed(band));
        e.set_sheet_number_size((together::NUMBER as f64 * k) as f32);
        e.set_sheet_number_band((together::MARGIN as f64 * k) as f32);
        e.set_sheet_positive(positive);
        if let Some(logo) = self.sheet_logo(band.2 as usize, positive) {
            e.set_sheet_logo(logo);
        }
    }

    /// The logo's band for a sheet `width` wide, drawn once for each width and ground.
    fn sheet_logo(&self, width: usize, positive: bool) -> Option<Image> {
        if let Some((key, logo)) = self.export.borrow().logo.as_ref()
            && *key == (width, positive)
        {
            return Some(logo.clone());
        }
        let inks = self.inks(positive)?;
        let logo = together::logo_image(width, inks.logo);
        self.export.borrow_mut().logo = Some(((width, positive), logo.clone()));
        Some(logo)
    }
}
