//! A photo's flight between the screens: the editor opening on it from the gallery and the gallery coming
//! back to it, the export opening on it from the editor and closing back onto it; the window draws the
//! flight and lands it where the next screen lays the photo, Rust names its picture and where it leaves.

use super::*;
use crate::{Flight, FlightRoute, Place, Walk};

impl Ctl {
    /// The tile of `path` where the grid lays it now, in the window's points, with its index; none while
    /// it lies outside the laid cells.
    pub(super) fn tile_in_window(&self, path: &str) -> Option<(usize, Place)> {
        let w = self.window()?;
        let lib = w.global::<Library>();
        let s = self.state.borrow();
        let i = s.order.iter().position(|p| p == path)?;
        if !s.visible.contains(&i) {
            return None;
        }
        let frame = s.layout.frame(i)?;
        Some((i, Place { x: lib.get_grid_x() + frame.x, y: lib.get_grid_y() + frame.y - s.scroll, width: frame.width, height: s.layout.tile.1 }))
    }

    /// Where the canvas shows the photo now, in the window's points; none before it has placed one.
    fn canvas_in_window(&self) -> Option<Place> {
        let w = self.window()?;
        let (c, f) = (w.global::<Canvas>(), w.global::<Flight>());
        (c.get_frame_width() > 0.0 && c.get_frame_height() > 0.0).then(|| Place {
            x: f.get_canvas_x() + c.get_frame_x(),
            y: f.get_canvas_y() + c.get_frame_y(),
            width: c.get_frame_width(),
            height: c.get_frame_height(),
        })
    }

    /// The flight of `path`'s thumbnail from `from`; none without a thumbnail or under Reduce Motion, the
    /// next screen then showing as it lands.
    fn fly(&self, path: &str, route: FlightRoute, from: Place, tile: Option<(usize, Place)>) {
        let Some(w) = self.window() else { return };
        if w.global::<crate::Motion>().get_reduced() {
            return;
        }
        let picture = {
            let s = self.state.borrow();
            s.fingerprints.get(path).and_then(|f| s.thumbs.get(f))
        };
        let Some(picture) = picture else { return };
        let f = w.global::<Flight>();
        let (index, place) = tile.map_or((-1, Place::default()), |(i, p)| (i as i32, p));
        f.set_picture(picture);
        f.set_route(route);
        f.set_from(from);
        f.set_tile(index);
        f.set_tile_place(place);
        f.set_serial(f.get_serial() + 1);
        f.set_flying(true);
    }

    /// The gallery's tile lifting into the canvas, started before the editor shows: only a tile on screen flies.
    pub(super) fn fly_into_editor(&self, path: &str) {
        if let Some(tile) = self.tile_in_window(path) {
            self.fly(path, FlightRoute::IntoEditor, tile.1.clone(), Some(tile));
        }
    }

    /// The canvas's photo shrinking into its tile, whose place the grid lays as it shows (`aim_flight`).
    pub(super) fn fly_into_gallery(&self, path: &str) {
        if let Some(from) = self.canvas_in_window() {
            self.fly(path, FlightRoute::IntoGallery, from, None);
        }
    }

    /// The canvas's photo moving onto the export's lone preview, started before the export shows.
    pub(super) fn fly_into_export(&self, path: &str) {
        if let Some(from) = self.canvas_in_window() {
            self.fly(path, FlightRoute::IntoExport, from, None);
        }
    }

    /// The export's lone preview moving back onto the canvas, from where the export laid it.
    pub(super) fn fly_out_of_export(&self, path: &str) {
        let Some(w) = self.window() else { return };
        let from = w.global::<Flight>().get_slot();
        if from.width > 0.0 {
            self.fly(path, FlightRoute::OutOfExport, from, None);
        }
    }

    /// The tile a flight back to the gallery lands in, each time the grid lays its cells.
    pub(super) fn aim_flight(&self) {
        let Some(w) = self.window() else { return };
        let f = w.global::<Flight>();
        if !f.get_flying() || f.get_route() != FlightRoute::IntoGallery {
            return;
        }
        let open = self.state.borrow().open_path().map(str::to_string);
        let Some((index, place)) = open.and_then(|p| self.tile_in_window(&p)) else { return };
        f.set_tile(index as i32);
        f.set_tile_place(place);
    }

    /// The photo left as the editor walks to `to`, beside a photo alone: the picture the canvas shows steps
    /// aside, the way the walk goes in the gallery's order, whatever its thumbnail's state.
    pub(super) fn walk_from(&self, to: &str) {
        let Some(w) = self.window() else { return };
        if w.global::<App>().get_gallery() || w.global::<crate::Exporting>().get_shown() || w.global::<crate::Motion>().get_reduced() {
            return;
        }
        let Some(from) = self.canvas_in_window() else { return };
        let way = {
            let s = self.state.borrow();
            let Some(left) = s.open_path().filter(|p| *p != to) else { return };
            let at = |p: &str| s.order.iter().position(|o| o == p);
            match (at(left), at(to)) {
                (Some(a), Some(b)) if b < a => -1,
                _ => 1,
            }
        };
        let (canvas, walk) = (w.global::<Canvas>(), w.global::<Walk>());
        walk.set_picture(canvas.get_frame());
        walk.set_clip_width(canvas.get_frame_clip_width());
        walk.set_clip_height(canvas.get_frame_clip_height());
        walk.set_from(from);
        walk.set_way(way);
        walk.set_serial(walk.get_serial() + 1);
        walk.set_walking(true);
    }

    /// Prepare takes the screen, as a photo of a roll never prepared opens it once its roll is read: a photo
    /// flying into the editor never lands on a canvas no longer shown.
    pub(super) fn flight_left(&self) {
        let Some(w) = self.window() else { return };
        let f = w.global::<Flight>();
        if f.get_route() == FlightRoute::IntoEditor {
            f.set_flying(false);
            f.set_landing(false);
        }
    }

    /// Prepare opened from the gallery, by its key or a photo of the roll opened there: each frame of the roll
    /// leaves from its tile as the grid laid it, the photo opened ringed; a tile laid off screen has no place.
    pub(super) fn wave_from_gallery(&self, paths: &[String], gallery: bool) {
        let Some(w) = self.window() else { return };
        if !gallery || w.global::<crate::Motion>().get_reduced() {
            return;
        }
        let opened = self.state.borrow().open_path().map(str::to_string);
        let places: Vec<Place> = paths.iter().map(|p| self.tile_in_window(p).map(|(_, place)| place).unwrap_or_default()).collect();
        let p = w.global::<crate::Preparing>();
        p.set_opened_cell(paths.iter().position(|p| Some(p) == opened.as_ref()).map_or(-1, |i| i as i32));
        p.set_wave_from(slint::ModelRc::new(slint::VecModel::from(places)));
        p.set_wave(p.get_wave() + 1);
    }
}
