//! The rolls being prepared, kept in one file in each of their frames' folders so a preparation left
//! halfway resumes: one write a step, each roll by its id, its base named by file, which travels with its folder.

use crate::atomic;
use crate::cg::Rect;
use crate::frame::{FlatField, PassOrder};
use crate::id::Id;
use crate::modes::ConversionMode;
use crate::sidecar::Error;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

pub const FILE: &str = ".oni-prepare.json";
/// Moved at every change of the file's shape; a newer file reads as none and is never rewritten.
pub const VERSION: i64 = 1;

/// The folder's file: a frame belongs to one draft at most, a draft holds one frame at least.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Kept {
    version: i64,
    drafts: Vec<Draft>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Draft {
    pub id: Id,
    /// The step reached, by its name, so a step added later moves no stored one.
    pub step: String,
    pub film: ConversionMode,
    pub capture: String,
    #[serde(rename = "passOrder", skip_serializing_if = "Option::is_none", default)]
    pub pass_order: Option<PassOrder>,
    /// The files skipped before the first triplet, so a roll shifted by a file resumes on its own leads.
    #[serde(rename = "passOffset", skip_serializing_if = "Option::is_none", default)]
    pub pass_offset: Option<usize>,
    #[serde(rename = "flatField", skip_serializing_if = "Option::is_none", default)]
    pub flat_field: Option<FlatField>,
    /// In the folder that holds the base frame only.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub base: Option<Base>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub window: Option<Rect>,
    /// Each frame of this folder in the roll, by file name; a file absent is no frame of it.
    #[serde(default)]
    pub frames: BTreeMap<String, Mark>,
}

/// The base pick: a frame of the same folder by its file name, never a path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Base {
    pub file: String,
    pub x: f64,
    pub y: f64,
    pub radius: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Mark {
    pub ticked: bool,
    pub flagged: bool,
}

pub fn path(folder: &Path) -> PathBuf {
    folder.join(FILE)
}

#[derive(Deserialize)]
struct Version {
    version: i64,
}

/// The folder's drafts; `Err` for a file of a newer version, none for an absent or unreadable one.
fn kept(folder: &Path) -> Result<Vec<Draft>, Error> {
    let Ok(bytes) = std::fs::read(path(folder)) else { return Ok(Vec::new()) };
    if serde_json::from_slice::<Version>(&bytes).is_ok_and(|v| v.version > VERSION) {
        return Err(Error::Newer);
    }
    Ok(serde_json::from_slice::<Kept>(&bytes).map(|k| k.drafts).unwrap_or_default())
}

/// Every roll being prepared in `folder`; none when absent, unreadable or written by a newer version.
pub fn read(folder: &Path) -> Vec<Draft> {
    kept(folder).unwrap_or_default()
}

/// The `frames` of `folder` (file names) taken out of every draft it keeps, then `draft` laid in place
/// of its id's: a draft left with no frame goes, and the file with the last one.
pub fn write(folder: &Path, frames: &[String], draft: Option<&Draft>) -> Result<(), Error> {
    let mut drafts = kept(folder)?;
    let id = draft.map(|d| d.id);
    drafts.retain(|d| Some(d.id) != id);
    for d in &mut drafts {
        d.frames.retain(|name, _| !frames.contains(name) && draft.is_none_or(|mine| !mine.frames.contains_key(name)));
    }
    drafts.retain(|d| !d.frames.is_empty());
    drafts.extend(draft.filter(|d| !d.frames.is_empty()).cloned());
    let at = path(folder);
    if drafts.is_empty() {
        return match std::fs::remove_file(&at) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(Error::Io(e)),
            _ => Ok(()),
        };
    }
    let mut text = serde_json::to_vec_pretty(&Kept { version: VERSION, drafts }).map_err(|e| Error::Io(io::Error::other(e)))?;
    text.push(b'\n');
    atomic::write_atomically(&at, &text).map_err(Error::Io)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(names: &[&str]) -> Draft {
        Draft {
            id: Id::new(),
            step: "base".into(),
            film: ConversionMode::Negative,
            capture: "camera".into(),
            pass_order: Some(PassOrder::Bgr),
            pass_offset: Some(0),
            flat_field: None,
            base: Some(Base { file: names[0].into(), x: 0.2, y: 0.3, radius: 4.0 }),
            window: Some(Rect::new(0.1, 0.1, 0.8, 0.8)),
            frames: names.iter().map(|n| (n.to_string(), Mark { ticked: true, flagged: false })).collect(),
        }
    }

    fn names(of: &Draft) -> Vec<String> {
        of.frames.keys().cloned().collect()
    }

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("oni-prepare-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_draft_reads_back_as_written_and_its_removal_takes_the_file_with_the_last() {
        let dir = folder("one");
        assert_eq!(read(&dir), []);
        let d = draft(&["A.NEF", "B.NEF"]);
        write(&dir, &names(&d), Some(&d)).unwrap();
        assert_eq!(read(&dir), std::slice::from_ref(&d));
        write(&dir, &names(&d), None).unwrap();
        assert!(!path(&dir).exists());
        write(&dir, &names(&d), None).unwrap();
        // Twin: a newer file reads as none and is never rewritten.
        let newer = br#"{"version": 2, "drafts": [], "more": true}"#;
        std::fs::write(path(&dir), newer).unwrap();
        assert_eq!(read(&dir), []);
        assert!(matches!(write(&dir, &names(&d), Some(&d)), Err(Error::Newer)));
        assert!(matches!(write(&dir, &names(&d), None), Err(Error::Newer)));
        assert_eq!(std::fs::read(path(&dir)).unwrap(), newer);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_rolls_in_one_folder_keep_each_its_draft() {
        let dir = folder("two");
        let (a, b) = (draft(&["A1.NEF", "A2.NEF"]), draft(&["B1.NEF", "B2.NEF"]));
        write(&dir, &names(&a), Some(&a)).unwrap();
        write(&dir, &names(&b), Some(&b)).unwrap();
        assert_eq!(read(&dir), [a.clone(), b.clone()]);
        // B's next step replaces B's alone; B left removes B's alone.
        let later = Draft { step: "flags".into(), ..b.clone() };
        write(&dir, &names(&b), Some(&later)).unwrap();
        assert_eq!(read(&dir), [a.clone(), later]);
        write(&dir, &names(&b), None).unwrap();
        assert_eq!(read(&dir), std::slice::from_ref(&a));
        // Twin: a roll taking A's frames takes them out of A's draft, which goes once empty.
        let c = draft(&["A1.NEF", "A2.NEF", "B1.NEF"]);
        write(&dir, &names(&c), Some(&c)).unwrap();
        assert_eq!(read(&dir), std::slice::from_ref(&c));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
