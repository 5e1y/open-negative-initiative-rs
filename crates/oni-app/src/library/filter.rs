//! The gallery's filter, by medal, body and lens, and the bodies and lenses its lists offer.

use oni_library::Entry;
use oni_settings::Medal;

/// What the gallery shows: every photo, or those wearing one medal, shot on one body, with one lens, or
/// any of these at once; a body or a lens matched as the Kit's counts match it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Filter {
    pub medal: Option<Medal>,
    pub body: Option<String>,
    pub lens: Option<String>,
}

impl Filter {
    /// The photos wearing `medal` alone, whatever their gear.
    pub fn of_medal(medal: Medal) -> Filter {
        Filter { medal: Some(medal), ..Filter::default() }
    }

    pub fn admits(&self, entry: &Entry) -> bool {
        let named = |wanted: &Option<String>, has: Option<&str>| {
            wanted.as_deref().is_none_or(|w| has.is_some_and(|h| oni_library::index::gear_key(h) == oni_library::index::gear_key(w)))
        };
        self.medal.is_none_or(|m| entry.label == m) && named(&self.body, entry.body()) && named(&self.lens, entry.lens())
    }

    /// Whether it leaves any photo out.
    pub fn cuts(&self) -> bool {
        *self != Filter::default()
    }
}

/// The bodies, or the lenses, the photos among `rows` were shot with, the one on the most photos first, each
/// in the spelling first met; the ONI Trash's, the passes and the contact sheets left out.
pub fn gear_names(rows: &[Entry], lens: bool) -> Vec<String> {
    let mut found: Vec<(String, String, usize)> = Vec::new();
    for entry in rows.iter().filter(|e| e.trashed_at.is_none() && e.pass_of.is_none() && !e.sheet) {
        let Some(name) = (if lens { entry.lens() } else { entry.body() }) else { continue };
        let key = oni_library::index::gear_key(name);
        match found.iter_mut().find(|(k, _, _)| *k == key) {
            Some((_, _, n)) => *n += 1,
            None => found.push((key, name.trim().to_owned(), 1)),
        }
    }
    found.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.1.cmp(&b.1)));
    found.into_iter().map(|(_, name, _)| name).collect()
}
