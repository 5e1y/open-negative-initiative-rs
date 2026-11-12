use super::*;
use oni_settings::migrate::draft::{DraftBase, RollDraft};
use oni_settings::prepare::Mark;
use oni_settings::sidecar::WriteOptions;
use std::path::PathBuf;

fn roll(paths: &[String]) -> Roll {
    let frames: Vec<Frame> = paths.iter().map(|p| Frame::new(p, "print")).collect();
    Roll { shown: true, step: Step::Base, sources: frames.clone(), frames, ..Roll::default() }
}

fn folder_draft(base: &str, frames: &[&str]) -> Draft {
    Draft {
        id: Id::new(),
        step: "flags".into(),
        film: ConversionMode::Negative,
        capture: "camera".into(),
        pass_order: None,
        pass_offset: None,
        flat_field: None,
        base: Some(Base { file: base.into(), x: 0.1, y: 0.2, radius: 3.0 }),
        window: None,
        frames: frames.iter().map(|n| (n.to_string(), Mark { ticked: true, flagged: false })).collect(),
    }
}

#[test]
fn a_draft_base_is_found_in_its_own_folder_wherever_the_folder_went() {
    let draft = folder_draft("DSC_0002.NEF", &["DSC_0001.NEF", "DSC_0002.NEF"]);
    let known = passes::Known::new();
    for folder in ["/Volumes/T7/Roll 8 bis", "/Volumes/Other/Roll 8", "E:/Roll 8"] {
        let paths: Vec<String> = ["DSC_0001.NEF", "DSC_0002.NEF"].iter().map(|n| format!("{folder}/{n}")).collect();
        let mut r = roll(&paths);
        assert!(r.resume(Some(&draft), &paths[0], &known));
        assert_eq!(r.base.as_ref().map(|b| b.path.as_str()), Some(paths[1].as_str()), "{folder}");
    }
    // Two folders holding the name: the base is the one in the draft's folder, never ambiguous.
    let paths = ["/r/a/DSC_0002.NEF", "/r/b/DSC_0002.NEF"].map(String::from);
    let mut r = roll(&paths);
    r.resume(Some(&draft), &paths[1], &known);
    assert_eq!(r.base.map(|b| b.path), Some(paths[1].clone()));
    // Twin: a draft storing the absolute path a renamed folder left finds no base.
    let renamed = ["/Volumes/T7/Roll 8 bis/DSC_0001.NEF", "/Volumes/T7/Roll 8 bis/DSC_0002.NEF"].map(String::from);
    let mut r = roll(&renamed);
    let pathed = Draft { base: Some(Base { file: "/Volumes/T7/Roll 8/DSC_0002.NEF".into(), x: 0.1, y: 0.2, radius: 3.0 }), ..draft };
    r.resume(Some(&pathed), &renamed[0], &known);
    assert_eq!(r.base, None);
}

/// A draft of format 21 as a frame's sidecar kept it.
fn kept(id: Id, step: &str, base: Option<DraftBase>, flagged: bool) -> RollDraft {
    let film = ConversionMode::Negative;
    RollDraft { id, step: step.into(), film, capture: "camera".into(), pass_order: None, pass_offset: None, flat_field: None, base, window: None, ticked: true, flagged }
}

/// A folder of `n` stand-in frames, each with a sidecar, and their paths.
fn stood(name: &str, n: usize) -> (PathBuf, Vec<String>) {
    let dir = std::env::temp_dir().join(format!("oni-prepare-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut edited = PipelineSettings::rest(ConversionMode::Negative, true);
    edited.print.exposure = 0.3;
    let paths: Vec<String> = (1..=n).map(|i| dir.join(format!("DSC_{i:04}.NEF")).to_string_lossy().into_owned()).collect();
    for p in &paths {
        std::fs::write(p, b"").unwrap();
        sidecar::write(&edited, Path::new(p), WriteOptions::default()).unwrap();
    }
    (dir, paths)
}

/// `p`'s sidecar as format 21 wrote it, keeping `draft`.
fn stored_with(p: &str, draft: &RollDraft) {
    let path = sidecar::url_for(Path::new(p));
    let current = oni_settings::document::decode(&std::fs::read(&path).unwrap()).unwrap();
    let mut value = oni_settings::migrate::reshape::as_stored(&current, 21);
    value["rollDraft"] = serde_json::to_value(draft).unwrap();
    std::fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
}

/// Each frame read, as entering does, then the draft its folder resumes.
fn entered(paths: &[String]) -> Vec<Option<Draft>> {
    for p in paths {
        sidecar::read(Path::new(p), true).unwrap();
    }
    found(paths)
}

/// Every file of `dir`, its bytes and its date.
fn listed(dir: &Path) -> BTreeMap<String, (Vec<u8>, std::time::SystemTime)> {
    let entries = std::fs::read_dir(dir).unwrap().flatten();
    entries.map(|e| (e.file_name().to_string_lossy().into_owned(), (std::fs::read(e.path()).unwrap(), e.metadata().unwrap().modified().unwrap()))).collect()
}

fn changed(before: &BTreeMap<String, (Vec<u8>, std::time::SystemTime)>, after: &BTreeMap<String, (Vec<u8>, std::time::SystemTime)>) -> usize {
    after.iter().filter(|(n, now)| before.get(*n) != Some(*now)).count() + before.keys().filter(|n| !after.contains_key(*n)).count()
}

#[test]
fn a_step_over_thirty_eight_frames_writes_one_file_and_leaving_removes_it_alone() {
    let (dir, paths) = stood("one-file", 38);
    let mut r = roll(&paths);
    r.read = paths.iter().map(|p| (p.clone(), PipelineSettings::default())).collect();
    let before = listed(&dir);
    assert!(written(r.drafts().unwrap()).is_empty());
    let after = listed(&dir);
    assert_eq!(changed(&before, &after), 1, "one write for the step");
    let draft = prepare::read(&dir).pop().unwrap();
    assert_eq!((draft.frames.len(), draft.step.as_str()), (38, "base"));
    // Leaving removes that file and moves no sidecar.
    assert!(written(r.dropped()).is_empty());
    assert_eq!(listed(&dir), before);
    // Twin: a draft kept in each frame's sidecar writes every one of them.
    for p in &paths {
        sidecar::set_label(oni_settings::Medal::Gold, Path::new(p), true).unwrap();
    }
    assert_eq!(changed(&before, &listed(&dir)), 38);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_drafts_frames_kept_in_format_21_join_their_folders_file_and_resume_the_roll() {
    let (dir, paths) = stood("stored", 3);
    let (roll_id, stray) = (Id::new(), Id::new());
    let base = DraftBase { path: format!("/Volumes/Scans/{}/{}", paths::name(&paths::parent(&paths[1])), paths::name(&paths[1])), x: 0.4, y: 0.5, radius: 2.0 };
    for (p, id) in paths.iter().zip([stray, roll_id, roll_id]) {
        stored_with(p, &kept(id, "flags", Some(base.clone()), p == &paths[2]));
    }
    let found = entered(&paths);
    let ids = |found: &[Option<Draft>]| found.iter().map(|f| f.as_ref().map(|d| d.id)).collect::<Vec<_>>();
    assert_eq!(ids(&found), [None, Some(roll_id), Some(roll_id)], "the majority's draft, the stray's frame given none");
    let mut r = roll(&paths);
    for (p, f) in paths.iter().zip(&found) {
        r.resume(f.as_ref(), p, &passes::Known::new());
    }
    assert_eq!((r.step, r.base.as_ref().map(|b| b.path.as_str())), (Step::Flags, Some(paths[1].as_str())));
    let places: Vec<(bool, bool)> = r.frames.iter().map(|f| (f.ticked, f.flagged)).collect();
    assert_eq!(places, [(false, false), (true, false), (true, true)], "the stray's frame left out");
    // The frames' own sidecars keep no draft once read: the folder's file holds both rolls.
    assert!(paths.iter().all(|p| !std::fs::read_to_string(sidecar::url_for(Path::new(p))).unwrap().contains("rollDraft")));
    let mut held: Vec<Id> = prepare::read(&dir).iter().map(|d| d.id).collect();
    held.sort_by_key(|id| id.to_string());
    let mut both = vec![stray, roll_id];
    both.sort_by_key(|id| id.to_string());
    assert_eq!(held, both);
    // Twin: the next step's write takes the stray's frame into the roll, its own draft emptied away.
    r.read = paths.iter().map(|p| (p.clone(), PipelineSettings::default())).collect();
    assert!(written(r.drafts().unwrap()).is_empty());
    assert_eq!(prepare::read(&dir).iter().map(|d| d.id).collect::<Vec<_>>(), [roll_id]);
    assert_eq!(ids(&super::found(&paths)), [Some(roll_id); 3]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Each frame's resumed draft by its id and step, as `found` gives them on entering `paths`.
fn resuming(paths: &[String]) -> Vec<Option<(Id, String)>> {
    entered(paths).into_iter().map(|f| f.map(|d| (d.id, d.step))).collect()
}

fn walked(paths: &[String], step: Step) -> Roll {
    let mut r = roll(paths);
    r.step = step;
    r.read = paths.iter().map(|p| (p.clone(), PipelineSettings::default())).collect();
    r
}

#[test]
fn two_rolls_in_one_folder_resume_each_its_own_draft() {
    let (dir, paths) = stood("two-rolls", 4);
    let (a, b) = (&paths[..2], &paths[2..]);
    let mut ra = walked(a, Step::Base);
    assert!(written(ra.drafts().unwrap()).is_empty());
    assert_eq!(resuming(b), [None, None], "A's draft reaches no frame of B");
    let mut rb = walked(b, Step::Flags);
    assert!(written(rb.drafts().unwrap()).is_empty());
    let (ida, idb) = (ra.draft_id.unwrap(), rb.draft_id.unwrap());
    assert_eq!(resuming(a), [Some((ida, "base".into())), Some((ida, "base".into()))], "B's step leaves A's draft");
    assert_eq!(resuming(b), [Some((idb, "flags".into())), Some((idb, "flags".into()))]);
    // B left or measured: its draft goes, A's stays.
    assert!(written(rb.dropped()).is_empty());
    assert_eq!((resuming(a)[0].clone(), resuming(b)), (Some((ida, "base".into())), vec![None, None]));
    // Twin: a roll of A's frames and one of B's takes them, A keeping none and its draft gone.
    let mut rc = walked(&paths[..3], Step::Window);
    assert!(written(rc.drafts().unwrap()).is_empty());
    let idc = rc.draft_id.unwrap();
    assert_eq!(resuming(a), [Some((idc, "window".into())), Some((idc, "window".into()))]);
    assert_eq!(prepare::read(&dir).len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_base_stored_in_one_folder_stays_off_its_namesake_in_another() {
    let ((x, xs), (y, ys)) = (stood("base-x", 2), stood("base-y", 2));
    let id = Id::new();
    let in_folder = |dir: &Path| DraftBase { path: format!("/Volumes/Old/{}/DSC_0002.NEF", paths::name(&dir.to_string_lossy())), x: 0.4, y: 0.5, radius: 2.0 };
    // Y's frames land first, each folder holding a DSC_0002.
    let paths: Vec<String> = ys.iter().chain(&xs).cloned().collect();
    let base_after = |base: DraftBase| {
        for p in &paths {
            stored_with(p, &kept(id, "flags", Some(base.clone()), false));
            std::fs::write(prepare::path(Path::new(&paths::parent(p))), br#"{"version": 1, "drafts": []}"#).unwrap();
        }
        let mut r = roll(&paths);
        for (p, f) in paths.iter().zip(entered(&paths)) {
            r.resume(f.as_ref(), p, &passes::Known::new());
        }
        r.base.map(|b| b.path)
    };
    assert_eq!(base_after(in_folder(&x)), Some(xs[1].clone()), "on X's frame, never Y's namesake");
    // Twin: a base stored under a folder of neither name is found on no frame, never guessed.
    assert_eq!(base_after(in_folder(Path::new("/Volumes/Old/Roll 12"))), None);
    let _ = (std::fs::remove_dir_all(&x), std::fs::remove_dir_all(&y));
}
