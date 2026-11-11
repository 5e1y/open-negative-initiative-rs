//! The person's gear: the bodies and the lenses typed anywhere in ONI, kept in the support folder's
//! `collection.json`, the last used first, and offered back where an export's EXIF is filled. A scan's
//! own metadata names the camera that digitised the film, never the one that took it, so only a hand adds.

use oni_settings::json;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// What a piece of gear is: a camera body or a lens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Gear {
    Body,
    Lens,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Collection {
    #[serde(default)]
    pub bodies: Vec<String>,
    #[serde(default)]
    pub lenses: Vec<String>,
    /// Whether the bodies a hand gave the rolls were taken in, once, so one forgotten never comes back.
    #[serde(default)]
    pub seeded: bool,
}

impl Collection {
    pub fn file_path(support: &Path) -> PathBuf {
        support.join("collection.json")
    }

    /// Empty while the file was never written or cannot be read: a hand fills it again.
    pub fn load(support: &Path) -> Collection {
        crate::read_json::<Collection>(&Self::file_path(support)).unwrap_or_default()
    }

    pub fn save(&self, support: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(support)?;
        let node = json::to_node(self).map_err(std::io::Error::other)?;
        crate::replace_readable(&Self::file_path(support), json::compact(&node).as_bytes())
    }

    pub fn list(&self, gear: Gear) -> &[String] {
        match gear {
            Gear::Body => &self.bodies,
            Gear::Lens => &self.lenses,
        }
    }

    fn list_mut(&mut self, gear: Gear) -> &mut Vec<String> {
        match gear {
            Gear::Body => &mut self.bodies,
            Gear::Lens => &mut self.lenses,
        }
    }

    /// A name used, its spaces trimmed, first in its list; one spelt alike but for its case is the same
    /// piece, now spelt as last typed. Whether the list changed.
    pub fn note(&mut self, gear: Gear, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty() {
            return false;
        }
        let list = self.list_mut(gear);
        if list.first().is_some_and(|first| first == name) {
            return false;
        }
        list.retain(|kept| kept.to_lowercase() != name.to_lowercase());
        list.insert(0, name.to_string());
        true
    }

    /// Names known elsewhere, each not kept yet added after the kept ones, in their order. Whether the
    /// list changed.
    pub fn take_in(&mut self, gear: Gear, names: &[String]) -> bool {
        let list = self.list_mut(gear);
        let before = list.len();
        for name in names.iter().map(|n| n.trim()).filter(|n| !n.is_empty()) {
            if !list.iter().any(|kept| kept.to_lowercase() == name.to_lowercase()) {
                list.push(name.to_string());
            }
        }
        list.len() != before
    }

    /// The piece so named gone from its list, its case aside. Whether it was there.
    pub fn forget(&mut self, gear: Gear, name: &str) -> bool {
        let list = self.list_mut(gear);
        let before = list.len();
        list.retain(|kept| kept.to_lowercase() != name.trim().to_lowercase());
        list.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_used_comes_first_once_whatever_its_case() {
        let mut c = Collection::default();
        assert!(c.note(Gear::Body, "Leica M6"));
        assert!(c.note(Gear::Body, " Nikon FM2 "));
        assert_eq!(c.bodies, ["Nikon FM2", "Leica M6"], "the last used first, trimmed");
        assert!(c.note(Gear::Body, "leica m6"));
        assert_eq!(c.bodies, ["leica m6", "Nikon FM2"], "one piece, spelt as last typed, first again");
        assert!(!c.note(Gear::Body, "leica m6"), "the first again changes nothing");
        assert!(!c.note(Gear::Body, "   "), "an empty name is no piece");
        // Twin: a lens is kept apart from the bodies, and a name merely alike stays its own piece.
        assert!(c.note(Gear::Lens, "Summicron 50"));
        assert!(c.note(Gear::Body, "Leica M6 TTL"));
        assert_eq!((c.bodies.len(), c.lenses.as_slice()), (3, ["Summicron 50".to_string()].as_slice()));
    }

    #[test]
    fn names_taken_in_follow_the_kept_ones_and_never_twice() {
        let mut c = Collection::default();
        c.note(Gear::Body, "Leica M6");
        assert!(c.take_in(Gear::Body, &["leica m6".into(), "Nikon F3".into(), " ".into(), "Pentax MX".into()]));
        assert_eq!(c.bodies, ["Leica M6", "Nikon F3", "Pentax MX"], "the kept first, a name alike but for case once");
        // Twin: taking in what is kept changes nothing.
        assert!(!c.take_in(Gear::Body, &["Nikon F3".into()]));
    }

    #[test]
    fn a_piece_forgotten_leaves_its_list_alone() {
        let mut c = Collection::default();
        c.note(Gear::Lens, "Summicron 50");
        c.note(Gear::Lens, "Elmarit 28");
        assert!(c.forget(Gear::Lens, "summicron 50"));
        assert_eq!(c.lenses, ["Elmarit 28"]);
        // Twin: forgetting what is not there says so and changes nothing.
        assert!(!c.forget(Gear::Lens, "Summicron 50"));
        assert!(!c.forget(Gear::Body, "Elmarit 28"));
        assert_eq!(c.lenses, ["Elmarit 28"]);
    }

    #[test]
    fn the_collection_reads_back_as_written_and_an_absent_file_is_empty() {
        let support = std::env::temp_dir().join(format!("oni-collection-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&support);
        assert_eq!(Collection::load(&support), Collection::default(), "no file, nothing kept");
        let mut c = Collection::default();
        c.note(Gear::Body, "Leica M6");
        c.note(Gear::Lens, "Summicron 50");
        c.save(&support).unwrap();
        assert_eq!(Collection::load(&support), c);
        // Twin: a file another build wrote without lenses still reads, its bodies kept.
        std::fs::write(Collection::file_path(&support), br#"{"bodies":["Nikon F3"]}"#).unwrap();
        assert_eq!(Collection::load(&support).bodies, ["Nikon F3"]);
        assert!(Collection::load(&support).lenses.is_empty());
        let _ = std::fs::remove_dir_all(&support);
    }
}
