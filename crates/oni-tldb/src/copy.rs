//! The Kit as last read, kept in the support folder's `kit.json`: ONI works from it offline and asks the
//! service only at sign-in, at launch and after a change.

use crate::piece::{Kind, Piece};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Kept {
    /// The account's display name, shown in the column and never logged.
    #[serde(default)]
    pub account: Option<String>,
    #[serde(default)]
    pub items: Vec<Piece>,
    #[serde(default)]
    pub etag: Option<String>,
}

impl Kept {
    pub fn file_path(support: &Path) -> PathBuf {
        support.join("kit.json")
    }

    /// Empty when never written or unreadable.
    pub fn load(support: &Path) -> Kept {
        oni_library::read_json(&Kept::file_path(support)).unwrap_or_default()
    }

    pub fn save(&self, support: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(support)?;
        let bytes = serde_json::to_vec(self).map_err(std::io::Error::other)?;
        oni_library::replace_readable(&Kept::file_path(support), &bytes)
    }

    /// The pieces of one kind, in the Kit's order.
    pub fn of(&self, kind: Kind) -> impl Iterator<Item = &Piece> {
        self.items.iter().filter(move |p| p.kind == kind)
    }

    /// The piece of that kind a typed name stands for: its full name or its model, case and spaces aside.
    pub fn named(&self, kind: Kind, name: &str) -> Option<&Piece> {
        let wanted = plain(name);
        if wanted.is_empty() {
            return None;
        }
        self.of(kind).find(|p| plain(&p.name) == wanted).or_else(|| self.of(kind).find(|p| plain(&p.model) == wanted))
    }
}

/// Lower case, with every run of spaces one space and none at the ends.
fn plain(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}
