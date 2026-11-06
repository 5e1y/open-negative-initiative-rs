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
    let mut h = History::default();
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

    let mut h2 = History::default();
    h2.record(
        &map(&[("/a", &edited), ("/b", &neutral)]),
        &map(&[("/a", &neutral), ("/b", &neutral)]),
        "Reset",
        None,
    );
    assert_eq!(h2.undo().unwrap().len(), 1, "an unchanged photo stays out of the step");
    // Twin: a gesture with no effect opens no step at all.
    let mut h3 = History::default();
    h3.record(&map(&[("/a", &neutral)]), &map(&[("/a", &neutral)]), "Reset", None);
    assert!(!h3.can_undo());

    let mut h4 = History::default();
    h4.record(&map(&[("/a", &edited)]), &map(&[("/a", &neutral)]), "un", None);
    h4.undo();
    h4.record(&map(&[("/a", &edited)]), &map(&[("/a", &neutral)]), "deux", None);
    assert!(h4.can_undo() && !h4.can_redo(), "a new gesture erases the undone branch");
}

#[test]
fn a_trash_step_comes_back_whole_and_an_empty_one_opens_none() {
    let trash = Trash { paths: BTreeSet::from(["/a".into(), "/b".into()]), marked: true };
    let mut h = History::default();
    h.record_trash(trash.clone(), "Move 2 Photos to ONI Trash");
    assert_eq!(h.undo_step().and_then(|s| s.trash), Some(trash));
    assert!(h.redo_step().and_then(|s| s.trash).is_some_and(|t| t.marked));
    // Twin: a restore that touched nothing opens no step.
    let mut empty = History::default();
    empty.record_trash(Trash { paths: BTreeSet::new(), marked: false }, "Restore");
    assert!(!empty.can_undo());
    let mut reset = History::default();
    reset.record(
        &Settings::new(),
        &Settings::new(),
        "Reset",
        Some(Trash { paths: BTreeSet::from(["/sheet.tif".into()]), marked: true }),
    );
    let mut bare = History::default();
    bare.record(&Settings::new(), &Settings::new(), "Reset", None);
    assert!(reset.can_undo() && !bare.can_undo());
}

#[test]
fn a_medal_step_keeps_what_each_photo_wore_and_one_that_changed_nothing_opens_none() {
    let worn = BTreeMap::from([("/a".to_string(), Medal::None), ("/b".to_string(), Medal::Gold)]);
    let mut h = History::default();
    h.record_medals(Medals { before: worn, after: Medal::Gold }, "Gold");
    let step = h.undo_step().and_then(|s| s.medals).expect("a medal step");
    assert_eq!(step.before, BTreeMap::from([("/a".to_string(), Medal::None)]), "a photo already gold stays out");
    assert_eq!(h.redo_paths(), BTreeSet::from(["/a".to_string()]));
    // Twin: the medal every photo already wears opens no step.
    let mut idle = History::default();
    idle.record_medals(Medals { before: BTreeMap::from([("/a".to_string(), Medal::Gold)]), after: Medal::Gold }, "Gold");
    assert!(!idle.can_undo());
}

#[test]
fn a_folder_renamed_after_a_new_roll_is_where_its_undo_walks_from() {
    let (neutral, edited) = pair();
    let mut h = History::default();
    h.record(&map(&[("/p/2026-10-26/A.NEF", &edited)]), &map(&[("/p/2026-10-26/A.NEF", &neutral)]), "Reset", None);
    h.record(&map(&[("/q/B.NEF", &edited)]), &map(&[("/q/B.NEF", &neutral)]), "Reset", None);
    let roll = Disk {
        folder: Some("/p/2026-10-26".into()),
        moves: vec![Move { from: "/p/A.NEF".into(), to: "/p/2026-10-26/A.NEF".into() }],
        ..Disk::default()
    };
    h.record_disk(roll, "New Roll");
    h.follow(&[Move { from: "/p/2026-10-26".into(), to: "/p/Portra".into() }]);
    let disk = h.undo_step().and_then(|s| s.disk).unwrap();
    assert_eq!(disk.moves, [Move { from: "/p/A.NEF".into(), to: "/p/Portra/A.NEF".into() }]);
    assert_eq!(disk.folder.as_deref(), Some("/p/Portra"));
    assert_eq!(h.undo_paths(), BTreeSet::from(["/q/B.NEF".to_string()]), "twin: a photo outside the folder keeps its path");
    h.undo_step();
    assert_eq!(h.undo_paths(), BTreeSet::from(["/p/Portra/A.NEF".to_string()]), "a settings step under it follows");
}

#[test]
fn the_stack_keeps_its_last_steps_up_to_its_limit() {
    let mut h = History::default();
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
    let mut idle = History::default();
    idle.record_disk(Disk::default(), "Move");
    assert!(!idle.can_undo());
}

#[test]
fn the_stack_keeps_its_steps_under_a_ceiling_of_bytes() {
    let (neutral, edited) = pair();
    let gesture = |h: &mut History, n: usize| {
        let paths: Vec<String> = (0..n).map(|i| format!("/r/{i}.NEF")).collect();
        let before: Settings = paths.iter().map(|p| (p.clone(), edited.clone())).collect();
        let after: Settings = paths.iter().map(|p| (p.clone(), neutral.clone())).collect();
        h.record(&before, &after, "Reset", None);
    };
    let mut probe = History::default();
    gesture(&mut probe, 50);
    let step = probe.bytes();
    let mut h = History::bounded(step * 3);
    for _ in 0..10 {
        gesture(&mut h, 50);
    }
    let mut undone = 0;
    while h.undo_step().is_some() {
        undone += 1;
    }
    assert_eq!(undone, 3, "three steps of {step} bytes fill the ceiling");
    // The newest step stays, even past the ceiling alone.
    let mut tight = History::bounded(1);
    gesture(&mut tight, 50);
    assert!(tight.can_undo());
    // Twin: counted by steps alone, the ten gestures would all be kept.
    let mut counted = History::bounded(usize::MAX);
    for _ in 0..10 {
        gesture(&mut counted, 50);
    }
    assert_eq!(counted.bytes(), step * 10);
}

fn exposed(by: f32) -> PipelineSettings {
    let mut s = PipelineSettings::default();
    s.print.exposure += by;
    s
}

/// An edit of `path` from `from` to `to`, at `at` seconds.
fn edit(h: &mut History, path: &str, from: &PipelineSettings, to: &PipelineSettings, at: f64) {
    h.record_edit(path, map(&[(path, from)]), map(&[(path, to)]), at);
}

/// How many steps the stack holds, each undone to count it.
fn depth(h: &mut History) -> usize {
    let mut steps = 0;
    while h.undo_step().is_some() {
        steps += 1;
    }
    steps
}

#[test]
fn a_drag_within_the_window_is_one_step() {
    let mut h = History::default();
    for (i, at) in [0.0, 0.1, 0.2, 0.35].into_iter().enumerate() {
        edit(&mut h, "/a", &exposed(0.1 * i as f32), &exposed(0.1 * (i + 1) as f32), at);
    }
    assert_eq!(h.undo_label(), None, "an edit is no named gesture");
    assert_eq!(h.undo(), Some(map(&[("/a", &exposed(0.0))])), "one undo takes the whole drag back");
    // Twin: the same notches spaced past the window are four steps.
    let mut spaced = History::default();
    for (i, at) in [0.0, 0.5, 1.0, 1.5].into_iter().enumerate() {
        edit(&mut spaced, "/a", &exposed(0.1 * i as f32), &exposed(0.1 * (i + 1) as f32), at);
    }
    assert_eq!(depth(&mut spaced), 4);
}

#[test]
fn two_settings_at_once_never_coalesce() {
    let mut both = exposed(0.2);
    both.sharpen.amount = 1.0;
    let mut again = both.clone();
    again.print.exposure += 0.1;
    again.sharpen.amount = 1.2;
    let mut h = History::default();
    edit(&mut h, "/a", &PipelineSettings::default(), &both, 0.0);
    edit(&mut h, "/a", &both, &again, 0.1);
    assert_eq!(depth(&mut h), 2);
    // Twin: one setting twice in the window joins.
    let mut one = History::default();
    edit(&mut one, "/a", &exposed(0.0), &exposed(0.2), 0.0);
    edit(&mut one, "/a", &exposed(0.2), &exposed(0.3), 0.1);
    assert_eq!(depth(&mut one), 1);
}

#[test]
fn an_edit_after_an_undo_drops_the_redo_branch_and_never_joins_the_past() {
    let mut h = History::default();
    edit(&mut h, "/a", &exposed(0.0), &exposed(0.1), 0.0);
    edit(&mut h, "/a", &exposed(0.1), &exposed(0.2), 1.0);
    h.undo();
    edit(&mut h, "/a", &exposed(0.1), &exposed(0.5), 1.1);
    assert!(!h.can_redo());
    assert_eq!(h.undo(), Some(map(&[("/a", &exposed(0.1))])));
    // Twin: without the undo the same notch within the window joins the top.
    let mut kept = History::default();
    edit(&mut kept, "/a", &exposed(0.0), &exposed(0.1), 0.0);
    edit(&mut kept, "/a", &exposed(0.1), &exposed(0.2), 1.0);
    edit(&mut kept, "/a", &exposed(0.2), &exposed(0.5), 1.1);
    assert_eq!(depth(&mut kept), 2);
}

#[test]
fn an_edit_joins_only_its_own_frame_and_a_gesture_between_cuts_it() {
    let mut h = History::default();
    edit(&mut h, "/a", &exposed(0.0), &exposed(0.1), 0.0);
    edit(&mut h, "/b", &exposed(0.0), &exposed(0.1), 0.1);
    assert_eq!(depth(&mut h), 2, "the same setting on another frame is a step of its own");
    let mut cut = History::default();
    edit(&mut cut, "/a", &exposed(0.0), &exposed(0.1), 0.0);
    cut.record_medals(Medals { before: BTreeMap::from([("/b".to_string(), Medal::None)]), after: Medal::Gold }, "Medal");
    edit(&mut cut, "/a", &exposed(0.1), &exposed(0.2), 0.1);
    assert_eq!(depth(&mut cut), 3, "a gesture between two edits keeps them apart");
    // Twin: the same two edits with nothing between are one step.
    let mut joined = History::default();
    edit(&mut joined, "/a", &exposed(0.0), &exposed(0.1), 0.0);
    edit(&mut joined, "/a", &exposed(0.1), &exposed(0.2), 0.1);
    assert_eq!(depth(&mut joined), 1);
}

#[test]
fn the_frames_an_edit_wrote_keep_their_first_before_and_their_last_after() {
    let mut h = History::default();
    let with = |open: f32, other: f32| map(&[("/a", &exposed(open)), ("/b", &exposed(other))]);
    h.record_edit("/a", with(0.0, 0.0), with(0.1, 0.1), 0.0);
    h.record_edit("/a", with(0.1, 0.1), with(0.2, 0.2), 0.1);
    let step = h.undo_step().unwrap();
    assert_eq!((step.before, step.after), (with(0.0, 0.0), with(0.2, 0.2)));
    assert!(!h.can_undo(), "one step");
    // Twin: an edit that wrote other frames alone, the open one unchanged, never joins.
    let mut alone = History::default();
    alone.record_edit("/a", map(&[("/b", &exposed(0.0))]), map(&[("/b", &exposed(0.1))]), 0.0);
    alone.record_edit("/a", map(&[("/b", &exposed(0.1))]), map(&[("/b", &exposed(0.2))]), 0.1);
    assert_eq!(depth(&mut alone), 2);
}
