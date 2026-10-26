//! What the info sidebar stands on: the anchor's roll, by folder marker or sidecar film, and the ringed project.

use super::*;

impl State {
    /// The stock the info sidebar's roll carries, picked by a hand or none.
    pub(in crate::app) fn roll_stock(&self) -> Option<String> {
        self.roll().and_then(|r| r.film.stock)
    }

    /// The roll a click or the anchor stands on: the folder's marker, else the roll every
    /// member's sidecar names; a folder of loose photos is no roll.
    pub(super) fn roll(&self) -> Option<Roll> {
        let anchor = self.picks.anchor.as_deref()?;
        let group = self.archive.as_ref()?.groups.iter().find(|g| g.entries.iter().any(|e| e.path == anchor))?;
        let paths: Vec<String> = group.entries.iter().map(|e| e.path.clone()).collect();
        let first = group.entries.first().cloned();
        if let Some(film) = &group.film {
            return Some(Roll { film: film.clone(), marker: Some(oni_library::paths::parent(anchor)), paths, first });
        }
        let film = group.entries.first()?.film.clone()?;
        group.entries.iter().all(|e| e.film.as_ref().is_some_and(|f| f.id == film.id)).then_some(Roll { film, marker: None, paths, first })
    }

    pub(super) fn project_group(&self) -> Option<project::ProjectGroup> {
        let ring = match &self.org.project_ring {
            Some(NodeId::Project(id)) => Some(*id),
            Some(NodeId::Photo { project, .. }) => Some(*project),
            None => None,
        };
        oni_library::info::resolved_project(&self.org.projects, &self.reach(), ring, self.picks.anchor.as_deref())
    }
}
