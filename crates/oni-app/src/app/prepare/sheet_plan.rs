//! The measure's contact sheet: what it is named after, and the former sheet it may replace.

use super::*;

/// What the measure's sheet is named after and may replace, read off the rows on this thread.
pub(super) struct SheetPlan {
    film: Option<oni_settings::Film>,
    earliest: oni_settings::Date,
    /// The gallery group of the first frame: a sheet of the roll's former reference there is rewritten.
    folder: Vec<String>,
    /// A slide roll's: the sheet of its folder's slides there is rewritten too.
    slides: bool,
}

impl SheetPlan {
    pub(super) fn of(rows: &[oni_library::Entry], archive: Option<&crate::library::Archive>, picked: &[Frame], slides: bool) -> SheetPlan {
        let known: Vec<&oni_library::Entry> = picked.iter().filter_map(|f| rows.iter().find(|e| e.path == f.path)).collect();
        let film = known.iter().find_map(|e| e.film.clone());
        let earliest =
            known.iter().map(|e| oni_library::film_group::capture(e)).reduce(|a, b| if b < a { b } else { a }).unwrap_or_else(oni_settings::Date::now);
        let first = picked.first().map(|f| f.path.as_str());
        let folder = archive
            .and_then(|a| a.groups.iter().find(|g| g.entries.iter().any(|e| Some(e.path.as_str()) == first)))
            .map(|g| g.entries.iter().map(|e| e.path.clone()).collect())
            .unwrap_or_default();
        SheetPlan { film, earliest, folder, slides }
    }

    /// The sheet laid from the measure's tiles and written over the roll's former sheet, else beside
    /// the first frame; `None` when nothing lays or the file cannot be written.
    pub(super) fn written<'a>(
        &self,
        paths: &[String],
        befores: impl Iterator<Item = &'a PipelineSettings>,
        tiles: &[(String, oni_measure::sheet::Picture)],
        reads: &[Option<Rect>],
    ) -> Option<Sheet> {
        let (layout, pixels) = sheets::laid(tiles, paths.first()?, oni_measure::sheet::roll_tile(), reads)?;
        let former: HashSet<Id> = befores.filter_map(|b| b.roll_reference.as_ref().map(|r| r.id)).collect();
        let replaced = self.folder.iter().find(|p| {
            !paths.contains(p) && (roll_membership::sheet_reference(p).is_some_and(|id| former.contains(&id)) || (self.slides && roll_membership::is_slide_sheet(p)))
        });
        let stem = sheets::named(self.film.as_ref(), self.earliest);
        let to = match replaced {
            Some(p) => sheets::To::Over(Path::new(p)),
            None => sheets::To::Beside { lead: Path::new(&paths[0]), stem: &stem },
        };
        let destination = sheets::write(&layout, &pixels, to).ok()?;
        let mosaic = tiles.iter().all(|(p, _)| oni_decode::decodes_via_mosaic(Path::new(p)));
        Some(Sheet { path: destination, grid: layout.grid, film: self.film.clone(), stands: sheets::stands(self.earliest), mosaic })
    }
}
