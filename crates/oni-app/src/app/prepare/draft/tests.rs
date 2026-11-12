use super::*;
use oni_settings::prepare::Mark;
use oni_settings::sidecar::{DraftBase, RollDraft, WriteOptions};
use std::fs::File;
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

/// A draft of format 21 as a frame's sidecar keeps it.
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
    assert!(written(r.drafts().unwrap(), &[]).is_empty());
    let after = listed(&dir);
    assert_eq!(changed(&before, &after), 1, "one write for the step");
    let draft = prepare::read(&dir).pop().unwrap();
    assert_eq!((draft.frames.len(), draft.step.as_str()), (38, "base"));
    // Leaving removes that file and moves no sidecar.
    assert!(written(r.dropped(), &[]).is_empty());
    assert_eq!(listed(&dir), before);
    // Twin: the per-frame draft writes every sidecar.
    let id = Id::new();
    for p in &paths {
        sidecar::set_roll_draft(Some(kept(id, "base", None, false)), Path::new(p), true).unwrap();
    }
    assert_eq!(changed(&before, &listed(&dir)), 38);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_drafts_frames_keep_from_format_21_resume_the_roll_and_go_at_its_next_write() {
    let (dir, paths) = stood("stored", 3);
    let (roll_id, stray) = (Id::new(), Id::new());
    let base = DraftBase { path: format!("/Volumes/Scans/{}/{}", paths::name(&paths::parent(&paths[1])), paths::name(&paths[1])), x: 0.4, y: 0.5, radius: 2.0 };
    for (p, id) in paths.iter().zip([stray, roll_id, roll_id]) {
        sidecar::set_roll_draft(Some(kept(id, "flags", Some(base.clone()), p == &paths[2])), Path::new(p), true).unwrap();
    }
    let loaded: Vec<_> = paths.iter().map(|p| sidecar::read(Path::new(p), true, false).ok()).collect();
    let found = found(&paths, &loaded);
    let ids = |found: &[Found]| found.iter().map(|f| f.draft.as_ref().map(|d| d.id)).collect::<Vec<_>>();
    assert!(found.iter().all(|f| f.stored));
    assert_eq!(ids(&found), [None, Some(roll_id), Some(roll_id)], "the majority's draft, the stray's frame given none");
    let mut r = roll(&paths);
    for (p, f) in paths.iter().zip(&found) {
        r.resume(f.draft.as_ref(), p, &passes::Known::new());
    }
    assert_eq!((r.step, r.base.as_ref().map(|b| b.path.as_str())), (Step::Flags, Some(paths[1].as_str())));
    let places: Vec<(bool, bool)> = r.frames.iter().map(|f| (f.ticked, f.flagged)).collect();
    assert_eq!(places, [(false, false), (true, false), (true, true)], "the stray's frame left out");
    // The next step's write lays the folder's file and clears every frame's own.
    r.read = paths.iter().map(|p| (p.clone(), PipelineSettings::default())).collect();
    assert!(written(r.drafts().unwrap(), &paths).is_empty());
    assert_eq!(prepare::read(&dir).iter().map(|d| d.id).collect::<Vec<_>>(), [roll_id]);
    assert!(paths.iter().all(|p| sidecar::read(Path::new(p), true, false).unwrap().roll_draft.is_none()));
    // Twin: entered again, the folder's file alone resumes the roll, the frames keeping none.
    let loaded: Vec<_> = paths.iter().map(|p| sidecar::read(Path::new(p), true, false).ok()).collect();
    let again = super::found(&paths, &loaded);
    assert!(again.iter().all(|f| !f.stored));
    assert_eq!(ids(&again), [Some(roll_id); 3], "the stray's frame now the roll's, left out");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Each frame's resumed draft by its id and step, as `found` gives them on entering `paths`.
fn resuming(paths: &[String]) -> Vec<Option<(Id, String)>> {
    let loaded: Vec<_> = paths.iter().map(|p| sidecar::read(Path::new(p), true, false).ok()).collect();
    found(paths, &loaded).into_iter().map(|f| f.draft.map(|d| (d.id, d.step))).collect()
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
    assert!(written(ra.drafts().unwrap(), &[]).is_empty());
    assert_eq!(resuming(b), [None, None], "A's draft reaches no frame of B");
    let mut rb = walked(b, Step::Flags);
    assert!(written(rb.drafts().unwrap(), &[]).is_empty());
    let (ida, idb) = (ra.draft_id.unwrap(), rb.draft_id.unwrap());
    assert_eq!(resuming(a), [Some((ida, "base".into())), Some((ida, "base".into()))], "B's step leaves A's draft");
    assert_eq!(resuming(b), [Some((idb, "flags".into())), Some((idb, "flags".into()))]);
    // B left or measured: its draft goes, A's stays.
    assert!(written(rb.dropped(), &[]).is_empty());
    assert_eq!((resuming(a)[0].clone(), resuming(b)), (Some((ida, "base".into())), vec![None, None]));
    // Twin: a roll of A's frames and one of B's takes them, A keeping none and its draft gone.
    let mut rc = walked(&paths[..3], Step::Window);
    assert!(written(rc.drafts().unwrap(), &[]).is_empty());
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
            sidecar::set_roll_draft(Some(kept(id, "flags", Some(base.clone()), false)), Path::new(p), true).unwrap();
        }
        let loaded: Vec<_> = paths.iter().map(|p| sidecar::read(Path::new(p), true, false).ok()).collect();
        let mut r = roll(&paths);
        for (p, f) in paths.iter().zip(found(&paths, &loaded)) {
            r.resume(f.draft.as_ref(), p, &passes::Known::new());
        }
        r.base.map(|b| b.path)
    };
    assert_eq!(base_after(in_folder(&x)), Some(xs[1].clone()), "on X's frame, never Y's namesake");
    // Twin: a base stored under a folder of neither name is found on no frame, never guessed.
    assert_eq!(base_after(in_folder(Path::new("/Volumes/Old/Roll 12"))), None);
    let _ = (std::fs::remove_dir_all(&x), std::fs::remove_dir_all(&y));
}

#[test]
fn a_draft_an_earlier_build_wrote_after_the_folders_file_is_the_one_resumed() {
    let (dir, paths) = stood("fresher", 2);
    let mut r = walked(&paths, Step::Base);
    assert!(written(r.drafts().unwrap(), &[]).is_empty());
    let file = prepare::path(&dir);
    let set_back = |by: u64| File::options().write(true).open(&file).unwrap().set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(by)).unwrap();
    // An earlier build walks the roll on to its flags step in each frame's sidecar, an hour later.
    set_back(3600);
    let later = Id::new();
    for p in &paths {
        sidecar::set_roll_draft(Some(kept(later, "flags", None, false)), Path::new(p), true).unwrap();
    }
    assert_eq!(resuming(&paths), [Some((later, "flags".into())), Some((later, "flags".into()))]);
    // Twin: the folder's file written after them is the one resumed.
    File::options().write(true).open(&file).unwrap().set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(60)).unwrap();
    let mine = r.draft_id.unwrap();
    assert_eq!(resuming(&paths), [Some((mine, "base".into())), Some((mine, "base".into()))]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_step_whose_file_is_refused_keeps_the_frames_own_drafts_for_the_discard() {
    let (dir, paths) = stood("refused", 2);
    let id = Id::new();
    for p in &paths {
        sidecar::set_roll_draft(Some(kept(id, "flags", None, false)), Path::new(p), true).unwrap();
    }
    let mut r = walked(&paths, Step::Flags);
    r.stored = paths.iter().cloned().collect();
    // A folder in the file's place refuses the step's write.
    std::fs::create_dir(prepare::path(&dir)).unwrap();
    let (drafts, stored) = r.step_drafts().unwrap();
    assert_eq!(written(drafts, &stored).len(), 1);
    let own = |p: &String| sidecar::read(Path::new(p), true, false).unwrap().roll_draft.is_some();
    assert!(paths.iter().all(own), "a step refused clears no frame's own draft");
    std::fs::remove_dir(prepare::path(&dir)).unwrap();
    // Twin: had the step taken them, the discard would leave each frame's draft behind.
    assert!(written(r.dropped(), &[]).is_empty());
    assert!(paths.iter().all(own));
    let kept_for: Vec<String> = std::mem::take(&mut r.stored).into_iter().collect();
    assert!(written(r.dropped(), &kept_for).is_empty());
    assert!(!paths.iter().any(own), "the discard clears them");
    let _ = std::fs::remove_dir_all(&dir);
}
