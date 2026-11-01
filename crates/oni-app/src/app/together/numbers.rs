//! Edit Together as a contact sheet: the sheet's black under its frames, its heading, and each
//! frame's place on its roll, orange under its picture as a film's edge print.

use super::*;
use crate::{EdgeNumber, SheetBox};
use super::plate::Lettering;
use super::sheet::Area;

impl Ctl {
    /// Each frame's place on its roll, read off this thread: a contact sheet's sidecar is opened to
    /// leave it out of its folder's count.
    pub(super) fn number_frames(&self, id: u64) {
        let Some(paths) = self.state.borrow().together.as_ref().map(|g| g.paths.clone()) else { return };
        let folders = self.roll_folders(&paths);
        let band = laid(&vec![UNKNOWN; paths.len()]).heading.2 as usize;
        if let Some(w) = self.window() {
            w.global::<Canvas>().set_together(true);
        }
        std::thread::spawn(move || {
            let numbers = ranks(&paths, &folders, oni_library::roll_membership::is_sheet);
            let plate = super::heading::drawn(band);
            let heading = (plate.rgba8(), plate.width, plate.height);
            with_shell(move |c| c.numbers_landed(id, numbers, heading));
        });
    }

    /// The numbers' ink and weight as the canvas sets them.
    pub(in crate::app) fn lettering(&self) -> Option<Lettering> {
        let w = self.window()?;
        let c = w.global::<Canvas>();
        let ink = c.get_number_ink();
        Some(Lettering { ink: [ink.red(), ink.green(), ink.blue()].map(|v| f32::from(v) / 255.0), weight: c.get_number_weight() as f32 })
    }

    /// Each folder of `paths` with its photos in shooting order, which `ranks` numbers them by.
    pub(in crate::app) fn roll_folders(&self, paths: &[String]) -> HashMap<String, Vec<String>> {
        let s = self.state.borrow();
        let folders: HashSet<String> = paths.iter().map(|p| oni_library::paths::parent(p)).collect();
        folders.into_iter().map(|f| (f.clone(), s.archive.as_ref().map(|a| a.folder_photos(&f)).unwrap_or_default())).collect()
    }

    fn numbers_landed(&self, id: u64, numbers: Vec<Option<usize>>, (bytes, width, height): (Vec<u8>, usize, usize)) {
        let shown = match self.state.borrow_mut().together.as_mut() {
            Some(g) if g.id == id => {
                g.numbers = numbers;
                Some(g.shown)
            }
            _ => None,
        };
        let Some(shown) = shown else { return };
        if let Some(w) = self.window() {
            w.global::<Canvas>().set_heading(image(&bytes, width, height));
        }
        if let Some(shown) = shown {
            self.lay_sheet(shown);
        }
    }

    /// The sheet's black where the view lays the full sheet (`sheet`, in canvas pixels), and each
    /// number under its picture, sized as the sheet is; those off the canvas are left out.
    pub(super) fn publish_numbers(&self, sheet: Area, full_width: f64) {
        let (Some(w), Some(view)) = (self.window(), self.canvas_size()) else { return };
        let density = f64::from(w.window().scale_factor());
        let k = sheet.2 / full_width.max(1.0);
        let (laid, band) = {
            let s = self.state.borrow();
            let Some(g) = s.together.as_ref() else { return };
            (under(&g.numbers, &g.sheet.pictures), g.sheet.heading)
        };
        let (size, number_band) = (NUMBER as f64 * k, MARGIN as f64 * k);
        let points = |v: f64| (v / density) as f32;
        let numbers: Vec<EdgeNumber> = laid
            .into_iter()
            .map(|(x, y, n)| (sheet.0 + x * k, sheet.1 + y * k, n))
            .filter(|(x, y, _)| *x < view.0 && *y < view.1 && *x + 4.0 * size > 0.0 && *y + number_band > 0.0)
            .map(|(x, y, n)| EdgeNumber { x: points(x), y: points(y), text: n.to_string().into() })
            .collect();
        let c = w.global::<Canvas>();
        c.set_sheet(SheetBox { x: points(sheet.0), y: points(sheet.1), width: points(sheet.2), height: points(sheet.3) });
        c.set_number_size(points(size));
        c.set_number_band(points(number_band));
        let (hx, hy, hw, hh) = band;
        c.set_heading_box(SheetBox { x: points(sheet.0 + hx * k), y: points(sheet.1 + hy * k), width: points(hw * k), height: points(hh * k) });
        if let Some(m) = moved_to(&c.get_numbers(), numbers) {
            c.set_numbers(m);
        }
    }

    /// Back to one frame: the canvas's own ground, no numbers.
    pub(super) fn unnumbered(&self) {
        let Some(w) = self.window() else { return };
        let c = w.global::<Canvas>();
        c.set_together(false);
        c.set_numbers(ModelRc::default());
        c.set_heading(slint::Image::default());
    }
}

/// Each frame's place on its roll from 1, its rank in its folder's shooting order with the contact
/// sheets left out; a frame its folder does not list, or a sheet, has none.
pub(in crate::app) fn ranks(paths: &[String], folders: &HashMap<String, Vec<String>>, sheet: impl Fn(&str) -> bool) -> Vec<Option<usize>> {
    let rolls: HashMap<&str, HashMap<&str, usize>> = folders
        .iter()
        .map(|(folder, photos)| {
            let frames = photos.iter().filter(|p| !sheet(p)).enumerate().map(|(i, p)| (p.as_str(), i + 1)).collect();
            (folder.as_str(), frames)
        })
        .collect();
    paths
        .iter()
        .map(|p| rolls.get(oni_library::paths::parent(p).as_str()).and_then(|roll| roll.get(p.as_str()).copied()))
        .collect()
}

/// Where each numbered frame's number band starts on the full sheet: at its picture's bottom left.
pub(super) fn under(numbers: &[Option<usize>], pictures: &[Area]) -> Vec<(f64, f64, usize)> {
    numbers.iter().zip(pictures).filter_map(|(n, &(x, y, _, h))| Some((x, y + h, (*n)?))).collect()
}

/// The model rewritten in place when it holds as many numbers, so a pan keeps every label; else a new one.
fn moved_to(model: &ModelRc<EdgeNumber>, numbers: Vec<EdgeNumber>) -> Option<ModelRc<EdgeNumber>> {
    match model.as_any().downcast_ref::<VecModel<EdgeNumber>>() {
        Some(kept) if kept.row_count() == numbers.len() => {
            for (i, n) in numbers.into_iter().enumerate() {
                if kept.row_data(i).as_ref() != Some(&n) {
                    kept.set_row_data(i, n);
                }
            }
            None
        }
        _ => Some(ModelRc::new(VecModel::from(numbers))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_is_numbered_by_its_place_in_its_folder_its_contact_sheets_left_out() {
        let roll: Vec<String> = ["/R/sheet.tif", "/R/a.RW2", "/R/b.RW2", "/R/c.RW2"].map(String::from).to_vec();
        let folders = HashMap::from([("/R".to_string(), roll)]);
        let paths: Vec<String> = ["/R/c.RW2", "/R/a.RW2", "/R/sheet.tif", "/S/x.RW2"].map(String::from).to_vec();
        let is_sheet = |p: &str| p.ends_with(".tif");
        assert_eq!(ranks(&paths, &folders, is_sheet), vec![Some(3), Some(1), None, None]);
        // Twin: the sheet counted, every frame after it reads one further down the roll.
        assert_eq!(ranks(&paths, &folders, |_| false)[0], Some(4));
    }

    #[test]
    fn a_number_lies_under_its_picture_whatever_its_side() {
        let sheet = laid(&[(3.0, 2.0), (2.0, 3.0), (3.0, 2.0)]);
        let laid = under(&[Some(1), Some(2), None], &sheet.pictures);
        let (x, y, _, h) = sheet.pictures[0];
        assert_eq!(laid[0], (x, y + h, 1), "at a landscape picture's bottom, inside its row");
        let (x, y, _, h) = sheet.pictures[1];
        assert_eq!(laid[1], (x, y + h, 2), "at a portrait one's, the row's own");
        assert!(laid[0].1 < laid[1].1, "twin: the landscape's band is not left at the row's bottom");
        // A frame with no number has no label.
        assert_eq!(laid.len(), 2);
    }
}
