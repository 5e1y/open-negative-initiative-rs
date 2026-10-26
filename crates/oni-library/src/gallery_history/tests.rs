use super::*;

fn pair() -> (PipelineSettings, PipelineSettings) {
    let mut neutral = PipelineSettings::default();
    neutral.gains.stops = [0.0, 0.0, 0.0];
    let mut edited = PipelineSettings::default();
    edited.gains.stops = [1.0, 0.0, -1.0];
    (neutral, edited)
}

fn map(items: &[(&str, &PipelineSettings)]) -> Settings {
    items.iter().map(|(p, s)| (p.to_string(), (*s).clone())).collect()
}

#[test]
fn a_gesture_undoes_and_redoes_whole_and_an_idle_one_opens_no_step() {
    let (neutral, edited) = pair();
    let mut h = GalleryHistory::default();
    assert!(!h.can_undo() && !h.can_redo());
    h.record(
        &map(&[("/a", &edited), ("/b", &edited)]),
        &map(&[("/a", &neutral), ("/b", &neutral)]),
        "Reset 2 photos",
        None,
    );
    assert_eq!(h.undo_label(), Some("Reset 2 photos"));
    let undone = h.undo().unwrap();
    assert_eq!((undone.len(), undone.get("/a")), (2, Some(&edited)));
    assert!(!h.can_undo() && h.can_redo());
    assert_eq!(h.redo().unwrap().get("/a"), Some(&neutral));

    let mut h2 = GalleryHistory::default();
    h2.record(
        &map(&[("/a", &edited), ("/b", &neutral)]),
        &map(&[("/a", &neutral), ("/b", &neutral)]),
        "Reset",
        None,
    );
    assert_eq!(h2.undo().unwrap().len(), 1, "an unchanged photo stays out of the step");
    // Twin: a gesture with no effect opens no step at all.
    let mut h3 = GalleryHistory::default();
    h3.record(&map(&[("/a", &neutral)]), &map(&[("/a", &neutral)]), "Reset", None);
    assert!(!h3.can_undo());

    let mut h4 = GalleryHistory::default();
    h4.record(&map(&[("/a", &edited)]), &map(&[("/a", &neutral)]), "un", None);
    h4.undo();
    h4.record(&map(&[("/a", &edited)]), &map(&[("/a", &neutral)]), "deux", None);
    assert!(h4.can_undo() && !h4.can_redo(), "a new gesture erases the undone branch");
}

#[test]
fn a_trash_step_comes_back_whole_and_an_empty_one_opens_none() {
    let trash = Trash { paths: BTreeSet::from(["/a".into(), "/b".into()]), marked: true };
    let mut h = GalleryHistory::default();
    h.record_trash(trash.clone(), "Move 2 Photos to ONI Trash");
    assert_eq!(h.undo_step().and_then(|s| s.trash), Some(trash));
    assert!(h.redo_step().and_then(|s| s.trash).is_some_and(|t| t.marked));
    // Twin: a restore that touched nothing opens no step.
    let mut empty = GalleryHistory::default();
    empty.record_trash(Trash { paths: BTreeSet::new(), marked: false }, "Restore");
    assert!(!empty.can_undo());
    let mut reset = GalleryHistory::default();
    reset.record(
        &Settings::new(),
        &Settings::new(),
        "Reset",
        Some(Trash { paths: BTreeSet::from(["/sheet.tif".into()]), marked: true }),
    );
    let mut bare = GalleryHistory::default();
    bare.record(&Settings::new(), &Settings::new(), "Reset", None);
    assert!(reset.can_undo() && !bare.can_undo());
}

#[test]
fn a_folder_renamed_after_a_new_roll_is_where_its_undo_walks_from() {
    let (neutral, edited) = pair();
    let mut h = GalleryHistory::default();
    h.record(&map(&[("/p/2026-10-26/A.NEF", &edited)]), &map(&[("/p/2026-10-26/A.NEF", &neutral)]), "Reset", None);
    h.record(&map(&[("/q/B.NEF", &edited)]), &map(&[("/q/B.NEF", &neutral)]), "Reset", None);
    let roll = Disk {
        folder: Some("/p/2026-10-26".into()),
        moves: vec![Move { from: "/p/A.NEF".into(), to: "/p/2026-10-26/A.NEF".into() }],
        ..Disk::default()
    };
    h.record_disk(roll, "New Roll");
    h.follow(&[Move { from: "/p/2026-10-26".into(), to: "/p/Portra".into() }]);
    assert_eq!(h.undo_moves(), [Move { from: "/p/Portra/A.NEF".into(), to: "/p/A.NEF".into() }]);
    assert_eq!(h.undo_step().and_then(|s| s.disk).and_then(|d| d.folder).as_deref(), Some("/p/Portra"));
    assert_eq!(h.undo_paths(), BTreeSet::from(["/q/B.NEF".to_string()]), "twin: a photo outside the folder keeps its path");
    h.undo_step();
    assert_eq!(h.undo_paths(), BTreeSet::from(["/p/Portra/A.NEF".to_string()]), "a settings step under it follows");
}

#[test]
fn the_stack_keeps_its_last_hundred_steps() {
    let mut h = GalleryHistory::default();
    for i in 0..(LIMIT + 5) {
        h.record_disk(
            Disk { moves: vec![Move { from: format!("/a{i}"), to: "/b".into() }], ..Disk::default() },
            &i.to_string(),
        );
    }
    let mut undone = 0;
    while h.undo_step().is_some() {
        undone += 1;
    }
    assert_eq!(undone, LIMIT);
    // Twin: a disk gesture that moved nothing adds no step.
    let mut idle = GalleryHistory::default();
    idle.record_disk(Disk::default(), "Move");
    assert!(!idle.can_undo());
}

#[test]
fn the_stack_keeps_its_steps_under_a_ceiling_of_bytes() {
    let (neutral, edited) = pair();
    let gesture = |h: &mut GalleryHistory, n: usize| {
        let paths: Vec<String> = (0..n).map(|i| format!("/r/{i}.NEF")).collect();
        let before: Settings = paths.iter().map(|p| (p.clone(), edited.clone())).collect();
        let after: Settings = paths.iter().map(|p| (p.clone(), neutral.clone())).collect();
        h.record(&before, &after, "Reset", None);
    };
    let mut probe = GalleryHistory::default();
    gesture(&mut probe, 50);
    let step = probe.bytes();
    let mut h = GalleryHistory::bounded(step * 3);
    for _ in 0..10 {
        gesture(&mut h, 50);
    }
    let mut undone = 0;
    while h.undo_step().is_some() {
        undone += 1;
    }
    assert_eq!(undone, 3, "three steps of {step} bytes fill the ceiling");
    // The newest step stays, even past the ceiling alone.
    let mut tight = GalleryHistory::bounded(1);
    gesture(&mut tight, 50);
    assert!(tight.can_undo());
    // Twin: counted by steps alone, the ten gestures would all be kept.
    let mut counted = GalleryHistory::bounded(usize::MAX);
    for _ in 0..10 {
        gesture(&mut counted, 50);
    }
    assert_eq!(counted.bytes(), step * 10);
}
