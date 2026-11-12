//! The roll draft a sidecar of format 21 and under keeps in each frame, gathered into its folder's
//! drafts: one a roll id, its shared fields from the first frame carrying it.

use crate::cg::Rect;
use crate::frame::{FlatField, PassOrder};
use crate::id::Id;
use crate::modes::ConversionMode;
use crate::prepare::{self, Base, Draft, Mark};
use crate::sidecar::Error;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A roll being prepared, as format 21 and under wrote it with each of its frames.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RollDraft {
    pub id: Id,
    pub step: String,
    pub film: ConversionMode,
    pub capture: String,
    #[serde(rename = "passOrder", skip_serializing_if = "Option::is_none", default)]
    pub pass_order: Option<PassOrder>,
    #[serde(rename = "passOffset", skip_serializing_if = "Option::is_none", default)]
    pub pass_offset: Option<usize>,
    #[serde(rename = "flatField", skip_serializing_if = "Option::is_none", default)]
    pub flat_field: Option<FlatField>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub base: Option<DraftBase>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub window: Option<Rect>,
    pub ticked: bool,
    pub flagged: bool,
}

/// The base pick as format 21 and under kept it: an absolute path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftBase {
    pub path: String,
    pub x: f64,
    pub y: f64,
    pub radius: f64,
}

/// The draft `name`'s sidecar kept, laid in its folder's file; a frame that file lists already keeps
/// the folder's word.
pub fn kept_in(folder: &Path, name: &str, stored: &RollDraft) -> Result<(), Error> {
    match placed(folder, name, stored) {
        Some(draft) => prepare::adopt(folder, draft),
        None => Ok(()),
    }
}

/// The draft `original`'s sidecar `bytes` of an earlier format carry, laid in its folder's file; a
/// newer build's file keeps its own word.
pub fn kept_from(original: &Path, bytes: &[u8]) -> Result<(), Error> {
    let (Some(folder), Some(name), Some(stored)) = (original.parent(), original.file_name().and_then(|n| n.to_str()), carried(bytes)) else { return Ok(()) };
    match kept_in(folder, name, &stored) {
        Err(Error::Newer) => Ok(()),
        other => other,
    }
}

/// The draft `name`'s sidecar keeps, as its folder holds it now.
fn placed(folder: &Path, name: &str, stored: &RollDraft) -> Option<Draft> {
    let mut draft = gathered(&folder.to_string_lossy(), [(name, stored)]).pop()?;
    if draft.base.is_none() {
        draft.base = renamed(folder, stored);
    }
    Some(draft)
}

/// A base whose folder was renamed since it was picked: the stored folder gone, its file in `folder`
/// a frame of the roll, and no folder beside it holding a frame of the roll by that name.
fn renamed(folder: &Path, stored: &RollDraft) -> Option<Base> {
    let b = stored.base.as_ref()?;
    let (was, file) = (parent(&b.path), last(&b.path));
    if was.is_empty() || Path::new(was).exists() || !carries(folder, file, stored) {
        return None;
    }
    let beside = folder.parent().and_then(|p| std::fs::read_dir(p).ok())?;
    let elsewhere = beside.flatten().map(|e| e.path()).any(|p| p.is_dir() && p != folder && carries(&p, file, stored));
    (!elsewhere).then(|| Base { file: file.to_owned(), x: b.x, y: b.y, radius: b.radius })
}

/// Whether `file` in `folder` is a frame of `stored`'s roll: by its sidecar's draft, or its folder's file.
fn carries(folder: &Path, file: &str, stored: &RollDraft) -> bool {
    let photo = folder.join(file);
    let by_sidecar = std::fs::read(crate::sidecar::url_for(&photo)).ok().and_then(|b| carried(&b)).is_some_and(|d| d.id == stored.id);
    photo.is_file() && (by_sidecar || prepare::read(folder).iter().any(|d| d.id == stored.id && d.frames.contains_key(file)))
}

/// The roll draft a sidecar of format 21 and under carries.
fn carried(bytes: &[u8]) -> Option<RollDraft> {
    #[derive(Deserialize)]
    struct Carried {
        #[serde(rename = "rollDraft")]
        roll_draft: RollDraft,
    }
    serde_json::from_slice::<Carried>(bytes).ok().map(|c| c.roll_draft)
}

/// `folder`'s drafts with those its sidecars of an earlier format still keep for `names`, read and
/// never written: a roll whose frames are not brought yet resumes all the same.
pub fn found(folder: &Path, names: &[String]) -> Vec<Draft> {
    let mut drafts = prepare::read(folder);
    for name in names {
        let Ok(bytes) = std::fs::read(crate::sidecar::url_for(&folder.join(name))) else { continue };
        if !matches!(super::standing(&bytes), super::Standing::Older(_)) {
            continue;
        }
        if let Some(draft) = carried(&bytes).and_then(|d| placed(folder, name, &d)) {
            prepare::laid(&mut drafts, draft);
        }
    }
    drafts
}

/// The drafts `frames` (each a file name and its stored draft) leave `folder`, in the order their ids
/// are met. A stored base stays only where its path's folder has `folder`'s name, never on a namesake.
pub fn gathered<'a>(folder: &str, frames: impl IntoIterator<Item = (&'a str, &'a RollDraft)>) -> Vec<Draft> {
    let mut drafts: Vec<Draft> = Vec::new();
    for (name, d) in frames {
        let mark = Mark { ticked: d.ticked, flagged: d.flagged };
        if let Some(draft) = drafts.iter_mut().find(|g| g.id == d.id) {
            draft.frames.insert(name.to_string(), mark);
            continue;
        }
        let base = d.base.as_ref().filter(|b| last(parent(&b.path)) == last(folder));
        drafts.push(Draft {
            id: d.id,
            step: d.step.clone(),
            film: d.film,
            capture: d.capture.clone(),
            pass_order: d.pass_order,
            pass_offset: d.pass_offset,
            flat_field: d.flat_field.clone(),
            base: base.map(|b| Base { file: last(&b.path).to_owned(), x: b.x, y: b.y, radius: b.radius }),
            window: d.window,
            frames: [(name.to_string(), mark)].into(),
        });
    }
    drafts
}

/// The last part of a path written on either system, a trailing separator ignored.
fn last(path: &str) -> &str {
    let path = path.trim_end_matches(['/', '\\']);
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn parent(path: &str) -> &str {
    path.rfind(['/', '\\']).map_or("", |at| &path[..at])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored(id: Id, base: &str, ticked: bool, flagged: bool) -> RollDraft {
        RollDraft {
            id,
            step: "flags".into(),
            film: ConversionMode::Negative,
            capture: "camera".into(),
            pass_order: None,
            pass_offset: None,
            flat_field: None,
            base: Some(DraftBase { path: base.into(), x: 0.1, y: 0.2, radius: 3.0 }),
            window: None,
            ticked,
            flagged,
        }
    }

    #[test]
    fn each_id_keeps_its_own_frames_and_the_base_stays_in_its_own_folder() {
        let (roll, stray) = (Id::new(), Id::new());
        let base = "C:\\Scans\\Roll 8\\B.NEF";
        let frames = [("A.NEF", stored(stray, base, true, false)), ("B.NEF", stored(roll, base, true, true)), ("C.NEF", stored(roll, base, false, false))];
        let drafts = gathered("E:/Roll 8", frames.iter().map(|(n, d)| (*n, d)));
        let ids: Vec<Id> = drafts.iter().map(|d| d.id).collect();
        assert_eq!(ids, [stray, roll], "each id its draft, none lost");
        let marks: Vec<(&str, Mark)> = drafts[1].frames.iter().map(|(n, m)| (n.as_str(), *m)).collect();
        assert_eq!(marks, [("B.NEF", Mark { ticked: true, flagged: true }), ("C.NEF", Mark { ticked: false, flagged: false })]);
        assert_eq!(drafts[1].base.as_ref().map(|b| b.file.as_str()), Some("B.NEF"), "a stored path reduced to its file name");
        // Twin: the roll's other folder, holding a namesake of the base, keeps no base.
        let elsewhere = gathered("/Volumes/Scans/Roll 9", frames[1..].iter().map(|(n, d)| (*n, d)));
        assert_eq!((elsewhere.len(), elsewhere[0].base.as_ref()), (1, None));
        assert!(gathered("/x", std::iter::empty()).is_empty());
    }

    /// `name` in `folder` with a sidecar of format 19 carrying `draft`.
    fn frame(folder: &Path, name: &str, draft: &RollDraft) {
        std::fs::create_dir_all(folder).unwrap();
        std::fs::write(folder.join(name), b"").unwrap();
        let old = serde_json::json!({ "version": 19, "settings": { "mode": "negative" }, "rollDraft": draft });
        std::fs::write(crate::sidecar::url_for(&folder.join(name)), serde_json::to_vec(&old).unwrap()).unwrap();
    }

    #[test]
    fn a_base_follows_its_folder_renamed_unless_a_folder_beside_holds_its_frame() {
        let dir = std::env::temp_dir().join(format!("oni-draft-renamed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let roll = Id::new();
        let draft = stored(roll, "/Volumes/Gone/Roll 8/B.NEF", true, false);
        let renamed = dir.join("one").join("Roll 8 Portra");
        frame(&renamed, "B.NEF", &draft);
        kept_in(&renamed, "B.NEF", &draft).unwrap();
        assert_eq!(prepare::read(&renamed)[0].base.as_ref().map(|b| b.file.as_str()), Some("B.NEF"), "the base kept after the rename");
        // Twin: a folder beside holding a frame of the roll by that name leaves the pick unsure: no base.
        let (here, beside) = (dir.join("two").join("Roll 8 Portra"), dir.join("two").join("Roll 8b"));
        frame(&here, "B.NEF", &draft);
        frame(&beside, "B.NEF", &draft);
        kept_in(&here, "B.NEF", &draft).unwrap();
        assert_eq!(prepare::read(&here)[0].base, None);
        // Twin: the stored folder still there keeps the base there, never here.
        let standing = dir.join("three").join("Roll 8");
        std::fs::create_dir_all(&standing).unwrap();
        let elsewhere = stored(roll, &standing.join("B.NEF").to_string_lossy(), true, false);
        let other = dir.join("three").join("Other");
        frame(&other, "B.NEF", &elsewhere);
        kept_in(&other, "B.NEF", &elsewhere).unwrap();
        assert_eq!(prepare::read(&other)[0].base, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_frame_not_brought_yet_resumes_its_roll_from_its_sidecar_unwritten() {
        let dir = std::env::temp_dir().join(format!("oni-draft-found-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let roll = Id::new();
        let draft = stored(roll, &dir.join("B.NEF").to_string_lossy(), true, true);
        frame(&dir, "A.NEF", &draft);
        let before = std::fs::read(crate::sidecar::url_for(&dir.join("A.NEF"))).unwrap();
        let found = found(&dir, &["A.NEF".to_string()]);
        assert_eq!(found.iter().map(|d| (d.id, d.frames.len())).collect::<Vec<_>>(), [(roll, 1)]);
        assert_eq!(std::fs::read(crate::sidecar::url_for(&dir.join("A.NEF"))).unwrap(), before, "nothing written");
        assert!(!prepare::path(&dir).exists());
        // Twin: the folder's file alone knows nothing of it yet.
        assert!(prepare::read(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

