//! The roll draft a sidecar of format 21 and under keeps in each frame, gathered into its folder's
//! drafts: one a roll id, its shared fields from the first frame carrying it.

use crate::prepare::{Base, Draft, Mark};
use crate::sidecar::RollDraft;

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
    use crate::modes::ConversionMode;
    use crate::sidecar::DraftBase;
    use crate::Id;

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
}
