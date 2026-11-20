//! The camera bodies a hand gave the rolls the index holds, which the person's collection starts from,
//! and how many photos each body and each lens of the Kit took.

use super::LibraryIndex;
use std::collections::HashMap;

/// How many photos each body and each lens took, a name matched trimmed, its spaces as one, case aside.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GearCounts {
    bodies: HashMap<String, usize>,
    lenses: HashMap<String, usize>,
}

impl GearCounts {
    pub fn body(&self, name: &str) -> usize {
        self.bodies.get(&key(name)).copied().unwrap_or(0)
    }

    pub fn lens(&self, name: &str) -> usize {
        self.lenses.get(&key(name)).copied().unwrap_or(0)
    }
}

/// A name as the counts and the gallery's filters match it: trimmed, each run of spaces one, lower case.
pub fn key(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// One more photo for `name`, a name blank naming no gear.
fn counted(counts: &mut HashMap<String, usize>, name: Option<&str>) {
    if let Some(k) = name.map(key).filter(|k| !k.is_empty()) {
        *counts.entry(k).or_default() += 1;
    }
}

impl LibraryIndex {
    /// Each body a roll names, the one on the most photos first; `None` while the index holds no photo,
    /// so a first launch, before any folder is read, takes nothing in yet.
    pub fn camera_bodies(&self) -> rusqlite::Result<Option<Vec<String>>> {
        if self.count() == 0 {
            return Ok(None);
        }
        let mut statement = self.connection.prepare(
            "SELECT TRIM(film_camera_body) AS body, COUNT(*) AS photos FROM photos \
             WHERE film_camera_body IS NOT NULL AND TRIM(film_camera_body) != '' \
             GROUP BY body ORDER BY photos DESC, body",
        )?;
        let bodies = statement.query_map([], |row| row.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(Some(bodies))
    }

    /// The photos of each body and lens, the ONI Trash's, passes and contact sheets left out: a photo's body
    /// is the camera its EXIF line keeps, else its folder's roll's, else its sidecar's roll's. Reads no disk.
    pub fn gear_counts(&self) -> rusqlite::Result<GearCounts> {
        let mut statement = self.connection.prepare(
            "SELECT kept_camera, marker_body, film_camera_body, kept_lens, marker_lens, film_lens FROM photos \
             WHERE trashed_at IS NULL AND pass_of IS NULL AND sheet = 0",
        )?;
        let mut rows = statement.query([])?;
        let mut counts = GearCounts::default();
        while let Some(row) = rows.next()? {
            let text = |i: usize| row.get::<_, Option<String>>(i);
            let (kept, marker, sidecar) = (text(0)?, text(1)?, text(2)?);
            let (kept_lens, marker_lens, roll_lens) = (text(3)?, text(4)?, text(5)?);
            // A folder made a roll is the roll its band shows, over any roll its sidecars name.
            let body = kept.filter(|k| !k.trim().is_empty()).or(marker.or(sidecar));
            let lens = kept_lens.filter(|k| !k.trim().is_empty()).or(marker_lens.or(roll_lens));
            counted(&mut counts.bodies, body.as_deref());
            counted(&mut counts.lenses, lens.as_deref());
        }
        Ok(counts)
    }
}

#[cfg(test)]
mod tests;
