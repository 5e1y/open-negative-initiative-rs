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

/// The folders whose file a read, a step or a migration holds, so no write of one drops another's.
static HELD: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());
static FREED: std::sync::Condvar = std::sync::Condvar::new();

/// A folder's file held from its read to its rename, let go when dropped; two spellings of one
/// folder are one file.
struct Held(PathBuf);

fn hold(folder: &Path) -> Held {
    let key = std::fs::canonicalize(folder).unwrap_or_else(|_| folder.to_path_buf());
    let mut held = HELD.lock().unwrap_or_else(|e| e.into_inner());
    while held.contains(&key) {
        held = FREED.wait(held).unwrap_or_else(|e| e.into_inner());
    }
    held.push(key.clone());
    Held(key)
}

impl Drop for Held {
    fn drop(&mut self) {
        let mut held = HELD.lock().unwrap_or_else(|e| e.into_inner());
        held.retain(|k| *k != self.0);
        drop(held);
        FREED.notify_all();
    }
}

/// The folder's drafts, none for an absent file; `Err` for a file of a newer version or one the disk
/// refuses. One that does not parse is none, and with `aside` moved beside as it is before a write.
fn kept(folder: &Path, aside: bool) -> Result<Vec<Draft>, Error> {
    let bytes = match std::fs::read(path(folder)) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(Error::Io(e)),
    };
    if serde_json::from_slice::<Version>(&bytes).is_ok_and(|v| v.version > VERSION) {
        return Err(Error::Newer);
    }
    match serde_json::from_slice::<Kept>(&bytes) {
        Ok(k) => Ok(k.drafts),
        Err(_) if aside => set_aside(folder).map(|_| Vec::new()).map_err(Error::Io),
        Err(_) => Ok(Vec::new()),
    }
}

/// The folder's file that does not parse moved whole to `.oni-prepare (unreadable).json`, numbered when
/// taken: a frame's own settings never wait on it, and nothing of it is lost.
fn set_aside(folder: &Path) -> io::Result<PathBuf> {
    for rank in 1..1000 {
        let name = if rank == 1 { ".oni-prepare (unreadable).json".to_owned() } else { format!(".oni-prepare (unreadable {rank}).json") };
        let target = folder.join(name);
        match std::fs::symlink_metadata(&target) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return std::fs::rename(path(folder), &target).map(|()| target),
            Err(e) => return Err(e),
            Ok(_) => {}
        }
    }
    Err(io::ErrorKind::AlreadyExists.into())
}

/// Every roll being prepared in `folder`; none when absent, unreadable or written by a newer version.
pub fn read(folder: &Path) -> Vec<Draft> {
    let _held = hold(folder);
    kept(folder, false).unwrap_or_default()
}

/// The `frames` of `folder` (file names) taken out of every draft it keeps, then `draft` laid in place
/// of its id's: a draft left with no frame goes, and the file with the last one.
pub fn write(folder: &Path, frames: &[String], draft: Option<&Draft>) -> Result<(), Error> {
    let _held = hold(folder);
    let mut drafts = kept(folder, true)?;
    let id = draft.map(|d| d.id);
    drafts.retain(|d| Some(d.id) != id);
    for d in &mut drafts {
        d.frames.retain(|name, _| !frames.contains(name) && draft.is_none_or(|mine| !mine.frames.contains_key(name)));
    }
    drafts.retain(|d| !d.frames.is_empty());
    drafts.extend(draft.filter(|d| !d.frames.is_empty()).cloned());
    stored(folder, drafts)
}

/// `draft` laid among the folder's drafts: its frames join its id's draft, or it stands as one of its
/// own; a frame another draft lists already stays there.
pub fn adopt(folder: &Path, draft: Draft) -> Result<(), Error> {
    let _held = hold(folder);
    let mut drafts = kept(folder, true)?;
    if laid(&mut drafts, draft) {
        stored(folder, drafts)?;
    }
    Ok(())
}

/// `draft` laid among `drafts` as `adopt` lays it; whether they changed.
pub fn laid(drafts: &mut Vec<Draft>, draft: Draft) -> bool {
    if drafts.iter().any(|d| d.frames.keys().any(|name| draft.frames.contains_key(name))) {
        return false;
    }
    match drafts.iter_mut().find(|d| d.id == draft.id) {
        Some(mine) => {
            mine.frames.extend(draft.frames);
            mine.base = mine.base.take().or(draft.base);
        }
        None => drafts.push(draft),
    }
    true
}

/// The folder's file holding `drafts`, removed with the last.
fn stored(folder: &Path, drafts: Vec<Draft>) -> Result<(), Error> {
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
    fn a_file_that_does_not_parse_is_set_aside_whole_and_frames_adopted_at_once_are_all_kept() {
        let dir = folder("held");
        let broken = b"{\"version\": 1, \"drafts\": [ trunc";
        std::fs::write(path(&dir), broken).unwrap();
        assert_eq!(read(&dir), [], "it reads as none");
        assert_eq!(std::fs::read(path(&dir)).unwrap(), broken, "a read moves nothing");
        let d = draft(&["A.NEF"]);
        adopt(&dir, d.clone()).unwrap();
        assert_eq!(read(&dir), std::slice::from_ref(&d), "the frame's draft kept, never blocked by the broken file");
        assert_eq!(std::fs::read(dir.join(".oni-prepare (unreadable).json")).unwrap(), broken, "set aside whole");
        // Twin: a second broken file goes beside the first, never over it.
        std::fs::write(path(&dir), broken).unwrap();
        write(&dir, &names(&d), Some(&d)).unwrap();
        assert_eq!(std::fs::read(dir.join(".oni-prepare (unreadable 2).json")).unwrap(), broken);
        std::fs::remove_file(path(&dir)).unwrap();
        // Many frames of one roll adopted from many threads at once: each frame kept, none dropped.
        let roll = draft(&["A.NEF"]);
        let threads: Vec<_> = (0..8)
            .map(|t| {
                let (dir, roll) = (dir.clone(), roll.clone());
                std::thread::spawn(move || {
                    for i in 0..25 {
                        let name = format!("F{t}-{i}.NEF");
                        adopt(&dir, Draft { frames: [(name, Mark::default())].into(), ..roll.clone() }).unwrap();
                    }
                })
            })
            .collect();
        threads.into_iter().for_each(|t| t.join().unwrap());
        assert_eq!(read(&dir).iter().map(|d| d.frames.len()).sum::<usize>(), 200);
        // Twin: the same adoptions laid on a list read once each, as two writers do, lose frames.
        let (mut first, mut second) = (Vec::new(), Vec::new());
        laid(&mut first, Draft { frames: [("X.NEF".to_string(), Mark::default())].into(), ..roll.clone() });
        laid(&mut second, Draft { frames: [("Y.NEF".to_string(), Mark::default())].into(), ..roll.clone() });
        assert_eq!(second[0].frames.len(), 1, "the second writer's list knows nothing of the first's frame");
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
