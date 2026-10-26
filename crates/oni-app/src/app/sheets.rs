//! The contact sheet: New Contact Sheet over the selection and the sheet a roll's measure lays,
//! each written as a new 16-bit TIFF in working light beside its lead, then adopted as a photograph.

use super::{Ctl, with_shell};
use oni_measure::sheet::{self, Layout, Picture};
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{Date, Film, PipelineSettings};
use oni_export::Failure;
use oni_export::file::{self, Refused};
use oni_i18n::{marked, tr};
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// `SheetStep`: the opaque steps after the decode, named so a silent wait never reads as a hang.
pub const WRITING: &str = marked("Writing the file");
pub const ADOPTING: &str = marked("Adding it to the library");
pub const OPENING: &str = marked("Opening it");

/// `makingSheet`, `sheetDone`, `sheetTotal`, `sheetStep`: a run under way bars a second.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Making {
    pub done: usize,
    pub total: usize,
    pub step: &'static str,
}

/// `canMakeSheet`.
pub fn can_make(making: bool, selected: usize) -> bool {
    !making && selected > 1
}

/// `sheetTitle`: says how many frames the long run will grind through.
pub fn title(making: bool, selected: usize) -> String {
    if making {
        tr("Making Contact Sheet…").into()
    } else if selected > 1 {
        tr!("New Contact Sheet from {count} Photos", count = selected)
    } else {
        tr("New Contact Sheet").into()
    }
}

/// A sheet written to disk, waiting to be adopted.
#[derive(Clone, Debug)]
pub(crate) struct Sheet {
    pub path: PathBuf,
    pub grid: oni_settings::frame::ContactSheetGrid,
    pub film: Option<Film>,
    /// A second before the earliest frame, so the sheet heads its own group.
    pub stands: Date,
    pub mosaic: bool,
}

/// The TIFF's bytes: 16-bit RGBA, uncompressed so a full sheet reopens fast, in working light.
pub fn encoded(layout: &Layout, pixels: &[[f32; 4]], icc: &[u8]) -> Option<Vec<u8>> {
    let samples: Vec<u16> = pixels.iter().flatten().map(|v| (v.clamp(0.0, 1.0) * 65535.0).round() as u16).collect();
    let none = oni_export::ifd::Directory::default();
    oni_export::tiff::encoded(layout.width, layout.height, &oni_colour::Quantised::Sixteen(samples), false, icc, &none, &none)
}

/// Where a sheet lands: beside its lead under the first free number, or over the roll's former sheet.
#[derive(Clone, Copy, Debug)]
pub enum To<'a> {
    Beside { lead: &'a Path, stem: &'a str },
    Over(&'a Path),
}

/// Names a sheet tries before giving up, each taken by a file arriving while the sheet is written.
const TRIES: usize = 64;

/// `ContactSheet.write`: a new file the app makes, or a sheet it made before swapped whole; never an
/// original, as the free name is held at the write itself, not when the sheet was asked for.
pub fn write(layout: &Layout, pixels: &[[f32; 4]], to: To<'_>) -> Result<PathBuf, Failure> {
    let icc = oni_colour::working::working_profile().encode().map_err(|_| Failure::Profile)?;
    let bytes = encoded(layout, pixels, &icc).ok_or(Failure::Write(std::io::ErrorKind::InvalidData))?;
    let failed = |e: std::io::Error| Failure::Write(e.kind());
    match to {
        To::Over(former) => match file::replace(former, &bytes) {
            Ok(()) => Ok(former.to_path_buf()),
            Err(Refused::Io(e)) => Err(failed(e)),
            Err(Refused::Taken) => Err(Failure::WouldOverwriteOriginal),
        },
        To::Beside { lead, stem } => {
            for _ in 0..TRIES {
                // A sidecar left by a sheet gone takes its name too: a new sheet never inherits its medal or trash mark.
                let taken = |p: &Path| std::fs::symlink_metadata(p).is_ok() || std::fs::symlink_metadata(sidecar::url_for(p)).is_ok();
                let destination = sheet::destination(lead, stem, taken);
                match file::create(&destination, &bytes) {
                    Ok(()) => return Ok(destination),
                    Err(Refused::Taken) => continue,
                    Err(Refused::Io(e)) => return Err(failed(e)),
                }
            }
            Err(Failure::WouldOverwriteOriginal)
        }
    }
}

/// `ContactSheet.compose`: each frame decoded to the tile side on its own settings, its head of the
/// chain run, then laid; `on_frame` counts every frame tried, read or not.
pub fn compose(paths: &[String], side: f64, on_frame: impl Fn(usize)) -> Option<(Layout, Vec<[f32; 4]>)> {
    let lead = paths.first()?;
    let m = crate::measuring::measurer()?;
    let mut pictures: Vec<(String, Picture)> = Vec::new();
    let mut windows = Vec::new();
    for (i, path) in paths.iter().enumerate() {
        let mosaic = oni_decode::decodes_via_mosaic(Path::new(path));
        let settings = sidecar::read(Path::new(path), mosaic, false).map(|l| l.settings).unwrap_or_default();
        if let Some(p) = m.reduced(path, &settings, side, |r, copy| sheet::tile(r, copy, &settings, f64::INFINITY)) {
            pictures.push((path.clone(), p));
            windows.push(oni_measure::window::in_crop(&settings.geometry));
        }
        on_frame(i + 1);
    }
    laid(&pictures, lead, side, &windows)
}

/// `ContactSheet.laid` on pictures already rendered: the grid, then the pixels on it.
pub fn laid(pictures: &[(String, Picture)], lead: &str, side: f64, windows: &[Option<oni_settings::cg::Rect>]) -> Option<(Layout, Vec<[f32; 4]>)> {
    let sizes: Vec<(String, (usize, usize))> = pictures.iter().map(|(p, b)| (p.clone(), (b.width, b.height))).collect();
    let layout = sheet::laid(&sizes, lead, side, windows)?;
    let only: Vec<Picture> = pictures.iter().map(|(_, b)| b.clone()).collect();
    let pixels = sheet::composed(&layout, &only);
    Some((layout, pixels))
}

/// `ContactSheet.name(forRoll:on:)`, the day spelled in local time.
pub fn named(film: Option<&Film>, day: Date) -> String {
    sheet::name(film, &oni_library::roll_folder::dated(day))
}

/// Seconds a sheet stands before its earliest frame.
const STANDS_BEFORE: f64 = 1.0;

pub fn stands(earliest: Date) -> Date {
    Date::since_reference_date(earliest.time_interval_since_reference_date() - STANDS_BEFORE)
}

impl Ctl {
    /// `makeContactSheet`: the selection in gallery order, decoded off this thread with its count,
    /// written beside the first frame, then adopted and opened.
    pub(super) fn make_contact_sheet(self: &Rc<Self>) {
        let plan = {
            let s = self.state.borrow();
            if s.making_sheet.is_some() {
                return;
            }
            let frames: Vec<&oni_library::Entry> =
                s.order.iter().filter(|p| s.picks.selection.contains(*p)).filter_map(|p| s.rows.iter().find(|e| e.path == *p)).collect();
            if frames.len() < 2 {
                return;
            }
            let paths: Vec<String> = frames.iter().map(|e| e.path.clone()).collect();
            let lead = paths[0].clone();
            let donor = if s.open.as_deref() == Some(lead.as_str()) {
                Some(s.store.settings().clone())
            } else {
                sidecar::read(Path::new(&lead), oni_decode::decodes_via_mosaic(Path::new(&lead)), false).ok().map(|l| l.settings)
            };
            let earliest = frames.iter().map(|e| oni_library::film_group::capture(e)).reduce(|a, b| if b < a { b } else { a }).unwrap_or_else(Date::now);
            let film = frames[0].film.clone();
            (paths, lead, donor, earliest, film, s.support.clone())
        };
        let (paths, lead, donor, earliest, film, support) = plan;
        let Some(donor) = donor.filter(|d| d.roll_reference.is_some()) else {
            return self.advise(tr("Something Went Wrong"), tr("Prepare this roll first: a contact sheet carries the window its roll was measured on."));
        };
        let stem = named(film.as_ref(), earliest);
        self.state.borrow_mut().making_sheet = Some(Making { done: 0, total: paths.len(), step: "" });
        self.publish_bar();
        let stands = stands(earliest);
        std::thread::Builder::new()
            .name("contact-sheet".into())
            .spawn(move || {
                let laid: Vec<String> = paths.iter().filter(|p| !crate::flat_fields::is_mask_source(&support, Path::new(p))).cloned().collect();
                let composed = compose(&laid, sheet::roll_tile(), |done| with_shell(move |c| c.sheet_progressed(done, "")));
                let made = composed.ok_or(Failure::Render).and_then(|(layout, pixels)| {
                    with_shell(|c| c.sheet_progressed(0, WRITING));
                    let path = write(&layout, &pixels, To::Beside { lead: Path::new(&lead), stem: &stem })?;
                    let mosaic = laid.iter().all(|p| oni_decode::decodes_via_mosaic(Path::new(p)));
                    Ok(Sheet { path, grid: layout.grid, film, stands, mosaic })
                });
                with_shell(move |c| c.sheet_made(made, donor));
            })
            .expect("contact-sheet thread");
    }

    fn sheet_progressed(self: &Rc<Self>, done: usize, step: &'static str) {
        if let Some(m) = self.state.borrow_mut().making_sheet.as_mut() {
            // Kept translated for the bar; the empty step, the decode's count, never goes through `tr`.
            if step.is_empty() {
                m.done = done;
                m.step = step;
            } else {
                m.step = tr(step);
            }
        }
        self.publish_bar();
    }

    fn sheet_made(self: &Rc<Self>, made: Result<Sheet, Failure>, donor: PipelineSettings) {
        let sheet = match made {
            Ok(sheet) => sheet,
            Err(failure) => {
                self.state.borrow_mut().making_sheet = None;
                self.publish_bar();
                let cause = if failure == Failure::Render { String::new() } else { format!(" {failure}") };
                return self.advise(tr("Something Went Wrong"), &format!("{}{cause}", tr("The contact sheet could not be written.")));
            }
        };
        self.sheet_progressed(0, ADOPTING);
        self.adopt_sheet(&sheet, &donor, true);
        self.state.borrow_mut().making_sheet = None;
        self.publish_bar();
    }

    /// `adopt(_:standingOn:in:settings:opening:)`: its sidecar on the roll's settings, its index row
    /// standing a second before the roll, its thumbnail drawn, then opened when asked.
    pub(super) fn adopt_sheet(self: &Rc<Self>, sheet: &Sheet, donor: &PipelineSettings, opening: bool) {
        let path = oni_library::paths::canonical(&sheet.path);
        let settings = sheet::settings(donor, sheet.grid.clone(), sheet.film.clone(), sheet.mosaic);
        let now = Date::now();
        if let Err(e) = sidecar::write(&settings, Path::new(&path), WriteOptions { imported_at: Some(now), ..WriteOptions::default() }) {
            self.say(&e.to_string());
        }
        {
            let s = self.state.borrow();
            if let Some(index) = &s.index {
                let fingerprint = oni_library::relink::fingerprint(Path::new(&path)).unwrap_or_else(|| format!("sheet-{path}"));
                let mut entry = oni_library::Entry::new(&path, now, &fingerprint);
                entry.captured_at = Some(sheet.stands);
                entry.film = sheet.film.clone();
                entry.mode = Some(settings.mode);
                let _ = index.upsert(&entry);
            }
        }
        self.reread(std::slice::from_ref(&path));
        self.redraw(&path);
        if opening {
            self.sheet_progressed(0, OPENING);
            self.open_in_editor(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plate(w: usize, h: usize, value: f32) -> Picture {
        Picture { width: w, height: h, pixels: vec![[value, 0.5, 0.25, 1.0]; w * h] }
    }

    fn centre(layout: &Layout, decoded: &crate::render::Linear) -> [f32; 4] {
        let f = layout.tiles[0].frame;
        let (x, y) = ((f.x + f.width / 2.0).round() as usize, (f.y + f.height / 2.0).round() as usize);
        decoded.pixels[(decoded.height - 1 - y) * decoded.width + x]
    }

    /// `ContactSheet.selfCheck`'s file: written as one file, read back still scene-linear; written
    /// under another profile, the same pixel comes back off.
    #[test]
    fn the_sheet_file_gives_its_pixels_back_in_working_light() {
        let dir = std::env::temp_dir().join(format!("oni-sheet-file-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let value = 0.130_204_9;
        let (layout, pixels) = laid(&[("k".into(), plate(80, 80, value))], "k", 60.0, &[]).unwrap();
        let file = write(&layout, &pixels, To::Beside { lead: &dir.join("k"), stem: "sheet" }).unwrap();
        assert_eq!(file, dir.join("sheet.tif"));
        let back = crate::render::decode(&file, crate::render::Reading::default()).unwrap().linear;
        assert_eq!((back.width, back.height), (layout.width, layout.height));
        let read = centre(&layout, &back);
        // 16 bits quantise at 1.5e-5.
        assert!((read[0] - value).abs() < 2e-5, "{value} laid, {} returned", read[0]);
        let corner = back.pixels[0];
        assert!(corner[3] < 1e-4, "the clear ground stays clear: {corner:?}");
        // Twin: the same values under sRGB's profile come back far from what was laid.
        let srgb = oni_colour::space::Space::Srgb.icc_data().unwrap();
        let wrong = dir.join("wrong.tif");
        std::fs::write(&wrong, encoded(&layout, &pixels, &srgb).unwrap()).unwrap();
        let off = centre(&layout, &crate::render::decode(&wrong, crate::render::Reading::default()).unwrap().linear);
        assert!((off[0] - value).abs() > 1e-3, "{} under sRGB", off[0]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_sheet_never_takes_the_name_of_a_sidecar_left_behind() {
        let dir = std::env::temp_dir().join(format!("oni-sheet-orphan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (layout, pixels) = laid(&[("k".into(), plate(40, 40, 0.2))], "k", 30.0, &[]).unwrap();
        let lead = dir.join("k");
        // A sheet's file deleted outside the app, its sidecar left: the next sheet goes past its name.
        std::fs::write(sidecar::url_for(&dir.join("sheet.tif")), br#"{"label":"gold"}"#).unwrap();
        assert_eq!(write(&layout, &pixels, To::Beside { lead: &lead, stem: "sheet" }).unwrap(), dir.join("sheet 2.tif"));
        // Twin: a name with no sidecar is taken as asked.
        assert_eq!(write(&layout, &pixels, To::Beside { lead: &lead, stem: "free" }).unwrap(), dir.join("free.tif"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_key_needs_two_photos_and_bars_a_second_run() {
        assert!(can_make(false, 2) && !can_make(false, 1) && !can_make(true, 5));
        assert_eq!(title(false, 1), "New Contact Sheet");
        assert_eq!(title(false, 12), "New Contact Sheet from 12 Photos");
        assert_eq!(title(true, 12), "Making Contact Sheet…");
        let day = Date::since_reference_date(800_000_000.0);
        assert_eq!(stands(day).time_interval_since_reference_date(), 799_999_999.0);
        assert!(named(None, day).starts_with("2026-05-") && named(None, day).ends_with(" — contact sheet"));
    }
}
