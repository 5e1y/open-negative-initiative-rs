//! How a folder is named on a row: the system's word for a standard folder, the parent beside a
//! favourite whose name another shares, and the end of a long name kept whole while the rest elides.

use super::Computer;
use crate::paths::{self, same};
use std::collections::HashMap;

/// The home's standard folders the Finder and the Explorer name in the reader's language.
#[cfg(not(windows))]
const STANDARD: [(&str, &str); 6] = [
    ("Desktop", oni_i18n::marked("Desktop")),
    ("Documents", oni_i18n::marked("Documents")),
    ("Downloads", oni_i18n::marked("Downloads")),
    ("Movies", oni_i18n::marked("Movies")),
    ("Music", oni_i18n::marked("Music")),
    ("Pictures", oni_i18n::marked("Pictures")),
];
#[cfg(windows)]
const STANDARD: [(&str, &str); 6] = [
    ("Desktop", oni_i18n::marked("Desktop")),
    ("Documents", oni_i18n::marked("Documents")),
    ("Downloads", oni_i18n::marked("Downloads")),
    ("Videos", oni_i18n::marked("Videos")),
    ("Music", oni_i18n::marked("Music")),
    ("Pictures", oni_i18n::marked("Pictures")),
];

/// A title cut in two: the end kept whole, so two names alike up to their last characters stay apart.
const KEPT_END: usize = 8;
/// Names this short are never cut: the elision would hide as much as it keeps.
const CUT_FROM: usize = 2 * KEPT_END;

impl Computer {
    /// A row's or the pill's title: a standard folder of the home in the reader's words, from the
    /// path alone; any other folder by its name on the disk.
    pub fn name(&self, path: &str) -> String {
        self.standard(path).map_or_else(|| paths::name(path), |word| oni_i18n::tr(word).to_owned())
    }

    /// The English word of a standard folder of the home, none for any other.
    fn standard(&self, path: &str) -> Option<&'static str> {
        if self.home.is_empty() || !same(&paths::parent(path), &self.home) {
            return None;
        }
        let name = paths::name(path);
        STANDARD.iter().find(|(disk, _)| *disk == name).map(|(_, word)| *word)
    }

    /// The home, a volume's top or the disk's root: opening one reads every folder of a disk.
    pub fn is_wide(&self, path: &str) -> bool {
        (!self.home.is_empty() && same(path, &self.home)) || self.is_volume_root(path) || paths::parent(path) == path
    }
}

/// For each folder whose name another of `folders` shares, the folders above it, nearest last,
/// as far up as tells them apart: "T7/" before one "Scans", "Pictures/" before the other.
pub fn trails(folders: &[String]) -> HashMap<String, String> {
    let mut named: HashMap<String, Vec<&String>> = HashMap::new();
    for folder in folders {
        named.entry(paths::name(folder)).or_default().push(folder);
    }
    let mut out = HashMap::new();
    for alike in named.into_values().filter(|a| a.len() > 1) {
        // Three folders up at most: past that the tooltip's whole path says it better.
        for up in 1..=3 {
            let trail = |f: &str| {
                let mut at = f.to_owned();
                let mut names = Vec::new();
                for _ in 0..up {
                    let parent = paths::parent(&at);
                    if parent == at {
                        break;
                    }
                    names.insert(0, paths::name(&parent));
                    at = parent;
                }
                names.into_iter().map(|n| format!("{n}/")).collect::<String>()
            };
            let trails: Vec<String> = alike.iter().map(|f| trail(f)).collect();
            let distinct = trails.iter().enumerate().all(|(i, t)| trails.iter().skip(i + 1).all(|u| u != t));
            if distinct || up == 3 {
                out.extend(alike.iter().map(|f| (*f).clone()).zip(trails));
                break;
            }
        }
    }
    out
}

/// `truncationMode(.middle)`: a long name's head, which elides, and its last characters, which
/// never do; a short one whole, with no end apart. A mark joining a letter stays with it.
pub fn split_end(name: &str) -> (String, String) {
    let chars: Vec<char> = name.chars().collect();
    if chars.len() < CUT_FROM {
        return (name.to_owned(), String::new());
    }
    let mut cut = chars.len() - KEPT_END;
    while cut > 0 && is_joining(chars[cut]) {
        cut -= 1;
    }
    (chars[..cut].iter().collect(), chars[cut..].iter().collect())
}

/// The combining marks a decomposed name carries after its letter, `É` as `E` and U+0301.
fn is_joining(c: char) -> bool {
    matches!(c, '\u{300}'..='\u{36F}' | '\u{1AB0}'..='\u{1AFF}' | '\u{1DC0}'..='\u{1DFF}' | '\u{20D0}'..='\u{20FF}' | '\u{FE20}'..='\u{FE2F}')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mac() -> Computer {
        Computer { home: "/Users/someone".into(), volumes: "/Volumes".into() }
    }

    #[test]
    fn a_standard_folder_of_the_home_takes_the_readers_word() {
        let c = mac();
        assert_eq!(c.standard("/Users/someone/Pictures"), Some("Pictures"));
        let french = include_str!("../../../oni-i18n/lang/fr/LC_MESSAGES/oni-ui.po");
        assert!(french.contains("msgid \"Pictures\"\nmsgstr \"Images\""), "the French reader reads Images");
        // Twin: the same name deeper down is a folder of the person's, named as on the disk.
        assert_eq!(c.standard("/Users/someone/Scans/Pictures"), None);
        assert_eq!(c.name("/Users/someone/Scans/Pictures"), "Pictures");
        assert!(c.is_wide("/Users/someone") && c.is_wide("/Volumes/T7") && c.is_wide("/"));
        assert!(!c.is_wide("/Users/someone/Scans") && !c.is_wide("/Volumes/T7/Scans"), "twin: a folder inside is no disk");
    }

    #[test]
    fn two_favourites_named_alike_show_what_tells_them_apart() {
        let folders: Vec<String> = ["/Users/someone/2024/Scans", "/Volumes/T7/2024/Scans", "/Volumes/T7/Film"].map(String::from).to_vec();
        let trails = trails(&folders);
        assert_eq!(trails.get("/Users/someone/2024/Scans").map(String::as_str), Some("someone/2024/"));
        assert_eq!(trails.get("/Volumes/T7/2024/Scans").map(String::as_str), Some("T7/2024/"));
        // Twin: a name no other shares carries no trail.
        assert!(!trails.contains_key("/Volumes/T7/Film"));
        // Twin: a trail one folder up would read the same for both.
        assert_eq!(paths::name(&paths::parent(&folders[0])), paths::name(&paths::parent(&folders[1])));
    }

    #[test]
    fn a_long_name_keeps_its_end_whole() {
        let (head, end) = split_end("2026-06-18 Lisbon Portra 400 roll 37");
        assert_eq!((head.as_str(), end.as_str()), ("2026-06-18 Lisbon Portra 400", " roll 37"));
        let other = split_end("2026-06-18 Lisbon Portra 400 roll 38");
        assert_ne!(end, other.1, "two rolls alike to their end differ in what stays");
        assert_eq!(split_end("Roll 12"), ("Roll 12".to_string(), String::new()), "twin: a short name is never cut");
        // A decomposed letter is never parted from its accent.
        let name = "abcdefghijklmnopqre\u{301}1234567";
        let (head, end) = split_end(name);
        assert_eq!((head.as_str(), end.as_str()), ("abcdefghijklmnopqr", "e\u{301}1234567"));
        assert_eq!(format!("{head}{end}"), name);
    }
}
