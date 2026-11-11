//! The camera bodies a hand gave the rolls the index holds, which the person's collection starts from.

use super::LibraryIndex;

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::Entry;
    use crate::scratch::Scratch;
    use oni_settings::{Date, Film};

    #[test]
    fn the_rolls_bodies_come_out_once_each_the_most_photos_first() {
        let scratch = Scratch::new("index-gear");
        let index = LibraryIndex::open(&scratch.path().join("index.sqlite")).unwrap();
        assert_eq!(index.camera_bodies().unwrap(), None, "an empty index answers nothing yet");
        let day = Date::since_reference_date(800_000_000.0);
        let roll = |body: Option<&str>| {
            let mut film = Film::new("Roll");
            film.camera_body = body.map(str::to_owned);
            film
        };
        for (i, body) in [Some("Leica M6"), Some("Nikon F3"), Some(" Nikon F3 "), None, Some("")].iter().enumerate() {
            let path = format!("/negs/{i}.RW2");
            index.upsert(&Entry::new(&path, day, &format!("f-{i}"))).unwrap();
            index.set_film(Some(&roll(*body)), &path).unwrap();
        }
        assert_eq!(index.camera_bodies().unwrap(), Some(vec!["Nikon F3".to_string(), "Leica M6".to_string()]));
        // Twin: a roll without a body names none, and the index still answers.
        index.set_film(Some(&roll(None)), "/negs/0.RW2").unwrap();
        assert_eq!(index.camera_bodies().unwrap(), Some(vec!["Nikon F3".to_string()]));
    }
}
