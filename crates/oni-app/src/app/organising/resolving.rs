//! What the info sidebar and the roll gestures stand on: the roll the gallery shows, by folder marker
//! or sidecar film, and the ringed project.

use super::*;
use oni_settings::blocks::SeparationFamily;
use oni_settings::film::{FilmStockKind, dye_family};

impl Roll {
    /// The roll as the sidebar keys its fields: its folder, else its film, as a folder's film is
    /// only made by the first field written.
    pub(super) fn key(&self) -> String {
        match &self.marker {
            Some(folder) => format!("folder {folder}"),
            None => format!("film {}", self.film.id),
        }
    }
}

impl State {
    /// The stock the info sidebar's roll carries, picked by a hand or none.
    pub(in crate::app) fn roll_stock(&self) -> Option<String> {
        self.roll().and_then(|r| r.film.stock)
    }

    /// The stock of the open photo's roll: its folder's marker, else the roll its sidecar names.
    pub(in crate::app) fn open_stock(&self) -> Option<&'static FilmStockKind> {
        let folder = oni_library::paths::parent(self.open_path()?);
        let rolls = &self.survey().rolls;
        let marker = rolls.get(&folder).or_else(|| rolls.iter().find(|(f, _)| oni_library::paths::same(f, &folder)).map(|(_, film)| film));
        film_stock(marker.or(self.store.settings().film.as_ref())?.stock.as_deref())
    }

    /// The dyes the open photo's roll stock names, which its Dye Separation rests on.
    pub(in crate::app) fn open_dyes(&self) -> Option<SeparationFamily> {
        self.open_stock().and_then(|k| dye_family(k.id))
    }

    /// The archive's band the gallery stands on: the folder of the anchor shown, else the opened
    /// folder; the whole roll, as the medal filter keeps the gallery and never the roll.
    fn shown_band(&self) -> Option<library::Group> {
        if self.org.projects_surface || self.trash_shown() {
            return None;
        }
        let archive = self.archive.as_ref()?;
        let anchored = self.picks.anchor.as_deref().filter(|a| self.order.iter().any(|p| p == a));
        anchored.and_then(|a| archive.band(&oni_library::paths::parent(a))).or_else(|| archive.band(self.shown()?))
    }

    /// The roll the gallery stands on. A folder is one: its marker, else the roll every member's
    /// sidecar names, else a roll its first field written makes, named after the folder.
    pub(super) fn roll(&self) -> Option<Roll> {
        let group = self.shown_band()?;
        let paths: Vec<String> = group.entries.iter().map(|e| e.path.clone()).collect();
        let first = group.entries.first().cloned();
        let folder = group.folder.clone();
        if let Some(film) = group.film {
            return Some(Roll { film, marker: folder, paths, first });
        }
        let named = group.entries.first().and_then(|e| e.film.clone()).filter(|film| group.entries.iter().all(|e| e.film.as_ref().is_some_and(|f| f.id == film.id)));
        if let Some(film) = named {
            return Some(Roll { film, marker: None, paths, first });
        }
        let folder = folder?;
        Some(Roll { film: Film::new(&oni_library::paths::name(&folder)), marker: Some(folder), paths, first })
    }

    /// The photos a roll gesture acts on: the targets, else in the gallery the roll it stands on.
    pub(in crate::app) fn roll_targets(&self, gallery: bool) -> Vec<String> {
        let targets = self.targets_in(gallery);
        if !targets.is_empty() || !gallery {
            return targets;
        }
        self.shown_band().map(|g| g.entries.iter().map(|e| e.path.clone()).collect()).unwrap_or_default()
    }

    /// The photo the editor opens from the gallery: the anchor while shown, else the first photo
    /// shown, else the one open; never a photo of the ONI Trash.
    pub(in crate::app) fn editor_subject(&self) -> Option<String> {
        let shown = |p: &String| self.order.contains(p);
        let gallery = (!self.trash_shown()).then(|| self.picks.anchor.clone().filter(shown).or_else(|| self.order.first().cloned()));
        gallery.flatten().or_else(|| self.open_path().map(str::to_string))
    }

    pub(super) fn project_group(&self) -> Option<project::ProjectGroup> {
        let ring = match &self.org.project_ring {
            Some(NodeId::Project(id)) => Some(*id),
            Some(NodeId::Roll { project, .. } | NodeId::Photo { project, .. }) => Some(*project),
            None => None,
        };
        let rows = self.reach();
        oni_library::info::resolved_project(&self.org.projects, &project::Reach::new(&rows, &self.org.found), ring, self.picks.anchor.as_deref())
    }
}
