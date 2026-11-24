//! What a gallery draws a heading for: a folder's photos under its roll's name or its own path.

use crate::index::Entry;
use crate::paths;
use oni_settings::{Date, Film};

#[derive(Debug, Clone, PartialEq)]
pub struct FilmGroup {
    pub folder: String,
    pub film: Option<Film>,
    pub entries: Vec<Entry>,
    pub heading: String,
}

impl FilmGroup {
    fn named(&self) -> Option<&str> {
        self.film.as_ref().map(|f| f.name.as_str()).filter(|n| !n.trim_matches(is_blank).is_empty())
    }

    /// A named roll heads with its name, a nameless one with its heading.
    pub fn display_title(&self) -> String {
        self.named().map_or_else(|| self.heading.clone(), str::to_owned)
    }

    pub fn trail(&self) -> Option<String> {
        self.heading.rfind('/').map(|cut| self.heading[..cut].to_owned())
    }

    pub fn heading_title(&self) -> String {
        self.named().map_or_else(|| paths::name(&self.heading), str::to_owned)
    }
}

/// `CharacterSet.whitespaces`: tabs and space separators, never line breaks.
fn is_blank(c: char) -> bool {
    c == '\t'
        || (c.is_whitespace() && !matches!(c, '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}'))
}

pub fn capture(entry: &Entry) -> Date {
    entry.captured_at.unwrap_or(entry.imported_at)
}
